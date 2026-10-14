// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{AccountCode, EntryError, EntryId, Yen, ids::valid_code};
use std::collections::BTreeMap;
use time::Date;

const META_KEY_MAX: usize = 32;
const META_VALUE_MAX: usize = 1000;
const META_PAIRS_MAX: usize = 16;

/// 借方・貸方の区別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 借方。
    Debit,
    /// 貸方。
    Credit,
}
/// 仕訳の明細。仕訳の作成時に検証します。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// 勘定科目コード。
    pub account: AccountCode,
    /// 借方・貸方。
    pub side: Side,
    /// 明細金額。
    pub amount: Yen,
    /// 任意の税コード。
    pub tax_code: Option<String>,
    /// 明細金額に含まれる税額。
    pub tax_amount: Yen,
    /// 明細のメモ。
    pub memo: String,
}
impl Line {
    /// 税情報とメモを空にして明細を作ります。
    #[must_use]
    pub fn new(account: AccountCode, side: Side, amount: Yen) -> Self {
        Self {
            account,
            side,
            amount,
            tax_code: None,
            tax_amount: Yen::ZERO,
            memo: String::new(),
        }
    }
    /// 税コードと税額を設定します。検証は仕訳の作成時に行います。
    #[must_use]
    pub fn with_tax(mut self, code: &str, amount: Yen) -> Self {
        self.tax_code = Some(code.to_owned());
        self.tax_amount = amount;
        self
    }
    /// メモを設定します。検証は仕訳の作成時に行います。
    #[must_use]
    pub fn with_memo(mut self, memo: &str) -> Self {
        memo.clone_into(&mut self.memo);
        self
    }
}
/// 貸借が一致する検証済みの仕訳。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    id: EntryId,
    date: Date,
    description: String,
    lines: Vec<Line>,
    total: Yen,
    // 鍵の昇順で持つ。挿入の順がハッシュに影響しないようにするため。
    meta: BTreeMap<String, String>,
}
impl Entry {
    /// 明細、文字列長、税額、貸借一致を検証して仕訳を作ります。
    ///
    /// # Errors
    /// 明細数・金額・税情報・文字列長・貸借の不正や桁あふれを返します。
    pub fn new(
        id: EntryId,
        date: Date,
        description: &str,
        lines: Vec<Line>,
    ) -> Result<Self, EntryError> {
        if lines.len() < 2 {
            return Err(EntryError::TooFewLines);
        }
        if lines.len() > 200 {
            return Err(EntryError::TooManyLines);
        }
        for (index, line) in lines.iter().enumerate() {
            if line.amount <= Yen::ZERO {
                return Err(EntryError::NonPositiveAmount { index });
            }
        }
        for (index, line) in lines.iter().enumerate() {
            if line.tax_amount < Yen::ZERO
                || line.tax_amount > line.amount
                || (line.tax_code.is_none() && line.tax_amount != Yen::ZERO)
            {
                return Err(EntryError::BadTax { index });
            }
        }
        for (index, line) in lines.iter().enumerate() {
            if line.tax_code.as_deref().is_some_and(|s| !valid_code(s, 32)) {
                return Err(EntryError::BadTaxCode { index });
            }
        }
        if description.len() > 1000 || lines.iter().any(|line| line.memo.len() > 1000) {
            return Err(EntryError::TextTooLong);
        }
        if !lines.iter().any(|line| line.side == Side::Debit)
            || !lines.iter().any(|line| line.side == Side::Credit)
        {
            return Err(EntryError::MissingSide);
        }
        let mut debit = Yen::ZERO;
        let mut credit = Yen::ZERO;
        for line in &lines {
            let total = match line.side {
                Side::Debit => &mut debit,
                Side::Credit => &mut credit,
            };
            *total = total.checked_add(line.amount).ok_or(EntryError::Overflow)?;
        }
        if debit != credit {
            return Err(EntryError::Unbalanced { debit, credit });
        }
        Ok(Self {
            id,
            date,
            description: description.to_owned(),
            lines,
            total: debit,
            meta: BTreeMap::new(),
        })
    }
    /// 付随情報（取引先の名前、入力の種類など）を1組足します。同じ鍵は上書きします。
    /// 付随情報はハッシュの対象です。鍵は半角の小文字・数字・`_` `-` `.` の1〜32バイト、
    /// 値は1000バイト以下、組は16までです。
    ///
    /// # Errors
    /// 鍵の形、値の長さ、組の数の違反を返します。
    pub fn with_meta(mut self, key: &str, value: &str) -> Result<Self, EntryError> {
        if key.is_empty()
            || key.len() > META_KEY_MAX
            || !key.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.')
            })
        {
            return Err(EntryError::BadMetaKey);
        }
        if value.len() > META_VALUE_MAX {
            return Err(EntryError::MetaValueTooLong);
        }
        if !self.meta.contains_key(key) && self.meta.len() >= META_PAIRS_MAX {
            return Err(EntryError::TooManyMeta);
        }
        self.meta.insert(key.to_owned(), value.to_owned());
        Ok(self)
    }
    /// 鍵の昇順の付随情報。
    #[must_use]
    pub const fn meta(&self) -> &BTreeMap<String, String> {
        &self.meta
    }
    /// 仕訳識別子。
    #[must_use]
    pub const fn id(&self) -> &EntryId {
        &self.id
    }
    /// 仕訳の日付。
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }
    /// 仕訳の摘要。
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
    /// 元の順序の明細。
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
    /// 借方合計（貸方合計と同額）。
    #[must_use]
    pub const fn total(&self) -> Yen {
        self.total
    }
}
