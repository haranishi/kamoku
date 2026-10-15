// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::Cell;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, HtmlSelectElement, Window};

thread_local! { static NEXT:Cell<u64>=const {Cell::new(0)}; }
pub(crate) fn uid(prefix: &str) -> String {
    NEXT.with(|v| {
        v.set(v.get() + 1);
        format!("{prefix}-{}", v.get())
    })
}
pub(crate) fn window() -> Window {
    web_sys::window().expect("browser window")
}
pub(crate) fn document() -> Document {
    window().document().expect("browser document")
}
pub(crate) fn h(tag: &str, class: &str, text: &str) -> Element {
    let el = document().create_element(tag).expect("static HTML element");
    if !class.is_empty() {
        el.set_class_name(class);
    }
    if !text.is_empty() {
        el.set_text_content(Some(text));
    }
    el
}
pub(crate) fn attrs(el: &Element, values: &[(&str, &str)]) {
    for (key, value) in values {
        el.set_attribute(key, value).expect("static HTML attribute");
    }
}
pub(crate) fn append(el: &Element, children: &[Element]) {
    for child in children {
        el.append_child(child).expect("DOM append");
    }
}
pub(crate) fn replace(el: &Element, children: &[Element]) {
    el.set_text_content(None);
    append(el, children);
}
pub(crate) fn set_text(el: &Element, text: &str) {
    el.set_text_content(Some(text));
}
pub(crate) fn focus(el: &Element) {
    if let Some(el) = el.dyn_ref::<HtmlElement>() {
        let _ = el.focus();
    }
}
pub(crate) fn disabled(el: &Element, value: bool) {
    if value {
        attrs(el, &[("disabled", "")]);
    } else {
        let _ = el.remove_attribute("disabled");
    }
}
pub(crate) fn busy(el: &Element, value: bool) {
    attrs(el, &[("aria-busy", if value { "true" } else { "false" })]);
}
pub(crate) fn button(label: &str, variant: &str) -> Element {
    let class = if variant.is_empty() || variant == "secondary" {
        "btn".into()
    } else {
        format!("btn btn--{variant}")
    };
    let b = h("button", &class, "");
    attrs(&b, &[("type", "button")]);
    append(&b, &[h("span", "", label)]);
    b
}
pub(crate) fn link(label: &str, href: &str, class: &str) -> Element {
    let a = h("a", class, label);
    attrs(&a, &[("href", href)]);
    a
}
pub(crate) fn input(kind: &str, value: &str) -> HtmlInputElement {
    let e = h("input", "input", "");
    attrs(
        &e,
        &[
            ("type", kind),
            ("autocomplete", "off"),
            ("spellcheck", "false"),
        ],
    );
    let e: HtmlInputElement = e.unchecked_into();
    e.set_value(value);
    e
}
/// Select the existing value once on entry, preserving later caret placement.
pub(crate) fn select_all_on_focus(ctx: &super::context::Context, control: &HtmlInputElement) {
    let pending = std::rc::Rc::new(Cell::new(false));
    let flag = pending.clone();
    let input = control.clone();
    ctx.on(control.as_ref(), "focus", move |_| {
        input.select();
        flag.set(true);
    });
    let flag = pending.clone();
    let input = control.clone();
    ctx.on(control.as_ref(), "mouseup", move |event| {
        if flag.replace(false) {
            event.prevent_default();
            input.select();
        }
    });
    ctx.on(control.as_ref(), "blur", move |_| pending.set(false));
}

pub(crate) fn select(options: &[(&str, &str)], value: &str) -> HtmlSelectElement {
    let e = h("select", "select", "");
    for (v, label) in options {
        let o = h("option", "", label);
        attrs(&o, &[("value", v)]);
        append(&e, &[o]);
    }
    let e: HtmlSelectElement = e.unchecked_into();
    e.set_value(value);
    e
}
pub(crate) fn icon(name: &str) -> Element {
    let svg = document()
        .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
        .expect("SVG");
    attrs(
        &svg,
        &[
            ("class", "icon"),
            ("viewBox", "0 0 20 20"),
            ("width", "20"),
            ("height", "20"),
            ("fill", "none"),
            ("stroke", "currentColor"),
            ("stroke-width", "1.75"),
            ("stroke-linecap", "round"),
            ("stroke-linejoin", "round"),
            ("aria-hidden", "true"),
            ("focusable", "false"),
        ],
    );
    for data in super::icons::paths(if name == "x" { "close" } else { name }) {
        let path = document()
            .create_element_ns(Some("http://www.w3.org/2000/svg"), "path")
            .expect("SVG path");
        attrs(&path, &[("d", data)]);
        append(&svg, &[path]);
    }
    svg
}
pub(crate) fn now() -> f64 {
    window().performance().map_or(0.0, |p| p.now())
}

pub(crate) async fn delay(milliseconds: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        if let Err(error) =
            window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, milliseconds)
        {
            let _ = reject.call1(&wasm_bindgen::JsValue::NULL, &error);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Give a table wrapper a tab stop only while its contents overflow.
pub(crate) fn focus_when_scrollable(
    ctx: &super::context::Context,
    wrapper: &Element,
    inner: &Element,
) {
    use wasm_bindgen::closure::Closure;
    use web_sys::ResizeObserver;

    let target = wrapper.clone();
    let callback = Closure::wrap(Box::new(move |_: js_sys::Array, observer: ResizeObserver| {
        if !target.is_connected() {
            observer.disconnect();
        } else if target.scroll_width() > target.client_width() + 1 {
            attrs(&target, &[("tabindex", "0")]);
        } else {
            let _ = target.remove_attribute("tabindex");
        }
    }) as Box<dyn FnMut(js_sys::Array, ResizeObserver)>);
    let Ok(observer) = ResizeObserver::new(callback.as_ref().unchecked_ref()) else {
        attrs(wrapper, &[("tabindex", "0")]);
        return;
    };
    observer.observe(wrapper);
    observer.observe(inner);
    ctx.scope.cleanup(move || {
        observer.disconnect();
        drop(callback);
    });
}

pub(crate) fn phrase(text: &str) -> Element {
    let root = h("span", "phrase", "");
    for (index, chunk) in crate::phrase::chunks(text).iter().enumerate() {
        if index > 0 {
            append(&root, &[h("wbr", "", "")]);
        }
        append(&root, &[h("span", "nw", chunk)]);
    }
    root
}
