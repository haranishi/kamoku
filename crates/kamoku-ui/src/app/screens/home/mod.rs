// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub(crate) mod chart;
pub(crate) mod chart_model;
pub(crate) mod common;
mod parts;

use super::reports::period::{shift_month, valid_month};
use crate::app::prelude::*;
use common::{banner_node, box_node, el, heading, icon_button, list, n, s};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Element, HtmlSelectElement};

struct Home {
    month: String,
    first: String,
    later: Vec<String>,
    sequence: u64,
    body: Element,
    months: HtmlSelectElement,
    previous: Element,
    next: Element,
    view: Option<Scope>,
    loading: Option<Scope>,
}
type Shared = Rc<RefCell<Home>>;

fn available(state: &Home) -> Vec<String> {
    let now = year_month(&today());
    let first = if valid_month(&state.first) && state.first <= now {
        &state.first
    } else {
        &now
    };
    let mut values = state.later.clone();
    let mut month = now.clone();
    loop {
        if month < *first {
            break;
        }
        values.push(month.clone());
        let next = shift_month(&month, -1);
        if next == month {
            break;
        }
        month = next;
    }
    values.push(state.month.clone());
    values.sort();
    values.dedup();
    values.reverse();
    values
}
fn paint_months(state: &Home) {
    let values = available(state);
    let options: Vec<_> = values
        .iter()
        .map(|m| {
            let o = h(
                "option",
                "",
                &format!("{}年{}月", &m[..4], m[5..].parse::<u32>().unwrap_or(1)),
            );
            attrs(&o, &[("value", m)]);
            o
        })
        .collect();
    replace(&el(&state.months), &options);
    state.months.set_value(&state.month);
    let at = values.iter().position(|m| m == &state.month).unwrap_or(0);
    disabled(&state.previous, at + 1 >= values.len());
    disabled(&state.next, at == 0);
}
fn go(ctx: &Context, state: &Shared, step: i32) {
    let state = state.borrow();
    let values = available(&state);
    let at = values.iter().position(|m| m == &state.month).unwrap_or(0);
    let next = if step < 0 {
        at.checked_add(1)
    } else {
        at.checked_sub(1)
    };
    if let Some(month) = next.and_then(|i| values.get(i)) {
        navigate(ctx, month);
    }
}
fn navigate(ctx: &Context, month: &str) {
    ctx.navigate(
        &build_hash(
            "/",
            &[(
                "month",
                if month == year_month(&today()) {
                    ""
                } else {
                    month
                },
            )],
        ),
        false,
    );
}
fn skeleton(lines: usize, label: &str) -> Element {
    let block = h("div", "skel-block", "");
    attrs(&block, &[("role", "status"), ("aria-label", label)]);
    for index in 0..lines {
        let bar = h("span", "skel", "");
        let widths = ["40%", "90%", "70%", "85%", "60%"];
        let _ = bar
            .unchecked_ref::<HtmlElement>()
            .style()
            .set_property("width", widths[index % widths.len()]);
        append(&block, &[bar]);
    }
    block
}
fn delayed_loading(ctx: &Context, state: &Shared, sequence: u64) -> Scope {
    let scope = Scope::new();
    let target = state.borrow().body.clone();
    let shown = Rc::new(RefCell::new(None::<Element>));
    let output = shown.clone();
    let state = Rc::downgrade(state);
    let context = ctx.clone();
    let callback = Closure::wrap(Box::new(move || {
        if !context.alive()
            || state
                .upgrade()
                .is_none_or(|s| s.borrow().sequence != sequence)
        {
            return;
        }
        let node = box_node(
            "div",
            "home",
            &[
                skeleton(2, "ホームの数字を読み込んでいます"),
                box_node(
                    "div",
                    "kpis",
                    &(0..4)
                        .map(|_| skeleton(2, "読み込んでいます"))
                        .collect::<Vec<_>>(),
                ),
                skeleton(5, "読み込んでいます"),
            ],
        );
        append(&target, std::slice::from_ref(&node));
        *output.borrow_mut() = Some(node);
    }) as Box<dyn FnMut()>);
    if let Ok(timer) = web_sys::window()
        .unwrap()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            300,
        )
    {
        scope.cleanup(move || {
            web_sys::window().unwrap().clear_timeout_with_handle(timer);
            if let Some(node) = shown.borrow_mut().take() {
                node.remove();
            }
            drop(callback);
        });
    }
    scope
}
fn load(ctx: &Context, state: &Shared) {
    let (month, sequence, body) = {
        let mut s = state.borrow_mut();
        s.sequence = s.sequence.wrapping_add(1);
        if let Some(scope) = s.view.take() {
            scope.dispose();
        }
        if let Some(scope) = s.loading.take() {
            scope.dispose();
        }
        replace(&s.body, &[]);
        paint_months(&s);
        (s.month.clone(), s.sequence, s.body.clone())
    };
    let loader = delayed_loading(ctx, state, sequence);
    state.borrow_mut().loading = Some(loader);
    let ctx2 = ctx.clone();
    let state = state.clone();
    ctx.task(async move {
        let result = async {
            let dash = ctx2.get("/reports/dashboard", &[("month", &month)]).await?;
            let status = ctx2.load("status", true).await?;
            let settings = ctx2.load("settings", false).await?;
            Ok::<_, ApiError>((dash, status, settings))
        }
        .await;
        if !ctx2.alive() || sequence != state.borrow().sequence {
            return Ok(());
        }
        if let Some(loader) = state.borrow_mut().loading.take() {
            loader.dispose();
        }
        match result {
            Ok((dash, status, settings)) => {
                {
                    let mut view = state.borrow_mut();
                    view.first = s(&settings, "opened_on").get(..7).unwrap_or("").into();
                    let now = year_month(&today());
                    if n(&dash, "fiscal_year").to_string() == now[..4] {
                        view.later = list(&dash["series"])
                            .into_iter()
                            .filter(|r| {
                                s(r, "month") > now.as_str()
                                    && (n(r, "revenue") != 0 || n(r, "expense") != 0)
                            })
                            .map(|r| s(&r, "month").into())
                            .collect();
                    }
                    paint_months(&view);
                }
                let scope = Scope::new();
                let view_ctx = Context {
                    route: ctx2.route.clone(),
                    scope: scope.clone(),
                    shared: ctx2.shared.clone(),
                };
                parts::paint(&view_ctx, &body, &dash, &status, &settings, &month);
                state.borrow_mut().view = Some(scope);
            }
            Err(error) => {
                let retry = button("もう一度読み込む", "");
                let c = ctx2.clone();
                let shared = state.clone();
                ctx2.on(&retry, "click", move |_| load(&c, &shared));
                replace(
                    &body,
                    &[
                        banner_node("ホームの数字を読み込めませんでした", &error.message, true),
                        retry,
                    ],
                );
            }
        }
        Ok(())
    });
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let root = h("div", "", "");
    let now = year_month(&today());
    let query = ctx.route.query("month");
    let month = if valid_month(&query) { query } else { now };
    let months = select(&[], &month);
    attrs(
        &el(&months),
        &[
            ("class", "select month-select"),
            ("aria-label", "表示する月"),
        ],
    );
    let previous = icon_button("前の月", "chevron-left", "month-step");
    let next = icon_button("次の月", "chevron-right", "month-step");
    let body = h("div", "home", "");
    let actions = box_node(
        "div",
        "month-picker",
        &[previous.clone(), el(&months), next.clone()],
    );
    let register = link("取引を登録", "#/entry", "btn hide-mobile");
    attrs(&register, &[("aria-keyshortcuts", "n")]);
    let head = heading("ホーム", Some(actions));
    if let Ok(Some(host)) = head.query_selector(".page-head-actions") {
        append(&host, &[register]);
    }
    append(&root, &[head, body.clone()]);
    let state = Rc::new(RefCell::new(Home {
        month,
        first: String::new(),
        later: vec![],
        sequence: 0,
        body,
        months: months.clone(),
        previous: previous.clone(),
        next: next.clone(),
        view: None,
        loading: None,
    }));
    for (button, step) in [(previous, -1), (next, 1)] {
        let c = ctx.clone();
        let state = state.clone();
        ctx.on(&button, "click", move |_| go(&c, &state, step));
    }
    let c = ctx.clone();
    ctx.on(&el(&months), "change", move |_| {
        navigate(&c, &months.value());
    });
    load(&ctx, &state);
    let cleanup = state.clone();
    ctx.scope.cleanup(move || {
        let mut view = cleanup.borrow_mut();
        if let Some(scope) = view.view.take() {
            scope.dispose();
        }
        if let Some(scope) = view.loading.take() {
            scope.dispose();
        }
    });
    let mut screen = Screen::new(&ctx, root);
    screen.update = Some(Rc::new(move |next| {
        let value = next.route.query("month");
        let value = if valid_month(&value) {
            value
        } else {
            year_month(&today())
        };
        if state.borrow().month != value {
            state.borrow_mut().month = value;
            load(&next, &state);
        }
        true
    }));
    Ok(screen)
}
