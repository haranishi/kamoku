// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub(crate) mod forms;
pub(crate) mod model;
mod view;

use crate::app::prelude::*;
use crate::app::screens::home::common::{box_node, checkbox, el, heading, list, n, s};
use serde_json::Value;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlInputElement, KeyboardEvent};

pub(super) struct Masters {
    page: Context,
    view: Option<Scope>,
    kind: String,
    items: Vec<Value>,
    balances: Value,
    counts: BTreeMap<String, i64>,
    query: String,
    all: bool,
    body: Element,
    count: Element,
    search: HtmlInputElement,
    checkbox: HtmlInputElement,
    sequence: u64,
}
pub(super) type Shared = Rc<RefCell<Masters>>;

pub(super) fn load(ctx: &Context, state: &Shared) {
    let (sequence, kind) = {
        let mut state = state.borrow_mut();
        state.sequence = state.sequence.wrapping_add(1);
        (state.sequence, state.kind.clone())
    };
    let c = ctx.clone();
    let state = state.clone();
    ctx.task(async move {
        let items = c
            .load(
                if kind == "partner" {
                    "partners"
                } else {
                    "accounts"
                },
                true,
            )
            .await?;
        let balances = if kind == "wallet" {
            c.get("/wallets", &[]).await?
        } else {
            Value::Null
        };
        let mut counts = BTreeMap::new();
        if kind == "wallet" {
            for wallet in list(&items).iter().filter(|a| a["wallet"].is_object()) {
                if !c.alive() {
                    return Ok(());
                }
                let code = s(wallet, "code");
                if let Ok(value) = c
                    .get(
                        "/drafts",
                        &[("status", "draft"), ("wallet", code), ("limit", "1")],
                    )
                    .await
                {
                    counts.insert(code.into(), n(&value, "total"));
                }
            }
        }
        if c.alive() && sequence == state.borrow().sequence {
            {
                let mut state = state.borrow_mut();
                state.items = list(&items);
                state.balances = balances;
                state.counts = counts;
            }
            paint(&state);
        }
        Ok(())
    });
}
pub(super) fn edit(_ctx: &Context, state: &Shared, item: Option<Value>) {
    let ctx = state.borrow().page.clone();
    if ctx.is_read_only() {
        ctx.error(&ctx.read_only_reason());
        return;
    }
    let (kind, accounts) = {
        let state = state.borrow();
        (state.kind.clone(), state.items.clone())
    };
    let c = ctx.clone();
    let state = state.clone();
    forms::open(&ctx, &kind, item, accounts, "bank", move |_| {
        load(&c, &state);
    });
}
fn paint(state: &Shared) {
    let page = state.borrow().page.clone();
    let scope = Scope::new();
    let old = state.borrow_mut().view.replace(scope.clone());
    if let Some(old) = old {
        old.dispose();
    }
    let ctx = Context {
        route: page.route,
        scope,
        shared: page.shared,
    };
    view::paint(&ctx, state);
}
fn commit(ctx: &Context, state: &Shared) {
    let hash = {
        let state = state.borrow();
        let path = if state.kind == "partner" {
            "/partners"
        } else {
            "/accounts"
        };
        build_hash(
            path,
            &[
                ("q", &state.query),
                ("all", if state.all { "1" } else { "" }),
            ],
        )
    };
    // Replacement navigation calls the same-screen update synchronously.
    // Release the read guard before that update mutates this state.
    ctx.navigate(&hash, true);
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let (kind, title, label) = match ctx.route.path.as_str() {
        "/wallets" => ("wallet", "口座", "口座を追加"),
        "/partners" => ("partner", "取引先", "取引先を足す"),
        _ => ("account", "勘定科目", "科目を足す"),
    };
    let root = h("div", "", "");
    let add = button(label, "primary");
    disabled(&add, ctx.is_read_only());
    let body = h("div", "masters", "");
    let count = h("span", "page-count", "");
    attrs(&count, &[("aria-live", "polite")]);
    let search = input("search", &ctx.route.query("q"));
    let search_label = if kind == "partner" {
        "取引先を探す"
    } else {
        "科目を探す"
    };
    attrs(
        &el(&search),
        &[("aria-label", search_label), ("data-search", "1")],
    );
    search.set_placeholder(if kind == "partner" {
        "名前・読み・登録番号で探す"
    } else {
        "名前・読み・コードで探す"
    });
    let (all, checkbox) = checkbox("使っていない科目も出す", ctx.route.query("all") == "1");
    append(&root, &[heading(title, Some(add.clone())), count.clone()]);
    if kind != "wallet" {
        let tools = box_node(
            "div",
            "masters-tools",
            &[box_node(
                "div",
                "search-box",
                &[icon("search"), el(&search)],
            )],
        );
        attrs(
            &tools,
            &[
                ("role", "search"),
                (
                    "aria-label",
                    if kind == "partner" {
                        "取引先の絞り込み"
                    } else {
                        "科目の絞り込み"
                    },
                ),
            ],
        );
        if kind == "account" {
            append(&tools, &[all]);
        }
        append(&root, &[tools]);
    }
    append(&root, std::slice::from_ref(&body));
    let state = Rc::new(RefCell::new(Masters {
        page: ctx.clone(),
        view: None,
        kind: kind.into(),
        items: vec![],
        balances: Value::Null,
        counts: BTreeMap::new(),
        query: ctx.route.query("q"),
        all: ctx.route.query("all") == "1",
        body,
        count,
        search: search.clone(),
        checkbox: checkbox.clone(),
        sequence: 0,
    }));
    let c = ctx.clone();
    let shared = state.clone();
    ctx.on(&add, "click", move |_| edit(&c, &shared, None));
    let shared = state.clone();
    let control = search.clone();
    ctx.on(&el(&search), "input", move |_| {
        shared.borrow_mut().query = control.value().trim().into();
        paint(&shared);
    });
    let c = ctx.clone();
    let shared = state.clone();
    ctx.on(&el(&search), "change", move |_| commit(&c, &shared));
    let c = ctx.clone();
    let shared = state.clone();
    ctx.on(&el(&search), "keydown", move |event| {
        if let Some(key) = event.dyn_ref::<KeyboardEvent>()
            && key.key() == "Enter"
            && !ime_ignore(key)
        {
            key.prevent_default();
            commit(&c, &shared);
        }
    });
    let c = ctx.clone();
    let shared = state.clone();
    ctx.on(&el(&checkbox), "change", move |_| {
        shared.borrow_mut().all = checkbox.checked();
        paint(&shared);
        commit(&c, &shared);
    });
    load(&ctx, &state);
    let cleanup = state.clone();
    ctx.scope.cleanup(move || {
        if let Some(scope) = cleanup.borrow_mut().view.take() {
            scope.dispose();
        }
    });
    let mut screen = Screen::new(&ctx, root);
    screen.update = Some(Rc::new(move |next| {
        let (q, all) = (next.route.query("q"), next.route.query("all") == "1");
        {
            let mut state = state.borrow_mut();
            if q == state.query && all == state.all {
                return true;
            }
            state.page = next.clone();
            state.query = q;
            state.all = all;
            state.search.set_value(&state.query);
            state.checkbox.set_checked(all);
        }
        paint(&state);
        true
    }));
    Ok(screen)
}
