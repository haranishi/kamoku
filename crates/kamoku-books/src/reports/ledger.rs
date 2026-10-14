// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ReportError, account, add, carried_opening, check_period, is_opening, signed};
use crate::{Chart, ser};
use kamoku_ledger::{Entry, Line, Side, Yen};
use serde::Serialize;
use time::Date;

/// 総勘定元帳の1行（1明細）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LedgerRow {
    /// 取引の日付。
    #[serde(with = "ser::date")]
    pub date: Date,
    /// 仕訳No。
    pub no: u64,
    /// 仕訳の識別子。
    pub entry_id: String,
    /// 仕訳の摘要。
    pub description: String,
    /// 相手科目の名前。反対側に科目が2つ以上あれば「諸口」。
    pub counter: String,
    /// 借方の金額（貸方の行は0）。
    #[serde(serialize_with = "ser::yen")]
    pub debit: Yen,
    /// 貸方の金額（借方の行は0）。
    #[serde(serialize_with = "ser::yen")]
    pub credit: Yen,
    /// この行までの残高（通常の側から見た値）。
    #[serde(serialize_with = "ser::yen")]
    pub balance: Yen,
}
/// 総勘定元帳（`docs/spec/api-reports.md`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountLedger {
    /// 科目のコード。
    pub account: String,
    /// 期首の残高。試算表の `opening` と同じ値。
    #[serde(serialize_with = "ser::yen")]
    pub opening: Yen,
    /// 日付順（同じ日は仕訳No順、同じ仕訳は明細の順）の行。
    pub rows: Vec<LedgerRow>,
    /// 期末の残高。試算表の `closing` と同じ値。
    #[serde(serialize_with = "ser::yen")]
    pub closing: Yen,
}

/// 1つの科目の総勘定元帳を作ります（AC-08.4）。`entries` は仕訳Noと仕訳の組です。
///
/// # Errors
/// 期間の誤り、年をまたぐ期間、知らない科目、集計値の上限超えを返します。
pub fn account_ledger(
    entries: &[(u64, &Entry)],
    chart: &Chart,
    code: &str,
    from: Date,
    to: Date,
) -> Result<AccountLedger, ReportError> {
    check_period(from, to)?;
    let acc = account(chart, code)?;
    let plain: Vec<&Entry> = entries.iter().map(|(_, e)| *e).collect();
    let opening = carried_opening(&plain, chart, from)?
        .get(code)
        .copied()
        .unwrap_or_default();
    let mut picked: Vec<(Date, u64, usize, &Entry, &Line)> = Vec::new();
    for (no, entry) in entries
        .iter()
        .filter(|(_, e)| from <= e.date() && e.date() <= to)
        .filter(|(_, e)| !(is_opening(e) && e.date() == from))
    {
        for (index, line) in entry.lines().iter().enumerate() {
            if line.account.as_str() == code {
                picked.push((entry.date(), *no, index, entry, line));
            }
        }
    }
    picked.sort_by_key(|(date, no, index, _, _)| (*date, *no, *index));
    let mut balance = opening;
    let mut rows = Vec::with_capacity(picked.len());
    for (date, no, _, entry, line) in picked {
        balance = add(balance, signed(line, acc.normal_side())?)?;
        let (debit, credit) = match line.side {
            Side::Debit => (line.amount, Yen::ZERO),
            Side::Credit => (Yen::ZERO, line.amount),
        };
        rows.push(LedgerRow {
            date,
            no,
            entry_id: entry.id().as_str().to_owned(),
            description: entry.description().to_owned(),
            counter: counter(entry, line, chart)?,
            debit,
            credit,
            balance,
        });
    }
    Ok(AccountLedger {
        account: code.to_owned(),
        opening,
        rows,
        closing: balance,
    })
}

fn counter(entry: &Entry, line: &Line, chart: &Chart) -> Result<String, ReportError> {
    let mut codes: Vec<&str> = entry
        .lines()
        .iter()
        .filter(|l| l.side != line.side)
        .map(|l| l.account.as_str())
        .collect();
    codes.sort_unstable();
    codes.dedup();
    match codes.as_slice() {
        [only] => Ok(account(chart, only)?.name.clone()),
        [] => Ok(String::new()),
        _ => Ok("諸口".to_owned()),
    }
}
