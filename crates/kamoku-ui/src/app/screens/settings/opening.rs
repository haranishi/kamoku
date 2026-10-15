// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::super::home::common::{
    box_node, cell, el, fact, icon_button, list, n, s, table, text_input, yes,
};
use super::super::reports::tree::chart_key;
use super::model::read_balance;
use crate::app::prelude::*;
#[derive(Clone)]
struct Row {
    account: Value,
    input: HtmlInputElement,
    node: Element,
}
#[derive(Clone)]
pub(crate) struct OpeningForm {
    pub el: Element,
    date: HtmlInputElement,
    date_field: Field,
    committed_date: Rc<RefCell<Option<String>>>,
    rows: Rc<RefCell<Vec<Row>>>,
    body: Element,
    totals: Element,
    error: Element,
    ctx: Context,
}
impl OpeningForm {
    #[allow(
        clippy::too_many_lines,
        reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
    )]
    pub(crate) fn new(
        ctx: &Context,
        accounts: &[Value],
        opening: &Value,
        default_date: &str,
        on_submit: Option<Rc<dyn Fn()>>,
    ) -> Self {
        let date = text_input(
            if s(opening, "date").is_empty() {
                default_date
            } else {
                s(opening, "date")
            },
            "期首の日付",
        );
        let date_field = field(
            "期首の日付",
            &el(&date),
            "期首の日付は、残高を入れる日です。ふつうは開業日と同じです。開業日と同じ年の日付にしてください。",
            true,
        );
        let _ = date_field.el.class_list().add_1("opening-date");
        let (wrap, body) = table(
            "科目ごとの期首の残高",
            &["科目", "残高", "操作"],
            "opening-table",
        );
        let _ = wrap.class_list().add_1("opening-wrap");
        if let Ok(heads) = wrap.query_selector_all("thead th") {
            if let Some(th) = heads.item(1).and_then(|n| n.dyn_into::<Element>().ok()) {
                let _ = th.class_list().add_1("num");
            }
            if let Some(th) = heads.item(2).and_then(|n| n.dyn_into::<Element>().ok()) {
                replace(&th, &[h("span", "visually-hidden", "操作")]);
            }
        }
        let totals = h("dl", "opening-totals", "");
        attrs(&totals, &[("aria-live", "polite")]);
        let error = h("p", "field-error opening-err", "");
        attrs(&error, &[("role", "alert"), ("id", &uid("opening-err"))]);
        let node = box_node("div", "opening-form stack", &[date_field.el.clone(), wrap]);
        let form = Self {
            el: node,
            date: date.clone(),
            date_field: date_field.clone(),
            committed_date: Rc::new(RefCell::new(parse_date(&date.value(), &today()).ok())),
            rows: Rc::new(RefCell::new(vec![])),
            body,
            totals: totals.clone(),
            error: error.clone(),
            ctx: ctx.clone(),
        };
        let mut usable: Vec<_> = accounts
            .iter()
            .filter(|a| {
                yes(a, "enabled")
                    && matches!(s(a, "kind"), "asset" | "liability")
                    && !matches!(s(a, "code"), "owner_draw" | "owner_inv")
            })
            .cloned()
            .collect();
        usable.sort_by_key(chart_key);
        let balances = list(&opening["balances"]);
        for a in &usable {
            let given = balances.iter().find(|b| s(b, "account") == s(a, "code"));
            if a["wallet"].is_object() || given.is_some() {
                form.add_row(a, given.map_or(0, |b| n(b, "amount")));
            }
        }
        let items = usable
            .iter()
            .filter(|a| !a["wallet"].is_object())
            .map(|a| ComboItem {
                value: s(a, "code").into(),
                label: s(a, "name").into(),
                reading: s(a, "reading").into(),
                meta: json!({"hint":if s(a,"kind")=="asset"{"資産"}else{"負債"}}),
            })
            .collect();
        let combo = Combo::new(ctx, items, "");
        combo.set_list_label("科目の候補");
        combo.input.set_placeholder("例: うりかけきん（売掛金）");
        let copy = form.clone();
        let c = combo.clone();
        combo.on_select(move |item, _, _| {
            if let Some(item) = item
                && let Some(a) = usable.iter().find(|a| s(a, "code") == item.value)
            {
                let input = copy.add_row(a, 0);
                c.set_value("", false);
                if let Some(input) = input {
                    let ctx = copy.ctx.clone();
                    let check = ctx.clone();
                    ctx.task(async move {
                        let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(
                            &JsValue::NULL,
                        ))
                        .await;
                        if check.alive() {
                            let _ = input.focus();
                        }
                        Ok(())
                    });
                }
                copy.paint_totals();
                return true;
            }
            false
        });
        let other_field = field(
            "口座でない科目を足す（売掛金・借入金など）",
            &combo.el,
            "",
            false,
        );
        let _ = other_field.el.class_list().add_1("opening-others");
        append(
            &form.el,
            &[
                other_field.el,
                totals,
                error,
                box_node(
                    "p",
                    "small muted opening-hint",
                    &[
                        icon("info"),
                        h(
                            "span",
                            "",
                            "分からなければ、0のままで進めてかまいません。元入金は、資産と負債の差額で自動で決まります。",
                        ),
                    ],
                ),
            ],
        );
        let f = form.clone();
        ctx.on(&el(&date), "blur", move |_| {
            f.commit_date();
        });
        let field = date_field;
        ctx.on(&el(&date), "input", move |_| field.set_error(""));
        let f = form.clone();
        ctx.on(&form.el, "keydown", move |event| {
            let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(key)
                || key.key() != "Enter"
                || key.default_prevented()
                || key.shift_key()
                || key.alt_key()
                || key.ctrl_key()
                || key.meta_key()
            {
                return;
            }
            let Some(target) = key.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
                return;
            };
            if matches!(target.tag_name().as_str(), "BUTTON" | "A") {
                return;
            }
            let mut order = vec![el(&f.date)];
            order.extend(f.rows.borrow().iter().map(|r| el(&r.input)));
            if let Some(i) = order.iter().position(|n| n == &target) {
                if let Some(next) = order.get(i + 1) {
                    key.prevent_default();
                    focus(next);
                } else if let Some(submit) = &on_submit {
                    key.prevent_default();
                    submit();
                }
            }
        });
        form.paint_totals();
        form
    }
    fn commit_date(&self) -> bool {
        match parse_date(&self.date.value(), &today()) {
            Ok(value) => {
                self.date.set_value(&value);
                // Keep the server error when an unchanged date loses focus, so the
                // submit button cannot move between pointer-down and pointer-up.
                let mut committed = self.committed_date.borrow_mut();
                if committed.as_deref() != Some(value.as_str()) {
                    self.date_field.set_error("");
                    *committed = Some(value);
                }
                true
            }
            Err(error) => {
                *self.committed_date.borrow_mut() = None;
                self.date_field.set_error(&error);
                false
            }
        }
    }
    fn row_error(&self, row: &Row, message: &str) {
        set_text(&self.error, message);
        if message.is_empty() {
            let _ = el(&row.input).remove_attribute("aria-invalid");
            let _ = el(&row.input).remove_attribute("aria-describedby");
        } else {
            attrs(
                &el(&row.input),
                &[
                    ("aria-invalid", "true"),
                    ("aria-describedby", &self.error.id()),
                ],
            );
        }
    }
    fn add_row(&self, account: &Value, value: i64) -> Option<HtmlInputElement> {
        if self
            .rows
            .borrow()
            .iter()
            .any(|r| s(&r.account, "code") == s(account, "code"))
        {
            return None;
        }
        let formatted = if value == 0 {
            String::new()
        } else {
            number(value)
        };
        let control = text_input(&formatted, &format!("{}の残高", s(account, "name")));
        attrs(
            &el(&control),
            &[
                ("class", "input input--num"),
                ("inputmode", "decimal"),
                ("autocomplete", "off"),
                ("placeholder", "0"),
            ],
        );
        let tr = h("tr", "", "");
        let label = box_node(
            "th",
            "",
            &[
                h("span", "opening-name keep", s(account, "name")),
                h(
                    "span",
                    "sub",
                    if s(account, "kind") == "asset" {
                        "資産"
                    } else if s(&account["wallet"], "type") == "card" {
                        "負債・カードの未払い額"
                    } else {
                        "負債"
                    },
                ),
            ],
        );
        attrs(&label, &[("scope", "row")]);
        append(&tr, &[label]);
        cell(
            &tr,
            "残高",
            box_node(
                "div",
                "affix",
                &[el(&control), h("span", "affix-text", "円")],
            ),
            "num",
        );
        let ops = cell(&tr, "操作", h("span", "", ""), "opening-ops");
        let row = Row {
            account: account.clone(),
            input: control.clone(),
            node: tr.clone(),
        };
        self.rows.borrow_mut().push(row.clone());
        append(&self.body, &[tr]);
        if !account["wallet"].is_object() {
            let remove = icon_button(&format!("{}の行を外す", s(account, "name")), "trash", "");
            replace(&ops, std::slice::from_ref(&remove));
            let f = self.clone();
            let code = s(account, "code").to_owned();
            self.ctx.on(&remove, "click", move |_| {
                let mut rows = f.rows.borrow_mut();
                if let Some(at) = rows.iter().position(|r| s(&r.account, "code") == code) {
                    rows.remove(at).node.remove();
                }
                drop(rows);
                f.paint_totals();
            });
        }
        let f = self.clone();
        let r = row.clone();
        self.ctx.on(&el(&control), "input", move |_| {
            f.row_error(&r, "");
            f.paint_totals();
        });
        let f = self.clone();
        self.ctx.on(&el(&control), "blur", move |_| {
            match read_balance(&row.input.value()) {
                Ok(v) => {
                    row.input
                        .set_value(&if v == 0 { String::new() } else { number(v) });
                    f.row_error(&row, "");
                }
                Err(e) => f.row_error(&row, &format!("{}: {e}", s(&row.account, "name"))),
            }
            f.paint_totals();
        });
        Some(control)
    }
    fn paint_totals(&self) {
        let mut assets = 0i64;
        let mut liabilities = 0i64;
        for row in self.rows.borrow().iter() {
            if let Ok(value) = read_balance(&row.input.value()) {
                let slot = if s(&row.account, "kind") == "asset" {
                    &mut assets
                } else {
                    &mut liabilities
                };
                let Some(total) = slot.checked_add(value) else {
                    set_text(
                        &self.error,
                        "残高の合計が大きすぎます。金額を確かめてください。",
                    );
                    return;
                };
                *slot = total;
            }
        }
        let Some(capital) = assets.checked_sub(liabilities) else {
            set_text(&self.error, "残高の差額が大きすぎます。");
            return;
        };
        let cap = fact("元入金（資産−負債）", capital);
        let _ = cap.class_list().add_1("opening-capital");
        if capital < 0
            && let Some(dd) = cap.last_element_child()
        {
            append(&dd, &[h("span", "badge badge--warn", "債務超過")]);
        }
        replace(
            &self.totals,
            &[
                fact("資産の合計", assets),
                fact("負債の合計", liabilities),
                cap,
            ],
        );
    }
    pub(crate) fn collect(&self) -> Option<Value> {
        set_text(&self.error, "");
        if !self.commit_date() {
            let _ = self.date.focus();
            return None;
        }
        let mut balances = vec![];
        let mut total = 0i64;
        for row in self.rows.borrow().iter() {
            match read_balance(&row.input.value()) {
                Ok(value) => {
                    let Some(next) = total.checked_add(value) else {
                        self.row_error(row, "残高の合計が大きすぎます。");
                        let _ = row.input.focus();
                        return None;
                    };
                    total = next;
                    balances.push(json!({"account":s(&row.account,"code"),"amount":value}));
                }
                Err(e) => {
                    self.row_error(row, &format!("{}: {e}", s(&row.account, "name")));
                    let _ = row.input.focus();
                    return None;
                }
            }
        }
        Some(json!({"date":self.date.value(),"balances":balances}))
    }
    pub(crate) fn show_error(&self, error: &ApiError) -> bool {
        if error.field.as_deref() == Some("date") {
            self.date_field.set_error(&error.message);
            let _ = self.date.focus();
            return true;
        }
        if let Some(index) = error
            .field
            .as_deref()
            .and_then(|v| v.strip_prefix("balances["))
            .and_then(|v| v.split(']').next())
            .and_then(|v| v.parse::<usize>().ok())
            && let Some(row) = self.rows.borrow().get(index)
        {
            self.row_error(
                row,
                &format!("{}: {}", s(&row.account, "name"), error.message),
            );
            let _ = row.input.focus();
            return true;
        }
        false
    }
}
