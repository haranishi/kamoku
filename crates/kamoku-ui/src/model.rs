// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Deserialize;

pub(crate) const PAGE_SIZE: u32 = 50;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Filters {
    pub q: String,
    pub from: String,
    pub to: String,
    pub state: String,
    pub page: u32,
}

impl Filters {
    pub fn read(mut get: impl FnMut(&str) -> Option<String>) -> Self {
        let state = get("state").unwrap_or_default();
        Self {
            q: get("q").unwrap_or_default().trim().to_owned(),
            from: get("from")
                .filter(|date| valid_date(date))
                .unwrap_or_default(),
            to: get("to")
                .filter(|date| valid_date(date))
                .unwrap_or_default(),
            state: if matches!(state.as_str(), "active" | "voided") {
                state
            } else {
                "all".into()
            },
            page: get("page")
                .and_then(|page| page.parse::<u32>().ok())
                .filter(|page| *page > 0)
                .unwrap_or(1),
        }
    }

    pub fn offset(&self) -> u64 {
        u64::from(self.page.saturating_sub(1)) * u64::from(PAGE_SIZE)
    }

    pub fn pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        for (name, value) in [("q", &self.q), ("from", &self.from), ("to", &self.to)] {
            if !value.is_empty() {
                pairs.push((name, value.clone()));
            }
        }
        if self.state != "all" {
            pairs.push(("state", self.state.clone()));
        }
        if self.page > 1 {
            pairs.push(("page", self.page.to_string()));
        }
        pairs
    }
}

fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year = date[..4].parse::<u32>().unwrap_or(0);
    let month = date[5..7].parse::<u32>().unwrap_or(0);
    let day = date[8..].parse::<u32>().unwrap_or(0);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => 0,
    };
    year > 0 && day > 0 && day <= days
}

#[derive(Debug, Deserialize)]
pub(crate) struct EntryPage {
    pub items: Vec<Entry>,
    pub total: u64,
    pub sum_amount: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Entry {
    pub id: String,
    pub no: u64,
    pub date: String,
    pub description: String,
    pub partner: String,
    pub total: i64,
    pub state: String,
    pub lines: Vec<Line>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Line {
    pub account_name: String,
    pub side: String,
}

pub(crate) fn yen(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut result = String::new();
    if value < 0 {
        result.push('−');
    }
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyDecision {
    Allow,
    Ignore,
    Block,
}

pub(crate) fn key_decision(
    composing: bool,
    code: u32,
    enter: bool,
    repeat: bool,
    since_composition_ms: f64,
) -> KeyDecision {
    if composing {
        KeyDecision::Ignore
    } else if code == 229 || (enter && since_composition_ms < 50.0) {
        KeyDecision::Block
    } else if enter && repeat {
        KeyDecision::Ignore
    } else {
        KeyDecision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filters(pairs: &[(&str, &str)]) -> Filters {
        Filters::read(|key| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn filters_normalize_untrusted_history_values_without_overflow() {
        let value = filters(&[
            ("q", "  通信  "),
            ("state", "invalid"),
            ("page", "0"),
            ("from", "2026-02-29"),
            ("to", "2024-02-29"),
        ]);
        assert_eq!(value.q, "通信");
        assert_eq!(value.state, "all");
        assert_eq!(value.page, 1);
        assert_eq!(value.from, "");
        assert_eq!(value.to, "2024-02-29");
        assert_eq!(value.offset(), 0);
        assert_eq!(filters(&[("page", "4294967295")]).offset(), 214_748_364_700);
        assert_eq!(filters(&[("page", "999999999999999999")]).page, 1);
        assert_eq!(filters(&[("from", "２０２６-10-01")]).from, "");
    }

    #[test]
    fn query_round_trip_retains_meaning_and_omits_defaults() {
        let before = filters(&[
            ("q", "A&B + 交通費"),
            ("state", "voided"),
            ("page", "3"),
            ("from", "2026-10-01"),
        ]);
        let pairs = before.pairs();
        let after = Filters::read(|key| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.clone())
        });
        assert_eq!(before, after);
        assert_eq!(before.offset(), 100);
        assert_eq!(filters(&[]).pairs(), []);
    }

    #[test]
    fn integer_yen_formatting_keeps_every_digit_including_minimum() {
        assert_eq!(yen(0), "0");
        assert_eq!(yen(1_234_567), "1,234,567");
        assert_eq!(yen(i64::MIN), "−9,223,372,036,854,775,808");
    }

    #[test]
    fn composition_enter_does_not_trigger_search_or_native_submit() {
        assert_eq!(
            key_decision(true, 13, true, false, 100.0),
            KeyDecision::Ignore
        );
        assert_eq!(
            key_decision(false, 229, true, false, 100.0),
            KeyDecision::Block
        );
        assert_eq!(
            key_decision(false, 13, true, false, 49.0),
            KeyDecision::Block
        );
        assert_eq!(
            key_decision(false, 13, true, true, 100.0),
            KeyDecision::Ignore
        );
        assert_eq!(
            key_decision(false, 13, true, false, 50.0),
            KeyDecision::Allow
        );
    }
}
