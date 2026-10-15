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
pub(crate) fn today() -> String {
    let d = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date()
    )
}
