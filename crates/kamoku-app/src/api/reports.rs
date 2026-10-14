// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 帳票（`/reports/*`）。計算は `kamoku-books` の関数を呼ぶだけ。期間の検査（1つの暦年）もそちら。

use super::{ApiError, Call, Reply, ok, to_value};
use crate::query::Query;
use crate::state::State;
use kamoku_books::{
    AccountLedger, BalanceSheet, IncomeStatement, TaxSummary, TrialBalance, account_ledger,
    balance_sheet as books_balance_sheet, income_statement as books_income_statement,
    tax_summary as books_tax_summary, trial_balance as books_trial_balance,
};
use kamoku_ledger::Entry;
use kamoku_store::Store;
use time::{Date, Month};

/// 期間。`year=2026` は1月1日〜12月31日の略記。`from` と `to` は両端を含む。
/// どれも無ければ、今日の年。`from` と `to` の片方だけは形の誤り。
pub(crate) fn period(query: &Query, today: Date) -> Result<(Date, Date), ApiError> {
    if let Some(text) = query.get("year") {
        let year = text
            .parse::<i32>()
            .ok()
            .filter(|y| (1000..=9999).contains(y))
            .ok_or_else(|| {
                ApiError::bad_request("year は4桁の年で入れてください。").with_field("year")
            })?;
        return year_bounds(year)
            .ok_or_else(|| ApiError::bad_request("year を読めません。").with_field("year"));
    }
    match (query.date("from")?, query.date("to")?) {
        (Some(from), Some(to)) => Ok((from, to)),
        (None, None) => year_bounds(today.year()).ok_or_else(|| ApiError::internal("今日の年")),
        (None, Some(_)) => Err(ApiError::bad_request("from も入れてください。").with_field("from")),
        (Some(_), None) => Err(ApiError::bad_request("to も入れてください。").with_field("to")),
    }
}

pub(crate) fn year_bounds(year: i32) -> Option<(Date, Date)> {
    Some((
        Date::from_calendar_date(year, Month::January, 1).ok()?,
        Date::from_calendar_date(year, Month::December, 31).ok()?,
    ))
}

/// 仕訳Noと仕訳の組（総勘定元帳が使う）。
pub(crate) fn numbered(store: &Store) -> Vec<(u64, &Entry)> {
    store
        .ledger()
        .current()
        .into_iter()
        .filter_map(|e| Some((store.entry_no(e.id())?, e)))
        .collect()
}

pub(crate) fn by_month(query: &Query) -> Result<bool, ApiError> {
    match query.get("by") {
        None | Some("total") => Ok(false),
        Some("month") => Ok(true),
        Some(_) => {
            Err(ApiError::bad_request("by は total か month にしてください。").with_field("by"))
        }
    }
}

pub(crate) fn make_trial_balance(state: &State, call: &Call<'_>) -> Result<TrialBalance, ApiError> {
    let (from, to) = period(call.query, call.today)?;
    let entries = state.store.ledger().current();
    Ok(books_trial_balance(
        &entries,
        state.store.chart(),
        from,
        to,
    )?)
}

pub(crate) fn make_income_statement(
    state: &State,
    call: &Call<'_>,
) -> Result<IncomeStatement, ApiError> {
    let (from, to) = period(call.query, call.today)?;
    let monthly = by_month(call.query)?;
    let entries = state.store.ledger().current();
    Ok(books_income_statement(
        &entries,
        state.store.chart(),
        from,
        to,
        monthly,
    )?)
}

pub(crate) fn make_balance_sheet(state: &State, call: &Call<'_>) -> Result<BalanceSheet, ApiError> {
    let as_of = call.query.date("as_of")?.unwrap_or(call.today);
    let entries = state.store.ledger().current();
    Ok(books_balance_sheet(&entries, state.store.chart(), as_of)?)
}

pub(crate) fn make_ledger(state: &State, call: &Call<'_>) -> Result<AccountLedger, ApiError> {
    let account = call.query.get("account").ok_or_else(|| {
        ApiError::bad_request("account（科目のコード）を入れてください。").with_field("account")
    })?;
    let (from, to) = period(call.query, call.today)?;
    let entries = numbered(&state.store);
    Ok(account_ledger(
        &entries,
        state.store.chart(),
        account,
        from,
        to,
    )?)
}

pub(crate) fn make_tax_summary(state: &State, call: &Call<'_>) -> Result<TaxSummary, ApiError> {
    let (from, to) = period(call.query, call.today)?;
    let entries = state.store.ledger().current();
    Ok(books_tax_summary(&entries, state.store.chart(), from, to)?)
}

/// `GET /reports/trial-balance?from&to|year`
pub(crate) fn trial_balance(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    ok(to_value(&make_trial_balance(state, call)?)?)
}

/// `GET /reports/income-statement?from&to&by=total|month`
pub(crate) fn income_statement(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    ok(to_value(&make_income_statement(state, call)?)?)
}

/// `GET /reports/balance-sheet?as_of=`（既定は今日）
pub(crate) fn balance_sheet(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    ok(to_value(&make_balance_sheet(state, call)?)?)
}

/// `GET /reports/ledger?account&from&to`
pub(crate) fn ledger(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    ok(to_value(&make_ledger(state, call)?)?)
}

/// `GET /reports/tax-summary?from&to`
pub(crate) fn tax_summary(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    ok(to_value(&make_tax_summary(state, call)?)?)
}
