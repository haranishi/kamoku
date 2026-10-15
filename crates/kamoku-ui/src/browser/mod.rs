// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

mod api;
mod dom;
mod events;
mod layout;
mod navigation;
mod view;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::model::Filters;
use view::View;

type App = Rc<RefCell<State>>;

struct State {
    view: View,
    filters: Filters,
    request: u64,
    loading: bool,
    composition_end: f64,
    focus: Option<String>,
}

pub(crate) fn start() -> Result<(), JsValue> {
    let view = layout::create()?;
    let filters = navigation::read()?;
    view.sync(&filters);
    let host = dom::document()?
        .get_element_by_id("app")
        .ok_or_else(|| JsValue::from_str("app is unavailable"))?;
    host.set_text_content(None);
    dom::append(&host, &view.root)?;
    let app = Rc::new(RefCell::new(State {
        view,
        filters,
        request: 0,
        loading: false,
        composition_end: f64::NEG_INFINITY,
        focus: navigation::remembered_entry(),
    }));
    events::attach(&app)?;
    load(&app);
    Ok(())
}

fn load(app: &App) {
    let (filters, request) = {
        let mut state = app.borrow_mut();
        state.request = state.request.wrapping_add(1);
        state.loading = true;
        if state.view.loading().is_err() {
            return;
        }
        (state.filters.clone(), state.request)
    };
    let app = Rc::clone(app);
    spawn_local(async move {
        let result = api::entries(&filters).await;
        let mut state = app.borrow_mut();
        if state.request != request {
            return;
        }
        state.loading = false;
        match result {
            Ok(page) => {
                if state.view.paint(&page, &filters).is_err() {
                    let _ = state
                        .view
                        .fail("画面を更新できませんでした。もう一度読み込んでください。");
                    return;
                }
                if let Some(id) = state.focus.take() {
                    let _ = events::focus_entry(&state.view, &id);
                }
            }
            Err(message) => {
                let _ = state.view.fail(&message);
            }
        }
    });
}

fn apply(app: &App, filters: Filters, first: bool) -> Result<(), JsValue> {
    let changed = filters != app.borrow().filters;
    if changed {
        navigation::push(&filters)?;
    }
    {
        let mut state = app.borrow_mut();
        state.filters = filters;
        state.view.sync(&state.filters);
        state.focus = first.then(String::new);
        if !changed {
            if first {
                events::focus_entry(&state.view, "")?;
            }
            return Ok(());
        }
    }
    load(app);
    Ok(())
}

fn restore(app: &App) -> Result<(), JsValue> {
    let filters = navigation::read()?;
    {
        let mut state = app.borrow_mut();
        state.filters = filters;
        state.view.sync(&state.filters);
        state.focus = navigation::remembered_entry();
    }
    load(app);
    Ok(())
}

fn show_failure(app: &App, result: &Result<(), JsValue>) {
    if result.is_err() {
        let _ = app
            .borrow()
            .view
            .fail("画面の操作に失敗しました。もう一度読み込んでください。");
    }
}
