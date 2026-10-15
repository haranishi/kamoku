// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::core::dom::{document, window};
use crate::app::prelude::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, File, HtmlInputElement, HtmlSelectElement};
mod check;
mod columns;
mod file;
mod loading;
mod model;
mod sample;
use model::{
    CSV_LIMIT, array, column_count, column_examples, column_options, date_warning, delimiter_label,
    describe_source, encoding_label, group, initial_mapping, is_number_column, mapping_problems,
    roles_of, sample_lines, sample_skipped, summary_text, text,
};
const STEPS: [&str; 3] = ["口座とファイル", "列の対応", "確かめて取り込む"];
const EXPIRED: &str = "時間が経ったため、読み込んだファイルの期限が切れました（30分）。もう一度ファイルを選んでください。";
const LAST_WALLET: &str = "kamoku.import.wallet";
struct State {
    ctx: Context,
    stepper: Element,
    body: Element,
    top_error: Element,
    wallets: RefCell<Vec<Value>>,
    wallet: RefCell<String>,
    filename: RefCell<String>,
    bytes: RefCell<Vec<u8>>,
    encoding: RefCell<String>,
    upload: RefCell<Value>,
    mapping: RefCell<Value>,
    preview: RefCell<Value>,
    step_ctx: RefCell<Option<Context>>,
    seq: Cell<u64>,
    busy: Cell<bool>,
}
impl State {
    fn begin_step(&self) -> Context {
        if let Some(ctx) = self.step_ctx.borrow_mut().take() {
            ctx.scope.dispose();
        }
        self.seq.set(self.seq.get() + 1);
        self.busy.set(false);
        let ctx = self.ctx.child();
        *self.step_ctx.borrow_mut() = Some(ctx.clone());
        ctx
    }
    fn show(&self, step: usize, content: Element, initial_focus: Option<&Element>) {
        self.paint_stepper(step);
        replace(&self.top_error, &[]);
        let heading = h(
            "h2",
            "card-title",
            &format!("{}. {}", step + 1, STEPS[step]),
        );
        attrs(&heading, &[("tabindex", "-1")]);
        let section = node("section", "card import-card", &[heading.clone(), content]);
        attrs(&section, &[("aria-label", STEPS[step])]);
        replace(&self.body, &[section]);
        window().scroll_to_with_x_and_y(0., 0.);
        focus(initial_focus.unwrap_or(&heading));
    }
    fn paint_stepper(&self, step: usize) {
        replace(&self.stepper, &[]);
        for (i, label) in STEPS.iter().enumerate() {
            let el = node(
                "li",
                &format!(
                    "step{}{}",
                    if i < step { " is-done" } else { "" },
                    if i == step { " is-current" } else { "" }
                ),
                &[
                    h(
                        "span",
                        "step-no",
                        if i < step {
                            "✓"
                        } else {
                            match i {
                                0 => "1",
                                1 => "2",
                                _ => "3",
                            }
                        },
                    ),
                    h("span", "step-label", label),
                ],
            );
            if i == step {
                attrs(&el, &[("aria-current", "step")]);
            }
            if i < step {
                append(&el, &[h("span", "visually-hidden", "（済み）")]);
            }
            append(&self.stepper, &[el]);
        }
    }
    fn fail(self: &Rc<Self>, error: &ApiError, first: bool) {
        if !self.ctx.alive() {
            return;
        }
        if error.status == 404 {
            *self.upload.borrow_mut() = Value::Null;
            self.step_file(Some(EXPIRED));
            return;
        }
        let banner = node(
            "div",
            "banner banner--danger",
            &[
                h(
                    "h2",
                    "banner-title",
                    if first {
                        "ファイルを読めませんでした"
                    } else {
                        "取り込めませんでした"
                    },
                ),
                h("p", "", &error.message),
            ],
        );
        attrs(&banner, &[("role", "alert")]);
        replace(&self.top_error, std::slice::from_ref(&banner));
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Nearest);
        banner.scroll_into_view_with_scroll_into_view_options(&options);
    }
    fn start_upload(self: &Rc<Self>, notify: bool, control: HtmlSelectElement) {
        if self.busy.replace(true) {
            return;
        }
        control.set_disabled(true);
        let filename = self.filename.borrow().clone();
        let encoding = self.encoding.borrow().clone();
        let wallet = self.wallet.borrow().clone();
        let bytes = self.bytes.borrow().clone();
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let state = self.clone();
        self.ctx.task(async move {
            let result = state
                .ctx
                .upload(
                    "/imports/upload",
                    &[
                        ("name", &filename),
                        ("encoding", &encoding),
                        ("wallet", &wallet),
                    ],
                    &bytes,
                )
                .await;
            if !state.ctx.alive() || seq != state.seq.get() {
                return Ok(());
            }
            state.busy.set(false);
            control.set_disabled(false);
            match result {
                Ok(up) => {
                    *state.upload.borrow_mut() = up;
                    state.step_columns(None);
                    if notify {
                        state.ctx.info("文字コードを変えて、読み直しました。");
                    }
                }
                Err(err) => state.fail(&err, true),
            }
            Ok(())
        });
    }
    fn preview(self: &Rc<Self>, mapping: &Value) {
        if self.busy.replace(true) {
            return;
        }
        self.mapping.borrow_mut().clone_from(mapping);
        let body = json!({
            "upload_id": self.upload.borrow()["upload_id"],
            "wallet": self.wallet.borrow().as_str(),
            "mapping": mapping,
        });
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let state = self.clone();
        self.ctx.task(async move {
            let result = state.ctx.post("/imports/preview", &body).await;
            if !state.ctx.alive() || seq != state.seq.get() {
                return Ok(());
            }
            state.busy.set(false);
            match result {
                Ok(preview) => {
                    *state.preview.borrow_mut() = preview;
                    state.step_check();
                }
                Err(err) => state.fail(&err, false),
            }
            Ok(())
        });
    }
    fn commit(self: &Rc<Self>, force: &BTreeSet<u64>, save_profile: bool, button: Element) {
        if self.ctx.is_read_only() {
            self.ctx.error(&self.ctx.read_only_reason());
            return;
        }
        if self.busy.replace(true) {
            return;
        }
        disabled(&button, true);
        let wallet = self.wallet.borrow().clone();
        let body = json!({
            "upload_id": self.upload.borrow()["upload_id"],
            "wallet": wallet,
            "mapping": *self.mapping.borrow(),
            "save_profile": save_profile,
            "force_lines": force,
        });
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let state = self.clone();
        self.ctx.task(async move {
            let result = state.ctx.post("/imports/commit", &body).await;
            let current = state.ctx.alive() && seq == state.seq.get();
            if current {
                state.busy.set(false);
            }
            match result {
                Ok(res) => {
                    if let Ok(Some(storage)) = window().local_storage() {
                        let _ = storage.set_item(LAST_WALLET, &wallet);
                    }
                    state.ctx.invalidate(&["status"]);
                    if current {
                        state.ctx.notice(
                            "success",
                            &format!(
                                "{}件を未確認に入れました。",
                                res["created"].as_u64().unwrap_or(0)
                            ),
                            Some((
                                "未確認を開く",
                                &build_hash("/inbox", &[("wallet", &wallet)]),
                            )),
                        );
                        state.result(&res);
                    }
                    let _ = state.ctx.load("status", true).await;
                }
                Err(err) if current => {
                    disabled(&button, false);
                    state.fail(&err, false);
                }
                Err(err) => state.ctx.error(&err.message),
            }
            Ok(())
        });
    }
}
fn node(tag: &str, class: &str, children: &[Element]) -> Element {
    let el = h(tag, class, "");
    append(&el, children);
    el
}
fn hidden(el: &Element, on: bool) {
    if on {
        attrs(el, &[("hidden", "")]);
    } else {
        let _ = el.remove_attribute("hidden");
    }
}
fn class(el: &Element, name: &str, on: bool) {
    let _ = el.class_list().toggle_with_force(name, on);
}
fn described(el: &Element, id: &str) {
    let mut ids = el.get_attribute("aria-describedby").unwrap_or_default();
    if !ids.is_empty() {
        ids.push(' ');
    }
    ids.push_str(id);
    attrs(el, &[("aria-describedby", &ids)]);
}
fn wrap_hint(field: &Field) {
    if let Ok(Some(hint)) = field.el.query_selector(".field-hint") {
        let text = hint.text_content().unwrap_or_default();
        replace(&hint, &[phrase(&text)]);
    }
}
fn table(ctx: &Context, caption: &str, class: &str, headers: &[&str], rows: &[Element]) -> Element {
    let head = h("tr", "", "");
    for label in headers {
        let th = h("th", "", label);
        attrs(&th, &[("scope", "col")]);
        append(&head, &[th]);
    }
    let inner = node(
        "table",
        &format!("table table--stack {class}"),
        &[
            h("caption", "", caption),
            node("thead", "", &[head]),
            node("tbody", "", rows),
        ],
    );
    let wrapper = node("div", "table-wrap", std::slice::from_ref(&inner));
    attrs(&wrapper, &[("role", "region"), ("aria-label", caption)]);
    focus_when_scrollable(ctx, &wrapper, &inner);
    wrapper
}

#[allow(
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Assemble the page under the shared mount(Context) -> Result<Screen, JsValue> contract."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let root = h("div", "import-page", "");
    let heading = h("h1", "", "明細を取り込む");
    attrs(&heading, &[("tabindex", "-1")]);
    let stepper = h("ol", "stepper", "");
    attrs(&stepper, &[("aria-label", "取込の段階")]);
    let body = h("div", "import-body", "");
    let top_error = h("div", "", "");
    append(
        &root,
        &[
            node(
                "div",
                "page-head",
                &[node(
                    "div",
                    "page-head-text",
                    &[
                        heading,
                        node(
                            "p",
                            "muted small",
                            &[phrase(
                                "銀行やカードの明細のCSVを、未確認に並べます。確定するまで、仕訳帳と帳票には入りません。",
                            )],
                        ),
                    ],
                )],
            ),
            stepper.clone(),
            top_error.clone(),
            body.clone(),
        ],
    );
    let state = Rc::new(State {
        ctx: ctx.clone(),
        stepper,
        body,
        top_error,
        wallets: RefCell::new(vec![]),
        wallet: RefCell::new(ctx.route.query("wallet")),
        filename: RefCell::new(String::new()),
        bytes: RefCell::new(vec![]),
        encoding: RefCell::new("auto".into()),
        upload: RefCell::new(Value::Null),
        mapping: RefCell::new(Value::Null),
        preview: RefCell::new(Value::Null),
        step_ctx: RefCell::new(None),
        seq: Cell::new(0),
        busy: Cell::new(false),
    });
    let s = state.clone();
    let loading = loading::Loading::new(&ctx, &s.body);
    ctx.task(async move {
        let result = s.ctx.load("accounts", true).await;
        drop(loading);
        if !s.ctx.alive() {
            return Ok(());
        }
        let data = match result {
            Ok(data) => data,
            Err(error) => {
                replace(
                    &s.body,
                    &[node(
                        "div",
                        "banner banner--danger",
                        &[
                            h("h2", "banner-title", "取込の画面を準備できませんでした"),
                            h("p", "", &error.message),
                        ],
                    )],
                );
                return Ok(());
            }
        };
        *s.wallets.borrow_mut() = array(&data)
            .iter()
            .filter(|a| a["wallet"].is_object() && a["enabled"] != false)
            .cloned()
            .collect();
        let usable = |code: &str| s.wallets.borrow().iter().any(|w| text(w, "code") == code);
        if !usable(&s.wallet.borrow()) {
            let last = window()
                .local_storage()
                .ok()
                .flatten()
                .and_then(|storage| storage.get_item(LAST_WALLET).ok().flatten())
                .unwrap_or_default();
            *s.wallet.borrow_mut() = if usable(&last) { last } else { String::new() };
        }
        if s.wallets.borrow().is_empty() {
            replace(
                &s.body,
                &[node(
                    "div",
                    "banner banner--warn",
                    &[
                        h("h2", "banner-title", "口座がまだありません"),
                        h("p", "", "明細を取り込む前に、口座を足してください。"),
                        link("口座の画面へ", "#/wallets", "btn btn--sm"),
                    ],
                )],
            );
        } else {
            s.step_file(None);
        }
        Ok(())
    });
    let mut screen = Screen::new(&ctx, root);
    screen.manages_focus = true;
    screen.update = Some(Rc::new(move |next| {
        let _ = &state;
        next.route.path == "/import"
    }));
    Ok(screen)
}
