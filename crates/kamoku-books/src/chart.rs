// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{Account, Kind};
use std::collections::BTreeMap;
use std::fmt;

/// 帳簿に必ず要る科目と、その分類。組み立てと帳票が、この科目を名指しで使うため。
pub const REQUIRED_ACCOUNTS: [(&str, Kind); 7] = [
    ("cash", Kind::Asset),
    ("dep_other", Kind::Asset),
    ("accrued", Kind::Liability),
    ("owner_draw", Kind::Asset),
    ("owner_inv", Kind::Liability),
    ("capital", Kind::Equity),
    ("sales", Kind::Revenue),
];

/// 科目の集合の検査に通らなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChartError {
    /// 同じコードが2つある。
    DuplicateCode(String),
    /// 親のコードが見つからない。
    MissingParent {
        /// 子の科目。
        code: String,
        /// 見つからない親。
        parent: String,
    },
    /// 親と子の分類が違う。
    ParentKindMismatch {
        /// 子の科目。
        code: String,
        /// 親の科目。
        parent: String,
    },
    /// 親をたどると自分に戻る。
    Cycle(String),
    /// 必須の科目が無い。
    MissingRequired(String),
    /// 必須の科目の分類が違う。
    RequiredKindMismatch(String),
    /// 既定の事業割合が100を超える。
    BadDefaultPct(String),
    /// 口座の型と科目の分類が合わない（カードは負債、それ以外は資産）。
    WalletKindMismatch(String),
}
impl fmt::Display for ChartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCode(code) => write!(f, "科目のコード「{code}」が重複しています"),
            Self::MissingParent { code, parent } => {
                write!(f, "科目「{code}」の親「{parent}」が見つかりません")
            }
            Self::ParentKindMismatch { code, parent } => {
                write!(f, "科目「{code}」と親「{parent}」の分類が違います")
            }
            Self::Cycle(code) => write!(f, "科目「{code}」の親をたどると、自分に戻ります"),
            Self::MissingRequired(code) => write!(f, "必須の科目「{code}」がありません"),
            Self::RequiredKindMismatch(code) => write!(f, "必須の科目「{code}」の分類が違います"),
            Self::BadDefaultPct(code) => {
                write!(f, "科目「{code}」の既定の割合が100を超えています")
            }
            Self::WalletKindMismatch(code) => {
                write!(f, "口座「{code}」の型と分類が合いません")
            }
        }
    }
}
impl std::error::Error for ChartError {}

/// 検査済みの科目の集合。分類、並び順、コードの順に並べて持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chart {
    accounts: Vec<Account>,
    index: BTreeMap<String, usize>,
}
impl Chart {
    /// 科目の集合を検査して作ります。コードの重複、親の存在と分類、循環、必須の科目、
    /// 既定の割合、口座の型と分類を確かめます。
    ///
    /// # Errors
    /// 最初に見つかった違反を返します。
    pub fn new(mut accounts: Vec<Account>) -> Result<Self, ChartError> {
        accounts.sort_by(|a, b| (a.kind, a.sort, &a.code).cmp(&(b.kind, b.sort, &b.code)));
        let mut index = BTreeMap::new();
        for (position, account) in accounts.iter().enumerate() {
            if index
                .insert(account.code.as_str().to_owned(), position)
                .is_some()
            {
                return Err(ChartError::DuplicateCode(account.code.as_str().to_owned()));
            }
        }
        let chart = Self { accounts, index };
        for account in &chart.accounts {
            let code = account.code.as_str().to_owned();
            if account.default_pct > 100 {
                return Err(ChartError::BadDefaultPct(code));
            }
            if account
                .wallet
                .as_ref()
                .is_some_and(|w| w.kind.kind() != account.kind)
            {
                return Err(ChartError::WalletKindMismatch(code));
            }
            if let Some(parent) = &account.parent {
                let parent_account =
                    chart
                        .get(parent.as_str())
                        .ok_or_else(|| ChartError::MissingParent {
                            code: code.clone(),
                            parent: parent.as_str().to_owned(),
                        })?;
                if parent_account.kind != account.kind {
                    return Err(ChartError::ParentKindMismatch {
                        code,
                        parent: parent.as_str().to_owned(),
                    });
                }
            }
            chart.check_cycle(account)?;
        }
        for (code, kind) in REQUIRED_ACCOUNTS {
            match chart.get(code) {
                None => return Err(ChartError::MissingRequired(code.to_owned())),
                Some(account) if account.kind != kind => {
                    return Err(ChartError::RequiredKindMismatch(code.to_owned()));
                }
                Some(_) => {}
            }
        }
        Ok(chart)
    }
    fn check_cycle(&self, start: &Account) -> Result<(), ChartError> {
        let mut current = start;
        // 科目の数より多くたどったら、どこかで輪になっている。
        for _ in 0..=self.accounts.len() {
            match current.parent.as_ref().and_then(|p| self.get(p.as_str())) {
                None => return Ok(()),
                Some(parent) if parent.code == start.code => break,
                Some(parent) => current = parent,
            }
        }
        Err(ChartError::Cycle(start.code.as_str().to_owned()))
    }
    /// コードの科目。
    #[must_use]
    pub fn get(&self, code: &str) -> Option<&Account> {
        self.index.get(code).and_then(|&i| self.accounts.get(i))
    }
    /// 分類、並び順、コードの順の全科目。
    pub fn iter(&self) -> std::slice::Iter<'_, Account> {
        self.accounts.iter()
    }
    /// 科目の数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.accounts.len()
    }
    /// 科目が無いか（検査を通った集合は、必須の科目があるので常に偽）。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }
    /// 直接の子の科目を、並び順で返します。
    pub fn children<'a>(&'a self, code: &'a str) -> impl Iterator<Item = &'a Account> + 'a {
        self.accounts
            .iter()
            .filter(move |a| a.parent.as_ref().is_some_and(|p| p.as_str() == code))
    }
    pub(crate) fn has_ancestor(&self, code: &str, ancestor: &str) -> bool {
        let mut current = self.get(code);
        // Chart::new が親の存在と非循環を確認済みなので、根に達すれば終わる。
        while let Some(account) = current {
            if account.code.as_str() == ancestor {
                return true;
            }
            current = account.parent.as_ref().and_then(|p| self.get(p.as_str()));
        }
        false
    }
    /// 口座だけを、並び順で返します。
    pub fn wallets(&self) -> impl Iterator<Item = &Account> {
        self.accounts.iter().filter(|a| a.is_wallet())
    }
    /// 科目の表示名。
    #[must_use]
    pub fn name(&self, code: &str) -> Option<&str> {
        self.get(code).map(|a| a.name.as_str())
    }
    /// 科目の分類。
    #[must_use]
    pub fn kind(&self, code: &str) -> Option<Kind> {
        self.get(code).map(|a| a.kind)
    }
}
impl<'a> IntoIterator for &'a Chart {
    type Item = &'a Account;
    type IntoIter = std::slice::Iter<'a, Account>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
