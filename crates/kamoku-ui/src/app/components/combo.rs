// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub(crate) use crate::choices::ComboItem;
use crate::{
    app::core::{
        context::Context,
        dom::{
            append, attrs, document, focus, h, icon, input, replace, select_all_on_focus, set_text,
            uid, window,
        },
        keys::ime_ignore,
    },
    choices::{self, InputRect, OPTION_LIMIT},
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, Event, HtmlInputElement, KeyboardEvent};

type Change = Rc<dyn Fn(String, Option<ComboItem>, bool)>;
type Selection = Rc<dyn Fn(Option<ComboItem>, &str, Option<Event>) -> bool>;
type Label = Rc<dyn Fn(&str) -> String>;
struct State {
    items: Vec<ComboItem>,
    selected: Option<ComboItem>,
    shown: Vec<ComboItem>,
    active: Option<usize>,
}
struct Inner {
    msg: Element,
    live: Element,
    state: RefCell<State>,
    open: Cell<bool>,
    free: Cell<bool>,
    arrow: Cell<bool>,
    needs_match: Cell<bool>,
    no_match: RefCell<String>,
    change: RefCell<Option<Change>>,
    select: RefCell<Option<Selection>>,
    create_label: RefCell<Option<Label>>,
}
#[derive(Clone)]
pub(crate) struct Combo {
    pub el: Element,
    pub input: HtmlInputElement,
    pub list: Element,
    inner: Rc<Inner>,
}
impl Combo {
    #[allow(
        clippy::too_many_lines,
        reason = "Keep declarative DOM assembly and ordered event registration together"
    )]
    pub(crate) fn new(ctx: &Context, items: Vec<ComboItem>, value: &str) -> Self {
        let id = uid("combo");
        let input = input("text", "");
        input.set_id(&id);
        attrs(
            &input,
            &[
                ("role", "combobox"),
                ("aria-autocomplete", "list"),
                ("aria-expanded", "false"),
                ("aria-controls", &format!("{id}-list")),
                ("autocomplete", "off"),
                ("autocapitalize", "off"),
                ("spellcheck", "false"),
            ],
        );
        let list = h("ul", "combo-list", "");
        list.set_id(&format!("{id}-list"));
        attrs(
            &list,
            &[("role", "listbox"), ("aria-label", "候補"), ("hidden", "")],
        );
        let msg = h("div", "combo-msg", "");
        attrs(&msg, &[("hidden", ""), ("aria-hidden", "true")]);
        let live = h("span", "visually-hidden", "");
        attrs(&live, &[("aria-live", "polite")]);
        let el = h("div", "combo", "");
        let arrow = icon("chevron-down");
        attrs(&arrow, &[("class", "icon combo-chevron")]);
        append(
            &el,
            &[
                input.clone().into(),
                arrow,
                list.clone(),
                msg.clone(),
                live.clone(),
            ],
        );
        let selected = items.iter().find(|i| i.value == value).cloned();
        if let Some(i) = &selected {
            input.set_value(&i.label);
        }
        let inner = Rc::new(Inner {
            msg,
            live,
            state: RefCell::new(State {
                items,
                selected,
                shown: Vec::new(),
                active: None,
            }),
            open: Cell::new(false),
            free: Cell::new(false),
            arrow: Cell::new(true),
            needs_match: Cell::new(false),
            no_match: RefCell::new(String::new()),
            change: RefCell::new(None),
            select: RefCell::new(None),
            create_label: RefCell::new(None),
        });
        let combo = Self {
            el,
            input,
            list,
            inner,
        };
        let c = combo.clone();
        ctx.on(&combo.input, "input", move |_| {
            let text = c.input.value();
            let cleared = {
                let mut s = c.inner.state.borrow_mut();
                if s.selected.as_ref().is_some_and(|i| i.label != text) {
                    s.selected = None;
                    true
                } else {
                    false
                }
            };
            if cleared && !c.inner.free.get() {
                c.notify("input", None);
            }
            c.open();
        });
        select_all_on_focus(ctx, &combo.input);
        let c = combo.clone();
        ctx.on(&combo.input, "click", move |_| {
            if !c.is_open() {
                c.open();
            }
        });
        let c = combo.clone();
        ctx.on(&combo.input, "compositionupdate", move |_| {
            let c = c.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(
                    &wasm_bindgen::JsValue::NULL,
                ))
                .await;
                if c.input.is_connected()
                    && document().active_element().as_ref() == Some(c.input.as_ref())
                {
                    c.open();
                }
            });
        });
        let c = combo.clone();
        ctx.on(&combo.input, "keydown", move |event| {
            if let Ok(e) = event.dyn_into::<KeyboardEvent>() {
                c.key(&e);
            }
        });
        let c = combo.clone();
        ctx.on(&combo.input, "blur", move |_| {
            c.close();
            c.message("");
            let text = c.input.value();
            let selected = c.inner.state.borrow().selected.is_some();
            let with_new = c.inner.free.get() && c.inner.create_label.borrow().is_some();
            if with_new {
                if !selected && !text.trim().is_empty() {
                    let found = {
                        let state = c.inner.state.borrow();
                        let matches = choices::same_name(&state.items, &text);
                        (matches.len() == 1).then(|| matches[0].clone())
                    };
                    if let Some(item) = found {
                        c.choose(Some(item), "blur", None);
                    }
                }
                return;
            }
            if c.inner.free.get() {
                return;
            }
            if selected && text.trim().is_empty() {
                c.choose(None, "blur", None);
                return;
            }
            if !selected && !text.trim().is_empty() {
                let found = choices::pick_on_blur(&c.inner.state.borrow().items, &text);
                if let Some(item) = found {
                    c.choose(Some(item), "blur", None);
                }
            }
        });
        ctx.on(&combo.list, "mousedown", |e| e.prevent_default());
        let c = combo.clone();
        ctx.on(&combo.list, "click", move |event| {
            let item = event
                .target()
                .and_then(|v| v.dyn_into::<Element>().ok())
                .and_then(|v| v.closest("[data-index]").ok().flatten())
                .and_then(|v| v.get_attribute("data-index"))
                .and_then(|v| v.parse::<usize>().ok())
                .and_then(|i| c.inner.state.borrow().shown.get(i).cloned());
            if let Some(item) = item {
                c.choose(Some(item), "click", Some(event));
                c.focus();
            }
        });
        let c = combo.clone();
        ctx.on(&combo.list, "mousemove", move |event| {
            let index = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| target.closest("[role='option']").ok().flatten())
                .and_then(|option| option.get_attribute("data-index"))
                .and_then(|index| index.parse::<usize>().ok());
            if let Some(index) = index {
                let mut state = c.inner.state.borrow_mut();
                if index < state.shown.len() && state.active != Some(index) {
                    state.active = Some(index);
                    drop(state);
                    c.sync(false);
                }
            }
        });
        let c = combo.clone();
        ctx.scope
            .listen(window().as_ref(), "scroll", true, move |_| {
                if c.is_open() {
                    c.position(&c.list);
                }
                if !c.inner.msg.has_attribute("hidden") {
                    c.position(&c.inner.msg);
                }
            });
        let c = combo.clone();
        ctx.scope
            .listen(window().as_ref(), "resize", false, move |_| {
                if c.is_open() {
                    c.position(&c.list);
                }
                if !c.inner.msg.has_attribute("hidden") {
                    c.position(&c.inner.msg);
                }
            });
        combo
    }
    pub(crate) fn value(&self) -> String {
        if self.inner.free.get() {
            self.input.value().trim().into()
        } else {
            self.inner
                .state
                .borrow()
                .selected
                .as_ref()
                .map(|i| i.value.clone())
                .unwrap_or_default()
        }
    }
    pub(crate) fn item(&self) -> Option<ComboItem> {
        self.inner.state.borrow().selected.clone()
    }
    pub(crate) fn is_open(&self) -> bool {
        self.inner.open.get()
    }
    pub(crate) fn allow_free(&self, value: bool) {
        self.inner.free.set(value);
    }
    pub(crate) fn arrow_opens(&self, value: bool) {
        self.inner.arrow.set(value);
    }
    pub(crate) fn enter_needs_match(&self, value: bool) {
        self.inner.needs_match.set(value);
    }
    pub(crate) fn set_list_label(&self, label: &str) {
        attrs(&self.list, &[("aria-label", label)]);
    }
    pub(crate) fn set_no_match(&self, text: &str) {
        *self.inner.no_match.borrow_mut() = text.into();
    }
    pub(crate) fn on_change(&self, f: impl Fn(String, Option<ComboItem>, bool) + 'static) {
        *self.inner.change.borrow_mut() = Some(Rc::new(f));
    }
    pub(crate) fn on_select(
        &self,
        f: impl Fn(Option<ComboItem>, &str, Option<Event>) -> bool + 'static,
    ) {
        *self.inner.select.borrow_mut() = Some(Rc::new(f));
    }
    pub(crate) fn create_label(&self, f: impl Fn(&str) -> String + 'static) {
        *self.inner.create_label.borrow_mut() = Some(Rc::new(f));
        // Screens configure this after set_value; preserve an imported, unknown partner name.
        if self.inner.free.get()
            && self.inner.state.borrow().selected.is_none()
            && !self.input.value().is_empty()
        {
            self.set_value(&self.input.value(), false);
        }
    }
    pub(crate) fn set_items(&self, items: Vec<ComboItem>) {
        let mut s = self.inner.state.borrow_mut();
        if let Some(selected) = &s.selected {
            s.selected = items
                .iter()
                .find(|i| i.value == selected.value)
                .cloned()
                .or_else(|| {
                    if selected.meta["isNew"] == true || selected.meta["kept"] == true {
                        Some(selected.clone())
                    } else {
                        None
                    }
                });
            if let Some(selected) = &s.selected {
                self.input.set_value(&selected.label);
            }
        }
        s.items = items;
        drop(s);
        if self.is_open() {
            self.open();
        }
    }
    pub(crate) fn set_value(&self, value: &str, notify: bool) {
        let item = self
            .inner
            .state
            .borrow()
            .items
            .iter()
            .find(|i| i.value == value)
            .cloned()
            .or_else(|| {
                (!value.is_empty()
                    && self.inner.free.get()
                    && self.inner.create_label.borrow().is_some())
                .then(|| ComboItem {
                    value: value.into(),
                    label: value.into(),
                    reading: String::new(),
                    meta: json!({"kept":true}),
                })
            });
        self.input
            .set_value(item.as_ref().map_or("", |v| v.label.as_str()));
        if item.is_none() && self.inner.free.get() {
            self.input.set_value(value);
        }
        self.inner.state.borrow_mut().selected = item;
        self.close();
        self.message("");
        if notify {
            self.notify("api", None);
        }
    }
    pub(crate) fn focus(&self) {
        focus(&self.input);
    }
    pub(crate) fn close(&self) {
        self.inner.open.set(false);
        attrs(&self.list, &[("hidden", "")]);
        attrs(&self.input, &[("aria-expanded", "false")]);
        let _ = self.input.remove_attribute("aria-activedescendant");
        self.inner.state.borrow_mut().active = None;
    }
    fn notify(&self, via: &str, event: Option<Event>) -> bool {
        let item = self.item();
        let change = self.inner.change.borrow().clone();
        let select = self.inner.select.borrow().clone();
        if let Some(f) = change {
            f(self.value(), item.clone(), via == "enter" || via == "tab");
        }
        select.is_some_and(|f| f(item, via, event))
    }
    fn choose(&self, item: Option<ComboItem>, via: &str, event: Option<Event>) -> bool {
        self.input
            .set_value(item.as_ref().map_or("", |i| i.label.as_str()));
        self.inner.state.borrow_mut().selected = item;
        self.close();
        self.message("");
        self.notify(via, event)
    }
    fn message(&self, text: &str) {
        set_text(&self.inner.msg, text);
        set_text(&self.inner.live, text);
        if text.is_empty() {
            attrs(&self.inner.msg, &[("hidden", "")]);
        } else {
            let _ = self.inner.msg.remove_attribute("hidden");
            self.position(&self.inner.msg);
        }
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Keep declarative DOM assembly and ordered event registration together"
    )]
    pub(crate) fn open(&self) {
        let text = self.input.value();
        let create_label = self
            .inner
            .create_label
            .borrow()
            .clone()
            .filter(|_| self.inner.free.get());
        let mut state = self.inner.state.borrow_mut();
        let exact_selected = state.selected.as_ref().is_some_and(|i| i.label == text);
        state.shown = if exact_selected {
            state.items.iter().take(OPTION_LIMIT).cloned().collect()
        } else if let Some(create_label) = &create_label {
            choices::options_with_new(&state.items, &text, create_label.as_ref(), OPTION_LIMIT)
        } else {
            choices::filter_options(&state.items, &text, OPTION_LIMIT)
        };
        state.active = if exact_selected {
            state
                .selected
                .as_ref()
                .and_then(|i| state.shown.iter().position(|v| v.value == i.value))
        } else {
            (!state.shown.is_empty()
                && (!self.inner.free.get() || create_label.is_some())
                && !text.trim().is_empty())
            .then_some(0)
        };
        let shown = state.shown.clone();
        drop(state);
        replace(&self.list, &[]);
        if shown.is_empty() {
            let no_match = self.inner.no_match.borrow().clone();
            let no_match = if no_match.is_empty() {
                self.input
                    .get_attribute("data-no-match")
                    .unwrap_or_default()
            } else {
                no_match
            };
            if !no_match.is_empty() || create_label.is_some() {
                self.close();
                self.message(if text.trim().is_empty() {
                    ""
                } else {
                    &no_match
                });
                return;
            }
            let empty = h("li", "combo-empty", "候補がありません");
            attrs(&empty, &[("role", "presentation")]);
            append(&self.list, &[empty]);
        }
        for (index, item) in shown.iter().enumerate() {
            let option = h(
                "li",
                if item.meta["isNew"] == true {
                    "combo-option combo-option--new"
                } else {
                    "combo-option"
                },
                "",
            );
            attrs(
                &option,
                &[
                    ("role", "option"),
                    ("id", &format!("{}-opt-{index}", self.input.id())),
                    ("data-index", &index.to_string()),
                    ("aria-selected", "false"),
                ],
            );
            let label = h("span", "combo-option-label", "");
            if item.meta["isNew"] == true {
                append(
                    &label,
                    &[
                        icon("plus"),
                        h(
                            "span",
                            "",
                            item.meta["optionLabel"].as_str().unwrap_or(&item.label),
                        ),
                    ],
                );
            } else {
                set_text(&label, &item.label);
            }
            append(&option, &[label]);
            if let Some(hint) = item.meta["hint"].as_str() {
                append(&option, &[h("span", "combo-option-hint", hint)]);
            }
            append(&self.list, &[option]);
        }
        self.message("");
        self.inner.open.set(true);
        let _ = self.list.remove_attribute("hidden");
        attrs(&self.input, &[("aria-expanded", "true")]);
        self.position(&self.list);
        self.sync(true);
    }
    fn sync(&self, scroll: bool) {
        let active = self.inner.state.borrow().active;
        let options = self.list.children();
        for index in 0..options.length() {
            if let Some(el) = options.item(index) {
                if el.get_attribute("role").as_deref() != Some("option") {
                    continue;
                }
                attrs(
                    &el,
                    &[(
                        "aria-selected",
                        if active == usize::try_from(index).ok() {
                            "true"
                        } else {
                            "false"
                        },
                    )],
                );
                if active == usize::try_from(index).ok() {
                    attrs(&self.input, &[("aria-activedescendant", &el.id())]);
                    if scroll {
                        let o = web_sys::ScrollIntoViewOptions::new();
                        o.set_block(web_sys::ScrollLogicalPosition::Nearest);
                        el.scroll_into_view_with_scroll_into_view_options(&o);
                    }
                }
            }
        }
        if active.is_none() {
            let _ = self.input.remove_attribute("aria-activedescendant");
        }
    }
    fn position(&self, target: &Element) {
        let rect = self.input.get_bounding_client_rect();
        let vw = window()
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(390.0);
        let vh = window()
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(844.0);
        let placement = choices::list_placement(
            InputRect {
                top: rect.top(),
                bottom: rect.bottom(),
                left: rect.left(),
                width: rect.width(),
            },
            vw,
            vh,
        );
        let style = target.unchecked_ref::<web_sys::HtmlElement>().style();
        for (k, v) in [
            ("width", format!("{:.0}px", placement.width)),
            ("left", format!("{:.0}px", placement.left)),
            (
                "top",
                if placement.above {
                    "auto".into()
                } else {
                    format!("{:.0}px", placement.top.unwrap_or_default())
                },
            ),
            (
                "bottom",
                if placement.above {
                    format!("{:.0}px", placement.bottom.unwrap_or_default())
                } else {
                    "auto".into()
                },
            ),
            ("max-height", format!("{:.0}px", placement.max_height)),
        ] {
            let _ = style.set_property(k, &v);
        }
    }
    fn key(&self, e: &KeyboardEvent) {
        if ime_ignore(e) {
            return;
        }
        match e.key().as_str() {
            "ArrowDown" | "ArrowUp" => {
                if self.is_open() {
                    e.prevent_default();
                    let mut s = self.inner.state.borrow_mut();
                    s.active =
                        choices::move_active(s.active, s.shown.len(), e.key() == "ArrowDown");
                } else {
                    if !(self.inner.arrow.get() || e.alt_key() && e.key() == "ArrowDown") {
                        return;
                    }
                    e.prevent_default();
                    self.open();
                    let mut s = self.inner.state.borrow_mut();
                    if s.active.is_none() && !s.shown.is_empty() {
                        s.active = Some(0);
                    }
                    drop(s);
                }
                self.sync(true);
            }
            "Enter" | "Tab" => {
                let item = {
                    let s = self.inner.state.borrow();
                    if self.is_open() {
                        s.active.and_then(|i| s.shown.get(i).cloned())
                    } else {
                        None
                    }
                };
                if let Some(item) = item {
                    let stop = self.choose(
                        Some(item),
                        if e.key() == "Enter" { "enter" } else { "tab" },
                        Some(e.clone().into()),
                    );
                    if stop && e.key() == "Enter" {
                        e.prevent_default();
                        e.stop_propagation();
                    }
                } else if e.key() == "Tab" {
                    self.close();
                } else if self.inner.needs_match.get()
                    && self.item().is_none()
                    && !e.ctrl_key()
                    && !e.meta_key()
                    && !self.input.value().trim().is_empty()
                {
                    self.open();
                    if self.inner.state.borrow().shown.is_empty() {
                        e.prevent_default();
                        e.stop_propagation();
                    }
                }
            }
            "Escape" if (self.is_open() || !self.inner.msg.has_attribute("hidden")) => {
                e.prevent_default();
                e.stop_propagation();
                self.close();
                self.message("");
            }
            _ => {}
        }
    }
}
