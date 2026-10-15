// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::model::counts_text;
use super::{
    BTreeSet, Element, Rc, RefCell, State, Value, append, array, attrs, build_hash, button,
    disabled, document, focus, group, h, icon, input, link, node, replace, set_text, summary_text,
    table, text, window,
};
use crate::app::prelude::{JsValue, phrase};
fn date_display(iso: &str) -> String {
    if crate::input::date_parts(iso).is_none() {
        return iso.into();
    }
    let date = js_sys::Date::new(&JsValue::from_str(&format!("{iso}T12:00:00Z")));
    let weekday = ["日", "月", "火", "水", "木", "金", "土"]
        .get(usize::try_from(date.get_utc_day()).unwrap_or(0))
        .copied()
        .unwrap_or("");
    format!("{iso} ({weekday})")
}
fn badge(status: &str) -> Element {
    let (label, variant, icon_name) = match status {
        "new" => ("新規", "ok", "plus"),
        "duplicate" => ("重複", "warn", "copy"),
        "invalid" => ("読めない", "danger", "alert"),
        _ => (status, "muted", "info"),
    };
    node(
        "span",
        &format!("badge badge--{variant}"),
        &[icon(icon_name), h("span", "", label)],
    )
}
fn paint_summary(summary: &Element, rows: &[Value], force: &BTreeSet<u64>, truncated: bool) {
    let text = summary_text(rows, force, truncated);
    replace(summary, &[]);
    if text.is_empty() {
        return;
    }
    for (i, part) in text.split('・').enumerate() {
        if i > 0 {
            let _ = summary.append_child(&document().create_text_node("・"));
        }
        append(summary, &[h("span", "nw", part)]);
    }
}
impl State {
    #[allow(
        clippy::too_many_lines,
        reason = "Build the preview summary, row-specific decisions and associated native handlers."
    )]
    pub fn step_check(self: &Rc<Self>) {
        let ctx = self.begin_step();
        let preview = self.preview.borrow().clone();
        let rows = array(&preview["rows"]).to_vec();
        let counts = preview["counts"].clone();
        let new = counts["new"].as_u64().unwrap_or(0);
        let duplicates = counts["duplicate"].as_u64().unwrap_or(0);
        let invalid = counts["invalid"].as_u64().unwrap_or(0);
        let truncated = preview["truncated"] == true;
        let force = Rc::new(RefCell::new(BTreeSet::<u64>::new()));
        let summary = h("p", "check-summary", "");
        attrs(&summary, &[("aria-live", "polite")]);
        let count_list = h("ul", "check-counts", "");
        attrs(&count_list, &[("aria-label", "判定の件数")]);
        for (status, n) in [
            ("new", new),
            ("duplicate", duplicates),
            ("invalid", invalid),
        ] {
            append(
                &count_list,
                &[node(
                    "li",
                    "",
                    &[
                        badge(status),
                        h("span", "num", &format!("{}件", group(i128::from(n)))),
                    ],
                )],
            );
        }
        let head = h("div", "check-head", "");
        if duplicates > 0 && new == 0 && invalid == 0 {
            append(
                &head,
                &[node(
                    "p",
                    "check-all-dup",
                    &[
                        icon("copy"),
                        h(
                            "span",
                            "",
                            &format!(
                                "このファイルの{}件は、すべて取り込み済みです。",
                                group(i128::from(duplicates))
                            ),
                        ),
                    ],
                )],
            );
        }
        append(
            &head,
            &[
                count_list,
                h("p", "visually-hidden", &counts_text(&counts)),
                summary.clone(),
            ],
        );
        let wallet = self.wallet.borrow().clone();
        if new == 0 && duplicates > 0 {
            append(
                &head,
                &[node(
                    "p",
                    "check-next",
                    &[link(
                        "未確認を開く",
                        &build_hash("/inbox", &[("wallet", &wallet)]),
                        "btn btn--sm",
                    )],
                )],
            );
        }
        let commit = button(&format!("{new}件を取り込む"), "primary");
        append(&commit, &[icon("import")]);
        disabled(&commit, new == 0 || self.ctx.is_read_only());
        if self.ctx.is_read_only() {
            attrs(&commit, &[("aria-describedby", "banner-readonly")]);
        }
        let mut trs = vec![];
        for row in &rows {
            let tr = h("tr", &format!("is-{}", text(row, "status")), "");
            attrs(
                &tr,
                &[("data-key", &row["line"].as_u64().unwrap_or(0).to_string())],
            );
            let reason = node("td", "wrap reason-cell", &[]);
            let duplicate = text(row, "status") == "duplicate";
            let message = if duplicate {
                "取り込み済みの明細と同じです。"
            } else {
                text(row, "reason")
            };
            if !message.is_empty() {
                append(&reason, &[h("span", "reason-text", message)]);
            }
            if duplicate && row["forceable"] == true {
                let key = row["line"].as_u64().unwrap_or(0);
                let check = input("checkbox", "");
                attrs(
                    &check,
                    &[(
                        "aria-label",
                        &format!(
                            "ファイルの{key}行目も取り込む（{}）",
                            text(row, "description")
                        ),
                    )],
                );
                let force = force.clone();
                let summary = summary.clone();
                let rows = rows.clone();
                let b = commit.clone();
                let c = check.clone();
                let read_only = self.ctx.is_read_only();
                ctx.on(&check, "change", move |_| {
                    if c.checked() {
                        force.borrow_mut().insert(key);
                    } else {
                        force.borrow_mut().remove(&key);
                    }
                    let total = new + force.borrow().len() as u64;
                    if let Some(span) = b.first_element_child() {
                        set_text(&span, &format!("{total}件を取り込む"));
                    }
                    disabled(&b, total == 0 || read_only);
                    paint_summary(&summary, &rows, &force.borrow(), truncated);
                });
                append(
                    &reason,
                    &[node(
                        "label",
                        "check force-check",
                        &[check.into(), h("span", "", "この行も取り込む")],
                    )],
                );
            }
            let amount = row["amount"].as_i64().unwrap_or(0);
            let amount = if amount == 0 {
                h("span", "", "—")
            } else {
                h(
                    "span",
                    &format!("amount amount--{}", text(row, "direction")),
                    &if text(row, "direction") == "in" {
                        format!("+{}", group(i128::from(amount)))
                    } else {
                        group(-i128::from(amount))
                    },
                )
            };
            let cells = [
                h("td", "num", &row["line"].as_u64().unwrap_or(0).to_string()),
                node("td", "", &[badge(text(row, "status"))]),
                h(
                    "td",
                    "",
                    &if text(row, "date").is_empty() {
                        "—".into()
                    } else {
                        date_display(text(row, "date"))
                    },
                ),
                h(
                    "td",
                    "wrap",
                    if text(row, "description").is_empty() {
                        "—"
                    } else {
                        text(row, "description")
                    },
                ),
                node("td", "num", &[amount]),
                reason,
            ];
            for (cell, label) in
                cells
                    .iter()
                    .zip(["ファイルの行", "判定", "日付", "摘要", "金額", "理由・選択"])
            {
                attrs(cell, &[("data-label", label)]);
            }
            append(&tr, &cells);
            trs.push(tr);
        }
        let notes = h("ul", "check-notes", "");
        for (condition, text) in [
            (
                duplicates > 0,
                "「重複」は、同じ口座・日付・金額・摘要（取引番号の列を選んだときは、その番号）の明細が、すでに取り込んであるものです。期間が重なる明細を取り込んだときに出ます。本当に別の取引なら、行ごとに「この行も取り込む」を選んでください。",
            ),
            (
                invalid > 0,
                "「読めない」行は取り込みません。理由を見て、列の対応を直すと読めることがあります（「戻る」で直せます）。",
            ),
            (
                truncated,
                "表に出しているのは先頭の500行です。件数はファイル全体の数です。",
            ),
        ] {
            if condition {
                append(
                    &notes,
                    &[node(
                        "li",
                        "",
                        &[icon("info"), node("span", "", &[phrase(text)])],
                    )],
                );
            }
        }
        let save = input("checkbox", "");
        save.set_checked(true);
        let wallet_name = self
            .wallets
            .borrow()
            .iter()
            .find(|w| text(w, "code") == wallet)
            .map(|w| text(w, "name").to_owned())
            .unwrap_or_default();
        let save_label = node(
            "label",
            "check",
            &[
                save.clone().into(),
                h(
                    "span",
                    "",
                    &format!(
                        "この列の対応を「{wallet_name}」に覚えさせる（次からの初期値になります）"
                    ),
                ),
            ],
        );
        let back = button("戻る", "ghost");
        let _ = back.insert_before(&icon("chevron-left"), back.first_child().as_ref());
        let weak = Rc::downgrade(self);
        ctx.on(&back, "click", move |_| {
            if let Some(s) = weak.upgrade()
                && !s.busy.get()
            {
                let mapping = s.mapping.borrow().clone();
                s.step_columns(Some(&mapping));
            }
        });
        let weak = Rc::downgrade(self);
        let f = force.clone();
        let b = commit.clone();
        ctx.on(&commit, "click", move |_| {
            if let Some(s) = weak.upgrade() {
                s.commit(&f.borrow(), save.checked(), b.clone());
            }
        });
        let actions = node("div", "import-actions", &[back, commit]);
        attrs(&actions, &[("data-bottom-bar", "true")]);
        paint_summary(&summary, &rows, &force.borrow(), truncated);
        let data = if trs.is_empty() {
            node(
                "div",
                "card",
                &[node(
                    "div",
                    "empty",
                    &[
                        h("p", "empty-title", "読める行がありません"),
                        h(
                            "p",
                            "empty-body",
                            "列の対応か、読み飛ばす行の数が合っていないかもしれません。「戻る」で直してください。",
                        ),
                    ],
                )],
            )
        } else {
            table(
                &ctx,
                "取り込む前の確認（行ごとの判定）",
                "check-table",
                &["ファイルの行", "判定", "日付", "摘要", "金額", "理由・選択"],
                &trs,
            )
        };
        self.show(
            2,
            node(
                "div",
                "import-step stack",
                &[head, notes, data, save_label, actions],
            ),
            None,
        );
    }
    pub fn result(self: &Rc<Self>, res: &Value) {
        let ctx = self.begin_step();
        self.paint_stepper(3);
        replace(&self.top_error, &[]);
        let created = res["created"].as_u64().unwrap_or(0);
        let mut skipped = vec![];
        let duplicates = res["duplicates"].as_u64().unwrap_or(0);
        let invalid = res["invalid"].as_u64().unwrap_or(0);
        if duplicates > 0 {
            skipped.push(format!("重複 {duplicates}件"));
        }
        if invalid > 0 {
            skipped.push(format!("読めない {invalid}件"));
        }
        let main = link(
            "未確認を開く",
            &build_hash("/inbox", &[("wallet", &self.wallet.borrow())]),
            "btn btn--primary",
        );
        let kbd = h("kbd", "", "g i");
        attrs(&kbd, &[("aria-hidden", "true")]);
        append(&main, &[kbd]);
        let again = button("別のファイルを取り込む", "secondary");
        let weak = Rc::downgrade(self);
        ctx.on(&again, "click", move |_| {
            if let Some(s) = weak.upgrade() {
                s.step_file(None);
            }
        });
        let content = node(
            "div",
            "card import-done",
            &[node(
                "div",
                "empty",
                &[
                    node("span", "empty-icon", &[icon("check-circle")]),
                    h(
                        "p",
                        "empty-title",
                        &format!("{created}件を未確認に入れました"),
                    ),
                    node(
                        "p",
                        "empty-body",
                        &[phrase(&format!(
                            "{}未確認の画面で科目を決めて、確定してください。確定するまでは、仕訳帳と帳票には出ません。",
                            if skipped.is_empty() {
                                String::new()
                            } else {
                                format!("{}は取り込んでいません。", skipped.join("と"))
                            }
                        ))],
                    ),
                    node("div", "empty-actions", &[main.clone(), again]),
                ],
            )],
        );
        replace(&self.body, &[content]);
        window().scroll_to_with_x_and_y(0., 0.);
        focus(&main);
    }
}
