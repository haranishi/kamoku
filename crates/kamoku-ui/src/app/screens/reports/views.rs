// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
use super::super::home::{
    chart::chart,
    common::{banner_node, box_node, cell, date_text, fact, list, n, row_open, s, table, yes},
};
use super::super::masters::model::KINDS;
use super::{
    period::Period,
    table::{Figure, Row, check, empty, figures, loss, model_row, note, report_table, yen},
    tree::{bs_rows, chart_key, collapse_rows, hidden_months_note, month_columns, tree_rows},
};
use crate::app::prelude::*;
fn error(e: String) -> ApiError {
    ApiError::message(e)
}
fn sum(mut values: impl Iterator<Item = i64>) -> Result<i64, ApiError> {
    values.try_fold(0i64, |a, b| {
        a.checked_add(b)
            .ok_or_else(|| error("表示する合計が大きすぎます。".into()))
    })
}
fn rows_of(section: &Value, accounts: &[Value], keys: &[&str]) -> Result<Vec<Value>, ApiError> {
    tree_rows(&list(&section["rows"]), accounts, keys)
        .map(|v| collapse_rows(&v))
        .map_err(error)
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(crate) fn monthly(
    ctx: &Context,
    res: &Value,
    period: &Period,
    accounts: &[Value],
) -> Result<Element, ApiError> {
    let series: Vec<_> = list(&res["months"])
        .into_iter()
        .map(|mut m| {
            m["profit"] = m["net_income"].clone();
            m
        })
        .collect();
    let now = today();
    let year = now[..4].parse::<i32>().unwrap_or(0);
    let until = match period.year.cmp(&year) {
        std::cmp::Ordering::Less => format!("{}-12", period.year),
        std::cmp::Ordering::Equal => year_month(&now),
        std::cmp::Ordering::Greater => format!("{}-00", period.year),
    };
    let mut all = list(&res["revenue"]["rows"]);
    all.extend(list(&res["expense"]["rows"]));
    let cols = month_columns(&series, &all, &until);
    let graph = chart(
        ctx,
        &series,
        &until,
        "",
        &format!("{}年の月次推移", period.year),
        false,
    );
    let amounts: Vec<bool> = (0..series.len())
        .map(|i| {
            all.iter()
                .any(|r| r["months"].get(i).and_then(Value::as_i64).unwrap_or(0) != 0)
        })
        .collect();
    let mut summary = vec![];
    for i in &cols {
        let row = &series[*i];
        let empty_month = n(row, "revenue") == 0 && n(row, "expense") == 0;
        let mut item = Row::new(
            &format!(
                "{}月",
                s(row, "month")
                    .get(5..)
                    .and_then(|m| m.parse::<u32>().ok())
                    .unwrap_or(0)
            ),
            ["revenue", "expense", "profit"]
                .iter()
                .map(|k| if empty_month { empty() } else { yen(n(row, k)) })
                .collect(),
        );
        if !empty_month {
            item.note = loss(n(row, "profit"));
        }
        summary.push(item);
    }
    let totals = [
        sum(cols.iter().map(|i| n(&series[*i], "revenue")))?,
        sum(cols.iter().map(|i| n(&series[*i], "expense")))?,
        sum(cols.iter().map(|i| n(&series[*i], "profit")))?,
    ];
    let mut total = Row::total("合計", totals.into_iter().map(yen).collect(), true);
    total.note = loss(totals[2]);
    summary.push(total);
    let future = cols
        .iter()
        .any(|i| s(&series[*i], "month") > year_month(&now).as_str());
    let caption = format!(
        "{}年の月次推移（{}）",
        period.year,
        if future {
            "今月より後の月は、表だけに出ます"
        } else {
            "グラフと同じ数字"
        }
    );
    let summary = report_table(
        ctx,
        &caption,
        &["月", "売上", "経費", "利益"],
        summary,
        "rt-months",
        "cards",
        Some(2),
    );
    let month_cells = |values: &[Value]| -> Vec<Element> {
        cols.iter()
            .map(|i| {
                if amounts[*i] {
                    yen(values.get(*i).and_then(Value::as_i64).unwrap_or(0))
                } else {
                    empty()
                }
            })
            .collect()
    };
    let mut body = vec![];
    for (title, key) in [("収益", "revenue"), ("費用", "expense")] {
        let section = &res[key];
        body.push(Row::group(title));
        for row in rows_of(section, accounts, &["amount", "months"])? {
            let mut item = model_row(&row, &[]);
            item.cells = month_cells(&list(&row["months"]));
            item.cells.push(yen(n(&row, "amount")));
            body.push(item);
        }
        let mut month_totals = vec![];
        let source = list(&section["rows"]);
        for i in 0..series.len() {
            month_totals.push(json!(sum(source
                .iter()
                .map(|row| row["months"].get(i).and_then(Value::as_i64).unwrap_or(0)))?));
        }
        let mut cells = month_cells(&month_totals);
        cells.push(yen(n(section, "total")));
        body.push(Row::total(&format!("{title}の合計"), cells, false));
    }
    let mut cells = month_cells(
        &series
            .iter()
            .map(|m| m["profit"].clone())
            .collect::<Vec<_>>(),
    );
    cells.push(yen(n(res, "net_income")));
    let mut grand = Row::total("当期純利益", cells, true);
    grand.note = loss(n(res, "net_income"));
    body.push(grand);
    let mut headers = vec!["科目".into()];
    headers.extend(cols.iter().map(|i| {
        format!(
            "{}月",
            s(&series[*i], "month")
                .get(5..)
                .and_then(|m| m.parse::<u32>().ok())
                .unwrap_or(0)
        )
    }));
    headers.push("合計".into());
    let heads: Vec<_> = headers.iter().map(String::as_str).collect();
    let by = report_table(
        ctx,
        &format!("{}年の科目ごとの月別", period.year),
        &heads,
        body,
        "rt-wide",
        "months",
        Some(cols.len()),
    );
    let title = h("h3", "", "月ごとの数字");
    attrs(&title, &[("id", "rt-sum-title")]);
    let section = box_node("section", "report-section", &[title]);
    attrs(&section, &[("aria-labelledby", "rt-sum-title")]);
    let hidden = hidden_months_note(&series, &cols, &year_month(&now));
    if !hidden.is_empty() {
        append(&section, &[note(&hidden)]);
    }
    append(&section, &[summary]);
    let graph = box_node("section", "card chart-card", &[graph]);
    attrs(&graph, &[("aria-label", "グラフ")]);
    Ok(box_node(
        "div",
        "report-body",
        &[
            graph,
            section,
            box_node(
                "details",
                "report-section report-more",
                &[h("summary", "", "科目ごとの月別を見る"), by],
            ),
        ],
    ))
}
pub(crate) fn income(ctx: &Context, res: &Value, accounts: &[Value]) -> Result<Element, ApiError> {
    let mut tables = vec![];
    for (label, key) in [("収益", "revenue"), ("費用", "expense")] {
        let section = &res[key];
        let rows = rows_of(section, accounts, &["amount"])?;
        let mut body: Vec<_> = rows.iter().map(|r| model_row(r, &["amount"])).collect();
        if body.is_empty() {
            body.push(Row::new("この期間は、ありません", vec![h("span", "", "")]));
        }
        body.push(Row::total(
            &format!("{label}の合計"),
            vec![yen(n(section, "total"))],
            true,
        ));
        tables.push(report_table(
            ctx,
            label,
            &[label, "金額"],
            body,
            "",
            "cards",
            Some(0),
        ));
    }
    Ok(box_node(
        "div",
        "report-body",
        &[
            figures(
                &[
                    Figure {
                        label: "当期純利益",
                        hint: "収益−費用",
                        value: n(res, "net_income"),
                        main: true,
                        loss: true,
                    },
                    Figure {
                        label: "収益の合計",
                        hint: "",
                        value: n(&res["revenue"], "total"),
                        main: false,
                        loss: false,
                    },
                    Figure {
                        label: "費用の合計",
                        hint: "",
                        value: n(&res["expense"], "total"),
                        main: false,
                        loss: false,
                    },
                ],
                "",
            ),
            box_node("div", "rt-pair", &tables),
            note(
                "事業主貸と事業主借は、売上にも経費にも入りません。取消済みの仕訳は集計に入りません。",
            ),
        ],
    ))
}
pub(crate) fn balance(ctx: &Context, res: &Value) -> Result<Element, ApiError> {
    let section_rows = |key: &str| {
        let rows = bs_rows(&list(&res[key]["rows"]));
        if rows.is_empty() {
            vec![Row::new(
                "残高のある科目はありません",
                vec![h("span", "", "")],
            )]
        } else {
            rows.iter().map(|r| model_row(r, &["amount"])).collect()
        }
    };
    let mut left = section_rows("assets");
    left.push(Row {
        kind: "fill",
        ..Row::default()
    });
    left.push(Row::total(
        "資産の合計",
        vec![yen(n(&res["assets"], "total"))],
        true,
    ));
    let mut right = vec![Row::group("負債")];
    right.extend(section_rows("liabilities"));
    right.push(Row::total(
        "負債の合計",
        vec![yen(n(&res["liabilities"], "total"))],
        false,
    ));
    right.push(Row::group("資本"));
    let mut equity = section_rows("equity");
    if let Some(last) = equity.last_mut() {
        last.note = loss(n(res, "net_income_current"));
    }
    right.extend(equity);
    right.push(Row::total(
        "資本の合計",
        vec![yen(n(&res["equity"], "total"))],
        false,
    ));
    right.push(Row {
        kind: "fill",
        ..Row::default()
    });
    let combined = sum([n(&res["liabilities"], "total"), n(&res["equity"], "total")].into_iter())?;
    right.push(Row::total("負債と資本の合計", vec![yen(combined)], true));
    let difference = i128::from(n(&res["assets"], "total")) - i128::from(combined);
    let diff =
        i64::try_from(difference.abs()).map_err(|_| error("貸借の差額が大きすぎます。".into()))?;
    Ok(box_node(
        "div",
        "report-body",
        &[
            check(
                yes(res, "balanced"),
                "資産の合計と、負債と資本の合計が一致しています",
                &format!("一致していません（差 {}円）", number(diff)),
            ),
            box_node(
                "div",
                "bs-grid",
                &[
                    report_table(
                        ctx,
                        "資産",
                        &["資産", "金額"],
                        left,
                        "bs-table",
                        "cards",
                        Some(0),
                    ),
                    report_table(
                        ctx,
                        "負債と資本",
                        &["負債と資本", "金額"],
                        right,
                        "bs-table",
                        "cards",
                        Some(0),
                    ),
                ],
            ),
            note(
                "事業主貸と事業主借は、その年の動きだけです。前の年までの利益と事業主貸・事業主借は、元入金に入れて見せています（締めの仕訳は作りません）。",
            ),
        ],
    ))
}
pub(crate) fn trial(ctx: &Context, res: &Value, accounts: &[Value]) -> Result<Element, ApiError> {
    let mut body = vec![];
    let rows = list(&res["rows"]);
    for &(kind, label) in KINDS {
        let group: Vec<_> = rows
            .iter()
            .filter(|r| s(r, "kind") == kind)
            .cloned()
            .collect();
        if group.is_empty() {
            continue;
        }
        body.push(Row::group(label));
        for r in collapse_rows(
            &tree_rows(&group, accounts, &["opening", "debit", "credit", "closing"])
                .map_err(error)?,
        ) {
            body.push(model_row(&r, &["opening", "debit", "credit", "closing"]));
        }
    }
    body.push(Row::total(
        "動きの合計",
        vec![
            h("span", "", ""),
            yen(n(res, "total_debit")),
            yen(n(res, "total_credit")),
            h("span", "", ""),
        ],
        true,
    ));
    Ok(box_node(
        "div",
        "report-body",
        &[
            check(
                yes(res, "balanced"),
                "借方と貸方の合計、期末の残高の貸借が一致しています",
                "貸借が一致していません。設定の「データ」で台帳を検査してください",
            ),
            if rows.is_empty() {
                box_node(
                    "div",
                    "card",
                    &[
                        h("h3", "", "この期間には、残高も動きもありません"),
                        h("p", "", "期間を変えるか、取引を登録してください。"),
                    ],
                )
            } else {
                report_table(
                    ctx,
                    "試算表",
                    &["科目", "期首", "借方", "貸方", "期末"],
                    body,
                    "",
                    "cards",
                    Some(3),
                )
            },
            note(
                "期首と期末は、科目のふだんの側から見た残高です。\n資産と費用は借方−貸方、負債・資本・収益は貸方−借方で数えます。\n前の年までの利益は、元入金の期首に入れています。",
            ),
        ],
    ))
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(crate) fn ledger(
    ctx: &Context,
    res: &Value,
    period: &Period,
    account: &str,
    accounts: &[Value],
    back: &str,
) -> Element {
    if account.is_empty() {
        return box_node(
            "div",
            "card",
            &[
                h("h3", "", "科目を選んでください"),
                h(
                    "p",
                    "",
                    "上の「科目」の欄で選ぶと、その科目の動きと残高の推移が出ます。口座を選ぶと、口座の出入りが見られます。",
                ),
            ],
        );
    }
    let node = h("div", "report-body", "");
    let rows = list(&res["rows"]);
    let mut kids: Vec<_> = accounts
        .iter()
        .filter(|a| s(a, "parent") == account)
        .cloned()
        .collect();
    kids.sort_by_key(chart_key);
    if !kids.is_empty() {
        let word = if kids.iter().all(|a| a["wallet"].is_object()) {
            "口座"
        } else {
            "科目"
        };
        let mut names = String::new();
        for child in &kids {
            names.push('「');
            names.push_str(s(child, "name"));
            names.push('」');
        }
        let text = if rows.is_empty() {
            format!("この科目の動きは、{word}{names}の側にあります。")
        } else {
            format!(
                "この表は、この科目への直接の記帳だけです。{word}{names}の動きは、{}{word}の元帳にあります。",
                if kids.len() > 1 {
                    "それぞれの"
                } else {
                    "その"
                }
            )
        };
        let links = h("ul", "ledger-children-links", "");
        attrs(&links, &[("aria-label", &format!("{word}の元帳"))]);
        for kid in &kids {
            let mut query = period.query();
            query.push(("account".into(), s(kid, "code").into()));
            let pairs: Vec<_> = query
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            append(
                &links,
                &[box_node(
                    "li",
                    "",
                    &[link(
                        &format!("{}の元帳", s(kid, "name")),
                        &build_hash("/reports/ledger", &pairs),
                        "btn btn--sm ledger-child-link",
                    )],
                )],
            );
        }
        let children = box_node(
            "div",
            "ledger-children",
            &[h("p", "ledger-children-text", &text), links],
        );
        attrs(&children, &[("role", "note")]);
        append(&node, &[children]);
    }
    let count = box_node(
        "div",
        "",
        &[
            h("dt", "", "期間の行"),
            box_node(
                "dd",
                "",
                &[box_node(
                    "span",
                    "money",
                    &[
                        h("span", "num", &rows.len().to_string()),
                        h("span", "unit", "行"),
                    ],
                )],
            ),
        ],
    );
    let closing = fact("期末の残高", n(res, "closing"));
    let _ = closing.class_list().add_1("is-main");
    append(
        &node,
        &[box_node(
            "dl",
            "ledger-facts",
            &[fact("期首の残高", n(res, "opening")), count, closing],
        )],
    );
    if rows.is_empty() {
        append(
            &node,
            &[box_node(
                "div",
                "card",
                &[
                    h(
                        "h3",
                        "",
                        if kids.is_empty() {
                            "この期間には、この科目の動きがありません"
                        } else {
                            "この期間には、この科目への直接の記帳がありません"
                        },
                    ),
                    h(
                        "p",
                        "",
                        if kids.is_empty() {
                            "期間を変えるか、別の科目を選んでください。"
                        } else {
                            "動きは、上のリンクの元帳で見られます。"
                        },
                    ),
                ],
            )],
        );
        return node;
    }
    let (wrap, tbody) = table(
        "総勘定元帳（行で Enter を押すと、その仕訳を開く）",
        &["日付", "No", "摘要", "相手科目", "借方", "貸方", "残高"],
        "table--stack ledger-table",
    );
    let heads = wrap.query_selector_all("thead th").unwrap();
    for (i, class) in [
        (0, "lg-date"),
        (1, "num col-wide lg-no"),
        (3, "col-wide"),
        (4, "num"),
        (5, "num"),
        (6, "num lg-balance"),
    ] {
        if let Some(th) = heads.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
            attrs(&th, &[("class", class)]);
        }
    }
    for (i, row) in rows.iter().enumerate() {
        let tr = h("tr", "", "");
        attrs(
            &tr,
            &[
                ("data-index", &i.to_string()),
                ("data-entry-id", s(row, "entry_id")),
            ],
        );
        let address = super::super::journal::return_path::detail_hash(s(row, "entry_id"), back);
        let link1 = link(&format!("No.{}", n(row, "no")), &address, "");
        attrs(&link1, &[("tabindex", "-1")]);
        let link2 = link(&format!("No.{}", n(row, "no")), &address, "");
        attrs(&link2, &[("tabindex", "-1")]);
        let date = h("span", "lg-ymd", s(row, "date"));
        let date_cell = box_node(
            "span",
            "",
            &[
                date,
                h(
                    "span",
                    "lg-dow",
                    &date_text(s(row, "date"))
                        .split_once(' ')
                        .map_or(String::new(), |(_, d)| format!(" {d}")),
                ),
            ],
        );
        cell(&tr, "日付", date_cell, "lg-date");
        cell(&tr, "No", link1, "num col-wide lg-no");
        let sub = h("span", "lg-sub", "");
        append(
            &sub,
            &[
                h(
                    "span",
                    "lg-sub-date",
                    &format!("{} · ", date_text(s(row, "date"))),
                ),
                link2,
            ],
        );
        // Keep the inline link within real sentence text, as in the original table.
        // This also preserves its inline-link target exemption on touch layouts.
        let _ = sub.append_child(&document().create_text_node(" · "));
        append(&sub, &[h("span", "keep", s(row, "counter"))]);
        cell(
            &tr,
            "",
            box_node(
                "span",
                "",
                &[
                    h(
                        "span",
                        "lg-desc-text keep",
                        if s(row, "description").is_empty() {
                            "（摘要なし）"
                        } else {
                            s(row, "description")
                        },
                    ),
                    sub,
                ],
            ),
            "wrap lg-desc",
        );
        cell(
            &tr,
            "相手科目",
            h("span", "keep", s(row, "counter")),
            "col-wide keep",
        );
        for key in ["debit", "credit", "balance"] {
            cell(
                &tr,
                match key {
                    "debit" => "借方",
                    "credit" => "貸方",
                    _ => "残高",
                },
                h(
                    "span",
                    "",
                    &if key != "balance" && n(row, key) == 0 {
                        String::new()
                    } else {
                        number(n(row, key))
                    },
                ),
                if key == "balance" {
                    "num lg-balance"
                } else {
                    "num"
                },
            );
        }
        let c = ctx.clone();
        row_open(ctx, &tr, move || c.navigate(&address, false));
        attrs(&tr, &[("tabindex", if i == 0 { "0" } else { "-1" })]);
        append(&tbody, &[tr]);
    }
    append(&node, &[wrap]);
    node
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(crate) fn tax(ctx: &Context, res: &Value, settings: &Value) -> Element {
    let node = h("div", "report-body", "");
    if s(settings, "tax_mode") == "exempt" {
        append(
            &node,
            &[banner_node(
                "設定上は免税事業者として表示しています。申告の要否は、国税庁の案内か税理士に確かめてください",
                "課税事業者になったときは、設定の「事業者」で課税の区分を変えてください。",
                false,
            )],
        );
    }
    let terms = h("dl", "tax-terms", "");
    attrs(&terms, &[("aria-label", "言葉の説明")]);
    for (term, description) in [
        (
            "経過措置",
            "登録番号の無い仕入先（免税事業者など）からの仕入れでも、決まった割合だけ税額を差し引ける、期間を区切った特例です。",
        ),
        (
            "一の仕入先ごとの上限",
            "同じ仕入先からの仕入れが、年に決まった額を超えると、超えた分にはこの特例を使えない決まりです。",
        ),
    ] {
        if s(res, "disclaimer").contains(term) {
            append(
                &terms,
                &[box_node(
                    "div",
                    "",
                    &[
                        box_node("dt", "", &[h("dfn", "", term), h("span", "", "：")]),
                        h("dd", "", description),
                    ],
                )],
            );
        }
    }
    let more = box_node(
        "details",
        "tax-more",
        &[
            h("summary", "tap", "くわしく"),
            box_node(
                "div",
                "tax-more-body",
                &[h("p", "", s(res, "disclaimer")), terms],
            ),
        ],
    );
    let notice = box_node(
        "div",
        "banner banner--info",
        &[
            h(
                "p",
                "banner-title",
                "参考値。簡易課税・特例は反映していません",
            ),
            more,
        ],
    );
    attrs(&notice, &[("role", "note")]);
    append(&node, &[notice]);
    let source = list(&res["rows"]);
    let rows = if source.is_empty() {
        vec![Row::new(
            "この期間には、税区分のある取引がありません",
            vec![h("span", "", ""), h("span", "", "")],
        )]
    } else {
        source
            .iter()
            .map(|r| Row::new(s(r, "label"), vec![yen(n(r, "gross")), yen(n(r, "tax"))]))
            .collect()
    };
    let detail = box_node(
        "section",
        "tax-detail",
        &[report_table(
            ctx,
            "税区分ごとの税込の金額と税額",
            &["税区分", "税込の金額", "含まれる税額"],
            rows,
            "",
            "cards",
            Some(1),
        )],
    );
    attrs(&detail, &[("aria-label", "税区分ごとの内訳")]);
    let transitional = list(&res["transitional"]);
    if !transitional.is_empty() {
        let title = h("h3", "", "経過措置の内訳");
        attrs(&title, &[("id", "tax-trans-title")]);
        let table = report_table(
            ctx,
            "経過措置の内訳（免税事業者等からの課税仕入れ）",
            &[
                "控除の割合",
                "支払額（税込）",
                "含まれる税額",
                "控除できる額",
            ],
            transitional
                .iter()
                .map(|r| {
                    Row::new(
                        &format!("{}%", n(r, "percent")),
                        vec![
                            yen(n(r, "gross")),
                            yen(n(r, "tax")),
                            yen(n(r, "deductible")),
                        ],
                    )
                })
                .collect(),
            "",
            "cards",
            Some(2),
        );
        let section = box_node("section", "report-section", &[title, table]);
        attrs(&section, &[("aria-labelledby", "tax-trans-title")]);
        append(&detail, &[section]);
    }
    let result = box_node(
        "section",
        "tax-result",
        &[figures(
            &[
                Figure {
                    label: "納付額の見込み",
                    hint: "参考・本則課税",
                    value: n(res, "estimate"),
                    main: true,
                    loss: false,
                },
                Figure {
                    label: "売上税額",
                    hint: "",
                    value: n(res, "sales_tax"),
                    main: false,
                    loss: false,
                },
                Figure {
                    label: "仕入税額",
                    hint: "経過措置の控除を含む",
                    value: n(res, "purchase_tax"),
                    main: false,
                    loss: false,
                },
            ],
            "tax-sums",
        )],
    );
    attrs(&result, &[("aria-label", "結果")]);
    append(
        &node,
        &[
            box_node("div", "tax-layout", &[result, detail]),
            note(
                "売上税額は、明細ごとの税額の合計です。仕入税額は、通常の仕入れは明細ごとの税額の合計、経過措置の仕入れは、税額に控除できる割合を掛けた額の合計です。どちらも、税込の合計から計算し直すと、1円未満の処理の分だけ違うことがあります。",
            ),
        ],
    );
    node
}
