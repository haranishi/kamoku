// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    chart,
    common::{amount, box_node, el, fact, list, n, s, text_input, yes},
};
use crate::app::prelude::*;
use crate::app::screens::reports::period::{days, shift_month};
use serde_json::Value;
use web_sys::Element;

fn card(title: &str, class: &str) -> Element {
    let heading = h("h2", "card-title", title);
    heading.set_id(&uid("home-title"));
    let node = box_node(
        "section",
        &format!("card home-card {class}"),
        &[box_node("div", "card-head", std::slice::from_ref(&heading))],
    );
    attrs(&node, &[("aria-labelledby", &heading.id())]);
    node
}
fn kpi(label: &str, value: i64, profit: bool) -> Element {
    let label = box_node("p", "kpi-label", &[h("span", "", label)]);
    if profit && value < 0 {
        append(&label, &[h("span", "badge badge--muted", "赤字")]);
    }
    let amount = box_node("p", "kpi-value", &[amount(value)]);
    let _ = amount
        .unchecked_ref::<HtmlElement>()
        .style()
        .set_property("--chars", &number(value).len().to_string());
    box_node(
        "div",
        if profit && value < 0 {
            "card kpi kpi--loss"
        } else {
            "card kpi"
        },
        &[label, amount],
    )
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(super) fn paint(
    ctx: &Context,
    body: &Element,
    dash: &Value,
    status: &Value,
    settings: &Value,
    month: &str,
) {
    let pending = n(&dash["next_actions"], "inbox_draft");
    let now = today();
    let current = year_month(&now);
    let day = now
        .get(8..)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(15);
    let year = now[..4].parse().unwrap_or(2026);
    let mm = now[5..7].parse().unwrap_or(1);
    let near = day <= 3 || day >= days(year, mm).saturating_sub(2);
    let main = h("div", "next-main", "");
    append(&main, &[h("h2", "card-title", "次にやること")]);
    let next = box_node(
        "section",
        if pending > 0 {
            "card next-card has-figures"
        } else {
            "card next-card"
        },
        std::slice::from_ref(&main),
    );
    if pending > 0 {
        append(
            &main,
            &[
                h(
                    "p",
                    "next-text",
                    &format!(
                        "未確認の取引が{}件あります。科目を決めて、確定しましょう。",
                        number(pending)
                    ),
                ),
                h(
                    "p",
                    "muted",
                    "取り込んだ明細のうち、まだ確定していないものです。確定すると、ホームと帳票の数字に入ります。",
                ),
                box_node(
                    "div",
                    "next-actions-buttons",
                    &[link("未確認を開く", "#/inbox", "btn btn--primary")],
                ),
            ],
        );
        let count = box_node(
            "span",
            "money",
            &[h("span", "num", &number(pending)), h("span", "unit", "件")],
        );
        append(
            &next,
            &[box_node(
                "dl",
                "next-figures",
                &[
                    box_node(
                        "div",
                        "",
                        &[h("dt", "", "未確認"), box_node("dd", "", &[count])],
                    ),
                    fact("そのうち出金", n(&dash["next_actions"], "inbox_out_amount")),
                ],
            )],
        );
    } else {
        append(
            &main,
            &[
                h("p", "next-text", "未確認はありません。"),
                h(
                    "p",
                    "muted",
                    if near {
                        "月の変わり目です。今月の数字を帳票で確かめましょう。"
                    } else {
                        "明細を取り込むか、取引を登録しましょう。"
                    },
                ),
                box_node(
                    "div",
                    "next-actions-buttons",
                    &[
                        if near {
                            link("帳票を開く", "#/reports", "btn btn--primary")
                        } else {
                            link("取引を登録", "#/entry", "btn btn--primary")
                        },
                        link("明細を取り込む", "#/import", "btn"),
                    ],
                ),
            ],
        );
    }
    replace(body, &[next]);
    if n(&status["ledger"], "entries") == 0 {
        append(
            body,
            &[box_node(
                "div",
                "card",
                &[
                    h("h2", "card-title", "最初の取引を登録しましょう"),
                    h(
                        "p",
                        "",
                        "取引を1件登録すると、ここに今月の売上・経費・利益と、口座の残高が出ます。",
                    ),
                    link("取引を登録", "#/entry", "btn btn--primary"),
                    link("明細を取り込む", "#/import", "btn"),
                ],
            )],
        );
        return;
    }
    let month_label = format!("{}月", month[5..].parse::<u32>().unwrap_or(1));
    let previous = shift_month(month, -1);
    let opened = s(settings, "opened_on");
    let first = opened.get(..7).unwrap_or("");
    let kpis = h("section", "kpis", "");
    attrs(
        &kpis,
        &[(
            "aria-label",
            &format!("{}年{month_label}の数字", &month[..4]),
        )],
    );
    for (key, label) in [("revenue", "売上"), ("expense", "経費"), ("profit", "利益")] {
        let value = n(&dash["month_summary"], key);
        let node = kpi(&format!("{month_label}の{label}"), value, key == "profit");
        let before = n(&dash["month_summary"]["prev"], key);
        if month < current.as_str() || first.is_empty() || previous.as_str() >= first {
            let line = h("p", "kpi-diff", "");
            let content = h("span", "", "");
            if month >= current.as_str() {
                append(
                    &content,
                    &[
                        h(
                            "span",
                            "",
                            &format!("{}月は ", previous[5..].parse::<u32>().unwrap_or(1)),
                        ),
                        amount(before),
                    ],
                );
                let span = if previous == current {
                    "途中"
                } else if previous > current
                    || (first == previous && opened.get(8..10) != Some("01"))
                {
                    ""
                } else {
                    "1か月分"
                };
                if !span.is_empty() {
                    append(&content, &[h("span", "keep", &format!("（{span}）"))]);
                }
            } else {
                let difference = value.saturating_sub(before);
                append(
                    &line,
                    &[icon(match difference.cmp(&0) {
                        std::cmp::Ordering::Greater => "arrow-up",
                        std::cmp::Ordering::Less => "arrow-down",
                        std::cmp::Ordering::Equal => "minus",
                    })],
                );
                append(
                    &content,
                    &[h(
                        "span",
                        "",
                        if difference == 0 {
                            "前月と同じ"
                        } else {
                            "前月比 "
                        },
                    )],
                );
                if difference != 0 {
                    let signed = if difference > 0 {
                        format!("+{}", number(difference))
                    } else {
                        number(difference).replacen('-', "−", 1)
                    };
                    append(
                        &content,
                        &[box_node(
                            "span",
                            "money",
                            &[h("span", "num", &signed), h("span", "unit", "円")],
                        )],
                    );
                }
            }
            // Keep the sentence in one inline flow inside the flex row so it can wrap
            // before a long, indivisible amount, as in the original home card DOM.
            append(&line, &[content]);
            append(&node, &[line]);
        }
        append(&kpis, &[node]);
    }
    let ytd = kpi(
        &format!("{}年の利益", n(dash, "fiscal_year")),
        n(&dash["ytd"], "profit"),
        true,
    );
    let start = if first.len() == 7
        && first[..4] == n(dash, "fiscal_year").to_string()
        && first <= month
        && list(&dash["series"])
            .iter()
            .filter(|v| s(v, "month") < first)
            .all(|v| n(v, "revenue") == 0 && n(v, "expense") == 0)
    {
        first[5..].parse::<u32>().unwrap_or(1)
    } else {
        1
    };
    append(
        &ytd,
        &[box_node(
            "dl",
            "kpi-notes",
            &[
                box_node(
                    "div",
                    "kpi-period",
                    &[
                        h("dt", "", "期間"),
                        h("dd", "", &format!("{start}月〜{month_label}")),
                    ],
                ),
                fact("売上", n(&dash["ytd"], "revenue")),
                fact("経費", n(&dash["ytd"], "expense")),
            ],
        )],
    );
    append(&kpis, &[ytd]);
    append(body, &[kpis]);
    let year = n(dash, "fiscal_year").to_string();
    let until = if year.as_str() < current.get(..4).unwrap_or("") {
        format!("{year}-12")
    } else {
        current
    };
    let graph = card(&format!("{year}年の推移"), "chart-card");
    append(
        &graph,
        &[chart::chart(
            ctx,
            &list(&dash["series"]),
            &until,
            month,
            &format!("{year}年の推移"),
            true,
        )],
    );
    append(body, &[graph]);
    let wallets = card("口座の残高", "wallets-card");
    if let Ok(Some(head)) = wallets.query_selector(".card-head") {
        append(
            &head,
            &[link("口座の一覧", "#/wallets", "small tap card-link")],
        );
    }
    if pending > 0 {
        append(
            &wallets,
            &[h(
                "p",
                "pending-note",
                &format!("未確認 {}件は、この残高に入っていません。", number(pending)),
            )],
        );
    }
    let rows = h("ul", "rows", "");
    for w in list(&dash["wallets"]) {
        let sub = match s(&w, "type") {
            "card" => "カード・未払い額",
            "bank" => "預金",
            "ewallet" => "電子マネー",
            _ => "現金",
        };
        append(
            &rows,
            &[box_node(
                "li",
                "row",
                &[
                    box_node(
                        "span",
                        "row-icon",
                        &[icon(match s(&w, "type") {
                            "bank" => "building",
                            "card" => "card",
                            _ => "wallet",
                        })],
                    ),
                    box_node("span", "row-main", &{
                        let mut nodes = vec![h("span", "keep", s(&w, "name"))];
                        if sub != s(&w, "name") {
                            nodes.push(h("span", "row-sub", sub));
                        }
                        nodes
                    }),
                    box_node("span", "row-value", &[amount(n(&w, "balance"))]),
                ],
            )],
        );
    }
    append(
        &wallets,
        &[
            rows,
            box_node(
                "dl",
                "totals",
                &[
                    fact("現金と預金の合計", n(&dash["wallets"], "cash_total")),
                    fact("カードの未払いの合計", n(&dash["wallets"], "owed_total")),
                ],
            ),
        ],
    );
    let top = card("経費の上位5科目", "");
    if let Ok(Some(head)) = top.query_selector(".card-head") {
        append(
            &head,
            &[h(
                "span",
                "small muted",
                &format!("{}年{month_label}", &month[..4]),
            )],
        );
    }
    let rank = h("ol", "rank", "");
    let top_items = list(&dash["top_expenses"]);
    let top_max = top_items
        .iter()
        .map(|e| n(e, "amount"))
        .max()
        .unwrap_or(1)
        .max(1);
    for expense in &top_items {
        let bar = h("span", "rank-bar", "");
        attrs(&bar, &[("aria-hidden", "true")]);
        let width = (i128::from(n(expense, "amount")) * 100 / i128::from(top_max)).max(2);
        let _ = bar
            .unchecked_ref::<HtmlElement>()
            .style()
            .set_property("--w", &format!("{width}%"));
        append(
            &rank,
            &[box_node(
                "li",
                "rank-item",
                &[
                    link(
                        s(expense, "name"),
                        &build_hash("/journal", &[("account", s(expense, "account"))]),
                        "rank-name tap keep",
                    ),
                    box_node("span", "rank-value", &[amount(n(expense, "amount"))]),
                    bar,
                ],
            )],
        );
    }
    if top_items.is_empty() {
        append(&top, &[h("p", "muted", "この月の経費は、まだありません。")]);
    } else {
        append(&top, &[rank]);
    }
    let ledger = ledger_card(ctx, &dash["ledger"]);
    let stack = box_node("div", "home-stack", &[ledger]);
    if yes(&dash["tax"], "applicable") {
        let tax = card("消費税の見込み", "kpi-host");
        append(
            &tax,
            &[
                h("span", "badge badge--muted", "参考"),
                box_node("p", "kpi-value", &[amount(n(&dash["tax"], "estimate"))]),
                box_node(
                    "dl",
                    "totals",
                    &[
                        fact("売上税額", n(&dash["tax"], "sales_tax")),
                        fact("仕入税額", n(&dash["tax"], "purchase_tax")),
                    ],
                ),
                h(
                    "p",
                    "kpi-note",
                    &format!(
                        "{}年1月1日から{}月{}日までの集計です。参考値で、簡易課税と特例は反映していません。",
                        s(&dash["tax"], "as_of").get(..4).unwrap_or(""),
                        s(&dash["tax"], "as_of")
                            .get(5..7)
                            .and_then(|m| m.parse::<u32>().ok())
                            .unwrap_or(1),
                        s(&dash["tax"], "as_of")
                            .get(8..10)
                            .and_then(|d| d.parse::<u32>().ok())
                            .unwrap_or(1)
                    ),
                ),
            ],
        );
        append(&stack, &[tax]);
    }
    append(
        body,
        &[box_node("div", "home-grid", &[wallets, top, stack])],
    );
}
pub(crate) fn ledger_card(ctx: &Context, ledger: &Value) -> Element {
    let node = card("台帳の状態", "");
    let verified = yes(ledger, "verified");
    append(
        &node,
        &[
            h(
                "span",
                if verified {
                    "badge badge--ok"
                } else {
                    "badge badge--danger"
                },
                if verified { "正常" } else { "要確認" },
            ),
            h(
                "p",
                "ledger-text",
                &if verified {
                    format!("記録は正常です（{}件）。", number(n(ledger, "events")))
                } else {
                    "記録の検査で、壊れた所が見つかりました。設定の「データ」で確かめてください。"
                        .into()
                },
            ),
        ],
    );
    let hash = s(ledger, "head_hash").to_owned();
    let short = if hash.len() > 16 {
        format!("{}…{}", &hash[..8], &hash[hash.len() - 8..])
    } else {
        hash.clone()
    };
    let manual = h("div", "hash-manual", "");
    let copy = button("控えを写し取る", "");
    let details = box_node(
        "details",
        "ledger-more",
        &[
            h("summary", "tap", "くわしく（照合用の控え）"),
            h(
                "p",
                "small muted",
                "台帳の最後の記録の印（ハッシュ）です。紙やメモに控えると、あとで台帳と照合できます。",
            ),
            h("code", "mono", &short),
            copy.clone(),
            manual.clone(),
        ],
    );
    append(&node, &[details]);
    let c = ctx.clone();
    ctx.on(&copy,"click",move |_|{let c2=c.clone();let hash=hash.clone();let manual=manual.clone();c.task(async move{if c2.copy_text(&hash).await.is_ok(){if c2.alive(){c2.success("末尾のハッシュを写し取りました。紙やメモに控えておくと、あとで台帳と照合できます。");}}else if c2.alive(){let field=text_input(&hash,"末尾のハッシュ（全文）");field.set_read_only(true);replace(&manual,&[el(&field),h("p","field-hint","自動で写せませんでした。選んである文字を Ctrl+C（Macは ⌘+C）で写してください。")]);let _=field.focus();field.select();}Ok(())});});
    node
}
