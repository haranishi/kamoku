// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::{core::theme, prelude::*};
mod commands;
pub(crate) use commands::{help, palette};

const NAV: &[(&str, &str, &str, &str, &str, &str)] = &[
    ("home", "ホーム", "", "#/", "home", "g h"),
    ("entry", "取引を登録", "登録", "#/entry", "plus-square", "n"),
    ("inbox", "未確認", "", "#/inbox", "inbox", "g i"),
    ("import", "取込", "", "#/import", "import", ""),
    ("journal", "仕訳帳", "", "#/journal", "book", "g j"),
    ("reports", "帳票", "", "#/reports", "chart", "g r"),
    ("wallets", "口座", "", "#/wallets", "wallet", "g w"),
    ("accounts", "勘定科目", "科目", "#/accounts", "list", ""),
    ("partners", "取引先", "", "#/partners", "building", ""),
    ("settings", "設定", "", "#/settings", "sliders", "g s"),
];
fn brand() -> Element {
    let a = link("", "#/", "brand");
    let mark = h("span", "brand-mark", "");
    attrs(&mark, &[("aria-hidden", "true")]);
    append(&mark, &[icon("book")]);
    append(&a, &[mark, h("span", "brand-text", "kamoku")]);
    a
}
fn kbd(text: &str) -> Element {
    let el = h("kbd", "nav-kbd", text);
    attrs(&el, &[("aria-hidden", "true")]);
    el
}
fn nav_link(item: &(&str, &str, &str, &str, &str, &str), with_keys: bool) -> Element {
    let (id, label, short, href, image, key) = *item;
    let a = link("", href, "nav-item");
    attrs(&a, &[("data-nav", id)]);
    if key == "n" {
        attrs(&a, &[("aria-keyshortcuts", key)]);
    }
    let name = h("span", "nav-label", "");
    append(
        &name,
        &[h(
            "span",
            if short.is_empty() {
                ""
            } else {
                "nav-label-long"
            },
            label,
        )],
    );
    if !short.is_empty() {
        append(&name, &[h("span", "nav-label-short", short)]);
    }
    let count = h("span", "nav-count", "");
    attrs(&count, &[("data-count", id), ("hidden", "")]);
    append(&a, &[icon(image), name, count]);
    if with_keys && !key.is_empty() {
        append(&a, &[kbd(key)]);
    }
    a
}
fn nav_list(keys: bool) -> Element {
    let list = h("ul", "nav", "");
    for (start, end, group) in [
        (0, 1, ""),
        (1, 5, "取引"),
        (5, 6, ""),
        (6, 9, "登録簿"),
        (9, 10, ""),
    ] {
        let section = h("ul", "nav", "");
        for item in &NAV[start..end] {
            let li = h("li", "", "");
            append(&li, &[nav_link(item, keys)]);
            append(&section, &[li]);
        }
        if group.is_empty() {
            while let Some(child) = section.first_element_child() {
                append(&list, &[child]);
            }
        } else {
            let li = h("li", "nav-group", "");
            append(&li, &[h("span", "nav-group-label", group), section]);
            append(&list, &[li]);
        }
    }
    list
}
fn action(
    ctx: &Context,
    label: &str,
    image: &str,
    action: &str,
    f: impl Fn(Event) + 'static,
) -> Element {
    let b = button("", "secondary");
    b.set_class_name("nav-item");
    attrs(&b, &[("data-action", action)]);
    append(&b, &[icon(image), h("span", "nav-label", label)]);
    ctx.on(&b, "click", f);
    b
}
fn open_menu() {
    let Some(ctx) = super::global_context() else {
        return;
    };
    let d = Dialog::new(&ctx, "メニュー");
    d.el.set_class_name("dialog drawer drawer--left menu-drawer");
    let list = nav_list(false);
    append(&d.body, std::slice::from_ref(&list));
    let x = d.clone();
    d.scope.on(&list, "click", move |e| {
        if e.target()
            .and_then(|v| v.dyn_into::<Element>().ok())
            .and_then(|v| v.closest("a").ok().flatten())
            .is_some()
        {
            x.close();
        }
    });
    d.show();
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep declarative DOM assembly and ordered event registration together"
)]
pub(crate) fn mount(ctx: &Context) {
    let Some(app) = document().get_element_by_id("app") else {
        return;
    };
    let sidebar = h("nav", "sidebar", "");
    attrs(&sidebar, &[("aria-label", "メインメニュー")]);
    let inner = h("div", "sidebar-inner", "");
    let head = h("div", "sidebar-head", "");
    let toggle = button("", "ghost");
    toggle.set_class_name("icon-btn rail-toggle");
    let rail_label = h("span", "rail-toggle-label", "広げる");
    attrs(&rail_label, &[("aria-hidden", "true")]);
    append(&toggle, &[icon("panel"), rail_label.clone()]);
    append(&head, &[brand(), toggle.clone()]);
    let rail = Rc::new(Cell::new(
        theme::stored("kamoku.rail").as_deref() == Some("open"),
    ));
    let set_rail = {
        let app = app.clone();
        let toggle = toggle.clone();
        let rail = rail.clone();
        Rc::new(move |open: bool| {
            rail.set(open);
            attrs(&app, &[("data-rail", if open { "open" } else { "closed" })]);
            let name = if open {
                "サイドバーを細くする"
            } else {
                "サイドバーを広げる"
            };
            attrs(
                &toggle,
                &[
                    ("aria-expanded", if open { "true" } else { "false" }),
                    ("aria-label", name),
                    ("title", name),
                ],
            );
            set_text(&rail_label, if open { "細くする" } else { "広げる" });
            theme::store("kamoku.rail", if open { "open" } else { "closed" });
        })
    };
    set_rail(rail.get());
    ctx.on(&toggle, "click", move |_| set_rail(!rail.get()));
    let foot = h("div", "sidebar-foot", "");
    let command = action(ctx, "コマンド", "search", "palette", |_| palette());
    append(
        &command,
        &[kbd(&format!("{} K", super::core::keys::mod_label()))],
    );
    attrs(
        &command,
        &[(
            "aria-keyshortcuts",
            if super::core::keys::mod_label() == "⌘" {
                "Meta+K"
            } else {
                "Control+K"
            },
        )],
    );
    let help_button = action(ctx, "ショートカット", "keyboard", "help", |_| help());
    attrs(&help_button, &[("aria-keyshortcuts", "?")]);
    if let Some(label) = help_button.query_selector(".nav-label").ok().flatten() {
        replace(
            &label,
            &[
                h("span", "nav-label-long", "ショートカット"),
                h("span", "nav-label-short", "キー"),
            ],
        );
    }
    append(&help_button, &[kbd("?")]);
    let theme_button = action(ctx, "", "monitor", "theme", |_| cycle_theme());
    let theme_label = h("span", "nav-label", "");
    append(
        &theme_label,
        &[
            h("span", "nav-label-long theme-label", ""),
            h("span", "nav-label-short", "表示"),
        ],
    );
    replace(&theme_button, &[icon("monitor"), theme_label]);
    append(&foot, &[command, help_button, theme_button]);
    append(
        &inner,
        &[head, h("p", "sidebar-business", ""), nav_list(true), foot],
    );
    append(&sidebar, &[inner]);
    let top = h("header", "topbar", "");
    let actions = h("div", "topbar-actions", "");
    for (image, text, label, act) in [
        ("search", "検索", "検索（コマンド一覧を開く）", "palette"),
        ("monitor", "表示", "表示", "theme"),
        ("menu", "メニュー", "メニュー", "menu"),
    ] {
        let b = button("", "secondary");
        b.set_class_name(if act == "menu" {
            "icon-btn topbar-btn topbar-menu"
        } else {
            "icon-btn topbar-btn"
        });
        attrs(
            &b,
            &[
                ("aria-label", label),
                ("title", label),
                ("data-action", act),
            ],
        );
        if act == "menu" {
            attrs(&b, &[("aria-haspopup", "dialog")]);
        }
        let label = h("span", "", text);
        attrs(&label, &[("aria-hidden", "true")]);
        append(&b, &[icon(image), label]);
        ctx.on(&b, "click", move |_| match act {
            "palette" => palette(),
            "theme" => cycle_theme(),
            _ => open_menu(),
        });
        append(&actions, &[b]);
    }
    append(&top, &[brand(), actions]);
    app.insert_before(&sidebar, app.first_child().as_ref())
        .expect("sidebar mount");
    app.insert_before(&top, Some(&sidebar))
        .expect("topbar mount");
    let bottom = h("nav", "bottomnav", "");
    attrs(&bottom, &[("aria-label", "よく使う画面")]);
    for id in ["home", "entry", "inbox", "journal"] {
        let item = NAV.iter().find(|v| v.0 == id).expect("navigation item");
        let a = link("", item.3, "bottomnav-item");
        attrs(&a, &[("data-nav", id)]);
        let count = h("span", "nav-count bottomnav-count", "");
        attrs(&count, &[("data-count", id), ("hidden", "")]);
        append(
            &a,
            &[
                icon(item.4),
                h("span", "", if item.2.is_empty() { item.1 } else { item.2 }),
                count,
            ],
        );
        append(&bottom, &[a]);
    }
    let menu = button("", "secondary");
    menu.set_class_name("bottomnav-item");
    attrs(&menu, &[("aria-haspopup", "dialog"), ("data-menu", "")]);
    append(&menu, &[icon("menu"), h("span", "", "メニュー")]);
    ctx.on(&menu, "click", |_| open_menu());
    append(&bottom, &[menu]);
    append(&app, &[bottom]);
    if let Some(skip) = document().get_element_by_id("skip-link") {
        ctx.on(&skip, "click", |e| {
            e.prevent_default();
            if let Some(el) = document().query_selector("main h1").ok().flatten() {
                attrs(&el, &[("tabindex", "-1")]);
                focus(&el);
            }
        });
    }
    super::components::notice::start(&ctx.scope);
    sync_theme();
}
fn cycle_theme() {
    theme::set_theme(match theme::get_theme().as_str() {
        "os" => "light",
        "light" => "dark",
        _ => "os",
    });
    if let Some(ctx) = super::global_context() {
        ctx.notice_replace(
            "info",
            &format!("表示を「{}」にしました。", theme::label()),
            "appearance",
        );
    }
}
pub(crate) fn sync_theme() {
    let label = format!("表示: {}", theme::label());
    for el in all(".theme-label") {
        set_text(&el, &label);
    }
    for el in all(".topbar [data-action=theme]") {
        attrs(&el, &[("aria-label", &label), ("title", &label)]);
    }
}
fn all(selector: &str) -> Vec<Element> {
    document()
        .query_selector_all(selector)
        .map(|n| {
            (0..n.length())
                .filter_map(|i| n.item(i).and_then(|v| v.dyn_into().ok()))
                .collect()
        })
        .unwrap_or_default()
}
pub(crate) fn sync(ctx: &Context) {
    let first = ctx.route.path.split('/').nth(1).unwrap_or("");
    let nav = if first.is_empty() {
        "home"
    } else if first == "entry" && !ctx.route.param("id").is_empty() {
        "journal"
    } else {
        first
    };
    for el in all("[data-nav]") {
        if el.get_attribute("data-nav").as_deref() == Some(nav) {
            attrs(&el, &[("aria-current", "page")]);
        } else {
            let _ = el.remove_attribute("aria-current");
        }
    }
    for el in all("[data-menu]") {
        if ["home", "entry", "inbox", "journal", "setup"].contains(&nav) {
            let _ = el.remove_attribute("aria-current");
        } else {
            attrs(&el, &[("aria-current", "true")]);
        }
    }
    if let Some(settings) = ctx.peek("settings") {
        for el in all(".sidebar-business") {
            set_text(&el, settings["business_name"].as_str().unwrap_or(""));
        }
    }
    if let Some(st) = ctx.peek("status") {
        let count = st["inbox"]["draft"].as_u64().unwrap_or(0);
        for el in all("[data-count=inbox]") {
            if count > 0 {
                let _ = el.remove_attribute("hidden");
                set_text(&el, &count.to_string());
                attrs(&el, &[("aria-label", &format!("{count}件"))]);
            } else {
                attrs(&el, &[("hidden", "")]);
                set_text(&el, "");
            }
        }
        if let Some(banners) = document().get_element_by_id("banners") {
            replace(&banners, &[]);
            if st["mode"] == "demo" {
                let b = h("div", "banner banner--info", "");
                append(
                    &b,
                    &[
                        icon("info"),
                        h("strong", "banner-title", "デモのデータです"),
                        h(
                            "p",
                            "",
                            "架空の帳簿です。本物の帳簿の置き場所には書き込みません。",
                        ),
                    ],
                );
                append(&banners, &[b]);
            }
            if ctx.is_read_only() {
                let b = h("div", "banner banner--danger", "");
                b.set_id("banner-readonly");
                append(
                    &b,
                    &[
                        icon("alert"),
                        h("strong", "banner-title", "読み取り専用で開いています"),
                        h(
                            "p",
                            "",
                            &format!(
                                "{}設定の「データ」で、検査の結果を確かめてください。",
                                ctx.read_only_reason()
                            ),
                        ),
                    ],
                );
                append(&banners, &[b]);
            }
        }
    }
}
