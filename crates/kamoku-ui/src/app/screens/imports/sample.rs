// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    Element, Value, append, array, attrs, class, column_examples, column_options, h,
    is_number_column, node, roles_of, sample_lines, sample_skipped,
};
use crate::app::prelude::{document, phrase};
#[allow(
    clippy::too_many_lines,
    reason = "Render equivalent wide-table and narrow-list representations of the same sample."
)]
pub(super) fn view(up: &Value, mapping: &Value) -> Vec<Element> {
    let opts = column_options(up);
    let lines = sample_lines(up);
    let head = h("tr", "", "");
    let first = h("th", "num line-col", "ファイルの行");
    attrs(&first, &[("scope", "col")]);
    append(&head, &[first]);
    for col in &opts {
        let roles = roles_of(mapping, col.value);
        let th = h("th", "", "");
        attrs(&th, &[("scope", "col")]);
        class(&th, "is-mapped", !roles.is_empty());
        class(&th, "num", is_number_column(up, col.value));
        append(
            &th,
            &[h(
                "span",
                "col-name",
                if col.name.is_empty() {
                    &col.label
                } else {
                    &col.name
                },
            )],
        );
        for role in roles {
            append(&th, &[h("span", "badge badge--ok col-tag", role)]);
        }
        append(&head, &[th]);
    }
    let body = h("tbody", "", "");
    for (i, row) in array(&up["sample"]).iter().take(20).enumerate() {
        let skipped = sample_skipped(up, mapping["skip_rows"].as_u64(), i);
        let tr = h("tr", if skipped { "is-skipped" } else { "" }, "");
        let line = h("td", "num muted line-col", &lines[i].to_string());
        if skipped {
            append(
                &line,
                &[h("span", "badge badge--muted skip-tag", "読み飛ばす")],
            );
        }
        append(&tr, &[line]);
        for col in &opts {
            let td = h("td", "", row[col.value].as_str().unwrap_or(""));
            class(&td, "is-mapped", !roles_of(mapping, col.value).is_empty());
            class(&td, "num", is_number_column(up, col.value));
            append(&tr, &[td]);
        }
        append(&body, &[tr]);
    }
    let table = node(
        "div",
        "table-wrap sample-wrap",
        &[node(
            "table",
            "table sample-table",
            &[
                h(
                    "caption",
                    "",
                    "ファイルの先頭の行（見本）。選んだ列には、役目の札が付きます",
                ),
                node("thead", "", &[head]),
                body,
            ],
        )],
    );
    attrs(
        &table,
        &[
            ("role", "region"),
            ("aria-label", "ファイルの先頭の行（見本）"),
            ("tabindex", "0"),
        ],
    );
    let list = h("ol", "col-list", "");
    attrs(&list, &[("aria-label", "ファイルの列（見本）")]);
    for col in opts {
        let roles = roles_of(mapping, col.value);
        let item = h("li", "col-item", "");
        class(&item, "is-mapped", !roles.is_empty());
        append(
            &item,
            &[node(
                "p",
                "col-item-head",
                &[
                    h("span", "col-no", &format!("{}列目", col.value + 1)),
                    h("span", "col-item-name keep", &col.name),
                ],
            )],
        );
        let role = h("p", "col-item-role", "");
        if roles.is_empty() {
            append(&role, &[h("span", "muted", "使わない")]);
        } else {
            let arrow = h("span", "col-arrow", "→");
            attrs(&arrow, &[("aria-hidden", "true")]);
            append(&role, &[arrow, h("span", "visually-hidden", "役目: ")]);
            for r in roles {
                append(&role, &[h("span", "badge badge--ok col-tag", r)]);
            }
        }
        append(&item, &[role]);
        let examples = column_examples(up, col.value, 3);
        if !examples.is_empty() {
            let p = h("p", "col-item-example", "例: ");
            for (index, value) in examples.into_iter().enumerate() {
                if index > 0 {
                    let _ = p.append_child(&document().create_text_node("　"));
                }
                append(&p, &[h("span", "keep", &value)]);
            }
            append(&item, &[p]);
        }
        append(&list, &[item]);
    }
    vec![
        node(
            "p",
            "small muted sample-note sample-note--table",
            &[phrase(&format!(
                "ファイルの先頭の行を、中身のまま見せています（{}行）。左の番号は、ファイルの何行目かです。",
                array(&up["sample"]).len().min(20)
            ))],
        ),
        node(
            "p",
            "small muted sample-note sample-note--list",
            &[phrase(
                "列ごとに、空でない値を3つまで、中身のまま見せています。",
            )],
        ),
        table,
        list,
    ]
}
