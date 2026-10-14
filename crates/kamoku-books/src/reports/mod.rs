// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 帳票。入力は `Ledger::current()` の仕訳（取消を除いた最新の版）。
//! 年またぎは、どの帳票も [`carried`] の「仮想の繰越」で扱う（`docs/spec/api-reports.md`）。

mod balance;
mod income;
mod ledger;
mod tax;
mod trial;
mod wallets;

pub use balance::{BalanceSheet, BsRow, NET_INCOME_ROW_CODE, balance_sheet};
pub use income::{
    IncomeStatement, IsRow, MonthPoint, MonthTotal, Section, income_statement, monthly_series,
};
pub use ledger::{AccountLedger, LedgerRow, account_ledger};
pub use tax::{TaxRow, TaxSummary, TransitionalRow, tax_disclaimer, tax_summary};
pub use trial::{TrialBalance, TrialRow, trial_balance};
pub use wallets::{WalletBalances, WalletItem, wallet_balances};

use crate::compose_parts::{CAPITAL, OWNER_DRAW, OWNER_INV};
use crate::{Account, Chart, Kind};
use kamoku_ledger::{Entry, Line, Side, Yen};
use std::collections::BTreeMap;
use std::fmt;
use time::Date;

/// 帳票が返す集計値の上限（ブラウザの数値で正確に扱える最大の整数）。
pub const MAX_REPORT_AMOUNT: i64 = 9_007_199_254_740_991;

/// 帳票を作れなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReportError {
    /// 仕訳または要求に、科目の集合に無いコードがある。
    UnknownAccount(String),
    /// 仕訳に、知らない税区分のコードがある。
    UnknownTaxCode(String),
    /// 期間の始めが終わりより後。
    BadPeriod,
    /// 期間が1つの暦年に収まらない。
    PeriodSpansYears,
    /// 集計値が上限を超えた。
    AmountRange,
}
impl ReportError {
    /// 失敗の `code`（`docs/spec/api-common.md`）。
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownAccount(_) => "unknown_account",
            Self::UnknownTaxCode(_) => "bad_tax_code",
            Self::BadPeriod => "bad_request",
            Self::PeriodSpansYears => "period_spans_years",
            Self::AmountRange => "amount_range",
        }
    }
    /// 応答の状態。
    #[must_use]
    pub const fn http_status(&self) -> u16 {
        match self {
            Self::BadPeriod => 400,
            _ => 422,
        }
    }
    /// 原因と直し方の日本語。
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnknownAccount(code) => {
                format!("科目「{code}」が見つかりません。勘定科目の一覧を確かめてください。")
            }
            Self::UnknownTaxCode(code) => {
                format!("仕訳に、知らない税区分「{code}」があります。その仕訳を訂正してください。")
            }
            Self::BadPeriod => "期間の始めが、終わりより後になっています。日付を入れ替えてください。".to_owned(),
            Self::PeriodSpansYears => {
                "期間は1つの年（1月1日〜12月31日）の中で選んでください。年をまたぐ期間は扱えません。"
                    .to_owned()
            }
            Self::AmountRange => {
                "集計した金額が大きすぎて、正確に表示できません（9,007,199,254,740,991円まで）。"
                    .to_owned()
            }
        }
    }
}
impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}
impl std::error::Error for ReportError {}

pub(crate) fn check_period(from: Date, to: Date) -> Result<(), ReportError> {
    if from > to {
        return Err(ReportError::BadPeriod);
    }
    if from.year() != to.year() {
        return Err(ReportError::PeriodSpansYears);
    }
    Ok(())
}
pub(crate) fn account<'a>(chart: &'a Chart, code: &str) -> Result<&'a Account, ReportError> {
    chart
        .get(code)
        .ok_or_else(|| ReportError::UnknownAccount(code.to_owned()))
}
pub(crate) fn add(a: Yen, b: Yen) -> Result<Yen, ReportError> {
    a.checked_add(b)
        .filter(|v| v.get().unsigned_abs() <= MAX_REPORT_AMOUNT.unsigned_abs())
        .ok_or(ReportError::AmountRange)
}
pub(crate) fn sub(a: Yen, b: Yen) -> Result<Yen, ReportError> {
    add(a, Yen::ZERO.checked_sub(b).ok_or(ReportError::AmountRange)?)
}
pub(crate) fn bump<'a>(
    map: &mut BTreeMap<&'a str, Yen>,
    code: &'a str,
    value: Yen,
) -> Result<(), ReportError> {
    let slot = map.entry(code).or_insert(Yen::ZERO);
    *slot = add(*slot, value)?;
    Ok(())
}
/// 明細の金額を、`normal` の側から見た符号つきの値にします。
pub(crate) fn signed(line: &Line, normal: Side) -> Result<Yen, ReportError> {
    if line.side == normal {
        Ok(line.amount)
    } else {
        sub(Yen::ZERO, line.amount)
    }
}
/// 年の初めに0から始まる科目（収益・費用・事業主貸・事業主借）か。
pub(crate) fn year_scoped(account: &Account, chart: &Chart) -> bool {
    matches!(account.kind, Kind::Revenue | Kind::Expense)
        || [OWNER_DRAW, OWNER_INV]
            .iter()
            .any(|ancestor| chart.has_ancestor(account.code.as_str(), ancestor))
}

/// 期首残高の仕訳（`meta.kind` が `opening`）か。
pub(crate) fn is_opening(entry: &Entry) -> bool {
    entry
        .meta()
        .get("kind")
        .is_some_and(|kind| kind == "opening")
}

/// 期間の初日 `from` の期首として数える仕訳か。`from` より前の日付の仕訳と、`from` と同じ日付の
/// 期首残高の仕訳。後者は、開業の年の試算表と元帳で、期首の列に期首残高が出るようにするため。
pub(crate) fn counts_as_opening(entry: &Entry, from: Date) -> bool {
    entry.date() < from || (entry.date() == from && is_opening(entry))
}

/// 仮想の繰越をした残高（通常の側から見た値）。`frame_year` の年の枠で、`through` の日の終わりまで。
/// 年の初めに0から始まる科目の、`frame_year` より前の動きは、元入金に足す（貸方なら増、借方なら減）。
/// 貸借対照表・口座の残高は、この関数で残高を出す（試算表と元帳の期首は [`carried_opening`]）。
///
/// # Errors
/// 科目の集合に無いコード、または集計値の上限超えを返します。
pub(crate) fn carried<'a>(
    entries: &[&Entry],
    chart: &'a Chart,
    frame_year: i32,
    through: Option<Date>,
) -> Result<BTreeMap<&'a str, Yen>, ReportError> {
    let Some(through) = through else {
        return Ok(BTreeMap::new());
    };
    carried_where(entries, chart, frame_year, |e| e.date() <= through)
}

/// 試算表と元帳の、期間の初日 `from` の期首の残高（仮想の繰越を含む）。
/// `from` と同じ日付の期首残高の仕訳も、期首に入れる（[`counts_as_opening`]）。
///
/// # Errors
/// 科目の集合に無いコード、または集計値の上限超えを返します。
pub(crate) fn carried_opening<'a>(
    entries: &[&Entry],
    chart: &'a Chart,
    from: Date,
) -> Result<BTreeMap<&'a str, Yen>, ReportError> {
    carried_where(entries, chart, from.year(), |e| counts_as_opening(e, from))
}

fn carried_where<'a>(
    entries: &[&Entry],
    chart: &'a Chart,
    frame_year: i32,
    include: impl Fn(&Entry) -> bool,
) -> Result<BTreeMap<&'a str, Yen>, ReportError> {
    let mut balances = BTreeMap::new();
    for entry in entries.iter().filter(|e| include(e)) {
        let before_frame = entry.date().year() < frame_year;
        for line in entry.lines() {
            let acc = account(chart, line.account.as_str())?;
            if before_frame && year_scoped(acc, chart) {
                bump(&mut balances, CAPITAL, signed(line, Side::Credit)?)?;
            } else {
                bump(
                    &mut balances,
                    acc.code.as_str(),
                    signed(line, acc.normal_side())?,
                )?;
            }
        }
    }
    Ok(balances)
}

/// 期間の中の、科目ごとの借方と貸方の動き。
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Movement {
    pub debit: Yen,
    pub credit: Yen,
}
pub(crate) fn movements<'a>(
    entries: &[&Entry],
    chart: &'a Chart,
    from: Date,
    to: Date,
) -> Result<BTreeMap<&'a str, Movement>, ReportError> {
    let mut out = BTreeMap::<&'a str, Movement>::new();
    for entry in entries
        .iter()
        .filter(|e| from <= e.date() && e.date() <= to)
        .filter(|e| !(is_opening(e) && e.date() == from))
    {
        for line in entry.lines() {
            let acc = account(chart, line.account.as_str())?;
            let m = out.entry(acc.code.as_str()).or_default();
            match line.side {
                Side::Debit => m.debit = add(m.debit, line.amount)?,
                Side::Credit => m.credit = add(m.credit, line.amount)?,
            }
        }
    }
    Ok(out)
}
