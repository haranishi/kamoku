// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    api::{self, ApiError},
    dom::{self, attrs},
    lifecycle::Scope,
    route::{self, Route},
    theme,
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    task::Waker,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Element, Event, KeyboardEvent};

type PendingResult = Option<Result<Value, ApiError>>;
#[derive(Default)]
struct Pending {
    value: RefCell<PendingResult>,
    waiters: RefCell<Vec<Waker>>,
}
#[derive(Default)]
pub(crate) struct Shared {
    pub values: RefCell<BTreeMap<String, Value>>,
    pending: RefCell<BTreeMap<String, Rc<Pending>>>,
    pub read_only: Cell<bool>,
}
#[derive(Clone)]
pub(crate) struct Context {
    pub route: Route,
    pub scope: Scope,
    pub shared: Rc<Shared>,
}
pub(crate) struct Screen {
    pub root: Element,
    pub scope: Scope,
    pub manages_focus: bool,
    pub update: Option<Rc<dyn Fn(Context) -> bool>>,
}
impl Screen {
    pub(crate) fn new(ctx: &Context, root: Element) -> Self {
        Self {
            root,
            scope: ctx.scope.clone(),
            manages_focus: false,
            update: None,
        }
    }
}
#[allow(
    clippy::unused_self,
    reason = "Context is the shared screen API; browser and notification operations use the same receiver contract"
)]
impl Context {
    pub(crate) fn child(&self) -> Self {
        let scope = Scope::new();
        let child = scope.clone();
        let id = self.scope.register_cleanup(move || child.dispose());
        let parent = self.scope.clone();
        scope.cleanup(move || parent.remove_cleanup(id));
        Self {
            route: self.route.clone(),
            scope,
            shared: self.shared.clone(),
        }
    }
    pub(crate) fn alive(&self) -> bool {
        self.scope.alive()
    }
    pub(crate) fn on(&self, target: &Element, event: &str, f: impl Fn(Event) + 'static) {
        self.scope.on(target, event, f);
    }
    pub(crate) fn key(&self, f: impl Fn(KeyboardEvent) -> bool + 'static) {
        self.scope.key(f);
    }
    pub(crate) fn dirty(&self, f: impl Fn() -> bool + 'static) {
        self.scope.dirty(f);
    }
    pub(crate) fn on_stay(&self, callback: impl Fn() + 'static) {
        self.scope.on_stay(callback);
    }
    pub(crate) fn allow_leave(&self) {
        self.scope.allow_leave();
    }
    pub(crate) fn navigate(&self, hash: &str, replace: bool) {
        route::navigate(hash, replace);
    }
    pub(crate) fn back(&self, fallback: &str) {
        route::back(fallback);
    }
    pub(crate) fn task(
        &self,
        future: impl std::future::Future<Output = Result<(), ApiError>> + 'static,
    ) {
        let ctx = self.clone();
        spawn_local(async move {
            if let Err(e) = future.await
                && ctx.alive()
            {
                ctx.error(&e.message);
            }
        });
    }
    pub(crate) fn notice(&self, kind: &str, message: &str, action: Option<(&str, &str)>) {
        super::super::components::notice::show(kind, message, action, None);
    }
    pub(crate) fn success(&self, message: &str) {
        self.notice("success", message, None);
    }
    pub(crate) fn success_for(&self, message: &str, milliseconds: u32) {
        super::super::components::notice::show_with(
            "success",
            message,
            None,
            None,
            Some(f64::from(milliseconds)),
            None,
        );
    }
    pub(crate) fn dismiss_error(&self, message: &str) {
        super::super::components::notice::dismiss_error(message);
    }
    pub(crate) fn notice_replace(&self, kind: &str, message: &str, key: &str) {
        super::super::components::notice::show_with(kind, message, None, None, None, Some(key));
    }
    pub(crate) fn info(&self, message: &str) {
        self.notice("info", message, None);
    }
    pub(crate) fn error(&self, message: &str) {
        self.notice("error", message, None);
    }
    pub(crate) fn notice_action(
        &self,
        kind: &str,
        message: &str,
        label: &str,
        f: impl Fn() + 'static,
    ) {
        super::super::components::notice::show(kind, message, Some((label, "")), Some(Rc::new(f)));
    }
    pub(crate) fn dismiss_notice_action(&self, label: &str) {
        super::super::components::notice::dismiss_action(label);
    }
    pub(crate) fn peek(&self, name: &str) -> Option<Value> {
        self.shared.values.borrow().get(name).cloned()
    }
    pub(crate) fn invalidate(&self, names: &[&str]) {
        let mut values = self.shared.values.borrow_mut();
        for name in names {
            values.remove(*name);
        }
    }
    pub(crate) fn is_read_only(&self) -> bool {
        self.shared.read_only.get()
            || self
                .peek("status")
                .is_some_and(|v| v["ledger"]["verified"] == false)
    }
    pub(crate) fn read_only_reason(&self) -> String {
        self.peek("status").and_then(|v|{let e=&v["ledger"]["error"];e["seq"].as_u64().map(|seq|format!("台帳の検証で、{seq}番目の記録に{}が見つかりました。読み取り専用で開いているため、書き込みはできません。", match e["kind"].as_str().unwrap_or("") { "bad_hash" => "ハッシュの不一致", "broken_link" => "前の記録とのつながりの切れ", "wrong_sequence" | "bad_seq" => "連番の飛びや入れ違い", "rule_violation" | "bad_rule" => "記録の決まりへの違反", "quick_check" => "データベースのファイルの傷み", _ => "記録の不整合" }))}).unwrap_or_else(||"台帳が読み取り専用のため、書き込みはできません。".into())
    }
    fn observe(&self, result: &Result<Value, ApiError>) {
        if result.as_ref().is_err_and(ApiError::is_read_only) {
            self.shared.read_only.set(true);
            if let Some(current) =
                super::super::current_context().or_else(super::super::global_context)
            {
                super::super::shell::sync(&current);
            }
        }
    }
    pub(crate) async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, ApiError> {
        let r = api::request("GET", path, query, None, None).await;
        self.observe(&r);
        r
    }
    pub(crate) async fn post(&self, path: &str, body: &Value) -> Result<Value, ApiError> {
        let r = api::request("POST", path, &[], Some(body), None).await;
        self.observe(&r);
        r
    }
    pub(crate) async fn post_abort(
        &self,
        path: &str,
        body: &Value,
        signal: &web_sys::AbortSignal,
    ) -> Result<Value, ApiError> {
        let r = api::post_abort(path, body, signal).await;
        self.observe(&r);
        r
    }
    pub(crate) async fn put(&self, path: &str, body: &Value) -> Result<Value, ApiError> {
        let r = api::request("PUT", path, &[], Some(body), None).await;
        self.observe(&r);
        r
    }
    pub(crate) async fn patch(&self, path: &str, body: &Value) -> Result<Value, ApiError> {
        let r = api::request("PATCH", path, &[], Some(body), None).await;
        self.observe(&r);
        r
    }
    pub(crate) async fn upload(
        &self,
        path: &str,
        query: &[(&str, &str)],
        bytes: &[u8],
    ) -> Result<Value, ApiError> {
        let r = api::request("POST", path, query, None, Some(bytes)).await;
        self.observe(&r);
        r
    }
    pub(crate) async fn load(&self, name: &str, force: bool) -> Result<Value, ApiError> {
        if !force && let Some(v) = self.peek(name) {
            return Ok(v);
        }
        let existing = self.shared.pending.borrow().get(name).cloned();
        if let Some(p) = existing {
            return std::future::poll_fn(move |cx| {
                if let Some(v) = p.value.borrow().clone() {
                    std::task::Poll::Ready(v)
                } else {
                    p.waiters.borrow_mut().push(cx.waker().clone());
                    std::task::Poll::Pending
                }
            })
            .await;
        }
        let pending = Rc::new(Pending::default());
        self.shared
            .pending
            .borrow_mut()
            .insert(name.into(), pending.clone());
        let (path, query) = match name {
            "accounts" => ("/accounts", vec![("include_disabled", "1")]),
            "partners" => ("/partners", vec![("limit", "500")]),
            "wallets" => ("/wallets", vec![]),
            "templates" => ("/templates", vec![]),
            "settings" => ("/settings", vec![]),
            "status" => ("/status", vec![]),
            _ => {
                self.shared.pending.borrow_mut().remove(name);
                return Err(ApiError::message(
                    "共有データの種類を確認できませんでした。",
                ));
            }
        };
        let result = self.get(path, &query).await.map(|v| {
            if matches!(name, "accounts" | "partners" | "wallets" | "templates") {
                v["items"].clone()
            } else {
                v
            }
        });
        if let Ok(value) = &result {
            self.shared
                .values
                .borrow_mut()
                .insert(name.into(), value.clone());
            if let Some(current) =
                super::super::current_context().or_else(super::super::global_context)
            {
                super::super::shell::sync(&current);
            }
        }
        *pending.value.borrow_mut() = Some(result.clone());
        self.shared.pending.borrow_mut().remove(name);
        for waker in pending.waiters.borrow_mut().drain(..) {
            waker.wake();
        }
        result
    }
    pub(crate) fn set_theme(&self, theme: &str) {
        theme::set_theme(theme);
    }
    pub(crate) fn set_density(&self, density: &str) {
        theme::set_density(density);
    }
    pub(crate) fn get_theme(&self) -> String {
        theme::get_theme()
    }
    pub(crate) fn get_density(&self) -> String {
        theme::get_density()
    }
    pub(crate) async fn copy_text(&self, text: &str) -> Result<(), ApiError> {
        JsFuture::from(dom::window().navigator().clipboard().write_text(text))
            .await
            .map_err(|_| {
                ApiError::message("コピーできませんでした。文字を選んでコピーしてください。")
            })?;
        Ok(())
    }
    pub(crate) async fn download(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<(), ApiError> {
        let response = api::response("GET", path, query, None, None).await?;
        let disposition = response
            .headers()
            .get("Content-Disposition")?
            .unwrap_or_default();
        let filename = disposition
            .split(';')
            .find_map(|v| {
                v.trim()
                    .strip_prefix("filename*=UTF-8''")
                    .and_then(|v| js_sys::decode_uri_component(v).ok())
                    .map(String::from)
            })
            .or_else(|| {
                disposition.split(';').find_map(|v| {
                    v.trim()
                        .strip_prefix("filename=")
                        .map(|s| s.trim_matches('"').to_owned())
                })
            })
            .unwrap_or_else(|| "kamoku-export".into());
        let warning = response
            .headers()
            .get("X-Kamoku-Warning")?
            .unwrap_or_default();
        let blob: web_sys::Blob = JsFuture::from(response.blob()?).await?.dyn_into()?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let a = dom::h("a", "", "");
        attrs(&a, &[("href", &url), ("download", &filename)]);
        dom::document()
            .body()
            .expect("document body")
            .append_child(&a)?;
        a.unchecked_ref::<web_sys::HtmlElement>().click();
        a.remove();
        spawn_local(async move {
            dom::delay(10_000).await;
            let _ = web_sys::Url::revoke_object_url(&url);
        });
        if let Some(count) = warning
            .strip_prefix("unmappable=")
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
        {
            self.info(&format!("Shift_JIS に無い文字が{count}か所あったので、「?」に置き換えて書き出しました。元の文字のまま残すときは、UTF-8（BOMつき）で書き出してください。"));
        }
        Ok(())
    }
}
