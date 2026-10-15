// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pure chart presentation. Coordinates may be fractional; displayed yen remain integers.
use num_traits::ToPrimitive;
use serde_json::Value;
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn int(v: &Value, key: &str) -> i64 {
    v[key].as_i64().unwrap_or(0)
}
pub(crate) fn empty_month(row: &Value) -> bool {
    int(row, "revenue") == 0 && int(row, "expense") == 0
}
pub(crate) fn segments(series: &[Value], until: &str) -> Vec<Vec<usize>> {
    let mut out = vec![];
    let mut run = vec![];
    for (index, row) in series.iter().enumerate() {
        if (!until.is_empty() && text(row, "month") > until) || empty_month(row) {
            if !run.is_empty() {
                out.push(std::mem::take(&mut run));
            }
        } else {
            run.push(index);
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out
}
pub(crate) fn nice_step(range: f64) -> f64 {
    if range <= 0.0 {
        return 1.0;
    }
    let raw = range / 4.0;
    let pow = 10f64.powf(raw.log10().floor());
    let fraction = raw / pow;
    let nice = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * pow
}
#[derive(Debug, PartialEq)]
pub(crate) struct Axis {
    pub lo: f64,
    pub hi: f64,
    pub step: f64,
}
pub(crate) fn axis(values: impl Iterator<Item = i64>) -> Axis {
    let (mut lo, mut hi) = (0i64, 0i64);
    for value in values {
        lo = lo.min(value);
        hi = hi.max(value);
    }
    let flat = hi == lo;
    let lo = lo.to_f64().unwrap_or(0.0);
    let mut hi = hi.to_f64().unwrap_or(0.0);
    if flat {
        hi = lo + 10_000.0;
    }
    let step = nice_step(hi - lo);
    let minor = step / 5.0;
    Axis {
        lo: (lo / minor).floor() * minor,
        hi: (hi / step).ceil() * step,
        step,
    }
}
fn wide_number(value: i128) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    if value < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
pub(crate) fn compact(value: i64) -> String {
    let magnitude = value.unsigned_abs();
    let sign = if value < 0 { "-" } else { "" };
    if magnitude < 10_000 {
        return crate::input::number(value);
    }
    let (unit, divisor) = if magnitude < 100_000_000 {
        ("万", 10_000)
    } else {
        ("億", 100_000_000)
    };
    let whole = magnitude / divisor;
    let tenth = (magnitude % divisor) / (divisor / 10);
    format!(
        "{sign}{}{}{unit}",
        wide_number(i128::from(whole)),
        if whole < 10 && tenth > 0 {
            format!(".{tenth}")
        } else {
            String::new()
        }
    )
}
fn month(value: &Value) -> u32 {
    text(value, "month")
        .get(5..7)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}
pub(crate) fn summary(series: &[Value], until: &str) -> String {
    let rows: Vec<_> = series
        .iter()
        .filter(|r| (until.is_empty() || text(r, "month") <= until) && !empty_month(r))
        .collect();
    let Some(first) = rows.first() else {
        return "表示できる月がありません。".into();
    };
    let last = rows[rows.len() - 1];
    let sum = |key| wide_number(rows.iter().map(|r| i128::from(int(r, key))).sum());
    let mut best = rows[0];
    let mut worst = rows[0];
    for row in &rows {
        if int(row, "profit") > int(best, "profit") {
            best = row;
        }
        if int(row, "profit") < int(worst, "profit") {
            worst = row;
        }
    }
    format!(
        "{}年{}月〜{}月の推移。売上の合計 {}円、経費の合計 {}円、利益の合計 {}円。利益が最も多いのは{}月の{}円、最も少ないのは{}月の{}円{}です。",
        text(first, "month").get(..4).unwrap_or(""),
        month(first),
        month(last),
        sum("revenue"),
        sum("expense"),
        sum("profit"),
        month(best),
        crate::input::number(int(best, "profit")),
        month(worst),
        crate::input::number(int(worst, "profit")),
        if int(worst, "profit") < 0 {
            "（赤字）"
        } else {
            ""
        }
    )
}
pub(crate) fn label_width(text: &str) -> f64 {
    text.chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                6.6
            } else {
                11.0
            }
        })
        .sum()
}
#[derive(Debug)]
pub(crate) struct LossLabel {
    pub index: usize,
    pub x: f64,
    pub text: String,
    pub row: usize,
}
pub(crate) fn loss_labels(
    points: &[(usize, i64)],
    x_of: impl Fn(usize) -> f64,
    left: f64,
    right: f64,
) -> Vec<LossLabel> {
    let mut ends: Vec<f64> = vec![];
    let mut out = vec![];
    let mut points = points.to_vec();
    points.sort_by_key(|p| p.0);
    for (index, profit) in points {
        if profit >= 0 {
            continue;
        }
        let text = compact(profit);
        let half = label_width(&text) / 2.0 + 2.0;
        let x = x_of(index).max(left + half).min(right - half);
        let row = if let Some(row) = ends.iter().position(|end| x - half >= *end) {
            row
        } else {
            if ends.len() >= 2 {
                continue;
            }
            ends.push(0.0);
            ends.len() - 1
        };
        ends[row] = x + half;
        out.push(LossLabel {
            index,
            x,
            text,
            row,
        });
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn m(month: &str, revenue: i64, expense: i64) -> Value {
        json!({"month":month,"revenue":revenue,"expense":expense,"profit":revenue-expense})
    }
    #[test]
    fn compact_axis_labels_match_existing_yen_units() {
        for (value, label) in [
            (0, "0"),
            (5000, "5,000"),
            (50_000, "5万"),
            (15_000, "1.5万"),
            (1_200_000, "120万"),
            (-300_000, "-30万"),
            (150_000_000, "1.5億"),
        ] {
            assert_eq!(compact(value), label);
        }
    }
    #[test]
    fn empty_and_future_months_break_profit_line() {
        let rows = [
            m("2026-01", 100, 40),
            m("2026-02", 0, 0),
            m("2026-03", 200, 250),
            m("2026-04", 300, 100),
            m("2026-05", 0, 0),
            m("2026-06", 0, 0),
            m("2026-07", 50, 10),
            m("2026-08", 70, 10),
        ];
        assert_eq!(
            segments(&rows, "2026-07"),
            vec![vec![0], vec![2, 3], vec![6]]
        );
        assert_eq!(segments(&rows, "").last(), Some(&vec![6, 7]));
        assert!(empty_month(&m("2026-01", 0, 0)));
        assert!(!empty_month(&m("2026-01", 0, -300)));
        assert_eq!(
            segments(&[m("2026-01", 0, 0)], ""),
            [] as [std::vec::Vec<usize>; 0]
        );
    }
    #[test]
    fn shallow_losses_do_not_extend_axis_a_whole_tick() {
        assert_eq!(
            axis([0, 1_005_400, 330_000, -25_175, 882_682].into_iter()),
            Axis {
                lo: -100_000.0,
                hi: 1_500_000.0,
                step: 500_000.0
            }
        );
        assert_eq!(
            axis([0, 120, 80].into_iter()),
            Axis {
                lo: 0.0,
                hi: 150.0,
                step: 50.0
            }
        );
        let zero = axis([0, 0, 0].into_iter());
        assert!(zero.hi > zero.lo);
        let deep = axis([0, 100_000, -900_000].into_iter());
        assert!(deep.lo <= -900_000.0 && deep.hi >= 100_000.0);
    }
    #[test]
    fn summary_excludes_empty_and_future_months_and_keeps_precise_sums() {
        let rows = [
            m("2026-01", 100, 40),
            m("2026-02", 0, 0),
            m("2026-03", 200, 250),
            m("2026-04", 900, 0),
        ];
        let text = summary(&rows, "2026-03");
        assert!(text.starts_with("2026年1月〜3月の推移。"));
        assert!(text.contains("売上の合計 300円"));
        assert!(text.contains("最も少ないのは3月の-50円（赤字）"));
        let large = [m("2026-01", 9_007_199_254_740_993, 0)];
        assert!(summary(&large, "").contains("9,007,199,254,740,993円"));
        assert_eq!(
            summary(&[m("2026-01", 0, 0)], ""),
            "表示できる月がありません。"
        );
    }
    #[test]
    fn loss_labels_use_two_rows_without_overlap_or_edge_overflow() {
        let points = [(0, 120_000), (3, -25175), (4, -8000), (7, -300_000)];
        let wide = loss_labels(
            &points,
            |i| 52.0 + 80.0 * f64::from(u32::try_from(i).unwrap_or(u32::MAX)) + 40.0,
            52.0,
            1012.0,
        );
        assert_eq!(
            wide.iter()
                .map(|p| (p.index, p.text.as_str(), p.row))
                .collect::<Vec<_>>(),
            vec![(3, "-2.5万", 0), (4, "-8,000", 0), (7, "-30万", 0)]
        );
        let narrow = loss_labels(
            &points,
            |i| 52.0 + 22.0 * f64::from(u32::try_from(i).unwrap_or(u32::MAX)) + 11.0,
            52.0,
            316.0,
        );
        assert_eq!(
            narrow.iter().map(|p| (p.index, p.row)).collect::<Vec<_>>(),
            vec![(3, 0), (4, 1), (7, 0)]
        );
        let tight = loss_labels(
            &[(3, -9000), (4, -9000), (5, -9000)],
            |i| 52.0 + 14.0 * f64::from(u32::try_from(i).unwrap_or(u32::MAX)),
            52.0,
            316.0,
        );
        assert_eq!(tight.len(), 2);
        let edge = loss_labels(&[(0, -1_200_000)], |_| 60.0, 52.0, 316.0);
        assert!(edge[0].x - label_width(&edge[0].text) / 2.0 >= 52.0);
        assert!(label_width("-2.5万") > label_width("-2.55"));
        assert!(
            loss_labels(
                &[(1, 0), (2, 5)],
                |index| f64::from(u32::try_from(index).unwrap()) * 40.0,
                0.0,
                400.0
            )
            .is_empty()
        );
    }
}
