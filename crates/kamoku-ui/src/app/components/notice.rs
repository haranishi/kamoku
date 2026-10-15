// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::core::{
    dom::{append, attrs, button, document, h, icon, link, now, uid, window},
    lifecycle::Scope,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Element, HtmlElement, MutationObserver, MutationObserverInit, ResizeObserver};

thread_local! {
    static LIVE:RefCell<BTreeMap<String,Scope>>=const{RefCell::new(BTreeMap::new())};
    static TRACKING:RefCell<Option<Tracking>>=const{RefCell::new(None)};
    static POSITION:RefCell<Position>=RefCell::new(Position::default());
}
struct Position {
    host: Option<Element>,
    key: Option<String>,
    choose: bool,
}
impl Default for Position {
    fn default() -> Self {
        Self {
            host: None,
            key: None,
            choose: true,
        }
    }
}
struct Tracking {
    scope: Scope,
    frame: Rc<Cell<Option<i32>>>,
    frame_callback: Closure<dyn FnMut(f64)>,
    sizes: Option<ResizeObserver>,
    changes: Option<MutationObserver>,
    _sizes_callback: Closure<dyn FnMut()>,
    _changes_callback: Closure<dyn FnMut()>,
}
impl Drop for Tracking {
    fn drop(&mut self) {
        if let Some(observer) = &self.sizes {
            observer.disconnect();
        }
        if let Some(observer) = &self.changes {
            observer.disconnect();
        }
        self.scope.dispose();
        if let Some(frame) = self.frame.take() {
            let _ = window().cancel_animation_frame(frame);
        }
    }
}
fn stop_tracking() {
    let tracking = TRACKING.with(|current| current.borrow_mut().take());
    drop(tracking);
}
fn schedule() {
    TRACKING.with(|current| {
        let current = current.borrow();
        let Some(tracking) = current.as_ref() else {
            return;
        };
        if tracking.frame.get().is_none()
            && let Ok(frame) =
                window().request_animation_frame(tracking.frame_callback.as_ref().unchecked_ref())
        {
            tracking.frame.set(Some(frame));
        }
    });
}
fn observe_bars() {
    TRACKING.with(|current| {
        if let Some(observer) = current
            .borrow()
            .as_ref()
            .and_then(|tracking| tracking.sizes.as_ref())
        {
            observer.disconnect();
            for bar in elements("[data-bottom-bar]") {
                observer.observe(&bar);
            }
        }
    });
}
fn choose_again() {
    POSITION.with(|position| position.borrow_mut().choose = true);
}
fn focused_toast() -> bool {
    region().is_some_and(|region| {
        region.contains(document().active_element().as_ref().map(AsRef::as_ref))
    })
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep observer ownership and its paired cleanup in one lifecycle operation"
)]
fn start_tracking() {
    if TRACKING.with(|current| current.borrow().is_some()) {
        return;
    }
    let scope = Scope::new();
    let frame = Rc::new(Cell::new(None));
    let pending = frame.clone();
    let frame_callback = Closure::wrap(Box::new(move |_| {
        pending.set(None);
        place();
    }) as Box<dyn FnMut(f64)>);
    let sizes_callback = Closure::wrap(Box::new(schedule) as Box<dyn FnMut()>);
    let sizes = ResizeObserver::new(sizes_callback.as_ref().unchecked_ref()).ok();
    let changes_callback = Closure::wrap(Box::new(|| {
        // Move into an opened dialog before its previous host can leave the document.
        place();
        observe_bars();
        schedule();
    }) as Box<dyn FnMut()>);
    let options = MutationObserverInit::new();
    options.set_child_list(true);
    options.set_subtree(true);
    options.set_attributes(true);
    let attributes = js_sys::Array::new();
    for attribute in ["hidden", "open", "class"] {
        attributes.push(&attribute.into());
    }
    options.set_attribute_filter(attributes.as_ref());
    let changes = MutationObserver::new(changes_callback.as_ref().unchecked_ref())
        .ok()
        .filter(|observer| {
            document().body().is_some_and(|body| {
                observer
                    .observe_with_options(body.as_ref(), &options)
                    .is_ok()
            })
        });
    scope.listen(document().as_ref(), "focusin", true, |_| {
        if !focused_toast() {
            choose_again();
            schedule();
        }
    });
    scope.listen(window().as_ref(), "resize", false, |_| {
        choose_again();
        schedule();
    });
    scope.listen(document().as_ref(), "scroll", true, |_| schedule());
    TRACKING.with(|current| {
        *current.borrow_mut() = Some(Tracking {
            scope,
            frame,
            frame_callback,
            sizes,
            changes,
            _sizes_callback: sizes_callback,
            _changes_callback: changes_callback,
        });
    });
    observe_bars();
}
fn region() -> Option<HtmlElement> {
    document()
        .get_element_by_id("toasts")
        .and_then(|e| e.dyn_into().ok())
}
fn elements(selector: &str) -> Vec<Element> {
    document()
        .query_selector_all(selector)
        .map(|list| {
            (0..list.length())
                .filter_map(|i| list.item(i).and_then(|v| v.dyn_into().ok()))
                .collect()
        })
        .unwrap_or_default()
}
fn dismiss(el: &Element) {
    let id = el.id();
    LIVE.with(|live| {
        if let Some(scope) = live.borrow_mut().remove(&id) {
            scope.dispose();
        }
    });
    el.remove();
    place();
}
pub(crate) fn dismiss_action(label: &str) {
    for el in elements(".toast[data-replace-key]") {
        if el.get_attribute("data-replace-key").as_deref() == Some(label) {
            dismiss(&el);
        }
    }
}
pub(crate) fn dismiss_error(message: &str) {
    for el in elements(".toast--error") {
        if el
            .query_selector(".toast-body")
            .ok()
            .flatten()
            .and_then(|e| e.text_content())
            .as_deref()
            == Some(message)
        {
            dismiss(&el);
        }
    }
}
pub(crate) fn start(scope: &Scope) {
    stop_tracking();
    POSITION.with(|position| *position.borrow_mut() = Position::default());
    scope.cleanup(stop_tracking);
}
pub(crate) fn show(
    kind: &str,
    message: &str,
    action: Option<(&str, &str)>,
    callback: Option<Rc<dyn Fn()>>,
) {
    show_with(kind, message, action, callback, None, None);
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep declarative DOM assembly and ordered event registration together"
)]
pub(crate) fn show_with(
    kind: &str,
    message: &str,
    action: Option<(&str, &str)>,
    callback: Option<Rc<dyn Fn()>>,
    duration: Option<f64>,
    replace_key: Option<&str>,
) {
    let Some(region) = region() else {
        return;
    };
    choose_again();
    if kind == "error"
        && elements(".toast--error").iter().any(|el| {
            el.query_selector(".toast-body")
                .ok()
                .flatten()
                .and_then(|e| e.text_content())
                .as_deref()
                == Some(message)
        })
    {
        place();
        return;
    }
    if let Some(key) = replace_key {
        dismiss_action(key);
    }
    if let Some((label, _)) = action
        && callback.is_some()
    {
        for old in elements(".toast[data-replace-key]") {
            if old.get_attribute("data-replace-key").as_deref() == Some(label) {
                dismiss(&old);
            }
        }
    }
    let el = h("div", &format!("toast toast--{kind}"), "");
    let id = uid("toast");
    el.set_id(&id);
    if let Some(key) = replace_key {
        attrs(&el, &[("data-replace-key", key)]);
    }
    if kind == "error" {
        attrs(&el, &[("role", "alert")]);
    }
    let image = icon(if kind == "error" {
        "alert"
    } else if kind == "success" {
        "check-circle"
    } else {
        "info"
    });
    attrs(&image, &[("class", "icon toast-icon")]);
    let body = h("div", "toast-body", message);
    let actions = h("div", "toast-actions", "");
    let scope = Scope::new();
    if let Some((label, href)) = action {
        let a = if callback.is_some() {
            button(label, "ghost")
        } else {
            link(label, href, "btn btn--ghost")
        };
        if let Some(callback) = callback {
            attrs(&el, &[("data-replace-key", label)]);
            let item = el.clone();
            scope.on(&a, "click", move |_| {
                callback();
                dismiss(&item);
            });
        }
        append(&actions, &[a]);
    }
    let close = button("", "ghost");
    close.set_class_name("icon-btn");
    attrs(&close, &[("aria-label", "この通知を閉じる")]);
    append(&close, &[icon("close")]);
    let item = el.clone();
    scope.on(&close, "click", move |_| dismiss(&item));
    append(&el, &[image, body, actions, close]);
    append(&region, std::slice::from_ref(&el));
    LIVE.with(|live| live.borrow_mut().insert(id, scope.clone()));
    if let Some(lifetime) =
        crate::notification::Lifetime::new(kind, action.is_some(), duration, now())
    {
        let lifetime = Rc::new(RefCell::new(lifetime));
        for (event, hovered) in [("mouseenter", true), ("mouseleave", false)] {
            let state = lifetime.clone();
            scope.on(&el, event, move |_| {
                state.borrow_mut().hover(hovered, now());
            });
        }
        for event in ["focusin", "focusout"] {
            let state = lifetime.clone();
            scope.on(&el, event, move |_| state.borrow_mut().restart(now()));
        }
        let target = el.clone();
        let callback = Closure::wrap(Box::new(move || {
            let focused = document()
                .active_element()
                .is_some_and(|active| target.contains(Some(&active)));
            if lifetime.borrow().expired(now(), focused) {
                dismiss(&target);
            }
        }) as Box<dyn FnMut()>);
        if let Ok(timer) = window().set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            250,
        ) {
            scope.cleanup(move || {
                window().clear_interval_with_handle(timer);
                drop(callback);
            });
        }
    }
    let live = elements(".toast");
    if live.len() > 4
        && let Some(old) = live
            .into_iter()
            .find(|e| !e.class_list().contains("toast--error"))
    {
        dismiss(&old);
    }
    place();
}
fn overlaps(a: &web_sys::DomRect, b: &web_sys::DomRect) -> bool {
    a.left() < b.right() && b.left() < a.right() && a.top() < b.bottom() && b.top() < a.bottom()
}
fn apply(
    region: &HtmlElement,
    key: &str,
    top: Option<f64>,
    bottom: Option<f64>,
    left: Option<f64>,
    right: Option<f64>,
    width: Option<f64>,
) {
    if key == "bottom" {
        let _ = region.remove_attribute("data-place");
        return;
    }
    attrs(region, &[("data-place", key)]);
    for (name, value) in [
        ("top", top),
        ("bottom", bottom),
        ("left", left),
        ("right", right),
        ("maxw", width),
    ] {
        let text = value.map_or_else(
            || {
                if name == "maxw" {
                    "none".into()
                } else {
                    "auto".into()
                }
            },
            |n| format!("{n:.0}px"),
        );
        let _ = region
            .style()
            .set_property(&format!("--toast-{name}"), &text);
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep declarative DOM assembly and ordered event registration together"
)]
pub(crate) fn place() {
    let Some(region) = region() else {
        return;
    };
    let body = document().body().expect("body");
    if region.child_element_count() == 0 {
        stop_tracking();
        POSITION.with(|position| *position.borrow_mut() = Position::default());
        let _ = region.style().set_property("--toast-lift", "0px");
        let _ = region.hide_popover();
        if region.parent_node().as_ref() != Some(body.as_ref()) {
            let _ = body.append_child(&region);
        }
        let _ = region.remove_attribute("data-place");
        return;
    }
    start_tracking();
    let dialogs = elements("dialog[open]");
    let dialog = dialogs.last();
    let host: Element = dialog.cloned().unwrap_or_else(|| body.clone().into());
    if region.parent_node().as_ref() != Some(host.as_ref()) {
        let _ = region.hide_popover();
        let _ = host.append_child(&region);
    }
    if !region.contains(document().active_element().as_ref().map(AsRef::as_ref)) {
        let _ = region.hide_popover();
        let _ = region.show_popover();
    }
    let vw = window()
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(390.0);
    let vh = window()
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(844.0);
    let mut lift: f64 = 0.0;
    if dialog.is_none() {
        for bar in elements("[data-bottom-bar]") {
            let rect = bar.get_bounding_client_rect();
            if let Ok(Some(css)) = window().get_computed_style(&bar) {
                let pos = css.get_property_value("position").unwrap_or_default();
                let bottom = css
                    .get_property_value("bottom")
                    .unwrap_or_default()
                    .trim_end_matches("px")
                    .parse::<f64>()
                    .ok();
                if let Some(inset) = bottom
                    && ["fixed", "sticky"].contains(&pos.as_str())
                    && rect.width() > 0.0
                    && rect.height() > 0.0
                    && rect.top() < vh
                    && rect.bottom() > 0.0
                    && css.get_property_value("visibility").unwrap_or_default() != "hidden"
                    && (rect.bottom() - (vh - inset)).abs() <= 2.0
                {
                    lift = lift.max(vh - rect.top() - if vw < 768.0 { inset } else { 0.0 });
                }
            }
        }
    }
    let _ = region
        .style()
        .set_property("--toast-lift", &format!("{:.0}px", lift.ceil()));
    let focus_rect = document()
        .active_element()
        .filter(|e| e.tag_name() != "BODY" && !region.contains(Some(e.as_ref())))
        .map(|e| {
            e.closest("label")
                .ok()
                .flatten()
                .filter(|_| {
                    e.matches("input[type=checkbox],input[type=radio]")
                        .unwrap_or(false)
                })
                .unwrap_or(e)
                .get_bounding_client_rect()
        });
    let mut candidates = Vec::new();
    if let Some(dialog) = dialog {
        let r = dialog.get_bounding_client_rect();
        if vw - r.right() >= 328.0 {
            candidates.push((
                "out-right",
                Some(16.0),
                None,
                None,
                Some(24.0),
                Some((vw - r.right() - 48.0).min(440.0)),
                false,
            ));
        }
        if r.left() >= 328.0 {
            candidates.push((
                "out-left",
                Some(16.0),
                None,
                None,
                Some(vw - r.left() + 24.0),
                Some((r.left() - 48.0).min(440.0)),
                false,
            ));
        }
        let top = dialog
            .query_selector(".dialog-head")
            .ok()
            .flatten()
            .map_or(r.top(), |e| e.get_bounding_client_rect().bottom())
            + 8.0;
        let foot = dialog
            .query_selector(".dialog-foot")
            .ok()
            .flatten()
            .map_or(r.bottom(), |e| e.get_bounding_client_rect().top());
        let w = (r.width() - 32.0).clamp(0.0, 440.0);
        candidates.push((
            "in-top",
            Some(top),
            None,
            Some(r.left() + 16.0),
            Some(vw - r.right() + 16.0),
            Some(w),
            false,
        ));
        candidates.push((
            "in-bottom",
            None,
            Some(vh - foot + 8.0),
            Some(r.left() + 16.0),
            Some(vw - r.right() + 16.0),
            Some(w),
            false,
        ));
    } else if vw < 768.0 {
        let top = elements(".topbar").first().map_or(16.0, |e| {
            let r = e.get_bounding_client_rect();
            if r.height() > 0.0 && r.bottom() > 0.0 {
                r.bottom() + 8.0
            } else {
                16.0
            }
        });
        candidates.push(("top", Some(top), None, Some(16.0), Some(16.0), None, false));
    } else {
        let column = elements(".main-col")
            .first()
            .map(Element::get_bounding_client_rect);
        candidates.push((
            "top",
            Some(16.0),
            None,
            Some(column.as_ref().map_or(0.0, |r| r.left()) + 24.0),
            Some(vw - column.as_ref().map_or(vw, |r| r.right()) + 24.0),
            Some(440.0),
            true,
        ));
    }
    candidates.push(("bottom", None, None, None, None, None, false));
    let kept = POSITION.with(|position| {
        let mut position = position.borrow_mut();
        let kept = if position.host.as_ref() == Some(&host) && (!position.choose || focused_toast())
        {
            position.key.clone()
        } else {
            None
        };
        position.host = Some(host);
        position.choose = false;
        kept
    });
    if let Some(kept) = kept
        && let Some((key, top, bottom, left, right, width, _)) =
            candidates.iter().find(|(key, ..)| *key == kept)
    {
        apply(&region, key, *top, *bottom, *left, *right, *width);
        return;
    }
    for strict in [true, false] {
        for (key, t, b, l, r, w, avoid) in &candidates {
            apply(&region, key, *t, *b, *l, *r, *w);
            let bounds = region.get_bounding_client_rect();
            if focus_rect.as_ref().is_some_and(|f| overlaps(&bounds, f)) {
                continue;
            }
            if strict&&*avoid&&elements("main a[href],main button,main input:not([type=hidden]),main select,main textarea,main [role=tab],main [tabindex='0']").iter().any(|e|{let r=e.get_bounding_client_rect();r.width()>0.0&&r.height()>0.0&&overlaps(&r,&bounds)}){continue;}
            POSITION.with(|position| position.borrow_mut().key = Some((*key).into()));
            return;
        }
    }
}
