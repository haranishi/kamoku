// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

mod components;
mod core;
pub(crate) mod prelude;
pub(crate) mod screens;
mod shell;

use prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::spawn_local;

struct Application {
    shared: Rc<core::context::Shared>,
    global: Scope,
    current: Option<Screen>,
    context: Option<Context>,
    previous: Option<String>,
}
thread_local! {static APP:RefCell<Option<Application>>=const{RefCell::new(None)};}

#[wasm_bindgen(start)]
fn start() -> Result<(), JsValue> {
    if window().location().pathname()?.starts_with("/rust/") {
        return crate::browser::start();
    }
    core::theme::apply();
    let global = Scope::new();
    let shared = Rc::new(core::context::Shared::default());
    APP.with(|cell| {
        *cell.borrow_mut() = Some(Application {
            shared: shared.clone(),
            global: global.clone(),
            current: None,
            context: None,
            previous: None,
        });
    });
    let ctx = Context {
        route: Route::current(),
        scope: global.clone(),
        shared,
    };
    shell::mount(&ctx);
    core::keys::start(&global);
    let scope = global.clone();
    spawn_local(async move {
        if let Err(e) = ctx.load("status", true).await {
            ctx.error(&e.message);
        }
        if let Err(e) = ctx.load("settings", true).await {
            ctx.error(&e.message);
        }
        if let Err(e) = ctx.load("accounts", true).await {
            ctx.error(&e.message);
        }
        core::route::start(&scope);
    });
    Ok(())
}
pub(crate) fn current_scope() -> Option<Scope> {
    APP.with(|a| {
        a.borrow()
            .as_ref()
            .and_then(|a| a.current.as_ref().map(|s| s.scope.clone()))
    })
}
pub(crate) fn current_context() -> Option<Context> {
    APP.with(|a| a.borrow().as_ref().and_then(|a| a.context.clone()))
}
pub(crate) fn global_context() -> Option<Context> {
    APP.with(|a| {
        a.borrow().as_ref().map(|a| Context {
            route: Route::current(),
            scope: a.global.clone(),
            shared: a.shared.clone(),
        })
    })
}
pub(crate) fn set_previous(previous: String) {
    APP.with(|a| {
        if let Some(a) = a.borrow_mut().as_mut() {
            a.previous = Some(previous);
        }
    });
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep declarative DOM assembly and ordered event registration together"
)]
pub(crate) fn render() {
    let Some((shared, previous, old_context, update)) = APP.with(|a| {
        a.borrow().as_ref().map(|a| {
            (
                a.shared.clone(),
                a.previous.clone(),
                a.context.clone(),
                a.current.as_ref().and_then(|s| s.update.clone()),
            )
        })
    }) else {
        return;
    };
    let mut route = Route::current();
    route.previous_hash = previous;
    if shared
        .values
        .borrow()
        .get("status")
        .is_some_and(|s| s["setup_done"] == false)
        && route.path != "/setup"
    {
        core::route::navigate("#/setup", true);
        return;
    }
    core::route::set_title(&route);
    if let (Some(old), Some(update)) = (old_context, update)
        && old.route.path.split('/').nth(1) == route.path.split('/').nth(1)
    {
        let ctx = Context {
            route: route.clone(),
            scope: old.scope,
            shared: shared.clone(),
        };
        if update(ctx.clone()) {
            APP.with(|a| {
                if let Some(a) = a.borrow_mut().as_mut() {
                    a.context = Some(ctx.clone());
                }
            });
            shell::sync(&ctx);
            return;
        }
    }
    let old = APP.with(|a| a.borrow_mut().as_mut().and_then(|a| a.current.take()));
    if let Some(old) = old {
        old.scope.dispose();
    }
    let ctx = Context {
        route: route.clone(),
        scope: Scope::new(),
        shared,
    };
    let mounted = match route.path.split('/').nth(1).unwrap_or_default() {
        "" => screens::home::mount(ctx.clone()),
        "entry" => screens::entry::mount(ctx.clone()),
        "journal" => screens::journal::mount(ctx.clone()),
        "inbox" => screens::inbox::mount(ctx.clone()),
        "import" => screens::imports::mount(ctx.clone()),
        "reports" => screens::reports::mount(ctx.clone()),
        "wallets" | "accounts" | "partners" => screens::masters::mount(ctx.clone()),
        "settings" => screens::settings::mount(ctx.clone()),
        "setup" => screens::setup::mount(ctx.clone()),
        _ => {
            let root = h("div", "page", "");
            append(
                &root,
                &[
                    h("h1", "", "ページが見つかりません"),
                    h(
                        "p",
                        "",
                        "このアドレスの画面はありません。ホームから選び直してください。",
                    ),
                    link("ホームへ", "#/", "btn"),
                ],
            );
            Ok(Screen::new(&ctx, root))
        }
    };
    let screen = if let Ok(screen) = mounted {
        screen
    } else {
        let root = h("section", "banner banner--danger", "");
        append(
            &root,
            &[
                h("h1", "", "画面を開けませんでした"),
                link("ホームへ", "#/", "btn"),
            ],
        );
        Screen::new(&ctx, root)
    };
    if let Some(outlet) = document().get_element_by_id("main") {
        replace(&outlet, std::slice::from_ref(&screen.root));
    }
    if !screen.manages_focus
        && let Some(title) = screen.root.query_selector("h1").ok().flatten()
    {
        attrs(&title, &[("tabindex", "-1")]);
        focus(&title);
    }
    APP.with(|a| {
        if let Some(a) = a.borrow_mut().as_mut() {
            a.current = Some(screen);
            a.context = Some(ctx.clone());
        }
    });
    shell::sync(&ctx);
}
