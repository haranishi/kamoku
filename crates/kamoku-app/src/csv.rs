// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! CSVの書き出し（`docs/spec/api-reports.md` の「書き出し」）。RFC 4180 の引用符、改行は CRLF、
//! 文字コードは BOM つき UTF-8（既定）か `Shift_JIS`。

use encoding_rs::{EncoderResult, SHIFT_JIS};

/// 1つの欄。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Cell {
    /// 文字列の欄。表計算ソフトが式として実行しないよう、危ない先頭の文字に `'` を足す。
    Text(String),
    /// 数値の欄（式の防止の対象外）。
    Num(i64),
}

pub(crate) fn text(value: impl Into<String>) -> Cell {
    Cell::Text(value.into())
}

pub(crate) const fn num(value: i64) -> Cell {
    Cell::Num(value)
}

/// 文字コード。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    /// BOMつきUTF-8（Excelで文字化けしない）。
    Utf8Bom,
    /// `Shift_JIS`（Windows-31J）。表せない文字は `?` にする。
    Sjis,
}

/// 書き出した結果。
pub(crate) struct Encoded {
    pub bytes: Vec<u8>,
    /// `Shift_JIS` で表せずに `?` にした文字の数。
    pub unmappable: usize,
}

// 先頭が `=` `+` `-` `@`、タブ、CR の文字列は、表計算ソフトが式として読む。
fn guard(value: &str) -> String {
    if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

fn quote(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// CSVの文字列を組み立てる道具。
#[derive(Debug, Default)]
pub(crate) struct Writer {
    out: String,
}
impl Writer {
    pub(crate) fn row(&mut self, cells: &[Cell]) {
        let fields: Vec<String> = cells
            .iter()
            .map(|cell| match cell {
                Cell::Text(value) => quote(&guard(value)),
                Cell::Num(value) => value.to_string(),
            })
            .collect();
        self.out.push_str(&fields.join(","));
        self.out.push_str("\r\n");
    }

    pub(crate) fn finish(self, encoding: Encoding) -> Encoded {
        match encoding {
            Encoding::Utf8Bom => {
                let mut bytes = Vec::with_capacity(self.out.len() + 3);
                bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
                bytes.extend_from_slice(self.out.as_bytes());
                Encoded {
                    bytes,
                    unmappable: 0,
                }
            }
            Encoding::Sjis => to_sjis(&self.out),
        }
    }
}

fn to_sjis(text: &str) -> Encoded {
    let mut encoder = SHIFT_JIS.new_encoder();
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text;
    let mut unmappable = 0usize;
    loop {
        let room = encoder
            .max_buffer_length_from_utf8_without_replacement(rest.len())
            .unwrap_or(rest.len().saturating_mul(2))
            .max(16);
        bytes.reserve(room);
        let (result, read) =
            encoder.encode_from_utf8_to_vec_without_replacement(rest, &mut bytes, true);
        rest = rest.get(read..).unwrap_or_default();
        match result {
            EncoderResult::InputEmpty => {
                return Encoded { bytes, unmappable };
            }
            EncoderResult::OutputFull => {}
            EncoderResult::Unmappable(_) => {
                bytes.push(b'?');
                unmappable = unmappable.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv(cells: &[Cell]) -> String {
        let mut w = Writer::default();
        w.row(cells);
        String::from_utf8(w.finish(Encoding::Utf8Bom).bytes.split_off(3)).unwrap()
    }

    #[test]
    fn quotes_like_rfc_4180_with_crlf() {
        assert_eq!(
            csv(&[text("a,b"), text("c\"d"), text("e\nf"), num(-5)]),
            "\"a,b\",\"c\"\"d\",\"e\nf\",-5\r\n"
        );
    }

    #[test]
    fn guards_formula_heads_but_not_numbers() {
        assert_eq!(
            csv(&[text("=1+1"), text("+1"), text("-1"), text("@x"), num(-1200)]),
            "'=1+1,'+1,'-1,'@x,-1200\r\n"
        );
        assert_eq!(csv(&[text("\tx"), text("\rx")]), "'\tx,\"'\rx\"\r\n");
        assert_eq!(csv(&[text("x=1")]), "x=1\r\n");
    }

    #[test]
    fn utf8_has_bom_and_sjis_marks_unmappable() {
        let mut w = Writer::default();
        w.row(&[text("通信費"), text("😀")]);
        let utf8 = Writer { out: w.out.clone() }.finish(Encoding::Utf8Bom);
        assert!(utf8.bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let sjis = w.finish(Encoding::Sjis);
        assert_eq!(sjis.unmappable, 1);
        let (decoded, _, _) = SHIFT_JIS.decode(&sjis.bytes);
        assert_eq!(decoded, "通信費,?\r\n");
    }
}
