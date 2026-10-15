// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Calendar-only report periods; no tax or accounting calculation.

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Period {
    pub mode: String,
    pub year: i32,
    pub month: String,
    pub from: String,
    pub to: String,
    pub as_of: String,
}

pub(crate) fn days(year: i32, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => 0,
    }
}
pub(crate) fn valid_date(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return false;
    }
    let y = value[..4].parse::<i32>().unwrap_or(-1);
    let m = value[5..7].parse().unwrap_or(0);
    let d = value[8..].parse::<u32>().unwrap_or(0);
    y >= 0 && d > 0 && d <= days(y, m)
}
pub(crate) fn valid_month(value: &str) -> bool {
    value.len() == 7 && valid_date(&format!("{value}-01"))
}
pub(crate) fn shift_month(value: &str, step: i32) -> String {
    if !valid_month(value) {
        return value.into();
    }
    let year = value[..4].parse::<i32>().unwrap_or(0);
    let month = value[5..].parse::<i32>().unwrap_or(1);
    let total = (year * 12 + month - 1 + step).clamp(0, 119_999);
    format!("{:04}-{:02}", total / 12, total % 12 + 1)
}
pub(crate) fn range_error(from: &str, to: &str) -> Option<String> {
    if !valid_date(from) || !valid_date(to) {
        return Some("期間の始めと終わりを、日付で入れてください。".into());
    }
    if from[..4] != to[..4] {
        return Some(format!(
            "期間は同じ年の中で選んでください（いまは{}年と{}年をまたいでいます）。帳票は1年ずつ作ります。",
            &from[..4],
            &to[..4]
        ));
    }
    if from > to {
        return Some("期間の始めが、終わりより後になっています。".into());
    }
    None
}
impl Period {
    pub fn year(year: i32) -> Self {
        let year = year.clamp(0, 9999);
        Self {
            mode: "year".into(),
            year,
            month: String::new(),
            from: format!("{year:04}-01-01"),
            to: format!("{year:04}-12-31"),
            as_of: String::new(),
        }
    }
    pub fn month(value: &str) -> Self {
        let year = value[..4].parse().unwrap_or(0);
        let month = value[5..].parse().unwrap_or(1);
        Self {
            mode: "month".into(),
            month: value.into(),
            from: format!("{value}-01"),
            to: format!("{value}-{:02}", days(year, month)),
            ..Self::year(year)
        }
    }
    pub fn date(value: &str) -> Self {
        Self {
            mode: "date".into(),
            as_of: value.into(),
            ..Self::year(value[..4].parse().unwrap_or(0))
        }
    }
    pub fn read(
        mut get: impl FnMut(&str) -> String,
        today: &str,
        date: bool,
        year_only: bool,
    ) -> Self {
        if date {
            let value = get("as_of");
            return Self::date(if valid_date(&value) { &value } else { today });
        }
        let month = get("month");
        if !year_only && valid_month(&month) {
            return Self::month(&month);
        }
        let (from, to) = (get("from"), get("to"));
        if !year_only && range_error(&from, &to).is_none() {
            return Self {
                mode: "range".into(),
                from: from.clone(),
                to,
                ..Self::year(from[..4].parse().unwrap_or(0))
            };
        }
        let value = get("year");
        let year = if value.len() == 4 && value.bytes().all(|b| b.is_ascii_digit()) {
            value.parse().unwrap_or(0)
        } else {
            today[..4].parse().unwrap_or(0)
        };
        Self::year(year)
    }
    pub fn query(&self) -> Vec<(String, String)> {
        match self.mode.as_str() {
            "date" => vec![("as_of".into(), self.as_of.clone())],
            "month" => vec![("month".into(), self.month.clone())],
            "range" => vec![
                ("from".into(), self.from.clone()),
                ("to".into(), self.to.clone()),
            ],
            _ => vec![("year".into(), format!("{:04}", self.year))],
        }
    }
    pub fn label(&self) -> String {
        match self.mode.as_str() {
            "date" => format!("{} 時点", self.as_of),
            "month" => format!(
                "{}年{}月",
                self.year,
                self.month[5..].parse::<u32>().unwrap_or(1)
            ),
            "range" => format!("{} 〜 {}", self.from, self.to),
            _ => format!("{}年", self.year),
        }
    }
    pub fn shift(&self, step: i32) -> Self {
        match self.mode.as_str() {
            "year" => Self::year(self.year + step),
            "month" => Self::month(&shift_month(&self.month, step)),
            "date" => {
                let month = Self::month(&shift_month(&self.as_of[..7], step));
                Self::date(&month.to)
            }
            _ => self.clone(),
        }
    }
    pub fn carry(&self, today: &str, date: bool, year_only: bool) -> Self {
        if date {
            if self.mode == "date" {
                self.clone()
            } else {
                Self::date(if self.to.as_str() > today && self.from.as_str() <= today {
                    today
                } else {
                    &self.to
                })
            }
        } else if self.mode == "date" || year_only {
            Self::year(self.year)
        } else {
            self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_boundaries_and_malformed_history() {
        assert!(valid_date("2000-02-29"));
        assert!(!valid_date("1900-02-29"));
        assert!(!valid_date("2026-02-29"));
        assert!(!valid_date("２０２６-10-01"));
        assert!(!valid_month("2026-13"));
        assert_eq!(shift_month("2026-01", -1), "2025-12");
        assert_eq!(Period::date("2024-01-30").shift(1).as_of, "2024-02-29");
        assert_eq!(
            Period::read(|_| "garbage".into(), "2026-10-15", false, false),
            Period::year(2026)
        );
    }
    #[test]
    fn range_and_tab_carry_preserve_the_effective_period() {
        assert!(range_error("2025-12-31", "2026-01-01").is_some());
        assert!(range_error("2026-10-02", "2026-10-01").is_some());
        let p = Period::month("2026-10");
        assert_eq!(p.carry("2026-10-15", true, false).as_of, "2026-10-15");
        assert_eq!(p.carry("2026-10-15", false, true), Period::year(2026));
        assert_eq!(p.shift(-10).month, "2025-12");
        let pairs = p.query();
        assert_eq!(
            Period::read(
                |k| pairs
                    .iter()
                    .find(|(name, _)| name == k)
                    .map_or(String::new(), |(_, v)| v.clone()),
                "2026-10-15",
                false,
                false
            ),
            p
        );
    }
    fn read_pairs(pairs: &[(&str, &str)], date: bool, year_only: bool) -> Period {
        Period::read(
            |key| {
                pairs
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map_or(String::new(), |(_, value)| (*value).into())
            },
            "2026-10-14",
            date,
            year_only,
        )
    }
    #[test]
    fn defaults_month_calendar_and_custom_range_match_saved_urls() {
        let current = read_pairs(&[], false, false);
        assert_eq!(current, Period::year(2026));
        assert_eq!(
            read_pairs(&[("year", "2025")], false, false),
            Period::year(2025)
        );
        let month = read_pairs(&[("month", "2026-02")], false, false);
        assert_eq!((&*month.from, &*month.to), ("2026-02-01", "2026-02-28"));
        let range = read_pairs(
            &[("from", "2026-04-01"), ("to", "2026-06-30")],
            false,
            false,
        );
        assert_eq!(range.mode, "range");
        assert_eq!(
            range.query(),
            vec![
                ("from".into(), "2026-04-01".into()),
                ("to".into(), "2026-06-30".into())
            ]
        );
    }
    #[test]
    fn date_mode_defaults_and_year_only_ignore_inapplicable_history() {
        assert_eq!(read_pairs(&[], true, false).as_of, "2026-10-14");
        assert_eq!(
            read_pairs(&[("as_of", "2025-12-31")], true, false).as_of,
            "2025-12-31"
        );
        assert_eq!(
            read_pairs(&[("as_of", "2026-02-30")], true, false).as_of,
            "2026-10-14"
        );
        assert_eq!(
            read_pairs(&[("month", "2025-03")], false, true),
            Period::year(2026)
        );
        assert_eq!(
            read_pairs(&[("year", "2025")], false, true),
            Period::year(2025)
        );
    }
    #[test]
    fn invalid_ranges_are_never_sent_and_labels_remain_exact() {
        assert!(
            range_error("2025-12-01", "2026-01-31")
                .unwrap()
                .contains("同じ年の中")
        );
        assert_eq!(
            read_pairs(
                &[("from", "2025-12-01"), ("to", "2026-01-31")],
                false,
                false
            ),
            Period::year(2026)
        );
        assert!(
            range_error("2026-06-01", "2026-04-01")
                .unwrap()
                .contains("後になっています")
        );
        assert_eq!(range_error("2026-04-01", "2026-06-30"), None);
        for (p, label) in [
            (Period::year(2026), "2026年"),
            (Period::month("2026-10"), "2026年10月"),
            (
                read_pairs(
                    &[("from", "2026-04-01"), ("to", "2026-06-30")],
                    false,
                    false,
                ),
                "2026-04-01 〜 2026-06-30",
            ),
            (Period::date("2026-10-14"), "2026-10-14 時点"),
        ] {
            assert_eq!(p.label(), label);
        }
    }
    #[test]
    fn shifting_and_tab_carry_retain_year_and_calendar_end() {
        assert_eq!(Period::year(2026).shift(-1), Period::year(2025));
        let previous = Period::month("2026-01").shift(-1);
        assert_eq!(
            (&*previous.from, &*previous.to),
            ("2025-12-01", "2025-12-31")
        );
        assert_eq!(Period::date("2026-03-15").shift(-1).as_of, "2026-02-28");
        assert_eq!(
            Period::year(2026).carry("2026-10-14", true, false).as_of,
            "2026-10-14"
        );
        assert_eq!(
            Period::year(2025).carry("2026-10-14", true, false).as_of,
            "2025-12-31"
        );
        assert_eq!(
            Period::month("2026-03")
                .carry("2026-10-14", true, false)
                .as_of,
            "2026-03-31"
        );
        assert_eq!(
            Period::date("2025-06-30")
                .carry("2026-10-14", false, false)
                .query(),
            vec![("year".into(), "2025".into())]
        );
    }
}
