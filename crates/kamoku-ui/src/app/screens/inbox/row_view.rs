// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::model::{tax_choice_label, tax_diff_text};
use super::{
    BTreeMap, BTreeSet, Cell, Combo, ComboItem, Context, Element, JsValue, Rc, RefCell, Row, State,
    Value, VecDeque, append, attrs, button, class, date_label, direction_warning, disabled, h,
    hidden, icon, id, input, is_wallet, node, replace, row_label, select, set_text, signed,
    tax_codes, tax_differs, tax_label, tax_touched, text, unset_tax, year,
};
use crate::app::prelude::phrase;
pub(super) fn short_date(iso: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(&format!("{iso}T12:00:00Z")));
    let days = ["日", "月", "火", "水", "木", "金", "土"];
    format!(
        "{} ({})",
        date_label(iso, &year()),
        days.get(date.get_utc_day() as usize).unwrap_or(&"")
    )
}
fn labelled(el: &Element, col: &str, id: u64, read_only: bool) {
    attrs(
        el,
        &[
            (
                "aria-labelledby",
                &format!("inbox-col-{col} draft-{id}-name"),
            ),
            ("aria-describedby", &format!("draft-{id}-msg")),
        ],
    );
    disabled(el, read_only);
}
fn combo_items(accounts: &[Value], income: bool, keep: &str) -> Vec<ComboItem> {
    let hidden: BTreeSet<_> = accounts
        .iter()
        .filter(|a| a["enabled"] != false && is_wallet(a) && text(&a["wallet"], "type") != "card")
        .map(|a| text(a, "parent"))
        .collect();
    let mut values: Vec<_> = accounts
        .iter()
        .filter(|a| {
            a["enabled"] != false && (!hidden.contains(text(a, "code")) || text(a, "code") == keep)
        })
        .collect();
    let rank = |a: &Value| {
        let kind = text(a, "kind");
        if kind == if income { "revenue" } else { "expense" } {
            0
        } else if text(a, "code") == if income { "owner_inv" } else { "owner_draw" } {
            1
        } else if is_wallet(a) {
            3
        } else if kind == if income { "expense" } else { "revenue" } {
            4
        } else {
            2
        }
    };
    values.sort_by_key(|a| (rank(a), a["sort"].as_i64().unwrap_or(0)));
    values
        .into_iter()
        .map(|a| ComboItem {
            value: text(a, "code").into(),
            label: text(a, "name").into(),
            reading: text(a, "reading").into(),
            meta: a.clone(),
        })
        .collect()
}
impl Row {
    #[allow(
        clippy::too_many_lines,
        reason = "Assemble a row's native controls and matching labelled cells in DOM order."
    )]
    pub fn new(s: &Rc<State>, parent_ctx: &Context, draft: Value) -> Rc<Self> {
        let ctx = parent_ctx.child();
        let key = id(&draft);
        let ro = s.ctx.is_read_only();
        let income = text(&draft, "direction") == "in";
        let account_value = s.account(text(&draft, "account"));
        let name = h("span", "visually-hidden", "");
        attrs(&name, &[("id", &format!("draft-{key}-name"))]);
        let check = input("checkbox", "");
        check.set_class_name("cand-check");
        let date = input("text", &short_date(text(&draft, "date")));
        date.set_class_name("input input--date");
        let sum_date = h("span", "sum-date", &short_date(text(&draft, "date")));
        let account = Rc::new(Combo::new(
            &ctx,
            combo_items(&s.accounts.borrow(), income, text(&draft, "account")),
            text(&draft, "account"),
        ));
        account.arrow_opens(false);
        account.set_list_label("科目の候補");
        account
            .set_no_match("合う科目がありません。漢字・ひらがなの読み・コードで探してください。");
        attrs(
            &account.input,
            &[("placeholder", "科目（必須）"), ("aria-required", "true")],
        );
        let tax = select(&[], "");
        let placeholder = h("option", "", "科目で決まる");
        attrs(
            &placeholder,
            &[("value", "__unset"), ("disabled", ""), ("hidden", "")],
        );
        append(&tax, &[placeholder]);
        for code in tax_codes(income) {
            let o = h("option", "", tax_choice_label(code));
            attrs(&o, &[("value", code)]);
            append(&tax, &[o]);
        }
        let partner = Rc::new(Combo::new(
            &ctx,
            s.partners
                .borrow()
                .iter()
                .filter(|p| p["enabled"] != false)
                .map(|p| ComboItem {
                    value: text(p, "name").into(),
                    label: text(p, "name").into(),
                    reading: text(p, "reading").into(),
                    meta: p.clone(),
                })
                .collect(),
            text(&draft, "partner"),
        ));
        partner.allow_free(true);
        partner.set_value(text(&draft, "partner"), false);
        partner.arrow_opens(false);
        partner.set_list_label("取引先の候補");
        attrs(&partner.input, &[("placeholder", "取引先")]);
        let memo = input("text", text(&draft, "memo"));
        attrs(&memo, &[("placeholder", "メモ"), ("maxlength", "200")]);
        let pct = (!income).then(|| {
            let p = input(
                "text",
                &draft["business_pct"].as_u64().unwrap_or(100).to_string(),
            );
            p.set_class_name("input input--num pct-cell");
            attrs(&p, &[("inputmode", "numeric"), ("maxlength", "3")]);
            p
        });
        for (el, col) in [
            (check.clone().into(), "cand"),
            (date.clone().into(), "date"),
            (account.input.clone().into(), "account"),
            (tax.clone().into(), "tax"),
            (partner.input.clone().into(), "partner"),
            (memo.clone().into(), "memo"),
        ]
        .into_iter()
        .chain(pct.as_ref().map(|p| (p.clone().into(), "pct")))
        {
            labelled(&el, col, key, ro);
        }
        if let Some(p) = &pct {
            attrs(
                p,
                &[(
                    "aria-describedby",
                    &format!("draft-{key}-msg inbox-pct-note"),
                )],
            );
        }
        let more = button("くわしく", "ghost");
        more.set_class_name("btn btn--ghost btn--sm more-btn");
        attrs(
            &more,
            &[
                ("aria-expanded", "false"),
                (
                    "aria-controls",
                    &["date", "tax", "partner", "memo", "pct"]
                        .map(|c| format!("draft-{key}-cell-{c}"))
                        .join(" "),
                ),
            ],
        );
        append(&more, &[icon("chevron-down")]);
        let ignore = button("無視", "ghost");
        ignore.set_class_name("btn btn--ghost btn--sm ignore-btn");
        attrs(
            &ignore,
            &[
                ("title", "x キーでも無視できます"),
                ("aria-keyshortcuts", "x"),
            ],
        );
        disabled(&ignore, ro);
        append(&ignore, &[icon("ban")]);
        let tr = h("tr", "inbox-row is-folded", "");
        attrs(
            &tr,
            &[
                ("data-id", &key.to_string()),
                ("data-dir", text(&draft, "direction")),
                ("data-order", "table"),
            ],
        );
        let cell = |col: &str, label: &str, children: &[Element]| {
            let td = node("td", &format!("c-{col}"), children);
            attrs(
                &td,
                &[
                    ("id", &format!("draft-{key}-cell-{col}")),
                    ("data-col", col),
                    ("data-label", label),
                ],
            );
            td
        };
        let badge = h("span", "cand-badge", "確定する");
        attrs(&badge, &[("aria-hidden", "true")]);
        let desc = h(
            "span",
            "desc-text",
            if text(&draft, "description").is_empty() {
                "（摘要なし）"
            } else {
                text(&draft, "description")
            },
        );
        attrs(&desc, &[("id", &format!("draft-{key}-desc"))]);
        let amount = h(
            "span",
            &format!("amount amount--{}", text(&draft, "direction")),
            &signed(&draft),
        );
        append(&amount, &[h("span", "amount-unit", "円")]);
        append(
            &tr,
            &[
                cell(
                    "cand",
                    "確定する",
                    &[node(
                        "label",
                        "cand",
                        &[check.clone().into(), h("span", "cand-text", "確定する")],
                    )],
                ),
                cell("date", "日付", &[date.clone().into()]),
                cell(
                    "desc",
                    "",
                    &[
                        desc,
                        node(
                            "span",
                            "sub",
                            &[
                                badge,
                                sum_date.clone(),
                                h("span", "keep", text(&draft, "wallet_name")),
                            ],
                        ),
                        name.clone(),
                    ],
                ),
                cell(
                    "amount",
                    "金額",
                    &[
                        amount,
                        h("span", "sub dir-text", if income { "入金" } else { "出金" }),
                    ],
                ),
                cell("account", "科目", std::slice::from_ref(&account.el)),
                cell("more", "", std::slice::from_ref(&more)),
                cell("tax", "税区分", &[tax.clone().into()]),
                cell("partner", "取引先", std::slice::from_ref(&partner.el)),
                cell("memo", "メモ", &[memo.clone().into()]),
                cell(
                    "pct",
                    "事業の割合",
                    &[if let Some(p) = &pct {
                        node(
                            "div",
                            "affix",
                            &[p.clone().into(), h("span", "affix-text", "%")],
                        )
                    } else {
                        h("span", "empty-value", "—")
                    }],
                ),
                cell("ops", "", std::slice::from_ref(&ignore)),
            ],
        );
        let msg = h("div", "row-msg", "");
        attrs(
            &msg,
            &[("id", &format!("draft-{key}-msg")), ("aria-live", "polite")],
        );
        let td = node("td", "", std::slice::from_ref(&msg));
        attrs(&td, &[("colspan", "10")]);
        let msg_tr = node("tr", "inbox-msg", &[td]);
        hidden(&msg_tr, true);
        let touched = tax_touched(
            &account_value,
            income,
            s.exempt.get(),
            text(&draft, "tax_code"),
        );
        let r = Rc::new(Self {
            parent: Rc::downgrade(s),
            ctx,
            tr,
            msg_tr,
            msg,
            draft: RefCell::new(draft),
            check,
            date,
            sum_date,
            account,
            tax,
            partner,
            memo,
            pct,
            more,
            ignore,
            name,
            touched: Cell::new(touched),
            errors: RefCell::new(BTreeMap::new()),
            leaving: Cell::new(false),
            queue: RefCell::new(VecDeque::new()),
            saving: Cell::new(false),
        });
        r.bind();
        r.paint();
        r
    }
    pub fn paint_name(&self) {
        let d = self.draft.borrow();
        let label = row_label(&d, &year());
        set_text(&self.name, &label);
        set_text(&self.sum_date, &short_date(text(&d, "date")));
        attrs(
            &self.ignore,
            &[("aria-label", &format!("無視する: {label}"))],
        );
        attrs(
            &self.more,
            &[(
                "aria-label",
                &format!(
                    "{}: {label}",
                    if self.more.get_attribute("aria-expanded").as_deref() == Some("true") {
                        "閉じる"
                    } else {
                        "くわしく"
                    }
                ),
            )],
        );
    }
    pub fn paint(&self) {
        let Some(s) = self.parent.upgrade() else {
            return;
        };
        let d = self.draft.borrow();
        let a = s.account(text(&d, "account"));
        let income = text(&d, "direction") == "in";
        let lock = is_wallet(&a) || s.ctx.is_read_only();
        let unset = !self.touched.get() && text(&d, "tax_code").is_empty() && !lock;
        let (label, title) = unset_tax(&a, !text(&d, "account").is_empty(), income);
        if let Some(o) = self.tax.first_element_child() {
            set_text(&o, &label);
        }
        self.tax.set_value(if unset {
            "__unset"
        } else {
            text(&d, "tax_code")
        });
        self.tax.set_title(if unset {
            &title
        } else {
            tax_label(text(&d, "tax_code"))
        });
        class(&self.tax, "is-placeholder", unset);
        self.tax.set_disabled(lock);
        if let Some(p) = &self.pct {
            p.set_disabled(lock);
        }
        class(&self.tr, "is-unset", text(&d, "account").is_empty());
        let on = s.selection.borrow().contains(id(&d));
        self.check.set_checked(on);
        class(&self.tr, "is-candidate", on);
        drop(d);
        self.paint_name();
        self.paint_messages();
    }
    pub fn paint_messages(&self) {
        let Some(s) = self.parent.upgrade() else {
            return;
        };
        let d = self.draft.borrow();
        let account = s.account(text(&d, "account"));
        let income = text(&d, "direction") == "in";
        let mut parts = vec![];
        for (field, text) in self.errors.borrow().iter() {
            let error = message("field-error", field, text);
            attrs(&error, &[("data-field", field)]);
            parts.push(error);
        }
        let warning = direction_warning(&account, income);
        if !warning.is_empty() {
            parts.push(note("warn", "account", warning));
        }
        if tax_differs(&account, income, s.exempt.get(), text(&d, "tax_code")) {
            parts.push(note(
                "info",
                "tax",
                &tax_diff_text(&account, income, s.exempt.get()),
            ));
        }
        if !s.exempt.get() && ["P10X", "P8X"].contains(&text(&d, "tax_code")) {
            parts.push(note("info","date","日付は、課税仕入れの日にしてください。商品の引渡し・役務の提供が完了した日です。経過措置の割合は、この行の日付で決まります。"));
        }
        hidden(&self.msg_tr, parts.is_empty());
        class(&self.tr, "has-msg", !parts.is_empty());
        replace(&self.msg, &parts);
    }
}
fn message(class_name: &str, field: &str, text: &str) -> Element {
    let p = node("p", class_name, &[icon("alert"), h("span", "", text)]);
    attrs(
        &p,
        &[
            ("data-for", field),
            (
                "data-side",
                if ["tax", "partner", "memo", "business_pct"].contains(&field) {
                    "right"
                } else {
                    "left"
                },
            ),
        ],
    );
    p
}
fn note(kind: &str, field: &str, text: &str) -> Element {
    let p = message(&format!("field-hint row-note row-note--{kind}"), field, "");
    attrs(&p, &[("data-note", kind)]);
    let cut = text.find('。').map_or(text.len(), |i| i + '。'.len_utf8());
    replace(
        &p,
        &[
            icon(if kind == "warn" { "alert" } else { "info" }),
            node(
                "span",
                "note-body",
                &[
                    node("span", "note-head", &[phrase(&text[..cut])]),
                    node("span", "note-detail", &[phrase(&text[cut..])]),
                ],
            ),
        ],
    );
    p
}
