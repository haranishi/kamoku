// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 銀行・カードの明細CSVを読む。文字コードの判定、列の対応、重複の判定に使う指紋を作る。入出力を持たない。
//!
//! 流れは [`decode`]（バイト列を文字に）→ [`parse_csv`]（表に）→ [`guess_mapping`]（列の対応の初期値）
//! → [`apply_mapping`]（行ごとに日付・摘要・金額を読む）→ [`number_occurrences`]（指紋と出現番号）。
//! 下書きの表に入れて重複を断るのは、保存の担当（`kamoku-store`）の仕事です。

mod csv;
mod decode;
mod error;
mod fingerprint;
mod mapping;
mod values;

pub use csv::{Delimiter, MAX_CELL_BYTES, MAX_ROWS, Row, Table, parse_csv};
pub use decode::{Decoded, EncodingChoice, TextEncoding, decode};
pub use error::ImportError;
pub use fingerprint::{fingerprint, number_occurrences};
pub use mapping::{
    AmountMapping, Direction, Inflow, MAX_AMOUNT, Mapping, ParsedRow, RowOutcome, apply_mapping,
    guess_mapping,
};
pub use values::{AmountError, normalize_text, parse_amount, parse_date};
