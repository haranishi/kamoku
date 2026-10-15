// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::dom::{attrs, document, window};
use std::cell::RefCell;
thread_local! {static THEME:RefCell<String>=RefCell::new("os".into());static DENSITY:RefCell<String>=RefCell::new("standard".into());}
pub(crate) fn stored(key: &str) -> Option<String> {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(key).ok().flatten())
}
pub(crate) fn store(key: &str, value: &str) {
    if let Ok(Some(s)) = window().local_storage() {
        let _ = s.set_item(key, value);
    }
}
pub(crate) fn get_theme() -> String {
    stored("kamoku.theme")
        .filter(|v| ["os", "light", "dark"].contains(&v.as_str()))
        .unwrap_or_else(|| THEME.with(|v| v.borrow().clone()))
}
pub(crate) fn get_density() -> String {
    stored("kamoku.density")
        .filter(|v| ["standard", "compact"].contains(&v.as_str()))
        .unwrap_or_else(|| DENSITY.with(|v| v.borrow().clone()))
}
pub(crate) fn apply() {
    if let Some(root) = document().document_element() {
        let t = get_theme();
        if t == "os" {
            let _ = root.remove_attribute("data-theme");
        } else {
            attrs(&root, &[("data-theme", &t)]);
        }
        if get_density() == "compact" {
            attrs(&root, &[("data-density", "compact")]);
        } else {
            let _ = root.remove_attribute("data-density");
        }
    }
}
pub(crate) fn set_theme(t: &str) {
    let t = if ["os", "light", "dark"].contains(&t) {
        t
    } else {
        "os"
    };
    THEME.with(|v| *v.borrow_mut() = t.into());
    store("kamoku.theme", t);
    apply();
    super::super::shell::sync_theme();
    changed();
}
pub(crate) fn set_density(d: &str) {
    let d = if d == "compact" { d } else { "standard" };
    DENSITY.with(|v| *v.borrow_mut() = d.into());
    store("kamoku.density", d);
    apply();
    changed();
}
pub(crate) fn label() -> &'static str {
    match get_theme().as_str() {
        "light" => "ライト",
        "dark" => "ダーク",
        _ => "OSに合わせる",
    }
}

fn changed() {
    if let Ok(event) = web_sys::Event::new("kamoku-appearance") {
        let _ = window().dispatch_event(&event);
    }
}
