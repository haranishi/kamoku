// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! CSVの読み取り。閉じ引用符の後は区切り・改行・終端だけを受け付ける。

use crate::ImportError;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;

/// 1ファイルの行の上限。
pub const MAX_ROWS: usize = 200_000;
/// 1つの欄の上限（バイト）。
pub const MAX_CELL_BYTES: usize = 64 * 1024;
const MAX_COLUMNS: usize = 1000;
const MAX_CELLS: usize = 2_000_000;
const SNIFF_RECORDS: usize = 20;

/// 区切りの文字。JSONでは `","` `"\t"` `";"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Delimiter {
    /// カンマ。
    #[serde(rename = ",")]
    Comma,
    /// タブ。
    #[serde(rename = "\t")]
    Tab,
    /// セミコロン。
    #[serde(rename = ";")]
    Semicolon,
}
impl Delimiter {
    /// 判定で試す順（同点なら前のもの）。
    pub const ALL: [Self; 3] = [Self::Comma, Self::Tab, Self::Semicolon];
    /// 区切りの文字。
    #[must_use]
    pub const fn as_char(self) -> char {
        match self {
            Self::Comma => ',',
            Self::Tab => '\t',
            Self::Semicolon => ';',
        }
    }
}

/// 1行（1レコード）。引用符の中の改行を含むと、物理的な行は複数になる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// レコードが始まった行（1始まり）。
    pub line: usize,
    /// 欄。
    pub cells: Vec<String>,
}
/// 読んだ表。空の行（欄がすべて空か空白だけ）は含まない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Table {
    /// 判定した区切り。
    pub delimiter: Delimiter,
    /// 行。
    pub rows: Vec<Row>,
}

/// CSVを読みます。区切りは、先頭の行を `,` タブ `;` で読み比べて、欄の数がそろうものを選びます。
/// 引用符の中の改行・区切り・二重の引用符、CRLF/LF/CR、空の行、行末の空白を扱います。
///
/// # Errors
/// 閉じていない引用符（行番号つき）、行や欄の上限超え、行が無いときに返します。
pub fn parse_csv(text: &str) -> Result<Table, ImportError> {
    let delimiter = detect_delimiter(text);
    let parsed = parse_with(text, delimiter.as_char(), None);
    if let Some(error) = parsed.error {
        return Err(error);
    }
    if parsed.rows.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(Table {
        delimiter,
        rows: parsed.rows,
    })
}

// （引用符の崩れの数の逆順, 欄の数がそろう行の数, 欄の数）。大きいほど区切りらしい。
type Score = (Reverse<usize>, usize, usize);

// 区切りを取り違えると、引用符の読み方が崩れる（閉じた引用符の後に文字が続く、引用符が欄の途中に出る）。
// そこで、崩れの少なさ、欄の数がそろう行の多さ、欄の数の順に比べる。欄が1つしかない区切りは選ばない。
fn detect_delimiter(text: &str) -> Delimiter {
    let mut best: Option<(Delimiter, Score)> = None;
    let mut bounded_candidate = None;
    for delimiter in Delimiter::ALL {
        // 途中で壊れていても、読めた分で比べる。
        let parsed = parse_with(text, delimiter.as_char(), Some(SNIFF_RECORDS));
        if matches!(parsed.error, Some(ImportError::TooManyColumns { .. })) {
            bounded_candidate.get_or_insert(delimiter);
        }
        let widths: Vec<usize> = parsed.rows.iter().map(|r| r.cells.len()).collect();
        let Some((same, width)) = widths
            .iter()
            .copied()
            .filter(|w| *w > 1)
            .map(|w| (widths.iter().filter(|&&c| c == w).count(), w))
            .max()
        else {
            continue;
        };
        let anomalies = parsed
            .anomalies
            .saturating_add(usize::from(parsed.error.is_some()));
        let score = (Reverse(anomalies), same, width);
        if best.as_ref().is_none_or(|(_, current)| score > *current) {
            best = Some((delimiter, score));
        }
    }
    best.map_or(
        bounded_candidate.unwrap_or(Delimiter::Comma),
        |(delimiter, _)| delimiter,
    )
}

struct Parsed {
    rows: Vec<Row>,
    error: Option<ImportError>,
    anomalies: usize,
}

#[derive(Default)]
struct Builder {
    rows: Vec<Row>,
    cells: Vec<String>,
    field: String,
    quoted: bool,
    after_quote: bool,
    record_line: usize,
    total_cells: usize,
    anomalies: usize,
}
impl Builder {
    fn end_field(&mut self, end_of_record: bool, delimiter: char) -> Result<(), ImportError> {
        if self.cells.len() >= MAX_COLUMNS {
            return Err(ImportError::TooManyColumns {
                line: self.record_line,
            });
        }
        if self.total_cells >= MAX_CELLS {
            return Err(ImportError::TooManyCells {
                line: self.record_line,
            });
        }
        self.total_cells += 1;
        if end_of_record && !self.quoted {
            // 行末の空白は、表計算ソフトが足すことがあるので落とす（引用符の中は残す）。
            let kept = self
                .field
                .trim_end_matches(|c| c == ' ' || (c == '\t' && delimiter != '\t'))
                .len();
            self.field.truncate(kept);
        }
        self.cells.push(std::mem::take(&mut self.field));
        self.quoted = false;
        self.after_quote = false;
        Ok(())
    }
    fn finish(self, error: Option<ImportError>) -> Parsed {
        Parsed {
            rows: self.rows,
            error,
            anomalies: self.anomalies,
        }
    }
    fn end_record(&mut self, delimiter: char, next_line: usize) -> Option<ImportError> {
        if let Err(error) = self.end_field(true, delimiter) {
            return Some(error);
        }
        let cells = std::mem::take(&mut self.cells);
        if !cells.iter().all(|c| c.trim().is_empty()) {
            self.rows.push(Row {
                line: self.record_line,
                cells,
            });
        }
        self.record_line = next_line;
        (self.rows.len() > MAX_ROWS).then_some(ImportError::TooManyRows)
    }
}

/// 読めた行と、最初の失敗と、引用符の崩れの数。`limit` があれば、その数の行で止める（区切りの判定用）。
fn parse_with(text: &str, delimiter: char, limit: Option<usize>) -> Parsed {
    let mut b = Builder {
        record_line: 1,
        ..Builder::default()
    };
    let mut line = 1usize;
    let mut in_quotes = false;
    let mut quote_line = 1usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if limit.is_some_and(|n| b.rows.len() >= n) {
            return b.finish(None);
        }
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    b.field.push('"');
                }
                '"' => {
                    in_quotes = false;
                    b.after_quote = true;
                }
                '\r' | '\n' => {
                    b.field.push(c);
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                        b.field.push('\n');
                    }
                    line += 1;
                }
                other => b.field.push(other),
            }
        } else {
            if b.after_quote && c != delimiter && !matches!(c, '\r' | '\n') {
                let line = b.record_line;
                return b.finish(Some(ImportError::InvalidQuote { line }));
            }
            match c {
                '"' if b.field.is_empty() && !b.quoted && !b.after_quote => {
                    in_quotes = true;
                    b.quoted = true;
                    quote_line = line;
                }
                '\r' | '\n' => {
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    line += 1;
                    if let Some(error) = b.end_record(delimiter, line) {
                        return b.finish(Some(error));
                    }
                }
                d if d == delimiter => {
                    if let Err(error) = b.end_field(false, delimiter) {
                        return b.finish(Some(error));
                    }
                }
                other => {
                    if other == '"' || b.after_quote {
                        b.anomalies = b.anomalies.saturating_add(1);
                    }
                    b.field.push(other);
                }
            }
        }
        if b.field.len() > MAX_CELL_BYTES {
            let line = b.record_line;
            return b.finish(Some(ImportError::CellTooLarge { line }));
        }
    }
    if in_quotes {
        return b.finish(Some(ImportError::UnclosedQuote { line: quote_line }));
    }
    if b.cells.is_empty() && b.field.is_empty() && !b.quoted {
        return b.finish(None);
    }
    let error = b.end_record(delimiter, line + 1);
    b.finish(error)
}
