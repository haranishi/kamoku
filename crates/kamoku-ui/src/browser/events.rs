// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Element, Event, EventTarget, HtmlElement, KeyboardEvent};

use super::{App, apply, dom, load, navigation, restore, show_failure, view::View};
use crate::model::{Filters, KeyDecision, key_decision};

pub(super) fn attach(app: &App) -> Result<(), JsValue> {
    let document = dom::document()?;
    let window = dom::window()?;
    let copy = Rc::clone(app);
    listen(document.as_ref(), "keydown", move |event| {
        if let Ok(key) = event.dyn_into::<KeyboardEvent>() {
            show_failure(&copy, &keydown(&copy, &key));
        }
    })?;
    let copy = Rc::clone(app);
    listen(document.as_ref(), "compositionend", move |_| {
        copy.borrow_mut().composition_end = dom::now();
    })?;
    let copy = Rc::clone(app);
    listen(document.as_ref(), "click", move |event| {
        show_failure(&copy, &click(&copy, &event));
    })?;
    let copy = Rc::clone(app);
    listen(document.as_ref(), "change", move |event| {
        if target(&event).is_some_and(|el| matches!(el.id().as_str(), "from" | "to" | "state")) {
            show_failure(&copy, &apply_controls(&copy, false));
        }
    })?;
    // Browser implicit form submission must never turn composition Enter into a
    // request. Explicit button clicks and guarded key handling are the only paths.
    listen(document.as_ref(), "submit", move |event| {
        event.prevent_default();
    })?;
    let copy = Rc::clone(app);
    listen(window.as_ref(), "popstate", move |_| {
        show_failure(&copy, &restore(&copy));
    })?;
    let copy = Rc::clone(app);
    listen(window.as_ref(), "pageshow", move |_| {
        show_failure(&copy, &restore(&copy));
    })?;
    Ok(())
}

fn listen(
    target: &EventTarget,
    name: &str,
    handler: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let closure = Closure::wrap(Box::new(handler) as Box<dyn FnMut(Event)>);
    target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())?;
    // There are seven page-lifetime delegated listeners, never one per row or
    // render. The browser releases the complete page/WASM instance on navigation.
    closure.forget();
    Ok(())
}

fn target(event: &Event) -> Option<Element> {
    event.target()?.dyn_into().ok()
}

fn controls(app: &App) -> Filters {
    let state = app.borrow();
    Filters::read(|key| match key {
        "q" => Some(state.view.search.value()),
        "from" => Some(state.view.from.value()),
        "to" => Some(state.view.to.value()),
        "state" => Some(state.view.state.value()),
        _ => None,
    })
}

fn apply_controls(app: &App, first: bool) -> Result<(), JsValue> {
    apply(app, controls(app), first)
}

fn click(app: &App, event: &Event) -> Result<(), JsValue> {
    let Some(target) = target(event) else {
        return Ok(());
    };
    if let Some(link) = target.closest("a[data-entry-id]")? {
        if let Some(id) = link.get_attribute("data-entry-id") {
            navigation::remember_entry(&id)?;
        }
        return Ok(());
    }
    let Some(button) = target.closest("button[data-action]")? else {
        return Ok(());
    };
    if button.has_attribute("disabled") {
        return Ok(());
    }
    match button.get_attribute("data-action").as_deref() {
        Some("search") => apply_controls(app, false)?,
        Some("clear") => apply(app, Filters::read(|_| None), false)?,
        Some("retry") => load(app),
        Some("previous" | "next") => {
            let mut filters = app.borrow().filters.clone();
            filters.page = if button.get_attribute("data-action").as_deref() == Some("previous") {
                filters.page.saturating_sub(1).max(1)
            } else {
                filters.page.saturating_add(1)
            };
            apply(app, filters, true)?;
        }
        _ => {}
    }
    Ok(())
}

fn keydown(app: &App, event: &KeyboardEvent) -> Result<(), JsValue> {
    let key = event.key();
    let state = app.borrow();
    let decision = key_decision(
        event.is_composing(),
        event.key_code(),
        key == "Enter",
        event.repeat(),
        dom::now() - state.composition_end,
    );
    drop(state);
    match decision {
        KeyDecision::Ignore => return Ok(()),
        KeyDecision::Block => {
            event.prevent_default();
            return Ok(());
        }
        KeyDecision::Allow => {}
    }
    if event.alt_key() || event.ctrl_key() || event.meta_key() {
        return Ok(());
    }
    let Some(target) = event.target().and_then(|el| el.dyn_into::<Element>().ok()) else {
        return Ok(());
    };
    if target.id() == "search" {
        match key.as_str() {
            "Enter" => {
                event.prevent_default();
                apply_controls(app, false)?;
            }
            "ArrowDown" => {
                event.prevent_default();
                apply_controls(app, true)?;
            }
            "Escape" => {
                event.prevent_default();
                let state = app.borrow();
                state.view.search.set_value(&state.filters.q);
                state.view.search.blur()?;
            }
            _ => {}
        }
        return Ok(());
    }
    if target
        .closest("input, select, textarea, [contenteditable]")?
        .is_some()
    {
        return Ok(());
    }
    if !app.borrow().view.shortcuts.checked() {
        return Ok(());
    }
    match key.as_str() {
        "/" => {
            event.prevent_default();
            app.borrow().view.search.focus()?;
        }
        "j" | "k" => {
            event.prevent_default();
            move_focus(&app.borrow().view, key == "j")?;
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn focus_entry(view: &View, id: &str) -> Result<(), JsValue> {
    let links = view.tbody.query_selector_all("a[data-entry-id]")?;
    for index in 0..links.length() {
        let Some(link) = links
            .item(index)
            .and_then(|node| node.dyn_into::<Element>().ok())
        else {
            continue;
        };
        if id.is_empty() || link.get_attribute("data-entry-id").as_deref() == Some(id) {
            return dom::focus(&link);
        }
    }
    Ok(())
}

fn move_focus(view: &View, forward: bool) -> Result<(), JsValue> {
    let links = view.tbody.query_selector_all("a[data-entry-id]")?;
    if links.length() == 0 {
        return Ok(());
    }
    let active = dom::document()?.active_element();
    let mut at = None;
    for index in 0..links.length() {
        if links.item(index).as_ref() == active.as_ref().map(AsRef::as_ref) {
            at = Some(index);
            break;
        }
    }
    let next = at.map_or(0, |index| {
        if forward {
            (index + 1).min(links.length() - 1)
        } else {
            index.saturating_sub(1)
        }
    });
    if let Some(el) = links
        .item(next)
        .and_then(|node| node.dyn_into::<HtmlElement>().ok())
    {
        el.focus()?;
    }
    Ok(())
}
