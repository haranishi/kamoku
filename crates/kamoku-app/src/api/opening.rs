// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 期首残高（`GET /opening`、`PUT /opening`）。組み立ては `kamoku-books` の `compose_opening`。

use super::body::{Fields, object};
use super::entries::fresh_id;
use super::entry_view::{Current, all_current};
use super::{ApiError, Call, Reply, ok};
use crate::state::State;
use kamoku_books::{Kind, compose_opening, format_date, parse_date};
use kamoku_ledger::{AccountCode, Side, Yen};
use kamoku_store::Store;
use serde_json::{Value, json};
use std::collections::HashSet;
use time::Date;

const REASON_REVISE: &str = "期首残高の見直し";
const REASON_NONE: &str = "期首残高なし";

// いま有効な期首残高の仕訳（取消済みは除く）。あれば最後に登録したもの。
fn active_opening(store: &Store) -> Option<Current<'_>> {
    all_current(store)
        .filter(|c| !c.voided() && c.meta("kind") == "opening")
        .max_by_key(|c| c.no)
}

fn opening_json(store: &Store) -> Value {
    let chart = store.chart();
    let Some(c) = active_opening(store) else {
        return json!({
            "date": store.settings().opened_on,
            "balances": [],
            "capital": 0,
            "entry_id": null,
        });
    };
    let mut balances = Vec::new();
    let mut capital = 0i64;
    for line in c.entry.lines() {
        let amount = line.amount.get();
        if line.account.as_str() == "capital" {
            // 元入金は、資産−負債（債務超過なら負）で返す。
            capital = if line.side == Side::Credit {
                amount
            } else {
                -amount
            };
            continue;
        }
        let kind = chart.kind(line.account.as_str());
        let expected = match kind {
            Some(Kind::Liability) => Side::Credit,
            _ => Side::Debit,
        };
        let signed = if line.side == expected {
            amount
        } else {
            -amount
        };
        balances.push(json!({ "account": line.account.as_str(), "amount": signed }));
    }
    json!({
        "date": format_date(c.entry.date()),
        "balances": balances,
        "capital": capital,
        "entry_id": c.latest.entry_id.as_str(),
    })
}

/// `GET /opening`。期首残高の仕訳が無ければ、日付は開業日、`entry_id` は `null`。
pub(crate) fn opening_get(state: &mut State, _call: &Call<'_>) -> Result<Reply, ApiError> {
    ok(opening_json(&state.store))
}

fn opening_date(f: &Fields<'_>, opened_on: &str) -> Result<Date, ApiError> {
    let opened = parse_date(opened_on);
    let date = match (f.opt_date("date")?, opened) {
        (Some(date), _) => date,
        (None, Some(opened)) => opened,
        (None, None) => {
            return Err(ApiError::rule(
                "期首残高の日付を入れるか、先に設定で開業日を決めてください。",
            )
            .with_field("date"));
        }
    };
    // 最初の会計年度（開業日の暦年）の中の日付にする。
    if opened.is_some_and(|o| o.year() != date.year()) {
        return Err(ApiError::rule(format!(
            "期首残高の日付は、最初の会計年度（{}年）の中にしてください。",
            opened.map_or(date.year(), Date::year)
        ))
        .with_field("date"));
    }
    Ok(date)
}

fn balances(f: &Fields<'_>) -> Result<Vec<(AccountCode, Yen)>, ApiError> {
    let rows = f
        .opt_array("balances")?
        .ok_or_else(|| f.missing("balances", "残高の行"))?;
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let Value::Object(map) = row else {
            return Err(
                ApiError::bad_request("残高の行は { account, amount } にしてください。")
                    .with_field(format!("balances[{i}]")),
            );
        };
        let rf = Fields::nested(map, format!("balances[{i}]."));
        let account = rf.req_str("account", "科目")?;
        let code = AccountCode::new(&account).map_err(|_| {
            ApiError::new(
                422,
                "unknown_account",
                format!("科目・口座「{account}」が見つかりません。"),
            )
            .with_field(rf.name("account"))
        })?;
        let amount = rf
            .opt_amount("amount")?
            .ok_or_else(|| rf.missing("amount", "金額"))?;
        out.push((code, Yen::new(amount)));
    }
    Ok(out)
}

/// `PUT /opening`。本文 `{ date, balances: [{ account, amount }] }`。
/// すでに仕訳があれば「期首残高の見直し」で訂正し、入れる行が無くなれば「期首残高なし」で取り消す。
pub(crate) fn opening_put(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let date = opening_date(&f, &state.store.settings().opened_on)?;
    let rows = balances(&f)?;
    let composed = compose_opening(date, &rows, state.store.chart())?;
    let existing = active_opening(&state.store).map(|c| {
        (
            c.latest.entry_id.clone(),
            c.latest.revision,
            c.entry.clone(),
        )
    });
    match (existing, composed) {
        (Some((id, revision, old)), Some(composed)) => {
            let entry = composed.to_entry(id).map_err(ApiError::internal)?;
            // 同じ内容なら、訂正の版を増やさない。
            if entry != old {
                state
                    .store
                    .revise(entry, revision, REASON_REVISE, call.now, None)?;
            }
        }
        (Some((id, revision, _)), None) => {
            state.store.void(&id, revision, REASON_NONE, call.now)?;
        }
        (None, Some(composed)) => {
            let id = fresh_id(&state.store, &mut HashSet::new())?;
            let entry = composed.to_entry(id).map_err(ApiError::internal)?;
            state.store.post(entry, call.now, None)?;
        }
        (None, None) => {}
    }
    ok(opening_json(&state.store))
}
