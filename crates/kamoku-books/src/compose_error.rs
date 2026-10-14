// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::ser::comma;
use kamoku_ledger::Yen;
use std::fmt;

/// 仕訳を組み立てられなかった理由。`code()` は通信の約束（`docs/spec/api-common.md`）の
/// 失敗の `code`、`message()` は原因と直し方の日本語、`field()` は結びつく入力欄。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComposeError {
    /// 借方と貸方の合計が違う。
    Unbalanced {
        /// 借方の合計。
        debit: Yen,
        /// 貸方の合計。
        credit: Yen,
    },
    /// 科目・口座が見つからない。
    UnknownAccount {
        /// 入力欄。
        field: String,
        /// 入力されたコード。
        code: String,
    },
    /// 口座の欄に、口座でない科目が入っている。
    NotAWallet {
        /// 入力欄。
        field: String,
        /// 科目の名前。
        name: String,
    },
    /// 無効にした科目・口座。
    DisabledAccount {
        /// 入力欄。
        field: String,
        /// 科目の名前。
        name: String,
    },
    /// 科目・口座が選ばれていない。
    MissingAccount {
        /// 入力欄。
        field: String,
    },
    /// 金額が1〜9,999,999,999,999円の外。
    AmountRange {
        /// 入力欄。
        field: String,
        /// 入力された金額。
        amount: i64,
    },
    /// 税区分のコードが使えない、または税区分と税額が合わない。
    BadTaxCode {
        /// 入力欄。
        field: String,
        /// 何が悪いか。
        message: String,
    },
    /// 上のどれにも当たらない、入力の規則の違反。
    BadRequest {
        /// 入力欄。
        field: Option<String>,
        /// 原因と直し方。
        message: String,
    },
}
impl ComposeError {
    pub(crate) fn bad(field: &str, message: impl Into<String>) -> Self {
        Self::BadRequest {
            field: Some(field.to_owned()),
            message: message.into(),
        }
    }
    /// 失敗の `code`。
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unbalanced { .. } => "unbalanced",
            Self::UnknownAccount { .. } | Self::NotAWallet { .. } => "unknown_account",
            Self::DisabledAccount { .. } => "disabled_account",
            Self::MissingAccount { .. } => "missing_account",
            Self::AmountRange { .. } => "amount_range",
            Self::BadTaxCode { .. } => "bad_tax_code",
            Self::BadRequest { .. } => "bad_request",
        }
    }
    /// 応答の状態。どれも入力の規則の違反なので422（形の壊れたJSONは、読む側が400にする）。
    #[must_use]
    pub const fn http_status(&self) -> u16 {
        422
    }
    /// 結びつく入力欄（`lines[0].amount` の形もある）。
    #[must_use]
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::Unbalanced { .. } => Some("lines"),
            Self::UnknownAccount { field, .. }
            | Self::NotAWallet { field, .. }
            | Self::DisabledAccount { field, .. }
            | Self::MissingAccount { field }
            | Self::AmountRange { field, .. }
            | Self::BadTaxCode { field, .. } => Some(field),
            Self::BadRequest { field, .. } => field.as_deref(),
        }
    }
    /// 原因と直し方の日本語。
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Unbalanced { debit, credit } => {
                format!(
                    "貸借が一致しません。借方 {}円、貸方 {}円です。差の{}円を、どちらかの行に足してください。",
                    comma(debit.get()),
                    comma(credit.get()),
                    comma(debit.get().abs_diff(credit.get()))
                )
            }
            Self::UnknownAccount { code, .. } => {
                format!("科目・口座「{code}」が見つかりません。一覧から選び直してください。")
            }
            Self::NotAWallet { name, .. } => {
                format!("「{name}」は口座ではありません。お金が動いた口座を選んでください。")
            }
            Self::DisabledAccount { name, .. } => format!(
                "「{name}」は無効にしてあります。勘定科目の画面で有効に戻すか、別の科目を選んでください。"
            ),
            Self::MissingAccount { field } => {
                if field.contains("wallet") {
                    "口座が選ばれていません。口座を選んでください。".to_owned()
                } else {
                    "科目が選ばれていません。科目を選んでください。".to_owned()
                }
            }
            Self::AmountRange { amount, .. } => format!(
                "金額は1円から9,999,999,999,999円までです。入力は{}円でした。",
                comma(*amount)
            ),
            Self::BadTaxCode { message, .. } | Self::BadRequest { message, .. } => message.clone(),
        }
    }
}
impl fmt::Display for ComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}
impl std::error::Error for ComposeError {}
