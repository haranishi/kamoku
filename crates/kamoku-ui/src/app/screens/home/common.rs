// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Semantic rendering helpers shared by the report and administration screens.

use crate::app::prelude::*;
use serde_json::Value;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlInputElement, KeyboardEvent};

pub(crate) fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
pub(crate) fn n(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(0)
}
pub(crate) fn yes(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}
pub(crate) fn list(v: &Value) -> Vec<Value> {
    v.as_array()
        .or_else(|| v.get("items").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default()
}
pub(crate) fn el<T: AsRef<Element>>(node: &T) -> Element {
    node.as_ref().clone()
}
pub(crate) fn box_node(tag: &str, class: &str, children: &[Element]) -> Element {
    let node = h(tag, class, "");
    append(&node, children);
    node
}
pub(crate) fn title(text: &str) -> Element {
    let node = h("h1", "", text);
    attrs(&node, &[("tabindex", "-1")]);
    node
}
pub(crate) fn heading(text: &str, action: Option<Element>) -> Element {
    let text = box_node("div", "page-head-text", &[title(text)]);
    let head = box_node("div", "page-head", &[text]);
    if let Some(action) = action {
        append(&head, &[box_node("div", "page-head-actions", &[action])]);
    }
    head
}
pub(crate) fn amount(value: i64) -> Element {
    box_node(
        "span",
        "money",
        &[h("span", "num", &number(value)), h("span", "unit", "円")],
    )
}
pub(crate) fn fact(label: &str, value: i64) -> Element {
    let dd = box_node("dd", "", &[amount(value)]);
    let _ = dd
        .unchecked_ref::<HtmlElement>()
        .style()
        .set_property("--chars", &number(value).len().to_string());
    box_node("div", "", &[h("dt", "", label), dd])
}
pub(crate) fn empty_value() -> Element {
    let dash = h("span", "", "—");
    attrs(&dash, &[("aria-hidden", "true")]);
    box_node(
        "span",
        "empty-value",
        &[dash, h("span", "visually-hidden", "なし")],
    )
}
pub(crate) fn banner_node(title: &str, body: &str, danger: bool) -> Element {
    let node = box_node(
        "div",
        if danger {
            "banner banner--danger"
        } else {
            "banner banner--info"
        },
        &[h("p", "banner-title", title), h("p", "banner-body", body)],
    );
    attrs(&node, &[("role", if danger { "alert" } else { "note" })]);
    node
}
pub(crate) fn checkbox(label: &str, checked: bool) -> (Element, HtmlInputElement) {
    let control = input("checkbox", "");
    control.set_checked(checked);
    (
        box_node("label", "check", &[el(&control), h("span", "", label)]),
        control,
    )
}
pub(crate) fn text_input(value: &str, label: &str) -> HtmlInputElement {
    let control = input("text", value);
    attrs(&el(&control), &[("class", "input"), ("aria-label", label)]);
    control
}
pub(crate) fn table(caption: &str, headers: &[&str], class: &str) -> (Element, Element) {
    let body = h("tbody", "", "");
    let row = h("tr", "", "");
    for header in headers {
        let cell = h("th", "", header);
        attrs(&cell, &[("scope", "col")]);
        append(&row, &[cell]);
    }
    let node = box_node(
        "table",
        &format!("table {class}"),
        &[
            h("caption", "", caption),
            box_node("thead", "", &[row]),
            body.clone(),
        ],
    );
    (box_node("div", "table-wrap", &[node]), body)
}
pub(crate) fn cell(row: &Element, label: &str, content: Element, class: &str) -> Element {
    let cell = box_node("td", class, &[content]);
    attrs(&cell, &[("data-label", label)]);
    append(row, std::slice::from_ref(&cell));
    cell
}
pub(crate) fn row_open(ctx: &Context, row: &Element, open: impl Fn() + 'static) {
    let open = std::rc::Rc::new(open);
    attrs(row, &[("tabindex", "0")]);
    let copy = open.clone();
    ctx.on(row, "click", move |_| copy());
    ctx.on(row, "keydown", move |event| {
        let Some(event) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if event.key() == "Enter" && !ime_ignore(event) {
            event.prevent_default();
            open();
        }
    });
}
pub(crate) fn text_field(
    label: &str,
    value: &str,
    hint: &str,
    required: bool,
) -> (HtmlInputElement, Field) {
    let control = text_input(value, label);
    let f = field(label, &el(&control), hint, required);
    if hint.contains('\n')
        && let Ok(Some(paragraph)) = f.el.query_selector(".field-hint")
    {
        replace(
            &paragraph,
            &hint
                .split('\n')
                .map(|line| h("span", "hint-line", line))
                .collect::<Vec<_>>(),
        );
    }
    (control, f)
}
pub(crate) fn set_hidden(node: &Element, hidden: bool) {
    if hidden {
        let _ = node.set_attribute("hidden", "");
    } else {
        let _ = node.remove_attribute("hidden");
    }
}
pub(crate) fn field_failure(error: &ApiError, fields: &[(&str, &Field)]) -> bool {
    if let Some((_, f)) = fields
        .iter()
        .find(|(name, _)| error.field.as_deref() == Some(*name))
    {
        f.set_error(&error.message);
        focus(&f.control);
        true
    } else {
        false
    }
}

#[derive(Clone)]
pub(crate) struct Segmented {
    pub node: Element,
    pub value: std::rc::Rc<std::cell::RefCell<String>>,
    pub radios: Vec<HtmlInputElement>,
}
impl Segmented {
    pub fn get(&self) -> String {
        self.value.borrow().clone()
    }
    pub fn set(&self, value: &str) {
        *self.value.borrow_mut() = value.into();
        for radio in &self.radios {
            radio.set_checked(radio.value() == value);
        }
    }
}
pub(crate) fn segmented(
    ctx: &Context,
    label: &str,
    options: &[(&str, &str)],
    value: &str,
    on_change: impl Fn(String) + 'static,
) -> Segmented {
    let group = h("fieldset", "seg seg--grid", "");
    append(&group, &[h("legend", "visually-hidden", label)]);
    let name = uid("seg");
    let state = std::rc::Rc::new(std::cell::RefCell::new(value.to_owned()));
    let callback = std::rc::Rc::new(on_change);
    let mut radios = Vec::new();
    for (key, label) in options {
        let radio = input("radio", key);
        radio.set_name(&name);
        radio.set_checked(*key == value);
        let option = box_node("label", "seg-option", &[el(&radio), h("span", "", label)]);
        append(&group, &[option]);
        let copy = radio.clone();
        let state = state.clone();
        let callback = callback.clone();
        ctx.on(&el(&radio), "change", move |_| {
            if copy.checked() {
                *state.borrow_mut() = copy.value();
                callback(copy.value());
            }
        });
        radios.push(radio);
    }
    Segmented {
        node: group,
        value: state,
        radios,
    }
}

pub(crate) fn date_text(value: &str) -> String {
    let Some((mut year, month, day)) = date_parts(value) else {
        return value.into();
    };
    let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if month < 3 {
        year -= 1;
    }
    let weekday = (year + year / 4 - year / 100
        + year / 400
        + offsets[usize::try_from(month - 1).unwrap_or_default()]
        + i32::try_from(day).unwrap_or_default())
    .rem_euclid(7);
    let weekday = usize::try_from(weekday).unwrap_or_default();
    format!(
        "{value} ({})",
        ["日", "月", "火", "水", "木", "金", "土"][weekday]
    )
}

#[derive(Clone)]
pub(crate) struct Failures {
    ctx: Context,
    messages: Rc<RefCell<BTreeSet<String>>>,
}
impl Failures {
    pub(crate) fn new(ctx: &Context) -> Self {
        let value = Self {
            ctx: ctx.clone(),
            messages: Rc::new(RefCell::new(BTreeSet::new())),
        };
        let cleanup = value.clone();
        ctx.scope.cleanup(move || cleanup.clear());
        value
    }
    pub(crate) fn show(&self, message: &str) {
        self.messages.borrow_mut().insert(message.into());
        self.ctx.error(message);
    }
    pub(crate) fn clear(&self) {
        for message in std::mem::take(&mut *self.messages.borrow_mut()) {
            self.ctx.dismiss_error(&message);
        }
    }
}

pub(crate) fn icon_button(label: &str, name: &str, class: &str) -> Element {
    let node = button("", "");
    node.set_class_name(&format!("icon-btn {class}"));
    attrs(&node, &[("aria-label", label), ("title", label)]);
    replace(&node, &[icon(name)]);
    node
}
