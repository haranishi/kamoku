// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::model::UndoModifiers;
use super::{
    COLUMNS, Context, Element, HtmlElement, JsCast, KeyboardEvent, PCT_NOTE, Rc, Row, State, Value,
    append, attrs, button, cell_order, class, disabled, document, focus, focus_when_scrollable, h,
    hidden, icon, id, ime_ignore, input, is_mac, is_text_entry, link, node, replace, row_label,
    row_view, set_text, signed, text, undo_key, window, year,
};
use crate::app::prelude::phrase;
impl State {
    #[allow(
        clippy::too_many_lines,
        reason = "Build one semantic draft table and its native controls."
    )]
    pub fn render(self: &Rc<Self>) {
        let ctx = self.reset_content();
        let ignored = self.filters.borrow().status == "ignored";
        hidden(&self.bar.el, ignored);
        let data = self.data.borrow().clone();
        if data.is_empty() {
            self.empty(&ctx);
            return;
        }
        if ignored {
            self.render_ignored(&ctx, &data);
            return;
        }
        let all = input("checkbox", "");
        all.set_class_name("cand-check cand-all-check");
        let count = h("span", "cand-all-count", "");
        let note = h("span", "cand-all-note", "");
        attrs(&note, &[("role", "status")]);
        let all_el = node(
            "div",
            "inbox-all",
            &[
                node(
                    "label",
                    "cand cand-all",
                    &[
                        all.clone().into(),
                        h("span", "", "科目のある行をすべて選ぶ"),
                        count.clone(),
                    ],
                ),
                note.clone(),
            ],
        );
        *self.all_check.borrow_mut() = Some(all.clone());
        *self.all_count.borrow_mut() = Some(count);
        *self.all_note.borrow_mut() = Some(note.clone());
        let weak = Rc::downgrade(self);
        let all_input = all.clone();
        ctx.on(&all, "change", move |_| {
            let Some(s) = weak.upgrade() else {
                return;
            };
            let on = all_input.checked();
            let rows = s.rows.borrow().clone();
            if on && !rows.iter().any(|r| r.ready()) {
                all_input.set_checked(false);
                set_text(&note, "科目のある行がありません。");
                return;
            }
            set_text(&note, "");
            for r in rows {
                if r.ready() || !on {
                    r.set_candidate(on);
                }
            }
            s.paint_bar();
        });
        let head = h("tr", "", "");
        let labels = [
            "確定する",
            "日付",
            "摘要",
            "金額（円）",
            "科目",
            "税区分",
            "取引先",
            "メモ",
            "事業の割合",
            "無視",
        ];
        for (col, label) in COLUMNS.into_iter().zip(labels) {
            let th = h(
                "th",
                &format!("c-{col}{}", if col == "amount" { " num" } else { "" }),
                label,
            );
            attrs(
                &th,
                &[("scope", "col"), ("id", &format!("inbox-col-{col}"))],
            );
            if col == "pct" {
                attrs(&th, &[("title", PCT_NOTE)]);
            }
            append(&head, &[th]);
        }
        let body = h("tbody", "", "");
        let mac = is_mac();
        let table = node(
            "table",
            if self.exempt.get() {
                "table inbox-table is-exempt"
            } else {
                "table inbox-table"
            },
            &[
                h(
                    "caption",
                    "",
                    &format!(
                        "未確認の明細（Enterで確定する行にして次の行へ、{}+Enterで確定）",
                        if mac { "⌘" } else { "Ctrl" }
                    ),
                ),
                node("thead", "", &[head]),
                body.clone(),
            ],
        );
        let pct_note = h("p", "visually-hidden", &format!("事業の割合は、{PCT_NOTE}"));
        attrs(&pct_note, &[("id", "inbox-pct-note")]);
        let wrapper = node(
            "div",
            "table-wrap inbox-wrap",
            &[all_el, table.clone(), pct_note],
        );
        attrs(
            &wrapper,
            &[
                ("role", "region"),
                ("aria-label", "未確認の明細"),
                ("tabindex", "-1"),
            ],
        );
        for d in data {
            let row = Row::new(self, &ctx, d);
            append(&body, &[row.tr.clone(), row.msg_tr.clone()]);
            self.rows.borrow_mut().push(row);
        }
        *self.table.borrow_mut() = Some(table);
        replace(&self.host, &[wrapper]);
        self.layout();
    }
    fn empty(self: &Rc<Self>, ctx: &Context) {
        let f = self.filters.borrow().clone();
        let filtered = !f.wallet.is_empty() || !f.q.is_empty();
        let (title, body) = if filtered {
            (
                "条件に合う明細はありません",
                "口座や探す言葉を変えるか、「条件を消す」で全部の明細に戻してください。",
            )
        } else if f.status == "ignored" {
            (
                "無視した行はありません",
                "未確認の表で「無視」を押した行（キーは x）が、ここに並びます。「戻す」で未確認に戻せます。",
            )
        } else {
            (
                "未確認はありません",
                "明細を取り込むか、取引を登録してください。",
            )
        };
        let content = node(
            "div",
            "empty",
            &[
                node(
                    "span",
                    "empty-icon",
                    &[icon(if filtered { "search" } else { "inbox" })],
                ),
                h("p", "empty-title", title),
                node("p", "empty-body", &[phrase(body)]),
            ],
        );
        let actions = h("div", "empty-actions", "");
        if filtered {
            let clear = button("条件を消す", "secondary");
            let weak = Rc::downgrade(self);
            ctx.on(&clear, "click", move |_| {
                if let Some(s) = weak.upgrade() {
                    s.navigate(|f| {
                        f.wallet.clear();
                        f.q.clear();
                    });
                }
            });
            append(&actions, &[clear]);
        } else if f.status == "draft" {
            append(
                &actions,
                &[
                    link("明細を取り込む", "#/import", "btn btn--primary"),
                    link("取引を登録", "#/entry", "btn"),
                ],
            );
        }
        if actions.child_element_count() > 0 {
            append(&content, &[actions]);
        }
        let card = node("div", "card", &[content]);
        replace(&self.host, &[card]);
    }
    fn render_ignored(self: &Rc<Self>, ctx: &Context, data: &[Value]) {
        let header = h("tr", "", "");
        for label in ["日付", "摘要", "金額", "操作"] {
            let th = h("th", if label == "金額" { "num" } else { "" }, label);
            attrs(&th, &[("scope", "col")]);
            append(&header, &[th]);
        }
        let body = h("tbody", "", "");
        for d in data {
            let row = h("tr", "", "");
            attrs(&row, &[("data-key", &id(d).to_string())]);
            let restore = button("戻す", "secondary");
            restore.set_class_name("btn btn--sm restore-btn");
            attrs(
                &restore,
                &[(
                    "aria-label",
                    &format!("未確認に戻す: {}", row_label(d, &year())),
                )],
            );
            disabled(&restore, self.ctx.is_read_only());
            let weak = Rc::downgrade(self);
            let key = id(d);
            ctx.on(&restore, "click", move |_| {
                if let Some(s) = weak.upgrade() {
                    s.restore(key);
                }
            });
            for (label, content) in [
                (
                    "日付",
                    h("span", "", &row_view::short_date(text(d, "date"))),
                ),
                (
                    "",
                    node(
                        "span",
                        "",
                        &[
                            h(
                                "span",
                                "",
                                if text(d, "description").is_empty() {
                                    "（摘要なし）"
                                } else {
                                    text(d, "description")
                                },
                            ),
                            h("span", "sub-inline", text(d, "wallet_name")),
                        ],
                    ),
                ),
                (
                    "金額",
                    h(
                        "span",
                        &format!("amount amount--{}", text(d, "direction")),
                        &signed(d),
                    ),
                ),
                ("", restore),
            ] {
                let td = node("td", if label == "金額" { "num" } else { "" }, &[content]);
                if label.is_empty()
                    && !td
                        .first_element_child()
                        .is_some_and(|el| el.class_list().contains("restore-btn"))
                {
                    class(&td, "wrap", true);
                }
                attrs(&td, &[("data-label", label)]);
                append(&row, &[td]);
            }
            append(&body, &[row]);
        }
        let table = node(
            "table",
            "table table--stack ignored-table",
            &[
                h("caption", "", "無視した行（「戻す」で未確認に戻る）"),
                node("thead", "", &[header]),
                body,
            ],
        );
        let wrapper = node("div", "table-wrap", std::slice::from_ref(&table));
        attrs(
            &wrapper,
            &[
                ("role", "region"),
                ("aria-label", "無視した行（「戻す」で未確認に戻る）"),
            ],
        );
        focus_when_scrollable(ctx, &wrapper, &table);
        replace(&self.host, &[wrapper]);
    }
    pub fn layout(&self) {
        let Some(table) = self.table.borrow().clone() else {
            return;
        };
        let width = self.host.get_bounding_client_rect().width();
        let (mode, order) = cell_order(width);
        let active = document().active_element();
        for row in self.rows.borrow().iter() {
            if row.tr.get_attribute("data-order").as_deref() != Some(mode) {
                for col in order {
                    if let Ok(Some(td)) = row
                        .tr
                        .query_selector(&format!(":scope > td[data-col=\"{col}\"]"))
                    {
                        let _ = row.tr.append_child(&td);
                    }
                }
                attrs(&row.tr, &[("data-order", mode)]);
            }
        }
        if let Some(el) = active
            && el.is_connected()
            && document().active_element() != Some(el.clone())
        {
            focus(&el);
        }
        for selector in ["th.c-tax", "th.c-partner"] {
            if let Ok(Some(th)) = table.query_selector(selector)
                && let Some(el) = th.dyn_ref::<HtmlElement>()
                && el.offset_width() > 0
            {
                if let Some(t) = table.dyn_ref::<HtmlElement>() {
                    let _ = t
                        .style()
                        .set_property("--inbox-right-x", &format!("{}px", el.offset_left()));
                }
                break;
            }
        }
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "Register table-local movement and page confirmation/Undo shortcuts together."
)]
pub(super) fn bind_keys(s: &Rc<State>) {
    let weak = Rc::downgrade(s);
    let callback = wasm_bindgen::closure::Closure::wrap(Box::new(
        move |_entries: js_sys::Array, _observer: web_sys::ResizeObserver| {
            if let Some(s) = weak.upgrade()
                && s.ctx.alive()
            {
                s.layout();
            }
        },
    )
        as Box<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>);
    if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
        observer.observe(&s.host);
        s.ctx.scope.cleanup(move || {
            observer.disconnect();
            drop(callback);
        });
    }
    let weak = Rc::downgrade(s);
    s.ctx.on(&s.host, "keydown", move |event| {
        let Ok(k) = event.dyn_into::<KeyboardEvent>() else {
            return;
        };
        if ime_ignore(&k) || k.default_prevented() || k.ctrl_key() || k.meta_key() || k.alt_key() {
            return;
        }
        let Some(s) = weak.upgrade() else {
            return;
        };
        if s.locked.get() {
            return;
        }
        let Some(el) = k.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return;
        };
        let Some(tr) = el.closest("tr.inbox-row").ok().flatten() else {
            return;
        };
        let key = tr
            .get_attribute("data-id")
            .and_then(|v| v.parse::<u64>().ok());
        let rows = s.rows.borrow().clone();
        let Some(at) = rows.iter().position(|r| Some(r.id()) == key) else {
            return;
        };
        let row = &rows[at];
        let col = el
            .closest("td")
            .ok()
            .flatten()
            .and_then(|e| e.get_attribute("data-col"))
            .unwrap_or_default();
        let open = row.account.is_open() || row.partner.is_open();
        match k.key().as_str() {
            "Enter" if !k.shift_key() && el.tag_name() != "BUTTON" => {
                k.prevent_default();
                row.mark_next(&col);
            }
            "ArrowDown" | "ArrowUp" if !k.shift_key() && !open => {
                k.prevent_default();
                let next = if k.key() == "ArrowDown" {
                    rows.get(at + 1)
                } else {
                    at.checked_sub(1).and_then(|i| rows.get(i))
                };
                if let Some(r) = next {
                    r.focus(&col, false);
                }
            }
            "x" | "X" if col == "cand" || col == "ops" => {
                k.prevent_default();
                s.ignore_row(row.clone());
            }
            " " if k.shift_key()
                && ["date", "account", "tax", "pct", "ops"].contains(&col.as_str()) =>
            {
                k.prevent_default();
                let on = !s.selection.borrow().contains(row.id());
                row.set_candidate(on);
            }
            "Escape" if col != "cand" && !open => {
                k.prevent_default();
                row.focus("cand", false);
            }
            _ => {}
        }
    });
    let weak = Rc::downgrade(s);
    s.ctx.key(move |k| {
        let Some(s) = weak.upgrade() else {
            return false;
        };
        if document()
            .query_selector("dialog[open]")
            .ok()
            .flatten()
            .is_some()
        {
            return false;
        }
        if k.key() == "Enter" && (k.ctrl_key() || k.meta_key()) && !k.alt_key() && !k.shift_key() {
            k.prevent_default();
            s.confirm();
            return true;
        }
        let mac = is_mac();
        let active = document().active_element();
        let is_text = active.as_ref().is_some_and(|el| {
            is_text_entry(
                &el.tag_name(),
                &el.get_attribute("type").unwrap_or_else(|| "text".into()),
                el.dyn_ref::<HtmlElement>()
                    .is_some_and(HtmlElement::is_content_editable),
            )
        });
        if undo_key(
            &k.key(),
            mac,
            UndoModifiers {
                meta: k.meta_key(),
                ctrl: k.ctrl_key(),
                extra: k.shift_key() || k.alt_key(),
            },
        ) && !is_text
        {
            let key = s.undo.borrow().last().copied();
            if let Some(key) = key {
                k.prevent_default();
                s.restore(key);
                return true;
            }
        }
        false
    });
    let weak = Rc::downgrade(s);
    s.ctx
        .scope
        .listen(window().as_ref(), "resize", false, move |_| {
            if let Some(s) = weak.upgrade() {
                s.layout();
            }
        });
}
