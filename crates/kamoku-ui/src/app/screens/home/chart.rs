// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    chart_model::{axis, compact, empty_month, loss_labels, segments, summary},
    common::{box_node, cell, empty_value, n, s, set_hidden, table},
};
use crate::app::prelude::*;
use num_traits::ToPrimitive;
use wasm_bindgen::closure::Closure;

fn svg(tag: &str, attributes: &[(&str, &str)], text: &str) -> Element {
    let element = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
        .expect("static SVG element");
    attrs(&element, attributes);
    set_text(&element, text);
    element
}
fn shape(tag: &str, class: &str, values: &[(&str, f64)]) -> Element {
    let element = svg(tag, &[("class", class)], "");
    for (name, value) in values {
        attrs(&element, &[(name, &value.to_string())]);
    }
    element
}
fn label(text: &str, x: f64, y: f64, class: &str, anchor: &str) -> Element {
    let element = shape("text", class, &[("x", x), ("y", y)]);
    attrs(&element, &[("text-anchor", anchor)]);
    set_text(&element, text);
    element
}
fn swatch(children: &[Element]) -> Element {
    let graphic = svg(
        "svg",
        &[
            ("width", "16"),
            ("height", "12"),
            ("viewBox", "0 0 16 12"),
            ("aria-hidden", "true"),
        ],
        "",
    );
    append(&graphic, children);
    graphic
}
fn legend(pattern: &str, highlight: &str) -> Element {
    let node = h("ul", "chart-legend", "");
    let revenue = shape(
        "rect",
        "chart-bar--revenue",
        &[
            ("x", 0.0),
            ("y", 0.0),
            ("width", 16.0),
            ("height", 12.0),
            ("rx", 2.0),
        ],
    );
    let expense = shape(
        "rect",
        "chart-bar--expense",
        &[
            ("x", 0.5),
            ("y", 0.5),
            ("width", 15.0),
            ("height", 11.0),
            ("rx", 2.0),
        ],
    );
    attrs(&expense, &[("fill", &format!("url(#{pattern})"))]);
    let line = svg("path", &[("d", "M1 6h14"), ("class", "chart-line")], "");
    let dot = shape(
        "circle",
        "chart-dot",
        &[("cx", 8.0), ("cy", 6.0), ("r", 3.0)],
    );
    for (text, graphic) in [
        ("売上", swatch(&[revenue])),
        ("経費", swatch(&[expense])),
        ("利益", swatch(&[line, dot])),
    ] {
        append(
            &node,
            &[box_node("li", "", &[graphic, h("span", "", text)])],
        );
    }
    if !highlight.is_empty() {
        append(
            &node,
            &[box_node(
                "li",
                "",
                &[
                    swatch(&[shape(
                        "rect",
                        "chart-highlight chart-swatch-outline",
                        &[
                            ("x", 0.5),
                            ("y", 0.5),
                            ("width", 15.0),
                            ("height", 11.0),
                            ("rx", 2.0),
                        ],
                    )]),
                    h("span", "", "選んだ月"),
                ],
            )],
        );
    }
    node
}
#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
fn draw(
    plot: &Element,
    series: &[Value],
    until: &str,
    highlight: &str,
    title: &str,
    pattern: &str,
) {
    let width = f64::from(plot.client_width().max(280));
    let width = if plot.client_width() == 0 {
        640.0
    } else {
        width
    };
    let plot_width = width - 64.0;
    let plot_height = 200.0;
    let visible = series
        .iter()
        .filter(|r| until.is_empty() || s(r, "month") <= until);
    let range = axis(visible.flat_map(|r| [n(r, "revenue"), n(r, "expense"), n(r, "profit")]));
    let y = |value: i64| {
        12.0 + (range.hi - value.to_f64().unwrap_or(0.0)) / (range.hi - range.lo) * plot_height
    };
    let group_width =
        plot_width / f64::from(u32::try_from(series.len().max(1)).unwrap_or(u32::MAX));
    let bar_width = (group_width * 0.3).clamp(3.0, 18.0);
    let x = |i: usize| {
        52.0 + group_width * f64::from(u32::try_from(i).unwrap_or(u32::MAX)) + group_width / 2.0
    };
    let segments = segments(series, until);
    let points: Vec<_> = segments
        .iter()
        .flatten()
        .map(|i| (*i, n(&series[*i], "profit")))
        .collect();
    let losses = loss_labels(&points, x, 52.0, width - 12.0);
    let height = 240.0
        + f64::from(
            u32::try_from(losses.iter().map(|l| l.row + 1).max().unwrap_or(0)).unwrap_or(u32::MAX),
        ) * 14.0;
    let graphic = svg(
        "svg",
        &[
            ("class", "chart-svg"),
            ("viewBox", &format!("0 0 {width} {height}")),
            ("width", &width.to_string()),
            ("height", &height.to_string()),
            ("role", "img"),
            (
                "aria-label",
                &format!("{title}。{}", summary(series, until)),
            ),
        ],
        "",
    );
    for (i, row) in series.iter().enumerate() {
        if s(row, "month") == highlight {
            append(
                &graphic,
                &[shape(
                    "rect",
                    "chart-highlight",
                    &[
                        (
                            "x",
                            52.0 + group_width * f64::from(u32::try_from(i).unwrap_or(u32::MAX))
                                + 1.0,
                        ),
                        ("y", 12.0),
                        ("width", group_width - 2.0),
                        ("height", plot_height),
                        ("rx", 4.0),
                    ],
                )],
            );
        }
    }
    let mut tick = (range.lo / range.step).ceil() * range.step;
    let mut ticks = 0;
    while tick <= range.hi + range.step / 2.0 && ticks < 20 {
        let yy = (12.0 + (range.hi - tick) / (range.hi - range.lo) * plot_height).round() + 0.5;
        append(
            &graphic,
            &[
                shape(
                    "line",
                    if tick == 0.0 {
                        "chart-zero"
                    } else {
                        "chart-grid"
                    },
                    &[("x1", 52.0), ("x2", width - 12.0), ("y1", yy), ("y2", yy)],
                ),
                label(
                    &tick.trunc().to_i64().map_or_else(String::new, compact),
                    44.0,
                    yy + 4.0,
                    "chart-label",
                    "end",
                ),
            ],
        );
        tick += range.step;
        ticks += 1;
    }
    for (i, row) in series.iter().enumerate() {
        let month = s(row, "month")
            .get(5..7)
            .and_then(|m| m.parse::<u32>().ok())
            .unwrap_or(0);
        append(
            &graphic,
            &[label(
                &if group_width < 34.0 {
                    month.to_string()
                } else {
                    format!("{month}月")
                },
                x(i),
                height - 8.0,
                "chart-label",
                "middle",
            )],
        );
        if (!until.is_empty() && s(row, "month") > until) || empty_month(row) {
            continue;
        }
        for (key, offset, class) in [
            ("revenue", -bar_width - 1.0, "chart-bar--revenue"),
            ("expense", 1.0, "chart-bar--expense"),
        ] {
            let value = n(row, key);
            let top = y(value).min(y(0));
            let size = (y(value) - y(0))
                .abs()
                .max(if value == 0 { 0.0 } else { 1.0 });
            let bar = shape(
                "rect",
                class,
                &[
                    ("x", x(i) + offset),
                    ("y", top),
                    ("width", bar_width),
                    ("height", size),
                    ("rx", 2.0),
                ],
            );
            if key == "expense" {
                attrs(&bar, &[("fill", &format!("url(#{pattern})"))]);
            }
            append(&graphic, &[bar]);
        }
    }
    for segment in segments {
        let path = segment
            .iter()
            .enumerate()
            .map(|(at, i)| {
                format!(
                    "{}{:.1} {:.1}",
                    if at == 0 { "M" } else { "L" },
                    x(*i),
                    y(n(&series[*i], "profit"))
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        append(
            &graphic,
            &[svg("path", &[("d", &path), ("class", "chart-line")], "")],
        );
        for i in segment {
            let value = n(&series[i], "profit");
            append(
                &graphic,
                &[shape(
                    "circle",
                    if value < 0 {
                        "chart-dot chart-dot--loss"
                    } else {
                        "chart-dot"
                    },
                    &[("cx", x(i)), ("cy", y(value)), ("r", 3.5)],
                )],
            );
        }
    }
    for loss in losses {
        let text = label(
            &loss.text,
            loss.x,
            225.0 + f64::from(u32::try_from(loss.row).unwrap_or(u32::MAX)) * 14.0,
            "chart-loss-label",
            "middle",
        );
        attrs(&text, &[("data-month-index", &loss.index.to_string())]);
        append(&graphic, &[text]);
    }
    replace(plot, &[graphic]);
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep the screen DOM and scoped event registrations in one auditable operation."
)]
pub(crate) fn chart(
    ctx: &Context,
    series: &[Value],
    until: &str,
    highlight: &str,
    title: &str,
    with_toggle: bool,
) -> Element {
    let pattern_id = uid("chart-hatch");
    let pattern = svg(
        "pattern",
        &[
            ("id", &pattern_id),
            ("width", "6"),
            ("height", "6"),
            ("patternUnits", "userSpaceOnUse"),
            ("patternTransform", "rotate(45)"),
        ],
        "",
    );
    append(
        &pattern,
        &[
            shape("rect", "chart-hatch-bg", &[("width", 6.0), ("height", 6.0)]),
            shape(
                "line",
                "chart-hatch-line",
                &[("x1", 1.0), ("y1", 0.0), ("x2", 1.0), ("y2", 6.0)],
            ),
        ],
    );
    let defs = svg("defs", &[], "");
    append(&defs, &[pattern]);
    let hidden = svg(
        "svg",
        &[
            ("width", "0"),
            ("height", "0"),
            ("aria-hidden", "true"),
            ("class", "visually-hidden"),
        ],
        "",
    );
    append(&hidden, &[defs]);
    let plot = h("div", "chart-plot", "");
    let table_host = h("div", "chart-table", "");
    set_hidden(&table_host, true);
    if with_toggle {
        let (wrap, tbody) = table(
            &format!("{title}（表）"),
            &["月", "売上", "経費", "利益"],
            "",
        );
        for row in series
            .iter()
            .filter(|r| until.is_empty() || s(r, "month") <= until)
        {
            let tr = h("tr", "", "");
            let month = s(row, "month")
                .get(5..7)
                .and_then(|m| m.parse::<u32>().ok())
                .unwrap_or(0);
            cell(&tr, "月", h("span", "", &format!("{month}月")), "");
            for (key, heading) in [("revenue", "売上"), ("expense", "経費"), ("profit", "利益")]
            {
                let value = if empty_month(row) {
                    empty_value()
                } else {
                    h(
                        "span",
                        "",
                        &format!(
                            "{}{}",
                            number(n(row, key)),
                            if key == "profit" && n(row, key) < 0 {
                                "（赤字）"
                            } else {
                                ""
                            }
                        ),
                    )
                };
                cell(&tr, heading, value, "num");
            }
            append(&tbody, &[tr]);
        }
        append(&table_host, &[wrap]);
    }
    let foot = box_node("div", "chart-foot", &[legend(&pattern_id, highlight)]);
    if with_toggle {
        let toggle = button("表で見る", "");
        let p = plot.clone();
        let t = table_host.clone();
        let b = toggle.clone();
        ctx.on(&toggle, "click", move |_| {
            let show = t.has_attribute("hidden");
            set_hidden(&t, !show);
            set_hidden(&p, show);
            set_text(
                &b,
                if show {
                    "グラフで見る"
                } else {
                    "表で見る"
                },
            );
        });
        append(&foot, &[toggle]);
    }
    let root = box_node("div", "chart", &[hidden, plot.clone()]);
    if with_toggle {
        append(&root, &[table_host]);
    }
    append(
        &root,
        &[
            foot,
            h(
                "p",
                "chart-note",
                &format!(
                    "塗りの棒が売上、斜線の棒が経費、線が利益です。赤字の月は、点を塗って額を添えます。{}",
                    if highlight.is_empty() {
                        ""
                    } else {
                        "網かけは選んだ月です。"
                    }
                ),
            ),
        ],
    );
    let draw_chart: Rc<dyn Fn()> = {
        let plot = plot.clone();
        let series = series.to_vec();
        let until = until.to_owned();
        let highlight = highlight.to_owned();
        let title = title.to_owned();
        let pattern_id = pattern_id.clone();
        Rc::new(move || draw(&plot, &series, &until, &highlight, &title, &pattern_id))
    };
    draw_chart();
    let resized = draw_chart.clone();
    let plot_copy = plot.clone();
    let last_width = Cell::new(0);
    let observer_callback = Closure::wrap(Box::new(move || {
        let width = plot_copy.client_width();
        if width > 0 && last_width.replace(width) != width {
            resized();
        }
    }) as Box<dyn FnMut()>);
    if let Ok(observer) = web_sys::ResizeObserver::new(observer_callback.as_ref().unchecked_ref()) {
        observer.observe(&plot);
        ctx.scope.cleanup(move || {
            observer.disconnect();
            drop(observer_callback);
        });
    }
    let action = draw_chart.clone();
    ctx.scope.listen(
        web_sys::window().unwrap().as_ref(),
        "resize",
        false,
        move |_| action(),
    );
    let c = ctx.clone();
    ctx.task(async move {
        let _ =
            wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL)).await;
        if c.alive() {
            draw_chart();
        }
        Ok(())
    });
    root
}
