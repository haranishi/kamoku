// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    controls::{
        AmountControl, DateControl, Note, combo_items, configure_combo, display_date, hidden,
        select_all_on_focus,
    },
    model::{
        NO_ACCOUNT, NO_WALLET, PCT_ERROR, TRANSITIONAL_NOTE, account_options, integer,
        is_transitional, is_wallet, locked_note, next_defaults, parse_pct, pct_source_note,
        tax_options, tax_source_note, text, wallet_label, wallet_options,
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

pub(crate) struct Simple {
    pub el: Element,
    pub date: Rc<DateControl>,
    pub amount: Rc<AmountControl>,
    ctx: Context,
    kind: RefCell<String>,
    radios: Vec<HtmlInputElement>,
    segment: Element,
    accounts: Vec<Value>,
    account: Rc<Combo>,
    account_field: Field,
    to_wallet: Rc<Combo>,
    to_wallet_field: Field,
    wallet: Rc<Combo>,
    wallet_field: Field,
    partner: Rc<Combo>,
    partner_field: Field,
    tax: HtmlSelectElement,
    tax_field: Field,
    pct: HtmlInputElement,
    pct_field: Field,
    description: HtmlInputElement,
    memo: RefCell<String>,
    exempt: bool,
    taxable: bool,
    detail_open: Cell<bool>,
    detail_cell: Element,
    detail_button: Element,
    lock_cell: Element,
    transitional: Note,
    pct_note: Note,
    tax_note: Note,
    partner_note: Note,
    own_draw: Element,
    previous_account: RefCell<Value>,
    last_pct: Cell<i64>,
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
impl Simple {
    #[allow(
        clippy::too_many_lines,
        reason = "Builds the labelled native form DOM and binds its feature state in one constructor."
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
        let kind = if text(initial, "kind").is_empty() {
            "expense"
        } else {
            text(initial, "kind")
        };
        let segment = h("fieldset", "seg", "");
        let options = h("div", "seg-options", "");
        append(
            &segment,
            &[h("legend", "field-label", "種別"), options.clone()],
        );
        let mut radios = Vec::new();
        let name = crate::app::core::dom::uid("seg");
        for (value, label, key) in [
            ("expense", "支出", "1"),
            ("income", "収入", "2"),
            ("transfer", "振替", "3"),
        ] {
            let radio = input("radio", value);
            radio.set_class_name("");
            radio.set_name(&name);
            radio.set_checked(value == kind);
            let check = icon("check");
            attrs(
                &check,
                &[
                    ("class", "icon seg-check"),
                    ("width", "16"),
                    ("height", "16"),
                ],
            );
            let kbd = h("kbd", "", key);
            attrs(&kbd, &[("aria-hidden", "true")]);
            let option = h("label", "seg-option", "");
            append(
                &option,
                &[radio.clone().into(), check, h("span", "", label), kbd],
            );
            append(&options, &[option]);
            radios.push(radio);
        }
        let date_value = if text(initial, "date").is_empty() {
            today()
        } else {
            text(initial, "date").into()
        };
        let date = DateControl::new(ctx, &date_value);
        let transitional = Note::new(date.input.as_ref());
        append(&date.field.el, std::slice::from_ref(&transitional.el));
        let amount = AmountControl::new(ctx, initial["amount"].as_i64());
        let account = Rc::new(Combo::new(
            ctx,
            combo_items(
                &account_options(accounts, kind, true, &[text(initial, "account").into()]),
                false,
            ),
            text(initial, "account"),
        ));
        configure_combo(&account, "科目の候補", "例: つうしん、通信費", NO_ACCOUNT);
        let account_field = field("科目", &account.el, "", true);
        let wallets = wallet_options(accounts);
        let to_wallet = Rc::new(Combo::new(
            ctx,
            combo_items(&wallets, false),
            text(initial, "to_wallet"),
        ));
        configure_combo(&to_wallet, "口座の候補", "", NO_WALLET);
        let to_wallet_field = field("移動先の口座", &to_wallet.el, "", true);
        let last = web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .and_then(|storage| storage.get_item("kamoku.entry.wallet").ok().flatten())
            .unwrap_or_default();
        let wallet_value = if initial.get("wallet").is_some_and(|v| !v.is_null()) {
            text(initial, "wallet").to_owned()
        } else {
            wallets
                .iter()
                .find(|wallet| wallet["code"] == last)
                .or_else(|| wallets.first())
                .map(|wallet| text(wallet, "code").to_owned())
                .unwrap_or_default()
        };
        let wallet = Rc::new(Combo::new(ctx, combo_items(&wallets, false), &wallet_value));
        configure_combo(&wallet, "口座の候補", "", NO_WALLET);
        let wallet_field = field(wallet_label(kind), &wallet.el, "", true);
        let tax = select(&tax_options(kind), text(initial, "tax_code"));
        let tax_note = Note::new(tax.as_ref());
        let tax_field = field("税区分", tax.as_ref(), "", false);
        attrs(
            &tax_field.el,
            &[("id", &crate::app::core::dom::uid("taxf"))],
        );
        append(&tax_field.el, std::slice::from_ref(&tax_note.el));
        let detail_button = button("詳細: 税区分を出す", "ghost");
        detail_button.set_class_name("btn btn--ghost btn--sm detail-toggle");
        attrs(
            &detail_button,
            &[
                ("aria-expanded", "false"),
                ("aria-controls", &tax_field.el.id()),
            ],
        );
        let detail_cell = h("div", "field detail-cell", "");
        append(
            &detail_cell,
            &[
                h("span", "field-label", "税区分"),
                detail_button.clone(),
                h(
                    "p",
                    "field-hint",
                    "免税事業者の設定なので、ふだんは隠しています。",
                ),
            ],
        );
        let lock_cell = h("div", "field detail-cell", "");
        append(
            &lock_cell,
            &[
                h("span", "field-label", "税区分と割合"),
                h(
                    "p",
                    "field-hint",
                    "相手が口座の取引（カードの引き落としなど）なので、税区分は対象外、割合は100%のままにします。",
                ),
            ],
        );
        let partner = Rc::new(Combo::new(
            ctx,
            combo_items(partners, true),
            text(initial, "partner"),
        ));
        partner.allow_free(true);
        partner.set_value(text(initial, "partner"), false);
        configure_combo(&partner, "取引先の候補", "例: サンプル通信", "");
        partner.create_label(|name| format!("「{name}」を新しい取引先として足す"));
        let partner_field = field(
            "取引先",
            &partner.el,
            "新しい名前は、一覧の最後の行で足せます。",
            false,
        );
        let partner_note = Note::new(partner.input.as_ref());
        append(&partner_field.el, std::slice::from_ref(&partner_note.el));
        let description = input("text", text(initial, "description"));
        attrs(
            description.as_ref(),
            &[
                ("placeholder", "例: 10月分 携帯電話"),
                ("enterkeyhint", "next"),
            ],
        );
        let desc_field = field("摘要", description.as_ref(), "", false);
        let _ = desc_field.el.class_list().add_1("span-2");
        let pct = input(
            "text",
            &initial["business_pct"].as_i64().unwrap_or(100).to_string(),
        );
        pct.set_class_name("input input--num pct-input");
        attrs(
            pct.as_ref(),
            &[("inputmode", "numeric"), ("enterkeyhint", "done")],
        );
        let own_draw = h("span", "pct-out", "");
        attrs(&own_draw, &[("aria-live", "polite")]);
        let affix = h("div", "affix", "");
        append(
            &affix,
            &[
                pct.clone().into(),
                h("span", "affix-text", "%"),
                own_draw.clone(),
            ],
        );
        let pct_field = field(
            "事業の割合",
            &affix,
            "仕事に使った分の割合。残りは事業主貸に入ります。",
            false,
        );
        let _ = pct_field.el.class_list().add_1("span-2");
        let pct_note = Note::new(pct.as_ref());
        append(&pct_field.el, std::slice::from_ref(&pct_note.el));
        let grid = h("div", "entry-grid", "");
        append(
            &grid,
            &[
                date.field.el.clone(),
                amount.field.el.clone(),
                account_field.el.clone(),
                to_wallet_field.el.clone(),
                wallet_field.el.clone(),
                tax_field.el.clone(),
                detail_cell.clone(),
                lock_cell.clone(),
                partner_field.el.clone(),
                desc_field.el,
                pct_field.el.clone(),
            ],
        );
        let el = h("form", "entry-form", "");
        attrs(
            &el,
            &[("novalidate", ""), ("aria-labelledby", "entry-title")],
        );
        append(&el, &[segment.clone(), grid]);
        let previous_account = accounts
            .iter()
            .find(|account| account["code"] == initial["account"])
            .cloned()
            .unwrap_or(Value::Null);
        let this = Rc::new(Self {
            el,
            date,
            amount,
            ctx: ctx.clone(),
            kind: RefCell::new(kind.into()),
            radios,
            segment,
            accounts: accounts.to_vec(),
            account,
            account_field,
            to_wallet,
            to_wallet_field,
            wallet,
            wallet_field,
            partner,
            partner_field,
            tax,
            tax_field,
            pct,
            pct_field,
            description,
            memo: RefCell::new(text(initial, "memo").into()),
            exempt: settings["tax_mode"] == "exempt",
            taxable: settings["tax_mode"] == "taxable",
            detail_open: Cell::new(
                settings["tax_mode"] == "exempt" && !text(initial, "tax_code").is_empty(),
            ),
            detail_cell,
            detail_button,
            lock_cell,
            transitional,
            pct_note,
            tax_note,
            partner_note,
            own_draw,
            previous_account: RefCell::new(previous_account),
            last_pct: Cell::new(initial["business_pct"].as_i64().unwrap_or(100)),
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
        this.wire();
        this.apply_kind();
        *this.baseline.borrow_mut() = this.fingerprint();
        this.changed();
        this
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Registers page-scoped input, selection and keyboard handlers for the native form."
    )]
    fn wire(self: &Rc<Self>) {
        for radio in &self.radios {
            let weak = Rc::downgrade(self);
            let radio = radio.clone();
            self.ctx.on(radio.clone().as_ref(), "change", move |_| {
                if radio.checked()
                    && let Some(this) = weak.upgrade()
                {
                    this.set_kind(&radio.value());
                }
            });
        }
        let weak = Rc::downgrade(self);
        self.date.on_change(move || {
            if let Some(this) = weak.upgrade() {
                let note = this.locked_note(false);
                if !this.date.value().is_empty() && !note.is_empty() {
                    this.date.field.set_error(&note);
                }
                this.changed();
            }
        });
        let weak = Rc::downgrade(self);
        self.amount.on_change(move || {
            if let Some(this) = weak.upgrade() {
                this.changed();
            }
        });
        let weak = Rc::downgrade(self);
        self.account.on_change(move |_, item, _| {
            if let Some(this) = weak.upgrade() {
                if item.is_some() {
                    this.account_field.set_error("");
                }
                let next = item.map_or(Value::Null, |item| item.meta);
                let (tax, pct) = next_defaults(
                    &this.previous_account.borrow(),
                    &next,
                    &this.tax.value(),
                    this.last_pct.get(),
                );
                *this.previous_account.borrow_mut() = next;
                this.tax.set_value(
                    if this.exempt && !this.detail_open.get()
                        || !tax_options(&this.kind())
                            .iter()
                            .any(|(code, _)| *code == tax)
                    {
                        ""
                    } else {
                        &tax
                    },
                );
                if this.kind() == "expense" {
                    this.pct.set_value(&pct.to_string());
                    this.last_pct.set(pct);
                    this.pct_field.set_error("");
                }
                this.apply_kind();
                this.changed();
            }
        });
        let weak = Rc::downgrade(self);
        self.account.on_select(move |_, via, _| {
            if via != "enter" {
                return false;
            }
            if let Some(this) = weak.upgrade() {
                this.focus_next(this.account.input.as_ref());
            }
            true
        });
        for (combo, which) in [
            (&self.wallet, "wallet"),
            (&self.to_wallet, "to_wallet"),
            (&self.partner, "partner"),
        ] {
            let weak = Rc::downgrade(self);
            combo.on_change(move |_, item, _| {
                if let Some(this) = weak.upgrade() {
                    if item.is_some() {
                        match which {
                            "wallet" => this.wallet_field.set_error(""),
                            "to_wallet" => this.to_wallet_field.set_error(""),
                            _ => this.partner_field.set_error(""),
                        }
                    }
                    this.changed();
                }
            });
            let weak = Rc::downgrade(self);
            combo.on_select(move |_, via, _| {
                if via != "enter" {
                    return false;
                }
                if let Some(this) = weak.upgrade() {
                    let control: &Element = match which {
                        "wallet" => this.wallet.input.as_ref(),
                        "to_wallet" => this.to_wallet.input.as_ref(),
                        _ => this.partner.input.as_ref(),
                    };
                    this.focus_next(control);
                }
                true
            });
        }
        for control in [
            self.description.clone().into(),
            self.pct.clone().into(),
            self.partner.input.clone().into(),
        ] {
            let weak = Rc::downgrade(self);
            self.ctx.on(&control, "input", move |_| {
                if let Some(this) = weak.upgrade() {
                    this.validate_pct(false);
                    this.changed();
                }
            });
        }
        select_all_on_focus(&self.ctx, &self.pct);
        let weak = Rc::downgrade(self);
        self.ctx.on(self.pct.as_ref(), "blur", move |_| {
            if let Some(this) = weak.upgrade() {
                this.validate_pct(true);
            }
        });
        let weak = Rc::downgrade(self);
        self.ctx.on(self.tax.as_ref(), "change", move |_| {
            if let Some(this) = weak.upgrade() {
                this.changed();
            }
        });
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.detail_button, "click", move |_| {
            if let Some(this) = weak.upgrade() {
                this.detail_open.set(true);
                let account = this.account_meta();
                let default = text(&account, "default_tax");
                if this.tax.value().is_empty()
                    && !is_wallet(&account)
                    && tax_options(&this.kind())
                        .iter()
                        .any(|(code, _)| *code == default)
                {
                    this.tax.set_value(default);
                }
                this.apply_kind();
                this.changed();
                focus(this.tax.as_ref());
            }
        });
        self.ctx
            .on(&self.el, "submit", |event| event.prevent_default());
        let weak = Rc::downgrade(self);
        self.ctx.on(&self.el, "keydown", move |event| {
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
                this.cancel();
                return;
            }
            if event.default_prevented() || event.alt_key() || event.ctrl_key() || event.meta_key()
            {
                return;
            }
            let target = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok());
            let Some(target) = target else {
                return;
            };
            if target.closest(".seg").ok().flatten().is_some() {
                let next = match event.key().as_str() {
                    "1" => Some("expense"),
                    "2" => Some("income"),
                    "3" => Some("transfer"),
                    _ => None,
                };
                if let Some(next) = next {
                    event.prevent_default();
                    this.set_kind(next);
                    this.focus_kind();
                    return;
                }
            }
            if event.key() == "Enter"
                && !event.shift_key()
                && target.tag_name() != "BUTTON"
                && target.tag_name() != "A"
            {
                event.prevent_default();
                this.focus_next(&target);
            }
        });
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
        let f = self.submit_callback.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }
    fn cancel(&self) {
        let f = self.cancel_callback.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }
    pub(crate) fn kind(&self) -> String {
        self.kind.borrow().clone()
    }
    fn account_meta(&self) -> Value {
        self.accounts
            .iter()
            .find(|account| account["code"] == self.account.value())
            .cloned()
            .unwrap_or(Value::Null)
    }
    fn is_locked(&self) -> bool {
        self.kind() != "transfer" && is_wallet(&self.account_meta())
    }
    pub(crate) fn set_kind(&self, kind: &str) {
        if kind == self.kind() {
            return;
        }
        *self.kind.borrow_mut() = kind.into();
        for radio in &self.radios {
            radio.set_checked(radio.value() == kind);
        }
        self.account.set_items(combo_items(
            &account_options(&self.accounts, kind, true, &[self.account.value()]),
            false,
        ));
        let choices = tax_options(kind);
        let old = self.tax.value();
        let account = self.account_meta();
        let default = text(&account, "default_tax");
        let next = if choices.iter().any(|(code, _)| *code == old) {
            old
        } else if (!self.exempt || self.detail_open.get())
            && choices.iter().any(|(code, _)| *code == default)
        {
            default.into()
        } else {
            String::new()
        };
        let opts: Vec<Element> = choices
            .iter()
            .map(|(value, label)| {
                let option = h("option", "", label);
                attrs(&option, &[("value", value)]);
                option
            })
            .collect();
        replace(self.tax.as_ref(), &opts);
        self.tax.set_value(&next);
        if kind != "expense" {
            self.pct.set_value("100");
            self.last_pct.set(100);
        }
        self.pct_field.set_error("");
        self.apply_kind();
        self.changed();
    }
    fn apply_kind(&self) {
        let kind = self.kind();
        let locked = self.is_locked();
        hidden(&self.account_field.el, kind == "transfer");
        hidden(&self.to_wallet_field.el, kind != "transfer");
        self.wallet_field.set_label(wallet_label(&kind));
        hidden(
            &self.tax_field.el,
            kind == "transfer" || locked || self.exempt && !self.detail_open.get(),
        );
        hidden(
            &self.detail_cell,
            kind == "transfer" || locked || !self.exempt || self.detail_open.get(),
        );
        hidden(&self.lock_cell, !locked);
        hidden(&self.pct_field.el, kind != "expense" || locked);
        attrs(
            &self.detail_button,
            &[(
                "aria-expanded",
                if self.detail_open.get() {
                    "true"
                } else {
                    "false"
                },
            )],
        );
        self.notes();
    }
    fn notes(&self) {
        let kind = self.kind();
        let account = self.account_meta();
        let expense = kind == "expense" && !self.is_locked();
        self.transitional
            .set(if expense && is_transitional(&self.tax.value()) {
                TRANSITIONAL_NOTE
            } else {
                ""
            });
        self.pct_note.set(&if expense {
            pct_source_note(&account)
        } else {
            String::new()
        });
        self.tax_note
            .set(if self.tax_field.el.has_attribute("hidden") {
                ""
            } else {
                tax_source_note(self.taxable, &kind, &account, &self.tax.value())
            });
        let item = self.partner.item();
        self.partner_note.set(
            &item
                .filter(|item| {
                    (item.meta["isNew"] == true || item.meta["is_new"] == true)
                        && self.partner.input.value().trim() == item.label
                })
                .map(|item| format!("新しい取引先: {}", item.label))
                .unwrap_or_default(),
        );
    }
    fn validate_pct(&self, empty: bool) -> bool {
        if let Some(pct) = parse_pct(&self.pct.value()) {
            self.last_pct.set(pct);
        }
        let problem = self.pct_problem();
        if empty || !self.pct.value().trim().is_empty() {
            self.pct_field.set_error(&problem);
        }
        problem.is_empty()
    }
    fn pct_problem(&self) -> String {
        if self.kind() == "expense" && !self.is_locked() && parse_pct(&self.pct.value()).is_none() {
            PCT_ERROR.into()
        } else {
            String::new()
        }
    }
    fn changed(&self) {
        if self.partner.input.value().trim().is_empty() {
            self.partner_field.set_error("");
        }
        self.notes();
        if self.active.get() {
            self.preview.schedule(self.snapshot(), &self.pct_problem());
        }
        let f = self.changed_callback.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }
    pub(crate) fn set_active(&self, active: bool) {
        self.active.set(active);
        if active {
            self.changed();
        }
    }
    pub(crate) fn paint_lines(&self, lines: &[Value]) {
        let own = lines
            .iter()
            .filter(|line| line["account"] == "owner_draw" && line["side"] == "debit")
            .map(|line| integer(line, "amount"))
            .sum::<i64>();
        set_text(
            &self.own_draw,
            &if self.kind() == "expense" && own > 0 {
                format!("→ 事業主貸 {}円", number(own))
            } else {
                String::new()
            },
        );
    }
    pub(crate) fn snapshot(&self) -> Option<Value> {
        let date = self.date.value();
        let amount = self.amount.value()?;
        let wallet = self.wallet.value();
        if date.is_empty() || wallet.is_empty() {
            return None;
        }
        let mut body = json!({"kind":self.kind(),"date":date,"amount":amount,"wallet":wallet,"partner":self.partner.value(),"description":self.description.value(),"memo":self.memo.borrow().clone()});
        if self.kind() == "transfer" {
            let destination = self.to_wallet.value();
            if destination.is_empty() {
                return None;
            }
            body["to_wallet"] = json!(destination);
        } else {
            let account = self.account.value();
            if account.is_empty() {
                return None;
            }
            body["account"] = json!(account);
            body["tax_code"] = if self.is_locked()
                || self.exempt && !self.detail_open.get()
                || self.tax.value().is_empty()
            {
                Value::Null
            } else {
                json!(self.tax.value())
            };
            body["business_pct"] = json!(if self.kind() == "expense" && !self.is_locked() {
                parse_pct(&self.pct.value())?
            } else {
                100
            });
        }
        Some(body)
    }
    pub(crate) fn collect(&self) -> Option<Value> {
        let mut problems: Vec<(Element, String)> = Vec::new();
        if !self.date.commit(false) {
            problems.push((self.date.input.clone().into(), "日付".into()));
        }
        if self.amount.input.value().trim().is_empty() {
            self.amount.field.set_error("金額を入れてください。");
            problems.push((self.amount.input.clone().into(), "金額".into()));
        } else if !self.amount.commit(false) {
            problems.push((self.amount.input.clone().into(), "金額".into()));
        }
        let mut pick = |combo: &Combo, field: &Field, label: &str, problem_label: &str| {
            field.set_error("");
            if combo.value().is_empty() {
                field.set_error(&if combo.input.value().trim().is_empty() {
                    format!("{label}を選んでください。")
                } else {
                    format!("打った文字に合う{label}がありません。候補から選んでください。")
                });
                problems.push((combo.input.clone().into(), problem_label.into()));
            }
        };
        if self.kind() == "transfer" {
            pick(
                &self.to_wallet,
                &self.to_wallet_field,
                "移動先の口座",
                "移動先の口座",
            );
        } else {
            pick(&self.account, &self.account_field, "科目", "科目");
        }
        pick(
            &self.wallet,
            &self.wallet_field,
            "口座",
            wallet_label(&self.kind()),
        );
        if self.kind() == "transfer"
            && !self.wallet.value().is_empty()
            && self.wallet.value() == self.to_wallet.value()
        {
            self.to_wallet_field
                .set_error("移動元と移動先が同じ口座です。別の口座を選んでください。");
            problems.push((self.to_wallet.input.clone().into(), "移動先の口座".into()));
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
            problems.push((self.partner.input.clone().into(), "取引先".into()));
        }
        if !self.validate_pct(true) {
            problems.push((self.pct.clone().into(), "事業の割合".into()));
        }
        *self.problems.borrow_mut() = problems.iter().map(|(_, name)| name.clone()).collect();
        if let Some((first, _)) = problems.first() {
            focus(first);
            return None;
        }
        self.snapshot()
    }
    pub(crate) fn problems(&self) -> Vec<String> {
        self.problems.borrow().clone()
    }
    fn fingerprint(&self) -> String {
        let date = self.date.value();
        let raw_date = self.date.input.value();
        let raw_date = if raw_date.trim() == date || raw_date.trim() == display_date(&date) {
            ""
        } else {
            raw_date.trim()
        };
        json!([
            self.kind(),
            date,
            raw_date,
            self.amount.value(),
            if self.amount.value().is_none() {
                self.amount.input.value()
            } else {
                String::new()
            },
            self.account.value(),
            self.account.input.value(),
            self.wallet.value(),
            self.wallet.input.value(),
            self.to_wallet.value(),
            self.to_wallet.input.value(),
            self.tax.value(),
            self.partner.input.value(),
            self.description.value(),
            self.pct.value(),
            self.memo.borrow().clone()
        ])
        .to_string()
    }
    pub(crate) fn dirty(&self) -> bool {
        self.fingerprint() != *self.baseline.borrow()
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
        let found: Option<(&Field, Element)> = match name {
            "date" => Some((&self.date.field, self.date.input.clone().into())),
            "amount" => Some((&self.amount.field, self.amount.input.clone().into())),
            "account" => Some((&self.account_field, self.account.input.clone().into())),
            "wallet" => Some((&self.wallet_field, self.wallet.input.clone().into())),
            "to_wallet" => Some((&self.to_wallet_field, self.to_wallet.input.clone().into())),
            "tax_code" => Some((&self.tax_field, self.tax.clone().into())),
            "partner" => Some((&self.partner_field, self.partner.input.clone().into())),
            "business_pct" => Some((&self.pct_field, self.pct.clone().into())),
            _ => None,
        };
        if let Some((field, control)) = found {
            field.set_error(message);
            focus(&control);
            true
        } else {
            false
        }
    }
    fn order(&self) -> Vec<Element> {
        let mut controls = vec![
            self.segment.clone(),
            self.date.input.clone().into(),
            self.amount.input.clone().into(),
        ];
        controls.push(if self.kind() == "transfer" {
            self.to_wallet.input.clone().into()
        } else {
            self.account.input.clone().into()
        });
        controls.push(self.wallet.input.clone().into());
        if !self.tax_field.el.has_attribute("hidden") {
            controls.push(self.tax.clone().into());
        }
        controls.push(self.partner.input.clone().into());
        controls.push(self.description.clone().into());
        if !self.pct_field.el.has_attribute("hidden") {
            controls.push(self.pct.clone().into());
        }
        controls
    }
    fn focus_next(&self, target: &Element) {
        let controls = self.order();
        if let Some(index) = controls
            .iter()
            .position(|control| control == target || control.contains(Some(target)))
        {
            if let Some(next) = controls.get(index + 1) {
                focus(next);
            } else {
                self.submit();
            }
        }
    }
    pub(crate) fn focus_kind(&self) {
        if let Some(radio) = self.radios.iter().find(|radio| radio.checked()) {
            focus(radio.as_ref());
        }
    }
    pub(crate) fn apply_template(&self, body: &Value) {
        if !text(body, "kind").is_empty() {
            self.set_kind(text(body, "kind"));
        }
        if let Some(amount) = body["amount"].as_i64() {
            self.amount.set_value(Some(amount));
        }
        self.account.set_value(text(body, "account"), false);
        self.to_wallet.set_value(text(body, "to_wallet"), false);
        if !text(body, "wallet").is_empty() {
            self.wallet.set_value(text(body, "wallet"), false);
        }
        self.tax.set_value(text(body, "tax_code"));
        self.pct
            .set_value(&body["business_pct"].as_i64().unwrap_or(100).to_string());
        self.last_pct
            .set(body["business_pct"].as_i64().unwrap_or(100));
        self.partner.set_value(text(body, "partner"), false);
        self.description.set_value(text(body, "description"));
        *self.memo.borrow_mut() = text(body, "memo").into();
        *self.previous_account.borrow_mut() = self.account_meta();
        if self.exempt && !text(body, "tax_code").is_empty() {
            self.detail_open.set(true);
        }
        self.apply_kind();
        self.changed();
    }
    pub(crate) fn reset(&self) {
        self.amount.set_value(None);
        self.account.set_value("", false);
        self.to_wallet.set_value("", false);
        self.tax.set_value("");
        self.partner.set_value("", false);
        self.description.set_value("");
        self.pct.set_value("100");
        self.last_pct.set(100);
        self.memo.borrow_mut().clear();
        *self.previous_account.borrow_mut() = Value::Null;
        for field in [
            &self.account_field,
            &self.to_wallet_field,
            &self.wallet_field,
            &self.tax_field,
            &self.partner_field,
            &self.pct_field,
        ] {
            field.set_error("");
        }
        self.apply_kind();
        self.changed();
        *self.baseline.borrow_mut() = self.fingerprint();
        if self.active.get() {
            self.preview.run();
        }
    }
    pub(crate) fn journal_seed(&self) -> Value {
        json!({"date":self.date.value(),"description":self.description.value(),"partner":self.partner.value(),"lines":self.preview.lines()})
    }
}
