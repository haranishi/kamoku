// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 未確認の一覧と、直し方・確定（`/drafts`）。

use super::body::{Fields, object};
use super::entries::{fresh_id, with_invoice};
use super::input::compose_settings;
use super::{ApiError, Call, Reply, ok};
use crate::state::State;
use kamoku_books::{
    Chart, ComposeSettings, Composed, SimpleInput, SimpleKind, Source, TaxCode, compose_simple,
    format_date, parse_date,
};
use kamoku_ledger::{Entry, EntryId, Line, Side, Yen};
use kamoku_store::{Draft, DraftAction, DraftFilter, DraftId, DraftPatch, DraftStatus, Store};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn draft_json(d: &Draft, chart: &Chart) -> Value {
    json!({
        "id": d.id,
        "batch_id": d.batch_id,
        "wallet": d.wallet,
        "wallet_name": chart.name(&d.wallet).unwrap_or(&d.wallet),
        "date": d.date,
        "description": d.description,
        "direction": d.direction,
        "amount": d.amount,
        "account": d.account,
        "tax_code": d.tax_code,
        "partner": d.partner,
        "memo": d.memo,
        "business_pct": d.business_pct,
        "status": d.status,
        "entry_id": d.entry_id,
    })
}

/// `GET /drafts?status=draft|ignored|confirmed&wallet=&q=&from=&to=&limit=&offset=`（`status` の既定は `draft`）。
pub(crate) fn drafts_list(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let q = call.query;
    let status = match q.get("status") {
        None | Some("draft") => Some(DraftStatus::Draft),
        Some("ignored") => Some(DraftStatus::Ignored),
        Some("confirmed") => Some(DraftStatus::Confirmed),
        Some("all") => None,
        Some(_) => {
            return Err(ApiError::bad_request(
                "status は draft・ignored・confirmed・all のどれかにしてください。",
            )
            .with_field("status"));
        }
    };
    let (limit, offset) = q.page()?;
    let filter = DraftFilter {
        status,
        wallet: q.get("wallet").map(str::to_owned),
        q: q.get("q").map(str::to_owned),
        from: q.date("from")?.map(format_date),
        to: q.date("to")?.map(format_date),
        limit,
        offset,
    };
    let page = state.store.drafts(filter)?;
    let chart = state.store.chart();
    let items: Vec<Value> = page.items.iter().map(|d| draft_json(d, chart)).collect();
    ok(json!({ "items": items, "total": page.total, "counts": state.store.draft_counts()? }))
}

fn draft_id(call: &Call<'_>) -> Result<DraftId, ApiError> {
    call.param(0)
        .parse()
        .map_err(|_| ApiError::not_found("未確認の行が見つかりません。"))
}

/// `PATCH /drafts/{id}`。本文は `date` `account` `tax_code` `partner` `memo` `business_pct` のうち変えるもの。
pub(crate) fn draft_patch(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let id = draft_id(call)?;
    let map = object(call.body)?;
    let f = Fields::new(&map);
    if f.has("date") && f.value("date").is_none() {
        return Err(
            ApiError::bad_request("日付は消せません。YYYY-MM-DD で入れてください。")
                .with_field("date"),
        );
    }
    let tax_code = if f.has("tax_code") {
        let code = f.opt_str("tax_code")?.filter(|t| !t.is_empty());
        if let Some(text) = &code
            && TaxCode::parse(text).is_none()
        {
            return Err(ApiError::new(
                422,
                "bad_tax_code",
                format!("税区分「{text}」は使えません。S10・S8・P10・P8・P10X・P8X・NT のどれかを選ぶか、空にしてください。"),
            )
            .with_field("tax_code"));
        }
        Some(code)
    } else {
        None
    };
    let patch = DraftPatch {
        date: f.opt_date("date")?.map(format_date),
        account: if f.has("account") {
            Some(f.opt_str("account")?.filter(|a| !a.is_empty()))
        } else {
            None
        },
        tax_code,
        partner: f.opt_str("partner")?.map(|p| p.trim().to_owned()),
        memo: f.opt_str("memo")?,
        business_pct: f.opt_pct("business_pct")?,
    };
    let draft = state.store.update_draft(id, patch, call.now)?;
    ok(draft_json(&draft, state.store.chart()))
}

fn set_status(state: &mut State, call: &Call<'_>, action: DraftAction) -> Result<Reply, ApiError> {
    let id = draft_id(call)?;
    state.store.set_draft_status(id, action, call.now)?;
    let status = match action {
        DraftAction::Ignore => "ignored",
        DraftAction::Restore => "draft",
    };
    ok(json!({ "id": id, "status": status, "entry_id": null }))
}

/// `POST /drafts/{id}/ignore`（取り込まない）。確定済みの行は `409 already_confirmed`。
pub(crate) fn draft_ignore(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    set_status(state, call, DraftAction::Ignore)
}

/// `POST /drafts/{id}/restore`（未確認に戻す）。確定済みの行は、結びついた仕訳が取消済みのときだけ。
pub(crate) fn draft_restore(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    set_status(state, call, DraftAction::Restore)
}

// 未確認の行から入力を作る規則は、api-inbox.md のとおり（出金は支出、入金は収入）。
fn compose_draft(
    d: &Draft,
    chart: &Chart,
    settings: ComposeSettings,
) -> Result<Composed, ApiError> {
    let input = SimpleInput {
        kind: if d.direction == "out" {
            SimpleKind::Expense
        } else {
            SimpleKind::Income
        },
        date: parse_date(&d.date).ok_or_else(|| ApiError::internal("下書きの日付を読めません"))?,
        amount: d.amount,
        account: d.account.clone(),
        wallet: d.wallet.clone(),
        to_wallet: None,
        tax_code: d.tax_code.clone(),
        partner: d.partner.clone(),
        description: d.description.clone(),
        business_pct: Some(d.business_pct),
        memo: d.memo.clone(),
    };
    compose_simple(&input, chart, &settings).map_err(ApiError::from)
}

// 未確認でない行は、保存の層に理由（確定済み・無視した行・見つからない）を返させる。
// そのための仮の仕訳。保存の層は状態の検査で断るので、台帳には入らない。
fn placeholder(id: EntryId) -> Result<Entry, ApiError> {
    let cash = kamoku_ledger::AccountCode::new("cash").map_err(ApiError::internal)?;
    let date = parse_date("2000-01-01").ok_or_else(|| ApiError::internal("日付"))?;
    Entry::new(
        id,
        date,
        "",
        vec![
            Line::new(cash.clone(), Side::Debit, Yen::new(1)),
            Line::new(cash, Side::Credit, Yen::new(1)),
        ],
    )
    .map_err(ApiError::internal)
}

fn waiting_drafts(
    store: &Store,
    wanted: &HashSet<DraftId>,
) -> Result<HashMap<DraftId, Draft>, ApiError> {
    let mut found = HashMap::new();
    let mut offset = 0u64;
    loop {
        let page = store.drafts(DraftFilter {
            status: Some(DraftStatus::Draft),
            limit: 500,
            offset,
            ..DraftFilter::default()
        })?;
        let count = u64::try_from(page.items.len()).map_err(ApiError::internal)?;
        found.extend(
            page.items
                .into_iter()
                .filter(|d| wanted.contains(&d.id))
                .map(|d| (d.id, d)),
        );
        offset = offset.saturating_add(count);
        if count == 0 || offset >= page.total {
            return Ok(found);
        }
    }
}

fn error_json(e: &ApiError) -> Value {
    let mut body = e.body();
    body.get_mut("error").map_or(Value::Null, Value::take)
}

/// 確定の一度の上限（api-inbox.md）。送った要素の数で数え、超えたら何も始める前に断る。
/// 処理の間は全体のロックを握るので、1つの要求の仕事の量に上限を置く。
const CONFIRM_MAX: usize = 2_000;

/// `POST /drafts/confirm`。本文 `{ "ids": [31, 32] }`。検査は1行ずつ行い、通った行の仕訳・下書きの状態・
/// 取引先・変更履歴を1つのトランザクションで書く。書き込みの失敗は全体の失敗（行ごとの失敗とは別）。
pub(crate) fn draft_confirm(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let given = f
        .opt_array("ids")?
        .ok_or_else(|| f.missing("ids", "確定する行"))?;
    if given.len() > CONFIRM_MAX {
        return Err(ApiError::rule(
            "一度に確定できるのは2,000件までです。分けて確定してください。",
        )
        .with_field("ids"));
    }
    // 重なった番号は最初の1つだけ使う。並びは送った順（失敗の並びもこの順）。
    let mut ids: Vec<DraftId> = Vec::with_capacity(given.len());
    let mut wanted: HashSet<DraftId> = HashSet::with_capacity(given.len());
    for (i, v) in given.iter().enumerate() {
        let id = v.as_i64().ok_or_else(|| {
            ApiError::bad_request("ids は、未確認の行の番号（整数）の配列にしてください。")
                .with_field(format!("ids[{i}]"))
        })?;
        if wanted.insert(id) {
            ids.push(id);
        }
    }
    let position: HashMap<DraftId, usize> =
        ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let waiting = waiting_drafts(&state.store, &wanted)?;
    let settings = compose_settings(state.store.settings(), Source::Import);
    let partners = state.store.partners()?;
    let invoices: HashMap<&str, &str> = partners
        .iter()
        .filter_map(|p| Some((p.name.as_str(), p.invoice_number.as_deref()?)))
        .collect();
    let mut taken = HashSet::new();
    let mut inputs = Vec::new();
    let mut failed: Vec<(usize, Value)> = Vec::new();
    for (order, id) in ids.iter().enumerate() {
        let entry_id = fresh_id(&state.store, &mut taken)?;
        let Some(draft) = waiting.get(id) else {
            inputs.push((*id, placeholder(entry_id)?));
            continue;
        };
        match compose_draft(draft, state.store.chart(), settings) {
            Err(e) => failed.push((order, json!({ "id": id, "error": error_json(&e) }))),
            Ok(composed) => {
                let invoice = invoices.get(draft.partner.as_str()).copied();
                let entry = composed.to_entry(entry_id).map_err(ApiError::internal)?;
                inputs.push((*id, with_invoice(entry, invoice)?));
            }
        }
    }
    let results = state.store.confirm_drafts(inputs, call.now)?;
    let mut confirmed = Vec::new();
    for result in results {
        let order = position.get(&result.id).copied().unwrap_or(usize::MAX);
        match result.result {
            Ok(posted) => confirmed
                .push(json!({ "id": result.id, "entry_id": posted.entry_id, "no": posted.no })),
            Err(e) => failed.push((
                order,
                json!({ "id": result.id, "error": error_json(&ApiError::from(e)) }),
            )),
        }
    }
    failed.sort_by_key(|(order, _)| *order);
    let failed: Vec<Value> = failed.into_iter().map(|(_, v)| v).collect();
    ok(json!({ "confirmed": confirmed, "failed": failed }))
}
