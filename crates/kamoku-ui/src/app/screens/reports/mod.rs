// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub(crate) mod period;
mod picker;
mod table;
pub(crate) mod tree;
mod views;

use super::{
    home::common::{banner_node, box_node, el, heading, list, n, s, set_hidden},
    masters::model::kind_label,
};
use crate::app::prelude::*;
use num_traits::ToPrimitive;
use period::Period;
use picker::Picker;

const TABS: [(&str, &str); 6] = [
    ("monthly", "月次推移"),
    ("income-statement", "損益計算書"),
    ("balance-sheet", "貸借対照表"),
    ("trial-balance", "試算表"),
    ("ledger", "総勘定元帳"),
    ("tax-summary", "消費税"),
];
#[derive(Clone)]
struct Csv {
    path: String,
    params: Vec<(String, String)>,
}
struct Reports {
    route: Route,
    tab: usize,
    period: Period,
    account: String,
    accounts: Vec<Value>,
    settings: Value,
    ready: bool,
    sequence: u64,
    tabbar: Element,
    tabs: Vec<Element>,
    panel: Element,
    picker_host: Element,
    account_host: Element,
    title: Element,
    previous_period: Element,
    content: Element,
    pending: Element,
    encoding: HtmlSelectElement,
    export: Element,
    csv: Option<Csv>,
    picker: Option<Picker>,
    combo: Option<Combo>,
    controls: Option<Scope>,
    view: Option<Scope>,
}
type Shared = Rc<RefCell<Reports>>;
fn current_period(route: &Route, tab: usize) -> Period {
    Period::read(|name| route.query(name), &today(), tab == 2, tab == 0)
}
fn address(tab: usize, period: &Period, account: &str) -> String {
    let mut query = period.query();
    if tab == 4 && !account.is_empty() {
        query.push(("account".into(), account.into()));
    }
    let pairs: Vec<_> = query
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    build_hash(&format!("/reports/{}", TABS[tab].0), &pairs)
}
fn go(ctx: &Context, tab: usize, period: &Period, account: &str) {
    ctx.navigate(&address(tab, period, account), false);
}
fn go_tab(ctx: &Context, state: &Shared, tab: usize) {
    let (period, account) = {
        let view = state.borrow();
        if tab == view.tab {
            return;
        }
        (
            view.period.carry(&today(), tab == 2, tab == 0),
            if tab == 4 {
                view.account.clone()
            } else {
                String::new()
            },
        )
    };
    go(ctx, tab, &period, &account);
}
fn overflow(tabbar: &Element) {
    let left = tabbar.scroll_left();
    let max = tabbar.scroll_width() - tabbar.client_width();
    let mut sides = vec![];
    if max > 1 {
        if left > 1 {
            sides.push("left");
        }
        if left < max - 1 {
            sides.push("right");
        }
    }
    if sides.is_empty() {
        let _ = tabbar.remove_attribute("data-more");
    } else {
        attrs(tabbar, &[("data-more", &sides.join(" "))]);
    }
}
fn sync_tabs(state: &Shared) {
    let view = state.borrow();
    for (i, tab) in view.tabs.iter().enumerate() {
        attrs(
            tab,
            &[
                (
                    "aria-selected",
                    if i == view.tab { "true" } else { "false" },
                ),
                ("tabindex", if i == view.tab { "0" } else { "-1" }),
            ],
        );
        if i == view.tab {
            attrs(&view.panel, &[("aria-labelledby", &tab.id())]);
            let bar = view.tabbar.get_bounding_client_rect();
            let rect = tab.get_bounding_client_rect();
            if view.tabbar.scroll_width() > view.tabbar.client_width() {
                let offset = rect.left() - bar.left() + f64::from(view.tabbar.scroll_left())
                    - (bar.width() - rect.width()) / 2.0;
                view.tabbar
                    .set_scroll_left(offset.max(0.0).round().to_i32().unwrap_or(i32::MAX));
            }
        }
    }
    overflow(&view.tabbar);
}
fn paint_title(state: &Shared) {
    let view = state.borrow();
    let name = if view.tab == 4 {
        view.accounts
            .iter()
            .find(|a| s(a, "code") == view.account)
            .map(|a| format!("{}（{}）", TABS[view.tab].1, s(a, "name")))
    } else {
        None
    };
    replace(
        &view.title,
        &[
            h("span", "", name.as_deref().unwrap_or(TABS[view.tab].1)),
            h("span", "", "　"),
            h("span", "nw", &view.period.label()),
        ],
    );
}
fn api_href(csv: &Csv, encoding: &str) -> String {
    let params = web_sys::UrlSearchParams::new().expect("URL parameters");
    for (k, v) in &csv.params {
        params.set(k, v);
    }
    params.set("encoding", encoding);
    format!("/api/v1{}?{}", csv.path, String::from(params.to_string()))
}
fn paint_export(state: &Shared) {
    let view = state.borrow();
    if let Some(csv) = &view.csv {
        for key in ["aria-disabled", "role", "title"] {
            let _ = view.export.remove_attribute(key);
        }
        attrs(
            &view.export,
            &[("href", &api_href(csv, &view.encoding.value()))],
        );
    } else {
        let _ = view.export.remove_attribute("href");
        attrs(
            &view.export,
            &[
                ("role", "link"),
                ("aria-disabled", "true"),
                ("title", "科目を選ぶと、書き出せます。"),
            ],
        );
    }
}
fn child_context(ctx: &Context, route: &Route, scope: &Scope) -> Context {
    Context {
        route: route.clone(),
        scope: scope.clone(),
        shared: ctx.shared.clone(),
    }
}
fn sync_controls(state: &Shared) {
    let (picker, combo, period, account) = {
        let view = state.borrow();
        (
            view.picker.clone(),
            view.combo.clone(),
            view.period.clone(),
            view.account.clone(),
        )
    };
    if let Some(p) = picker {
        p.set(period);
    }
    if let Some(c) = combo
        && c.value() != account
    {
        c.set_value(&account, false);
    }
}
fn controls(ctx: &Context, state: &Shared, rebuild: bool) {
    if !rebuild {
        sync_controls(state);
        return;
    }
    let (tab, period, account, accounts, settings, route, host, account_host, old) = {
        let mut view = state.borrow_mut();
        (
            view.tab,
            view.period.clone(),
            view.account.clone(),
            view.accounts.clone(),
            view.settings.clone(),
            view.route.clone(),
            view.picker_host.clone(),
            view.account_host.clone(),
            view.controls.take(),
        )
    };
    if let Some(scope) = old {
        scope.dispose();
    }
    let scope = Scope::new();
    let control = child_context(ctx, &route, &scope);
    let first = s(&settings, "opened_on")
        .get(..4)
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or_else(|| today()[..4].parse::<i32>().unwrap_or(period.year) - 2);
    let c = ctx.clone();
    let shared = state.clone();
    let invalid_state = state.clone();
    let picker = Picker::new(
        &control,
        period,
        first,
        tab == 2,
        tab == 0,
        move |period| {
            let (tab, account) = {
                let view = shared.borrow();
                (view.tab, view.account.clone())
            };
            go(&c, tab, &period, &account);
        },
        move |bad| {
            let view = invalid_state.borrow();
            set_hidden(&view.previous_period, !bad);
            let text = if bad {
                format!("前の期間（{}）を表示中です。", view.period.label())
            } else {
                String::new()
            };
            set_text(&view.previous_period, &text);
        },
    );
    replace(&host, std::slice::from_ref(&picker.el));
    replace(&account_host, &[]);
    let combo = if tab == 4 {
        let mut accounts = accounts;
        accounts.sort_by_key(tree::chart_key);
        accounts.sort_by_key(|a| !a["wallet"].is_object());
        let items=accounts.iter().map(|a|{let wallet=&a["wallet"];let words=match s(wallet,"type"){"cash"=>"げんきん 現金","bank"=>"よきん ふつうよきん 預金 普通預金","card"=>"かーど くれじっとかーど カード クレジットカード","ewallet"=>"でんしまねー 電子マネー",_=>""};ComboItem{value:s(a,"code").into(),label:s(a,"name").into(),reading:s(a,"reading").into(),meta:json!({"hint":if wallet.is_object(){"口座"}else{kind_label(s(a,"kind"))},"keywords":words})}}).collect();
        let combo = Combo::new(&control, items, &account);
        combo.input.set_placeholder("例: げんきん、通信費");
        combo.set_list_label("科目の候補");
        combo.set_no_match("合う科目がありません。ひらがなの読みか、科目の名前で探せます。");
        let shared = state.clone();
        let c = ctx.clone();
        combo.on_select(move |item, _, _| {
            if let Some(item) = item {
                let (tab, period, account) = {
                    let view = shared.borrow();
                    (view.tab, view.period.clone(), view.account.clone())
                };
                if item.value != account {
                    go(&c, tab, &period, &item.value);
                }
            }
            false
        });
        append(&account_host, &[field("科目", &combo.el, "", false).el]);
        Some(combo)
    } else {
        None
    };
    let mut view = state.borrow_mut();
    view.controls = Some(scope);
    view.picker = Some(picker);
    view.combo = combo;
}
enum FocusMemo {
    Entry(String),
    Content,
}
fn memo(content: &Element) -> Option<FocusMemo> {
    let active = web_sys::window()?.document()?.active_element()?;
    if !content.contains(Some(&active)) {
        return None;
    }
    Some(
        active
            .closest("tr[data-entry-id]")
            .ok()
            .flatten()
            .and_then(|tr| tr.get_attribute("data-entry-id"))
            .map_or(FocusMemo::Content, FocusMemo::Entry),
    )
}
fn refocus(content: &Element, title: &Element, memo: Option<FocusMemo>) {
    let Some(memo) = memo else { return };
    let entry = match memo {
        FocusMemo::Entry(id) => Some(id),
        FocusMemo::Content => None,
    };
    let document = web_sys::window().unwrap().document().unwrap();
    if document
        .active_element()
        .is_some_and(|e| e.tag_name() != "BODY")
    {
        return;
    }
    let rows = content.query_selector_all("tr[data-entry-id]").ok();
    let same = rows.as_ref().and_then(|rows| {
        (0..rows.length())
            .filter_map(|i| rows.item(i).and_then(|r| r.dyn_into::<Element>().ok()))
            .find(|r| {
                entry.as_ref().is_some_and(|id| {
                    r.get_attribute("data-entry-id").as_deref() == Some(id.as_str())
                })
            })
    });
    let first = content
        .query_selector("tr[data-entry-id][tabindex='0']")
        .ok()
        .flatten();
    focus(&same.or(first).unwrap_or_else(|| title.clone()));
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
fn pending(
    ctx: &Context,
    state: &Shared,
    seq: u64,
    tab: usize,
    period: Period,
    account: String,
    accounts: Vec<Value>,
) {
    let c = ctx.clone();
    let state = state.clone();
    ctx.task(async move {
        let wallet = if tab == 4
            && accounts
                .iter()
                .any(|a| s(a, "code") == account && a["wallet"].is_object())
        {
            account.as_str()
        } else {
            ""
        };
        let result = async {
            if tab == 4 && account.is_empty() {
                return Ok::<_, ApiError>(None);
            }
            let mut total = 0i64;
            let mut out = 0i64;
            let mut incoming = 0i64;
            let mut offset = 0usize;
            let mut complete = false;
            for _ in 0..20 {
                let offset_text = offset.to_string();
                let mut query = vec![
                    ("status", "draft"),
                    (
                        "to",
                        if tab == 2 {
                            period.as_of.as_str()
                        } else {
                            period.to.as_str()
                        },
                    ),
                    ("limit", "500"),
                    ("offset", &offset_text),
                ];
                if tab != 2 {
                    query.push(("from", &period.from));
                }
                if !wallet.is_empty() {
                    query.push(("wallet", wallet));
                }
                let value = c.get("/drafts", &query).await?;
                if !c.alive() || state.borrow().sequence != seq {
                    return Ok(None);
                }
                total = n(&value, "total");
                let items = list(&value);
                for item in &items {
                    let sum = if s(item, "direction") == "out" {
                        &mut out
                    } else {
                        &mut incoming
                    };
                    *sum = sum
                        .checked_add(n(item, "amount"))
                        .ok_or_else(|| ApiError::message("未確認の合計が大きすぎます。"))?;
                }
                offset = offset.saturating_add(items.len());
                if items.is_empty() || i64::try_from(offset).unwrap_or(i64::MAX) >= total {
                    complete = true;
                    break;
                }
            }
            if total == 0 {
                return Ok(None);
            }
            let text = if complete {
                format!(
                    "未確認 {}件（出金 {}円・入金 {}円）は、この集計に入っていません。 ",
                    number(total),
                    number(out),
                    number(incoming)
                )
            } else {
                format!("未確認 {}件は、この集計に入っていません。 ", number(total))
            };
            let href = if wallet.is_empty() {
                "#/inbox".into()
            } else {
                build_hash("/inbox", &[("wallet", wallet)])
            };
            let line = box_node(
                "p",
                "pending-note",
                &[
                    icon("info"),
                    box_node(
                        "span",
                        "",
                        &[h("span", "", &text), link("未確認を開く", &href, "tap")],
                    ),
                ],
            );
            Ok(Some(line))
        }
        .await;
        if c.alive() && state.borrow().sequence == seq {
            let host = state.borrow().pending.clone();
            let node = result.ok().flatten();
            set_hidden(&host, node.is_none());
            replace(&host, &node.into_iter().collect::<Vec<_>>());
        }
        Ok(())
    });
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
fn load(ctx: &Context, state: &Shared) {
    let (tab, period, account, accounts, settings, route, seq, content, title, old) = {
        let mut view = state.borrow_mut();
        view.sequence = view.sequence.wrapping_add(1);
        view.csv = None;
        (
            view.tab,
            view.period.clone(),
            view.account.clone(),
            view.accounts.clone(),
            view.settings.clone(),
            view.route.clone(),
            view.sequence,
            view.content.clone(),
            view.title.clone(),
            view.view.take(),
        )
    };
    let focus_memo = memo(&content);
    if let Some(scope) = old {
        scope.dispose();
    }
    replace(
        &content,
        &[h("p", "muted", &format!("{}を作っています…", TABS[tab].1))],
    );
    paint_title(state);
    paint_export(state);
    pending(
        ctx,
        state,
        seq,
        tab,
        period.clone(),
        account.clone(),
        accounts.clone(),
    );
    let c = ctx.clone();
    let state = state.clone();
    ctx.task(async move {
        let from = period.from.as_str();
        let to = period.to.as_str();
        let response = match tab {
            0 => {
                c.get(
                    "/reports/income-statement",
                    &[("from", from), ("to", to), ("by", "month")],
                )
                .await
            }
            1 => {
                c.get(
                    "/reports/income-statement",
                    &[("from", from), ("to", to), ("by", "total")],
                )
                .await
            }
            2 => {
                c.get("/reports/balance-sheet", &[("as_of", &period.as_of)])
                    .await
            }
            3 => {
                c.get("/reports/trial-balance", &[("from", from), ("to", to)])
                    .await
            }
            4 if !account.is_empty() => {
                c.get(
                    "/reports/ledger",
                    &[("account", &account), ("from", from), ("to", to)],
                )
                .await
            }
            4 => Ok(Value::Null),
            _ => {
                c.get("/reports/tax-summary", &[("from", from), ("to", to)])
                    .await
            }
        };
        if !c.alive() || state.borrow().sequence != seq {
            return Ok(());
        }
        let scope = Scope::new();
        let view_ctx = child_context(&c, &route, &scope);
        let rendered = response.and_then(|res| match tab {
            0 => views::monthly(&view_ctx, &res, &period, &accounts),
            1 => views::income(&view_ctx, &res, &accounts),
            2 => views::balance(&view_ctx, &res),
            3 => views::trial(&view_ctx, &res, &accounts),
            4 => Ok(views::ledger(
                &view_ctx,
                &res,
                &period,
                &account,
                &accounts,
                &route.hash,
            )),
            _ => Ok(views::tax(&view_ctx, &res, &settings)),
        });
        match rendered {
            Ok(node) => {
                let mut params = if tab == 2 {
                    vec![("as_of".into(), period.as_of)]
                } else {
                    vec![("from".into(), period.from), ("to".into(), period.to)]
                };
                if tab < 2 {
                    params.insert(
                        0,
                        ("by".into(), if tab == 0 { "month" } else { "total" }.into()),
                    );
                }
                if tab == 4 {
                    params.insert(0, ("account".into(), account.clone()));
                }
                state.borrow_mut().csv = if tab == 4 && account.is_empty() {
                    None
                } else {
                    Some(Csv {
                        path: format!(
                            "/export/{}.csv",
                            if tab == 0 {
                                "income-statement"
                            } else {
                                TABS[tab].0
                            }
                        ),
                        params,
                    })
                };
                replace(&content, &[node]);
                refocus(&content, &title, focus_memo);
                state.borrow_mut().view = Some(scope);
            }
            Err(e) => {
                scope.dispose();
                replace(
                    &content,
                    &[banner_node(
                        &format!("{}を作れませんでした", TABS[tab].1),
                        &e.message,
                        true,
                    )],
                );
            }
        }
        paint_export(&state);
        Ok(())
    });
}
fn move_row(state: &Shared, step: i32) -> bool {
    let content = state.borrow().content.clone();
    let Ok(nodes) = content.query_selector_all(".ledger-table tr[data-entry-id]") else {
        return false;
    };
    let rows: Vec<_> = (0..nodes.length())
        .filter_map(|i| nodes.item(i).and_then(|r| r.dyn_into::<Element>().ok()))
        .collect();
    if rows.is_empty() {
        return false;
    }
    let active = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .active_element()
        .and_then(|e| e.closest("tr[data-entry-id]").ok().flatten());
    let current = active
        .as_ref()
        .and_then(|active| rows.iter().position(|r| r == active));
    let next = current.map_or(0, |index| {
        if step < 0 {
            index.saturating_sub(1)
        } else {
            index.saturating_add(1).min(rows.len() - 1)
        }
    });
    for (i, row) in rows.iter().enumerate() {
        attrs(row, &[("tabindex", if i == next { "0" } else { "-1" })]);
    }
    focus(&rows[next]);
    true
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
#[allow(
    clippy::unnecessary_wraps,
    reason = "Every screen implements the same owned Context and fallible mount interface."
)]
pub(crate) fn mount(ctx: Context) -> Result<Screen, JsValue> {
    let tab = TABS
        .iter()
        .position(|(id, _)| *id == ctx.route.param("name"));
    let Some(tab) = tab else {
        let c = ctx.clone();
        let hash = ctx.route.hash.clone();
        ctx.task(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL))
                .await;
            if c.alive() {
                let suffix = hash
                    .split_once('?')
                    .map_or(String::new(), |(_, q)| format!("?{q}"));
                c.navigate(&format!("#/reports/monthly{suffix}"), true);
            }
            Ok(())
        });
        return Ok(Screen::new(&ctx, h("div", "", "")));
    };
    let root = h("div", "", "");
    let tabbar = h("div", "tabs report-tabs", "");
    attrs(
        &tabbar,
        &[
            ("role", "tablist"),
            ("aria-label", "帳票の種類"),
            ("data-allow-hscroll", ""),
        ],
    );
    let panel = h("div", "tabpanel report-panel", "");
    attrs(
        &panel,
        &[("role", "tabpanel"), ("id", &uid("reports-panel"))],
    );
    let tabs: Vec<_> = TABS
        .iter()
        .map(|(id, label)| {
            let tab = button(label, "");
            attrs(
                &tab,
                &[
                    ("role", "tab"),
                    ("class", "tab"),
                    ("id", &uid("report-tab")),
                    ("data-tab", id),
                    ("aria-controls", &panel.id()),
                ],
            );
            append(&tabbar, std::slice::from_ref(&tab));
            tab
        })
        .collect();
    let picker_host = h("div", "report-period", "");
    let account_host = h("div", "report-account", "");
    let title = h("h2", "report-title", "");
    attrs(&title, &[("aria-live", "polite"), ("tabindex", "-1")]);
    let previous_period = h("p", "report-stale", "");
    set_hidden(&previous_period, true);
    let content = h("div", "report-content", "");
    let pending = h("div", "report-pending", "");
    set_hidden(&pending, true);
    let encoding_value = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("kamoku.csv.encoding").ok().flatten())
        .filter(|s| s == "sjis")
        .unwrap_or_else(|| "utf8bom".into());
    let encoding = select(
        &[("utf8bom", "UTF-8（BOMつき）"), ("sjis", "Shift_JIS")],
        &encoding_value,
    );
    attrs(
        &el(&encoding),
        &[
            ("aria-label", "CSVの文字コード"),
            ("class", "select csv-enc"),
        ],
    );
    let export = link("CSVで書き出す", "", "btn csv-link");
    attrs(&export, &[("download", "")]);
    let export_box = box_node(
        "div",
        "report-export",
        &[
            h("span", "report-export-label", "この帳票を書き出す"),
            box_node("div", "report-export-row", &[el(&encoding), export.clone()]),
        ],
    );
    append(
        &panel,
        &[
            box_node(
                "div",
                "card report-toolbar",
                &[picker_host.clone(), account_host.clone()],
            ),
            pending.clone(),
            box_node(
                "div",
                "report-head",
                &[title.clone(), previous_period.clone()],
            ),
            content.clone(),
            export_box,
        ],
    );
    append(
        &root,
        &[heading("帳票", None), tabbar.clone(), panel.clone()],
    );
    let state = Rc::new(RefCell::new(Reports {
        route: ctx.route.clone(),
        tab,
        period: current_period(&ctx.route, tab),
        account: ctx.route.query("account"),
        accounts: vec![],
        settings: Value::Null,
        ready: false,
        sequence: 0,
        tabbar: tabbar.clone(),
        tabs: tabs.clone(),
        panel,
        picker_host,
        account_host,
        title,
        previous_period,
        content,
        pending,
        encoding: encoding.clone(),
        export: export.clone(),
        csv: None,
        picker: None,
        combo: None,
        controls: None,
        view: None,
    }));
    sync_tabs(&state);
    paint_title(&state);
    paint_export(&state);
    for (i, button) in tabs.iter().enumerate() {
        let c = ctx.clone();
        let shared = state.clone();
        ctx.on(button, "click", move |_| go_tab(&c, &shared, i));
    }
    let c = ctx.clone();
    let shared = state.clone();
    ctx.on(&tabbar, "keydown", move |event| {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if ime_ignore(key) {
            return;
        }
        let view = shared.borrow();
        let active = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .active_element();
        let Some(at) = active
            .as_ref()
            .and_then(|e| view.tabs.iter().position(|t| t == e))
        else {
            return;
        };
        let next = match key.key().as_str() {
            "ArrowRight" => (at + 1) % TABS.len(),
            "ArrowLeft" => (at + TABS.len() - 1) % TABS.len(),
            "Home" => 0,
            "End" => TABS.len() - 1,
            _ => return,
        };
        let button = view.tabs[next].clone();
        drop(view);
        key.prevent_default();
        focus(&button);
        go_tab(&c, &shared, next);
    });
    let b = tabbar.clone();
    ctx.on(&tabbar, "scroll", move |_| overflow(&b));
    let shared = state.clone();
    ctx.scope.listen(
        web_sys::window().unwrap().as_ref(),
        "resize",
        false,
        move |_| sync_tabs(&shared),
    );
    let shared = state.clone();
    ctx.on(&el(&encoding), "change", move |_| {
        if let Ok(Some(storage)) = web_sys::window().unwrap().local_storage() {
            let _ = storage.set_item("kamoku.csv.encoding", &shared.borrow().encoding.value());
        }
        paint_export(&shared);
    });
    let shared = state.clone();
    let c = ctx.clone();
    let b = export.clone();
    ctx.on(&export, "click", move |event| {
        if let Some(m) = event.dyn_ref::<web_sys::MouseEvent>()
            && (m.button() != 0 || m.ctrl_key() || m.meta_key() || m.shift_key() || m.alt_key())
        {
            return;
        }
        event.prevent_default();
        if b.get_attribute("aria-busy").as_deref() == Some("true") {
            return;
        }
        let csv = {
            let view = shared.borrow();
            view.csv.clone().map(|mut csv| {
                csv.params.push(("encoding".into(), view.encoding.value()));
                csv
            })
        };
        let Some(csv) = csv else { return };
        busy(&b, true);
        let c = c.clone();
        let b = b.clone();
        let task = c.clone();
        c.task(async move {
            let params: Vec<_> = csv
                .params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let result = task.download(&csv.path, &params).await;
            if task.alive() {
                busy(&b, false);
            }
            result
        });
    });
    let shared = state.clone();
    ctx.key(move |key| {
        if key.ctrl_key()
            || key.meta_key()
            || key.alt_key()
            || !single_keys_enabled()
            || key
                .target()
                .and_then(|t| t.dyn_into::<Element>().ok())
                .is_some_and(|e| is_typing_target(&e))
        {
            return false;
        }
        let picker = shared.borrow().picker.clone();
        match key.key().as_str() {
            "[" => picker.is_some_and(|p| p.shift(-1)),
            "]" => picker.is_some_and(|p| p.shift(1)),
            "j" => move_row(&shared, 1),
            "k" => move_row(&shared, -1),
            _ => false,
        }
    });
    let c = ctx.clone();
    let shared = state.clone();
    ctx.task(async move {
        let response = async {
            let settings = c.load("settings", true).await?;
            if !c.alive() {
                return Ok::<_, ApiError>(None);
            }
            let accounts = c.load("accounts", true).await?;
            Ok(Some((settings, list(&accounts))))
        }
        .await;
        if !c.alive() {
            return Ok(());
        }
        match response {
            Ok(Some((settings, accounts))) => {
                {
                    let mut view = shared.borrow_mut();
                    view.settings = settings;
                    view.accounts = accounts;
                    view.ready = true;
                }
                controls(&c, &shared, true);
                load(&c, &shared);
                sync_tabs(&shared);
            }
            Ok(None) => {}
            Err(e) => replace(
                &shared.borrow().content,
                &[banner_node(
                    "帳票の画面を準備できませんでした",
                    &e.message,
                    true,
                )],
            ),
        }
        Ok(())
    });
    let shared = state.clone();
    ctx.scope.cleanup(move || {
        let mut view = shared.borrow_mut();
        if let Some(scope) = view.controls.take() {
            scope.dispose();
        }
        if let Some(scope) = view.view.take() {
            scope.dispose();
        }
    });
    let mut screen = Screen::new(&ctx, root);
    screen.update = Some(Rc::new(move |next| {
        let Some(tab) = TABS
            .iter()
            .position(|(id, _)| *id == next.route.param("name"))
        else {
            return false;
        };
        let rebuild = {
            let mut view = state.borrow_mut();
            let changed = view.tab != tab;
            view.route = next.route.clone();
            view.tab = tab;
            view.period = current_period(&next.route, tab);
            view.account = next.route.query("account");
            changed
        };
        sync_tabs(&state);
        if state.borrow().ready {
            controls(&next, &state, rebuild);
            load(&next, &state);
        }
        true
    }));
    Ok(screen)
}
