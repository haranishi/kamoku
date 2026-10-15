// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::model::Filters;
use wasm_bindgen::JsValue;
use web_sys::UrlSearchParams;

use super::dom;

pub(super) const PREVIEW_PATH: &str = "/rust/index.html";

pub(super) fn read() -> Result<Filters, JsValue> {
    let params = UrlSearchParams::new_with_str(&dom::window()?.location().search()?)?;
    Ok(Filters::read(|key| params.get(key)))
}

pub(super) fn params(filters: &Filters) -> Result<UrlSearchParams, JsValue> {
    let params = UrlSearchParams::new()?;
    for (name, value) in filters.pairs() {
        params.set(name, &value);
    }
    Ok(params)
}

pub(super) fn query(filters: &Filters) -> Result<String, JsValue> {
    let value = params(filters)?.to_string().as_string().unwrap_or_default();
    Ok(if value.is_empty() {
        value
    } else {
        format!("?{value}")
    })
}

pub(super) fn push(filters: &Filters) -> Result<(), JsValue> {
    let url = format!("{PREVIEW_PATH}{}", query(filters)?);
    dom::window()?
        .history()?
        .push_state_with_url(&JsValue::NULL, "", Some(&url))
}

pub(super) fn remember_entry(id: &str) -> Result<(), JsValue> {
    // The current history entry owns the focus target, so Back/reload restores it
    // without persisting accounting data in browser storage.
    dom::window()?
        .history()?
        .replace_state_with_url(&JsValue::from_str(id), "", None)
}

pub(super) fn remembered_entry() -> Option<String> {
    dom::window()
        .ok()?
        .history()
        .ok()?
        .state()
        .ok()?
        .as_string()
}

pub(super) fn old_detail(id: &str, filters: &Filters) -> Result<String, JsValue> {
    // IDs are escaped with URLSearchParams rather than interpolated into markup.
    let encoded = UrlSearchParams::new()?;
    encoded.set("id", id);
    let encoded = encoded.to_string().as_string().unwrap_or_default();
    Ok(format!(
        "/#/journal/{}{}",
        encoded.strip_prefix("id=").unwrap_or_default(),
        query(filters)?
    ))
}
