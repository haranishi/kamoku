// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

mod compound;
pub(crate) mod controls;
pub(crate) mod dialogs;
pub(crate) mod model;
pub(crate) mod preview;
mod simple;

use self::{
    compound::Compound,
    controls::{hidden, shortcut},
    dialogs::{ask, choose_template, correction, diff_rows, diff_table, save_template},
    model::{date_concern, integer, journal_input, list, text},
    preview::Preview,
    simple::Simple,
};
use super::journal::return_path::{detail_hash, encode, from_back_param};
use crate::app::prelude::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Element, HtmlInputElement, KeyboardEvent};

#[derive(Clone)]
struct Data {
    settings: Value,
    accounts: Vec<Value>,
    partners: Vec<Value>,
    templates: Vec<Value>,
}
enum Form {
    Simple(Rc<Simple>),
    Compound(Rc<Compound>),
}
impl Form {
    fn collect(&self) -> Option<Value> {
        match self {
            Self::Simple(form) => form.collect(),
            Self::Compound(form) => form.collect(),
        }
    }
    fn problems(&self) -> Vec<String> {
        match self {
            Self::Simple(form) => form.problems(),
            Self::Compound(form) => form.problems(),
        }
    }
    fn focus_date(&self) {
        match self {
            Self::Simple(form) => focus(form.date.input.as_ref()),
            Self::Compound(form) => focus(form.date.input.as_ref()),
        }
    }
    fn focus_amount(&self) {
        match self {
            Self::Simple(form) => {
                let options = web_sys::ScrollIntoViewOptions::new();
                options.set_block(web_sys::ScrollLogicalPosition::Nearest);
                form.date
                    .field
                    .el
                    .scroll_into_view_with_scroll_into_view_options(&options);
                focus(form.amount.input.as_ref());
            }
            Self::Compound(form) => form.focus_amount(),
        }
    }
    fn error(&self, name: &str, message: &str) -> bool {
        match self {
            Self::Simple(form) => form.show_error(name, message),
            Self::Compound(form) => form.show_error(name, message),
        }
    }
    fn locked_note(&self, on_save: bool) -> String {
        match self {
            Self::Simple(form) => form.locked_note(on_save),
            Self::Compound(form) => form.locked_note(on_save),
        }
    }
}
type AnnounceTimer = (i32, Closure<dyn FnMut()>);

struct Entry {
    ctx: Context,
    id: String,
    after_hash: String,
    title: Element,
    subtitle: Element,
    body: Element,
    form_host: Element,
    stale: Element,
    reason: HtmlInputElement,
    reason_field: Field,
    preview: Rc<Preview>,
    error: Element,
    block_note: Element,
    save: Element,
    save_only: Option<Element>,
    template_save: Option<Element>,
    template_foot: Element,
    simple: RefCell<Option<Rc<Simple>>>,
    compound: RefCell<Option<Rc<Compound>>>,
    journal: Cell<bool>,
    data: RefCell<Option<Data>>,
    original: RefCell<Value>,
    busy: Cell<bool>,
    submitting: Cell<bool>,
    canceling: Cell<bool>,
    client_id: RefCell<String>,
    voided: Cell<bool>,
    announced: Cell<bool>,
    announce_seq: Cell<u64>,
    announce_timer: RefCell<Option<AnnounceTimer>>,
    tabs: RefCell<Vec<Element>>,
    panel: RefCell<Option<Element>>,
    last_field: RefCell<Option<Element>>,
}
impl Entry {
    fn restore_field(&self) {
        let field = self
            .last_field
            .borrow()
            .clone()
            .filter(|field| field.is_connected());
        if let Some(field) = field {
            focus(&field);
        }
    }
    fn form(&self) -> Option<Form> {
        if self.journal.get() {
            self.compound.borrow().clone().map(Form::Compound)
        } else {
            self.simple.borrow().clone().map(Form::Simple)
        }
    }
    fn dirty(&self) -> bool {
        self.simple
            .borrow()
            .as_ref()
            .is_some_and(|form| form.dirty())
            || self
                .compound
                .borrow()
                .as_ref()
                .is_some_and(|form| form.dirty())
            || !self.id.is_empty() && !self.reason.value().trim().is_empty()
    }
    fn set_error(&self, message: &str) {
        self.announce_seq.set(self.announce_seq.get() + 1);
        self.announced.set(false);
        set_text(&self.error, message);
    }
    fn clear_announced(&self) {
        self.announce_seq.set(self.announce_seq.get() + 1);
        if self.announced.replace(false) {
            set_text(&self.error, "");
        }
    }
    fn announce(self: &Rc<Self>, names: &[String]) {
        self.set_error("");
        if names.is_empty() {
            return;
        }
        let sequence = self.announce_seq.get();
        let message = format!("{}か所を直してください：{}", names.len(), names.join("、"));
        let weak = Rc::downgrade(self);
        if let Some((id, _)) = self.announce_timer.borrow_mut().take() {
            web_sys::window()
                .expect("window")
                .clear_timeout_with_handle(id);
        }
        let callback = Closure::wrap(Box::new(move || {
            if let Some(this) = weak
                .upgrade()
                .filter(|this| this.ctx.alive() && this.announce_seq.get() == sequence)
            {
                this.announced.set(true);
                set_text(&this.error, &message);
            }
        }) as Box<dyn FnMut()>);
        let id = web_sys::window()
            .expect("window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                100,
            )
            .expect("timer");
        *self.announce_timer.borrow_mut() = Some((id, callback));
    }
    fn update_save(&self) {
        let read_only = self.ctx.is_read_only();
        let reason = if read_only {
            self.ctx.read_only_reason()
        } else if self.voided.get() {
            "この仕訳は取消済みのため、訂正できません。".into()
        } else if self.preview.failed() {
            "プレビューを作れないため、保存を止めています。".into()
        } else if self.journal.get()
            && self
                .compound
                .borrow()
                .as_ref()
                .is_some_and(|form| !form.balanced())
        {
            "貸借が合うまで、保存できません。".into()
        } else {
            String::new()
        };
        set_text(&self.block_note, &reason);
        for button in std::iter::once(&self.save)
            .chain(self.save_only.iter())
            .chain(self.template_save.iter())
        {
            disabled(
                button,
                !reason.is_empty() || self.busy.get() || self.submitting.get(),
            );
            if reason.is_empty() {
                let _ = button.remove_attribute("aria-describedby");
            } else {
                attrs(
                    button,
                    &[(
                        "aria-describedby",
                        if read_only {
                            "banner-readonly entry-block-note"
                        } else {
                            "entry-block-note"
                        },
                    )],
                );
            }
        }
        hidden(&self.template_foot, self.journal.get());
    }
    fn bind_simple(self: &Rc<Self>, form: &Rc<Simple>) {
        let weak = Rc::downgrade(self);
        form.on_change(move || {
            if let Some(this) = weak.upgrade() {
                this.clear_announced();
                this.update_save();
            }
        });
        let weak = Rc::downgrade(self);
        form.on_submit(move || {
            if let Some(this) = weak.upgrade() {
                this.submit(true);
            }
        });
        let weak = Rc::downgrade(self);
        form.on_cancel(move || {
            if let Some(this) = weak.upgrade() {
                this.cancel();
            }
        });
    }
    fn show_compound(self: &Rc<Self>, initial: &Value, keep_focus: bool) {
        let Some(data) = self.data.borrow().clone() else {
            return;
        };
        if let Some(form) = self.simple.borrow().as_ref() {
            form.set_active(false);
        }
        self.preview.cancel();
        let form = Compound::new(
            &self.ctx,
            &data.accounts,
            &data.partners,
            &data.settings,
            initial,
            self.preview.clone(),
            !self.id.is_empty(),
        );
        let weak = Rc::downgrade(self);
        form.on_change(move || {
            if let Some(this) = weak.upgrade() {
                this.clear_announced();
                this.update_save();
            }
        });
        let weak = Rc::downgrade(self);
        form.on_submit(move || {
            if let Some(this) = weak.upgrade() {
                this.submit(true);
            }
        });
        let weak = Rc::downgrade(self);
        form.on_cancel(move || {
            if let Some(this) = weak.upgrade() {
                this.cancel();
            }
        });
        replace(&self.form_host, std::slice::from_ref(&form.el));
        *self.compound.borrow_mut() = Some(form.clone());
        self.journal.set(true);
        form.set_active(true);
        self.preview.run();
        if !keep_focus {
            focus(form.date.input.as_ref());
        }
        self.update_save();
    }
    fn switch(self: &Rc<Self>, journal: bool, keep_focus: bool) {
        if self.journal.get() == journal {
            return;
        }
        self.clear_announced();
        if journal {
            let seed = self
                .simple
                .borrow()
                .as_ref()
                .map_or(Value::Null, |form| form.journal_seed());
            self.show_compound(&seed, keep_focus);
        } else {
            self.preview.cancel();
            if let Some(form) = self.compound.borrow().as_ref() {
                form.set_active(false);
            }
            self.compound.borrow_mut().take();
            self.journal.set(false);
            if let Some(form) = self.simple.borrow().as_ref() {
                replace(&self.form_host, std::slice::from_ref(&form.el));
                form.set_active(true);
                self.preview.run();
            }
            self.update_save();
        }
        self.sync_tabs();
    }
    fn sync_tabs(&self) {
        for (index, tab) in self.tabs.borrow().iter().enumerate() {
            let on = (index == 1) == self.journal.get();
            attrs(
                tab,
                &[
                    ("aria-selected", if on { "true" } else { "false" }),
                    ("tabindex", if on { "0" } else { "-1" }),
                ],
            );
            if on && let Some(panel) = self.panel.borrow().as_ref() {
                attrs(panel, &[("aria-labelledby", &tab.id())]);
            }
        }
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Assembles the input tabs, correction fields, preview rail and save-bar DOM."
    )]
    fn build(self: &Rc<Self>) {
        let Some(data) = self.data.borrow().clone() else {
            return;
        };
        let correcting = !self.id.is_empty();
        let entry = self.original.borrow().clone();
        if correcting && entry["state"] == "voided" {
            let box_el = h("div", "banner banner--danger", "");
            append(
                &box_el,
                &[
                    h("h2", "", "この仕訳は取消済みです"),
                    h("p", "", "取消済みの仕訳は訂正できません。"),
                    link("仕訳の詳細に戻る", &self.after_hash, ""),
                ],
            );
            replace(&self.body, &[box_el]);
            focus(&self.title);
            return;
        }
        let main = h("section", "card entry-main", "");
        attrs(
            &main,
            &[(
                "aria-label",
                if correcting {
                    "訂正の入力"
                } else {
                    "取引の入力"
                },
            )],
        );
        if correcting {
            set_text(
                &self.subtitle,
                &format!(
                    "No.{} の仕訳（いまは第{}版）。元の記録は履歴に残ります。",
                    integer(&entry, "no"),
                    integer(&entry, "revision")
                ),
            );
            append(
                &main,
                &[
                    self.stale.clone(),
                    self.form_host.clone(),
                    self.reason_field.el.clone(),
                ],
            );
        } else {
            let tabs = h("div", "tabs", "");
            attrs(&tabs, &[("role", "tablist"), ("aria-label", "入力の形")]);
            let panel = h("div", "tabpanel", "");
            let base = crate::app::core::dom::uid("tabs");
            attrs(
                &panel,
                &[("id", &format!("{base}-panel")), ("role", "tabpanel")],
            );
            for (index, (kind, label)) in [("simple", "かんたん入力"), ("journal", "仕訳で入力")]
                .iter()
                .enumerate()
            {
                let tab = button(label, "");
                tab.set_class_name("tab");
                attrs(
                    &tab,
                    &[
                        ("role", "tab"),
                        ("id", &format!("{base}-{kind}")),
                        ("aria-controls", &panel.id()),
                    ],
                );
                let weak = Rc::downgrade(self);
                self.ctx.on(&tab, "click", move |_| {
                    if let Some(this) = weak.upgrade() {
                        let active = crate::app::core::dom::document().active_element();
                        let keep = active.as_ref().is_some_and(|el| {
                            el.get_attribute("role").as_deref() == Some("tab")
                                && el.matches(":focus-visible").unwrap_or(false)
                        });
                        this.switch(index == 1, keep);
                    }
                });
                append(&tabs, std::slice::from_ref(&tab));
                self.tabs.borrow_mut().push(tab);
            }
            let weak = Rc::downgrade(self);
            self.ctx.on(&tabs, "keydown", move |event| {
                let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                    return;
                };
                if ime_ignore(&event) {
                    return;
                }
                let Some(this) = weak.upgrade() else {
                    return;
                };
                let current = usize::from(this.journal.get());
                let next = match event.key().as_str() {
                    "ArrowLeft" | "ArrowRight" => Some(1 - current),
                    "Home" => Some(0),
                    "End" => Some(1),
                    _ => None,
                };
                if let Some(next) = next {
                    event.prevent_default();
                    if let Some(tab) = this.tabs.borrow().get(next) {
                        focus(tab);
                    }
                    this.switch(next == 1, true);
                }
            });
            append(&panel, std::slice::from_ref(&self.form_host));
            append(&main, &[tabs, self.stale.clone(), panel.clone()]);
            *self.panel.borrow_mut() = Some(panel);
            self.sync_tabs();
        }
        if correcting && entry["input"].is_null() {
            self.show_compound(&journal_input(&entry), true);
        } else {
            let initial = if correcting {
                entry["input"].clone()
            } else {
                Value::Null
            };
            let form = Simple::new(
                &self.ctx,
                &data.accounts,
                &data.partners,
                &data.settings,
                &initial,
                self.preview.clone(),
                correcting,
            );
            self.bind_simple(&form);
            replace(&self.form_host, std::slice::from_ref(&form.el));
            *self.simple.borrow_mut() = Some(form);
        }
        let rail = h("div", "entry-rail", "");
        let aside = h("aside", "entry-side", "");
        append(&aside, std::slice::from_ref(&self.preview.el));
        let actions = h("div", "entry-actions", "");
        let cancel = button("やめる", "ghost");
        shortcut(&cancel, "Esc", "Escape");
        let weak = Rc::downgrade(self);
        self.ctx.on(&cancel, "click", move |_| {
            if let Some(this) = weak.upgrade() {
                this.cancel();
            }
        });
        let mut buttons = vec![cancel];
        if let Some(save) = &self.save_only {
            buttons.push(save.clone());
        }
        buttons.push(self.save.clone());
        append(&actions, &buttons);
        let bar = h("section", "entry-actions-card", "");
        attrs(&bar, &[("data-bottom-bar", ""), ("aria-label", "保存")]);
        append(
            &bar,
            &[self.error.clone(), self.block_note.clone(), actions],
        );
        append(&rail, &[aside, bar, self.template_foot.clone()]);
        replace(&self.body, &[main, rail]);
        self.update_save();
        if crate::app::core::dom::document()
            .query_selector("dialog[open]")
            .ok()
            .flatten()
            .is_none()
            && let Some(form) = self.form()
        {
            if correcting {
                form.focus_amount();
            } else {
                form.focus_date();
            }
        }
    }
    fn submit(self: &Rc<Self>, cont: bool) {
        if self.busy.get() || self.submitting.get() {
            return;
        }
        let Some(form) = self.form() else {
            return;
        };
        self.set_error("");
        if self.ctx.is_read_only() {
            self.set_error(&self.ctx.read_only_reason());
            return;
        }
        let Some(input) = form.collect() else {
            self.announce(&form.problems());
            return;
        };
        if self.preview.failed() {
            self.set_error("仕訳のプレビューを作れないため、保存を止めています。サーバーが動いているかを確かめて、プレビューの「もう一度試す」を押してください。");
            return;
        }
        self.submitting.set(true);
        self.update_save();
        let this = self.clone();
        self.ctx.task(async move {
            this.submit_inner(input, cont).await;
            this.submitting.set(false);
            this.busy.set(false);
            if this.ctx.alive() {
                this.update_save();
            }
            Ok(())
        });
    }
    async fn date_checked(&self, input: &Value, settings: &Value, entry: &Value) -> bool {
        let concern = date_concern(
            text(input, "date"),
            &today(),
            text(settings, "opened_on"),
            if self.id.is_empty() {
                ""
            } else {
                text(entry, "date")
            },
        );
        if concern.is_empty() {
            return true;
        }
        let accepted = ask(
            &self.ctx,
            "日付は合っていますか",
            vec![h("p", "", &concern)],
            "直す",
            "このまま保存",
            false,
            false,
        )
        .await;
        if !accepted
            && self.ctx.alive()
            && let Some(form) = self.form()
        {
            form.focus_date();
        }
        accepted
    }

    async fn submit_inner(self: &Rc<Self>, mut input: Value, cont: bool) {
        let Some(data) = self.data.borrow().clone() else {
            return;
        };
        let entry = self.original.borrow().clone();
        let correcting = !self.id.is_empty();
        let reason = self.reason.value().trim().to_owned();
        let rows = if correcting {
            let before = if self.journal.get() {
                journal_input(&entry)
            } else {
                entry["input"].clone()
            };
            let rows = diff_rows(&before, &input, self.journal.get(), &data.accounts);
            if rows.is_empty() {
                self.set_error(
                    "変わった欄がありません。直したい欄を変えてから、保存してください。",
                );
                return;
            }
            if reason.is_empty() {
                self.reason_field.set_error("訂正の理由を入れてください。");
                focus(self.reason.as_ref());
                return;
            }
            self.reason_field.set_error("");
            rows
        } else {
            Vec::new()
        };
        if !self.date_checked(&input, &data.settings, &entry).await || !self.ctx.alive() {
            return;
        }
        if correcting && !correction(&self.ctx, &entry, &rows, &reason).await {
            return;
        }
        if !self.ctx.alive() {
            return;
        }
        self.busy.set(true);
        self.update_save();
        let result = if correcting {
            input["reason"] = json!(reason);
            input["expected_revision"] = entry["revision"].clone();
            self.ctx
                .put(&format!("/entries/{}", encode(&self.id)), &input)
                .await
        } else {
            input["client_id"] = json!(self.client_id.borrow().clone());
            self.ctx.post("/entries", &input).await
        };
        if !self.ctx.alive() {
            return;
        }
        match result {
            Ok(response) => self.show_saved(&response, &input, cont),
            Err(error) if correcting && error.is_stale() => {
                self.reload_after_stale(&input, &data.accounts).await;
            }
            Err(error) => self.show_save_error(&error),
        }
    }

    fn show_saved(self: &Rc<Self>, response: &Value, input: &Value, cont: bool) {
        self.ctx.invalidate(&["partners", "status"]);
        let ctx = self.ctx.clone();
        self.ctx.task(async move {
            ctx.load("status", true).await?;
            Ok(())
        });
        if !self.id.is_empty() {
            self.ctx.notice(
                "success",
                &format!(
                    "No.{} を訂正しました（第{}版）。",
                    integer(response, "no"),
                    integer(response, "revision")
                ),
                Some(("仕訳を見る", &self.after_hash)),
            );
            self.ctx.allow_leave();
            self.ctx.navigate(&self.after_hash, false);
            return;
        }
        *self.client_id.borrow_mut() = new_client_id();
        if let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        {
            let _ = storage.set_item("kamoku.entry.wallet", text(input, "wallet"));
        }
        let detail = detail_hash(text(response, "id"), "");
        self.ctx.notice(
            "success",
            &format!("No.{} を登録しました。", integer(response, "no")),
            Some(("仕訳を見る", &detail)),
        );
        if !cont {
            self.ctx.allow_leave();
            self.ctx.navigate(&detail, false);
        } else if self.journal.get() {
            if let Some(form) = self.simple.borrow().as_ref() {
                form.reset();
            }
            self.show_compound(&json!({"date":input["date"]}), false);
        } else if let Some(form) = self.simple.borrow().as_ref() {
            form.reset();
            let options = web_sys::ScrollIntoViewOptions::new();
            options.set_block(web_sys::ScrollLogicalPosition::Nearest);
            form.date
                .field
                .el
                .scroll_into_view_with_scroll_into_view_options(&options);
            focus(form.amount.input.as_ref());
        }
    }

    async fn reload_after_stale(&self, input: &Value, accounts: &[Value]) {
        self.ctx
            .info("他の場所で更新されました。読み込み直します。");
        let latest = match self
            .ctx
            .get(&format!("/entries/{}", encode(&self.id)), &[])
            .await
        {
            Ok(latest) if self.ctx.alive() => latest,
            Ok(_) => return,
            Err(error) => {
                if self.ctx.alive() {
                    self.set_error(&error.message);
                }
                return;
            }
        };
        set_text(
            &self.subtitle,
            &format!(
                "No.{} の仕訳（いまは第{}版）",
                integer(&latest, "no"),
                integer(&latest, "revision")
            ),
        );
        let before = if self.journal.get() {
            journal_input(&latest)
        } else {
            latest["input"].clone()
        };
        let rows = diff_rows(&before, input, self.journal.get(), accounts);
        let block = h(
            "div",
            if latest["state"] == "voided" {
                "banner banner--danger"
            } else {
                "banner banner--warn"
            },
            "",
        );
        if latest["state"] == "voided" {
            append(
                &block,
                &[
                    h("h2", "", "この仕訳は、ほかの場所で取り消されました"),
                    h(
                        "p",
                        "",
                        "取消済みの仕訳は訂正できません。仕訳帳で履歴を確かめてください。",
                    ),
                ],
            );
            self.voided.set(true);
        } else {
            append(
                &block,
                &[
                    h(
                        "h2",
                        "",
                        &format!(
                            "ほかの場所で更新されたため、最新の版（第{}版）を読み込みました",
                            integer(&latest, "revision")
                        ),
                    ),
                    h(
                        "p",
                        "",
                        if rows.is_empty() {
                            "あなたの入力は、最新の版と同じです。直す必要はありません。"
                        } else {
                            "あなたの入力は残しています。下は、最新の版とあなたの入力の違いです。よければ、もう一度保存してください。"
                        },
                    ),
                ],
            );
            if !rows.is_empty() {
                append(&block, &[diff_table(&rows, "最新の版との違い")]);
            }
        }
        *self.original.borrow_mut() = latest;
        replace(&self.stale, &[block]);
        self.stale.scroll_into_view();
    }

    fn show_save_error(&self, error: &ApiError) {
        let message = if error.code == "locked_period" {
            self.form()
                .map(|form| form.locked_note(true))
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| error.message.clone())
        } else if error.is_read_only() {
            self.ctx.read_only_reason()
        } else {
            error.message.clone()
        };
        if error.is_stale() {
            self.set_error("ほかの場所で台帳が更新されたため、保存できませんでした。最新の状態を読み込みました。入力はそのままです。もう一度保存してください。");
        } else if !error
            .field
            .as_ref()
            .is_some_and(|name| self.form().is_some_and(|form| form.error(name, &message)))
        {
            self.set_error(&message);
        }
    }
    fn cancel(self: &Rc<Self>) {
        if self.canceling.replace(true) {
            return;
        }
        let this = self.clone();
        self.ctx.task(async move {
            let leave = !this.dirty()
                || ask(
                    &this.ctx,
                    "入力中の内容を捨てますか",
                    vec![h(
                        "p",
                        "",
                        "まだ保存していない入力があります。捨てると、元に戻せません。",
                    )],
                    "入力を続ける",
                    "捨てて戻る",
                    true,
                    false,
                )
                .await;
            this.canceling.set(false);
            if leave && this.ctx.alive() {
                this.ctx.allow_leave();
                this.ctx.back(if this.id.is_empty() {
                    "#/"
                } else {
                    &this.after_hash
                });
            } else if this.ctx.alive() {
                this.restore_field();
            }
            Ok(())
        });
    }
    fn choose_template(self: &Rc<Self>) {
        let Some(data) = self.data.borrow().clone() else {
            return;
        };
        let this = self.clone();
        self.ctx.task(async move {
            let templates = this
                .ctx
                .load("templates", true)
                .await
                .map(|value| list(&value))
                .unwrap_or(data.templates);
            if !this.ctx.alive() {
                return Ok(());
            }
            if let Some(template) = choose_template(&this.ctx, &templates, &data.accounts).await
                && this.ctx.alive()
            {
                this.switch(false, false);
                if let Some(form) = this.simple.borrow().as_ref() {
                    form.apply_template(&template["body"]);
                    focus(form.amount.input.as_ref());
                }
            }
            Ok(())
        });
    }
    fn save_template(self: &Rc<Self>) {
        let body = if self.journal.get() {
            None
        } else {
            self.simple
                .borrow()
                .as_ref()
                .and_then(|form| form.snapshot())
        };
        let Some(body) = body else {
            self.set_error("テンプレートにするには、金額・科目・口座を入れてください（かんたん入力のときだけ登録できます）。");
            return;
        };
        let ctx = self.ctx.clone();
        self.ctx.task(async move {
            save_template(&ctx, body).await;
            Ok(())
        });
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "Creates the screen DOM and registers its scoped routes, form, keyboard and lifecycle handlers."
)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Matches the router's uniform owned-Context, fallible screen-mount interface."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let id = ctx.route.param("id");
    let correcting = !id.is_empty();
    let after_hash = detail_hash(&id, &from_back_param(&ctx.route.query("back")));
    let root = h("div", "", "");
    let title = h(
        "h1",
        "",
        if correcting {
            "取引を訂正"
        } else {
            "取引を登録"
        },
    );
    attrs(&title, &[("tabindex", "-1"), ("id", "entry-title")]);
    let subtitle = h("p", "muted small", "");
    let head_text = h("div", "page-head-text", "");
    append(&head_text, &[title.clone(), subtitle.clone()]);
    let head = h("div", "page-head entry-head", "");
    let actions = h("div", "page-head-actions", "");
    let template = if correcting {
        None
    } else {
        Some(button("テンプレートから入力", ""))
    };
    if let Some(button) = &template {
        append(&actions, std::slice::from_ref(button));
    }
    append(&head, &[head_text, actions]);
    let body = h("div", "entry-layout", "");
    append(&body, &[h("p", "muted", "入力欄を準備しています")]);
    append(&root, &[head, body.clone()]);
    let reason = input("text", "");
    attrs(
        reason.as_ref(),
        &[("placeholder", "例: 金額の入力ミス"), ("maxlength", "200")],
    );
    let reason_field = field(
        "訂正の理由",
        reason.as_ref(),
        "履歴に残ります。あとで見たときに、直した訳が分かるように書いてください。",
        true,
    );
    let error = h("p", "field-error form-error", "");
    attrs(&error, &[("role", "alert")]);
    let block_note = h("p", "field-hint block-note", "");
    attrs(&block_note, &[("id", "entry-block-note")]);
    let save = button(
        if correcting {
            "訂正を確かめる"
        } else {
            "保存して続ける"
        },
        "primary",
    );
    let _ = save.class_list().add_1("save-main");
    let kbd = h(
        "kbd",
        "btn-kbd",
        &format!(
            "{} Enter",
            if js_sys::Reflect::get(
                &crate::app::core::dom::window().navigator(),
                &"platform".into()
            )
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default()
            .contains("Mac")
            {
                "⌘"
            } else {
                "Ctrl"
            }
        ),
    );
    attrs(&kbd, &[("aria-hidden", "true")]);
    attrs(
        &save,
        &[(
            "aria-keyshortcuts",
            if kbd.text_content().unwrap_or_default().starts_with('⌘') {
                "Meta+Enter"
            } else {
                "Control+Enter"
            },
        )],
    );
    append(&save, &[kbd]);
    let save_only = if correcting {
        None
    } else {
        Some(button("保存", ""))
    };
    let template_save = if correcting {
        None
    } else {
        Some(button("テンプレートに登録", "ghost"))
    };
    let template_foot = h("div", "pv-foot", "");
    if let Some(button) = &template_save {
        let _ = button.class_list().add_1("btn--sm");
        append(&template_foot, std::slice::from_ref(button));
    } else {
        hidden(&template_foot, true);
    }
    let preview = Preview::new(&ctx);
    let state = Rc::new(Entry {
        ctx: ctx.clone(),
        id,
        after_hash,
        title,
        subtitle,
        body,
        form_host: h("div", "entry-form-host", ""),
        stale: h("div", "stale-box", ""),
        reason,
        reason_field,
        preview,
        error,
        block_note,
        save,
        save_only,
        template_save,
        template_foot,
        simple: RefCell::new(None),
        compound: RefCell::new(None),
        journal: Cell::new(false),
        data: RefCell::new(None),
        original: RefCell::new(Value::Null),
        busy: Cell::new(false),
        submitting: Cell::new(false),
        canceling: Cell::new(false),
        client_id: RefCell::new(new_client_id()),
        voided: Cell::new(false),
        announced: Cell::new(false),
        announce_seq: Cell::new(0),
        announce_timer: RefCell::new(None),
        tabs: RefCell::new(Vec::new()),
        panel: RefCell::new(None),
        last_field: RefCell::new(None),
    });
    let weak = Rc::downgrade(&state);
    state.preview.on_change(move || {
        if let Some(state) = weak.upgrade() {
            state.update_save();
        }
    });
    let weak = Rc::downgrade(&state);
    state.preview.on_lines(move |lines| {
        if let Some(state) = weak.upgrade()
            && let Some(form) = state.simple.borrow().as_ref()
        {
            form.paint_lines(&lines);
        }
    });
    let weak = Rc::downgrade(&state);
    ctx.on(&state.save, "click", move |_| {
        if let Some(state) = weak.upgrade() {
            state.submit(true);
        }
    });
    if let Some(save) = &state.save_only {
        let weak = Rc::downgrade(&state);
        ctx.on(save, "click", move |_| {
            if let Some(state) = weak.upgrade() {
                state.submit(false);
            }
        });
    }
    if let Some(button) = &template {
        let weak = Rc::downgrade(&state);
        ctx.on(button, "click", move |_| {
            if let Some(state) = weak.upgrade() {
                state.choose_template();
            }
        });
    }
    if let Some(button) = &state.template_save {
        let weak = Rc::downgrade(&state);
        ctx.on(button, "click", move |_| {
            if let Some(state) = weak.upgrade() {
                state.save_template();
            }
        });
    }
    let weak = Rc::downgrade(&state);
    ctx.on(state.reason.as_ref(), "keydown", move |event| {
        let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
            return;
        };
        if ime_ignore(&event) {
            return;
        }
        if let Some(state) = weak.upgrade() {
            if event.key() == "Enter" {
                event.prevent_default();
                state.submit(true);
            } else if event.key() == "Escape" {
                event.prevent_default();
                state.cancel();
            }
        }
    });
    for event in ["input", "change"] {
        let weak = Rc::downgrade(&state);
        ctx.on(&state.body, event, move |_| {
            if let Some(state) = weak.upgrade() {
                state.clear_announced();
            }
        });
    }
    let weak = Rc::downgrade(&state);
    ctx.dirty(move || weak.upgrade().is_some_and(|state| state.dirty()));
    let weak = Rc::downgrade(&state);
    ctx.on_stay(move || {
        if let Some(state) = weak.upgrade() {
            state.restore_field();
        }
    });
    let weak = Rc::downgrade(&state);
    ctx.on(&state.body, "focusin", move |event| {
        if let Some(state) = weak.upgrade()
            && let Some(target) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .filter(|target| target.closest("form, .entry-main").ok().flatten().is_some())
        {
            *state.last_field.borrow_mut() = Some(target);
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
        let Some(state) = weak.upgrade() else {
            return false;
        };
        if event.key() == "n" {
            return true;
        }
        let kind = match event.key().as_str() {
            "1" => "expense",
            "2" => "income",
            "3" => "transfer",
            _ => return false,
        };
        if state.journal.get() {
            return false;
        }
        if let Some(form) = state.simple.borrow().as_ref() {
            form.set_kind(kind);
            form.focus_kind();
            return true;
        }
        false
    });
    let weak = Rc::downgrade(&state);
    ctx.scope.cleanup(move || {
        if let Some(state) = weak.upgrade() {
            state.preview.cancel();
            if let Some((id, _)) = state.announce_timer.borrow_mut().take() {
                web_sys::window()
                    .expect("window")
                    .clear_timeout_with_handle(id);
            }
        }
    });
    let loading = state.clone();
    ctx.task(async move {
        let loaded: Result<Data, ApiError> = async {
            let settings = loading.ctx.load("settings", true).await?;
            let accounts = list(&loading.ctx.load("accounts", true).await?);
            let partners = list(&loading.ctx.load("partners", true).await?);
            let templates = list(&loading.ctx.load("templates", true).await?);
            if !loading.id.is_empty() {
                let entry = loading
                    .ctx
                    .get(&format!("/entries/{}", encode(&loading.id)), &[])
                    .await?;
                *loading.original.borrow_mut() = entry;
            }
            Ok(Data {
                settings,
                accounts,
                partners,
                templates,
            })
        }
        .await;
        if !loading.ctx.alive() {
            return Ok(());
        }
        match loaded {
            Ok(data) => {
                *loading.data.borrow_mut() = Some(data);
                loading.build();
            }
            Err(error) => {
                let banner = h("div", "banner banner--danger", "");
                append(
                    &banner,
                    &[
                        h("h2", "", "入力欄を準備できませんでした"),
                        h("p", "", &error.message),
                    ],
                );
                replace(&loading.body, &[banner]);
                focus(&loading.title);
            }
        }
        Ok(())
    });
    let mut screen = Screen::new(&ctx, root);
    screen.manages_focus = true;
    screen.update = Some(Rc::new(move |next| {
        let handled = next.route.param("id") == state.id;
        if handled {
            state.update_save();
        }
        handled
    }));
    Ok(screen)
}
