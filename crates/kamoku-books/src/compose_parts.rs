// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 組み立ての結果（`Composed`）と小さな部品（科目の引き当て、金額・文字・税区分の検査、明細の表現）。

use crate::{Account, Chart, ComposeError, Source, TaxCode, ser, tax_included};
use kamoku_jp_tax::Rounding;
use kamoku_ledger::{AccountCode, Entry, EntryError, EntryId, Line, Side, Yen};
use serde::Serialize;
use std::collections::BTreeMap;
use time::Date;

/// 1明細の金額の上限。ブラウザの数値で正確に扱える範囲に、合計まで収めるため。
pub const MAX_AMOUNT: i64 = 9_999_999_999_999;
const TEXT_MAX: usize = 1000;
pub(crate) const OWNER_DRAW: &str = "owner_draw";
pub(crate) const OWNER_INV: &str = "owner_inv";
pub(crate) const CAPITAL: &str = "capital";

/// 組み立てた仕訳。画面に見せる明細と、保存する明細は、どちらもこれから作る（AC-02.2）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composed {
    /// 取引の日付。
    pub date: Date,
    /// 摘要。
    pub description: String,
    /// 明細。0円の明細は含まない。
    pub lines: Vec<Line>,
    /// 付随情報（`kind` `source` `partner` `business_pct`）。呼び出し側が `draft` などを足してよい。
    pub meta: BTreeMap<String, String>,
    /// 保存は止めないが、利用者に確かめてほしいこと。
    pub warnings: Vec<String>,
}
impl Composed {
    pub(crate) fn new(
        date: Date,
        description: &str,
        kind: &str,
        source: Source,
        partner: &str,
    ) -> Self {
        let mut meta = BTreeMap::new();
        meta.insert("kind".to_owned(), kind.to_owned());
        meta.insert("source".to_owned(), source.as_str().to_owned());
        if !partner.is_empty() {
            meta.insert("partner".to_owned(), partner.to_owned());
        }
        Self {
            date,
            description: description.to_owned(),
            lines: Vec::new(),
            meta,
            warnings: Vec::new(),
        }
    }
    /// 借方の合計。桁あふれのときは `None`。
    #[must_use]
    pub fn total(&self) -> Option<Yen> {
        self.lines
            .iter()
            .filter(|l| l.side == Side::Debit)
            .try_fold(Yen::ZERO, |sum, l| sum.checked_add(l.amount))
    }
    /// 台帳に入れる仕訳を作ります。付随情報も含めます。
    ///
    /// # Errors
    /// `Entry::new` と `Entry::with_meta` の検査の違反を返します。
    pub fn to_entry(&self, id: EntryId) -> Result<Entry, EntryError> {
        let entry = Entry::new(id, self.date, &self.description, self.lines.clone())?;
        self.meta
            .iter()
            .try_fold(entry, |entry, (key, value)| entry.with_meta(key, value))
    }
    /// 明細を、通信の約束の形（科目名つき）にします。
    #[must_use]
    pub fn line_views(&self, chart: &Chart) -> Vec<LineView> {
        self.lines.iter().map(|l| LineView::new(l, chart)).collect()
    }
    // 保存のときに初めて断られることが無いよう、組み立ての時点で同じ検査を通しておく。
    pub(crate) fn validated(self) -> Result<Self, ComposeError> {
        let id = EntryId::new("preview").map_err(|_| ComposeError::BadRequest {
            field: None,
            message: "内部の識別子を作れません。".to_owned(),
        })?;
        match self.to_entry(id) {
            Ok(_) => Ok(self),
            Err(EntryError::Unbalanced { debit, credit }) => {
                Err(ComposeError::Unbalanced { debit, credit })
            }
            Err(EntryError::TooFewLines | EntryError::TooManyLines) => Err(ComposeError::bad(
                "lines",
                "仕訳の行は2行から200行までです。行の数を直してください。",
            )),
            Err(EntryError::MetaValueTooLong) => Err(ComposeError::bad(
                "partner",
                "取引先の名前が長すぎます（1000バイトまで）。短くしてください。",
            )),
            Err(other) => Err(ComposeError::BadRequest {
                field: None,
                message: format!("{other}。入力を見直してください。"),
            }),
        }
    }
}

/// 明細の、通信の約束の形（`{ account, account_name, side, amount, tax_code, tax_amount, memo }`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineView {
    /// 科目のコード。
    pub account: String,
    /// 科目の表示名（科目が見つからなければコード）。
    pub account_name: String,
    /// `debit` か `credit`。
    #[serde(serialize_with = "ser::side::serialize")]
    pub side: Side,
    /// 金額。
    #[serde(serialize_with = "ser::yen")]
    pub amount: Yen,
    /// 税区分のコード。
    pub tax_code: Option<String>,
    /// 税額。
    #[serde(serialize_with = "ser::yen")]
    pub tax_amount: Yen,
    /// 行のメモ。
    pub memo: String,
}
impl LineView {
    /// 明細と科目の集合から作ります。
    #[must_use]
    pub fn new(line: &Line, chart: &Chart) -> Self {
        let code = line.account.as_str();
        Self {
            account: code.to_owned(),
            account_name: chart.name(code).unwrap_or(code).to_owned(),
            side: line.side,
            amount: line.amount,
            tax_code: line.tax_code.clone(),
            tax_amount: line.tax_amount,
            memo: line.memo.clone(),
        }
    }
}

pub(crate) struct Lookup<'a> {
    pub chart: &'a Chart,
    pub check_enabled: bool,
}
impl<'a> Lookup<'a> {
    fn find(&self, field: &str, code: Option<&str>) -> Result<&'a Account, ComposeError> {
        let code = code
            .filter(|c| !c.is_empty())
            .ok_or_else(|| ComposeError::MissingAccount {
                field: field.to_owned(),
            })?;
        self.chart
            .get(code)
            .ok_or_else(|| ComposeError::UnknownAccount {
                field: field.to_owned(),
                code: code.to_owned(),
            })
    }
    fn enabled(&self, field: &str, account: &'a Account) -> Result<&'a Account, ComposeError> {
        if self.check_enabled && !account.enabled {
            return Err(ComposeError::DisabledAccount {
                field: field.to_owned(),
                name: account.name.clone(),
            });
        }
        Ok(account)
    }
    pub(crate) fn account(
        &self,
        field: &str,
        code: Option<&str>,
    ) -> Result<&'a Account, ComposeError> {
        let account = self.find(field, code)?;
        self.enabled(field, account)
    }
    pub(crate) fn wallet(
        &self,
        field: &str,
        code: Option<&str>,
    ) -> Result<&'a Account, ComposeError> {
        let account = self.find(field, code)?;
        if !account.is_wallet() {
            return Err(ComposeError::NotAWallet {
                field: field.to_owned(),
                name: account.name.clone(),
            });
        }
        self.enabled(field, account)
    }
    pub(crate) fn system_code(&self, code: &str) -> Result<AccountCode, ComposeError> {
        self.chart
            .get(code)
            .map(|a| a.code.clone())
            .ok_or_else(|| ComposeError::UnknownAccount {
                field: "account".to_owned(),
                code: code.to_owned(),
            })
    }
}

pub(crate) fn check_amount(field: &str, amount: i64) -> Result<(), ComposeError> {
    if (1..=MAX_AMOUNT).contains(&amount) {
        return Ok(());
    }
    Err(ComposeError::AmountRange {
        field: field.to_owned(),
        amount,
    })
}
pub(crate) fn check_text(field: &str, text: &str, label: &str) -> Result<(), ComposeError> {
    if text.len() > TEXT_MAX {
        return Err(ComposeError::bad(
            field,
            format!("{label}が長すぎます（1000バイトまで）。短くしてください。"),
        ));
    }
    Ok(())
}
pub(crate) fn parse_tax(field: &str, code: Option<&str>) -> Result<Option<TaxCode>, ComposeError> {
    match code.filter(|c| !c.is_empty()) {
        None => Ok(None),
        Some(text) => TaxCode::parse(text).map(Some).ok_or_else(|| ComposeError::BadTaxCode {
            field: field.to_owned(),
            message: format!(
                "税区分「{text}」は使えません。S10・S8・P10・P8・P10X・P8X・NT のどれかを選ぶか、空にしてください。"
            ),
        }),
    }
}
pub(crate) fn taxed(
    line: Line,
    code: Option<TaxCode>,
    rounding: Rounding,
) -> Result<Line, ComposeError> {
    let Some(code) = code else {
        return Ok(line);
    };
    let tax = tax_included(code, line.amount, rounding).map_err(|_| ComposeError::AmountRange {
        field: "amount".to_owned(),
        amount: line.amount.get(),
    })?;
    Ok(line.with_tax(code.as_str(), tax))
}
/// 家事按分の事業分。支払額×割合を円未満で四捨五入（`docs/spec/data-model.md`）。
pub(crate) fn business_share(amount: Yen, pct: u8) -> Option<Yen> {
    let scaled = i128::from(amount.get()).checked_mul(i128::from(pct))?;
    i64::try_from((scaled + 50) / 100).ok().map(Yen::new)
}
