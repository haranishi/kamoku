// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    Shared, edit,
    model::{KINDS, tax_label, wallet_label},
};
use crate::app::prelude::*;
use crate::app::screens::{
    home::common::{amount, box_node, cell, empty_value, fact, list, n, row_open, s, table, yes},
    reports::tree,
};
use serde_json::Value;
use web_sys::Element;

#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(super) fn paint(ctx: &Context, state: &Shared) {
    let snapshot = state.borrow();
    if snapshot.kind == "wallet" {
        wallets(
            ctx,
            state,
            &snapshot.body,
            &snapshot.items,
            &snapshot.balances,
            &snapshot.counts,
        );
        return;
    }
    let query = normalize_search(&snapshot.query);
    let hits: Vec<_> = snapshot
        .items
        .iter()
        .filter(|a| snapshot.kind == "partner" || snapshot.all || a["enabled"] != false)
        .filter(|a| {
            query.is_empty()
                || normalize_search(&format!(
                    "{} {} {} {}",
                    s(a, "name"),
                    s(a, "reading"),
                    s(a, "code"),
                    s(a, "invoice_number")
                ))
                .contains(&query)
        })
        .cloned()
        .collect();
    set_text(
        &snapshot.count,
        &if snapshot.kind == "partner" {
            if query.is_empty() {
                format!("{}件", snapshot.items.len())
            } else {
                format!("{}件（全{}件）", hits.len(), snapshot.items.len())
            }
        } else {
            String::new()
        },
    );
    if hits.is_empty() {
        replace(
            &snapshot.body,
            &[box_node(
                "div",
                "card",
                &[
                    h(
                        "p",
                        "",
                        if snapshot.kind == "partner" {
                            "当てはまる取引先はありません"
                        } else {
                            "当てはまる科目はありません"
                        },
                    ),
                    h(
                        "p",
                        "muted",
                        "探す言葉を変えるか、「使っていない科目も出す」を選んでください。",
                    ),
                ],
            )],
        );
        return;
    }
    let mut sections = Vec::new();
    if snapshot.kind == "partner" {
        let (table, tbody) = table(
            "取引先の一覧（行で Enter を押すと直す）",
            &["名前", "読み", "登録番号", "メモ", ""],
            "table--stack partners-table",
        );
        watch_table(ctx, &table);
        if let Ok(Some(head)) = table.query_selector("thead th:last-child") {
            head.set_class_name("row-go");
        }
        for partner in hits {
            let row = h(
                "tr",
                if partner["enabled"] == false {
                    "is-off"
                } else {
                    ""
                },
                "",
            );
            let name = cell(
                &row,
                "",
                h("span", "acct-name keep", s(&partner, "name")),
                "wrap",
            );
            if partner["enabled"] == false {
                append(&name, &[h("span", "badge badge--muted", "使っていない")]);
            }
            cell(
                &row,
                "読み",
                if s(&partner, "reading").is_empty() {
                    empty_value()
                } else {
                    h("span", "keep", s(&partner, "reading"))
                },
                "",
            );
            cell(
                &row,
                "登録番号",
                if s(&partner, "invoice_number").is_empty() {
                    empty_value()
                } else {
                    h("code", "mono", s(&partner, "invoice_number"))
                },
                "",
            );
            cell(
                &row,
                "メモ",
                if s(&partner, "note").is_empty() {
                    empty_value()
                } else {
                    h("span", "keep", s(&partner, "note"))
                },
                "wrap",
            );
            cell(
                &row,
                "",
                box_node(
                    "span",
                    "row-go-mark",
                    &[h("span", "", "直す"), icon("chevron-right")],
                ),
                "row-go",
            );
            wire(ctx, state, &row, partner);
            append(&tbody, &[row]);
        }
        sections.push(table);
    } else {
        for (kind, label) in KINDS {
            let rows: Vec<_> = hits
                .iter()
                .filter(|a| s(a, "kind") == *kind)
                .cloned()
                .collect();
            if rows.is_empty() {
                continue;
            }
            let ordered =
                tree::tree_rows(&rows, &snapshot.items, &[]).unwrap_or_else(|_| rows.clone());
            let (table, tbody) = table(
                &format!("{label}の科目（行で Enter を押すと直す）"),
                &["名前", "読み", "コード", "既定の税区分", "既定の割合"],
                "table--stack accounts-table",
            );
            watch_table(ctx, &table);
            for source in ordered {
                let Some(account) = snapshot
                    .items
                    .iter()
                    .find(|a| s(a, "code") == s(&source, "code"))
                else {
                    continue;
                };
                let row = h(
                    "tr",
                    if account["enabled"] == false {
                        "is-off"
                    } else {
                        ""
                    },
                    "",
                );
                let name = h(
                    "span",
                    if yes(&source, "parent") {
                        "acct-name is-parent"
                    } else {
                        "acct-name"
                    },
                    "",
                );
                append(&name, &[h("span", "keep", s(account, "name"))]);
                if account["wallet"].is_object() {
                    append(&name, &[h("span", "badge badge--muted", "口座")]);
                }
                if account["enabled"] == false {
                    append(&name, &[h("span", "badge badge--muted", "使っていない")]);
                }
                let main = box_node(
                    "div",
                    "acct-main",
                    &[
                        name,
                        box_node(
                            "span",
                            "acct-sub",
                            &[
                                h("span", "keep", s(account, "reading")),
                                h("code", "mono", s(account, "code")),
                            ],
                        ),
                    ],
                );
                let _ = main
                    .unchecked_ref::<HtmlElement>()
                    .style()
                    .set_property("--depth", &n(&source, "depth").to_string());
                cell(&row, "名前", main, "acct-cell");
                cell(
                    &row,
                    "読み",
                    h("span", "keep", s(account, "reading")),
                    "acct-col-wide",
                );
                cell(
                    &row,
                    "コード",
                    h("code", "mono", s(account, "code")),
                    "acct-col-wide",
                );
                cell(
                    &row,
                    "既定の税区分",
                    h(
                        "span",
                        "keep",
                        if matches!(*kind, "revenue" | "expense") {
                            tax_label(s(account, "default_tax"))
                        } else {
                            "—"
                        },
                    ),
                    "acct-tax",
                );
                cell(
                    &row,
                    "既定の割合",
                    h(
                        "span",
                        "",
                        &if *kind == "expense" {
                            format!("{}%", n(account, "default_pct"))
                        } else {
                            "—".into()
                        },
                    ),
                    "acct-pct num",
                );
                wire(ctx, state, &row, account.clone());
                append(&tbody, &[row]);
            }
            if let Ok(heads) = table.query_selector_all("th") {
                for i in [1, 2] {
                    if let Some(node) = heads.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                        node.set_class_name("acct-col-wide");
                    }
                }
            }
            let heading = h("h2", "card-title", &format!("{label} {}件", rows.len()));
            heading.set_id(&format!("kind-{kind}"));
            let section = box_node("section", "masters-section", &[heading, table]);
            attrs(&section, &[("aria-labelledby", &format!("kind-{kind}"))]);
            sections.push(section);
        }
    }
    replace(&snapshot.body, &sections);
}
fn watch_table(ctx: &Context, wrapper: &Element) {
    if let Ok(Some(inner)) = wrapper.query_selector("table") {
        focus_when_scrollable(ctx, wrapper, &inner);
    }
}
use wasm_bindgen::JsCast;
fn wire(ctx: &Context, state: &Shared, row: &Element, value: Value) {
    let ctx2 = ctx.clone();
    let state = state.clone();
    row_open(ctx, row, move || edit(&ctx2, &state, Some(value.clone())));
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
fn wallets(
    ctx: &Context,
    state: &Shared,
    body: &Element,
    accounts: &[Value],
    balances: &Value,
    counts: &std::collections::BTreeMap<String, i64>,
) {
    let mut wallets: Vec<_> = accounts
        .iter()
        .filter(|a| a["wallet"].is_object())
        .cloned()
        .collect();
    wallets.sort_by_key(|a| n(a, "sort"));
    let active = h("ul", "wallet-list", "");
    attrs(&active, &[("aria-label", "使っている口座")]);
    let off = h("ul", "wallet-list", "");
    for wallet in wallets {
        let code = s(&wallet, "code");
        let kind = s(&wallet["wallet"], "type");
        let balance = list(balances)
            .into_iter()
            .find(|a| s(a, "code") == code)
            .map(|a| n(&a, "balance"));
        let name = box_node("p", "wallet-name", &[h("span", "keep", s(&wallet, "name"))]);
        if wallet["enabled"] == false {
            append(&name, &[h("span", "badge badge--muted", "使っていない")]);
        }
        let note = wallet_label(kind);
        let main = box_node(
            "div",
            "wallet-main",
            &[
                name,
                h(
                    "p",
                    "muted small",
                    &format!(
                        "{} {}",
                        if note == s(&wallet, "name") { "" } else { note },
                        s(&wallet["wallet"], "institution")
                    ),
                ),
            ],
        );
        let ledger = link(
            "",
            &build_hash("/reports/ledger", &[("account", code)]),
            "btn btn--sm wallet-link",
        );
        append(
            &ledger,
            &[
                box_node(
                    "span",
                    "",
                    &[
                        h("span", "wallet-link-long", "この口座の"),
                        h("span", "", "元帳"),
                    ],
                ),
                h(
                    "span",
                    "visually-hidden",
                    &format!("（{}）", s(&wallet, "name")),
                ),
            ],
        );
        let drafts = link(
            "",
            &build_hash("/inbox", &[("wallet", code)]),
            "btn btn--sm wallet-link",
        );
        append(
            &drafts,
            &[
                h("span", "", "未確認"),
                h(
                    "span",
                    if counts.get(code).copied().unwrap_or(0) > 0 {
                        "badge badge--info"
                    } else {
                        "muted"
                    },
                    &format!("{}件", number(counts.get(code).copied().unwrap_or(0))),
                ),
                h(
                    "span",
                    "visually-hidden",
                    &format!("（{}）", s(&wallet, "name")),
                ),
            ],
        );
        let edit_button = button("直す", "");
        attrs(
            &edit_button,
            &[("aria-label", &format!("「{}」を直す", s(&wallet, "name")))],
        );
        disabled(&edit_button, ctx.is_read_only());
        let c = ctx.clone();
        let shared = state.clone();
        let w = wallet.clone();
        ctx.on(&edit_button, "click", move |_| {
            edit(&c, &shared, Some(w.clone()));
        });
        let item = box_node(
            "li",
            if wallet["enabled"] == false {
                "card wallet-item is-off"
            } else {
                "card wallet-item"
            },
            &[
                box_node("span", "row-icon wallet-icon", &[icon("wallet")]),
                main,
                box_node(
                    "div",
                    "wallet-balance",
                    &[
                        h(
                            "span",
                            "muted small",
                            if kind == "card" {
                                "未払い額"
                            } else {
                                "残高"
                            },
                        ),
                        box_node(
                            "span",
                            "wallet-amount",
                            &[if wallet["enabled"] == false {
                                empty_value()
                            } else {
                                balance.map_or_else(empty_value, amount)
                            }],
                        ),
                    ],
                ),
                box_node("div", "wallet-links", &[ledger, drafts, edit_button]),
            ],
        );
        if wallet["enabled"] == false {
            append(&off, &[item]);
        } else {
            append(&active, &[item]);
        }
    }
    let mut children = vec![
        box_node(
            "dl",
            "card totals wallet-totals",
            &[
                fact("現金と預金の合計", n(balances, "cash_total")),
                fact("カードの未払いの合計", n(balances, "owed_total")),
            ],
        ),
        active,
    ];
    if off.child_element_count() > 0 {
        children.push(box_node(
            "section",
            "masters-off",
            &[h("h2", "card-title", "使っていない口座"), off],
        ));
    }
    replace(body, &children);
}
