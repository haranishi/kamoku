// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    dom::{document, focus, now},
    lifecycle::Scope,
    theme,
};
use crate::interaction::{ImeEvent, ImeState, KeyInput};
use std::cell::{Cell, RefCell};
use wasm_bindgen::JsCast;
use web_sys::{Element, KeyboardEvent};
thread_local! {static IME:RefCell<ImeState>=const{RefCell::new(ImeState::new())};static G_UNTIL:Cell<f64>=const{Cell::new(0.0)};static SINGLE:Cell<Option<bool>>=const{Cell::new(None)};}
pub(crate) fn ime_ignore(e: &KeyboardEvent) -> bool {
    let key = e.key();
    let kind = IME.with(|state| {
        state.borrow().classify(
            Some(KeyInput {
                key: &key,
                key_code: e.key_code(),
                is_composing: e.is_composing(),
                repeat: e.repeat(),
            }),
            now(),
        )
    });
    if kind.prevents_default() {
        e.prevent_default();
    }
    kind.should_ignore()
}
pub(crate) fn single_enabled() -> bool {
    SINGLE.with(|v| {
        v.get()
            .unwrap_or_else(|| theme::stored("kamoku.singleKeys").as_deref() != Some("off"))
    })
}
pub(crate) fn set_single(on: bool) {
    SINGLE.with(|v| v.set(Some(on)));
    theme::store("kamoku.singleKeys", if on { "on" } else { "off" });
}
pub(crate) fn mod_label() -> &'static str {
    let platform = super::dom::window()
        .navigator()
        .platform()
        .unwrap_or_default();
    if ["Mac", "iPhone", "iPad"]
        .iter()
        .any(|name| platform.contains(name))
    {
        "⌘"
    } else {
        "Ctrl"
    }
}
pub(crate) fn typing(e: &Element) -> bool {
    matches!(e.tag_name().as_str(), "TEXTAREA" | "SELECT")
        || (e.tag_name() == "INPUT"
            && !matches!(
                e.get_attribute("type").as_deref(),
                Some(
                    "checkbox"
                        | "radio"
                        | "button"
                        | "submit"
                        | "reset"
                        | "range"
                        | "color"
                        | "file"
                        | "image"
                )
            ))
        || e.closest("[contenteditable=true]").ok().flatten().is_some()
}
#[allow(
    clippy::too_many_lines,
    reason = "Keyboard precedence is kept in one ordered dispatch function"
)]
pub(crate) fn start(scope: &Scope) {
    IME.with(|state| state.borrow_mut().install());
    scope.cleanup(|| IME.with(|state| state.borrow_mut().uninstall()));
    scope.listen(document().as_ref(), "compositionstart", true, |_| {
        IME.with(|state| state.borrow_mut().observe(ImeEvent::Start, now()));
    });
    scope.listen(document().as_ref(), "compositionend", true, |_| {
        IME.with(|state| state.borrow_mut().observe(ImeEvent::End, now()));
    });
    scope.listen(document().as_ref(), "focusout", true, |_| {
        IME.with(|state| state.borrow_mut().observe(ImeEvent::FocusOut, now()));
    });
    scope.listen(document().as_ref(), "keydown", false, move |event| {
        let Ok(e) = event.dyn_into::<KeyboardEvent>() else {
            return;
        };
        if ime_ignore(&e) || e.default_prevented() {
            return;
        }
        if (e.ctrl_key() || e.meta_key())
            && !e.alt_key()
            && !e.shift_key()
            && e.key().eq_ignore_ascii_case("k")
        {
            e.prevent_default();
            G_UNTIL.with(|v| v.set(0.0));
            super::super::shell::palette();
            return;
        }
        if document()
            .query_selector("dialog[open]")
            .ok()
            .flatten()
            .is_some()
        {
            return;
        }
        if super::super::current_scope().is_some_and(|s| s.dispatch(e.clone())) {
            e.prevent_default();
            return;
        }
        if e.ctrl_key() || e.meta_key() || e.alt_key() {
            return;
        }
        if e.target()
            .and_then(|v| v.dyn_into::<Element>().ok())
            .is_some_and(|el| typing(&el))
        {
            G_UNTIL.with(|v| v.set(0.0));
            return;
        }
        let key = e.key();
        if key.chars().count() == 1 && !single_enabled() {
            G_UNTIL.with(|v| v.set(0.0));
            return;
        }
        let g = G_UNTIL.with(|v| v.replace(0.0));
        if g > now() {
            let hash = match key.as_str() {
                "h" => Some("#/"),
                "n" => Some("#/entry"),
                "i" => Some("#/inbox"),
                "j" => Some("#/journal"),
                "r" => Some("#/reports"),
                "w" => Some("#/wallets"),
                "s" => Some("#/settings"),
                _ => None,
            };
            if let Some(hash) = hash {
                e.prevent_default();
                super::route::navigate(hash, false);
            }
            return;
        }
        if key == "g" {
            G_UNTIL.with(|v| v.set(now() + 1500.0));
            return;
        }
        match key.as_str() {
            "n" => {
                e.prevent_default();
                super::route::navigate("#/entry", false);
            }
            "?" => {
                e.prevent_default();
                super::super::shell::help();
            }
            "/" => {
                e.prevent_default();
                if let Some(el) = document()
                    .query_selector("main [data-search]")
                    .ok()
                    .flatten()
                {
                    focus(&el);
                    if let Some(el) = el.dyn_ref::<web_sys::HtmlInputElement>() {
                        el.select();
                    }
                } else {
                    super::super::shell::palette();
                }
            }
            _ => {}
        }
    });
}
