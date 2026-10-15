// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::return_path::encode;
use crate::app::prelude::*;
use crate::app::screens::entry::{
    controls::{display_date, shortcut},
    dialogs::ResultCell,
    model::{integer, kind_label, line_memo, list, locked_detail, tax_label, text},
    preview::preview_table,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlTextAreaElement, KeyboardEvent};

pub(crate) fn money_el(value: i64) -> Element {
    let el = h("span", "money", "");
    append(
        &el,
        &[h("span", "num", &number(value)), h("span", "unit", "円")],
    );
    el
}
fn empty_value() -> Element {
    let el = h("span", "empty-value", "");
    let dash = h("span", "", "—");
    attrs(&dash, &[("aria-hidden", "true")]);
    append(&el, &[dash, h("span", "visually-hidden", "なし")]);
    el
}
pub(crate) fn state_badge(entry: &Value, quiet: bool) -> Option<Element> {
    if entry["state"] == "voided" {
        let el = h("span", "badge badge--danger", "");
        append(&el, &[icon("ban"), h("span", "", "取消済み")]);
        Some(el)
    } else if integer(entry, "revision") > 1 {
        let el = h("span", "badge badge--danger", "");
        append(
            &el,
            &[
                icon("edit"),
                h(
                    "span",
                    "",
                    &format!("訂正 第{}版", integer(entry, "revision")),
                ),
            ],
        );
        Some(el)
    } else if quiet {
        None
    } else {
        Some(h("span", "state-plain", "有効"))
    }
}
fn event_label(kind: &str) -> (&str, &str) {
    match kind {
        "posted" | "create" | "created" => ("登録", "plus"),
        "revised" | "correct" | "corrected" => ("訂正", "edit"),
        "voided" | "void" => ("取消", "ban"),
        _ => (kind, "clock"),
    }
}
fn datetime(value: &str) -> String {
    let date = js_sys::Date::new(&value.into());
    if !date.get_time().is_finite() {
        return value.into();
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes()
    )
}
fn account_name(line: &Value) -> &str {
    if text(line, "account_name").is_empty() {
        text(line, "account")
    } else {
        text(line, "account_name")
    }
}
pub(crate) fn detail_lines(lines: &[Value]) -> Vec<Element> {
    let table = h("table", "table detail-lines", "");
    let head = h("tr", "", "");
    for (index, label) in ["貸借", "科目・メモ", "税区分・税額", "金額"]
        .iter()
        .enumerate()
    {
        let th = h("th", if index == 3 { "num" } else { "" }, label);
        attrs(&th, &[("scope", "col")]);
        append(&head, &[th]);
    }
    let thead = h("thead", "", "");
    append(&thead, &[head]);
    let tbody = h("tbody", "", "");
    for line in lines {
        let row = h("tr", "", "");
        let account = h("td", "", "");
        append(
            &account,
            &[h("span", "keep detail-account", account_name(line))],
        );
        let memo = line_memo(line);
        if !memo.is_empty() {
            append(&account, &[h("span", "sub", &memo)]);
        }
        let tax = h("td", "", "");
        let amount = h("span", "sub", "税額 ");
        append(
            &amount,
            &[if integer(line, "tax_amount") != 0 {
                money_el(integer(line, "tax_amount"))
            } else {
                empty_value()
            }],
        );
        append(
            &tax,
            &[h("span", "keep", tax_label(text(line, "tax_code"))), amount],
        );
        let total = h("td", "num", "");
        append(&total, &[money_el(integer(line, "amount"))]);
        append(
            &row,
            &[
                h(
                    "td",
                    "detail-side",
                    if line["side"] == "debit" {
                        "借方"
                    } else {
                        "貸方"
                    },
                ),
                account,
                tax,
                total,
            ],
        );
        append(&tbody, &[row]);
    }
    append(&table, &[h("caption", "", "明細"), thead, tbody]);
    let wrap = h("div", "table-wrap detail-lines-wrap", "");
    append(&wrap, &[table]);
    let sides = h("div", "detail-sides", "");
    attrs(
        &sides,
        &[("role", "group"), ("aria-label", "明細（借方と貸方）")],
    );
    for (key, title) in [("debit", "借方"), ("credit", "貸方")] {
        let col = h("div", "detail-side-col", "");
        let ul = h("ul", "detail-side-list", "");
        for line in lines.iter().filter(|line| line["side"] == key) {
            let li = h("li", "", "");
            let amount = h("span", "detail-amount", "");
            append(&amount, &[money_el(integer(line, "amount"))]);
            let tax = h("span", "sub", tax_label(text(line, "tax_code")));
            if integer(line, "tax_amount") != 0 {
                append(
                    &tax,
                    &[h("span", "", "・税"), money_el(integer(line, "tax_amount"))],
                );
            }
            append(
                &li,
                &[
                    h("span", "keep detail-account", account_name(line)),
                    amount,
                    tax,
                ],
            );
            let memo = line_memo(line);
            if !memo.is_empty() {
                append(&li, &[h("span", "sub", &memo)]);
            }
            append(&ul, &[li]);
        }
        append(&col, &[h("p", "detail-side-title", title), ul]);
        append(&sides, &[col]);
    }
    vec![wrap, sides]
}
fn history_list(history: &[Value]) -> Element {
    let ol = h("ol", "timeline", "");
    for event in history {
        let (label, icon_name) = event_label(text(event, "kind"));
        let li = h(
            "li",
            &format!("timeline-item timeline-item--{icon_name}"),
            "",
        );
        let mark = h("span", "timeline-icon", "");
        append(&mark, &[icon(icon_name)]);
        let body = h("div", "timeline-body", "");
        let title = h("p", "timeline-title", "");
        append(
            &title,
            &[
                h("strong", "", label),
                h("span", "", &format!(" 第{}版", integer(event, "revision"))),
                h(
                    "span",
                    "muted small",
                    &format!("　{}", datetime(text(event, "recorded_at"))),
                ),
            ],
        );
        append(&body, &[title]);
        if !text(event, "reason").is_empty() {
            append(
                &body,
                &[h("p", "", &format!("理由: {}", text(event, "reason")))],
            );
        }
        let hash = text(event, "hash");
        let short = if hash.chars().count() > 12 {
            format!("{}…", hash.chars().take(12).collect::<String>())
        } else {
            hash.into()
        };
        let code = h("code", "mono", &short);
        attrs(&code, &[("title", hash)]);
        let record = h("p", "muted small", "記録のハッシュ ");
        append(
            &record,
            &[
                code,
                h(
                    "span",
                    "",
                    &format!("（{}番目の記録）", integer(event, "seq")),
                ),
            ],
        );
        append(&body, &[record]);
        if event["lines"].is_array() {
            let details = h("details", "timeline-lines", "");
            append(
                &details,
                &[
                    h("summary", "", "この版の明細"),
                    preview_table(
                        &list(&event["lines"]),
                        &format!("第{}版の明細", integer(event, "revision")),
                    ),
                ],
            );
            append(&body, &[details]);
        }
        append(&li, &[mark, body]);
        append(&ol, &[li]);
    }
    ol
}

fn void_summary(summary: &Element, entry: &Value) {
    set_text(
        summary,
        &format!(
            "No.{}（{}・{}・{}円）を取り消します。取り消すと、元に戻せません。取り消した仕訳は帳票の集計から外れ、記録は履歴に残ります。",
            integer(entry, "no"),
            display_date(text(entry, "date")),
            if text(entry, "description").is_empty() {
                "摘要なし"
            } else {
                text(entry, "description")
            },
            number(integer(entry, "total"))
        ),
    );
}
#[allow(
    clippy::too_many_lines,
    reason = "Builds the reason dialog and registers its scoped submit, cancellation and keyboard handlers."
)]
pub(crate) async fn void_dialog(ctx: &Context, entry: Value) -> Option<Value> {
    let dialog = Dialog::new(ctx, "仕訳を取り消しますか");
    let reason: HtmlTextAreaElement = h("textarea", "textarea", "").unchecked_into();
    attrs(
        reason.as_ref(),
        &[
            ("rows", "3"),
            ("maxlength", "200"),
            ("placeholder", "例: 二重に登録したため"),
        ],
    );
    let reason_field = Rc::new(field(
        "取消の理由",
        reason.as_ref(),
        "履歴に残ります。",
        true,
    ));
    let summary = h("p", "", "");
    void_summary(&summary, &entry);
    let stack = h("div", "stack", "");
    append(&stack, &[summary.clone(), reason_field.el.clone()]);
    append(&dialog.body, &[stack]);
    let cancel = button("やめる", "");
    let yes = button("取り消す", "danger");
    shortcut(
        &yes,
        &format!("{} Enter", mod_label()),
        if mod_label() == "⌘" {
            "Meta+Enter"
        } else {
            "Control+Enter"
        },
    );
    append(&dialog.footer, &[cancel.clone(), yes.clone()]);
    let d = dialog.clone();
    dialog.scope.on(&cancel, "click", move |_| d.close());
    let result = ResultCell::new();
    let saved = Rc::new(RefCell::new(None));
    let saved_close = saved.clone();
    let answer = result.clone();
    dialog.scope.on(dialog.el.as_ref(), "close", move |_| {
        answer.resolve(saved_close.borrow().clone());
    });
    let current = Rc::new(RefCell::new(entry));
    let sending = Rc::new(Cell::new(false));
    let context = ctx.clone();
    let d = dialog.clone();
    let reason_submit = reason.clone();
    let submit = Rc::new(move || {
        if sending.get() {
            return;
        }
        let reason = reason_submit.value().trim().to_owned();
        if reason.is_empty() {
            reason_field.set_error("取消の理由を入れてください。");
            focus(reason_submit.as_ref());
            return;
        }
        if current.borrow()["state"] == "voided" {
            d.set_error("この仕訳は、ほかの場所ですでに取り消されています。");
            return;
        }
        reason_field.set_error("");
        d.set_error("");
        sending.set(true);
        d.set_busy(true);
        let d = d.clone();
        let ctx = context.clone();
        let current = current.clone();
        let saved = saved.clone();
        let sending = sending.clone();
        let summary = summary.clone();
        let field = reason_field.clone();
        let control = reason_submit.clone();
        let entry = current.borrow().clone();
        context.task(async move {
            let path = format!("/entries/{}", encode(text(&entry, "id")));
            let response = ctx.post(
                &format!("{path}/void"),
                &json!({"reason":reason,"expected_revision":entry["revision"]}),
            ).await;
            if !ctx.alive() || !d.el.open() {
                sending.set(false);
                return Ok(());
            }
            d.set_busy(false);
            match response {
                Ok(value) => {
                    ctx.invalidate(&["status"]);
                    ctx.success(&format!("No.{} を取り消しました。", integer(&entry, "no")));
                    *saved.borrow_mut() = Some(value);
                    d.close();
                }
                Err(error) if error.is_stale() => {
                    ctx.info("他の場所で更新されました。読み込み直します。");
                    match ctx.get(&path, &[]).await {
                        Ok(latest) if ctx.alive() && d.el.open() => {
                            void_summary(&summary, &latest);
                            let message = if latest["state"] == "voided" {
                                "この仕訳は、ほかの場所ですでに取り消されています。".into()
                            } else {
                                format!("最新の版（第{}版）を読み込みました。内容を確かめて、もう一度「取り消す」を押してください。", integer(&latest, "revision"))
                            };
                            d.set_error(&message);
                            *current.borrow_mut() = latest;
                        }
                        Ok(_) => {}
                        Err(error) => d.set_error(&error.message),
                    }
                }
                Err(error) => {
                    if error.field.as_deref() == Some("reason") {
                        field.set_error(&error.message);
                        focus(control.as_ref());
                    } else {
                        d.set_error(&error.message);
                    }
                }
            }
            sending.set(false);
            Ok(())
        });
    });
    let submit_click = submit.clone();
    dialog.scope.on(&yes, "click", move |_| submit_click());
    dialog
        .scope
        .on(dialog.el.as_ref(), "keydown", move |event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(&event) {
                return;
            }
            if event.key() == "Enter" && (event.ctrl_key() || event.meta_key()) {
                event.prevent_default();
                submit();
            }
        });
    dialog.show();
    focus(reason.as_ref());
    result.wait().await
}
pub(crate) fn mod_label() -> &'static str {
    let platform = js_sys::Reflect::get(
        &crate::app::core::dom::window().navigator(),
        &"platform".into(),
    )
    .ok()
    .and_then(|value| value.as_string())
    .unwrap_or_default();
    if platform.contains("Mac") {
        "⌘"
    } else {
        "Ctrl"
    }
}

pub(crate) struct Drawer {
    pub id: String,
    pub dialog: Dialog,
    ctx: Context,
    content: Element,
    badge: Element,
    entry: RefCell<Value>,
    locked: RefCell<String>,
    how: RefCell<String>,
    sequence: Cell<u64>,
    focus_after_load: Cell<bool>,
    edit_hash: Rc<dyn Fn() -> String>,
    changed: Rc<dyn Fn()>,
}
impl Drawer {
    pub(crate) fn new(
        ctx: &Context,
        id: &str,
        edit_hash: impl Fn() -> String + 'static,
        changed: impl Fn() + 'static,
        closed: impl Fn(String) + 'static,
    ) -> Rc<Self> {
        let dialog = Dialog::new(ctx, "仕訳の詳細");
        let _ = dialog.el.class_list().add_1("drawer");
        let _ = dialog.footer.set_attribute("hidden", "");
        let content = h("div", "detail", "");
        append(&content, &[h("p", "muted", "読み込んでいます…")]);
        append(&dialog.body, std::slice::from_ref(&content));
        let badge = h("div", "", "");
        if let Some(head) = dialog.el.query_selector(".dialog-head").ok().flatten() {
            if let Some(close) = head.query_selector("button").ok().flatten() {
                let _ = head.insert_before(&badge, Some(&close));
            } else {
                append(&head, std::slice::from_ref(&badge));
            }
        }
        if let Some(title) = dialog.el.query_selector("h2").ok().flatten() {
            attrs(&title, &[("tabindex", "-1")]);
        }
        let this = Rc::new(Self {
            id: id.into(),
            dialog,
            ctx: ctx.clone(),
            content,
            badge,
            entry: RefCell::new(Value::Null),
            locked: RefCell::new(String::new()),
            how: RefCell::new(String::new()),
            sequence: Cell::new(0),
            focus_after_load: Cell::new(false),
            edit_hash: Rc::new(edit_hash),
            changed: Rc::new(changed),
        });
        let weak = Rc::downgrade(&this);
        this.dialog
            .scope
            .on(this.dialog.el.as_ref(), "close", move |_| {
                if let Some(this) = weak.upgrade() {
                    closed(this.how.borrow().clone());
                }
            });
        let weak = Rc::downgrade(&this);
        this.dialog
            .scope
            .on(this.dialog.el.as_ref(), "click", move |event| {
                if let Some(this) = weak.upgrade()
                    && event.target().as_ref() == Some(this.dialog.el.as_ref())
                    && let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>()
                {
                    let rect = this.dialog.el.get_bounding_client_rect();
                    if f64::from(mouse.client_x()) < rect.left()
                        || f64::from(mouse.client_x()) > rect.right()
                        || f64::from(mouse.client_y()) < rect.top()
                        || f64::from(mouse.client_y()) > rect.bottom()
                    {
                        this.close("cancel");
                    }
                }
            });
        let weak = Rc::downgrade(&this);
        this.dialog
            .scope
            .on(this.dialog.el.as_ref(), "keydown", move |event| {
                let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                    return;
                };
                if ime_ignore(&event) || event.alt_key() || event.ctrl_key() || event.meta_key() {
                    return;
                }
                let typing = event
                    .target()
                    .and_then(|target| target.dyn_into::<Element>().ok())
                    .is_some_and(|target| {
                        target
                            .closest("input,textarea,select,[contenteditable=true]")
                            .ok()
                            .flatten()
                            .is_some()
                    });
                if typing {
                    return;
                }
                if let Some(this) = weak.upgrade() {
                    match event.key().as_str() {
                        "e" => {
                            event.prevent_default();
                            this.edit();
                        }
                        "v" => {
                            event.prevent_default();
                            this.do_void();
                        }
                        _ => {}
                    }
                }
            });
        this.dialog.show();
        this.focus_title();
        this.load();
        this
    }
    pub(crate) fn close(&self, how: &str) {
        *self.how.borrow_mut() = how.into();
        self.dialog.close();
    }
    fn focus_title(&self) {
        if let Some(title) = self.dialog.el.query_selector("h2").ok().flatten() {
            focus(&title);
        }
    }
    fn blocked(&self) -> bool {
        let entry = self.entry.borrow();
        entry.is_null()
            || self.ctx.is_read_only()
            || entry["state"] == "voided"
            || entry["kind"] == "opening"
            || entry["meta"]["kind"] == "opening"
            || !locked_detail(text(&entry, "date"), &self.locked.borrow()).is_empty()
    }
    fn edit(&self) {
        if self.blocked() {
            return;
        }
        let hash = (self.edit_hash)();
        self.close("edit");
        self.ctx.navigate(&hash, false);
    }
    fn do_void(self: &Rc<Self>) {
        if self.blocked() {
            return;
        }
        let this = self.clone();
        let entry = self.entry.borrow().clone();
        self.ctx.task(async move {
            if void_dialog(&this.ctx, entry).await.is_some()
                && this.ctx.alive()
                && this.dialog.el.open()
            {
                (this.changed)();
                this.focus_after_load.set(true);
                this.load();
            }
            Ok(())
        });
    }
    pub(crate) fn load(self: &Rc<Self>) {
        let sequence = self.sequence.get() + 1;
        self.sequence.set(sequence);
        let this = self.clone();
        self.ctx.task(async move {
            let entry = this
                .ctx
                .get(&format!("/entries/{}", encode(&this.id)), &[])
                .await;
            let settings = this
                .ctx
                .load("settings", false)
                .await
                .unwrap_or(Value::Null);
            if !this.ctx.alive() || !this.dialog.el.open() || this.sequence.get() != sequence {
                return Ok(());
            }
            match entry {
                Ok(entry) => {
                    *this.entry.borrow_mut() = entry;
                    *this.locked.borrow_mut() = text(&settings, "locked_until").into();
                    this.render();
                    if this.focus_after_load.replace(false)
                        && !crate::app::core::dom::document()
                            .active_element()
                            .is_some_and(|active| this.dialog.el.contains(Some(&active)))
                    {
                        this.focus_title();
                    }
                }
                Err(error) => replace(&this.content, &[h("p", "field-error", &error.message)]),
            }
            Ok(())
        });
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Assembles the semantic detail, actions, lines and history DOM for one entry."
    )]
    fn render(self: &Rc<Self>) {
        let entry = self.entry.borrow().clone();
        self.dialog
            .set_title(&format!("No.{} の仕訳", integer(&entry, "no")));
        replace(
            &self.badge,
            &state_badge(&entry, false).into_iter().collect::<Vec<_>>(),
        );
        let total = h("p", "detail-total", "");
        append(
            &total,
            &[
                h("span", "detail-total-label", "金額"),
                money_el(integer(&entry, "total")),
            ],
        );
        let facts = h("dl", "detail-facts", "");
        for (label, key) in [
            ("日付", "date"),
            ("種別", "kind"),
            ("摘要", "description"),
            ("取引先", "partner"),
            ("メモ", "memo"),
        ] {
            let value = if key == "memo" {
                text(&entry["input"], "memo")
            } else {
                text(&entry, key)
            };
            if key == "memo" && value.is_empty() {
                continue;
            }
            let row = h("div", "", "");
            let dd = h("dd", "", "");
            let child = if value.is_empty() {
                empty_value()
            } else {
                h(
                    "span",
                    if key == "partner" { "keep" } else { "" },
                    &match key {
                        "date" => display_date(value),
                        "kind" => kind_label(value).into(),
                        _ => value.into(),
                    },
                )
            };
            append(&dd, &[child]);
            append(&row, &[h("dt", "", label), dd]);
            append(&facts, &[row]);
        }
        let mut parts = vec![total, facts];
        let history = list(&entry["history"]);
        if history.len() >= 2
            && let Some(last) = history.last()
        {
            let (label, icon_name) = event_label(text(last, "kind"));
            let latest = h("p", "detail-latest", "");
            let content = h("span", "", "");
            append(
                &content,
                &[
                    h(
                        "strong",
                        "",
                        &format!("{label} 第{}版", integer(last, "revision")),
                    ),
                    h(
                        "span",
                        "",
                        &format!(
                            "（{}）{}",
                            datetime(text(last, "recorded_at")),
                            if text(last, "reason").is_empty() {
                                String::new()
                            } else {
                                format!("　理由: {}", text(last, "reason"))
                            }
                        ),
                    ),
                ],
            );
            append(&latest, &[icon(icon_name), content]);
            parts.push(latest);
        }
        if entry["meta"]["kind"] == "opening" || entry["kind"] == "opening" {
            let opening = h("p", "detail-actions detail-opening", "");
            let link = link(
                "設定の「期首残高」を開く",
                "#/settings?section=opening",
                "tap",
            );
            let weak = Rc::downgrade(self);
            self.dialog.scope.on(&link, "click", move |_| {
                if let Some(this) = weak.upgrade() {
                    this.close("edit");
                }
            });
            append(
                &opening,
                &[
                    icon("info"),
                    h("span", "", "期首残高は、設定の「期首残高」から見直します。"),
                    link,
                ],
            );
            parts.push(opening);
        } else {
            let edit = button("訂正する", "");
            shortcut(&edit, "e", "e");
            let void = button("取り消す", "");
            let _ = void.class_list().add_1("btn-void");
            shortcut(&void, "v", "v");
            let actions = h("div", "detail-actions", "");
            append(&actions, &[edit.clone(), void.clone()]);
            let note = if self.ctx.is_read_only() {
                self.ctx.read_only_reason()
            } else if entry["state"] == "voided" {
                "取消済みの仕訳は、訂正も取消もできません。".into()
            } else {
                locked_detail(text(&entry, "date"), &self.locked.borrow())
            };
            for control in [&edit, &void] {
                disabled(control, !note.is_empty());
                if !note.is_empty() {
                    attrs(control, &[("aria-describedby", "detail-note")]);
                }
            }
            let note_el = h("p", "field-hint", &note);
            attrs(&note_el, &[("id", "detail-note")]);
            parts.extend([actions, note_el]);
            let weak = Rc::downgrade(self);
            self.dialog.scope.on(&edit, "click", move |_| {
                if let Some(this) = weak.upgrade() {
                    this.edit();
                }
            });
            let weak = Rc::downgrade(self);
            self.dialog.scope.on(&void, "click", move |_| {
                if let Some(this) = weak.upgrade() {
                    this.do_void();
                }
            });
        }
        let lines = h("section", "detail-section", "");
        attrs(&lines, &[("aria-labelledby", "lines-title")]);
        let title = h("h3", "", "明細");
        attrs(&title, &[("id", "lines-title")]);
        append(&lines, &[title]);
        append(&lines, &detail_lines(&list(&entry["lines"])));
        parts.push(lines);
        if entry["state"] == "voided" {
            parts.push(h("p","small muted","取消済みの仕訳は、最後に有効だった版の明細を出しています。帳票の集計には入りません。"));
        }
        let section = h("section", "detail-section", "");
        attrs(&section, &[("aria-labelledby", "history-title")]);
        let title = h("h3", "", "履歴");
        attrs(&title, &[("id", "history-title")]);
        append(
            &section,
            &[
                title,
                h(
                    "p",
                    "detail-hash-note",
                    "「記録のハッシュ」は、記録ごとの印です。設定の「データ」で、台帳が後から書き換えられていないかを確かめるときに使います。",
                ),
                history_list(&history),
            ],
        );
        parts.push(section);
        replace(&self.content, &parts);
    }
}
