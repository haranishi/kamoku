// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

mod detail;
mod filters;
pub(crate) mod model;
pub(crate) mod return_path;

use self::{
    detail::{Drawer, money_el, state_badge, void_dialog},
    filters::FilterUi,
    model::{Filters, PAGE_SIZE, side_summary},
    return_path::{correct_hash, detail_hash, from_back_param, is_return_target, same_address},
};
use crate::app::prelude::*;
use crate::app::screens::entry::{
    controls::{display_date, shortcut},
    dialogs::ResultCell,
    model::{integer, list, text},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::spawn_local;
use web_sys::{Element, KeyboardEvent};

fn list_hash(filters: &Filters) -> String {
    let query = filters.query();
    build_hash(
        "/journal",
        &query
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>(),
    )
}
struct PendingFocus {
    id: String,
    index: usize,
    force: bool,
}
struct Journal {
    ctx: Context,
    route: RefCell<Route>,
    filters: RefCell<Filters>,
    filter_host: Element,
    filter_ui: RefCell<Option<Rc<FilterUi>>>,
    summary: Element,
    list_host: Element,
    pager_top: Element,
    pager: Element,
    table_wrap: Element,
    tbody: Element,
    items: RefCell<Vec<Value>>,
    rows: RefCell<Vec<Element>>,
    current: Cell<usize>,
    total: Cell<u64>,
    sequence: Cell<u64>,
    loading: Cell<bool>,
    pending: RefCell<Option<PendingFocus>>,
    drawer: RefCell<Option<Rc<Drawer>>>,
    opened_from_list: Cell<bool>,
    prev_hash: RefCell<Option<String>>,
    back_to: RefCell<String>,
}
impl Journal {
    fn apply(&self, filters: &Filters) {
        self.ctx.navigate(&list_hash(filters), false);
    }
    fn focus_row(&self, index: usize) {
        let rows = self.rows.borrow();
        if rows.is_empty() {
            return;
        }
        let index = index.min(rows.len() - 1);
        self.current.set(index);
        for (at, row) in rows.iter().enumerate() {
            attrs(row, &[("tabindex", if at == index { "0" } else { "-1" })]);
        }
        focus(&rows[index]);
        rows[index].scroll_into_view();
    }
    fn focused_index(&self) -> Option<usize> {
        let active = crate::app::core::dom::document().active_element()?;
        self.rows.borrow().iter().position(|row| *row == active)
    }
    fn focused(&self) -> Option<Value> {
        self.focused_index()
            .and_then(|index| self.items.borrow().get(index).cloned())
    }
    fn focus_entry(&self, pending: PendingFocus) {
        if self.loading.get() {
            *self.pending.borrow_mut() = Some(pending);
            return;
        }
        if crate::app::core::dom::document()
            .query_selector("dialog[open]")
            .ok()
            .flatten()
            .is_some()
            || self.items.borrow().is_empty()
        {
            return;
        }
        let active = crate::app::core::dom::document().active_element();
        if !pending.force
            && active.as_ref().is_some_and(|active| {
                active.tag_name() != "BODY" && !self.list_host.contains(Some(active))
            })
        {
            return;
        }
        let index = self
            .items
            .borrow()
            .iter()
            .position(|entry| entry["id"] == pending.id)
            .unwrap_or(pending.index);
        self.focus_row(index);
    }
    fn render_rows(&self) {
        let items = self.items.borrow();
        let mut rows = Vec::new();
        for (index, entry) in items.iter().enumerate() {
            let row = h(
                "tr",
                if entry["state"] == "voided" {
                    "is-voided"
                } else {
                    ""
                },
                "",
            );
            attrs(
                &row,
                &[
                    ("data-index", &index.to_string()),
                    ("data-key", text(entry, "id")),
                    ("data-entry-id", text(entry, "id")),
                    (
                        "tabindex",
                        if index == self.current.get() {
                            "0"
                        } else {
                            "-1"
                        },
                    ),
                ],
            );
            let no = h("td", "num c-no", &integer(entry, "no").to_string());
            attrs(&no, &[("data-label", "No")]);
            let date = h("td", "c-date", &display_date(text(entry, "date")));
            attrs(&date, &[("data-label", "日付")]);
            let description = h("td", "wrap c-desc", "");
            attrs(&description, &[("data-label", "")]);
            append(
                &description,
                &[h(
                    "span",
                    "",
                    if text(entry, "description").is_empty() {
                        "（摘要なし）"
                    } else {
                        text(entry, "description")
                    },
                )],
            );
            if !text(entry, "partner").is_empty() {
                append(
                    &description,
                    &[
                        h("span", "visually-hidden", "、取引先 "),
                        h("span", "journal-partner", text(entry, "partner")),
                    ],
                );
            }
            let mut cells = vec![no, date, description];
            for (side, label, class) in
                [("debit", "借方", "c-debit"), ("credit", "貸方", "c-credit")]
            {
                let td = h("td", class, "");
                attrs(&td, &[("data-label", label)]);
                append(
                    &td,
                    &[h(
                        "span",
                        "keep",
                        &side_summary(&list(&entry["lines"]), side),
                    )],
                );
                cells.push(td);
            }
            let total = h("td", "num c-total", "");
            attrs(&total, &[("data-label", "金額（円）")]);
            append(
                &total,
                &[h("span", "amount", &number(integer(entry, "total")))],
            );
            let state = h("td", "c-state", "");
            attrs(&state, &[("data-label", "詳細")]);
            if let Some(badge) = state_badge(entry, true) {
                append(&state, &[badge]);
            }
            let go = h("td", "c-go", "");
            attrs(&go, &[("data-label", "")]);
            append(&go, &[icon("chevron-right")]);
            cells.extend([total, state, go]);
            append(&row, &cells);
            rows.push(row);
        }
        if !rows.is_empty() && self.current.get() >= rows.len() {
            self.current.set(rows.len() - 1);
            attrs(rows.last().expect("nonempty"), &[("tabindex", "0")]);
        }
        replace(&self.tbody, &rows);
        *self.rows.borrow_mut() = rows;
    }
    fn paint_pager(self: &Rc<Self>) {
        let total = self.total.get();
        let filters = self.filters.borrow().clone();
        let pages = total.div_ceil(PAGE_SIZE);
        for nav in [&self.pager_top, &self.pager] {
            replace(nav, &[]);
            if total <= PAGE_SIZE {
                continue;
            }
            let prev = button("前の50件", "");
            let next = button("次の50件", "");
            let _ = prev.class_list().add_1("btn--sm");
            let _ = next.class_list().add_1("btn--sm");
            append(&prev, &[icon("chevron-left")]);
            append(&next, &[icon("chevron-right")]);
            disabled(&prev, filters.page <= 1);
            disabled(&next, filters.page >= pages);
            append(
                nav,
                &[
                    prev.clone(),
                    h(
                        "span",
                        "pager-text",
                        &format!("{} / {pages} ページ", filters.page),
                    ),
                    next.clone(),
                ],
            );
            for (button, delta) in [(&prev, -1_i64), (&next, 1)] {
                let weak = Rc::downgrade(self);
                self.ctx.on(button, "click", move |_| {
                    if let Some(this) = weak.upgrade() {
                        let mut filters = this.filters.borrow().clone();
                        filters.page = if delta < 0 {
                            filters.page.saturating_sub(1).max(1)
                        } else {
                            filters.page.saturating_add(1)
                        };
                        this.apply(&filters);
                    }
                });
            }
        }
    }
    fn load(self: &Rc<Self>) {
        let sequence = self.sequence.get() + 1;
        self.sequence.set(sequence);
        self.loading.set(true);
        let filters = self.filters.borrow().clone();
        let this = self.clone();
        self.ctx.task(async move {
            let params = filters.api();
            let query: Vec<_> = params.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();
            let response = this.ctx.get("/entries", &query).await;
            if !this.ctx.alive() || this.sequence.get() != sequence { return Ok(()); }
            this.loading.set(false);
            match response {
                Ok(response) => {
                    let total = response["total"].as_u64().unwrap_or(0);
                    this.total.set(total);
                    *this.items.borrow_mut() = list(&response["items"]);
                    let from = if total > 0 { filters.page.saturating_sub(1).saturating_mul(PAGE_SIZE) + 1 } else { 0 };
                    let to = filters.page.saturating_mul(PAGE_SIZE).min(total);
                    let count = |value: u64| number(i64::try_from(value).unwrap_or(i64::MAX));
                    let mut parts = vec![h("span", "", &if total > 0 {
                        format!("{}件のうち {}〜{}件目（日付の新しい順）", count(total), count(from), count(to))
                    } else { "0件".into() })];
                    if total > 0 {
                        let scope = match filters.get("state") { "active" => "", "voided" => "（取消済みだけ）", _ => "（取消済みも含む）" };
                        let sum = h("span", "list-sum", &format!("条件に合う{}件の合計{scope} ", count(total)));
                        append(&sum, &[money_el(integer(&response, "sum_amount"))]);
                        parts.push(sum);
                    }
                    replace(&this.summary, &parts);
                    if this.items.borrow().is_empty() {
                        let filtered = !filters.query().is_empty();
                        let card = h("div", "card", "");
                        let empty = h("div", "empty", "");
                        append(&empty, &[
                            h("h2", "empty-title", if filtered { "条件に合う仕訳はありません" } else { "まだ仕訳がありません" }),
                            h("p", "muted", if filtered { "条件をゆるめるか、「条件を消す」で全部の仕訳に戻してください。" } else { "取引を登録するか、明細を取り込んで確定すると、ここに並びます。" }),
                            link(if filtered { "条件を消す" } else { "取引を登録" }, if filtered { "#/journal" } else { "#/entry" }, if filtered { "btn" } else { "btn btn--primary" }),
                        ]);
                        append(&card, &[empty]);
                        replace(&this.list_host, &[card]);
                        this.rows.borrow_mut().clear();
                    } else {
                        this.render_rows();
                        replace(&this.list_host, std::slice::from_ref(&this.table_wrap));
                    }
                    this.paint_pager();
                    let pending = this.pending.borrow_mut().take();
                    if let Some(pending) = pending { this.focus_entry(pending); }
                }
                Err(error) => {
                    this.pending.borrow_mut().take();
                    let banner = h("div", "banner banner--danger", "");
                    let retry = button("もう一度読み込む", "");
                    let _ = retry.class_list().add_1("btn--sm");
                    append(&banner, &[h("h2", "", "仕訳を読み込めませんでした"), h("p", "", &error.message), retry.clone()]);
                    let weak = Rc::downgrade(&this);
                    this.ctx.on(&retry, "click", move |_| if let Some(this) = weak.upgrade() { this.load(); });
                    replace(&this.list_host, &[banner]);
                }
            }
            Ok(())
        });
    }
    fn open_detail(&self, id: &str) {
        self.opened_from_list.set(true);
        let hash = detail_hash(id, &list_hash(&self.filters.borrow()));
        self.ctx.navigate(&hash, false);
    }
    fn close_target(&self) -> (String, bool) {
        let list = list_hash(&self.filters.borrow());
        let previous = self.prev_hash.borrow().clone();
        if self.opened_from_list.get() {
            let via = previous
                .as_ref()
                .is_none_or(|previous| same_address(previous, &list));
            return (list, via);
        }
        let back_to = self.back_to.borrow().clone();
        if !back_to.is_empty() {
            let via = previous
                .as_ref()
                .is_some_and(|previous| same_address(previous, &back_to));
            return (back_to, via);
        }
        if let Some(previous) = previous
            && (is_return_target(&previous) || same_address(&previous, &list))
        {
            return (previous, true);
        }
        (list, false)
    }
    fn sync_drawer(self: &Rc<Self>, id: &str) {
        if self
            .drawer
            .borrow()
            .as_ref()
            .is_some_and(|drawer| drawer.id == id)
        {
            return;
        }
        let old = self.drawer.borrow_mut().take();
        if let Some(drawer) = old {
            drawer.close("route");
        }
        if id.is_empty() {
            return;
        }
        let weak = Rc::downgrade(self);
        let edit_id = id.to_owned();
        let change_weak = Rc::downgrade(self);
        let close_weak = Rc::downgrade(self);
        let close_id = id.to_owned();
        let drawer = Drawer::new(
            &self.ctx,
            id,
            move || {
                weak.upgrade().map_or_else(
                    || correct_hash(&edit_id, ""),
                    |this| correct_hash(&edit_id, &this.close_target().0),
                )
            },
            move || {
                if let Some(this) = change_weak.upgrade() {
                    this.load();
                }
            },
            move |how| {
                if let Some(this) = close_weak.upgrade() {
                    this.drawer_closed(&close_id, &how);
                }
            },
        );
        *self.drawer.borrow_mut() = Some(drawer);
    }
    fn drawer_closed(&self, id: &str, how: &str) {
        if how == "route"
            || self
                .drawer
                .borrow()
                .as_ref()
                .is_none_or(|drawer| drawer.id != id)
        {
            return;
        }
        self.drawer.borrow_mut().take();
        if how == "edit" {
            return;
        }
        let (target, back) = self.close_target();
        self.opened_from_list.set(false);
        if back {
            self.ctx.back(&target);
        } else {
            self.ctx.navigate(&target, true);
        }
        if same_address(&target, &list_hash(&self.filters.borrow())) {
            self.focus_entry(PendingFocus {
                id: id.into(),
                index: self.current.get(),
                force: true,
            });
        } else {
            focus_entry_row_soon(id.into(), target);
        }
    }
    fn update(self: &Rc<Self>, next: &Context) -> bool {
        if next.route.path != "/journal" && !next.route.path.starts_with("/journal/") {
            return false;
        }
        self.prev_hash
            .borrow_mut()
            .clone_from(&next.route.previous_hash);
        *self.back_to.borrow_mut() = from_back_param(&next.route.query("back"));
        let filters = Filters::read(|key| next.route.query(key));
        let changed = *self.filters.borrow() != filters;
        *self.filters.borrow_mut() = filters.clone();
        *self.route.borrow_mut() = next.route.clone();
        if changed {
            if let Some(ui) = self.filter_ui.borrow().as_ref() {
                ui.sync(filters);
            }
            // Retire the previous query's rows before its replacement loads.
            // Readers and key handlers must not use rows from stale conditions.
            replace(
                &self.list_host,
                &[h("p", "muted", "仕訳を読み込んでいます")],
            );
            self.rows.borrow_mut().clear();
            self.load();
        }
        self.sync_drawer(&next.route.param("id"));
        true
    }
    fn do_void(self: &Rc<Self>, entry: Value) {
        let index = self.focused_index().unwrap_or(self.current.get());
        let this = self.clone();
        self.ctx.task(async move {
            if void_dialog(&this.ctx, entry.clone()).await.is_some() && this.ctx.alive() {
                *this.pending.borrow_mut() = Some(PendingFocus {
                    id: text(&entry, "id").into(),
                    index,
                    force: false,
                });
                this.load();
            }
            Ok(())
        });
    }
}

pub(crate) fn focus_entry_row_soon(id: String, hash: String) {
    spawn_local(async move {
        let want = Route::parse(&hash, None).path;
        let mut elapsed = 0;
        loop {
            let document = crate::app::core::dom::document();
            let main = document.get_element_by_id("main");
            let here = Route::current().path == want
                && document
                    .query_selector("dialog[open]")
                    .ok()
                    .flatten()
                    .is_none();
            if here && let Some(main) = main.as_ref() {
                let active = document.active_element();
                if active
                    .as_ref()
                    .is_some_and(|active| active.tag_name() != "BODY" && active.tag_name() != "H1")
                {
                    return;
                }
                if let Ok(rows) = main.query_selector_all("tr[data-entry-id]") {
                    for index in 0..rows.length() {
                        if let Some(row) = rows
                            .item(index)
                            .and_then(|row| row.dyn_into::<Element>().ok())
                            && row.get_attribute("data-entry-id").as_deref() == Some(&id)
                        {
                            focus(&row);
                            row.scroll_into_view();
                            return;
                        }
                    }
                }
            }
            if elapsed >= 1000 {
                if here
                    && let Some(first) = main
                        .and_then(|main| main.query_selector("table [tabindex='0']").ok().flatten())
                {
                    focus(&first);
                }
                return;
            }
            let done = ResultCell::new();
            let signal = done.clone();
            let callback = Closure::once(move || signal.resolve(()));
            let _ = crate::app::core::dom::window()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.as_ref().unchecked_ref(),
                    50,
                );
            done.wait().await;
            drop(callback);
            elapsed += 50;
        }
    });
}

#[allow(
    clippy::too_many_lines,
    reason = "Creates the journal DOM and registers its scoped row, filter, route and lifecycle handlers."
)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Matches the router's uniform owned-Context, fallible screen-mount interface."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let filters = Filters::read(|key| ctx.route.query(key));
    let root = h("div", "", "");
    let head = h("div", "page-head", "");
    let text_box = h("div", "page-head-text", "");
    let title = h("h1", "", "仕訳帳");
    attrs(&title, &[("tabindex", "-1")]);
    append(&text_box, &[title]);
    let actions = h("div", "page-head-actions", "");
    let add = link("", "#/entry", "btn hide-mobile");
    attrs(&add, &[("aria-keyshortcuts", "n")]);
    append(&add, &[icon("plus-square"), h("span", "", "取引を登録")]);
    shortcut(&add, "n", "n");
    append(&actions, &[add]);
    append(&head, &[text_box, actions]);
    let filter_host = h("div", "", "");
    let summary = h("p", "list-summary", "");
    attrs(&summary, &[("aria-live", "polite")]);
    let list_host = h("div", "list-host", "");
    append(&list_host, &[h("p", "muted", "仕訳を読み込んでいます")]);
    let pager_top = h("nav", "pager pager--top", "");
    attrs(&pager_top, &[("aria-label", "ページ送り（一覧の上）")]);
    let pager = h("nav", "pager", "");
    attrs(&pager, &[("aria-label", "ページ送り（一覧の下）")]);
    append(
        &root,
        &[
            head,
            filter_host.clone(),
            summary.clone(),
            pager_top.clone(),
            list_host.clone(),
            pager.clone(),
        ],
    );
    let table = h("table", "table journal-table", "");
    let caption = "仕訳の一覧（Enterで詳細を開く）";
    let header = h("tr", "", "");
    for (index, label) in [
        "No",
        "日付",
        "摘要",
        "借方",
        "貸方",
        "金額（円）",
        "詳細",
        "詳細を開く",
    ]
    .iter()
    .enumerate()
    {
        let th = h(
            "th",
            if index == 0 || index == 5 {
                "num"
            } else if index == 6 {
                "c-state"
            } else {
                ""
            },
            "",
        );
        attrs(&th, &[("scope", "col")]);
        if index == 1 {
            attrs(&th, &[("aria-sort", "descending")]);
        }
        append(
            &th,
            &[h(
                "span",
                if index == 7 { "visually-hidden" } else { "" },
                label,
            )],
        );
        append(&header, &[th]);
    }
    let thead = h("thead", "", "");
    append(&thead, &[header]);
    let tbody = h("tbody", "", "");
    append(&table, &[h("caption", "", caption), thead, tbody.clone()]);
    let table_wrap = h("div", "table-wrap", "");
    attrs(&table_wrap, &[("role", "region"), ("aria-label", caption)]);
    append(&table_wrap, &[table]);
    let state = Rc::new(Journal {
        ctx: ctx.clone(),
        route: RefCell::new(ctx.route.clone()),
        filters: RefCell::new(filters),
        filter_host,
        filter_ui: RefCell::new(None),
        summary,
        list_host,
        pager_top,
        pager,
        table_wrap,
        tbody,
        items: RefCell::new(Vec::new()),
        rows: RefCell::new(Vec::new()),
        current: Cell::new(0),
        total: Cell::new(0),
        sequence: Cell::new(0),
        loading: Cell::new(false),
        pending: RefCell::new(None),
        drawer: RefCell::new(None),
        opened_from_list: Cell::new(false),
        prev_hash: RefCell::new(ctx.route.previous_hash.clone()),
        back_to: RefCell::new(from_back_param(&ctx.route.query("back"))),
    });
    let weak = Rc::downgrade(&state);
    ctx.on(&state.tbody, "click", move |event| {
        let Some(this) = weak.upgrade() else {
            return;
        };
        let Some(target) = event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
        else {
            return;
        };
        if target
            .closest("a,button,input,select,textarea")
            .ok()
            .flatten()
            .is_some()
        {
            return;
        }
        if let Some(row) = target.closest("tr[data-index]").ok().flatten()
            && let Some(index) = row
                .get_attribute("data-index")
                .and_then(|index| index.parse::<usize>().ok())
        {
            this.focus_row(index);
            let id = this
                .items
                .borrow()
                .get(index)
                .map(|entry| text(entry, "id").to_owned());
            if let Some(id) = id {
                this.open_detail(&id);
            }
        }
    });
    let weak = Rc::downgrade(&state);
    ctx.on(&state.tbody, "focusin", move |event| {
        if let Some(this) = weak.upgrade()
            && let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            && let Some(index) = target
                .get_attribute("data-index")
                .and_then(|index| index.parse::<usize>().ok())
        {
            this.current.set(index);
            for (at, row) in this.rows.borrow().iter().enumerate() {
                attrs(row, &[("tabindex", if at == index { "0" } else { "-1" })]);
            }
        }
    });
    let weak = Rc::downgrade(&state);
    ctx.on(&state.tbody, "keydown", move |event| {
        let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
            return;
        };
        if ime_ignore(&event) {
            return;
        }
        let Some(this) = weak.upgrade() else {
            return;
        };
        let Some(index) = this.focused_index() else {
            return;
        };
        match event.key().as_str() {
            "ArrowDown" => {
                event.prevent_default();
                this.focus_row(index + 1);
            }
            "ArrowUp" => {
                event.prevent_default();
                this.focus_row(index.saturating_sub(1));
            }
            "Home" => {
                event.prevent_default();
                this.focus_row(0);
            }
            "End" => {
                event.prevent_default();
                this.focus_row(this.items.borrow().len().saturating_sub(1));
            }
            "Enter" => {
                event.prevent_default();
                let id = this
                    .items
                    .borrow()
                    .get(index)
                    .map(|entry| text(entry, "id").to_owned());
                if let Some(id) = id {
                    this.open_detail(&id);
                }
            }
            _ => {}
        }
    });
    let weak = Rc::downgrade(&state);
    ctx.key(move |event| {
        if !single_keys_enabled()
            || event.ctrl_key()
            || event.meta_key()
            || event.alt_key()
            || event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .is_some_and(|target| is_typing_target(&target))
        {
            return false;
        }
        let Some(this) = weak.upgrade() else {
            return false;
        };
        match event.key().as_str() {
            "j" | "k" => {
                if this.items.borrow().is_empty() {
                    return false;
                }
                let current = this.focused_index();
                let index = if event.key() == "j" {
                    current.map_or(0, |index| index + 1)
                } else {
                    current.unwrap_or(0).saturating_sub(1)
                };
                this.focus_row(index);
                true
            }
            "[" | "]" => {
                this.apply(
                    &this
                        .filters
                        .borrow()
                        .shift(if event.key() == "[" { -1 } else { 1 }, &today()),
                );
                true
            }
            "e" | "v" => {
                let Some(entry) = this.focused() else {
                    return false;
                };
                if entry["state"] == "voided"
                    || entry["kind"] == "opening"
                    || this.ctx.is_read_only()
                {
                    return false;
                }
                if event.key() == "e" {
                    this.ctx.navigate(
                        &correct_hash(text(&entry, "id"), &list_hash(&this.filters.borrow())),
                        false,
                    );
                } else {
                    this.do_void(entry);
                }
                true
            }
            _ => false,
        }
    });
    let loading = state.clone();
    ctx.task(async move {
        match loading.ctx.load("accounts", false).await {
            Ok(accounts) if loading.ctx.alive() => {
                let weak = Rc::downgrade(&loading);
                let down_weak = Rc::downgrade(&loading);
                let ui = FilterUi::new(
                    &loading.ctx,
                    loading.filters.borrow().clone(),
                    &list(&accounts),
                    move |filters| {
                        if let Some(this) = weak.upgrade() {
                            this.apply(&filters);
                        }
                    },
                    move |searching| {
                        if let Some(this) = down_weak.upgrade() {
                            if searching {
                                *this.pending.borrow_mut() = Some(PendingFocus {
                                    id: String::new(),
                                    index: 0,
                                    force: true,
                                });
                            } else if !this.loading.get() && !this.items.borrow().is_empty() {
                                this.focus_row(0);
                            }
                        }
                    },
                );
                replace(&loading.filter_host, std::slice::from_ref(&ui.el));
                *loading.filter_ui.borrow_mut() = Some(ui);
            }
            Ok(_) => {}
            Err(_) => replace(&loading.filter_host, &[]),
        }
        Ok(())
    });
    let weak = Rc::downgrade(&state);
    ctx.scope.cleanup(move || {
        if let Some(state) = weak.upgrade() {
            state.sequence.set(state.sequence.get() + 1);
            let drawer = state.drawer.borrow_mut().take();
            if let Some(drawer) = drawer {
                drawer.close("route");
            }
        }
    });
    state.load();
    state.sync_drawer(&ctx.route.param("id"));
    let mut screen = Screen::new(&ctx, root);
    screen.update = Some(Rc::new(move |next| state.update(&next)));
    Ok(screen)
}
