// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
use super::super::home::common::{amount, box_node, cell, empty_value, n, s, yes};
use crate::app::prelude::*;
use wasm_bindgen::closure::Closure;
#[derive(Default)]
pub(crate) struct Row {
    pub name: String,
    pub sub: String,
    pub kind: &'static str,
    pub depth: i64,
    pub parent: bool,
    pub direct: bool,
    pub cells: Vec<Element>,
    pub note: Option<Element>,
}
impl Row {
    pub fn new(name: &str, cells: Vec<Element>) -> Self {
        Self {
            name: name.into(),
            cells,
            ..Self::default()
        }
    }
    pub fn group(name: &str) -> Self {
        Self {
            name: name.into(),
            kind: "group",
            ..Self::default()
        }
    }
    pub fn total(name: &str, cells: Vec<Element>, grand: bool) -> Self {
        Self {
            name: name.into(),
            cells,
            kind: if grand { "grand" } else { "total" },
            ..Self::default()
        }
    }
}
pub(crate) fn yen(value: i64) -> Element {
    h(
        "span",
        if value == 0 { "rt-zero" } else { "" },
        &number(value),
    )
}
pub(crate) fn empty() -> Element {
    empty_value()
}
pub(crate) fn loss(value: i64) -> Option<Element> {
    (value < 0).then(|| {
        box_node(
            "span",
            "badge badge--muted rt-loss",
            &[icon("alert"), h("span", "", "赤字")],
        )
    })
}
pub(crate) fn note(text: &str) -> Element {
    let lines = h("span", "", "");
    for line in text.split('\n') {
        append(&lines, &[h("span", "report-note-line", line)]);
    }
    box_node("p", "small muted report-note", &[icon("info"), lines])
}
pub(crate) fn check(ok: bool, yes: &str, no: &str) -> Element {
    box_node(
        "p",
        "report-check",
        &[box_node(
            "span",
            if ok {
                "badge badge--ok"
            } else {
                "badge badge--danger"
            },
            &[
                icon(if ok { "check-circle" } else { "alert" }),
                h("span", "", if ok { yes } else { no }),
            ],
        )],
    )
}
pub(crate) struct Figure<'a> {
    pub label: &'a str,
    pub hint: &'a str,
    pub value: i64,
    pub main: bool,
    pub loss: bool,
}
pub(crate) fn figures(items: &[Figure<'_>], class: &str) -> Element {
    let long = items.iter().any(|f| !f.main && number(f.value).len() >= 11);
    let node = h(
        "dl",
        &format!(
            "rt-figures {} {class}",
            if long { "rt-figures--long" } else { "" }
        ),
        "",
    );
    for item in items {
        let label = h("dt", "", item.label);
        if !item.hint.is_empty() {
            append(
                &label,
                &[h("span", "rt-figure-hint", &format!("（{}）", item.hint))],
            );
        }
        let value = box_node("dd", "", &[amount(item.value)]);
        if item.loss
            && let Some(loss) = loss(item.value)
        {
            append(&value, &[loss]);
        }
        append(
            &node,
            &[box_node(
                "div",
                if item.main {
                    "rt-figure is-main"
                } else {
                    "rt-figure"
                },
                &[label, value],
            )],
        );
    }
    node
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(crate) fn report_table(
    ctx: &Context,
    caption: &str,
    head: &[&str],
    rows: Vec<Row>,
    class: &str,
    fold: &str,
    key: Option<usize>,
) -> Element {
    let tbody = h("tbody", "", "");
    for row in rows {
        let tr = h(
            "tr",
            if row.kind == "group" {
                "rt-group"
            } else if row.kind == "fill" {
                "rt-fill"
            } else if row.kind == "grand" {
                "rt-grand"
            } else if row.kind == "total" {
                "rt-total"
            } else if row.parent {
                "rt-parent"
            } else {
                ""
            },
            "",
        );
        if row.kind == "fill" {
            attrs(&tr, &[("aria-hidden", "true")]);
            let td = h("td", "", "");
            attrs(&td, &[("colspan", &head.len().to_string())]);
            append(&tr, &[td]);
        } else if row.kind == "group" {
            let th = box_node("th", "", &[h("span", "rt-group-label", &row.name)]);
            attrs(
                &th,
                &[("scope", "colgroup"), ("colspan", &head.len().to_string())],
            );
            append(&tr, &[th]);
        } else {
            let name = box_node(
                "th",
                &format!(
                    "rt-name{}{}",
                    if row.parent { " is-parent" } else { "" },
                    if row.direct { " is-direct" } else { "" }
                ),
                &[h("span", "rt-name-text keep", &row.name)],
            );
            attrs(&name, &[("scope", "row")]);
            if !row.sub.is_empty() {
                append(&name, &[h("span", "rt-name-sub keep", &row.sub)]);
            }
            if let Some(note) = row.note {
                append(&name, &[note]);
            }
            append(&tr, &[name]);
            for (i, value) in row.cells.into_iter().enumerate() {
                cell(
                    &tr,
                    head.get(i + 1).copied().unwrap_or(""),
                    value,
                    if key == Some(i) { "num rt-key" } else { "num" },
                );
            }
            if row.depth > 0 {
                let _ = tr
                    .unchecked_ref::<HtmlElement>()
                    .style()
                    .set_property("--depth", &row.depth.to_string());
            }
        }
        append(&tbody, &[tr]);
    }
    let header = h("tr", "", "");
    for (i, label) in head.iter().enumerate() {
        let th = h("th", if i > 0 { "num" } else { "" }, label);
        attrs(&th, &[("scope", "col")]);
        append(&header, &[th]);
    }
    let table = box_node(
        "table",
        &format!(
            "table report-table {} {class}",
            if fold.is_empty() {
                String::new()
            } else {
                format!("rt-fold-{fold}")
            }
        ),
        &[
            h("caption", "", caption),
            box_node("thead", "", &[header]),
            tbody,
        ],
    );
    let wrap = box_node("div", "table-wrap rt-wrap", std::slice::from_ref(&table));
    attrs(&wrap, &[("role", "region"), ("aria-label", caption)]);
    if !fold.is_empty() {
        let _ = wrap.class_list().add_1("rt-cards");
        attrs(&wrap, &[("data-fit", "cards")]);
        let measure: Rc<dyn Fn()> = {
            let wrap = wrap.clone();
            let table = table.clone();
            Rc::new(move || {
                if !wrap.is_connected() || wrap.client_width() == 0 {
                    return;
                }
                let _ = wrap.class_list().remove_1("rt-cards");
                let over =
                    table.unchecked_ref::<HtmlElement>().offset_width() > wrap.client_width() + 1;
                if over {
                    let _ = wrap.class_list().add_1("rt-cards");
                }
                attrs(&wrap, &[("data-fit", if over { "cards" } else { "table" })]);
            })
        };
        let sizer = h("div", "rt-sizer", "");
        attrs(&sizer, &[("aria-hidden", "true")]);
        let _ = wrap.insert_before(&sizer, wrap.first_child().as_ref());
        let observed = measure.clone();
        let size = sizer.clone();
        let last_width = Cell::new(0);
        let callback = Closure::wrap(Box::new(move || {
            let width = size.client_width();
            if width > 0 && last_width.replace(width) != width {
                observed();
            }
        }) as Box<dyn FnMut()>);
        if let Ok(observer) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            observer.observe(&sizer);
            ctx.scope.cleanup(move || {
                observer.disconnect();
                drop(callback);
            });
        }
        let m = measure.clone();
        ctx.scope.listen(
            web_sys::window().unwrap().as_ref(),
            "resize",
            false,
            move |_| m(),
        );
        let m = measure.clone();
        ctx.scope.listen(
            web_sys::window().unwrap().document().unwrap().as_ref(),
            "toggle",
            true,
            move |_| m(),
        );
        let c = ctx.clone();
        ctx.task(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL))
                .await;
            if c.alive() {
                measure();
            }
            Ok(())
        });
    }
    wrap
}
pub(crate) fn model_row(value: &Value, keys: &[&str]) -> Row {
    Row {
        name: s(value, "name").into(),
        sub: s(value, "sub").into(),
        depth: n(value, "depth"),
        parent: yes(value, "parent"),
        direct: yes(value, "direct"),
        cells: keys.iter().map(|k| yen(n(value, k))).collect(),
        ..Row::default()
    }
}
