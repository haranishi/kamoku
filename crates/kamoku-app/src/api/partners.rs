// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 取引先（`/partners`）と、よく使う取引（`/templates`）。

use super::body::{Fields, object};
use super::input::{EntryInput, compose, compose_settings, parse_entry};
use super::{ApiError, Call, Reply, created, ok, to_value};
use crate::state::State;
use kamoku_books::Source;
use kamoku_store::{NewPartner, PartnerPatch};
use serde_json::{Value, json};

fn invoice(f: &Fields<'_>) -> Result<Option<String>, ApiError> {
    Ok(f.opt_str("invoice_number")?
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty()))
}

fn partner_name(f: &Fields<'_>, required: bool) -> Result<Option<String>, ApiError> {
    let value = if required {
        Some(f.req_str("name", "名前")?)
    } else {
        f.opt_str("name")?
    };
    match value.map(|v| v.trim().to_owned()) {
        Some(v) if v.is_empty() => {
            Err(ApiError::rule("名前を入れてください（空白だけは使えません）。").with_field("name"))
        }
        other => Ok(other),
    }
}

fn id_param(call: &Call<'_>, what: &str) -> Result<i64, ApiError> {
    call.param(0)
        .parse()
        .map_err(|_| ApiError::not_found(format!("{what}が見つかりません。")))
}

/// `GET /partners?q=&limit=&offset=`。`q` は名前と読みに効く。
pub(crate) fn partners_list(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let (limit, offset) = call.query.page()?;
    let needle = call.query.get("q");
    let all = state.store.partners()?;
    let matched: Vec<_> = all
        .iter()
        .filter(|p| needle.is_none_or(|q| p.name.contains(q) || p.reading.contains(q)))
        .collect();
    let items = matched
        .iter()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(to_value)
        .collect::<Result<Vec<Value>, ApiError>>()?;
    ok(json!({ "items": items, "total": matched.len() }))
}

/// `POST /partners`
pub(crate) fn partner_add(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let partner = state.store.add_partner(
        NewPartner {
            name: partner_name(&f, true)?.unwrap_or_default(),
            reading: f.text("reading")?,
            invoice_number: invoice(&f)?,
            note: f.text("note")?,
        },
        call.now,
    )?;
    created(to_value(&partner)?)
}

/// `PATCH /partners/{id}`。`invoice_number` を `null` か空にすると消す。
pub(crate) fn partner_patch(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let id = id_param(call, "取引先")?;
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let patch = PartnerPatch {
        name: partner_name(&f, false)?,
        reading: f.opt_str("reading")?,
        invoice_number: if f.has("invoice_number") {
            Some(invoice(&f)?)
        } else {
            None
        },
        note: f.opt_str("note")?,
        enabled: f.opt_bool("enabled")?,
    };
    let partner = state.store.update_partner(id, patch, call.now)?;
    ok(to_value(&partner)?)
}

/// `GET /templates`
pub(crate) fn templates_list(state: &mut State, _call: &Call<'_>) -> Result<Reply, ApiError> {
    let items = state.store.templates()?;
    ok(json!({ "total": items.len(), "items": to_value(&items)? }))
}

/// `POST /templates`。本文 `{ name, body }`。`body` は日付を除いた単純な形の入力。
/// 形を先に検査し、読み直した形（`date` を除く）で保存する。保存の層が読めない形を渡さないため。
pub(crate) fn template_add(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let map = object(call.body)?;
    let f = Fields::new(&map);
    let name = partner_name(&f, true)?.unwrap_or_default();
    let body = f
        .opt_object("body")?
        .ok_or_else(|| f.missing("body", "取引の入力"))?;
    if body.contains_key("date") {
        return Err(ApiError::rule("テンプレートには日付を入れません。").with_field("body.date"));
    }
    let mut with_date = body.clone();
    with_date.insert("date".to_owned(), json!("2000-01-01"));
    let EntryInput::Simple(input) = parse_entry(&with_date).map_err(|e| prefixed(e, "body."))?
    else {
        return Err(
            ApiError::rule("テンプレートにできるのは、支出・収入・振替の入力だけです。")
                .with_field("body.kind"),
        );
    };
    let settings = compose_settings(state.store.settings(), Source::Template);
    compose(
        &EntryInput::Simple(input.clone()),
        state.store.chart(),
        settings,
    )
    .map_err(|e| prefixed(e, "body."))?;
    let mut normalized = to_value(&input)?;
    if let Value::Object(fields) = &mut normalized {
        fields.remove("date");
    }
    let template = state.store.add_template(&name, normalized, call.now)?;
    created(to_value(&template)?)
}

fn prefixed(mut error: ApiError, prefix: &str) -> ApiError {
    error.field = error.field.map(|f| format!("{prefix}{f}"));
    error
}

/// `DELETE /templates/{id}`。応答は `{ "id", "deleted": true }`（本文の無い204は使わない）。
pub(crate) fn template_delete(state: &mut State, call: &Call<'_>) -> Result<Reply, ApiError> {
    let id = id_param(call, "テンプレート")?;
    state.store.delete_template(id, call.now)?;
    ok(json!({ "id": id, "deleted": true }))
}
