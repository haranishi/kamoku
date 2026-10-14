// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 状態と検査（`GET /status`、`POST /verify`）。

use super::body::{Fields, object};
use super::{ApiError, Call, Reply, ok};
use crate::state::State;
use kamoku_ledger::{EventKind, Hash};
use kamoku_store::VerifyFailure;
use serde_json::{Value, json};

// 保存の層の移行の版（`PRAGMA user_version`）。保存の層はこの値を公開していないので、移行を足したら合わせる。
const SCHEMA_VERSION: u32 = 1;

pub(crate) fn failure_json(failure: &VerifyFailure) -> Value {
    json!({ "seq": failure.seq, "kind": failure.kind, "rule": failure.rule })
}

/// `GET /status`
pub(crate) fn status(state: &mut State, _call: &Call<'_>) -> Result<Reply, ApiError> {
    let store = &state.store;
    let ledger = store.ledger();
    let events = ledger.events();
    let loaded = u64::try_from(events.len()).map_err(ApiError::internal)?;
    let head = events.last();
    let failure = store.read_only();
    let entries = events
        .iter()
        .filter(|e| e.kind == EventKind::Posted)
        .count();
    // 読み取り専用のときは、壊れた連番の手前までしか読み込んでいない。DBの件数は起動時に数えたもの。
    let in_db = if failure.is_some() {
        state.events_in_db
    } else {
        loaded
    };
    let counts = store.draft_counts()?;
    ok(json!({
        "app": "kamoku",
        "version": env!("CARGO_PKG_VERSION"),
        "schema": SCHEMA_VERSION,
        "mode": state.mode.as_str(),
        "setup_done": store.settings().setup_done,
        "ledger": {
            "events": loaded,
            "events_loaded": loaded,
            "events_in_db": in_db,
            "entries": entries,
            "head_seq": head.map_or(0, |e| e.seq),
            "head_hash": ledger.head_hash().to_hex(),
            "last_recorded_at": head.map(|e| e.recorded_at),
            "verified": failure.is_none(),
            "error": failure.map(failure_json),
        },
        "inbox": { "draft": counts.draft, "ignored": counts.ignored },
    }))
}

/// `POST /verify`。壊れていても状態は200で、`ok` が偽になる。
pub(crate) fn verify(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let noted = f
        .opt_str("noted_hash")?
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    if let Some(hash) = &noted
        && Hash::from_hex(hash).is_err()
    {
        return Err(
            ApiError::bad_request("控えたハッシュは、16進の64桁で入れてください。")
                .with_field("noted_hash"),
        );
    }
    let store = &state.store;
    let noted_json = noted.map(|hash| {
        let found = store.noted_hash_lookup(&hash);
        json!({
            "found": found.is_some(),
            "seq": found.as_ref().map(|n| n.seq),
            "is_head": found.is_some_and(|n| n.is_head),
        })
    });
    ok(match store.verify() {
        Ok(v) => json!({
            "ok": true,
            "events": v.events,
            "head_hash": v.head_hash,
            "noted": noted_json,
        }),
        Err(failure) => json!({
            "ok": false,
            "error": failure_json(&failure),
            "noted": noted_json,
        }),
    })
}
