// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::core::dom::{self, append, attrs, h, set_text, uid};
use std::{cell::RefCell, rc::Rc};
use web_sys::{Element, Text};

#[derive(Clone)]
pub(crate) struct Field {
    pub el: Element,
    pub control: Element,
    target: Element,
    error: Element,
    label: Element,
    error_text: Rc<RefCell<Option<Text>>>,
}
pub(crate) fn field(label: &str, control: &Element, hint: &str, required: bool) -> Field {
    let target = if control.matches("input,select,textarea").unwrap_or(false) {
        control.clone()
    } else {
        control
            .query_selector("input,select,textarea")
            .ok()
            .flatten()
            .unwrap_or_else(|| control.clone())
    };
    let id = if target.id().is_empty() {
        uid("f")
    } else {
        target.id()
    };
    target.set_id(&id);
    let label_el = h("label", "field-label", "");
    attrs(&label_el, &[("for", &id)]);
    let label_text = h("span", "", label);
    append(&label_el, std::slice::from_ref(&label_text));
    if required {
        attrs(&target, &[("aria-required", "true")]);
        let mark = h("span", "req", "必須");
        attrs(&mark, &[("aria-hidden", "true")]);
        append(&label_el, &[mark]);
    }
    let hint_el = h("p", "field-hint", hint);
    hint_el.set_id(&format!("{id}-hint"));
    if !hint.is_empty() {
        attrs(&target, &[("aria-describedby", &hint_el.id())]);
    }
    let error = h("p", "field-error", "");
    error.set_id(&format!("{id}-err"));
    attrs(&error, &[("aria-live", "polite")]);
    let el = h("div", "field", "");
    append(
        &el,
        &[label_el, control.clone(), hint_el.clone(), error.clone()],
    );
    Field {
        el,
        control: control.clone(),
        target,
        error,
        label: label_text,
        error_text: Rc::new(RefCell::new(None)),
    }
}
impl Field {
    pub(crate) fn set_error(&self, message: &str) {
        let error_id = self.error.id();
        let old = self
            .target
            .get_attribute("aria-describedby")
            .unwrap_or_default();
        let mut ids: Vec<_> = old
            .split_whitespace()
            .filter(|v| *v != error_id)
            .map(str::to_owned)
            .collect();
        if message.is_empty() {
            let _ = self.target.remove_attribute("aria-invalid");
        } else {
            attrs(&self.target, &[("aria-invalid", "true")]);
            ids.push(error_id);
        }
        if ids.is_empty() {
            let _ = self.target.remove_attribute("aria-describedby");
        } else {
            attrs(&self.target, &[("aria-describedby", &ids.join(" "))]);
        }
        let previous = self.error_text.borrow().clone();
        if let Some(text) = previous {
            if text.data() != message {
                text.set_data(message);
            }
        } else if !message.is_empty() {
            let text = dom::document().create_text_node(message);
            let span = h("span", "field-error-text", "");
            span.append_child(&text).expect("field error text");
            append(&self.error, &[dom::icon("alert"), span]);
            *self.error_text.borrow_mut() = Some(text);
        }
    }
    pub(crate) fn set_label(&self, text: &str) {
        set_text(&self.label, text);
    }
}
