// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::model::Filters;
use crate::app::prelude::*;
use crate::app::screens::entry::{
    controls::{DateControl, combo_items, configure_combo, hidden},
    model::{NO_ACCOUNT, account_options, text, wallet_options},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlInputElement, HtmlSelectElement, KeyboardEvent};

pub(crate) struct FilterUi {
    pub el: Element,
    current: RefCell<Filters>,
    search: HtmlInputElement,
    label: Element,
    prev: Element,
    next: Element,
    all: Element,
    month: Element,
    more: Element,
    count: Element,
    auto_opened: Cell<bool>,
    from: Rc<DateControl>,
    to: Rc<DateControl>,
    account: Rc<Combo>,
    wallet: HtmlSelectElement,
    kind: HtmlSelectElement,
    state: HtmlSelectElement,
    min: HtmlInputElement,
    max: HtmlInputElement,
    apply: Rc<dyn Fn(Filters)>,
    down: Rc<dyn Fn(bool)>,
}
impl FilterUi {
    #[allow(
        clippy::too_many_lines,
        reason = "Builds the labelled filter DOM and registers period, search and field handlers together."
    )]
    pub(crate) fn new(
        ctx: &Context,
        filters: Filters,
        accounts: &[Value],
        apply: impl Fn(Filters) + 'static,
        down: impl Fn(bool) + 'static,
    ) -> Rc<Self> {
        let label = h("span", "period-label", "");
        attrs(&label, &[("aria-live", "polite")]);
        let prev = button("", "ghost");
        prev.set_class_name("icon-btn");
        attrs(&prev, &[("aria-label", "前の月（[ キー）")]);
        append(&prev, &[icon("chevron-left")]);
        let next = button("", "ghost");
        next.set_class_name("icon-btn");
        attrs(&next, &[("aria-label", "次の月（] キー）")]);
        append(&next, &[icon("chevron-right")]);
        let all = button("すべての期間", "ghost");
        let _ = all.class_list().add_1("btn--sm");
        let month = button("月ごとに見る", "ghost");
        let _ = month.class_list().add_1("btn--sm");
        let period = h("div", "period", "");
        append(
            &period,
            &[
                prev.clone(),
                label.clone(),
                next.clone(),
                all.clone(),
                month.clone(),
            ],
        );
        let search = input("search", "");
        attrs(
            search.as_ref(),
            &[
                ("placeholder", "摘要・取引先・金額（Enterで探します）"),
                ("aria-label", "文字で探す"),
                ("data-search", "1"),
            ],
        );
        let search_box = h("div", "search-box", "");
        let key = h("kbd", "", "/");
        attrs(&key, &[("aria-hidden", "true")]);
        append(&search_box, &[icon("search"), search.clone().into(), key]);
        let from = DateControl::named(ctx, filters.get("from"), "期間の始め", false);
        let to = DateControl::named(ctx, filters.get("to"), "期間の終わり", false);
        let account = Rc::new(Combo::new(
            ctx,
            combo_items(&account_options(accounts, "expense", false, &[]), false),
            filters.get("account"),
        ));
        configure_combo(&account, "科目の候補", "すべての科目", NO_ACCOUNT);
        let wallets = wallet_options(accounts);
        let mut wallet_values = vec![(String::new(), "すべての口座".to_owned())];
        wallet_values.extend(
            wallets
                .iter()
                .map(|wallet| (text(wallet, "code").into(), text(wallet, "name").into())),
        );
        let wallet = select(
            &wallet_values
                .iter()
                .map(|(value, label)| (value.as_str(), label.as_str()))
                .collect::<Vec<_>>(),
            filters.get("wallet"),
        );
        let kind = select(
            &[
                ("", "すべて"),
                ("expense", "支出"),
                ("income", "収入"),
                ("transfer", "振替"),
                ("journal", "仕訳の形"),
                ("opening", "期首残高"),
            ],
            filters.get("kind"),
        );
        let state = select(
            &[
                ("all", "取消済みも出す"),
                ("active", "有効だけ（取消済みを隠す）"),
                ("voided", "取消済みだけ"),
            ],
            filters.get("state"),
        );
        let min = input("text", filters.get("min"));
        let max = input("text", filters.get("max"));
        for (input, placeholder) in [(&min, "下限なし"), (&max, "上限なし")] {
            input.set_class_name("input input--num");
            attrs(
                input.as_ref(),
                &[("inputmode", "numeric"), ("placeholder", placeholder)],
            );
        }
        let grid = h("div", "filter-grid", "");
        let mut fields = vec![
            from.field.el.clone(),
            to.field.el.clone(),
            field("科目", &account.el, "", false).el,
            field("口座", wallet.as_ref(), "", false).el,
            field("種別", kind.as_ref(), "", false).el,
            field("状態", state.as_ref(), "", false).el,
        ];
        for (input, label) in [(&min, "金額の下限"), (&max, "金額の上限")] {
            let affix = h("div", "affix", "");
            append(
                &affix,
                &[input.clone().into(), h("span", "affix-text", "円")],
            );
            fields.push(field(label, &affix, "", false).el);
        }
        append(&grid, &fields);
        let count = h("span", "badge badge--muted filter-count", "");
        let summary = h("summary", "", "");
        append(
            &summary,
            &[icon("filter"), h("span", "", "くわしい条件"), count.clone()],
        );
        let clear = button("条件を消す", "");
        let _ = clear.class_list().add_1("btn--sm");
        let actions = h("div", "filter-actions", "");
        append(&actions, std::slice::from_ref(&clear));
        let more = h("details", "filter-more", "");
        append(&more, &[summary, grid, actions]);
        let top = h("div", "filter-top", "");
        append(&top, &[period, search_box]);
        let el = h("div", "card filters", "");
        attrs(&el, &[("role", "search"), ("aria-label", "仕訳の絞り込み")]);
        append(&el, &[top, more.clone()]);
        let this = Rc::new(Self {
            el,
            current: RefCell::new(filters.clone()),
            search,
            label,
            prev,
            next,
            all,
            month,
            more,
            count,
            auto_opened: Cell::new(false),
            from,
            to,
            account,
            wallet,
            kind,
            state,
            min,
            max,
            apply: Rc::new(apply),
            down: Rc::new(down),
        });
        for (button, delta) in [(&this.prev, -1), (&this.next, 1), (&this.month, 0)] {
            let weak = Rc::downgrade(&this);
            ctx.on(button, "click", move |_| {
                if let Some(this) = weak.upgrade() {
                    (this.apply)(this.current.borrow().shift(delta, &today()));
                }
            });
        }
        let weak = Rc::downgrade(&this);
        ctx.on(&this.all, "click", move |_| {
            if let Some(this) = weak.upgrade() {
                let next = this.current.borrow().patch("from", "").patch("to", "");
                (this.apply)(next);
            }
        });
        let weak = Rc::downgrade(&this);
        ctx.on(&clear, "click", move |_| {
            if let Some(this) = weak.upgrade() {
                (this.apply)(Filters::read(|_| String::new()));
            }
        });
        let weak = Rc::downgrade(&this);
        ctx.on(this.search.as_ref(), "change", move |_| {
            if let Some(this) = weak.upgrade() {
                this.commit_search();
            }
        });
        let weak = Rc::downgrade(&this);
        ctx.on(this.search.as_ref(), "keydown", move |event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(&event) {
                return;
            }
            let Some(this) = weak.upgrade() else {
                return;
            };
            match event.key().as_str() {
                "Enter" => {
                    event.prevent_default();
                    this.commit_search();
                }
                "ArrowDown"
                    if !event.alt_key()
                        && !event.ctrl_key()
                        && !event.meta_key()
                        && !event.shift_key() =>
                {
                    event.prevent_default();
                    let searching = this.search.value().trim() != this.current.borrow().get("q");
                    (this.down)(searching);
                    if searching {
                        this.commit_search();
                    }
                }
                "Escape" => {
                    event.prevent_default();
                    this.search.set_value(this.current.borrow().get("q"));
                    let _ = this.search.blur();
                }
                _ => {}
            }
        });
        for (date, key) in [(&this.from, "from"), (&this.to, "to")] {
            let weak = Rc::downgrade(&this);
            date.on_change(move || {
                if let Some(this) = weak.upgrade() {
                    let value = if key == "from" {
                        this.from.value()
                    } else {
                        this.to.value()
                    };
                    this.patch(key, &value);
                }
            });
        }
        let weak = Rc::downgrade(&this);
        this.account.on_change(move |value, _, _| {
            if let Some(this) = weak.upgrade() {
                this.patch("account", &value);
            }
        });
        for (control, key) in [
            (&this.wallet, "wallet"),
            (&this.kind, "kind"),
            (&this.state, "state"),
        ] {
            let weak = Rc::downgrade(&this);
            let control = control.clone();
            ctx.on(control.clone().as_ref(), "change", move |_| {
                if let Some(this) = weak.upgrade() {
                    this.patch(key, &control.value());
                }
            });
        }
        for (control, key) in [(&this.min, "min"), (&this.max, "max")] {
            let weak = Rc::downgrade(&this);
            let control = control.clone();
            ctx.on(control.clone().as_ref(), "change", move |_| {
                if let Some(this) = weak.upgrade() {
                    let value = parse_amount(&control.value())
                        .ok()
                        .map(|value| value.to_string())
                        .unwrap_or_default();
                    this.patch(key, &value);
                }
            });
        }
        this.sync(filters);
        this
    }
    fn patch(&self, key: &str, value: &str) {
        let next = {
            let old = self.current.borrow();
            if old.get(key) == value {
                return;
            }
            old.patch(key, value)
        };
        (self.apply)(next);
    }
    fn commit_search(&self) {
        self.patch("q", self.search.value().trim());
    }
    pub(crate) fn sync(&self, filters: Filters) {
        set_text(&self.label, &filters.label());
        let all = filters.get("from").is_empty() && filters.get("to").is_empty();
        for el in [&self.prev, &self.next, &self.all] {
            hidden(el, all);
        }
        hidden(&self.month, !all);
        if crate::app::core::dom::document().active_element().as_ref() != Some(self.search.as_ref())
        {
            self.search.set_value(filters.get("q"));
        }
        self.from.set_value(filters.get("from"));
        self.to.set_value(filters.get("to"));
        self.account.set_value(filters.get("account"), false);
        self.wallet.set_value(filters.get("wallet"));
        self.kind.set_value(filters.get("kind"));
        self.state.set_value(filters.get("state"));
        self.min.set_value(filters.get("min"));
        self.max.set_value(filters.get("max"));
        let count = filters.extra_count();
        hidden(&self.count, count == 0);
        set_text(
            &self.count,
            &if count == 0 {
                String::new()
            } else {
                format!("{count}件の条件")
            },
        );
        if count > 0 && !self.auto_opened.replace(true) {
            attrs(&self.more, &[("open", "")]);
        }
        *self.current.borrow_mut() = filters;
    }
}
