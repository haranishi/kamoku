// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use kamoku_jp_tax::{Rate, Rounding, TaxError, split_tax_included};
use kamoku_ledger::Yen;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 税区分。名前は通信の約束（`docs/spec/api-entries.md`）のコードと同じにしてある。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "&'static str")]
#[allow(clippy::upper_case_acronyms)] // 通信で使うコードと、そのまま対応させるため。
pub enum TaxCode {
    /// 課税売上10%。
    S10,
    /// 課税売上8%（軽減）。
    S8,
    /// 課税仕入10%。
    P10,
    /// 課税仕入8%（軽減）。
    P8,
    /// 免税事業者等からの課税仕入れ10%（経過措置）。
    P10X,
    /// 免税事業者等からの課税仕入れ8%（軽減・経過措置）。
    P8X,
    /// 非課税。
    NT,
}
/// 税区分の大きな分け方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxClass {
    /// 課税売上。
    Sales,
    /// 課税仕入。
    Purchase,
    /// 免税事業者等からの課税仕入れ（経過措置）。
    TransitionalPurchase,
    /// 非課税。
    NonTaxable,
}
impl TaxClass {
    /// 画面に出す日本語の名前。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sales => "売上",
            Self::Purchase => "仕入",
            Self::TransitionalPurchase => "経過措置の仕入",
            Self::NonTaxable => "非課税",
        }
    }
}
impl TaxCode {
    /// すべての税区分。集計の表に並べる順。
    pub const ALL: [Self; 7] = [
        Self::S10,
        Self::S8,
        Self::P10,
        Self::P8,
        Self::P10X,
        Self::P8X,
        Self::NT,
    ];
    /// コードから税区分を返します。大文字・小文字は区別し、知らないコードは `None`。
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.as_str() == s)
    }
    /// 通信と台帳で使うコード。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::S10 => "S10",
            Self::S8 => "S8",
            Self::P10 => "P10",
            Self::P8 => "P8",
            Self::P10X => "P10X",
            Self::P8X => "P8X",
            Self::NT => "NT",
        }
    }
    /// 画面に出す日本語の名前。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::S10 => "課税売上10%",
            Self::S8 => "課税売上8%（軽減）",
            Self::P10 => "課税仕入10%",
            Self::P8 => "課税仕入8%（軽減）",
            Self::P10X => "免税事業者等からの課税仕入れ10%（経過措置）",
            Self::P8X => "免税事業者等からの課税仕入れ8%（軽減・経過措置）",
            Self::NT => "非課税",
        }
    }
    /// 売上・仕入・経過措置の仕入・非課税の別。
    #[must_use]
    pub const fn class(self) -> TaxClass {
        match self {
            Self::S10 | Self::S8 => TaxClass::Sales,
            Self::P10 | Self::P8 => TaxClass::Purchase,
            Self::P10X | Self::P8X => TaxClass::TransitionalPurchase,
            Self::NT => TaxClass::NonTaxable,
        }
    }
    /// 税率。非課税は `None`。経過措置の仕入れは、支払額に含まれる税相当額を出すための税率。
    #[must_use]
    pub const fn rate(self) -> Option<Rate> {
        match self {
            Self::S10 | Self::P10 | Self::P10X => Some(Rate::Standard),
            Self::S8 | Self::P8 | Self::P8X => Some(Rate::Reduced),
            Self::NT => None,
        }
    }
}
impl From<TaxCode> for &'static str {
    fn from(code: TaxCode) -> Self {
        code.as_str()
    }
}
impl TryFrom<String> for TaxCode {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).ok_or_else(|| format!("税区分「{value}」は使えません"))
    }
}
impl fmt::Display for TaxCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 税込の金額に含まれる税額を、`kamoku-jp-tax` の `split_tax_included` で出します。非課税は0円。
/// 1明細ずつの記帳上の目安で、請求書ごとの税率別の集計とは違います（`docs/spec/data-model.md`）。
///
/// # Errors
/// 負の金額または桁あふれを返します。
pub fn tax_included(code: TaxCode, gross: Yen, rounding: Rounding) -> Result<Yen, TaxError> {
    match code.rate() {
        None => Ok(Yen::ZERO),
        Some(rate) => Ok(split_tax_included(gross, rate, rounding)?.tax),
    }
}
