// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! ホームのまとめ（`GET /reports/dashboard?month=YYYY-MM`）。画面の最初の描画に要る数字を1回で返す。
//!
//! 仕様が決めていない所は、次のように決めてある。今年のまとめ（`ytd`）は
//! 1月1日からその月の末まで、経費の上位5科目はその月、口座の残高と消費税の見込みは
//! 「その月の末と今日の早い方」（ただし、その年の1月1日より前にはしない）の時点。

use super::{ApiError, Call, Reply, ok, to_value};
use crate::state::State;
use kamoku_books::{
    IncomeStatement, format_date, income_statement, monthly_series, tax_summary, wallet_balances,
};
use kamoku_store::{Store, StoreError};
use serde_json::{Value, json};
use time::{Date, Month};

fn month_of(text: &str) -> Option<(i32, Month)> {
    let (year, month) = text.split_once('-')?;
    if year.len() != 4 || month.len() != 2 {
        return None;
    }
    let year: i32 = year.parse().ok()?;
    let month = Month::try_from(month.parse::<u8>().ok()?).ok()?;
    Some((year, month))
}

fn bounds(year: i32, month: Month) -> Option<(Date, Date)> {
    let from = Date::from_calendar_date(year, month, 1).ok()?;
    let to = Date::from_calendar_date(year, month, month.length(year)).ok()?;
    Some((from, to))
}

fn summary(statement: &IncomeStatement) -> Value {
    json!({
        "revenue": statement.revenue.total.get(),
        "expense": statement.expense.total.get(),
        "profit": statement.net_income.get(),
    })
}

// 合計は、ブラウザの数値で正確に扱える範囲（data-model.md の決まり16）を超えたら断る。
fn inbox_out_amount(store: &Store) -> Result<i64, ApiError> {
    store.pending_out_amount().map_err(|error| match error {
        StoreError::AmountRange => {
            ApiError::new(422, "amount_range", "未確認の出金の合計が大きすぎます。")
        }
        other => other.into(),
    })
}

/// `GET /reports/dashboard?month=YYYY-MM`（既定は今月）
pub(crate) fn dashboard(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let (year, month) = match call.query.get("month") {
        Some(text) => month_of(text).ok_or_else(|| {
            ApiError::bad_request("month は YYYY-MM で入れてください。").with_field("month")
        })?,
        None => (call.today.year(), call.today.month()),
    };
    let bad = || ApiError::bad_request("month の日付を作れません。").with_field("month");
    let (from, to) = bounds(year, month).ok_or_else(bad)?;
    let prev_day = from.previous_day().ok_or_else(bad)?;
    let (prev_from, prev_to) = bounds(prev_day.year(), prev_day.month()).ok_or_else(bad)?;
    let year_start = Date::from_calendar_date(year, Month::January, 1).map_err(|_| bad())?;
    let as_of = to.min(call.today).max(year_start);
    let store = &state.store;
    let chart = store.chart();
    let entries = store.ledger().current();
    let current = income_statement(&entries, chart, from, to, false)?;
    let previous = income_statement(&entries, chart, prev_from, prev_to, false)?;
    let ytd = income_statement(&entries, chart, year_start, to, false)?;
    let mut top: Vec<_> = current
        .expense
        .rows
        .iter()
        .filter(|r| r.amount.get() != 0)
        .collect();
    top.sort_by(|a, b| b.amount.cmp(&a.amount).then_with(|| a.code.cmp(&b.code)));
    let top: Vec<Value> = top
        .into_iter()
        .take(5)
        .map(|r| json!({ "account": r.code, "name": r.name, "amount": r.amount.get() }))
        .collect();
    let tax = if store.settings().tax_mode == "taxable" {
        let t = tax_summary(&entries, chart, year_start, as_of)?;
        json!({
            "applicable": true,
            "sales_tax": t.sales_tax.get(),
            "purchase_tax": t.purchase_tax.get(),
            "estimate": t.estimate.get(),
            "as_of": format_date(as_of),
        })
    } else {
        json!({ "applicable": false })
    };
    let mut month_summary = summary(&current);
    if let Value::Object(map) = &mut month_summary {
        map.insert("prev".to_owned(), summary(&previous));
    }
    ok(json!({
        "month": format!("{year:04}-{:02}", u8::from(month)),
        "fiscal_year": year,
        "next_actions": {
            "inbox_draft": store.draft_counts()?.draft,
            "inbox_out_amount": inbox_out_amount(store)?,
        },
        "month_summary": month_summary,
        "ytd": summary(&ytd),
        "wallets": to_value(&wallet_balances(&entries, chart, as_of)?)?,
        "series": to_value(&monthly_series(&entries, chart, year)?)?,
        "top_expenses": top,
        "tax": tax,
        "ledger": {
            "events": store.ledger().events().len(),
            "head_hash": store.ledger().head_hash().to_hex(),
            "verified": store.read_only().is_none(),
        },
    }))
}
