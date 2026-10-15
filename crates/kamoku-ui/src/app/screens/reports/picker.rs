// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
use super::super::home::common::{Segmented, box_node, el, icon_button, segmented, text_input};
use super::period::{Period, range_error};
use crate::app::prelude::*;
#[derive(Clone)]
pub(crate) struct Picker {
    pub el: Element,
    host: Element,
    mode: Option<Segmented>,
    period: Rc<RefCell<Period>>,
    ctx: Context,
    draw_scope: Rc<RefCell<Scope>>,
    first: i32,
    on_change: Rc<dyn Fn(Period)>,
    on_invalid: Rc<dyn Fn(bool)>,
}
impl Picker {
    pub fn new(
        ctx: &Context,
        period: Period,
        first: i32,
        date: bool,
        year_only: bool,
        change: impl Fn(Period) + 'static,
        invalid: impl Fn(bool) + 'static,
    ) -> Self {
        let state = Rc::new(RefCell::new(period));
        let host = h("div", "period-controls", "");
        let node = h("div", "period-picker", "");
        let mode = if date || year_only {
            None
        } else {
            Some(segmented(
                ctx,
                "期間の選び方",
                &[("year", "年"), ("month", "月"), ("range", "自由な期間")],
                &state.borrow().mode,
                |_| {},
            ))
        };
        if let Some(seg) = &mode {
            let _ = seg.node.class_list().add_1("period-mode");
            append(&node, std::slice::from_ref(&seg.node));
        }
        append(&node, std::slice::from_ref(&host));
        let draw_scope = Rc::new(RefCell::new(Scope::new()));
        let cleanup = draw_scope.clone();
        ctx.scope.cleanup(move || cleanup.borrow().dispose());
        let picker = Self {
            el: node,
            host,
            mode,
            period: state,
            ctx: ctx.clone(),
            draw_scope,
            first,
            on_change: Rc::new(change),
            on_invalid: Rc::new(invalid),
        };
        if let Some(mode) = &picker.mode {
            for radio in &mode.radios {
                let picker = picker.clone();
                let r = radio.clone();
                ctx.on(&el(radio), "change", move |_| {
                    if !r.checked() {
                        return;
                    }
                    let value = picker.period.borrow().clone();
                    match r.value().as_str() {
                        "year" => (picker.on_change)(Period::year(value.year)),
                        "month" => {
                            let month = if value.mode == "month" {
                                value.month[5..].to_owned()
                            } else {
                                today()[5..7].to_owned()
                            };
                            (picker.on_change)(Period::month(&format!(
                                "{:04}-{month}",
                                value.year
                            )));
                        }
                        _ => {
                            picker.period.borrow_mut().mode = "range".into();
                            picker.draw();
                        }
                    }
                });
            }
        }
        picker.draw();
        picker
    }
    pub fn set(&self, value: Period) {
        *self.period.borrow_mut() = value;
        self.draw();
    }
    pub fn shift(&self, step: i32) -> bool {
        let picker = self.period.borrow().clone();
        if picker.mode == "range" {
            return false;
        }
        (self.on_change)(picker.shift(step));
        true
    }
    fn years(&self, period: &Period) -> HtmlSelectElement {
        let now = today()[..4].parse::<i32>().unwrap_or(period.year);
        let values: Vec<_> = (self.first.min(period.year)..=now.max(period.year))
            .map(|year_control| (format!("{year_control:04}"), format!("{year_control}年")))
            .collect();
        let pairs: Vec<_> = values
            .iter()
            .map(|(v, l)| (v.as_str(), l.as_str()))
            .collect();
        let select = select(&pairs, &format!("{:04}", period.year));
        attrs(
            &el(&select),
            &[
                ("class", "select period-year"),
                ("aria-label", "年"),
                ("data-role", "year"),
            ],
        );
        select
    }
    fn previous_next(&self, label: &str) -> (Element, Element) {
        let previous = icon_button(&format!("前の{label}"), "chevron-left", "");
        let next = icon_button(&format!("次の{label}"), "chevron-right", "");
        for (b, step, key, role) in [(&previous, -1, "[", "prev"), (&next, 1, "]", "next")] {
            attrs(
                b,
                &[
                    ("aria-keyshortcuts", key),
                    (
                        "title",
                        &format!(
                            "{}（{key} キー）",
                            b.get_attribute("aria-label").unwrap_or_default()
                        ),
                    ),
                    ("data-role", role),
                ],
            );
            let picker = self.clone();
            self.on(b, "click", move |_| {
                picker.shift(step);
            });
        }
        (previous, next)
    }
    fn on(&self, target: &Element, event: &str, listener: impl Fn(Event) + 'static) {
        self.draw_scope.borrow().on(target, event, listener);
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
    )]
    fn draw(&self) {
        self.draw_scope.borrow().dispose();
        *self.draw_scope.borrow_mut() = Scope::new();
        let active = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|date_control| date_control.active_element())
            .filter(|event| self.host.contains(Some(event)))
            .and_then(|event| event.get_attribute("data-role"));
        (self.on_invalid)(false);
        let period = self.period.borrow().clone();
        if let Some(mode) = &self.mode {
            mode.set(&period.mode);
        }
        let row = h("div", "period-row", "");
        match period.mode.as_str() {
            "date" => {
                let date = text_input(&period.as_of, "いつの時点か");
                attrs(&el(&date), &[("data-role", "date")]);
                let (previous_button, next_button) = self.previous_next("月末");
                let today_button = button("今日", "");
                attrs(&today_button, &[("data-role", "today")]);
                let change = self.on_change.clone();
                self.on(&today_button, "click", move |_| {
                    change(Period::date(&today()));
                });
                let change: Rc<dyn Fn()> = {
                    let date_control = date.clone();
                    let picker = self.clone();
                    Rc::new(move || match parse_date(&date_control.value(), &today()) {
                        Ok(v) => {
                            let _ = el(&date_control).remove_attribute("aria-invalid");
                            date_control.set_value(&v);
                            if v != picker.period.borrow().as_of {
                                (picker.on_change)(Period::date(&v));
                            }
                        }
                        Err(event) => {
                            attrs(&el(&date_control), &[("aria-invalid", "true")]);
                            picker.ctx.error(&event);
                        }
                    })
                };
                let blur_change = change.clone();
                self.on(&el(&date), "blur", move |_| blur_change());
                self.on(&el(&date), "keydown", move |event| {
                    if let Some(key) = event.dyn_ref::<KeyboardEvent>()
                        && !ime_ignore(key)
                        && key.key() == "Enter"
                    {
                        key.prevent_default();
                        change();
                    }
                });
                append(
                    &row,
                    &[
                        previous_button,
                        box_node("div", "period-date", &[el(&date)]),
                        next_button,
                        today_button,
                    ],
                );
                replace(&self.host, &[row]);
            }
            "year" => {
                let (previous_button, next_button) = self.previous_next("年");
                let year = self.years(&period);
                let select = year.clone();
                let change = self.on_change.clone();
                self.on(&el(&year), "change", move |_| {
                    if let Ok(year_control) = select.value().parse() {
                        change(Period::year(year_control));
                    }
                });
                append(&row, &[previous_button, el(&year), next_button]);
                replace(&self.host, &[row]);
            }
            "month" => {
                let (previous_button, next_button) = self.previous_next("月");
                let year = self.years(&period);
                let values: Vec<_> = (1..=12)
                    .map(|m| (format!("{m:02}"), format!("{m}月")))
                    .collect();
                let pairs: Vec<_> = values
                    .iter()
                    .map(|(v, l)| (v.as_str(), l.as_str()))
                    .collect();
                let month = select(&pairs, &period.month[5..]);
                attrs(
                    &el(&month),
                    &[
                        ("class", "select period-month"),
                        ("aria-label", "月"),
                        ("data-role", "month"),
                    ],
                );
                let change: Rc<dyn Fn()> = {
                    let year_control = year.clone();
                    let m = month.clone();
                    let change = self.on_change.clone();
                    Rc::new(move || {
                        change(Period::month(&format!(
                            "{}-{}",
                            year_control.value(),
                            m.value()
                        )));
                    })
                };
                for control in [el(&year), el(&month)] {
                    let change = change.clone();
                    self.on(&control, "change", move |_| change());
                }
                append(&row, &[previous_button, el(&year), el(&month), next_button]);
                replace(&self.host, &[row]);
            }
            _ => self.range(&period, &row),
        }
        if let Some(role) = active
            && let Ok(Some(control)) = self.host.query_selector(&format!("[data-role='{role}']"))
        {
            focus(&control);
        }
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
    )]
    fn range(&self, period: &Period, row: &Element) {
        let _ = row.class_list().add_1("period-range");
        let year = self.years(period);
        let from = text_input(&period.from, "期間の始め");
        let to = text_input(&period.to, "期間の終わり");
        attrs(&el(&from), &[("data-role", "from")]);
        attrs(&el(&to), &[("data-role", "to")]);
        let error = h("p", "field-error", "");
        attrs(&error, &[("role", "alert"), ("id", &uid("period-err"))]);
        let check: Rc<dyn Fn(bool) -> Option<(String, Element)>> = {
            let from_control = from.clone();
            let to_control = to.clone();
            let year_control = year.clone();
            let error = error.clone();
            let invalid = self.on_invalid.clone();
            Rc::new(move |commit| {
                for input in [&from_control, &to_control] {
                    let _ = el(input).remove_attribute("aria-invalid");
                    let _ = el(input).remove_attribute("aria-describedby");
                }
                let read = |input: &HtmlInputElement| parse_date(&input.value(), &today());
                let a = read(&from_control);
                let b = read(&to_control);
                if commit {
                    if let Ok(v) = &a {
                        from_control.set_value(v);
                    }
                    if let Ok(v) = &b {
                        to_control.set_value(v);
                    }
                }
                let problem = match (a, b) {
                    (Err(_), _) => Some((
                        "期間の始めと終わりを、日付で入れてください。".into(),
                        el(&from_control),
                    )),
                    (_, Err(_)) => Some((
                        "期間の始めと終わりを、日付で入れてください。".into(),
                        el(&to_control),
                    )),
                    (Ok(a), Ok(b)) => {
                        let outside = format!(
                            "{}年の中の日付にしてください。帳票は1年ずつ作るので、年をまたぐ期間は選べません。",
                            year_control.value()
                        );
                        if a[..4] != year_control.value() {
                            Some((outside, el(&from_control)))
                        } else if b[..4] != year_control.value() {
                            Some((outside, el(&to_control)))
                        } else {
                            range_error(&a, &b).map(|event| (event, el(&from_control)))
                        }
                    }
                };
                set_text(&error, problem.as_ref().map_or("", |(m, _)| m));
                if let Some((_, input)) = &problem {
                    attrs(
                        input,
                        &[("aria-invalid", "true"), ("aria-describedby", &error.id())],
                    );
                }
                invalid(problem.is_some());
                problem
            })
        };
        let apply = button("この期間で見る", "primary");
        attrs(&apply, &[("data-role", "apply")]);
        let submit: Rc<dyn Fn()> = {
            let check = check.clone();
            let from_control = from.clone();
            let to_control = to.clone();
            let year_control = year.clone();
            let change = self.on_change.clone();
            Rc::new(move || {
                if let Some((_, input)) = check(true) {
                    focus(&input);
                    return;
                }
                let mut picker = Period::year(year_control.value().parse().unwrap_or(0));
                picker.mode = "range".into();
                picker.from = from_control.value();
                picker.to = to_control.value();
                change(picker);
            })
        };
        let action = submit.clone();
        self.on(&apply, "click", move |_| action());
        for input in [from.clone(), to.clone()] {
            let check = check.clone();
            self.on(&el(&input), "blur", move |_| {
                check(true);
            });
            let action = submit.clone();
            self.on(&el(&input), "keydown", move |event| {
                if let Some(key) = event.dyn_ref::<KeyboardEvent>()
                    && !ime_ignore(key)
                    && key.key() == "Enter"
                {
                    key.prevent_default();
                    action();
                }
            });
            let from_control = from.clone();
            let to_control = to.clone();
            let error = error.clone();
            let invalid = self.on_invalid.clone();
            self.on(&el(&input), "input", move |_| {
                for input in [&from_control, &to_control] {
                    let _ = el(input).remove_attribute("aria-invalid");
                    let _ = el(input).remove_attribute("aria-describedby");
                }
                set_text(&error, "");
                invalid(false);
            });
        }
        let from_control = from.clone();
        let to_control = to.clone();
        let year_control = year.clone();
        let check2 = check.clone();
        self.on(&el(&year), "change", move |_| {
            for input in [&from_control, &to_control] {
                let value = input.value();
                let suffix = value.get(4..).unwrap_or("-01-01");
                input.set_value(&format!("{}{suffix}", year_control.value()));
            }
            check2(true);
        });
        append(
            row,
            &[
                el(&year),
                box_node("div", "period-date", &[el(&from)]),
                h("span", "muted", "〜"),
                box_node("div", "period-date", &[el(&to)]),
                apply,
            ],
        );
        replace(&self.host, &[row.clone(), error]);
        check(false);
    }
}
