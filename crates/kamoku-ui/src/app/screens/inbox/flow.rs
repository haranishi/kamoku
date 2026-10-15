// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::model::confirm_outcome;
use super::{
    ApiError, Cell, Confirmation, Context, Dialog, Element, HtmlElement, JsCast, KeyboardEvent, Rc,
    RefCell, Row, State, Summary, append, attrs, button, class, confirmed_nos, focus, group, h, id,
    ime_ignore, json, node, text,
};
use crate::app::prelude::phrase;
impl State {
    fn freeze(&self, on: bool) {
        for e in [&self.tools.el, &self.host, &self.pager] {
            if let Some(el) = e.dyn_ref::<HtmlElement>() {
                el.set_inert(on);
            }
        }
        if on {
            attrs(&self.host, &[("aria-busy", "true")]);
        } else {
            let _ = self.host.remove_attribute("aria-busy");
        }
    }
    pub fn confirm(self: &Rc<Self>) {
        if self.locked.get()
            || !self.ready.get()
            || self.bar.el.has_attribute("hidden")
            || self.filters.borrow().status != "draft"
        {
            return;
        }
        if self.ctx.is_read_only() {
            self.paint_bar();
            return;
        }
        let spot = self.spot();
        if let Some((id, _, col)) = &spot {
            let row = self.rows.borrow().iter().find(|r| r.id() == *id).cloned();
            if let Some(row) = row
                && !row.commit(col)
            {
                return;
            }
        }
        if self.selection.borrow().summary().count == 0 {
            self.bar.empty_attempt(&self.ctx);
            self.ctx
                .info("確定する行がありません。行で Enter を押すと、確定する行になります。");
            return;
        }
        self.locked.set(true);
        self.freeze(true);
        let state = self.clone();
        self.ctx.task(async move {
            let result = state.confirm_run().await;
            state.locked.set(false);
            state.sending.borrow_mut().take();
            state.freeze(false);
            if state.ctx.alive() {
                state.paint_bar();
                let rows = state.rows.borrow();
                let same = spot
                    .as_ref()
                    .and_then(|(id, _, _)| rows.iter().find(|r| r.id() == *id));
                if let Some(row) = same {
                    row.focus(spot.as_ref().map_or("account", |s| s.2.as_str()), false);
                } else if spot.is_none() && !state.bar.confirm.has_attribute("disabled") {
                    focus(&state.bar.confirm);
                } else if let Some(row) = rows.first() {
                    row.focus("account", true);
                } else {
                    focus(&state.heading);
                }
            }
            result
        });
    }
    async fn confirm_run(self: &Rc<Self>) -> Result<(), ApiError> {
        let rows = self.rows.borrow().clone();
        for row in &rows {
            row.flush().await;
        }
        if !self.ctx.alive() {
            return Ok(());
        }
        if self.ctx.is_read_only() {
            self.ctx.error(&self.ctx.read_only_reason());
            return Ok(());
        }
        let summary = self.selection.borrow().summary();
        let body = confirm_body(&summary);
        if !confirm_rows(&self.ctx, &summary, body).await || !self.ctx.alive() {
            return Ok(());
        }
        *self.sending.borrow_mut() = Some(summary);
        self.paint_bar();
        let mut batches = Confirmation::new(self.selection.borrow().ids());
        let mut error = None;
        while let Some(chunk) = batches.take_next() {
            match self
                .ctx
                .post("/drafts/confirm", &json!({"ids":chunk}))
                .await
            {
                Ok(res) => {
                    batches.complete(&res);
                }
                Err(e) => {
                    batches.stop();
                    error = Some(e);
                    break;
                }
            }
            if !self.ctx.alive() {
                return Ok(());
            }
        }
        if !self.ctx.alive() {
            return Ok(());
        }
        let confirmed = batches.confirmed;
        let outcome = confirm_outcome(&confirmed, &batches.failed);
        for key in outcome.unmark {
            self.selection.borrow_mut().remove(key);
        }
        let fixable = outcome.fixable;
        let missing = outcome.gone.len();
        if !confirmed.is_empty() {
            let nos: Vec<_> = confirmed.iter().filter_map(|c| c["no"].as_u64()).collect();
            self.ctx.notice(
                "success",
                &format!(
                    "{}件を確定しました（{}）。",
                    confirmed.len(),
                    confirmed_nos(&nos)
                ),
                Some(("仕訳帳を見る", "#/journal")),
            );
        }
        if !fixable.is_empty() {
            self.ctx.error(&format!("{}件は確定できませんでした。行の下の理由を確かめて、直してからもう一度確定してください。",fixable.len()));
        }
        if missing > 0 {
            self.ctx.info(&format!("{missing}件は、ほかの場所で確定済みか、見つかりませんでした。確定する行から外しました。"));
        }
        if let Some(err) = error {
            if err.is_stale() || err.code == "already_confirmed" {
                self.ctx
                    .info("他の場所で更新されました。読み込み直します。");
            } else {
                self.ctx.error(&format!(
                    "{} 一覧を読み込み直して、確定できた分を確かめます。",
                    err.message
                ));
            }
        }
        self.ctx.invalidate(&["status"]);
        let _ = self.ctx.load("status", true).await;
        if !self.ctx.alive() {
            return Ok(());
        }
        self.load(None).await?;
        if !self.ctx.alive() {
            return Ok(());
        }
        for f in fixable {
            if let Some(row) = self.rows.borrow().iter().find(|r| r.id() == id(&f)) {
                row.set_error(text(&f["error"], "field"), text(&f["error"], "message"));
            }
        }
        Ok(())
    }
    pub fn ignore_row(self: &Rc<Self>, row: Rc<Row>) {
        if self.ctx.is_read_only() || self.locked.get() || self.leaving.borrow().contains(&row.id())
        {
            return;
        }
        self.leaving.borrow_mut().insert(row.id());
        row.leaving.set(true);
        row.set_candidate(false);
        class(&row.tr, "is-leaving", true);
        self.paint_bar();
        let rows = self.rows.borrow().clone();
        if let Some(at) = rows.iter().position(|r| r.id() == row.id()) {
            let free = |r: &&Rc<Row>| !r.leaving.get() && r.id() != row.id();
            if let Some(next) = rows
                .iter()
                .skip(at + 1)
                .find(free)
                .or_else(|| rows[..at].iter().rev().find(free))
            {
                next.focus("cand", false);
            }
        }
        self.ignore_queue.borrow_mut().push_back(row);
        if self.ignoring.replace(true) {
            return;
        }
        let state = self.clone();
        self.ctx.task(async move {
            loop {
                let row = state.ignore_queue.borrow_mut().pop_front();
                let Some(row) = row else {
                    break;
                };
                let key = row.id();
                row.flush().await;
                let result = state
                    .ctx
                    .post(&format!("/drafts/{key}/ignore"), &json!({}))
                    .await;
                state.leaving.borrow_mut().remove(&key);
                match result {
                    Ok(_) => {
                        state.finish_ignore(&row);
                        let _ = state.ctx.load("status", true).await;
                    }
                    Err(err) => {
                        row.leaving.set(false);
                        if state.ctx.alive() {
                            if err.code == "already_confirmed" {
                                state.stale();
                            } else if let Some(visible) =
                                state.rows.borrow().iter().find(|r| r.id() == key)
                            {
                                visible.leaving.set(false);
                                class(&visible.tr, "is-leaving", false);
                                visible.set_error("row", &err.message);
                            } else {
                                state.ctx.error(&err.message);
                            }
                            state.paint_bar();
                        } else {
                            state.ctx.error(&err.message);
                        }
                    }
                }
            }
            state.ignoring.set(false);
            if state.ctx.alive() && state.rows.borrow().is_empty() {
                state.load(None).await?;
                if let Some(first) = state.rows.borrow().first() {
                    first.focus("cand", false);
                } else {
                    focus(&state.heading);
                }
            }
            Ok(())
        });
    }
    fn finish_ignore(self: &Rc<Self>, original: &Row) {
        let key = original.id();
        let visible = self.rows.borrow().iter().find(|r| r.id() == key).cloned();
        self.rows.borrow_mut().retain(|r| r.id() != key);
        original.ctx.scope.dispose();
        self.selection.borrow_mut().remove(key);
        if self.ctx.alive()
            && let Some(row) = visible
        {
            row.ctx.scope.dispose();
            row.tr.remove();
            row.msg_tr.remove();
            self.total.set(self.total.get().saturating_sub(1));
            let mut counts = self.counts.borrow_mut();
            let draft = counts["draft"].as_u64().unwrap_or(0);
            let ignored = counts["ignored"].as_u64().unwrap_or(0);
            counts["draft"] = json!(draft.saturating_sub(1));
            counts["ignored"] = json!(ignored + 1);
        }
        self.undo.borrow_mut().push(key);
        self.undo_notice.set(Some(key));
        if self.ctx.alive() {
            self.paint_counts();
            self.paint_bar();
        }
        self.ctx.invalidate(&["status"]);
        let weak = Rc::downgrade(self);
        let context = self.ctx.clone();
        let description = text(&original.draft.borrow(), "description").to_owned();
        self.ctx.notice_action(
            "success",
            &format!(
                "「{}」を無視しました。",
                if description.is_empty() {
                    "摘要なし"
                } else {
                    &description
                }
            ),
            "元に戻す",
            move || {
                if let Some(s) = weak.upgrade().filter(|s| s.ctx.alive()) {
                    s.restore(key);
                } else {
                    let context = context.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        restore_request(&context, key).await;
                    });
                }
            },
        );
    }
    pub fn restore(self: &Rc<Self>, key: u64) {
        if self.ctx.is_read_only() || self.locked.get() {
            return;
        }
        let index = if self.filters.borrow().status == "ignored" {
            self.data.borrow().iter().position(|d| id(d) == key)
        } else {
            None
        };
        self.undo.borrow_mut().retain(|id| *id != key);
        if self.undo_notice.get() == Some(key) {
            self.undo_notice.set(None);
            self.ctx.dismiss_notice_action("元に戻す");
        }
        let state = self.clone();
        self.ctx.task(async move {
            if !restore_request(&state.ctx, key).await || !state.ctx.alive() {
                return Ok(());
            }
            state
                .load(if index.is_none() {
                    Some((key, "cand".into()))
                } else {
                    None
                })
                .await?;
            if !state.ctx.alive() {
                return Ok(());
            }
            if let Some(at) = index {
                let buttons = state.host.query_selector_all(".restore-btn").ok();
                if let Some(next) = buttons
                    .and_then(|b| u32::try_from(at).ok().and_then(|index| b.item(index)))
                    .and_then(|n| n.dyn_into::<Element>().ok())
                {
                    focus(&next);
                } else {
                    focus(&state.tools.draft);
                }
            }
            Ok(())
        });
    }
}

fn confirm_body(summary: &Summary) -> Element {
    node(
        "div",
        "stack",
        &[
            confirm_sums(summary),
            node(
                "p",
                "small muted",
                &[phrase(
                    "確定すると、仕訳帳と帳票の集計に入ります。確定した後は、訂正と取消の手続きでしか変えられません。",
                )],
            ),
        ],
    )
}

/// Notices outlive their page. A requested restore still updates shared counts,
/// while the caller decides whether its original page can be reloaded/focused.
async fn restore_request(ctx: &Context, key: u64) -> bool {
    if ctx.is_read_only() {
        ctx.error(&ctx.read_only_reason());
        return false;
    }
    if let Err(error) = ctx
        .post(&format!("/drafts/{key}/restore"), &json!({}))
        .await
    {
        ctx.error(&error.message);
        return false;
    }
    ctx.success("未確認に戻しました。");
    ctx.invalidate(&["status"]);
    let _ = ctx.load("status", true).await;
    true
}
fn confirm_sums(s: &Summary) -> Element {
    let list = h("dl", "confirm-sums", "");
    for (label, n, unit) in [
        ("件数", s.count as i128, "件"),
        ("入金の合計", s.incoming, "円"),
        ("出金の合計", s.outgoing, "円"),
    ] {
        append(
            &list,
            &[node(
                "div",
                "",
                &[
                    h("dt", "", label),
                    node(
                        "dd",
                        "",
                        &[node(
                            "span",
                            "money",
                            &[h("span", "num", &group(n)), h("span", "unit", unit)],
                        )],
                    ),
                ],
            )],
        );
    }
    list
}

async fn confirm_rows(ctx: &Context, summary: &Summary, body: Element) -> bool {
    let dialog = Dialog::new(ctx, &format!("{}件を確定しますか", summary.count));
    append(&dialog.body, &[body]);
    let cancel = button("やめる", "secondary");
    let yes = button("確定する", "primary");
    append(&dialog.footer, &[cancel.clone(), yes.clone()]);
    let result = Rc::new(Cell::new(None));
    let waiters = Rc::new(RefCell::new(Vec::<std::task::Waker>::new()));
    let value = result.clone();
    let d = dialog.clone();
    dialog.scope.on(&yes, "click", move |_| {
        value.set(Some(true));
        d.close();
    });
    let d = dialog.clone();
    dialog.scope.on(&cancel, "click", move |_| d.close());
    let value = result.clone();
    let wake = waiters.clone();
    dialog.scope.on(&dialog.el, "close", move |_| {
        if value.get().is_none() {
            value.set(Some(false));
        }
        for waker in wake.borrow_mut().drain(..) {
            waker.wake();
        }
    });
    let d = dialog.clone();
    let value = result.clone();
    dialog.scope.on(&dialog.el, "keydown", move |event| {
        let Ok(key) = event.dyn_into::<KeyboardEvent>() else {
            return;
        };
        if !ime_ignore(&key)
            && key.key() == "Enter"
            && (key.ctrl_key() || key.meta_key())
            && !key.shift_key()
            && !key.alt_key()
        {
            key.prevent_default();
            key.stop_propagation();
            value.set(Some(true));
            d.close();
        }
    });
    dialog.show();
    focus(&yes);
    std::future::poll_fn(move |cx| {
        if let Some(value) = result.get() {
            std::task::Poll::Ready(value)
        } else {
            waiters.borrow_mut().push(cx.waker().clone());
            std::task::Poll::Pending
        }
    })
    .await
}
