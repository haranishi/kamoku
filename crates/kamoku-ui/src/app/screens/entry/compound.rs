// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    controls::{
        AmountControl, DateControl, Note, combo_items, configure_combo, described, display_date,
    },
    model::{
        NO_ACCOUNT, account_options, list, locked_note, next_defaults, tax_label, tax_options, text,
    },
    preview::Preview,
};
use crate::app::prelude::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlInputElement, HtmlSelectElement, KeyboardEvent};

struct Line {
    el: Element,
    side: HtmlSelectElement,
    account: Rc<Combo>,
    amount: Rc<AmountControl>,
    tax: HtmlSelectElement,
    tax_full: Element,
    memo: HtmlInputElement,
    remove: Element,
    previous: RefCell<Value>,
}
impl Line {
    fn value(&self) -> Option<Value> {
        let account = self.account.value();
        if account.is_empty() {
            return None;
        }
        Some(
            json!({"account":account,"side":self.side.value(),"amount":self.amount.value()?,"tax_code":if self.tax.value().is_empty(){Value::Null}else{json!(self.tax.value())},"memo":self.memo.value()}),
        )
    }
    fn sync_tax(&self) {
        let value = self.tax.value();
        let label = tax_label(&value);
        set_text(
            &self.tax_full,
            if label.chars().count() > 12 {
                label
            } else {
                ""
            },
        );
    }
}

pub(crate) struct Compound {
    pub el: Element,
    pub date: Rc<DateControl>,
    ctx: Context,
    accounts: Vec<Value>,
    keep: Vec<String>,
    partner: Rc<Combo>,
    partner_field: Field,
    partner_note: Note,
    description: HtmlInputElement,
    tbody: Element,
    balance: Element,
    line_error: Element,
    problem: RefCell<Option<Element>>,
    rows: RefCell<Vec<Rc<Line>>>,
    preview: Rc<Preview>,
    active: Cell<bool>,
    baseline: RefCell<String>,
    problems: RefCell<Vec<String>>,
    locked: String,
    original_date: String,
    correcting: bool,
    changed_callback: RefCell<Option<Rc<dyn Fn()>>>,
    submit_callback: RefCell<Option<Rc<dyn Fn()>>>,
    cancel_callback: RefCell<Option<Rc<dyn Fn()>>>,
}
impl Drop for Compound {
    fn drop(&mut self) {
        self.ctx.scope.dispose();
    }
}

impl Compound {
    #[allow(
        clippy::too_many_lines,
        reason = "Builds the compound-entry form DOM and registers its scoped field and keyboard handlers."
    )]
    pub(crate) fn new(
        ctx: &Context,
        accounts: &[Value],
        partners: &[Value],
        settings: &Value,
        initial: &Value,
        preview: Rc<Preview>,
        correcting: bool,
    ) -> Rc<Self> {
        let ctx = ctx.child();
        let date_value = if text(initial, "date").is_empty() {
            today()
        } else {
            text(initial, "date").into()
        };
        let date = DateControl::new(&ctx, &date_value);
        let partner = Rc::new(Combo::new(
            &ctx,
            combo_items(partners, true),
            text(initial, "partner"),
        ));
        partner.allow_free(true);
        partner.set_value(text(initial, "partner"), false);
        partner.create_label(|name| format!("「{name}」を新しい取引先として足す"));
        configure_combo(&partner, "取引先の候補", "", "");
        let partner_field = field("取引先", &partner.el, "", false);
        let partner_note = Note::new(partner.input.as_ref());
        append(&partner_field.el, std::slice::from_ref(&partner_note.el));
        let description = input("text", text(initial, "description"));
        attrs(
            description.as_ref(),
            &[("placeholder", "例: 開業時の備品の購入")],
        );
        let desc = field("摘要", description.as_ref(), "", false);
        let _ = desc.el.class_list().add_1("span-2");
        let tbody = h("tbody", "", "");
        let head = h("tr", "", "");
        for (index, label) in ["借方・貸方", "科目", "金額", "税区分", "メモ", "行の操作"]
            .iter()
            .enumerate()
        {
            let th = h("th", if index == 2 { "num" } else { "" }, "");
            attrs(&th, &[("scope", "col")]);
            append(
                &th,
                &[h(
                    "span",
                    if index == 5 { "visually-hidden" } else { "" },
                    label,
                )],
            );
            append(&head, &[th]);
        }
        let thead = h("thead", "", "");
        append(&thead, &[head]);
        let table = h("table", "table table--stack jl-table", "");
        append(
            &table,
            &[h("caption", "", "仕訳の明細"), thead, tbody.clone()],
        );
        let help = h(
            "p",
            "field-hint jl-help",
            "取引を借方と貸方に分けて書き、借方の合計と貸方の合計を同じにします（例: 現金で消耗品を買ったら、借方に消耗品費、貸方に現金）。",
        );
        attrs(&help, &[("id", &crate::app::core::dom::uid("jl-help"))]);
        let wrap = h("div", "table-wrap jl-wrap", "");
        attrs(
            &wrap,
            &[
                ("role", "region"),
                ("aria-label", "仕訳の明細"),
                ("aria-describedby", &help.id()),
                ("tabindex", "0"),
            ],
        );
        append(&wrap, &[table]);
        let balance = h("p", "jl-balance", "");
        attrs(&balance, &[("aria-live", "polite")]);
        let add = button("行を足す", "");
        let _ = add.class_list().add_1("btn--sm");
        let foot = h("div", "jl-foot", "");
        append(&foot, &[add.clone(), balance.clone()]);
        let line_error = h("p", "field-error", "");
        attrs(
            &line_error,
            &[
                ("role", "alert"),
                ("id", &crate::app::core::dom::uid("line-error")),
            ],
        );
        let grid = h("div", "entry-grid", "");
        append(
            &grid,
            &[date.field.el.clone(), partner_field.el.clone(), desc.el],
        );
        let el = h("form", "entry-form", "");
        attrs(
            &el,
            &[("novalidate", ""), ("aria-labelledby", "entry-title")],
        );
        append(&el, &[grid, help, wrap, foot, line_error.clone()]);
        let lines = list(&initial["lines"]);
        let keep = lines
            .iter()
            .map(|line| text(line, "account").into())
            .collect();
        let this = Rc::new(Self {
            el,
            date,
            ctx: ctx.clone(),
            accounts: accounts.to_vec(),
            keep,
            partner,
            partner_field,
            partner_note,
            description,
            tbody,
            balance,
            line_error,
            problem: RefCell::new(None),
            rows: RefCell::new(Vec::new()),
            preview,
            active: Cell::new(true),
            baseline: RefCell::new(String::new()),
            problems: RefCell::new(Vec::new()),
            locked: text(settings, "locked_until").into(),
            original_date: text(initial, "date").into(),
            correcting,
            changed_callback: RefCell::new(None),
            submit_callback: RefCell::new(None),
            cancel_callback: RefCell::new(None),
        });
        let weak = Rc::downgrade(&this);
        ctx.on(&add, "click", move |_| {
            if let Some(this) = weak.upgrade() {
                this.add_row(&Value::Null, true);
            }
        });
        let weak = Rc::downgrade(&this);
        this.date.on_change(move || {
            if let Some(this) = weak.upgrade() {
                let note = this.locked_note(false);
                if !this.date.value().is_empty() && !note.is_empty() {
                    this.date.field.set_error(&note);
                }
                this.changed();
            }
        });
        let weak = Rc::downgrade(&this);
        this.partner.on_change(move |_, item, _| {
            if let Some(this) = weak.upgrade() {
                if item.is_some() {
                    this.partner_field.set_error("");
                }
                this.changed();
            }
        });
        let weak = Rc::downgrade(&this);
        this.partner.on_select(move |_, via, _| {
            if via != "enter" {
                return false;
            }
            if let Some(this) = weak.upgrade() {
                focus(this.description.as_ref());
            }
            true
        });
        for control in [
            this.partner.input.clone().into(),
            this.description.clone().into(),
        ] {
            let weak = Rc::downgrade(&this);
            ctx.on(&control, "input", move |_| {
                if let Some(this) = weak.upgrade() {
                    this.changed();
                }
            });
        }
        ctx.on(&this.el, "submit", |event| event.prevent_default());
        let weak = Rc::downgrade(&this);
        ctx.on(&this.el, "keydown", move |event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(&event) {
                return;
            }
            let Some(this) = weak.upgrade() else {
                return;
            };
            if event.key() == "Enter" && (event.ctrl_key() || event.meta_key()) {
                event.prevent_default();
                this.submit();
                return;
            }
            if event.key() == "Escape" && !event.default_prevented() {
                event.prevent_default();
                let callback = this.cancel_callback.borrow().clone();
                if let Some(callback) = callback {
                    callback();
                }
                return;
            }
            if event.key() != "Enter"
                || event.default_prevented()
                || event.shift_key()
                || event.alt_key()
            {
                return;
            }
            let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            if target.tag_name() == "BUTTON" {
                return;
            }
            event.prevent_default();
            this.next(&target);
        });
        if lines.len() >= 2 {
            for line in lines {
                this.add_row(&line, false);
            }
        } else {
            this.add_row(&json!({"side":"debit"}), false);
            this.add_row(&json!({"side":"credit"}), false);
        }
        *this.baseline.borrow_mut() = this.fingerprint();
        this
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Builds one accessible native table row and registers all of that row's scoped handlers."
    )]
    fn add_row(self: &Rc<Self>, value: &Value, interactive: bool) {
        let diff = self.sums().2;
        let side = select(
            &[("debit", "借方"), ("credit", "貸方")],
            if text(value, "side").is_empty() {
                if diff > 0 { "credit" } else { "debit" }
            } else {
                text(value, "side")
            },
        );
        let account = Rc::new(Combo::new(
            &self.ctx,
            combo_items(
                &account_options(&self.accounts, "expense", true, &self.keep),
                false,
            ),
            text(value, "account"),
        ));
        configure_combo(&account, "科目の候補", "", NO_ACCOUNT);
        let amount = AmountControl::bare(
            &self.ctx,
            value["amount"].as_i64().or_else(|| {
                if interactive && diff != 0 {
                    Some(diff.abs())
                } else {
                    None
                }
            }),
        );
        let tax = select(&tax_options("journal"), text(value, "tax_code"));
        let tax_full = h("span", "jl-tax-full", "");
        attrs(&tax_full, &[("aria-hidden", "true")]);
        let memo = input("text", text(value, "memo"));
        let remove = button("", "ghost");
        remove.set_class_name("btn btn--ghost btn--icon");
        append(&remove, &[icon("trash")]);
        let el = h("tr", "", "");
        for (label, class, children) in [
            ("借方・貸方", "jl-side", vec![side.clone().into()]),
            ("科目", "jl-account", vec![account.el.clone()]),
            ("金額", "num jl-amount", vec![amount.input.clone().into()]),
            (
                "税区分",
                "jl-tax",
                vec![tax.clone().into(), tax_full.clone()],
            ),
            ("メモ", "jl-memo", vec![memo.clone().into()]),
            ("", "jl-del", vec![remove.clone()]),
        ] {
            let td = h("td", class, "");
            attrs(&td, &[("data-label", label)]);
            append(&td, &children);
            append(&el, &[td]);
        }
        let previous = self
            .accounts
            .iter()
            .find(|account| account["code"] == value["account"])
            .cloned()
            .unwrap_or(Value::Null);
        let row = Rc::new(Line {
            el,
            side,
            account,
            amount,
            tax,
            tax_full,
            memo,
            remove,
            previous: RefCell::new(previous),
        });
        row.sync_tax();
        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        row.account.on_change(move |_, item, _| {
            if let (Some(this), Some(row)) = (weak.upgrade(), weak_row.upgrade()) {
                if item.is_some() {
                    this.clear_problem(row.account.input.as_ref());
                }
                let next = item.map_or(Value::Null, |item| item.meta);
                let (tax, _) = next_defaults(&row.previous.borrow(), &next, &row.tax.value(), 100);
                row.tax.set_value(&tax);
                *row.previous.borrow_mut() = next;
                row.sync_tax();
                this.changed();
            }
        });
        let weak_row = Rc::downgrade(&row);
        row.account.on_select(move |_, via, _| {
            if via != "enter" {
                return false;
            }
            if let Some(row) = weak_row.upgrade() {
                focus(row.amount.input.as_ref());
            }
            true
        });
        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        row.amount.on_change(move || {
            if let (Some(this), Some(row)) = (weak.upgrade(), weak_row.upgrade()) {
                if row.amount.value().is_some() {
                    this.clear_problem(row.amount.input.as_ref());
                }
                this.changed();
            }
        });
        for (control, event) in [
            (row.side.clone().into(), "change"),
            (row.tax.clone().into(), "change"),
            (row.memo.clone().into(), "input"),
        ] {
            let weak = Rc::downgrade(self);
            let weak_row = Rc::downgrade(&row);
            self.ctx.on(&control, event, move |_| {
                if let (Some(this), Some(row)) = (weak.upgrade(), weak_row.upgrade()) {
                    row.sync_tax();
                    this.changed();
                }
            });
        }
        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        self.ctx.on(&row.remove, "click", move |_| {
            if let (Some(this), Some(row)) = (weak.upgrade(), weak_row.upgrade()) {
                if this.rows.borrow().len() <= 2 {
                    set_text(
                        &this.line_error,
                        "明細は2行以上が要ります。行を消す前に、別の行を足してください。",
                    );
                    return;
                }
                let index = this
                    .rows
                    .borrow()
                    .iter()
                    .position(|other| Rc::ptr_eq(other, &row))
                    .unwrap_or(0);
                this.rows
                    .borrow_mut()
                    .retain(|other| !Rc::ptr_eq(other, &row));
                row.el.remove();
                this.renumber();
                if let Some(next) = this
                    .rows
                    .borrow()
                    .get(index.min(this.rows.borrow().len() - 1))
                {
                    focus(next.side.as_ref());
                }
                this.changed();
            }
        });
        self.rows.borrow_mut().push(row.clone());
        append(&self.tbody, std::slice::from_ref(&row.el));
        self.renumber();
        if interactive {
            focus(row.side.as_ref());
        }
        self.changed();
    }
    fn renumber(&self) {
        for (index, row) in self.rows.borrow().iter().enumerate() {
            let n = index + 1;
            for (control, label) in [
                (row.side.clone().into(), "借方か貸方"),
                (row.account.input.clone().into(), "科目"),
                (row.amount.input.clone().into(), "金額"),
                (row.tax.clone().into(), "税区分"),
                (row.memo.clone().into(), "メモ"),
            ] {
                attrs(&control, &[("aria-label", &format!("{n}行目 {label}"))]);
            }
            attrs(
                &row.remove,
                &[
                    ("aria-label", &format!("{n}行目を消す")),
                    ("title", &format!("{n}行目を消す")),
                ],
            );
        }
    }
    fn sums(&self) -> (i64, i64, i64) {
        let (mut debit, mut credit) = (0_i64, 0_i64);
        for row in self.rows.borrow().iter() {
            let amount = row.amount.value().unwrap_or(0);
            if row.side.value() == "debit" {
                debit = debit.saturating_add(amount);
            } else {
                credit = credit.saturating_add(amount);
            }
        }
        (debit, credit, debit - credit)
    }
    pub(crate) fn balanced(&self) -> bool {
        self.sums().2 == 0 && self.rows.borrow().len() >= 2
    }
    fn changed(&self) {
        if self.partner.input.value().trim().is_empty() {
            self.partner_field.set_error("");
        }
        let (debit, credit, diff) = self.sums();
        let badge = if debit == 0 && credit == 0 {
            h(
                "span",
                "badge badge--muted",
                "金額を入れると、貸借の差を確かめます",
            )
        } else if diff == 0 {
            h("span", "badge badge--ok", "貸借が合っています")
        } else {
            h(
                "span",
                "badge badge--warn",
                &format!(
                    "差 {}円（{}が足りません）",
                    number(diff.abs()),
                    if diff > 0 { "貸方" } else { "借方" }
                ),
            )
        };
        replace(
            &self.balance,
            &[
                h("span", "", &format!("借方 {}円", number(debit))),
                h("span", "", &format!("貸方 {}円", number(credit))),
                badge,
            ],
        );
        self.partner_note.set(
            &self
                .partner
                .item()
                .filter(|item| item.meta["isNew"] == true || item.meta["is_new"] == true)
                .map(|item| format!("新しい取引先: {}", item.label))
                .unwrap_or_default(),
        );
        if self.active.get() {
            self.preview.schedule(self.snapshot(), "");
        }
        let callback = self.changed_callback.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }
    pub(crate) fn on_change(&self, f: impl Fn() + 'static) {
        *self.changed_callback.borrow_mut() = Some(Rc::new(f));
    }
    pub(crate) fn on_submit(&self, f: impl Fn() + 'static) {
        *self.submit_callback.borrow_mut() = Some(Rc::new(f));
    }
    pub(crate) fn on_cancel(&self, f: impl Fn() + 'static) {
        *self.cancel_callback.borrow_mut() = Some(Rc::new(f));
    }
    fn submit(&self) {
        let callback = self.submit_callback.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }
    pub(crate) fn set_active(&self, on: bool) {
        self.active.set(on);
        if on {
            self.changed();
        }
    }
    pub(crate) fn snapshot(&self) -> Option<Value> {
        let date = self.date.value();
        if date.is_empty() || self.rows.borrow().len() < 2 {
            return None;
        }
        let lines: Option<Vec<Value>> = self.rows.borrow().iter().map(|row| row.value()).collect();
        Some(
            json!({"kind":"journal","date":date,"partner":self.partner.value(),"description":self.description.value(),"lines":lines?}),
        )
    }
    pub(crate) fn collect(&self) -> Option<Value> {
        if let Some(previous) = self.problem.borrow_mut().take() {
            described(&previous, &self.line_error.id(), false);
        }
        set_text(&self.line_error, "");
        let mut problems: Vec<(Element, String, String)> = Vec::new();
        if !self.date.commit(false) {
            problems.push((self.date.input.clone().into(), "日付".into(), String::new()));
        }
        let name = self.partner.input.value();
        let unsure = !name.trim().is_empty() && self.partner.item().is_none();
        self.partner_field.set_error(&if unsure {
            format!(
                "「{}」は候補にありません。候補から選ぶか、一覧の最後の行で足してください。",
                name.trim()
            )
        } else {
            String::new()
        });
        if unsure {
            problems.push((
                self.partner.input.clone().into(),
                "取引先".into(),
                String::new(),
            ));
        }
        for (index, row) in self.rows.borrow().iter().enumerate() {
            let _ = row.account.input.remove_attribute("aria-invalid");
            let _ = row.amount.input.remove_attribute("aria-invalid");
            let n = index + 1;
            if row.account.value().is_empty() {
                attrs(row.account.input.as_ref(), &[("aria-invalid", "true")]);
                problems.push((
                    row.account.input.clone().into(),
                    format!("{n}行目の科目"),
                    format!("{n}行目の科目を選んでください。"),
                ));
            }
            if !row.amount.commit(false) {
                attrs(row.amount.input.as_ref(), &[("aria-invalid", "true")]);
                problems.push((
                    row.amount.input.clone().into(),
                    format!("{n}行目の金額"),
                    format!("{n}行目の金額を入れてください（1円以上）。"),
                ));
            }
        }
        if problems.is_empty()
            && self.sums().2 != 0
            && let Some(row) = self.rows.borrow().last()
        {
            problems.push((
                row.amount.input.clone().into(),
                "貸借の差".into(),
                format!(
                    "貸借が合っていません。差の{}円を、どちらかの行に足してください。",
                    number(self.sums().2.abs())
                ),
            ));
        }
        *self.problems.borrow_mut() = problems.iter().map(|(_, name, _)| name.clone()).collect();
        if let Some((first, _, message)) = problems.first() {
            set_text(&self.line_error, message);
            described(first, &self.line_error.id(), !message.is_empty());
            if !message.is_empty() {
                *self.problem.borrow_mut() = Some(first.clone());
            }
            focus(first);
            None
        } else {
            self.snapshot()
        }
    }
    pub(crate) fn problems(&self) -> Vec<String> {
        self.problems.borrow().clone()
    }
    fn clear_problem(&self, control: &Element) {
        let _ = control.remove_attribute("aria-invalid");
        if self.problem.borrow().as_ref() == Some(control) {
            self.problem.borrow_mut().take();
            described(control, &self.line_error.id(), false);
            set_text(&self.line_error, "");
        }
    }
    pub(crate) fn locked_note(&self, on_save: bool) -> String {
        locked_note(
            &self.date.value(),
            &self.locked,
            self.correcting,
            &self.original_date,
            on_save,
        )
    }
    pub(crate) fn show_error(&self, name: &str, message: &str) -> bool {
        if name == "date" {
            self.date.field.set_error(message);
            focus(self.date.input.as_ref());
        } else {
            set_text(&self.line_error, message);
        }
        true
    }
    fn fingerprint(&self) -> String {
        let date = self.date.value();
        let raw = self.date.input.value();
        let raw = if raw.trim() == date || raw.trim() == display_date(&date) {
            ""
        } else {
            raw.trim()
        };
        let rows: Vec<Value> = self
            .rows
            .borrow()
            .iter()
            .map(|row| {
                json!([
                    row.side.value(),
                    row.account.value(),
                    row.account.input.value(),
                    row.amount.value(),
                    if row.amount.value().is_none() {
                        row.amount.input.value()
                    } else {
                        String::new()
                    },
                    row.tax.value(),
                    row.memo.value()
                ])
            })
            .collect();
        json!([
            date,
            raw,
            self.partner.input.value(),
            self.description.value(),
            rows
        ])
        .to_string()
    }
    pub(crate) fn dirty(&self) -> bool {
        self.fingerprint() != *self.baseline.borrow()
    }
    pub(crate) fn focus_amount(&self) {
        if let Some(row) = self.rows.borrow().first() {
            focus(row.amount.input.as_ref());
        }
    }
    fn next(self: &Rc<Self>, target: &Element) {
        let mut controls: Vec<Element> = vec![
            self.date.input.clone().into(),
            self.partner.input.clone().into(),
            self.description.clone().into(),
        ];
        for row in self.rows.borrow().iter() {
            controls.extend([
                row.side.clone().into(),
                row.account.input.clone().into(),
                row.amount.input.clone().into(),
                row.tax.clone().into(),
                row.memo.clone().into(),
            ]);
        }
        if let Some(index) = controls.iter().position(|control| control == target) {
            if let Some(next) = controls.get(index + 1) {
                focus(next);
            } else if self.balanced() {
                self.submit();
            } else {
                self.add_row(&Value::Null, true);
            }
        }
    }
}
