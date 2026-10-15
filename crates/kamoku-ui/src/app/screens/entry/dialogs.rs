// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    controls::{display_date, shortcut},
    model::{changed_fields, integer, kind_label, list, tax_label, text},
};
use crate::app::prelude::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    task::{Poll, Waker},
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, KeyboardEvent};

pub(crate) struct ResultCell<T> {
    value: RefCell<Option<T>>,
    waiter: RefCell<Option<Waker>>,
}
impl<T> ResultCell<T> {
    pub(crate) fn new() -> Rc<Self> {
        Rc::new(Self {
            value: RefCell::new(None),
            waiter: RefCell::new(None),
        })
    }
    pub(crate) fn resolve(&self, value: T) {
        if self.value.borrow().is_none() {
            *self.value.borrow_mut() = Some(value);
            if let Some(waiter) = self.waiter.borrow_mut().take() {
                waiter.wake();
            }
        }
    }
    pub(crate) async fn wait(&self) -> T {
        let value = std::future::poll_fn(|cx| {
            if let Some(value) = self.value.borrow_mut().take() {
                Poll::Ready(value)
            } else {
                *self.waiter.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await;
        // Native close cleanup restores the opener in a microtask. Callers can
        // then move focus to a field or row without that cleanup undoing it.
        let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(
            &wasm_bindgen::JsValue::NULL,
        ))
        .await;
        value
    }
}

pub(crate) async fn ask(
    ctx: &Context,
    title: &str,
    body: Vec<Element>,
    cancel: &str,
    yes: &str,
    danger: bool,
    focus_yes: bool,
) -> bool {
    let dialog = Dialog::new(ctx, title);
    append(&dialog.body, &body);
    if let Some(paragraph) = body.first().filter(|el| el.tag_name() == "P") {
        let id = crate::app::core::dom::uid("confirm-description");
        attrs(paragraph, &[("id", &id)]);
        attrs(dialog.el.as_ref(), &[("aria-describedby", &id)]);
    }
    let cancel = button(cancel, "");
    let yes = button(yes, if danger { "danger" } else { "primary" });
    if focus_yes {
        let platform = js_sys::Reflect::get(
            &crate::app::core::dom::window().navigator(),
            &"platform".into(),
        )
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_default();
        shortcut(
            &yes,
            if platform.contains("Mac") {
                "⌘ Enter"
            } else {
                "Ctrl Enter"
            },
            if platform.contains("Mac") {
                "Meta+Enter"
            } else {
                "Control+Enter"
            },
        );
    }
    append(&dialog.footer, &[cancel.clone(), yes.clone()]);
    let result = ResultCell::new();
    let accepted = Rc::new(Cell::new(false));
    let d = dialog.clone();
    dialog.scope.on(&cancel, "click", move |_| d.close());
    let d = dialog.clone();
    let flag = accepted.clone();
    dialog.scope.on(&yes, "click", move |_| {
        flag.set(true);
        d.close();
    });
    let answer = result.clone();
    let flag = accepted.clone();
    dialog.scope.on(dialog.el.as_ref(), "close", move |_| {
        answer.resolve(flag.get());
    });
    let yes_key = yes.clone();
    dialog
        .scope
        .on(dialog.el.as_ref(), "keydown", move |event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if ime_ignore(&event) {
                return;
            }
            if focus_yes && event.key() == "Enter" && (event.ctrl_key() || event.meta_key()) {
                event.prevent_default();
                yes_key.unchecked_ref::<HtmlElement>().click();
            }
        });
    dialog.show();
    focus(if focus_yes { &yes } else { &cancel });
    result.wait().await
}

fn name_of(accounts: &[Value], code: &str) -> String {
    accounts
        .iter()
        .find(|account| account["code"] == code)
        .map_or_else(|| code.into(), |account| text(account, "name").into())
}
fn diff_value(value: &Value, key: &str, accounts: &[Value]) -> String {
    let current = &value[key];
    let raw = current.as_str().unwrap_or_default();
    match key {
        "kind" => kind_label(raw).into(),
        "date" => display_date(raw),
        "amount" => current.as_i64().map_or_else(
            || "（なし）".into(),
            |amount| format!("{}円", number(amount)),
        ),
        "account" | "wallet" | "to_wallet" => {
            if raw.is_empty() {
                "（なし）".into()
            } else {
                name_of(accounts, raw)
            }
        }
        "tax_code" => tax_label(raw).into(),
        "business_pct" => format!("{}%", current.as_i64().unwrap_or(100)),
        "lines" => list(current)
            .iter()
            .map(|line| {
                format!(
                    "{} {} {}円{}{}",
                    if line["side"] == "debit" {
                        "借方"
                    } else {
                        "貸方"
                    },
                    name_of(accounts, text(line, "account")),
                    number(integer(line, "amount")),
                    if text(line, "tax_code").is_empty() {
                        String::new()
                    } else {
                        format!("（{}）", tax_label(text(line, "tax_code")))
                    },
                    if text(line, "memo").is_empty() {
                        String::new()
                    } else {
                        format!(" {}", text(line, "memo"))
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => {
            if raw.is_empty() {
                "（なし）".into()
            } else {
                raw.into()
            }
        }
    }
}
pub(crate) fn diff_rows(
    before: &Value,
    after: &Value,
    journal: bool,
    accounts: &[Value],
) -> Vec<(String, String, String)> {
    changed_fields(before, after)
        .into_iter()
        .filter(|(key, _)| !journal || matches!(*key, "date" | "description" | "partner" | "lines"))
        .map(|(key, label)| {
            (
                label.into(),
                diff_value(before, key, accounts),
                diff_value(after, key, accounts),
            )
        })
        .collect()
}
pub(crate) fn diff_table(rows: &[(String, String, String)], caption: &str) -> Element {
    let table = h("table", "table diff-table", "");
    let head = h("tr", "", "");
    for label in ["欄", "変更前", "変更後"] {
        let th = h("th", "", label);
        attrs(&th, &[("scope", "col")]);
        append(&head, &[th]);
    }
    let thead = h("thead", "", "");
    append(&thead, &[head]);
    let tbody = h("tbody", "", "");
    for (label, before, after) in rows {
        let row = h("tr", "", "");
        let th = h("th", "", label);
        attrs(&th, &[("scope", "row")]);
        append(
            &row,
            &[
                th,
                h("td", "wrap diff-before", before),
                h("td", "wrap diff-after", after),
            ],
        );
        append(&tbody, &[row]);
    }
    append(&table, &[h("caption", "", caption), thead, tbody]);
    let wrap = h("div", "table-wrap", "");
    append(&wrap, &[table]);
    wrap
}
pub(crate) async fn correction(
    ctx: &Context,
    entry: &Value,
    rows: &[(String, String, String)],
    reason: &str,
) -> bool {
    let version = integer(entry, "revision");
    let reason_el = h("p", "", "");
    append(
        &reason_el,
        &[h("strong", "", "理由: "), h("span", "", reason)],
    );
    ask(
        ctx,
        &format!("No.{} を訂正します", integer(entry, "no")),
        vec![
            h(
                "p",
                "",
                &format!(
                    "第{version}版を残したまま、第{}版を追記します。元の記録は履歴に残ります。",
                    version + 1
                ),
            ),
            diff_table(rows, "変わる欄"),
            reason_el,
        ],
        "戻って直す",
        "訂正を保存",
        false,
        true,
    )
    .await
}

pub(crate) async fn choose_template(
    ctx: &Context,
    templates: &[Value],
    accounts: &[Value],
) -> Option<Value> {
    let dialog = Dialog::new(ctx, "テンプレートから入力");
    let selected = Rc::new(RefCell::new(None));
    let result = ResultCell::new();
    if templates.is_empty() {
        let empty = h("div", "empty", "");
        append(
            &empty,
            &[
                h("h3", "empty-title", "テンプレートはまだありません"),
                h(
                    "p",
                    "muted",
                    "取引を入れてから「テンプレートに登録」を押すと、ここから呼び出せます。",
                ),
            ],
        );
        append(&dialog.body, &[empty]);
    } else {
        let ul = h("ul", "tpl-list", "");
        for template in templates {
            let item = button("", "");
            item.set_class_name("tpl-item");
            let body = &template["body"];
            let mut parts = vec![kind_label(text(body, "kind")).to_owned()];
            if let Some(amount) = body["amount"].as_i64() {
                parts.push(format!("{}円", number(amount)));
            }
            for key in ["account", "wallet"] {
                if !text(body, key).is_empty() {
                    parts.push(name_of(accounts, text(body, key)));
                }
            }
            if !text(body, "partner").is_empty() {
                parts.push(text(body, "partner").into());
            }
            let content = h("span", "tpl-text", "");
            append(
                &content,
                &[
                    h("span", "tpl-name", text(template, "name")),
                    h("span", "tpl-sum", &parts.join("・")),
                ],
            );
            append(&item, &[icon("bookmark"), content]);
            let li = h("li", "", "");
            append(&li, std::slice::from_ref(&item));
            append(&ul, &[li]);
            let chosen = selected.clone();
            let value = template.clone();
            let d = dialog.clone();
            dialog.scope.on(&item, "click", move |_| {
                *chosen.borrow_mut() = Some(value.clone());
                d.close();
            });
        }
        append(&dialog.body, &[ul]);
    }
    let close = button("閉じる", "");
    append(&dialog.footer, std::slice::from_ref(&close));
    let d = dialog.clone();
    dialog.scope.on(&close, "click", move |_| d.close());
    let selected = selected.clone();
    let answer = result.clone();
    dialog.scope.on(dialog.el.as_ref(), "close", move |_| {
        answer.resolve(selected.borrow().clone());
    });
    dialog.show();
    let first = dialog
        .body
        .query_selector("button")
        .ok()
        .flatten()
        .unwrap_or(close);
    focus(&first);
    result.wait().await
}

pub(crate) async fn save_template(ctx: &Context, body: Value) -> bool {
    let dialog = Dialog::new(ctx, "テンプレートに登録");
    let name = input("text", "");
    attrs(
        name.as_ref(),
        &[("placeholder", "例: 携帯電話の利用料"), ("maxlength", "60")],
    );
    let field = Rc::new(field(
        "テンプレートの名前",
        name.as_ref(),
        "日付は入りません。呼び出すと、それ以外の欄が入ります。",
        true,
    ));
    append(&dialog.body, std::slice::from_ref(&field.el));
    let cancel = button("やめる", "");
    let save = button("登録する", "primary");
    append(&dialog.footer, &[cancel.clone(), save.clone()]);
    let d = dialog.clone();
    dialog.scope.on(&cancel, "click", move |_| d.close());
    let result = ResultCell::new();
    let saved = Rc::new(Cell::new(false));
    let running = Rc::new(Cell::new(false));
    let saved_close = saved.clone();
    let answer = result.clone();
    dialog.scope.on(dialog.el.as_ref(), "close", move |_| {
        answer.resolve(saved_close.get());
    });
    let mut rest = body;
    if let Some(map) = rest.as_object_mut() {
        map.remove("date");
        map.remove("client_id");
    }
    let ctx_save = ctx.clone();
    let d = dialog.clone();
    let field_save = field.clone();
    let name_save = name.clone();
    let submit = Rc::new(move || {
        if running.get() {
            return;
        }
        let name_text = name_save.value().trim().to_owned();
        if name_text.is_empty() {
            field_save.set_error("名前を入れてください。");
            focus(name_save.as_ref());
            return;
        }
        running.set(true);
        d.set_busy(true);
        let context = ctx_save.clone();
        let d = d.clone();
        let field = field_save.clone();
        let running = running.clone();
        let saved = saved.clone();
        let body = json!({"name":name_text,"body":rest});
        ctx_save.task(async move {
            let result = context.post("/templates", &body).await;
            running.set(false);
            if !context.alive() || !d.el.open() {
                return Ok(());
            }
            d.set_busy(false);
            match result {
                Ok(_) => {
                    saved.set(true);
                    context.invalidate(&["templates"]);
                    context.success(&format!("テンプレート「{name_text}」を登録しました。"));
                    d.close();
                }
                Err(error) => {
                    if error.field.as_deref() == Some("name") {
                        field.set_error(&error.message);
                    } else {
                        d.set_error(&error.message);
                    }
                }
            }
            Ok(())
        });
    });
    let submit_click = submit.clone();
    dialog.scope.on(&save, "click", move |_| submit_click());
    dialog.scope.on(name.as_ref(), "keydown", move |event| {
        let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
            return;
        };
        if ime_ignore(&event) {
            return;
        }
        if event.key() == "Enter" {
            event.prevent_default();
            submit();
        }
    });
    dialog.show();
    focus(name.as_ref());
    result.wait().await
}
