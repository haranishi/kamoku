// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::TaxCode;
use kamoku_ledger::{AccountCode, Side};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

/// 勘定科目の分類。JSONでは `asset` `liability` `equity` `revenue` `expense`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// 資産。
    Asset,
    /// 負債。
    Liability,
    /// 資本。
    Equity,
    /// 収益。
    Revenue,
    /// 費用。
    Expense,
}
impl Kind {
    /// 帳票に並べる順の、すべての分類。
    pub const ALL: [Self; 5] = [
        Self::Asset,
        Self::Liability,
        Self::Equity,
        Self::Revenue,
        Self::Expense,
    ];
    /// 残高が増えるときの側。資産と費用は借方、負債・資本・収益は貸方。
    #[must_use]
    pub const fn normal_side(self) -> Side {
        match self {
            Self::Asset | Self::Expense => Side::Debit,
            Self::Liability | Self::Equity | Self::Revenue => Side::Credit,
        }
    }
    /// JSONで使う英字の名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Liability => "liability",
            Self::Equity => "equity",
            Self::Revenue => "revenue",
            Self::Expense => "expense",
        }
    }
    /// 英字の名前から分類を返します。知らない名前は `None`。
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
    /// 画面に出す日本語の名前。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Asset => "資産",
            Self::Liability => "負債",
            Self::Equity => "資本",
            Self::Revenue => "収益",
            Self::Expense => "費用",
        }
    }
}

/// 口座の型。JSONでは `cash` `bank` `card` `ewallet`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalletType {
    /// 現金。
    Cash,
    /// 預金。
    Bank,
    /// クレジットカード。
    Card,
    /// 電子マネー。
    Ewallet,
}
impl WalletType {
    /// JSONで使う英字の名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Bank => "bank",
            Self::Card => "card",
            Self::Ewallet => "ewallet",
        }
    }
    /// 画面に出す日本語の名前。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cash => "現金",
            Self::Bank => "預金",
            Self::Card => "クレジットカード",
            Self::Ewallet => "電子マネー",
        }
    }
    /// この型の口座が属する分類。カードは未払いの残高を持つので負債。
    #[must_use]
    pub const fn kind(self) -> Kind {
        match self {
            Self::Card => Kind::Liability,
            Self::Cash | Self::Bank | Self::Ewallet => Kind::Asset,
        }
    }
    /// 新しい口座の親の科目の既定。
    #[must_use]
    pub const fn default_parent(self) -> &'static str {
        match self {
            Self::Cash | Self::Ewallet => "cash",
            Self::Bank => "dep_other",
            Self::Card => "accrued",
        }
    }
}

/// 口座の情報。JSONでは `{ "type": "card", "institution": "" }`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Wallet {
    /// 口座の型。
    #[serde(rename = "type")]
    pub kind: WalletType,
    /// 金融機関などの名前（自由記入）。
    #[serde(default)]
    pub institution: String,
}

/// 勘定科目。口座も、`wallet` を持つ科目として表します。
/// JSONの形は `docs/spec/api-masters.md` の科目と同じで、`normal_side` は `kind` から出します。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// コード。一度使ったら変えない。
    pub code: AccountCode,
    /// 表示名。
    pub name: String,
    /// ひらがなの読み。候補の絞り込みに使う。
    pub reading: String,
    /// 分類。
    pub kind: Kind,
    /// 親の科目のコード。
    pub parent: Option<AccountCode>,
    /// 口座なら、その型と金融機関。
    pub wallet: Option<Wallet>,
    /// 選んだときに入れる税区分の既定。
    pub default_tax: Option<TaxCode>,
    /// 選んだときに入れる事業の割合（0〜100）。
    pub default_pct: u8,
    /// 青色申告決算書（一般用）の行名。固定の行が無い経費は「（空欄）」。
    pub form_line: Option<String>,
    /// 有効か。無効の科目には新しく記帳できない。
    pub enabled: bool,
    /// システムが用意した科目か。
    pub system: bool,
    /// 並び順。分類の中で小さい順。
    pub sort: u32,
}
/// 名前や分類を変えられない科目のコード（事業主貸、事業主借、元入金）。
pub const PROTECTED_CODES: [&str; 3] = ["owner_draw", "owner_inv", "capital"];
impl Account {
    /// 残高が増えるときの側。
    #[must_use]
    pub const fn normal_side(&self) -> Side {
        self.kind.normal_side()
    }
    /// 口座か。
    #[must_use]
    pub const fn is_wallet(&self) -> bool {
        self.wallet.is_some()
    }
    /// 事業主貸・事業主借・元入金のように、名前と分類を変えられない科目か。
    #[must_use]
    pub fn is_protected(&self) -> bool {
        PROTECTED_CODES.contains(&self.code.as_str())
    }
}
impl Serialize for Account {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("Account", 13)?;
        s.serialize_field("code", self.code.as_str())?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("reading", &self.reading)?;
        s.serialize_field("kind", &self.kind)?;
        s.serialize_field("normal_side", crate::ser::side_str(self.normal_side()))?;
        s.serialize_field("parent", &self.parent.as_ref().map(AccountCode::as_str))?;
        s.serialize_field("wallet", &self.wallet)?;
        s.serialize_field("default_tax", &self.default_tax)?;
        s.serialize_field("default_pct", &self.default_pct)?;
        s.serialize_field("form_line", &self.form_line)?;
        s.serialize_field("enabled", &self.enabled)?;
        s.serialize_field("system", &self.system)?;
        s.serialize_field("sort", &self.sort)?;
        s.end()
    }
}
