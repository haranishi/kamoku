// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, Window};

pub(super) fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| JsValue::from_str("window is unavailable"))
}

pub(super) fn document() -> Result<Document, JsValue> {
    window()?
        .document()
        .ok_or_else(|| JsValue::from_str("document is unavailable"))
}

pub(super) fn element(tag: &str, class: &str, text: &str) -> Result<Element, JsValue> {
    let el = document()?.create_element(tag)?;
    if !class.is_empty() {
        el.set_class_name(class);
    }
    if !text.is_empty() {
        el.set_text_content(Some(text));
    }
    Ok(el)
}

pub(super) fn append(parent: &Element, child: &Element) -> Result<(), JsValue> {
    parent.append_child(child)?;
    Ok(())
}

pub(super) fn link(label: &str, href: &str, class: &str) -> Result<Element, JsValue> {
    let el = element("a", class, label)?;
    el.set_attribute("href", href)?;
    Ok(el)
}

pub(super) fn button(label: &str, action: &str, class: &str) -> Result<Element, JsValue> {
    let el = element("button", class, label)?;
    el.set_attribute("type", "button")?;
    el.set_attribute("data-action", action)?;
    Ok(el)
}

pub(super) fn input(kind: &str, id: &str) -> Result<HtmlInputElement, JsValue> {
    let el = element("input", "", "")?;
    el.set_attribute("type", kind)?;
    el.set_attribute("id", id)?;
    Ok(el.dyn_into()?)
}

pub(super) fn focus(el: &Element) -> Result<(), JsValue> {
    if let Some(html) = el.dyn_ref::<HtmlElement>() {
        html.focus()?;
    }
    Ok(())
}

pub(super) fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |clock| clock.now())
}

pub(super) fn disabled(el: &Element, value: bool) -> Result<(), JsValue> {
    if value {
        el.set_attribute("disabled", "")
    } else {
        el.remove_attribute("disabled")
    }
}
