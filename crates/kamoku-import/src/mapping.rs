// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 列の対応（`docs/spec/api-inbox.md` の `mapping`）と、行ごとの読み取り。

use crate::values::{normalize_text, parse_amount, parse_date};
use crate::{Row, Table};
use serde::{Deserialize, Serialize, Serializer};
use time::Date;

/// 1明細の金額の上限（台帳と同じ）。
pub const MAX_AMOUNT: i64 = 9_999_999_999_999;
const HEADER_SEARCH_ROWS: usize = 10;

/// 入金がどちらの符号か（符号つきの1列のとき）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Inflow {
    /// 正の数が入金。
    Positive,
    /// 負の数が入金（カードの明細のように、支払いを正で書くもの）。
    Negative,
}
/// 金額の列。JSONでは `{ "mode": "split", "out_col", "in_col" }` か `{ "mode": "signed", "col", "inflow" }`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum AmountMapping {
    /// 出金と入金が別の列。
    Split {
        /// 出金の列（0始まり）。
        out_col: usize,
        /// 入金の列（0始まり）。
        in_col: usize,
    },
    /// 符号つきの1列。
    Signed {
        /// 金額の列（0始まり）。
        col: usize,
        /// 入金の符号。
        inflow: Inflow,
    },
}
/// 列の対応。列の番号は0始まり。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Mapping {
    /// 先頭から飛ばす行の数（見出しなど）。
    pub skip_rows: usize,
    /// 日付の列。
    pub date_col: usize,
    /// 摘要の列（複数なら1つの空白で連ねる）。
    #[serde(default)]
    pub desc_cols: Vec<usize>,
    /// 金融機関が付けた取引番号の列（任意）。あれば、重複の判定に使う。
    #[serde(default)]
    pub id_col: Option<usize>,
    /// 金額の列。
    pub amount: AmountMapping,
}

/// お金の向き。JSONでは `in` `out`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// 入金。
    In,
    /// 出金。
    Out,
}
/// 読めた行。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ParsedRow {
    /// ファイルの中の行（1始まり）。
    pub line: usize,
    /// 取引の日付。
    #[serde(serialize_with = "date_str")]
    pub date: Date,
    /// 正規化した摘要。
    pub description: String,
    /// お金の向き。
    pub direction: Direction,
    /// 金額（正、円）。
    pub amount: i64,
    /// 正規化した取引番号（列の指定があり、空でないとき）。
    pub id: Option<String>,
}
impl ParsedRow {
    /// 符号つきの金額（入金は正、出金は負）。
    #[must_use]
    pub const fn signed_amount(&self) -> i64 {
        match self.direction {
            Direction::In => self.amount,
            Direction::Out => self.amount.saturating_neg(),
        }
    }
}
// serialize_with は参照を渡すので、値渡しにできない。
#[allow(clippy::trivially_copy_pass_by_ref)]
fn date_str<S: Serializer>(date: &Date, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    ))
}
/// 1行の読み取りの結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowOutcome {
    /// 読めた。
    Parsed(ParsedRow),
    /// 読めない（取り込みでは飛ばし、一覧で理由を見せる）。
    Invalid {
        /// ファイルの中の行（1始まり）。
        line: usize,
        /// 理由の日本語。
        reason: String,
    },
}

const DATE_WORDS: [&str; 4] = ["日付", "年月日", "ご利用日", "取引日"];
const DESC_WORDS: [&str; 4] = ["摘要", "内容", "ご利用先", "お取引内容"];
const OUT_WORDS: [&str; 3] = ["出金", "お支払金額", "引出"];
const IN_WORDS: [&str; 3] = ["入金", "お預り金額", "預入"];
const AMOUNT_WORDS: [&str; 1] = ["金額"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Date,
    Desc,
    Out,
    In,
    Amount,
}
fn role(cell: &str) -> Option<Role> {
    let cell = normalize_text(cell).replace(' ', "");
    let has = |words: &[&str]| words.iter().any(|w| cell.contains(w));
    // 「お支払金額」「お預り金額」は「金額」も含むので、出金と入金を先に見る。
    [
        (Role::Out, &OUT_WORDS[..]),
        (Role::In, &IN_WORDS[..]),
        (Role::Date, &DATE_WORDS[..]),
        (Role::Desc, &DESC_WORDS[..]),
        (Role::Amount, &AMOUNT_WORDS[..]),
    ]
    .into_iter()
    .find(|(_, words)| has(words))
    .map(|(r, _)| r)
}

/// 見出しの語から列の対応を推測します。先頭の10行から、日付の列と、金額（出金と入金、または
/// 金額の1列）の列がある行を見出しとみなします。金額が1列なら、カードの明細のように正の数を
/// 出金とみなします（`inflow: negative`）。見つからなければ `None`。取引番号の列は推測しません。
#[must_use]
pub fn guess_mapping(table: &Table) -> Option<Mapping> {
    for (index, row) in table.rows.iter().take(HEADER_SEARCH_ROWS).enumerate() {
        let roles: Vec<Option<Role>> = row.cells.iter().map(|c| role(c)).collect();
        let find = |r: Role| roles.iter().position(|x| *x == Some(r));
        let Some(date_col) = find(Role::Date) else {
            continue;
        };
        let amount = match (find(Role::Out), find(Role::In), find(Role::Amount)) {
            (Some(out_col), Some(in_col), _) => AmountMapping::Split { out_col, in_col },
            (_, _, Some(col)) => AmountMapping::Signed {
                col,
                inflow: Inflow::Negative,
            },
            _ => continue,
        };
        let desc_cols = (0..roles.len())
            .filter(|&i| roles.get(i) == Some(&Some(Role::Desc)))
            .collect();
        return Some(Mapping {
            skip_rows: index + 1,
            date_col,
            desc_cols,
            id_col: None,
            amount,
        });
    }
    None
}

/// 列の対応に従って、見出しの後の各行を読みます。日付が読めない、入金と出金の両方に金額がある、
/// どちらも空か0円、小数点がある行などは、理由つきの `Invalid` にします。
#[must_use]
pub fn apply_mapping(table: &Table, mapping: &Mapping) -> Vec<RowOutcome> {
    table
        .rows
        .iter()
        .skip(mapping.skip_rows)
        .map(|row| match read_row(row, mapping) {
            Ok(parsed) => RowOutcome::Parsed(parsed),
            Err(reason) => RowOutcome::Invalid {
                line: row.line,
                reason,
            },
        })
        .collect()
}

fn read_row(row: &Row, mapping: &Mapping) -> Result<ParsedRow, String> {
    if mapping.desc_cols.len() > 8 {
        return Err("摘要の列の指定が多すぎます（8個まで）".into());
    }
    let cell = |col: usize| row.cells.get(col).map_or("", String::as_str);
    let date_text = cell(mapping.date_col).trim();
    let date = parse_date(date_text).ok_or_else(|| {
        if date_text.is_empty() {
            "日付が空です。".to_owned()
        } else {
            format!("日付「{date_text}」を読めません（実在しない日を含む）。")
        }
    })?;
    let (direction, amount) = match mapping.amount {
        AmountMapping::Split { out_col, in_col } => {
            let out = parse_amount(cell(out_col)).map_err(|e| e.reason().to_owned())?;
            let inn = parse_amount(cell(in_col)).map_err(|e| e.reason().to_owned())?;
            if out.is_some_and(|v| v < 0) || inn.is_some_and(|v| v < 0) {
                return Err(
                    "入金・出金の列に負の金額があります。符号つきの1列の形を選んでください。"
                        .to_owned(),
                );
            }
            match (out.filter(|v| *v != 0), inn.filter(|v| *v != 0)) {
                (Some(_), Some(_)) => return Err("入金と出金の両方に金額があります。".to_owned()),
                (None, None) => return Err("入金と出金が、どちらも空か0円です。".to_owned()),
                (Some(v), None) => (Direction::Out, v),
                (None, Some(v)) => (Direction::In, v),
            }
        }
        AmountMapping::Signed { col, inflow } => {
            let value = parse_amount(cell(col))
                .map_err(|e| e.reason().to_owned())?
                .filter(|v| *v != 0)
                .ok_or_else(|| "金額が空か0円です。".to_owned())?;
            let positive_is_in = inflow == Inflow::Positive;
            let direction = if (value > 0) == positive_is_in {
                Direction::In
            } else {
                Direction::Out
            };
            (direction, value.abs())
        }
    };
    if amount > MAX_AMOUNT {
        return Err("金額が大きすぎます（9,999,999,999,999円まで）。".to_owned());
    }
    let too_long = || "摘要が長すぎます（1000バイトまで）".to_owned();
    let mut joined = String::new();
    for (index, &column) in mapping.desc_cols.iter().enumerate() {
        let value = cell(column);
        let separator = usize::from(index > 0);
        if joined.len() + separator > 1000 || value.len() > 1000 - joined.len() - separator {
            return Err(too_long());
        }
        if separator > 0 {
            joined.push(' ');
        }
        joined.push_str(value);
    }
    let description = normalize_text(&joined);
    if description.len() > 1000 {
        return Err(too_long());
    }
    let id = mapping
        .id_col
        .map(|c| normalize_text(cell(c)))
        .filter(|s| !s.is_empty());
    Ok(ParsedRow {
        line: row.line,
        date,
        description,
        direction,
        amount,
        id,
    })
}
