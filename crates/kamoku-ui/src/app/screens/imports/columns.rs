// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::wrap_hint;
use super::{
    Cell, Context, Element, Field, HtmlInputElement, HtmlSelectElement, Rc, State, Value, append,
    array, attrs, button, column_count, column_options, date_warning, delimiter_label,
    describe_source, described, encoding_label, field, focus, h, hidden, icon, initial_mapping,
    input, json, mapping_problems, node, replace, sample, select, set_text, text,
};
use crate::app::prelude::phrase;
struct Controls {
    owner: std::rc::Weak<State>,
    ctx: Context,
    up: Value,
    date: HtmlSelectElement,
    date_field: Field,
    date_warning: Element,
    date_seq: Cell<u64>,
    skip: HtmlInputElement,
    skip_field: Field,
    desc: Vec<HtmlInputElement>,
    id: HtmlSelectElement,
    id_field: Field,
    out: HtmlSelectElement,
    incoming: HtmlSelectElement,
    amount: HtmlSelectElement,
    out_field: Field,
    in_field: Field,
    amount_field: Field,
    amount_error: Element,
    mode: Vec<HtmlInputElement>,
    inflow: Vec<HtmlInputElement>,
    inflow_el: Element,
    sample: Element,
    examples: Vec<(HtmlSelectElement, Element)>,
}
impl Controls {
    fn split(&self) -> bool {
        self.mode.first().is_some_and(HtmlInputElement::checked)
    }
    fn current(&self) -> Value {
        let col = |s: &HtmlSelectElement| s.value().parse::<u64>().map_or(Value::Null, Value::from);
        json!({"skip_rows":self.skip.value().trim().parse::<u64>().ok(),"date_col":col(&self.date),"desc_cols":self.desc.iter().filter(|c|c.checked()).filter_map(|c|c.value().parse::<u64>().ok()).collect::<Vec<_>>(),"id_col":col(&self.id),"amount":if self.split(){json!({"mode":"split","out_col":col(&self.out),"in_col":col(&self.incoming)})}else{json!({"mode":"signed","col":col(&self.amount),"inflow":if self.inflow.first().is_some_and(HtmlInputElement::checked){"positive"}else{"negative"}})}})
    }
    fn paint(&self) {
        let mapping = self.current();
        hidden(&self.out_field.el, !self.split());
        hidden(&self.in_field.el, !self.split());
        hidden(&self.amount_field.el, self.split());
        hidden(&self.inflow_el, self.split());
        replace(&self.sample, &sample::view(&self.up, &mapping));
        let opts = column_options(&self.up);
        for (sel, line) in &self.examples {
            let value = sel.value().parse::<usize>().ok();
            let text = value
                .and_then(|i| opts.get(i))
                .filter(|c| !c.example.is_empty())
                .map_or_else(String::new, |c| format!("例: {}", c.example));
            set_text(line, &text);
        }
    }
    fn validate(&self) -> Vec<(&'static str, &'static str)> {
        let problems = mapping_problems(&self.current(), column_count(&self.up));
        let message = |field: &str| problems.iter().find(|p| p.0 == field).map_or("", |p| p.1);
        self.date_field.set_error(message("date_col"));
        self.skip_field.set_error(message("skip_rows"));
        self.id_field.set_error(message("id_col"));
        set_text(&self.amount_error, message("amount"));
        problems
    }
    fn check_dates(self: &Rc<Self>) {
        let n = self.date_seq.get() + 1;
        self.date_seq.set(n);
        set_text(&self.date_warning, "");
        let mapping = self.current();
        if !mapping_problems(&mapping, column_count(&self.up)).is_empty() {
            return;
        }
        let Some(s) = self.owner.upgrade() else {
            return;
        };
        let body = json!({"upload_id":s.upload.borrow()["upload_id"],"wallet":s.wallet.borrow().as_str(),"mapping":mapping});
        let controls = self.clone();
        let ctx = self.ctx.clone();
        self.ctx.task(async move {
            let result = ctx.post("/imports/preview", &body).await;
            if ctx.alive()
                && controls.date_seq.get() == n
                && controls.date_warning.is_connected()
                && let Ok(value) = result
            {
                let warning = date_warning(array(&value["rows"]));
                if !warning.is_empty() {
                    replace(
                        &controls.date_warning,
                        &[icon("alert"), node("span", "", &[phrase(&warning)])],
                    );
                }
            }
            Ok(())
        });
    }
    fn submit(&self) {
        let problems = self.validate();
        if let Some((field, _)) = problems.first() {
            focus(match *field {
                "date_col" => self.date.as_ref(),
                "skip_rows" => self.skip.as_ref(),
                "id_col" => self.id.as_ref(),
                _ => {
                    if self.split() {
                        self.out.as_ref()
                    } else {
                        self.amount.as_ref()
                    }
                }
            });
            return;
        }
        if let Some(s) = self.owner.upgrade() {
            s.preview(&self.current());
        }
    }
}
fn radios(
    legend: &str,
    name: &str,
    choices: &[(&str, &str)],
    value: &str,
) -> (Element, Vec<HtmlInputElement>) {
    let inputs: Vec<_> = choices
        .iter()
        .map(|(v, _)| {
            let el = input("radio", v);
            attrs(&el, &[("name", name)]);
            el.set_checked(*v == value);
            el
        })
        .collect();
    let options = h("div", "seg-options", "");
    for ((_, label), input) in choices.iter().zip(&inputs) {
        let mark = icon("check");
        attrs(&mark, &[("class", "icon seg-check")]);
        append(
            &options,
            &[node(
                "label",
                "seg-option",
                &[input.clone().into(), mark, h("span", "", label)],
            )],
        );
    }
    let el = node(
        "fieldset",
        "seg",
        &[h("legend", "field-label", legend), options],
    );
    (el, inputs)
}
fn column_select(up: &Value, value: &Value, none: bool) -> HtmlSelectElement {
    let sel = select(&[], "");
    if none {
        let option = h("option", "", "使わない");
        attrs(&option, &[("value", "")]);
        append(&sel, &[option]);
    }
    for col in column_options(up) {
        let option = h("option", "", &col.label);
        attrs(&option, &[("value", &col.value.to_string())]);
        append(&sel, &[option]);
    }
    sel.set_value(&value.as_u64().map(|v| v.to_string()).unwrap_or_default());
    sel
}
impl State {
    #[allow(
        clippy::too_many_lines,
        reason = "Assemble the labelled mapping form, sample and native control handlers together."
    )]
    pub fn step_columns(self: &Rc<Self>, kept: Option<&Value>) {
        let ctx = self.begin_step();
        let up = self.upload.borrow().clone();
        let (mapping, source) = initial_mapping(&up, kept);
        let n = column_count(&up);
        let amount = &mapping["amount"];
        let date = column_select(&up, &mapping["date_col"], false);
        let date_field = field("日付の列", &date, "", true);
        let date_warning = h("p", "field-hint col-warn", "");
        attrs(
            &date_warning,
            &[("id", "import-date-warning"), ("aria-live", "polite")],
        );
        described(&date, "import-date-warning");
        append(&date_field.el, std::slice::from_ref(&date_warning));
        let desc: Vec<_> = column_options(&up)
            .iter()
            .map(|col| {
                let input = input("checkbox", &col.value.to_string());
                input.set_checked(array(&mapping["desc_cols"]).contains(&json!(col.value)));
                input
            })
            .collect();
        let desc_list = h("div", "check-list", "");
        for (col, input) in column_options(&up).iter().zip(&desc) {
            append(
                &desc_list,
                &[node(
                    "label",
                    "check",
                    &[input.clone().into(), h("span", "keep", &col.label)],
                )],
            );
        }
        let desc_set = node(
            "fieldset",
            "check-group",
            &[
                node(
                    "legend",
                    "field-label",
                    &[phrase(
                        "摘要の列（いくつでも選べます。選んだ順に、空白でつなぎます）",
                    )],
                ),
                desc_list,
            ],
        );
        let out = column_select(
            &up,
            &if text(amount, "mode") == "split" {
                amount["out_col"].clone()
            } else {
                json!(2.min(n - 1))
            },
            false,
        );
        let incoming = column_select(
            &up,
            &if text(amount, "mode") == "split" {
                amount["in_col"].clone()
            } else {
                json!(3.min(n - 1))
            },
            false,
        );
        let amount_sel = column_select(
            &up,
            &if text(amount, "mode") == "signed" {
                amount["col"].clone()
            } else {
                json!(2.min(n - 1))
            },
            false,
        );
        let out_field = field("出金の列", &out, "", false);
        let in_field = field("入金の列", &incoming, "", false);
        let amount_field = field("金額の列", &amount_sel, "", false);
        let (id_sel, reference_field) = {
            let sel = column_select(&up, &mapping["id_col"], true);
            let f = field(
                "取引番号の列（任意）",
                &sel,
                "金融機関が付けた番号の列があれば選んでください。重複を見分ける、いちばん確かな手がかりです。",
                false,
            );
            wrap_hint(&f);
            (sel, f)
        };
        let skip = input(
            "text",
            &mapping["skip_rows"].as_u64().unwrap_or(0).to_string(),
        );
        skip.set_class_name("input input--num skip-input");
        attrs(&skip, &[("inputmode", "numeric")]);
        let skip_field = field(
            "読み飛ばす行の数",
            &node(
                "div",
                "affix",
                &[skip.clone().into(), h("span", "affix-text", "行")],
            ),
            "ファイルの先頭から、見出しや注意書きの行を数えて入れます。",
            false,
        );
        wrap_hint(&skip_field);
        let (mode_el, mode) = radios(
            "金額の形",
            "import-amount-mode",
            &[("split", "入金と出金が別の列"), ("signed", "符号つきの1列")],
            text(amount, "mode"),
        );
        let (inflow_el, inflow) = radios(
            "入金の向き",
            "import-inflow",
            &[
                ("positive", "正の数が入金（銀行の明細に多い）"),
                ("negative", "正の数が出金（カードの明細に多い）"),
            ],
            if text(amount, "inflow") == "negative" {
                "negative"
            } else {
                "positive"
            },
        );
        inflow_el.set_class_name("seg seg--wrap");
        let amount_error = h("p", "field-error", "");
        attrs(&amount_error, &[("role", "alert")]);
        let sample = h("div", "import-sample-host", "");
        let mut examples = vec![];
        for (sel, f) in [
            (&date, &date_field),
            (&out, &out_field),
            (&incoming, &in_field),
            (&amount_sel, &amount_field),
            (&id_sel, &reference_field),
        ] {
            let line = h("p", "field-hint col-example", "");
            let id = format!("{}-ex", sel.id());
            attrs(&line, &[("id", &id)]);
            described(sel, &id);
            append(&f.el, std::slice::from_ref(&line));
            examples.push((sel.clone(), line));
        }
        let form = node(
            "form",
            "import-map",
            &[
                node(
                    "div",
                    "map-grid",
                    &[
                        date_field.el.clone(),
                        skip_field.el.clone(),
                        desc_set,
                        reference_field.el.clone(),
                    ],
                ),
                node(
                    "div",
                    "map-amount",
                    &[
                        mode_el,
                        node(
                            "div",
                            "map-grid",
                            &[
                                out_field.el.clone(),
                                in_field.el.clone(),
                                amount_field.el.clone(),
                            ],
                        ),
                        inflow_el.clone(),
                        amount_error.clone(),
                    ],
                ),
            ],
        );
        attrs(&form, &[("novalidate", ""), ("aria-label", "列の対応")]);
        ctx.on(&form, "submit", |e| e.prevent_default());
        let c = Rc::new(Controls {
            owner: Rc::downgrade(self),
            ctx: ctx.clone(),
            up: up.clone(),
            date,
            date_field,
            date_warning,
            date_seq: Cell::new(0),
            skip,
            skip_field,
            desc,
            id: id_sel,
            id_field: reference_field,
            out,
            incoming,
            amount: amount_sel,
            out_field,
            in_field,
            amount_field,
            amount_error,
            mode,
            inflow,
            inflow_el,
            sample: sample.clone(),
            examples,
        });
        let ctrl = c.clone();
        ctx.on(&form, "change", move |_| {
            ctrl.paint();
            ctrl.validate();
        });
        let ctrl = c.clone();
        ctx.on(&c.skip, "input", move |_| {
            ctrl.validate();
            ctrl.paint();
        });
        let ctrl = c.clone();
        ctx.on(&c.date, "change", move |_| ctrl.check_dates());
        c.paint();
        let auto_label = if *self.encoding.borrow() == "auto" {
            format!("自動で判定（{}）", encoding_label(text(&up, "encoding")))
        } else {
            "自動で判定".into()
        };
        let encoding = select(
            &[
                ("auto", &auto_label),
                ("utf-8", "UTF-8"),
                ("shift_jis", "Shift_JIS"),
            ],
            &self.encoding.borrow(),
        );
        attrs(&encoding, &[("aria-label", "文字コード")]);
        let weak = Rc::downgrade(self);
        let enc = encoding.clone();
        ctx.on(&encoding, "change", move |_| {
            if let Some(s) = weak.upgrade() {
                if s.busy.get() {
                    return;
                }
                *s.encoding.borrow_mut() = enc.value();
                s.start_upload(true, enc.clone());
            }
        });
        let facts = h("dl", "import-facts", "");
        for (label, value) in [
            ("ファイル", h("dd", "", &self.filename.borrow())),
            ("文字コード", node("dd", "enc-dd", &[encoding.into()])),
            (
                "区切り",
                h("dd", "", &delimiter_label(text(&up, "delimiter"))),
            ),
            (
                "行の数",
                h("dd", "", &format!("{}行", up["rows"].as_u64().unwrap_or(0))),
            ),
        ] {
            append(&facts, &[node("div", "", &[h("dt", "", label), value])]);
        }
        let back = button("戻る", "ghost");
        let _ = back.insert_before(&icon("chevron-left"), back.first_child().as_ref());
        let weak = Rc::downgrade(self);
        ctx.on(&back, "click", move |_| {
            if let Some(s) = weak.upgrade() {
                s.step_file(None);
            }
        });
        let next = button("確かめる", "primary");
        append(&next, &[icon("chevron-right")]);
        let ctrl = c.clone();
        ctx.on(&next, "click", move |_| ctrl.submit());
        let actions = node("div", "import-actions", &[back, next]);
        attrs(&actions, &[("data-bottom-bar", "true")]);
        let source = node(
            "p",
            "import-source",
            &[
                icon(if source == "none" { "alert" } else { "info" }),
                node("span", "", &[phrase(describe_source(source))]),
            ],
        );
        let mut content = vec![facts];
        if !array(&up["warnings"]).is_empty() {
            content.push(node(
                "div",
                "banner banner--warn",
                &[
                    h("h3", "banner-title", "読めない文字がありました"),
                    h(
                        "p",
                        "",
                        &array(&up["warnings"])
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                ],
            ));
        }
        content.extend([source, sample, form, actions]);
        self.show(1, node("div", "import-step stack", &content), None);
    }
}
