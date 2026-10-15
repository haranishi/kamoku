// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳の一覧と詳細（`GET /entries`、`GET /entries/{id}`）。

use super::entry_view::{Current, all_current, detail_json, entry_id, entry_json};
use super::{ApiError, Call, Reply, ok};
use crate::query::Query;
use crate::state::State;
use kamoku_books::{Chart, MAX_REPORT_AMOUNT};
use serde_json::{Value, json};
use std::cmp::Reverse;
use time::Date;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StateFilter {
    Active,
    Voided,
    All,
}

struct Filter {
    from: Option<Date>,
    to: Option<Date>,
    account: Option<String>,
    wallet: Option<String>,
    text: Option<String>,
    kind: Option<String>,
    source: Option<String>,
    state: StateFilter,
    min: Option<i64>,
    max: Option<i64>,
}

fn choice(query: &Query, key: &str, choices: &[&str]) -> Result<Option<String>, ApiError> {
    match query.get(key) {
        None => Ok(None),
        Some(v) if choices.contains(&v) => Ok(Some(v.to_owned())),
        Some(_) => Err(ApiError::bad_request(format!(
            "{key} は {} のどれかにしてください。",
            choices.join("・")
        ))
        .with_field(key)),
    }
}

impl Filter {
    fn read(query: &Query) -> Result<Self, ApiError> {
        let state = match choice(query, "state", &["active", "voided", "all"])?.as_deref() {
            Some("voided") => StateFilter::Voided,
            Some("all") => StateFilter::All,
            _ => StateFilter::Active,
        };
        Ok(Self {
            from: query.date("from")?,
            to: query.date("to")?,
            account: query.get("account").map(str::to_owned),
            wallet: query.get("wallet").map(str::to_owned),
            text: query
                .get("q")
                .map(|q| q.trim().to_owned())
                .filter(|q| !q.is_empty()),
            kind: choice(
                query,
                "kind",
                &["expense", "income", "transfer", "journal", "opening"],
            )?,
            source: choice(
                query,
                "source",
                &["manual", "import", "template", "opening", "demo"],
            )?,
            state,
            min: query.int("min")?,
            max: query.int("max")?,
        })
    }

    fn matches(&self, c: &Current<'_>, chart: &Chart) -> bool {
        let entry = c.entry;
        let total = entry.total().get();
        let has_account = |code: &str| entry.lines().iter().any(|l| l.account.as_str() == code);
        let state_ok = match self.state {
            StateFilter::Active => !c.voided(),
            StateFilter::Voided => c.voided(),
            StateFilter::All => true,
        };
        state_ok
            && self.from.is_none_or(|d| entry.date() >= d)
            && self.to.is_none_or(|d| entry.date() <= d)
            && self.account.as_deref().is_none_or(has_account)
            && self.wallet.as_deref().is_none_or(has_account)
            && self.kind.as_deref().is_none_or(|k| c.meta("kind") == k)
            && self.source.as_deref().is_none_or(|s| c.meta("source") == s)
            && self.min.is_none_or(|m| total >= m)
            && self.max.is_none_or(|m| total <= m)
            && self
                .text
                .as_deref()
                .is_none_or(|q| text_matches(c, chart, q))
    }
}

// 文字列は、摘要・取引先・メモ・科目名と、金額（カンマや円を除いた数字）に効く。
fn text_matches(c: &Current<'_>, chart: &Chart, needle: &str) -> bool {
    let entry = c.entry;
    let in_text = entry.description().contains(needle)
        || c.meta("partner").contains(needle)
        || entry.lines().iter().any(|l| {
            l.memo.contains(needle)
                || chart
                    .name(l.account.as_str())
                    .is_some_and(|name| name.contains(needle))
        });
    if in_text {
        return true;
    }
    let digits: String = needle
        .chars()
        .filter(|ch| !matches!(ch, ',' | '，' | '円' | '¥' | '￥'))
        .collect();
    !digits.is_empty()
        && digits.chars().all(|ch| ch.is_ascii_digit())
        && std::iter::once(entry.total())
            .chain(entry.lines().iter().map(|l| l.amount))
            .any(|amount| amount.get().to_string().contains(&digits))
}

fn page_rows<'a, 'entry>(
    rows: &'a mut [Current<'entry>],
    limit: u32,
    offset: u64,
) -> &'a mut [Current<'entry>] {
    let start = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(rows.len());
    if start == rows.len() {
        return &mut rows[start..];
    }
    let key = |row: &Current<'_>| Reverse((row.entry.date(), row.no));
    if start > 0 {
        rows.select_nth_unstable_by_key(start, key);
    }
    let remaining = &mut rows[start..];
    let count = usize::try_from(limit)
        .unwrap_or(usize::MAX)
        .min(remaining.len());
    if count < remaining.len() {
        remaining.select_nth_unstable_by_key(count, key);
    }
    let page = &mut remaining[..count];
    page.sort_unstable_by_key(key);
    page
}

/// `GET /entries`。日付の新しい順、同じ日は番号の大きい順。`state` の既定は `active`。
/// `sum_amount` は、絞り込んだ全件（ページの外も含む）の合計。
pub(crate) fn entries_list(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let filter = Filter::read(call.query)?;
    let (limit, offset) = call.query.page()?;
    let chart = state.store.chart();
    let mut rows: Vec<Current<'_>> = all_current(&state.store)
        .filter(|c| filter.matches(c, chart))
        .collect();
    let total = rows.len();
    let sum = rows
        .iter()
        .try_fold(0i64, |sum, c| sum.checked_add(c.entry.total().get()))
        .filter(|s| *s <= MAX_REPORT_AMOUNT)
        .ok_or_else(|| {
            ApiError::new(
                422,
                "amount_range",
                "合計が大きすぎて、正確に表示できません。期間を短くしてください。",
            )
        })?;
    let items: Vec<Value> = page_rows(&mut rows, limit, offset)
        .iter()
        .map(|c| entry_json(c, chart))
        .collect();
    ok(json!({ "items": items, "total": total, "sum_amount": sum }))
}

/// `GET /entries/{id}`。表現に、組み立て直した入力（`input`）と履歴（`history`）を足す。
pub(crate) fn entry_detail(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let id = entry_id(call.param(0))?;
    ok(detail_json(&state.store, &id, true)?)
}

#[cfg(test)]
#[path = "entry_list_tests.rs"]
mod tests;
