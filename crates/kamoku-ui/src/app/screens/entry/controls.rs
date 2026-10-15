// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::prelude::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlInputElement, KeyboardEvent};

type Change = Rc<dyn Fn()>;

pub(crate) fn shortcut(control: &Element, label: &str, keys: &str) {
    let hint = h("kbd", "btn-kbd", label);
    attrs(&hint, &[("aria-hidden", "true")]);
    attrs(control, &[("aria-keyshortcuts", keys)]);
    append(control, &[hint]);
}

pub(crate) use crate::app::core::dom::select_all_on_focus;

pub(crate) fn hidden(el: &Element, value: bool) {
    if value {
        attrs(el, &[("hidden", "")]);
    } else {
        let _ = el.remove_attribute("hidden");
    }
}

pub(crate) fn described(control: &Element, id: &str, on: bool) {
    let current = control
        .get_attribute("aria-describedby")
        .unwrap_or_default();
    let mut ids: Vec<&str> = current
        .split_whitespace()
        .filter(|old| *old != id)
        .collect();
    if on {
        ids.push(id);
    }
    if ids.is_empty() {
        let _ = control.remove_attribute("aria-describedby");
    } else {
        attrs(control, &[("aria-describedby", &ids.join(" "))]);
    }
}

pub(crate) struct Note {
    pub el: Element,
    control: Element,
}
impl Note {
    pub(crate) fn new(control: &Element) -> Self {
        let el = h("p", "field-hint field-note", "");
        attrs(&el, &[("id", &crate::app::core::dom::uid("note"))]);
        hidden(&el, true);
        Self {
            el,
            control: control.clone(),
        }
    }
    pub(crate) fn set(&self, text: &str) {
        set_text(&self.el, text);
        hidden(&self.el, text.is_empty());
        described(&self.control, &self.el.id(), !text.is_empty());
    }
}

pub(crate) fn display_date(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let date = js_sys::Date::new(&format!("{iso}T12:00:00Z").into());
    let day = ["日", "月", "火", "水", "木", "金", "土"]
        .get(date.get_utc_day() as usize)
        .copied()
        .unwrap_or("");
    format!("{iso} ({day})")
}

pub(crate) struct DateControl {
    pub input: HtmlInputElement,
    pub field: Rc<Field>,
    value: RefCell<String>,
    required: bool,
    callback: RefCell<Option<Change>>,
}
impl DateControl {
    pub(crate) fn new(ctx: &Context, initial: &str) -> Rc<Self> {
        Self::named(ctx, initial, "日付", true)
    }
    pub(crate) fn named(ctx: &Context, initial: &str, label: &str, required: bool) -> Rc<Self> {
        let input = input("text", &display_date(initial));
        attrs(
            input.as_ref(),
            &[
                ("enterkeyhint", "next"),
                ("placeholder", "例: 1005、10/5、t（今日）"),
            ],
        );
        let field = Rc::new(field(label, input.as_ref(), "", required));
        let this = Rc::new(Self {
            input,
            field,
            value: RefCell::new(initial.into()),
            required,
            callback: RefCell::new(None),
        });
        let weak = Rc::downgrade(&this);
        ctx.on(this.input.as_ref(), "focus", move |_| {
            if let Some(this) = weak.upgrade()
                && !this.value.borrow().is_empty()
            {
                this.input.set_value(&this.value.borrow());
            }
        });
        select_all_on_focus(ctx, &this.input);
        let weak = Rc::downgrade(&this);
        ctx.on(this.input.as_ref(), "blur", move |_| {
            if let Some(this) = weak.upgrade()
                && this.commit(true)
            {
                this.input.set_value(&display_date(&this.value()));
            }
        });
        let weak = Rc::downgrade(&this);
        ctx.on(this.input.as_ref(), "keydown", move |event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(&event) || event.alt_key() || event.ctrl_key() || event.meta_key() {
                return;
            }
            let Some(this) = weak.upgrade() else {
                return;
            };
            let raw = this.input.value();
            let all = this.input.selection_start().ok().flatten() == Some(0)
                && this.input.selection_end().ok().flatten()
                    == u32::try_from(raw.encode_utf16().count()).ok();
            let old = this.value();
            let base = if old.is_empty() { today() } else { old.clone() };
            let next = match event.key().as_str() {
                "ArrowUp" => Some(if old.is_empty() {
                    base
                } else {
                    super::model::add_days(&base, 1)
                }),
                "ArrowDown" => Some(if old.is_empty() {
                    base
                } else {
                    super::model::add_days(&base, -1)
                }),
                "+" if all => Some(super::model::add_days(&base, 1)),
                "-" if all => Some(super::model::add_days(&base, -1)),
                "t" | "T" if all || raw.is_empty() => Some(today()),
                "y" | "Y" if all || raw.is_empty() => Some(super::model::add_days(&today(), -1)),
                _ => None,
            };
            if let Some(next) = next {
                event.prevent_default();
                this.set_value(&next);
                this.input.set_value(&next);
                this.input.select();
                this.notify();
            }
        });
        this
    }
    pub(crate) fn value(&self) -> String {
        self.value.borrow().clone()
    }
    pub(crate) fn on_change(&self, f: impl Fn() + 'static) {
        *self.callback.borrow_mut() = Some(Rc::new(f));
    }
    fn notify(&self) {
        let f = self.callback.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }
    pub(crate) fn set_value(&self, value: &str) {
        *self.value.borrow_mut() = value.into();
        self.input.set_value(&display_date(value));
        self.field.set_error("");
    }
    pub(crate) fn commit(&self, notify: bool) -> bool {
        let old = self.value();
        let raw = self.input.value();
        if !self.required && raw.trim().is_empty() {
            self.value.borrow_mut().clear();
            self.field.set_error("");
            if notify {
                self.notify();
            }
            return true;
        }
        if !old.is_empty() && (raw.trim() == old || raw.trim() == display_date(&old)) {
            return true;
        }
        // The old control retains the year when entering a short date while editing.
        let normalized = normalize_width(raw.trim()).to_ascii_lowercase();
        let reference = if old.is_empty()
            || matches!(
                normalized.as_str(),
                "t" | "today" | "y" | "yesterday" | "今日" | "昨日"
            ) {
            today()
        } else {
            old
        };
        match parse_date(raw.trim(), &reference) {
            Ok(next) => {
                *self.value.borrow_mut() = next;
                self.field.set_error("");
                if notify {
                    self.notify();
                }
                true
            }
            Err(error) => {
                self.value.borrow_mut().clear();
                self.field.set_error(&error);
                if notify {
                    self.notify();
                }
                false
            }
        }
    }
}

pub(crate) struct AmountControl {
    pub input: HtmlInputElement,
    pub field: Rc<Field>,
    pub note: Note,
    callback: RefCell<Option<Change>>,
    feedback: bool,
}
impl AmountControl {
    pub(crate) fn new(ctx: &Context, initial: Option<i64>) -> Rc<Self> {
        Self::build(ctx, initial, true)
    }
    pub(crate) fn bare(ctx: &Context, initial: Option<i64>) -> Rc<Self> {
        Self::build(ctx, initial, false)
    }
    fn build(ctx: &Context, initial: Option<i64>, feedback: bool) -> Rc<Self> {
        let input = input("text", &initial.map(number).unwrap_or_default());
        input.set_class_name("input input--num");
        attrs(
            input.as_ref(),
            &[
                ("inputmode", "decimal"),
                ("enterkeyhint", "next"),
                ("placeholder", "例: 1650、1000+650"),
            ],
        );
        let affix = h("div", "affix", "");
        append(
            &affix,
            &[input.clone().into(), h("span", "affix-text", "円")],
        );
        let field = Rc::new(field("金額", &affix, "", true));
        let note = Note::new(input.as_ref());
        append(&field.el, std::slice::from_ref(&note.el));
        let this = Rc::new(Self {
            input,
            field,
            note,
            callback: RefCell::new(None),
            feedback,
        });
        select_all_on_focus(ctx, &this.input);
        let weak = Rc::downgrade(&this);
        ctx.on(this.input.as_ref(), "input", move |_| {
            if let Some(this) = weak.upgrade() {
                this.note.set("");
                if this.value().is_some() {
                    this.field.set_error("");
                }
                this.notify();
            }
        });
        let weak = Rc::downgrade(&this);
        ctx.on(this.input.as_ref(), "blur", move |_| {
            if let Some(this) = weak.upgrade()
                && !this.input.value().trim().is_empty()
            {
                this.commit(true);
            }
        });
        this
    }
    pub(crate) fn on_change(&self, f: impl Fn() + 'static) {
        *self.callback.borrow_mut() = Some(Rc::new(f));
    }
    fn notify(&self) {
        let f = self.callback.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }
    pub(crate) fn value(&self) -> Option<i64> {
        parse_amount(&self.input.value()).ok()
    }
    pub(crate) fn set_value(&self, value: Option<i64>) {
        self.input.set_value(&value.map(number).unwrap_or_default());
        self.field.set_error("");
        self.note.set("");
    }
    pub(crate) fn commit(&self, notify: bool) -> bool {
        let raw = self.input.value();
        match parse_amount_details(&raw) {
            Ok(result) => {
                self.input.set_value(&number(result.value));
                self.field.set_error("");
                let mut notes = Vec::new();
                if result.is_expr {
                    notes.push(format!("{} = {}円", raw.trim(), number(result.value)));
                }
                if result.rounded {
                    notes.push("割り算の端数は切り捨てました".into());
                }
                if self.feedback {
                    self.note.set(&notes.join("。"));
                }
                if notify {
                    self.notify();
                }
                true
            }
            Err(error) => {
                if self.feedback {
                    self.field.set_error(&error);
                }
                if notify {
                    self.notify();
                }
                false
            }
        }
    }
}

pub(crate) fn combo_items(values: &[serde_json::Value], partner: bool) -> Vec<ComboItem> {
    values
        .iter()
        .filter(|value| value["enabled"] != false)
        .map(|value| {
            let mut meta = value.clone();
            if !partner {
                let kind = super::model::text(value, "kind");
                let wallet = &value["wallet"];
                let wallet_type = match super::model::text(wallet, "type") {
                    "cash" => "現金",
                    "bank" => "預金",
                    "card" => "カード",
                    "ewallet" => "電子マネー",
                    _ => "",
                };
                let hint = if super::model::is_wallet(value) {
                    format!("口座・{wallet_type}")
                } else {
                    match kind {
                        "asset" => "資産",
                        "liability" => "負債",
                        "equity" => "資本",
                        "revenue" => "収益",
                        "expense" => "費用",
                        _ => "",
                    }
                    .into()
                };
                meta["hint"] = serde_json::json!(hint);
            }
            ComboItem {
                value: super::model::text(value, if partner { "name" } else { "code" }).into(),
                label: super::model::text(value, "name").into(),
                reading: super::model::text(value, "reading").into(),
                meta,
            }
        })
        .collect()
}

pub(crate) fn configure_combo(combo: &Combo, list_label: &str, placeholder: &str, no_match: &str) {
    combo.set_list_label(list_label);
    combo.set_no_match(no_match);
    combo.enter_needs_match(!no_match.is_empty());
    attrs(combo.input.as_ref(), &[("placeholder", placeholder)]);
}
