// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::core::{
    context::Context,
    dom::{append, attrs, busy, button, delay, disabled, document, focus, h, icon, set_text, uid},
    lifecycle::Scope,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    task::{Poll, Waker},
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{Element, HtmlDialogElement};

struct State {
    result: Cell<Option<bool>>,
    waiters: RefCell<Vec<Waker>>,
    busy: Cell<bool>,
    closed: Cell<bool>,
}
impl State {
    fn wake(&self) {
        for waiter in self.waiters.take() {
            waiter.wake();
        }
    }
}
#[derive(Clone)]
pub(crate) struct Dialog {
    pub el: HtmlDialogElement,
    pub body: Element,
    pub footer: Element,
    pub scope: Scope,
    title: Element,
    error: Element,
    opener: Option<Element>,
    state: Rc<State>,
}
impl Dialog {
    pub(crate) fn new(ctx: &Context, title: &str) -> Self {
        let el: HtmlDialogElement = h("dialog", "dialog", "").unchecked_into();
        let id = uid("dialog-title");
        attrs(&el, &[("aria-labelledby", &id)]);
        let head = h("div", "dialog-head", "");
        let title = h("h2", "", title);
        title.set_id(&id);
        let title_box = h("div", "page-head-text", "");
        append(&title_box, std::slice::from_ref(&title));
        let close = button("", "ghost");
        close.set_class_name("icon-btn");
        attrs(
            &close,
            &[("aria-label", "閉じる"), ("aria-keyshortcuts", "Escape")],
        );
        append(&close, &[icon("close")]);
        append(&head, &[title_box, close.clone()]);
        let body = h("div", "dialog-body", "");
        let footer = h("div", "dialog-foot", "");
        let error = h("p", "field-error", "");
        attrs(&error, &[("role", "alert")]);
        let error_box = h("div", "dialog-error", "");
        append(&error_box, std::slice::from_ref(&error));
        append(&el, &[head, body.clone(), error_box, footer.clone()]);
        let scope = Scope::new();
        let state = Rc::new(State {
            result: Cell::new(None),
            waiters: RefCell::new(Vec::new()),
            busy: Cell::new(false),
            closed: Cell::new(false),
        });
        let d = Self {
            el,
            body,
            footer,
            scope,
            title,
            error,
            opener: document().active_element(),
            state,
        };
        let x = d.clone();
        d.scope.on(&close, "click", move |_| {
            if !x.state.busy.get() {
                x.close();
            }
        });
        let x = d.clone();
        d.scope.on(&d.el, "cancel", move |e| {
            if x.state.busy.get() {
                e.prevent_default();
            } else {
                x.state.result.set(Some(false));
                x.state.wake();
            }
        });
        let x = d.clone();
        let parent_scope = ctx.scope.clone();
        let cleanup_id = Rc::new(Cell::new(0));
        let finished_id = cleanup_id.clone();
        d.scope.on(&d.el, "close", move |_| {
            if x.state.closed.replace(true) {
                return;
            }
            if x.state.result.get().is_none() {
                x.state.result.set(Some(false));
            }
            x.state.wake();
            let d = x.clone();
            let parent_scope = parent_scope.clone();
            let cleanup_id = finished_id.clone();
            spawn_local(async move {
                // All close listeners, including a screen's result resolver,
                // must run before their Scope removes the listener closures.
                delay(0).await;
                d.return_notices();
                parent_scope.remove_cleanup(cleanup_id.get());
                d.scope.dispose();
                d.el.remove();
                super::notice::place();
            });
        });
        let d_cleanup = d.clone();
        cleanup_id.set(ctx.scope.register_cleanup(move || {
            if d_cleanup.el.open() {
                d_cleanup.close();
            } else {
                d_cleanup.return_notices();
                d_cleanup.scope.dispose();
                d_cleanup.el.remove();
                super::notice::place();
            }
        }));
        d
    }
    pub(crate) fn show(&self) {
        let first = self
            .body
            .query_selector("input,select,textarea")
            .ok()
            .flatten()
            .or_else(|| self.footer.query_selector("button").ok().flatten())
            .unwrap_or_else(|| {
                attrs(&self.title, &[("tabindex", "-1")]);
                self.title.clone()
            });
        attrs(&first, &[("autofocus", "")]);
        if !self.el.is_connected() {
            document()
                .body()
                .expect("body")
                .append_child(&self.el)
                .expect("dialog mount");
        }
        if !self.el.open() {
            self.el.show_modal().expect("native dialog");
        }
        focus(&first);
        super::notice::place();
    }
    pub(crate) fn close(&self) {
        if self.el.open() {
            if self.state.result.get().is_none() {
                self.state.result.set(Some(false));
            }
            // A decision completes before closing removes the modal role.
            // DOM disposal still waits for every native close listener below.
            self.state.wake();
            self.el.close();
            if let Some(opener) = &self.opener
                && opener.is_connected()
            {
                focus(opener);
            }
        }
    }
    fn return_notices(&self) {
        if let Some(toasts) = document().get_element_by_id("toasts")
            && self.el.contains(Some(&toasts))
        {
            let _ = document().body().expect("body").append_child(&toasts);
        }
    }
    pub(crate) fn set_busy(&self, value: bool) {
        self.state.busy.set(value);
        busy(&self.el, value);
        if value {
            attrs(&self.body, &[("inert", "")]);
        } else {
            let _ = self.body.remove_attribute("inert");
        }
        for parent in [&self.footer, self.el.as_ref()] {
            if let Ok(nodes) = parent.query_selector_all("button") {
                for n in 0..nodes.length() {
                    if let Some(node) = nodes.item(n).and_then(|v| v.dyn_into::<Element>().ok()) {
                        disabled(&node, value);
                    }
                }
            }
        }
    }
    pub(crate) fn set_error(&self, text: &str) {
        set_text(&self.error, text);
    }
    pub(crate) fn set_title(&self, text: &str) {
        set_text(&self.title, text);
    }
    fn resolve(&self, value: bool) {
        self.state.result.set(Some(value));
        self.close();
    }
    async fn wait(&self) -> bool {
        let state = self.state.clone();
        std::future::poll_fn(move |cx| {
            if let Some(value) = state.result.get() {
                Poll::Ready(value)
            } else {
                state.waiters.borrow_mut().push(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}
pub(crate) async fn confirm(
    ctx: &Context,
    title: &str,
    body: &str,
    label: &str,
    danger: bool,
) -> bool {
    confirm_with_cancel(ctx, title, body, label, "やめる", danger).await
}
pub(crate) async fn confirm_with_cancel(
    ctx: &Context,
    title: &str,
    body: &str,
    label: &str,
    cancel_label: &str,
    danger: bool,
) -> bool {
    let d = Dialog::new(ctx, title);
    let message = h("p", "", body);
    message.set_id(&uid("confirm-body"));
    attrs(&d.el, &[("aria-describedby", &message.id())]);
    append(&d.body, &[message]);
    let cancel = button(cancel_label, "secondary");
    let yes = button(label, if danger { "danger" } else { "primary" });
    append(&d.footer, &[cancel.clone(), yes.clone()]);
    let x = d.clone();
    d.scope.on(&cancel, "click", move |_| x.resolve(false));
    let x = d.clone();
    d.scope.on(&yes, "click", move |_| x.resolve(true));
    d.show();
    focus(&cancel);
    d.wait().await
}
