// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ReportError, account, add, check_period, signed, sub};
use crate::{Chart, Kind, ser};
use kamoku_ledger::{Entry, Yen};
use serde::Serialize;
use std::collections::BTreeMap;
use time::{Date, Month};

/// 帳票の区分（行と合計）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section<R> {
    /// 行。
    pub rows: Vec<R>,
    /// 合計。
    #[serde(serialize_with = "ser::yen")]
    pub total: Yen,
}
/// 損益計算書の1行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IsRow {
    /// 科目のコード。
    pub code: String,
    /// 科目の名前。
    pub name: String,
    /// 期間の金額（収益は貸方−借方、費用は借方−貸方）。
    #[serde(serialize_with = "ser::yen")]
    pub amount: Yen,
    /// 月別のとき、1月〜12月の金額（期間の外の月は0）。
    #[serde(
        serialize_with = "ser::months",
        skip_serializing_if = "Option::is_none"
    )]
    pub months: Option<[Yen; 12]>,
}
/// 月別の損益計算書の、月ごとの合計。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonthTotal {
    /// 月（`YYYY-MM`）。
    pub month: String,
    /// 収益の合計。
    #[serde(serialize_with = "ser::yen")]
    pub revenue: Yen,
    /// 費用の合計。
    #[serde(serialize_with = "ser::yen")]
    pub expense: Yen,
    /// 収益−費用。
    #[serde(serialize_with = "ser::yen")]
    pub net_income: Yen,
}
/// 損益計算書（`docs/spec/api-reports.md`）。事業主貸と事業主借は入らない（AC-07.3）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IncomeStatement {
    /// 収益。
    pub revenue: Section<IsRow>,
    /// 費用。
    pub expense: Section<IsRow>,
    /// 収益−費用。
    #[serde(serialize_with = "ser::yen")]
    pub net_income: Yen,
    /// 月別のとき、1月〜12月の合計。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub months: Option<Vec<MonthTotal>>,
}
/// ホームの12か月の推移の1点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonthPoint {
    /// 月（`YYYY-MM`）。
    pub month: String,
    /// 売上（収益の合計）。
    #[serde(serialize_with = "ser::yen")]
    pub revenue: Yen,
    /// 経費（費用の合計）。
    #[serde(serialize_with = "ser::yen")]
    pub expense: Yen,
    /// 利益。
    #[serde(serialize_with = "ser::yen")]
    pub profit: Yen,
}

/// 損益計算書を作ります。`by_month` なら、行ごとに1月〜12月の金額も付けます（AC-08.3）。
///
/// # Errors
/// 期間の誤り、年をまたぐ期間、知らない科目、集計値の上限超えを返します。
pub fn income_statement(
    entries: &[&Entry],
    chart: &Chart,
    from: Date,
    to: Date,
    by_month: bool,
) -> Result<IncomeStatement, ReportError> {
    check_period(from, to)?;
    let mut sums = BTreeMap::<&str, [Yen; 12]>::new();
    for entry in entries
        .iter()
        .filter(|e| from <= e.date() && e.date() <= to)
    {
        let month = usize::from(u8::from(entry.date().month())) - 1;
        for line in entry.lines() {
            let acc = account(chart, line.account.as_str())?;
            if !matches!(acc.kind, Kind::Revenue | Kind::Expense) {
                continue;
            }
            let slots = sums.entry(acc.code.as_str()).or_default();
            if let Some(slot) = slots.get_mut(month) {
                *slot = add(*slot, signed(line, acc.normal_side())?)?;
            }
        }
    }
    let mut revenue = Section {
        rows: Vec::new(),
        total: Yen::ZERO,
    };
    let mut expense = revenue.clone();
    let mut revenue_months = [Yen::ZERO; 12];
    let mut expense_months = [Yen::ZERO; 12];
    for acc in chart {
        let Some(months) = sums.get(acc.code.as_str()) else {
            continue;
        };
        let amount = months.iter().try_fold(Yen::ZERO, |s, v| add(s, *v))?;
        let (section, totals) = match acc.kind {
            Kind::Revenue => (&mut revenue, &mut revenue_months),
            _ => (&mut expense, &mut expense_months),
        };
        section.total = add(section.total, amount)?;
        for (total, value) in totals.iter_mut().zip(months) {
            *total = add(*total, *value)?;
        }
        section.rows.push(IsRow {
            code: acc.code.as_str().to_owned(),
            name: acc.name.clone(),
            amount,
            months: by_month.then_some(*months),
        });
    }
    let net_income = sub(revenue.total, expense.total)?;
    let months = if by_month {
        let mut out = Vec::new();
        for (i, (r, e)) in revenue_months.iter().zip(&expense_months).enumerate() {
            out.push(MonthTotal {
                month: month_label(from.year(), i),
                revenue: *r,
                expense: *e,
                net_income: sub(*r, *e)?,
            });
        }
        Some(out)
    } else {
        None
    };
    Ok(IncomeStatement {
        revenue,
        expense,
        net_income,
        months,
    })
}

fn month_label(year: i32, index: usize) -> String {
    format!("{year:04}-{:02}", index + 1)
}

/// ホームの12か月の推移（1月〜12月。月がなければ0）。損益計算書と同じ計算から作ります。
///
/// # Errors
/// 知らない科目、集計値の上限超えを返します。
pub fn monthly_series(
    entries: &[&Entry],
    chart: &Chart,
    year: i32,
) -> Result<Vec<MonthPoint>, ReportError> {
    let from =
        Date::from_calendar_date(year, Month::January, 1).map_err(|_| ReportError::BadPeriod)?;
    let to =
        Date::from_calendar_date(year, Month::December, 31).map_err(|_| ReportError::BadPeriod)?;
    let statement = income_statement(entries, chart, from, to, true)?;
    Ok(statement
        .months
        .unwrap_or_default()
        .into_iter()
        .map(|m| MonthPoint {
            month: m.month,
            revenue: m.revenue,
            expense: m.expense,
            profit: m.net_income,
        })
        .collect())
}
