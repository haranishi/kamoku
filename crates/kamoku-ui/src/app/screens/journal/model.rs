// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) const PAGE_SIZE: u64 = 50;
const KEYS: [&str; 10] = [
    "from", "to", "account", "wallet", "q", "kind", "state", "min", "max", "page",
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Filters {
    pub values: BTreeMap<String, String>,
    pub page: u64,
}
impl Filters {
    pub(crate) fn read(mut get: impl FnMut(&str) -> String) -> Self {
        let mut values = BTreeMap::new();
        for key in KEYS {
            let value = get(key);
            if !value.is_empty() {
                values.insert(key.into(), value);
            }
        }
        for key in ["from", "to"] {
            if values.get(key).is_some_and(|date| !is_iso(date)) {
                values.remove(key);
            }
        }
        if !values
            .get("state")
            .is_some_and(|state| matches!(state.as_str(), "active" | "voided" | "all"))
        {
            values.insert("state".into(), "all".into());
        }
        if !values.get("kind").is_some_and(|kind| {
            matches!(
                kind.as_str(),
                "expense" | "income" | "transfer" | "journal" | "opening"
            )
        }) {
            values.remove("kind");
        }
        for key in ["min", "max"] {
            if values
                .get(key)
                .is_some_and(|amount| !amount.bytes().all(|byte| byte.is_ascii_digit()))
            {
                values.remove(key);
            }
        }
        let page = values
            .remove("page")
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|page| *page > 1)
            .unwrap_or(1);
        Self { values, page }
    }
    pub(crate) fn get(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or_default()
    }
    pub(crate) fn patch(&self, key: &str, value: &str) -> Self {
        let mut next = self.clone();
        next.page = 1;
        if value.is_empty() {
            next.values.remove(key);
        } else {
            next.values.insert(key.into(), value.into());
        }
        next
    }
    pub(crate) fn query(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = KEYS
            .iter()
            .filter_map(|key| {
                self.values
                    .get(*key)
                    .filter(|value| {
                        !(value.is_empty() || *key == "state" && value.as_str() == "all")
                    })
                    .map(|value| ((*key).into(), value.clone()))
            })
            .collect();
        if self.page > 1 {
            out.push(("page".into(), self.page.to_string()));
        }
        out
    }
    pub(crate) fn api(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        out.push(("limit".into(), PAGE_SIZE.to_string()));
        out.push((
            "offset".into(),
            self.page
                .saturating_sub(1)
                .saturating_mul(PAGE_SIZE)
                .to_string(),
        ));
        out
    }
    pub(crate) fn exact_month(&self) -> Option<String> {
        let from = self.get("from");
        let to = self.get("to");
        if !is_iso(from) || !is_iso(to) {
            return None;
        }
        let ym = &from[..7];
        let (first, last) = month_range(ym)?;
        if first == from && last == to {
            Some(ym.into())
        } else {
            None
        }
    }
    pub(crate) fn shift(&self, delta: i32, today: &str) -> Self {
        let exact = self.exact_month();
        let source = exact.as_deref().unwrap_or_else(|| {
            if self.get("from").is_empty() {
                today
            } else {
                self.get("from")
            }
        });
        let ym = if source.len() >= 7 {
            &source[..7]
        } else {
            "2026-01"
        };
        let month = if exact.is_some() {
            shift_month(ym, delta)
        } else {
            ym.into()
        };
        let Some((from, to)) = month_range(&month) else {
            return self.clone();
        };
        self.patch("from", &from).patch("to", &to)
    }
    pub(crate) fn label(&self) -> String {
        if let Some(month) = self.exact_month() {
            let (year, month) = month.split_once('-').unwrap_or(("", ""));
            format!("{year}年{}月", month.parse::<u32>().unwrap_or(0))
        } else if self.get("from").is_empty() && self.get("to").is_empty() {
            "すべての期間".into()
        } else {
            format!(
                "{} 〜 {}",
                if self.get("from").is_empty() {
                    "最初"
                } else {
                    self.get("from")
                },
                if self.get("to").is_empty() {
                    "最後"
                } else {
                    self.get("to")
                }
            )
        }
    }
    pub(crate) fn extra_count(&self) -> usize {
        ["from", "to", "account", "wallet", "kind", "min", "max"]
            .iter()
            .filter(|key| !self.get(key).is_empty())
            .count()
            + usize::from(self.get("state") != "all")
    }
}
fn is_iso(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}
pub(crate) fn month_range(value: &str) -> Option<(String, String)> {
    let (year, month) = value.split_once('-')?;
    let year = year.parse::<i32>().ok()?;
    let month = month.parse::<i32>().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    let days = match month {
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    Some((
        format!("{year:04}-{month:02}-01"),
        format!("{year:04}-{month:02}-{days}"),
    ))
}
fn shift_month(value: &str, delta: i32) -> String {
    let (year, month) = value.split_once('-').unwrap_or(("2026", "1"));
    let year = year.parse::<i32>().unwrap_or(2026);
    let month = month.parse::<i32>().unwrap_or(1);
    let absolute = year * 12 + month - 1 + delta;
    format!(
        "{:04}-{:02}",
        absolute.div_euclid(12),
        absolute.rem_euclid(12) + 1
    )
}
pub(crate) fn side_summary(lines: &[Value], side: &str) -> String {
    let values: Vec<&Value> = lines.iter().filter(|line| line["side"] == side).collect();
    let Some(first) = values.first() else {
        return String::new();
    };
    let name = first["account_name"]
        .as_str()
        .or_else(|| first["account"].as_str())
        .unwrap_or_default();
    if values.len() > 1 {
        format!("{name} ほか{}行", values.len() - 1)
    } else {
        name.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn filters_discard_invalid_parameters_and_keep_defaults_out_of_address() {
        let data = BTreeMap::from([
            ("from", "2026/10/01"),
            ("to", "2026-10-31"),
            ("state", "garbage"),
            ("kind", "expense"),
            ("min", "1e3"),
            ("max", "1000"),
            ("page", "2.5"),
        ]);
        let filters = Filters::read(|key| data.get(key).copied().unwrap_or_default().into());
        assert_eq!(filters.page, 1);
        assert_eq!(filters.get("from"), "");
        assert_eq!(filters.get("state"), "all");
        assert_eq!(filters.get("min"), "");
        assert_eq!(
            filters.query(),
            vec![
                ("to".into(), "2026-10-31".into()),
                ("kind".into(), "expense".into()),
                ("max".into(), "1000".into())
            ]
        );
        assert!(filters.api().contains(&("offset".into(), "0".into())));
    }
    #[test]
    fn month_shifts_use_the_current_month_first_then_cross_year_and_leap_days() {
        let f = Filters::read(|_| String::new());
        let next = f.shift(1, "2026-10-15");
        assert_eq!(next.get("from"), "2026-10-01");
        assert_eq!(next.get("to"), "2026-10-31");
        assert_eq!(next.shift(1, "2026-10-15").get("from"), "2026-11-01");
        let feb = f
            .patch("from", "2024-01-01")
            .patch("to", "2024-01-31")
            .shift(1, "2024-01-01");
        assert_eq!(feb.get("to"), "2024-02-29");
        assert_eq!(
            f.patch("from", "2026-01-01")
                .patch("to", "2026-01-31")
                .shift(-1, "2026-01-01")
                .get("from"),
            "2025-12-01"
        );
    }
    #[test]
    fn summaries_keep_account_names_and_compound_line_count() {
        assert_eq!(
            side_summary(
                &[
                    json!({"side":"debit","account_name":"通信費"}),
                    json!({"side":"debit","account_name":"事業主貸"})
                ],
                "debit"
            ),
            "通信費 ほか1行"
        );
        assert_eq!(side_summary(&[], "credit"), "");
    }
}
