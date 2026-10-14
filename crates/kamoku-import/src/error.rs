// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

/// CSVを読めなかった理由。通信では、どれも `400 invalid_csv` になる。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImportError {
    /// 引用符（`"`）が閉じていない。
    UnclosedQuote {
        /// 閉じていない引用符が始まった行（1始まり）。
        line: usize,
    },
    /// 閉じ引用符の後に区切り・改行以外の文字がある。
    InvalidQuote {
        /// 不正な欄が始まった行（1始まり）。
        line: usize,
    },
    /// 1行の列数が1000を超える。
    TooManyColumns {
        /// 上限を超えた行（1始まり）。
        line: usize,
    },
    /// ファイル全体のセル数が200万を超える。
    TooManyCells {
        /// 上限を超えた行（1始まり）。
        line: usize,
    },
    /// 行が20万を超える。
    TooManyRows,
    /// 1つの欄が64KBを超える。
    CellTooLarge {
        /// その欄が始まった行（1始まり）。
        line: usize,
    },
    /// 行が1つも無い。
    Empty,
    /// 文字のファイルではない（NUL文字を含む。表計算ソフトの形式など）。
    NotText,
}
impl ImportError {
    /// 失敗の `code`。
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "invalid_csv"
    }
    /// 問題のある行（1始まり）。行に結びつかない失敗は `None`。
    #[must_use]
    pub const fn line(&self) -> Option<usize> {
        match self {
            Self::UnclosedQuote { line }
            | Self::CellTooLarge { line }
            | Self::InvalidQuote { line }
            | Self::TooManyColumns { line }
            | Self::TooManyCells { line } => Some(*line),
            Self::TooManyRows | Self::Empty | Self::NotText => None,
        }
    }
    /// 原因と直し方の日本語。
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnclosedQuote { line } => format!(
                "{line}行目で始まる引用符（\"）が、ファイルの終わりまで閉じていません。その行の引用符の数を確かめてください。"
            ),
            Self::InvalidQuote { line } => format!(
                "{line}行目の閉じ引用符の後に文字があります。区切りか改行に直してください。"
            ),
            Self::TooManyColumns { line } => format!(
                "{line}行目の列が多すぎます（1000列まで）。明細のCSVか確かめてください。"
            ),
            Self::TooManyCells { line } => format!(
                "{line}行目でセルが200万個を超えました。ファイルを分けてください。"
            ),
            Self::TooManyRows => {
                "行が多すぎます（20万行まで）。期間を分けて書き出したファイルを、1つずつ取り込んでください。".to_owned()
            }
            Self::CellTooLarge { line } => format!(
                "{line}行目の欄が大きすぎます（1つの欄は64KBまで）。明細のCSVか確かめてください。"
            ),
            Self::Empty => "ファイルに行がありません。中身のあるCSVを選んでください。".to_owned(),
            Self::NotText => {
                "文字のCSVではないようです。表計算ソフトの形式（.xlsx など）なら、CSVで保存し直してください。".to_owned()
            }
        }
    }
}
impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}
impl std::error::Error for ImportError {}
