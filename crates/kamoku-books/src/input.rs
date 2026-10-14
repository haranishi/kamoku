// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳の入力の形（`docs/spec/api-entries.md` の「入力の形」）と、組み立ての設定。

use crate::ser;
use kamoku_jp_tax::Rounding;
use kamoku_ledger::Side;
use serde::{Deserialize, Serialize};
use time::Date;

/// 単純な形の種別。JSONでは `expense` `income` `transfer`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimpleKind {
    /// 支出。
    Expense,
    /// 収入。
    Income,
    /// 振替（口座から口座へ）。
    Transfer,
}
impl SimpleKind {
    /// 付随情報 `kind` に入れる英字の名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Expense => "expense",
            Self::Income => "income",
            Self::Transfer => "transfer",
        }
    }
}

/// 入力の出どころ（付随情報 `source`）。JSONでは `manual` `import` `template` `opening` `demo`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// 画面からの手入力。
    Manual,
    /// CSVの取込からの確定。
    Import,
    /// よく使う取引からの入力。
    Template,
    /// 期首残高。
    Opening,
    /// デモの合成データ。
    Demo,
}
impl Source {
    /// 付随情報に入れる英字の名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Import => "import",
            Self::Template => "template",
            Self::Opening => "opening",
            Self::Demo => "demo",
        }
    }
    /// 英字の名前から返します。知らない名前は `None`。
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Manual,
            Self::Import,
            Self::Template,
            Self::Opening,
            Self::Demo,
        ]
        .into_iter()
        .find(|v| v.as_str() == s)
    }
}

/// 消費税の課税の区分（設定の `tax_mode`）。JSONでは `exempt` `taxable`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaxMode {
    /// 免税事業者。
    Exempt,
    /// 課税事業者。
    Taxable,
}

/// 組み立てに使う設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComposeSettings {
    /// 税額の円未満の端数処理（設定の `rounding`）。
    pub rounding: Rounding,
    /// 課税の区分。警告の判定に使う。
    pub tax_mode: TaxMode,
    /// 付随情報 `source` に入れる出どころ。
    pub source: Source,
}

/// 単純な形の入力。項目は通信の約束と同じで、仕訳の詳細の `input` にもこの形で出す。
/// 金額・割合・税区分は、組み立てるときに検査します（エラーの `code` を正しく返すため）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimpleInput {
    /// 種別。
    pub kind: SimpleKind,
    /// 取引の日付。
    #[serde(with = "ser::date")]
    pub date: Date,
    /// 支払額・入金額・移す額（税込、円）。
    pub amount: i64,
    /// 相手の科目（振替では無し）。
    #[serde(default)]
    pub account: Option<String>,
    /// 口座（支出は払った口座、収入は入金先、振替は移す元）。
    pub wallet: String,
    /// 振替の移す先。
    #[serde(default)]
    pub to_wallet: Option<String>,
    /// 税区分のコード。`None` は対象外。
    #[serde(default)]
    pub tax_code: Option<String>,
    /// 取引先の名前。
    #[serde(default)]
    pub partner: String,
    /// 摘要。
    #[serde(default)]
    pub description: String,
    /// 事業の割合（0〜100）。`None` なら科目の既定。
    #[serde(default)]
    pub business_pct: Option<u8>,
    /// メモ。口座の明細に入れる。
    #[serde(default)]
    pub memo: String,
}

/// 仕訳の形の1行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalLineInput {
    /// 科目のコード。
    pub account: String,
    /// 借方か貸方か（`debit` / `credit`）。
    #[serde(with = "ser::side")]
    pub side: Side,
    /// 金額（円）。
    pub amount: i64,
    /// 税区分のコード。
    #[serde(default)]
    pub tax_code: Option<String>,
    /// 税額。省くと、この行の金額から計算する。
    #[serde(default)]
    pub tax_amount: Option<i64>,
    /// 行のメモ。
    #[serde(default)]
    pub memo: String,
}

/// 仕訳の形の入力（`kind` が `journal`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalInput {
    /// 取引の日付。
    #[serde(with = "ser::date")]
    pub date: Date,
    /// 摘要。
    #[serde(default)]
    pub description: String,
    /// 取引先の名前。
    #[serde(default)]
    pub partner: String,
    /// 明細の行。
    pub lines: Vec<JournalLineInput>,
}
