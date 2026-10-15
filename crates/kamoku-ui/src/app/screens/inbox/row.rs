// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    BTreeMap, Cell, Combo, Context, Element, Event, HtmlInputElement, HtmlSelectElement, JsCast,
    KeyboardEvent, Rc, RefCell, State, Value, VecDeque, Weak, add_days, attrs, class, focus, id,
    ime_ignore, is_wallet, json, parse_date, pause, pct_value, row_view, set_text, step_option,
    tax_after, text, today,
};
pub(super) struct Row {
    pub parent: Weak<State>,
    pub ctx: Context,
    pub tr: Element,
    pub msg_tr: Element,
    pub msg: Element,
    pub draft: RefCell<Value>,
    pub check: HtmlInputElement,
    pub date: HtmlInputElement,
    pub sum_date: Element,
    pub account: Rc<Combo>,
    pub tax: HtmlSelectElement,
    pub partner: Rc<Combo>,
    pub memo: HtmlInputElement,
    pub pct: Option<HtmlInputElement>,
    pub more: Element,
    pub ignore: Element,
    pub name: Element,
    pub touched: Cell<bool>,
    pub errors: RefCell<BTreeMap<String, String>>,
    pub leaving: Cell<bool>,
    pub(super) queue: RefCell<VecDeque<Value>>,
    pub(super) saving: Cell<bool>,
}
impl Row {
    pub fn id(&self) -> u64 {
        id(&self.draft.borrow())
    }
    pub fn ready(&self) -> bool {
        !text(&self.draft.borrow(), "account").is_empty() && !self.leaving.get()
    }
    pub fn choose(self: &Rc<Self>, code: &str) {
        let Some(s) = self.parent.upgrade() else {
            return;
        };
        if s.ctx.is_read_only() {
            return;
        }
        let old = self.draft.borrow().clone();
        if text(&old, "account") == code {
            return;
        }
        let prev = s.account(text(&old, "account"));
        let next = s.account(code);
        let income = text(&old, "direction") == "in";
        if is_wallet(&next) {
            self.touched.set(false);
        }
        let tax = tax_after(
            &next,
            income,
            s.exempt.get(),
            text(&old, "tax_code"),
            self.touched.get(),
        );
        let pct = if income {
            100
        } else {
            u64::try_from(
                crate::app::screens::entry::model::next_defaults(
                    &prev,
                    &next,
                    text(&old, "tax_code"),
                    old["business_pct"].as_i64().unwrap_or(100),
                )
                .1,
            )
            .unwrap_or(100)
        };
        let mut patch = json!({"account":if code.is_empty(){Value::Null}else{json!(code)}});
        if tax != text(&old, "tax_code") {
            patch["tax_code"] = if tax.is_empty() {
                Value::Null
            } else {
                json!(tax)
            };
        }
        if pct != old["business_pct"].as_u64().unwrap_or(100) {
            patch["business_pct"] = json!(pct);
        }
        self.errors.borrow_mut().remove("account");
        self.enqueue(patch);
        if let Some(input) = &self.pct {
            input.set_value(&pct.to_string());
        }
        if code.is_empty() {
            self.set_candidate(false);
        }
        self.paint();
        s.paint_bar();
    }
    pub fn enqueue(self: &Rc<Self>, patch: Value) {
        let Some(s) = self.parent.upgrade() else {
            return;
        };
        if s.ctx.is_read_only() {
            return;
        }
        if let (Some(d), Some(p)) = (self.draft.borrow_mut().as_object_mut(), patch.as_object()) {
            for (k, v) in p {
                d.insert(k.clone(), v.clone());
            }
        }
        self.queue.borrow_mut().push_back(patch);
        s.bar.saved(&s.ctx, "saving");
        if self.saving.replace(true) {
            return;
        }
        let row = self.clone();
        let ctx = s.ctx.clone();
        ctx.task(async move {
            loop {
                let patch = row.queue.borrow_mut().pop_front();
                let Some(patch) = patch else {
                    break;
                };
                let result = s.ctx.patch(&format!("/drafts/{}", row.id()), &patch).await;
                if !s.ctx.alive() {
                    if let Err(error) = result {
                        s.ctx.error(&format!(
                            "未確認の欄を保存できませんでした。{}",
                            error.message
                        ));
                    }
                    continue;
                }
                match result {
                    Ok(_) => {
                        if let Some(p) = patch.as_object().filter(|_| row.ctx.alive()) {
                            for k in p.keys() {
                                row.set_error(if k == "tax_code" { "tax" } else { k }, "");
                            }
                        }
                        s.bar.saved(&s.ctx, "saved");
                    }
                    Err(err) => {
                        s.bar.saved(&s.ctx, "failed");
                        if err.code == "already_confirmed" || err.status == 404 || err.is_stale() {
                            s.stale();
                        } else if row.ctx.alive() {
                            row.set_error(err.field.as_deref().unwrap_or("row"), &err.message);
                        } else {
                            s.ctx.error(&format!(
                                "未確認の欄を保存できませんでした。{}",
                                err.message
                            ));
                        }
                    }
                }
            }
            row.saving.set(false);
            Ok(())
        });
    }
    pub async fn flush(&self) {
        while self.saving.get() {
            pause(5).await;
        }
    }
    pub fn set_candidate(self: &Rc<Self>, on: bool) -> bool {
        let Some(s) = self.parent.upgrade() else {
            return false;
        };
        if s.ctx.is_read_only() {
            self.check.set_checked(false);
            return false;
        }
        if on && !self.ready() {
            self.check.set_checked(false);
            if !self.leaving.get() {
                self.set_error(
                    "account",
                    "科目を選んでください。科目が空の行は、確定できません。",
                );
            }
            return false;
        }
        let d = self.draft.borrow().clone();
        s.selection.borrow_mut().set(&d, on);
        self.check.set_checked(on);
        class(&self.tr, "is-candidate", on);
        if on && s.exempt.get() && !text(&d, "tax_code").is_empty() {
            self.enqueue(json!({"tax_code":null}));
            self.paint();
        }
        s.paint_bar();
        true
    }
    pub fn commit(self: &Rc<Self>, col: &str) -> bool {
        let Some(s) = self.parent.upgrade() else {
            return false;
        };
        let old = self.draft.borrow().clone();
        match col {
            "account" => {
                let v = self.account.value();
                if v != text(&old, "account") {
                    self.choose(&v);
                }
                if v.is_empty() && !self.account.input.value().trim().is_empty() {
                    self.set_error(
                        "account",
                        "打った文字に合う科目がありません。候補から選んでください。",
                    );
                    return false;
                }
            }
            "date" => {
                let raw = self.date.value();
                if raw != row_view::short_date(text(&old, "date")) {
                    match parse_date(&raw, &today()) {
                        Ok(date) => {
                            self.date.set_value(&row_view::short_date(&date));
                            if date != text(&old, "date") {
                                self.enqueue(json!({"date":date}));
                            }
                            self.set_error("date", "");
                            self.paint_name();
                        }
                        Err(err) => {
                            self.set_error("date", &err);
                            return false;
                        }
                    }
                }
            }
            "partner" => {
                let v = self.partner.value();
                if v != text(&old, "partner") {
                    self.enqueue(json!({"partner":v}));
                }
            }
            "memo" => {
                let v = self.memo.value();
                if v != text(&old, "memo") {
                    self.enqueue(json!({"memo":v}));
                }
            }
            "pct" => {
                if let Some(input) = &self.pct
                    && !input.disabled()
                {
                    let Some(n) = pct_value(&input.value()) else {
                        self.set_error(
                            "business_pct",
                            "事業の割合は、0から100の整数で入れてください。",
                        );
                        return false;
                    };
                    self.set_error("business_pct", "");
                    if n != old["business_pct"].as_u64().unwrap_or(100) {
                        self.enqueue(json!({"business_pct":n}));
                    }
                }
            }
            _ => {}
        }
        if !s.ctx.alive() {
            return false;
        }
        true
    }
    pub fn mark_next(self: &Rc<Self>, col: &str) {
        if !self.commit(col) || !self.set_candidate(true) {
            return;
        }
        if let Some(s) = self.parent.upgrade() {
            let rows = s.rows.borrow();
            let index = rows.iter().position(|r| r.id() == self.id());
            if let Some(next) = index.and_then(|i| rows.get(i + 1)) {
                next.focus("account", false);
            } else {
                focus(&s.bar.confirm);
            }
        }
    }
    pub fn focus(&self, col: &str, nearest: bool) {
        let requested: Element = match col {
            "date" => self.date.clone().into(),
            "account" => self.account.input.clone().into(),
            "tax" => self.tax.clone().into(),
            "partner" => self.partner.input.clone().into(),
            "memo" => self.memo.clone().into(),
            "pct" => self
                .pct
                .as_ref()
                .map_or_else(|| self.check.clone().into(), |e| e.clone().into()),
            _ => self.check.clone().into(),
        };
        let rect = requested.get_bounding_client_rect();
        let target = if requested.has_attribute("disabled")
            || rect.width() == 0.
            || rect.height() == 0.
            || requested.closest("[hidden]").ok().flatten().is_some()
        {
            self.check.clone().into()
        } else {
            requested
        };
        focus(&target);
        if nearest {
            let options = web_sys::ScrollIntoViewOptions::new();
            options.set_block(web_sys::ScrollLogicalPosition::Nearest);
            self.tr
                .scroll_into_view_with_scroll_into_view_options(&options);
        }
        if let Some(input) = target.dyn_ref::<HtmlInputElement>()
            && input.type_() != "checkbox"
        {
            let () = input.select();
        }
    }
    pub fn set_open(&self, open: bool) {
        class(&self.tr, "is-folded", !open);
        attrs(
            &self.more,
            &[("aria-expanded", if open { "true" } else { "false" })],
        );
        if let Some(label) = self.more.first_element_child() {
            set_text(&label, if open { "閉じる" } else { "くわしく" });
        }
        self.paint_name();
    }
    pub fn set_error(&self, field: &str, text: &str) {
        let field = match field {
            "tax_code" => "tax",
            "pct" => "business_pct",
            "account" | "date" | "tax" | "partner" | "memo" | "business_pct" => field,
            _ => "row",
        };
        if text.is_empty() {
            self.errors.borrow_mut().remove(field);
        } else {
            self.errors.borrow_mut().insert(field.into(), text.into());
            if ["date", "tax", "partner", "memo", "business_pct"].contains(&field) {
                self.set_open(true);
            }
        }
        self.paint_messages();
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Register the native controls' related row handlers together."
    )]
    pub fn bind(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.account.on_select(move |item, via, _event| {
            if via == "input" {
                return false;
            }
            if let Some(r) = weak.upgrade() {
                r.choose(item.as_ref().map_or("", |item| item.value.as_str()));
                if via == "enter" && item.is_some() {
                    r.mark_next("account");
                    return true;
                }
            }
            false
        });
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.check, "change", move |_| {
            if let Some(r) = weak.upgrade() {
                r.set_candidate(r.check.checked());
            }
        });
        for (el, col) in [
            (self.account.input.clone().into(), "account"),
            (self.partner.input.clone().into(), "partner"),
            (self.memo.clone().into(), "memo"),
            (self.date.clone().into(), "date"),
        ]
        .into_iter()
        .chain(self.pct.as_ref().map(|p| (p.clone().into(), "pct")))
        {
            let weak = Rc::downgrade(self);
            self.ctx.on(&el, "blur", move |_| {
                if let Some(r) = weak.upgrade() {
                    r.commit(col);
                }
            });
        }
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.date, "focus", move |_| {
            if let Some(r) = weak.upgrade() {
                r.date.set_value(text(&r.draft.borrow(), "date"));
                let () = r.date.select();
            }
        });
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.date, "keydown", move |event| {
            let Ok(key) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(&key) || key.ctrl_key() || key.meta_key() || key.alt_key() {
                return;
            }
            let Some(row) = weak.upgrade() else {
                return;
            };
            let value = match key.key().as_str() {
                "t" | "T" => today(),
                "+" => add_days(text(&row.draft.borrow(), "date"), 1),
                "-" => add_days(text(&row.draft.borrow(), "date"), -1),
                _ => return,
            };
            key.prevent_default();
            row.date.set_value(&value);
            row.commit("date");
        });
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.tax, "change", move |_| {
            if let Some(r) = weak.upgrade() {
                r.touched.set(true);
                let v = r.tax.value();
                r.enqueue(
                    json!({"tax_code":if v.is_empty()||v=="__unset"{Value::Null}else{json!(v)}}),
                );
                r.paint();
            }
        });
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.tax, "keydown", move |e| {
            let Ok(k) = e.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(&k) || k.alt_key() || k.ctrl_key() || k.meta_key() || k.shift_key() {
                return;
            }
            let delta = match k.key().as_str() {
                "ArrowDown" => 1,
                "ArrowUp" => -1,
                _ => return,
            };
            k.prevent_default();
            if let Some(r) = weak.upgrade() {
                let options = r.tax.children();
                let selectable: Vec<_> = (0..options.length())
                    .map(|i| {
                        options.item(i).is_some_and(|o| {
                            !o.has_attribute("disabled") && !o.has_attribute("hidden")
                        })
                    })
                    .collect();
                if let Some(i) = step_option(&selectable, r.tax.selected_index(), delta) {
                    let Ok(i) = i32::try_from(i) else {
                        return;
                    };
                    r.tax.set_selected_index(i);
                    if let Ok(event) = Event::new("change") {
                        let _ = r.tax.dispatch_event(&event);
                    }
                }
            }
        });
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.more, "click", move |_| {
            if let Some(r) = weak.upgrade() {
                let open = r.more.get_attribute("aria-expanded").as_deref() == Some("true");
                r.set_open(!open);
            }
        });
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.ignore, "click", move |_| {
            if let Some(r) = weak.upgrade()
                && let Some(s) = r.parent.upgrade()
            {
                s.ignore_row(r);
            }
        });
    }
}
