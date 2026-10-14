// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSONへの書き出しの小さな部品。`time` の serde 機能を足さずに、日付を `YYYY-MM-DD` で扱うため。

use kamoku_ledger::{Side, Yen};
use serde::{Deserialize, Deserializer, Serializer, de::Error};
use time::{Date, Month};

// serde の serialize_with は参照を渡すので、値渡しにできない。
#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn yen<S: Serializer>(value: &Yen, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_i64(value.get())
}
#[allow(clippy::ref_option)]
pub(crate) fn months<S: Serializer>(value: &Option<[Yen; 12]>, s: S) -> Result<S::Ok, S::Error> {
    match value {
        None => s.serialize_none(),
        Some(values) => s.collect_seq(values.iter().map(|v| v.get())),
    }
}
pub(crate) const fn side_str(side: Side) -> &'static str {
    match side {
        Side::Debit => "debit",
        Side::Credit => "credit",
    }
}
/// 日付を `YYYY-MM-DD` にします。
#[must_use]
pub fn format_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}
/// `YYYY-MM-DD`（ゼロ詰め）だけを読みます。実在しない日は `None`。
#[must_use]
pub fn parse_date(text: &str) -> Option<Date> {
    let bytes = text.as_bytes();
    if bytes.len() != 10
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return None;
    }
    let year: i32 = text.get(0..4)?.parse().ok()?;
    let month: u8 = text.get(5..7)?.parse().ok()?;
    let day: u8 = text.get(8..10)?.parse().ok()?;
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

pub(crate) mod date {
    use super::{Date, Deserialize, Deserializer, Error, Serializer, format_date, parse_date};
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(crate) fn serialize<S: Serializer>(value: &Date, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format_date(*value))
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        let text = String::deserialize(d)?;
        parse_date(&text)
            .ok_or_else(|| D::Error::custom(format!("日付「{text}」を読めません（YYYY-MM-DD）")))
    }
}

pub(crate) mod side {
    use super::{Deserialize, Deserializer, Error, Serializer, Side, side_str};
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(crate) fn serialize<S: Serializer>(value: &Side, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(side_str(*value))
    }
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Side, D::Error> {
        match String::deserialize(d)?.as_str() {
            "debit" => Ok(Side::Debit),
            "credit" => Ok(Side::Credit),
            other => Err(D::Error::custom(format!(
                "貸借「{other}」を読めません（debit か credit）"
            ))),
        }
    }
}

/// 金額を「11,000」の形にします（エラーの文で使う）。
pub(crate) fn comma(value: impl Into<i128>) -> String {
    let value: i128 = value.into();
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if value < 0 { format!("-{out}") } else { out }
}
