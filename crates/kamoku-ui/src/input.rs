// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Browser-independent input rules, shared by every authored-Rust screen.
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use unicode_normalization::UnicodeNormalization;

pub(crate) const MAX_AMOUNT: i64 = 9_999_999_999_999;

pub(crate) fn normalize_width(text: &str) -> String {
    text.nfkc()
        .map(|c| match c {
            '−' | '‐' | '‑' | '‒' | '–' | '—' | '―' | 'ー' | '﹣' => '-',
            '×' => '*',
            '÷' => '/',
            c => c,
        })
        .collect()
}

pub(crate) fn normalize_search(text: &str) -> String {
    text.nfkc()
        .flat_map(char::to_lowercase)
        .filter(|c| !c.is_whitespace())
        .map(|c| {
            if ('ァ'..='ヶ').contains(&c) {
                char::from_u32(u32::from(c) - 0x60).expect("kana scalar")
            } else {
                c
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Amount {
    pub value: i64,
    pub rounded: bool,
    pub is_expr: bool,
}

struct Parser {
    chars: Vec<char>,
    at: usize,
    depth: usize,
    rounded: bool,
}
impl Parser {
    fn expression(&mut self) -> Result<BigInt, String> {
        let mut n = self.term()?;
        while let Some(op @ ('+' | '-')) = self.chars.get(self.at).copied() {
            self.at += 1;
            let right = self.term()?;
            if op == '+' {
                n += right;
            } else {
                n -= right;
            }
        }
        Ok(n)
    }
    fn term(&mut self) -> Result<BigInt, String> {
        let mut n = self.factor()?;
        while let Some(op @ ('*' | '/')) = self.chars.get(self.at).copied() {
            self.at += 1;
            let right = self.factor()?;
            if op == '*' {
                n *= right;
            } else {
                if right.is_zero() {
                    return Err("0では割れません。式を確かめてください。".into());
                }
                self.rounded |= !(&n % &right).is_zero();
                n /= right;
            }
        }
        Ok(n)
    }
    fn factor(&mut self) -> Result<BigInt, String> {
        self.depth += 1;
        if self.depth > 21 {
            return Err("かっこが深すぎます。式を短くしてください。".into());
        }
        let result = match self.chars.get(self.at).copied() {
            Some(op @ ('+' | '-')) => {
                self.at += 1;
                self.factor().map(|v| if op == '-' { -v } else { v })
            }
            Some('(') => {
                self.at += 1;
                let n = self.expression()?;
                if self.chars.get(self.at) != Some(&')') {
                    return Err("かっこが閉じていません。「)」を足してください。".into());
                }
                self.at += 1;
                Ok(n)
            }
            Some('0'..='9') => {
                let start = self.at;
                while self.chars.get(self.at).is_some_and(char::is_ascii_digit) {
                    self.at += 1;
                }
                let digits: String = self.chars[start..self.at].iter().collect();
                digits.parse().map_err(|_| syntax())
            }
            _ => Err(syntax()),
        };
        self.depth -= 1;
        result
    }
}
fn syntax() -> String {
    "式の形が正しくありません（例: 1000+650）。".into()
}

pub(crate) fn parse_amount_details(text: &str) -> Result<Amount, String> {
    let marked: String = normalize_width(text)
        .chars()
        .filter(|c| !c.is_whitespace() && !['円', '¥', '\\'].contains(c))
        .collect();
    for digits in marked.split(|c: char| !c.is_ascii_digit() && c != ',') {
        if digits.contains(',') {
            let groups: Vec<_> = digits.split(',').collect();
            if groups[0].is_empty()
                || groups[0].len() > 3
                || groups[1..].iter().any(|g| g.len() != 3)
            {
                return Err("桁区切りの位置が違います。3桁ごとに入れるか、カンマを外してください（例: 1,234）。".into());
            }
        }
    }
    let source = marked.replace(',', "");
    if source.is_empty() {
        return Err("金額を入れてください。".into());
    }
    if source.chars().count() > 100 {
        return Err("式が長すぎます。100文字以内にしてください。".into());
    }
    for c in source.chars() {
        if !c.is_ascii_digit() && !"+-*/()".contains(c) {
            return Err(if c == '.' {
                "小数は使えません。円の整数で入れてください（例: 1650）。".into()
            } else {
                format!("使えない文字「{c}」があります。数字と + - * / ( ) だけで入れてください。")
            });
        }
    }
    let mut parser = Parser {
        chars: source.chars().collect(),
        at: 0,
        depth: 0,
        rounded: false,
    };
    let n = parser.expression()?;
    if parser.at != parser.chars.len() {
        return Err(syntax());
    }
    if n < BigInt::from(1) {
        return Err("金額は1円以上にしてください。".into());
    }
    if n > BigInt::from(MAX_AMOUNT) {
        return Err("金額は9,999,999,999,999円以下にしてください。".into());
    }
    Ok(Amount {
        value: n.to_i64().ok_or_else(syntax)?,
        rounded: parser.rounded,
        is_expr: source.contains(['+', '-', '*', '/', '(', ')']),
    })
}
pub(crate) fn parse_amount(text: &str) -> Result<i64, String> {
    parse_amount_details(text).map(|v| v.value)
}
pub(crate) fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        4 | 6 | 9 | 11 => 30,
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    }
}
pub(crate) fn real_date(y: i32, m: u32, d: u32) -> bool {
    (1..=9999).contains(&y) && (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}
pub(crate) fn date_parts(iso: &str) -> Option<(i32, u32, u32)> {
    let mut parts = iso.split('-');
    let (y, m, d) = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    (parts.next().is_none() && real_date(y, m, d)).then_some((y, m, d))
}
pub(crate) fn add_days(iso: &str, mut delta: i32) -> String {
    let Some((mut y, mut m, mut d)) = date_parts(iso) else {
        return iso.into();
    };
    while delta > 0 {
        d += 1;
        if d > days_in_month(y, m) {
            d = 1;
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
        delta -= 1;
    }
    while delta < 0 {
        if d == 1 {
            if m == 1 {
                m = 12;
                y -= 1;
            } else {
                m -= 1;
            }
            d = days_in_month(y, m);
        } else {
            d -= 1;
        }
        delta += 1;
    }
    format!("{y:04}-{m:02}-{d:02}")
}
pub(crate) fn parse_date(text: &str, today: &str) -> Result<String, String> {
    let src: String = normalize_width(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    if src.is_empty() {
        return Err("日付を入れてください。".into());
    }
    if ["t", "今日"].contains(&src.as_str()) {
        return Ok(today.into());
    }
    if ["y", "昨日"].contains(&src.as_str()) {
        return Ok(add_days(today, -1));
    }
    let base = today
        .get(..4)
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(2026);
    let date = src.strip_suffix('日').unwrap_or(&src);
    let normalized = date.replace(['年', '月', '/', '.'], "-");
    let parts: Vec<_> = normalized.split('-').collect();
    let digits = |s: &str, min: usize, max: usize| {
        (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
    };
    let separators: Vec<_> = date.chars().filter(|c| !c.is_ascii_digit()).collect();
    let candidate = if parts.len() == 3
        && digits(parts[0], 4, 4)
        && digits(parts[1], 1, 2)
        && digits(parts[2], 1, 2)
        && separators.len() == 2
        && "/-.年".contains(separators[0])
        && "/-.月".contains(separators[1])
    {
        Some((
            parts[0].parse::<i32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ))
    } else if parts.len() == 2
        && digits(parts[0], 1, 2)
        && digits(parts[1], 1, 2)
        && separators.len() == 1
        && "/-.月".contains(separators[0])
    {
        Some((Ok(base), parts[0].parse::<u32>(), parts[1].parse::<u32>()))
    } else if digits(&src, 8, 8) {
        Some((
            src[..4].parse::<i32>(),
            src[4..6].parse::<u32>(),
            src[6..].parse::<u32>(),
        ))
    } else if digits(&src, 4, 4) {
        Some((Ok(base), src[..2].parse::<u32>(), src[2..].parse::<u32>()))
    } else {
        None
    };
    if let Some((Ok(y), Ok(m), Ok(d))) = candidate {
        if !real_date(y, m, d) {
            return Err(format!(
                "{y}年{m}月{d}日はありません。日付を確かめてください。"
            ));
        }
        return Ok(format!("{y:04}-{m:02}-{d:02}"));
    }
    Err("日付は「1005」「10/5」「2026-10-05」の形で入れてください。「t」で今日になります。".into())
}
pub(crate) fn number(value: i64) -> String {
    let raw = value.unsigned_abs().to_string();
    let mut out = String::new();
    if value < 0 {
        out.push('-');
    }
    for (i, c) in raw.chars().enumerate() {
        if i > 0 && (raw.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
pub(crate) fn year_month(value: &str) -> String {
    value.get(..7).unwrap_or(value).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn amount_cases() {
        // AC-02.3
        for (s, n) in [
            ("1,650", 1650),
            ("１６５０", 1650),
            ("１０００＋６５０", 1650),
            ("¥1,650", 1650),
            ("￥１，６５０円", 1650),
            (" 1 650 ", 1650),
            ("\\1650", 1650),
            ("12,345,678", 12_345_678),
            ("1,000+2,500", 3500),
            ("(1,200)*2", 2400),
            ("1200+300*2", 1800),
            ("10000-2000-3000", 5000),
            ("-500+1000", 500),
            ("2×3000", 6000),
            ("9000÷3", 3000),
            ("1000－650", 350),
            ("1000−650", 350),
            ("9007199254740993-9007199254740992", 1),
            ("(9999999999999*9999+1)/9999", MAX_AMOUNT),
            ("1000+650", 1650),
            ("(1200+300)*2", 3000),
            ("1000ー650", 350),
            ("9,999,999,999,999", MAX_AMOUNT),
            (
                "99999999999999999999999999999/99999999999999999999999999999",
                1,
            ),
        ] {
            assert_eq!(parse_amount(s), Ok(n), "{s}");
        }
        assert!(parse_amount_details("100/3").unwrap().rounded);
        assert!(!parse_amount_details("99/3").unwrap().rounded);
        assert!(parse_amount_details("1200+300").unwrap().is_expr);
        assert!(!parse_amount_details("1500").unwrap().is_expr);
        for s in [
            "",
            "0",
            "-1",
            "1,2,3",
            "1,0000",
            "10,00",
            ",100",
            "100,",
            "1,000,00",
            "1-1",
            "100-200",
            "abc",
            "alert(1)",
            "1+",
            "((1)",
            "1.5",
            "1/0",
            "99999999999999",
            "(1+2",
            "1a",
            "1**2",
        ] {
            assert!(parse_amount(s).is_err(), "{s}");
        }
        assert!(
            parse_amount(&"1".repeat(101))
                .unwrap_err()
                .contains("長すぎ")
        );
        assert!(parse_amount(&format!("{}1{}", "(".repeat(30), ")".repeat(30))).is_err());
        assert!(parse_amount(&format!("{}1", "-".repeat(40))).is_err());
    }
    #[test]
    fn date_cases() {
        // AC-02.4
        for s in [
            "1005",
            "10/5",
            "10-5",
            "2026/10/5",
            "2026-10-05",
            "20261005",
            "１０／５",
            "１００５",
            "2026年10月5日",
        ] {
            assert_eq!(parse_date(s, "2026-10-15"), Ok("2026-10-05".into()), "{s}");
        }
        assert_eq!(parse_date("y", "2024-03-01"), Ok("2024-02-29".into()));
        assert_eq!(parse_date("Ｔ", "2026-10-14"), Ok("2026-10-14".into()));
        assert_eq!(parse_date("y", "2026-01-01"), Ok("2025-12-31".into()));
        assert_eq!(
            parse_date("2028-02-29", "2026-10-14"),
            Ok("2028-02-29".into())
        );
        for s in [
            "2026-02-29",
            "2026-13-01",
            "1/32",
            "",
            "2026-1-1-1",
            "0230",
            "13/1",
            "0/5",
            "1032",
            "105",
            "abc",
            "26/1/1",
            "+2026/1/1",
            "1年1",
            "2026月1月1",
            "1/1日日",
        ] {
            assert!(parse_date(s, "2026-10-15").is_err());
        }
    }
    #[test]
    fn kana_width_and_sort_keys() {
        assert_eq!(normalize_search("ﾂｳｼﾝ　ＡＢＣ"), "つうしんabc");
        assert_eq!(normalize_search("ツウシン"), normalize_search("つうしん"));
    }
    #[test]
    fn precise_grouping() {
        for (value, text) in [
            (0, "0"),
            (999, "999"),
            (1000, "1,000"),
            (11_000, "11,000"),
            (-1200, "-1,200"),
            (MAX_AMOUNT, "9,999,999,999,999"),
        ] {
            assert_eq!(number(value), text);
        }
        assert_eq!(number(i64::MIN), "-9,223,372,036,854,775,808");
        assert_eq!(number(MAX_AMOUNT), "9,999,999,999,999");
    }
}
