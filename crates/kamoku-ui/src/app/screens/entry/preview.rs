// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::model::{integer, line_memo, list, text};
use crate::app::prelude::*;
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{AbortController, Element};

pub(crate) fn preview_table(lines: &[Value], caption: &str) -> Element {
    let table = h("table", "table pv-table", "");
    let head = h("tr", "", "");
    for (label, class) in [("貸借", ""), ("科目", ""), ("金額", "num")] {
        let th = h("th", class, label);
        attrs(&th, &[("scope", "col")]);
        append(&head, &[th]);
    }
    let thead = h("thead", "", "");
    append(&thead, &[head]);
    let tbody = h("tbody", "", "");
    for side in ["debit", "credit"] {
        for line in lines.iter().filter(|line| line["side"] == side) {
            let row = h(
                "tr",
                if side == "credit" {
                    "pv-credit"
                } else {
                    "pv-debit"
                },
                "",
            );
            let account = h("td", "pv-account", "");
            let name = if text(line, "account_name").is_empty() {
                text(line, "account")
            } else {
                text(line, "account_name")
            };
            append(&account, &[h("span", "", name)]);
            let memo = line_memo(line);
            if !memo.is_empty() {
                append(&account, &[h("span", "sub", &memo)]);
            }
            let amount = h("td", "num", &number(integer(line, "amount")));
            if integer(line, "tax_amount") != 0 {
                append(
                    &amount,
                    &[h(
                        "span",
                        "sub",
                        &format!("（税{}）", number(integer(line, "tax_amount"))),
                    )],
                );
            }
            append(
                &row,
                &[
                    h(
                        "td",
                        "pv-side",
                        if side == "debit" { "借方" } else { "貸方" },
                    ),
                    account,
                    amount,
                ],
            );
            append(&tbody, &[row]);
        }
    }
    let total = lines
        .iter()
        .filter(|line| line["side"] == "debit")
        .map(|line| integer(line, "amount"))
        .sum();
    let label = h("th", "", "合計（借方と貸方で同じ）");
    attrs(&label, &[("scope", "row"), ("colspan", "2")]);
    let footrow = h("tr", "", "");
    append(&footrow, &[label, h("td", "num", &number(total))]);
    let foot = h("tfoot", "", "");
    append(&foot, &[footrow]);
    append(&table, &[h("caption", "", caption), thead, tbody, foot]);
    let wrap = h("div", "table-wrap pv-wrap", "");
    append(&wrap, &[table]);
    wrap
}

pub(crate) fn preview_summary(lines: &[Value]) -> Vec<Element> {
    let mut parts = Vec::new();
    for side in ["debit", "credit"] {
        let values: Vec<&Value> = lines.iter().filter(|line| line["side"] == side).collect();
        let Some(first) = values.first() else {
            continue;
        };
        if !parts.is_empty() {
            let sep = h("span", "pv-sum-sep", "／");
            attrs(&sep, &[("aria-hidden", "true")]);
            parts.push(sep);
        }
        let name = text(first, "account_name");
        let name = if values.len() > 1 {
            format!("{name} ほか{}行", values.len() - 1)
        } else {
            name.into()
        };
        let total = values.iter().map(|line| integer(line, "amount")).sum();
        let part = h("span", "pv-sum-side", "");
        append(
            &part,
            &[
                h(
                    "span",
                    "pv-sum-label",
                    if side == "debit" {
                        "借方 "
                    } else {
                        "貸方 "
                    },
                ),
                h("span", "pv-sum-name keep", &format!("{name} ")),
                h("span", "num", &number(total)),
            ],
        );
        parts.push(part);
    }
    parts
}

type Timer = (i32, Closure<dyn FnMut()>);
type LinesHandler = Rc<dyn Fn(Vec<Value>)>;

pub(crate) struct Preview {
    pub el: Element,
    body: Element,
    summary: Element,
    ctx: Context,
    failed: Cell<bool>,
    sequence: Cell<u64>,
    timer: RefCell<Option<Timer>>,
    controller: RefCell<Option<AbortController>>,
    last: RefCell<Vec<Value>>,
    request: RefCell<Option<(Option<Value>, String)>>,
    changed: RefCell<Option<Rc<dyn Fn()>>>,
    on_lines: RefCell<Option<LinesHandler>>,
}
impl Preview {
    pub(crate) fn new(ctx: &Context) -> Rc<Self> {
        let body = h("div", "pv-body", "");
        attrs(&body, &[("id", "pv-body"), ("aria-live", "polite")]);
        let summary = h("p", "pv-summary", "");
        attrs(&summary, &[("aria-live", "polite")]);
        let toggle = button("くわしく", "ghost");
        toggle.set_class_name("btn btn--ghost btn--sm pv-toggle");
        attrs(
            &toggle,
            &[("aria-expanded", "false"), ("aria-controls", "pv-body")],
        );
        append(&toggle, &[icon("chevron-down")]);
        let title = h("h2", "card-title", "仕訳のプレビュー");
        attrs(&title, &[("id", "pv-title")]);
        let head = h("div", "pv-head", "");
        append(&head, &[title, toggle.clone()]);
        let el = h("section", "card pv-card", "");
        attrs(&el, &[("aria-labelledby", "pv-title")]);
        append(&el, &[head, summary.clone(), body.clone()]);
        let this = Rc::new(Self {
            el,
            body,
            summary,
            ctx: ctx.clone(),
            failed: Cell::new(false),
            sequence: Cell::new(0),
            timer: RefCell::new(None),
            controller: RefCell::new(None),
            last: RefCell::new(Vec::new()),
            request: RefCell::new(None),
            changed: RefCell::new(None),
            on_lines: RefCell::new(None),
        });
        let weak = Rc::downgrade(&this);
        ctx.on(&toggle.clone(), "click", move |_| {
            if let Some(this) = weak.upgrade() {
                let open = toggle.get_attribute("aria-expanded").as_deref() != Some("true");
                attrs(
                    &toggle,
                    &[("aria-expanded", if open { "true" } else { "false" })],
                );
                let _ = this.el.class_list().toggle_with_force("is-open", open);
            }
        });
        let weak = Rc::downgrade(&this);
        ctx.scope.cleanup(move || {
            if let Some(this) = weak.upgrade() {
                this.cancel();
            }
        });
        this.idle("日付・金額・科目・口座を入れると、ここに保存される仕訳が出ます。");
        this
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed.get()
    }
    pub(crate) fn lines(&self) -> Vec<Value> {
        self.last.borrow().clone()
    }
    pub(crate) fn on_change(&self, callback: impl Fn() + 'static) {
        *self.changed.borrow_mut() = Some(Rc::new(callback));
    }
    pub(crate) fn on_lines(&self, callback: impl Fn(Vec<Value>) + 'static) {
        *self.on_lines.borrow_mut() = Some(Rc::new(callback));
    }
    fn set_failed(&self, failed: bool) {
        if self.failed.replace(failed) != failed {
            let callback = self.changed.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        }
    }
    fn set_lines(&self, lines: Vec<Value>) {
        self.last.borrow_mut().clone_from(&lines);
        let callback = self.on_lines.borrow().clone();
        if let Some(callback) = callback {
            callback(lines);
        }
    }
    fn idle(&self, message: &str) {
        let _ = self.el.class_list().remove_1("has-lines");
        replace(&self.body, &[h("p", "muted pv-idle", message)]);
    }
    pub(crate) fn cancel(&self) {
        self.sequence.set(self.sequence.get() + 1);
        if let Some((id, _)) = self.timer.borrow_mut().take() {
            web_sys::window()
                .expect("window")
                .clear_timeout_with_handle(id);
        }
        if let Some(controller) = self.controller.borrow_mut().take() {
            controller.abort();
        }
    }
    pub(crate) fn schedule(self: &Rc<Self>, body: Option<Value>, problem: &str) {
        self.cancel();
        *self.request.borrow_mut() = Some((body, problem.into()));
        let weak = Rc::downgrade(self);
        let callback = Closure::wrap(Box::new(move || {
            if let Some(this) = weak.upgrade() {
                this.run();
            }
        }) as Box<dyn FnMut()>);
        let id = web_sys::window()
            .expect("window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                150,
            )
            .expect("timer");
        *self.timer.borrow_mut() = Some((id, callback));
    }
    pub(crate) fn run(self: &Rc<Self>) {
        if let Some((id, _)) = self.timer.borrow().as_ref() {
            web_sys::window()
                .expect("window")
                .clear_timeout_with_handle(*id);
        }
        if let Some(controller) = self.controller.borrow_mut().take() {
            controller.abort();
        }
        self.sequence.set(self.sequence.get() + 1);
        let Some((input, problem)) = self.request.borrow().clone() else {
            return;
        };
        let sequence = self.sequence.get();
        if !problem.is_empty() {
            self.set_failed(false);
            self.set_lines(Vec::new());
            let _ = self.el.class_list().remove_1("has-lines");
            replace(&self.body, &[h("p", "field-error pv-blocked", &problem)]);
            return;
        }
        let Some(input) = input else {
            self.set_failed(false);
            self.set_lines(Vec::new());
            self.idle("日付・金額・科目・口座を入れると、ここに保存される仕訳が出ます。");
            return;
        };
        let controller = AbortController::new().expect("AbortController");
        *self.controller.borrow_mut() = Some(controller.clone());
        let weak = Rc::downgrade(self);
        let ctx = self.ctx.clone();
        self.ctx.task(async move {
            let result = ctx.post_abort("/entries/preview", &input, &controller.signal()).await;
            let Some(this) = weak.upgrade().filter(|this| this.ctx.alive() && this.sequence.get() == sequence) else { return Ok(()); };
            match result {
                Ok(response) => {
                    this.set_failed(false); let lines = list(&response["lines"]); this.set_lines(lines.clone());
                    let _ = this.el.class_list().add_1("has-lines"); replace(&this.summary, &preview_summary(&lines));
                    let mut parts = vec![preview_table(&lines, "保存される仕訳")];
                    let warnings = list(&response["warnings"]);
                    if !warnings.is_empty() { let ul = h("ul", "pv-warnings", ""); for warning in warnings { append(&ul, &[h("li", "", warning.as_str().unwrap_or_default())]); } parts.push(ul); }
                    replace(&this.body, &parts);
                }
                Err(error) => {
                    if controller.signal().aborted() { return Ok(()); }
                    this.set_lines(Vec::new()); let _ = this.el.class_list().remove_1("has-lines");
                    if error.status == 0 || error.status >= 500 {
                        this.set_failed(true); let block = h("div", "pv-failed", ""); attrs(&block, &[("id", "preview-error"), ("role", "alert")]);
                        let retry = button("もう一度試す", ""); retry.set_class_name("btn btn--sm");
                        append(&block, &[h("p", "field-error", &format!("仕訳のプレビューを作れませんでした。{} 保存は止めています。", error.message)), retry.clone()]);
                        let weak = Rc::downgrade(&this); this.ctx.on(&retry, "click", move |_| if let Some(this) = weak.upgrade() { this.run(); }); replace(&this.body, &[block]);
                    } else { this.set_failed(false); replace(&this.body, &[h("p", "pv-note", &error.message)]); }
                }
            }
            Ok(())
        });
    }
}
