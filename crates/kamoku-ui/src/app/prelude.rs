// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Common screen vocabulary. Imports are intentionally available to all screens.
#[allow(unused_imports)]
pub(crate) use {
    super::components::{
        combo::{Combo, ComboItem},
        dialog::{Dialog, confirm},
        field::{Field, field},
    },
    super::core::{
        api::{ApiError, new_client_id},
        context::{Context, Screen},
        dom::*,
        keys::{ime_ignore, single_enabled as single_keys_enabled, typing as is_typing_target},
        lifecycle::Scope,
        route::{Route, build_hash},
    },
    crate::input::{
        Amount, MAX_AMOUNT, add_days, date_parts, days_in_month, normalize_search, normalize_width,
        number, parse_amount, parse_amount_details, parse_date, year_month,
    },
    serde_json::{Value, json},
    std::{
        cell::{Cell, RefCell},
        collections::{BTreeMap, BTreeSet},
        rc::Rc,
    },
    wasm_bindgen::{JsCast, JsValue},
    web_sys::{
        Element, Event, HtmlElement, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement,
        KeyboardEvent,
    },
};
pub(crate) fn web_demo() -> bool {
    meta_content("kamoku-edition").as_deref() == Some("web-demo")
}

/// お試し版のソースの場所（index.html の `kamoku-source`）。https の場所だけを使う。
pub(crate) fn web_demo_source() -> Option<String> {
    meta_content("kamoku-source").filter(|url| url.starts_with("https:"))
}

fn meta_content(name: &str) -> Option<String> {
    document()
        .query_selector(&format!("meta[name='{name}']"))
        .ok()
        .flatten()
        .and_then(|el| el.get_attribute("content"))
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

pub(crate) fn today() -> String {
    if let Some(fixed) = meta_content("kamoku-demo-today")
        && is_iso_date(&fixed)
    {
        return fixed;
    }
    let d = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date()
    )
}
