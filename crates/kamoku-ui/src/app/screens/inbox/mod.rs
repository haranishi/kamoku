// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::core::dom::{document, window};
use crate::app::prelude::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, VecDeque},
    future::{Future, poll_fn},
    rc::{Rc, Weak},
    task::Poll,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, Event, HtmlElement, HtmlInputElement, HtmlSelectElement, KeyboardEvent};
mod flow;
mod model;
mod row;
mod row_view;
mod table;
mod view;
use model::{
    COLUMNS, Confirmation, PAGE_SIZE, PCT_NOTE, Selection, Summary, cell_order, confirmed_nos,
    date_label, direction_warning, group, id, is_text_entry, is_wallet, items, pct_value,
    row_label, signed, step_option, tax_after, tax_codes, tax_differs, tax_label, tax_touched,
    text, undo_key, unset_tax,
};
use row::Row;
use view::{Bar, Loading, Tools};

#[derive(Clone, PartialEq, Eq)]
struct Filters {
    status: String,
    wallet: String,
    q: String,
    page: u64,
}
impl Filters {
    fn read(ctx: &Context) -> Self {
        Self {
            status: if ctx.route.query("status") == "ignored" {
                "ignored".into()
            } else {
                "draft".into()
            },
            wallet: ctx.route.query("wallet"),
            q: ctx.route.query("q"),
            page: ctx.route.query("page").parse::<u64>().unwrap_or(1).max(1),
        }
    }
    fn hash(&self) -> String {
        let page = self.page.to_string();
        let mut q = vec![];
        if self.status == "ignored" {
            q.push(("status", "ignored"));
        }
        if !self.wallet.is_empty() {
            q.push(("wallet", self.wallet.as_str()));
        }
        if !self.q.is_empty() {
            q.push(("q", self.q.as_str()));
        }
        if self.page > 1 {
            q.push(("page", page.as_str()));
        }
        build_hash("/inbox", &q)
    }
}
struct State {
    ctx: Context,
    heading: Element,
    count: Element,
    summary: Element,
    host: Element,
    pager: Element,
    tools: Tools,
    bar: Bar,
    filters: RefCell<Filters>,
    accounts: RefCell<Vec<Value>>,
    partners: RefCell<Vec<Value>>,
    exempt: Cell<bool>,
    rows: RefCell<Vec<Rc<Row>>>,
    data: RefCell<Vec<Value>>,
    selection: RefCell<Selection>,
    counts: RefCell<Value>,
    total: Cell<u64>,
    seq: Cell<u64>,
    ready: Cell<bool>,
    locked: Cell<bool>,
    sending: RefCell<Option<Summary>>,
    leaving: RefCell<BTreeSet<u64>>,
    ignore_queue: RefCell<VecDeque<Rc<Row>>>,
    ignoring: Cell<bool>,
    undo: RefCell<Vec<u64>>,
    undo_notice: Cell<Option<u64>>,
    all_check: RefCell<Option<HtmlInputElement>>,
    all_count: RefCell<Option<Element>>,
    all_note: RefCell<Option<Element>>,
    table: RefCell<Option<Element>>,
    content_ctx: RefCell<Option<Context>>,
    pager_ctx: RefCell<Option<Context>>,
    loading_cancel: RefCell<Option<Rc<dyn Fn()>>>,
}
impl State {
    fn reset_content(&self) -> Context {
        if let Some(ctx) = self.content_ctx.borrow_mut().take() {
            ctx.scope.dispose();
        }
        self.rows.borrow_mut().clear();
        self.all_check.borrow_mut().take();
        self.all_count.borrow_mut().take();
        self.all_note.borrow_mut().take();
        self.table.borrow_mut().take();
        let ctx = self.ctx.child();
        *self.content_ctx.borrow_mut() = Some(ctx.clone());
        ctx
    }
    fn begin_loading(&self) -> Loading {
        if let Some(cancel) = self.loading_cancel.borrow_mut().take() {
            cancel();
        }
        let loading = Loading::new(&self.ctx, &self.host, 6, "未確認の明細を読み込んでいます");
        *self.loading_cancel.borrow_mut() = Some(loading.cancel());
        loading
    }
    fn navigate(&self, patch: impl FnOnce(&mut Filters)) {
        if self.locked.get() {
            return;
        }
        let mut f = self.filters.borrow().clone();
        f.page = 1;
        patch(&mut f);
        self.ctx.navigate(&f.hash(), false);
    }
    fn account(&self, code: &str) -> Value {
        self.accounts
            .borrow()
            .iter()
            .find(|a| text(a, "code") == code)
            .cloned()
            .unwrap_or(Value::Null)
    }
    fn paint_counts(&self) {
        let counts = self.counts.borrow();
        let n = counts["draft"].as_u64().unwrap_or(0);
        set_text(&self.count, &format!("{n}件"));
        set_text(&self.tools.draft, &format!("未確認 {n}件"));
        set_text(
            &self.tools.ignored,
            &format!("無視した行 {}件", counts["ignored"].as_u64().unwrap_or(0)),
        );
        let f = self.filters.borrow();
        let total = self.total.get();
        set_text(
            &self.summary,
            &if total == 0 {
                String::new()
            } else {
                format!(
                    "{total}件のうち {}〜{}件目（日付の古い順）",
                    f.page
                        .saturating_sub(1)
                        .saturating_mul(PAGE_SIZE)
                        .saturating_add(1),
                    f.page.saturating_mul(PAGE_SIZE).min(total)
                )
            },
        );
    }
    fn paint_bar(&self) {
        let s = self
            .sending
            .borrow()
            .clone()
            .unwrap_or_else(|| self.selection.borrow().summary());
        let ro = self.ctx.is_read_only();
        let reason = if ro {
            self.ctx.read_only_reason()
        } else if self.sending.borrow().is_some() {
            "確定しています…".into()
        } else if s.count == 0 {
            "確定する行は、まだありません。行で Enter を押すと、確定する行になります。".into()
        } else {
            String::new()
        };
        self.bar.update(&self.ctx, &s, &reason, !ro && s.count == 0);
        if let Some(check) = self.all_check.borrow().as_ref() {
            let rows = self.rows.borrow();
            let ready: Vec<_> = rows.iter().filter(|r| r.ready()).collect();
            let marked = ready
                .iter()
                .filter(|r| self.selection.borrow().contains(r.id()))
                .count();
            check.set_checked(!ready.is_empty() && marked == ready.len());
            check.set_indeterminate(marked > 0 && marked < ready.len());
            check.set_disabled(ro);
            if let Some(count) = self.all_count.borrow().as_ref() {
                set_text(
                    count,
                    &if ready.is_empty() {
                        "（0件）".into()
                    } else {
                        format!("（{}件のうち{marked}件を選択中）", ready.len())
                    },
                );
            }
            if !ready.is_empty()
                && let Some(note) = self.all_note.borrow().as_ref()
            {
                set_text(note, "");
            }
        }
    }
    fn paint_pager(self: &Rc<Self>) {
        if let Some(ctx) = self.pager_ctx.borrow_mut().take() {
            ctx.scope.dispose();
        }
        replace(&self.pager, &[]);
        let total = self.total.get();
        if total <= PAGE_SIZE {
            return;
        }
        let ctx = self.ctx.child();
        *self.pager_ctx.borrow_mut() = Some(ctx.clone());
        let page = self.filters.borrow().page;
        let pages = total.div_ceil(PAGE_SIZE);
        let prev = button("前の200件", "secondary");
        prev.set_class_name("btn btn--sm");
        disabled(&prev, page <= 1);
        let next = button("次の200件", "secondary");
        next.set_class_name("btn btn--sm");
        disabled(&next, page >= pages);
        for (el, delta) in [(&prev, -1_i32), (&next, 1)] {
            let weak = Rc::downgrade(self);
            ctx.on(el, "click", move |_| {
                if let Some(s) = weak.upgrade() {
                    s.navigate(|f| {
                        f.page = if delta < 0 {
                            page.saturating_sub(1).max(1)
                        } else {
                            page.saturating_add(1)
                        }
                    });
                }
            });
        }
        append(
            &self.pager,
            &[
                prev,
                h("span", "pager-text", &format!("{page} / {pages} ページ")),
                next,
            ],
        );
    }
    fn spot(&self) -> Option<(u64, usize, String)> {
        let active = document().active_element()?;
        let tr = active.closest("tr.inbox-row").ok().flatten()?;
        let key = tr.get_attribute("data-id")?.parse().ok()?;
        let at = self.rows.borrow().iter().position(|r| r.id() == key)?;
        let col = active
            .closest("td")
            .ok()
            .flatten()
            .and_then(|e| e.get_attribute("data-col"))
            .unwrap_or_else(|| "account".into());
        Some((key, at, col))
    }
    fn reload(self: &Rc<Self>) {
        let s = self.clone();
        self.ctx.task(async move { s.load(None).await });
    }
    async fn load(self: &Rc<Self>, after: Option<(u64, String)>) -> Result<(), ApiError> {
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let spot = self.spot();
        let f = self.filters.borrow().clone();
        let loading = self.begin_loading();
        let offset = f
            .page
            .saturating_sub(1)
            .saturating_mul(PAGE_SIZE)
            .to_string();
        let result = self
            .ctx
            .get(
                "/drafts",
                &[
                    ("status", &f.status),
                    ("wallet", &f.wallet),
                    ("q", &f.q),
                    ("limit", "200"),
                    ("offset", &offset),
                ],
            )
            .await;
        drop(loading);
        if !self.ctx.alive() || seq != self.seq.get() {
            return Ok(());
        }
        let data = match result {
            Ok(v) => v,
            Err(e) => {
                self.error_view("未確認の明細を読み込めませんでした", &e.message, true);
                return Ok(());
            }
        };
        let values = items(&data);
        let total = data["total"].as_u64().unwrap_or(0);
        self.total.set(total);
        if data["counts"].is_object() {
            *self.counts.borrow_mut() = data["counts"].clone();
        }
        if f.status == "draft"
            && f.wallet.is_empty()
            && f.q.is_empty()
            && f.page == 1
            && total <= PAGE_SIZE
        {
            self.selection
                .borrow_mut()
                .keep(&values.iter().map(id).collect::<Vec<_>>());
        }
        *self.data.borrow_mut() = values;
        self.render();
        self.paint_counts();
        self.paint_pager();
        self.paint_bar();
        if let Some((id, col)) = after {
            if let Some(row) = self.rows.borrow().iter().find(|r| r.id() == id) {
                row.focus(&col, true);
            }
        } else if let Some((id, at, col)) = spot {
            let rows = self.rows.borrow();
            if let Some(row) = rows
                .iter()
                .find(|r| r.id() == id)
                .or_else(|| rows.get(at.min(rows.len().saturating_sub(1))))
            {
                row.focus(&col, false);
            }
        }
        Ok(())
    }
    fn error_view(self: &Rc<Self>, title: &str, message: &str, retry: bool) {
        let ctx = self.reset_content();
        let banner = h("div", "banner banner--danger", "");
        append(
            &banner,
            &[h("h2", "banner-title", title), h("p", "", message)],
        );
        if retry {
            let b = button("もう一度読み込む", "secondary");
            let weak = Rc::downgrade(self);
            ctx.on(&b, "click", move |_| {
                if let Some(s) = weak.upgrade() {
                    s.reload();
                }
            });
            append(&banner, &[b]);
        }
        replace(&self.host, &[banner]);
    }
    fn stale(self: &Rc<Self>) {
        self.ctx
            .info("他の場所で更新されました。読み込み直します。");
        self.reload();
    }
}
async fn pause(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
fn class(el: &Element, name: &str, on: bool) {
    let _ = el.class_list().toggle_with_force(name, on);
}
fn hidden(el: &Element, on: bool) {
    if on {
        attrs(el, &[("hidden", "")]);
    } else {
        let _ = el.remove_attribute("hidden");
    }
}
fn node(tag: &str, class: &str, children: &[Element]) -> Element {
    let el = h(tag, class, "");
    append(&el, children);
    el
}
fn year() -> String {
    today().chars().take(4).collect()
}
fn is_mac() -> bool {
    let navigator = window().navigator();
    let platform = navigator.platform().unwrap_or_default();
    let platform = if platform.is_empty() {
        navigator.user_agent().unwrap_or_default()
    } else {
        platform
    };
    ["Mac", "iPhone", "iPad"]
        .iter()
        .any(|name| platform.contains(name))
}
fn import_link() -> Element {
    let link = link("", "#/import", "btn");
    let mark = icon("import");
    attrs(&mark, &[("width", "18"), ("height", "18")]);
    append(&link, &[mark, h("span", "", "明細を取り込む")]);
    link
}

async fn initial_data(ctx: &Context) -> Result<(Value, Value, Value), ApiError> {
    let mut requests =
        ["settings", "accounts", "partners"].map(|name| Box::pin(ctx.load(name, true)));
    let mut results = [None, None, None];
    let results = poll_fn(|cx| {
        for (request, result) in requests.iter_mut().zip(&mut results) {
            if result.is_none()
                && let Poll::Ready(value) = request.as_mut().poll(cx)
            {
                *result = Some(value);
            }
        }
        if results.iter().all(Option::is_some) {
            Poll::Ready(std::mem::take(&mut results))
        } else {
            Poll::Pending
        }
    })
    .await;
    // Finish every shared load even when one fails, so its pending cache is released.
    let [Some(settings), Some(accounts), Some(partners)] = results else {
        return Err(ApiError::message("初期データを確認できませんでした。"));
    };
    Ok((settings?, accounts?, partners?))
}

#[allow(
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Assemble the page under the shared mount(Context) -> Result<Screen, JsValue> contract."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let root = h("div", "inbox-page", "");
    let heading = h("h1", "", "未確認");
    attrs(&heading, &[("tabindex", "-1")]);
    let count = h("span", "page-count", "");
    attrs(&count, &[("aria-live", "polite")]);
    let tools = Tools::new();
    let summary = h("p", "list-summary", "");
    attrs(&summary, &[("aria-live", "polite")]);
    let host = h("div", "inbox-host", "");
    let pager = h("nav", "pager", "");
    attrs(&pager, &[("aria-label", "ページ送り")]);
    let bar = Bar::new();
    // The confirmation landmark becomes available with the first rendered list.
    hidden(&bar.el, true);
    append(
        &root,
        &[
            node(
                "div",
                "page-head",
                &[
                    node("div", "page-head-text", &[heading.clone(), count.clone()]),
                    node("div", "page-head-actions", &[import_link()]),
                ],
            ),
            tools.el.clone(),
            tools.panel.clone(),
        ],
    );
    append(
        &tools.panel,
        &[summary.clone(), host.clone(), pager.clone(), bar.el.clone()],
    );
    let s = Rc::new(State {
        filters: RefCell::new(Filters::read(&ctx)),
        ctx: ctx.clone(),
        heading,
        count,
        summary,
        host,
        pager,
        tools,
        bar,
        accounts: RefCell::new(vec![]),
        partners: RefCell::new(vec![]),
        exempt: Cell::new(false),
        rows: RefCell::new(vec![]),
        data: RefCell::new(vec![]),
        selection: RefCell::new(Selection::default()),
        counts: RefCell::new(json!({"draft":0,"ignored":0})),
        total: Cell::new(0),
        seq: Cell::new(0),
        ready: Cell::new(false),
        locked: Cell::new(false),
        sending: RefCell::new(None),
        leaving: RefCell::new(BTreeSet::new()),
        ignore_queue: RefCell::new(VecDeque::new()),
        ignoring: Cell::new(false),
        undo: RefCell::new(vec![]),
        undo_notice: Cell::new(None),
        all_check: RefCell::new(None),
        all_count: RefCell::new(None),
        all_note: RefCell::new(None),
        table: RefCell::new(None),
        content_ctx: RefCell::new(None),
        pager_ctx: RefCell::new(None),
        loading_cancel: RefCell::new(None),
    });
    s.tools.bind(&s);
    s.bar.bind(&s);
    table::bind_keys(&s);
    s.paint_bar();
    let state = s.clone();
    let loading = s.begin_loading();
    ctx.task(async move {
        let result = async {
            let (settings, accounts, partners) = initial_data(&state.ctx).await?;
            if !state.ctx.alive() {
                return Ok(());
            }
            state.exempt.set(text(&settings, "tax_mode") == "exempt");
            *state.accounts.borrow_mut() = items(&accounts);
            *state.partners.borrow_mut() = items(&partners);
            state.tools.wallets(&state.accounts.borrow());
            state.tools.sync(&state.filters.borrow());
            state.ready.set(true);
            Ok::<(), ApiError>(())
        }
        .await;
        drop(loading);
        match result {
            Ok(()) if state.ctx.alive() && state.ready.get() => state.load(None).await,
            Err(error) if state.ctx.alive() => {
                state.error_view("未確認の画面を準備できませんでした", &error.message, false);
                Ok(())
            }
            _ => Ok(()),
        }
    });
    let mut screen = Screen::new(&ctx, root);
    screen.update = Some(Rc::new(move |next| {
        if next.route.path != "/inbox" {
            return false;
        }
        let f = Filters::read(&next);
        if *s.filters.borrow() != f && !s.locked.get() {
            *s.filters.borrow_mut() = f;
            s.tools.sync(&s.filters.borrow());
            if s.ready.get() {
                s.reload();
            }
        }
        true
    }));
    Ok(screen)
}
