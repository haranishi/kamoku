// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 欄の値の読み取り（日付・金額）と、摘要の正規化。

use time::{Date, Month};

/// 全角の英数字・記号（U+FF01〜U+FF5E）を半角に、全角の空白を半角の空白にします。
fn halfwidth(c: char) -> char {
    match c {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(u32::from(c) - 0xFEE0).unwrap_or(c),
        '\u{3000}' => ' ',
        other => other,
    }
}

/// 摘要の正規化（`docs/spec/api-inbox.md`）。全角の英数字と記号を半角にし、前後の空白を除き、
/// 連続する空白（全角・タブ・改行を含む）を1つの空白にします。半角カナは変えません。
#[must_use]
pub fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text
        .chars()
        .map(halfwidth)
        .collect::<String>()
        .split_whitespace()
    {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// 日付を読みます。`2026/10/1` `2026-10-01` `2026.10.1` `20261001` `2026年10月1日`
/// `R8.10.1` `令和8年10月1日` `令和元年5月1日` `H31.4.30`（全角の数字も可）。
/// 実在しない日と、元号の期間の外の日（令和元年4月30日など）は `None`。
#[must_use]
pub fn parse_date(text: &str) -> Option<Date> {
    let s: String = text.trim().chars().map(halfwidth).collect();
    let s = s.trim();
    if let Some((era, rest)) = era_prefix(s) {
        let (year, month, day) = split_ymd(rest, true)?;
        let date = ymd(era.offset.checked_add(year)?, month, day)?;
        return (era.first <= date && era.last.is_none_or(|last| date <= last)).then_some(date);
    }
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i32>().ok();
        return ymd(n(0..4)?, n(4..6)?, n(6..8)?);
    }
    let (year, month, day) = split_ymd(s, false)?;
    if !(1000..=9999).contains(&year) {
        return None;
    }
    ymd(year, month, day)
}

struct Era {
    offset: i32,
    first: Date,
    last: Option<Date>,
}
fn era_prefix(s: &str) -> Option<(Era, &str)> {
    let reiwa = || Era {
        offset: 2018,
        first: ymd(2019, 5, 1).unwrap_or(Date::MIN),
        last: None,
    };
    let heisei = || Era {
        offset: 1988,
        first: ymd(1989, 1, 8).unwrap_or(Date::MIN),
        last: Some(ymd(2019, 4, 30).unwrap_or(Date::MIN)),
    };
    for (prefix, era) in [
        ("令和", reiwa()),
        ("R", reiwa()),
        ("r", reiwa()),
        ("平成", heisei()),
        ("H", heisei()),
        ("h", heisei()),
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return Some((era, rest));
        }
    }
    None
}
/// 「年 月 日」を `/` `-` `.`（同じ区切り）か「年・月・日」で分けます。`era` なら年に「元」を許します。
fn split_ymd(s: &str, era: bool) -> Option<(i32, i32, i32)> {
    let number = |part: &str| -> Option<i32> {
        if era && part == "元" {
            return Some(1);
        }
        if part.is_empty() || part.len() > 4 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    if let Some(body) = s.strip_suffix('日') {
        let (year, rest) = body.split_once('年')?;
        let (month, day) = rest.split_once('月')?;
        return Some((number(year)?, number(month)?, number(day)?));
    }
    for sep in ['/', '-', '.'] {
        let mut parts = s.split(sep);
        if let (Some(y), Some(m), Some(d), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        {
            return Some((number(y)?, number(m)?, number(d)?));
        }
    }
    None
}
fn ymd(year: i32, month: i32, day: i32) -> Option<Date> {
    let month = Month::try_from(u8::try_from(month).ok()?).ok()?;
    Date::from_calendar_date(year, month, u8::try_from(day).ok()?).ok()
}

/// 金額を読めなかった理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountError {
    /// 小数点がある（円未満は扱わない）。
    Decimal,
    /// 数として読めない。
    NotNumber,
    /// 大きすぎる。
    TooLarge,
}
impl AmountError {
    /// 行に付ける理由の日本語。
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Decimal => "金額に小数点があります。円未満の金額は取り込めません。",
            Self::NotNumber => "金額を数として読めません。",
            Self::TooLarge => "金額が大きすぎます（9,999,999,999,999円まで）。",
        }
    }
}

/// 金額を読みます。カンマ、`¥` `￥` `\`（シフトJISの円記号）`円`、全角の数字、空白を除き、
/// 先頭の `△` `▲` `−`（U+2212）`-`、`(1,650)`、末尾の `-` を負として読みます。空の欄は `Ok(None)`。
///
/// # Errors
/// 小数点がある、数として読めない、大きすぎるときに返します。
pub fn parse_amount(text: &str) -> Result<Option<i64>, AmountError> {
    let s: String = text
        .chars()
        .map(halfwidth)
        .filter(|c| !matches!(c, ',' | ' ' | '¥' | '￥' | '\\' | '円'))
        .collect();
    if s.is_empty() {
        return Ok(None);
    }
    let (negative, digits) =
        if let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
            (true, inner)
        } else if let Some(rest) = s.strip_prefix(['△', '▲', '−', '-']) {
            (true, rest)
        } else if let Some(rest) = s.strip_suffix('-') {
            (true, rest)
        } else {
            (false, s.strip_prefix('+').unwrap_or(&s))
        };
    if digits.contains('.') {
        return Err(AmountError::Decimal);
    }
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AmountError::NotNumber);
    }
    let value: i64 = digits.parse().map_err(|_| AmountError::TooLarge)?;
    Ok(Some(if negative { -value } else { value }))
}
