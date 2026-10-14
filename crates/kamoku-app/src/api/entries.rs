// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳を作る・直す・取り消す（`POST /entries/preview` `POST /entries` `PUT /entries/{id}`
//! `POST /entries/{id}/void`）。明細は `kamoku-books` の組み立てだけで作る（プレビューと保存で同じ）。

use super::body::{Fields, object};
use super::client_ids::{self, CLIENT_ID_MAX_LEN, Seen};
use super::entry_view::{current, detail_json, entry_id};
use super::input::{compose, compose_settings, parse_entry};
use super::{ApiError, Call, Reply, created, ok, to_value};
use crate::state::State;
use crate::{clock, ids};
use kamoku_books::Source;
use kamoku_ledger::{Entry, EntryId};
use kamoku_store::{NewPartner, Store};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::time::Instant;

/// 新しい仕訳ID（台帳にも `taken` にも無いもの）。
pub(crate) fn fresh_id(store: &Store, taken: &mut HashSet<EntryId>) -> Result<EntryId, ApiError> {
    for _ in 0..16 {
        let mut random = [0u8; 4];
        getrandom::fill(&mut random).map_err(ApiError::internal)?;
        let id = EntryId::new(&ids::entry_id(
            clock::now_millis(),
            u32::from_be_bytes(random),
        ))
        .map_err(ApiError::internal)?;
        if store.ledger().history(&id).is_empty() && taken.insert(id.clone()) {
            return Ok(id);
        }
    }
    Err(ApiError::internal(
        "仕訳IDを作れませんでした（重なりが続いた）",
    ))
}

/// 取引先の扱い。マスタに無い名前は、保存と同じトランザクションで足す。登録番号があれば
/// 付随情報 `invoice_no` に入れる（その時点の番号を台帳に残すため）。
pub(crate) fn partner_link(
    store: &Store,
    name: &str,
) -> Result<(Option<NewPartner>, Option<String>), ApiError> {
    if name.is_empty() {
        return Ok((None, None));
    }
    Ok(
        match store.partners()?.into_iter().find(|p| p.name == name) {
            Some(partner) => (None, partner.invoice_number),
            None => (
                Some(NewPartner {
                    name: name.to_owned(),
                    ..NewPartner::default()
                }),
                None,
            ),
        },
    )
}

pub(crate) fn with_invoice(entry: Entry, invoice: Option<&str>) -> Result<Entry, ApiError> {
    match invoice {
        Some(number) => entry
            .with_meta("invoice_no", number)
            .map_err(ApiError::internal),
        None => Ok(entry),
    }
}

/// `POST /entries/preview`。保存せずに、保存で作られるのと同じ明細を返す。
pub(crate) fn entry_preview(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let input = parse_entry(&map)?;
    let settings = compose_settings(state.store.settings(), Source::Manual);
    let chart = state.store.chart();
    let composed = compose(&input, chart, settings)?;
    // 組み立ては仕訳の検査まで通してあるので、合計は桁あふれしない。念のため失敗は断る。
    let total = composed
        .total()
        .ok_or_else(|| ApiError::new(422, "amount_range", "金額の合計が大きすぎます。"))?;
    ok(json!({
        "lines": to_value(&composed.line_views(chart))?,
        "total": total.get(),
        "warnings": composed.warnings,
    }))
}

fn client_id(map: &Map<String, Value>) -> Result<Option<String>, ApiError> {
    let id = Fields::new(map)
        .opt_str("client_id")?
        .filter(|s| !s.is_empty());
    if id.as_ref().is_some_and(|s| s.len() > CLIENT_ID_MAX_LEN) {
        return Err(
            ApiError::bad_request("client_id が長すぎます（64バイトまで）。")
                .with_field("client_id"),
        );
    }
    Ok(id)
}

/// `POST /entries` → 201。同じ `client_id` が10分以内にもう一度来たら、作らずに最初の結果を200で返す。
/// 再送防止の表がいっぱいなら、新しい印の要求は保存の前に `503 busy`（[`client_ids`]）。
pub(crate) fn entry_create(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let client = client_id(&map)?;
    let now = Instant::now();
    let remember = match client {
        Some(id) => {
            let digest = client_ids::digest(&map)?;
            match state.clients.check(&id, &digest, now)? {
                Seen::Again(response) => return Ok(Reply::Json(200, response)),
                Seen::New => Some((id, digest)),
            }
        }
        None => None,
    };
    let input = parse_entry(&map)?;
    let settings = compose_settings(state.store.settings(), Source::Manual);
    let composed = compose(&input, state.store.chart(), settings)?;
    let (partner, invoice) = partner_link(&state.store, input.partner())?;
    let id = fresh_id(&state.store, &mut HashSet::new())?;
    let entry = composed.to_entry(id.clone()).map_err(ApiError::internal)?;
    let entry = with_invoice(entry, invoice.as_deref())?;
    state.store.post(entry, call.now, partner)?;
    let response = detail_json(&state.store, &id, false)?;
    if let Some((client, digest)) = remember {
        state.clients.insert(client, digest, response.clone(), now);
    }
    created(response)
}

fn reason(f: &Fields<'_>) -> Result<String, ApiError> {
    let reason = f.opt_str("reason")?.unwrap_or_default();
    if reason.trim().is_empty() {
        return Err(ApiError::new(
            422,
            "reason_required",
            "理由を入れてください（空白だけは使えません）。",
        )
        .with_field("reason"));
    }
    Ok(reason)
}

fn expected_revision(f: &Fields<'_>) -> Result<u32, ApiError> {
    let value = f
        .opt_int("expected_revision")?
        .ok_or_else(|| f.missing("expected_revision", "見ていた版"))?;
    u32::try_from(value).map_err(|_| {
        ApiError::bad_request("expected_revision は1以上の整数で入れてください。")
            .with_field("expected_revision")
    })
}

const OPENING_MESSAGE: &str = "期首残高は、設定の「期首残高」から見直してください。";

/// `PUT /entries/{id}`。理由と、画面が見ていた版が必須。元の仕訳の出どころ（`source` `draft`）は引き継ぐ。
pub(crate) fn entry_revise(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let id = entry_id(call.param(0))?;
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let (source, draft) = {
        let c = current(&state.store, &id)
            .ok_or_else(|| ApiError::not_found("仕訳が見つかりません。"))?;
        if c.meta("kind") == "opening" {
            return Err(ApiError::rule(OPENING_MESSAGE));
        }
        let draft = c.entry.meta().get("draft").cloned();
        (
            Source::parse(c.meta("source")).unwrap_or(Source::Manual),
            draft,
        )
    };
    let reason = reason(&f)?;
    let expected = expected_revision(&f)?;
    let input = parse_entry(&map)?;
    let settings = compose_settings(state.store.settings(), source);
    let composed = compose(&input, state.store.chart(), settings)?;
    let (partner, invoice) = partner_link(&state.store, input.partner())?;
    let mut entry = composed.to_entry(id.clone()).map_err(ApiError::internal)?;
    if let Some(draft) = draft {
        entry = entry
            .with_meta("draft", &draft)
            .map_err(ApiError::internal)?;
    }
    let entry = with_invoice(entry, invoice.as_deref())?;
    state
        .store
        .revise(entry, expected, &reason, call.now, partner)?;
    ok(detail_json(&state.store, &id, false)?)
}

/// `POST /entries/{id}/void`。本文 `{ reason, expected_revision }`（どちらも必須）。
pub(crate) fn entry_void(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let id = entry_id(call.param(0))?;
    let map = object(call.body)?;
    let f = Fields::new(&map);
    {
        let c = current(&state.store, &id)
            .ok_or_else(|| ApiError::not_found("仕訳が見つかりません。"))?;
        if c.meta("kind") == "opening" {
            return Err(ApiError::rule(OPENING_MESSAGE));
        }
    }
    let reason = reason(&f)?;
    let expected = expected_revision(&f)?;
    state.store.void(&id, expected, &reason, call.now)?;
    ok(detail_json(&state.store, &id, false)?)
}
