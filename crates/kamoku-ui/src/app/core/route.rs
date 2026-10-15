// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    dom::{document, window},
    lifecycle::Scope,
};
use std::{cell::RefCell, collections::BTreeMap};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::UrlSearchParams;

#[derive(Clone, Debug)]
pub(crate) struct Route {
    pub path: String,
    pub hash: String,
    pub previous_hash: Option<String>,
    params: BTreeMap<String, String>,
    query: BTreeMap<String, String>,
}
impl Route {
    pub(crate) fn parse(hash: &str, previous_hash: Option<String>) -> Self {
        let hash = if hash.is_empty() { "#/" } else { hash };
        let clean = hash.strip_prefix('#').unwrap_or(hash);
        let (path, query) = clean.split_once('?').unwrap_or((clean, ""));
        let mut params = BTreeMap::new();
        let bits = path.trim_matches('/').split('/').collect::<Vec<_>>();
        if bits.len() == 2 {
            let key = if bits[0] == "reports" { "name" } else { "id" };
            params.insert(
                key.into(),
                js_sys::decode_uri_component(bits[1]).map_or_else(|_| bits[1].into(), String::from),
            );
        }
        let mut values = BTreeMap::new();
        if let Ok(search) = UrlSearchParams::new_with_str(query) {
            for pair in search.entries().into_iter().flatten() {
                let pair = js_sys::Array::from(&pair);
                if let (Some(k), Some(v)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
                    values.insert(k, v);
                }
            }
        }
        Self {
            path: if path.is_empty() {
                "/".into()
            } else {
                path.into()
            },
            hash: hash.into(),
            previous_hash,
            params,
            query: values,
        }
    }
    pub(crate) fn param(&self, name: &str) -> String {
        self.params.get(name).cloned().unwrap_or_default()
    }
    pub(crate) fn query(&self, name: &str) -> String {
        self.query.get(name).cloned().unwrap_or_default()
    }
    pub(crate) fn current() -> Self {
        Self::parse(&window().location().hash().unwrap_or_default(), None)
    }
}
pub(crate) fn build_hash(path: &str, values: &[(&str, &str)]) -> String {
    let params = UrlSearchParams::new().expect("URL parameters");
    for (k, v) in values {
        if !v.is_empty() {
            params.set(k, v);
        }
    }
    let q = String::from(params.to_string());
    format!("#{}{}{}", path, if q.is_empty() { "" } else { "?" }, q)
}
struct Router {
    shown: String,
    index: i32,
    reverting: bool,
    approved: Option<String>,
    asking: bool,
}
thread_local! {static ROUTER:RefCell<Router>=const { RefCell::new(Router{shown:String::new(),index:1,reverting:false,approved:None,asking:false}) };}
fn history_index() -> Option<i32> {
    window()
        .history()
        .ok()?
        .state()
        .ok()
        .and_then(|v| js_sys::Reflect::get(&v, &JsValue::from_str("__kamokuIndex")).ok())
        .and_then(|v| v.as_f64())
        .and_then(|n| {
            if n.is_finite() && n.fract() == 0.0 && n >= 1.0 && n <= f64::from(i32::MAX) {
                num_traits::ToPrimitive::to_i32(&n)
            } else {
                None
            }
        })
}
fn mark(index: i32, hash: &str) {
    let state = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &state,
        &JsValue::from_str("__kamokuIndex"),
        &JsValue::from_f64(f64::from(index)),
    );
    let _ = window()
        .history()
        .and_then(|h| h.replace_state_with_url(&state, "", Some(hash)));
}
pub(crate) fn start(scope: &Scope) {
    let hash = window().location().hash().unwrap_or_else(|_| "#/".into());
    let hash = if hash.is_empty() { "#/".into() } else { hash };
    let index = history_index().unwrap_or(1);
    mark(index, &hash);
    ROUTER.with(|r| {
        let mut r = r.borrow_mut();
        r.shown = hash;
        r.index = index;
    });
    scope.listen(window().as_ref(), "hashchange", false, move |_| changed());
    scope.listen(window().as_ref(), "beforeunload", false, move |e| {
        if super::super::current_scope().is_some_and(|s| s.is_dirty()) {
            e.prevent_default();
            if let Some(e) = e.dyn_ref::<web_sys::BeforeUnloadEvent>() {
                e.set_return_value("");
            }
        }
    });
    super::super::render();
}
fn changed() {
    let target = window().location().hash().unwrap_or_default();
    let target = if target.is_empty() {
        "#/".into()
    } else {
        target
    };
    let action = ROUTER.with(|cell| {
        let mut r = cell.borrow_mut();
        if r.reverting {
            r.reverting = false;
            if target != r.shown {
                mark(r.index, &r.shown);
            }
            return 0;
        }
        if r.approved.as_ref() == Some(&target) {
            r.approved = None;
            return 1;
        }
        if r.asking {
            return 2;
        }
        1
    });
    if action == 0 {
        return;
    }
    if action == 2 {
        put_back();
        return;
    }
    let same_entry = super::super::current_context().is_some_and(|ctx| {
        ctx.route.path.starts_with("/entry") && Route::parse(&target, None).path == ctx.route.path
    });
    let dirty = !same_entry && super::super::current_scope().is_some_and(|s| s.is_dirty());
    if !dirty {
        accept(&target);
        return;
    }
    let delta = put_back();
    ROUTER.with(|r| r.borrow_mut().asking = true);
    let Some(ctx) = super::super::current_context() else {
        return;
    };
    if let Ok(dialogs) = document().query_selector_all("dialog[open]") {
        for i in 0..dialogs.length() {
            if let Some(d) = dialogs
                .item(i)
                .and_then(|v| v.dyn_into::<web_sys::HtmlDialogElement>().ok())
                && d.text_content()
                    .unwrap_or_default()
                    .contains("入力中の内容を捨てますか")
            {
                d.close();
            }
        }
    }
    let opener = document().active_element();
    spawn_local(async move {
        let ok = super::super::components::dialog::confirm_with_cancel(
            &ctx,
            "入力中の内容を捨てますか",
            "まだ保存していない入力があります。捨てると、元に戻せません。",
            "捨てて離れる",
            "入力を続ける",
            true,
        )
        .await;
        ROUTER.with(|r| r.borrow_mut().asking = false);
        if ok && ctx.alive() {
            ctx.allow_leave();
            ROUTER.with(|r| r.borrow_mut().approved = Some(target.clone()));
            if delta == 0 {
                let index = ROUTER.with(|r| r.borrow().index);
                mark(index, &target);
                changed();
            } else {
                let _ = window().history().and_then(|h| h.go_with_delta(delta));
            }
        } else if ctx.alive() {
            let active = document().active_element();
            // Native dialog close restores its opener before this future resumes.
            // Preserve any subsequent focus move instead of stealing it back.
            if active
                .as_ref()
                .is_none_or(|active| active.tag_name() == "BODY" || opener.as_ref() == Some(active))
            {
                ctx.scope.stay();
            }
        }
    });
}
fn put_back() -> i32 {
    ROUTER.with(|cell| {
        let mut r = cell.borrow_mut();
        let delta = history_index().map_or(1, |i| i - r.index);
        if delta == 0 {
            mark(r.index, &r.shown);
        } else {
            r.reverting = true;
            let _ = window().history().and_then(|h| h.go_with_delta(-delta));
        }
        delta
    })
}
fn accept(hash: &str) {
    let previous = ROUTER.with(|cell| {
        let mut r = cell.borrow_mut();
        let previous = r.shown.clone();
        let index = history_index().unwrap_or(r.index + 1);
        mark(index, hash);
        r.index = index;
        r.shown = hash.into();
        previous
    });
    super::super::set_previous(previous);
    super::super::render();
}
pub(crate) fn navigate(hash: &str, replace: bool) {
    let hash = if hash.starts_with('#') {
        hash.into()
    } else {
        format!("#{hash}")
    };
    if replace {
        let index = ROUTER.with(|r| r.borrow().index);
        mark(index, &hash);
        accept(&hash);
    } else if window().location().hash().ok().as_deref() == Some(&hash) {
        super::super::render();
    } else {
        let _ = window().location().set_hash(&hash);
    }
}
pub(crate) fn back(fallback: &str) {
    if ROUTER.with(|r| r.borrow().index) > 1 {
        let _ = window().history().and_then(|h| h.back());
    } else {
        navigate(fallback, true);
    }
}
pub(crate) fn title(route: &Route) -> String {
    match route.path.split('/').nth(1).unwrap_or_default() {
        "" => "ホーム".into(),
        "entry" => {
            if route.param("id").is_empty() {
                "取引を登録".into()
            } else {
                "取引を訂正".into()
            }
        }
        "journal" => "仕訳帳".into(),
        "inbox" => "未確認".into(),
        "import" => "明細を取り込む".into(),
        "wallets" => "口座".into(),
        "accounts" => "勘定科目".into(),
        "partners" => "取引先".into(),
        "settings" => "設定".into(),
        "setup" => "初期設定".into(),
        "reports" => match route.param("name").as_str() {
            "income-statement" => "損益計算書（帳票）",
            "balance-sheet" => "貸借対照表（帳票）",
            "trial-balance" => "試算表（帳票）",
            "ledger" => "総勘定元帳（帳票）",
            "tax-summary" => "消費税（帳票）",
            _ => "帳票",
        }
        .into(),
        _ => "ページが見つかりません".into(),
    }
}
pub(crate) fn set_title(route: &Route) {
    document().set_title(&format!("{} | kamoku", title(route)));
}
