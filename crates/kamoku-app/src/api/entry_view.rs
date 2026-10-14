// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 仕訳の表現（`docs/spec/api-entries.md` の「仕訳の表現」）と、履歴。

use super::ApiError;
use kamoku_books::{Chart, LineView, format_date, reconstruct_simple};
use kamoku_ledger::{Entry, EntryId, Event, EventKind};
use kamoku_store::Store;
use serde_json::{Value, json};
use std::collections::HashMap;

/// 1つの仕訳のいまの姿。
pub(crate) struct Current<'a> {
    /// 仕訳No（登録の順）。
    pub no: u64,
    /// 最新のイベント（取消なら取消のイベント）。
    pub latest: &'a Event,
    /// 最後に有効だった版の仕訳。
    pub entry: &'a Entry,
}
impl Current<'_> {
    pub(crate) fn voided(&self) -> bool {
        self.latest.kind == EventKind::Voided
    }
    pub(crate) fn meta(&self, key: &str) -> &str {
        self.entry.meta().get(key).map_or("", String::as_str)
    }
}

/// すべての仕訳のいまの姿を、登録の順で返します。イベントを1回だけ通して作る。
pub(crate) fn all_current(store: &Store) -> Vec<Current<'_>> {
    let events = store.ledger().events();
    let mut order: Vec<&EntryId> = Vec::new();
    let mut slots: HashMap<&EntryId, (usize, Option<usize>)> = HashMap::new();
    for (i, event) in events.iter().enumerate() {
        let slot = slots.entry(&event.entry_id).or_insert_with(|| {
            order.push(&event.entry_id);
            (i, None)
        });
        slot.0 = i;
        if event.entry.is_some() {
            slot.1 = Some(i);
        }
    }
    order
        .into_iter()
        .zip(1u64..)
        .filter_map(|(id, no)| {
            let (latest, last_entry) = slots.get(id)?;
            Some(Current {
                no,
                latest: events.get(*latest)?,
                entry: events.get((*last_entry)?)?.entry.as_ref()?,
            })
        })
        .collect()
}

/// 1つの仕訳のいまの姿。
pub(crate) fn current<'a>(store: &'a Store, id: &EntryId) -> Option<Current<'a>> {
    let history = store.ledger().history(id);
    let latest = *history.last()?;
    let entry = history.iter().rev().find_map(|e| e.entry.as_ref())?;
    Some(Current {
        no: store.entry_no(id)?,
        latest,
        entry,
    })
}

fn kind_name(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Posted => "posted",
        EventKind::Revised => "revised",
        _ => "voided",
    }
}

pub(crate) fn lines_json(entry: &Entry, chart: &Chart) -> Value {
    let views: Vec<LineView> = entry
        .lines()
        .iter()
        .map(|l| LineView::new(l, chart))
        .collect();
    serde_json::to_value(views).unwrap_or(Value::Null)
}

/// 一覧に出す形。
pub(crate) fn entry_json(c: &Current<'_>, chart: &Chart) -> Value {
    json!({
        "id": c.latest.entry_id.as_str(),
        "no": c.no,
        "date": format_date(c.entry.date()),
        "kind": c.meta("kind"),
        "description": c.entry.description(),
        "partner": c.meta("partner"),
        "total": c.entry.total().get(),
        "revision": c.latest.revision,
        "state": if c.voided() { "voided" } else { "active" },
        "recorded_at": c.latest.recorded_at,
        "last_reason": c.latest.reason,
        "lines": lines_json(c.entry, chart),
        "meta": c.entry.meta(),
    })
}

/// 詳細・作る・直す応答の形（`input` つき）。`history` なら履歴も付ける。
pub(crate) fn detail_json(store: &Store, id: &EntryId, history: bool) -> Result<Value, ApiError> {
    let chart = store.chart();
    let c = current(store, id).ok_or_else(|| ApiError::not_found("仕訳が見つかりません。"))?;
    let mut out = entry_json(&c, chart);
    let input = reconstruct_simple(c.entry, chart)
        .map(|i| serde_json::to_value(i).map_err(ApiError::internal))
        .transpose()?;
    if let Value::Object(map) = &mut out {
        map.insert("input".to_owned(), input.unwrap_or(Value::Null));
        if history {
            let items: Vec<Value> = store
                .ledger()
                .history(id)
                .iter()
                .map(|e| {
                    json!({
                        "seq": e.seq,
                        "kind": kind_name(e.kind),
                        "revision": e.revision,
                        "recorded_at": e.recorded_at,
                        "reason": e.reason,
                        "hash": e.hash.to_hex(),
                        "lines": e.entry.as_ref().map(|en| lines_json(en, chart)),
                    })
                })
                .collect();
            map.insert("history".to_owned(), Value::Array(items));
        }
    }
    Ok(out)
}

/// パスの仕訳ID。形が違うものは「見つからない」にする。
pub(crate) fn entry_id(text: &str) -> Result<EntryId, ApiError> {
    EntryId::new(text).map_err(|_| ApiError::not_found("仕訳が見つかりません。"))
}
