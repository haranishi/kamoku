// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 問い合わせの文字列（`?a=1&b=2`）とパスの読み取り。

use crate::api::ApiError;
use time::Date;

/// パーセント符号化を戻します。`plus_space` なら `+` を空白にします（問い合わせの文字列の決まり）。
/// 壊れた `%` の並びと、UTF-8 にならないバイト列は `None`。
pub(crate) fn percent_decode(text: &str, plus_space: bool) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'%' => {
                let hi = bytes.get(i + 1).and_then(|c| (*c as char).to_digit(16))?;
                let lo = bytes.get(i + 2).and_then(|c| (*c as char).to_digit(16))?;
                out.push(u8::try_from(hi * 16 + lo).ok()?);
                i += 3;
            }
            b'+' if plus_space => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// 読んだ問い合わせ。同じ名前が2回あれば、最初のものを使う。値が空の名前は無いものとして扱う
/// （画面は空の値を送らないが、`?from=&to=` のような手入力も受け付けるため）。
#[derive(Debug, Default, Clone)]
pub(crate) struct Query(Vec<(String, String)>);
impl Query {
    pub(crate) fn parse(raw: &str) -> Result<Self, ApiError> {
        let mut pairs = Vec::new();
        for part in raw.split('&').filter(|p| !p.is_empty()) {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            let bad = || ApiError::bad_request("問い合わせの文字列の符号化が正しくありません。");
            let key = percent_decode(key, true).ok_or_else(bad)?;
            let value = percent_decode(value, true).ok_or_else(bad)?;
            pairs.push((key, value));
        }
        Ok(Self(pairs))
    }
    /// 値（空でないもの）。
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }
    /// `YYYY-MM-DD` の日付。
    pub(crate) fn date(&self, key: &str) -> Result<Option<Date>, ApiError> {
        self.get(key)
            .map(|v| {
                kamoku_books::parse_date(v).ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "日付「{v}」を読めません。実在する日を YYYY-MM-DD で入れてください。"
                    ))
                    .with_field(key)
                })
            })
            .transpose()
    }
    /// 整数。
    pub(crate) fn int(&self, key: &str) -> Result<Option<i64>, ApiError> {
        self.get(key)
            .map(|v| {
                v.parse::<i64>().map_err(|_| {
                    ApiError::bad_request(format!("「{key}」は整数で入れてください。"))
                        .with_field(key)
                })
            })
            .transpose()
    }
    /// `1` か `true` なら真。
    pub(crate) fn flag(&self, key: &str) -> bool {
        matches!(self.get(key), Some("1" | "true"))
    }
    /// 一覧の `limit`（1〜500、既定50）と `offset`（0以上、既定0）。
    pub(crate) fn page(&self) -> Result<(u32, u64), ApiError> {
        let limit = match self.int("limit")? {
            None => 50,
            Some(n) => u32::try_from(n)
                .ok()
                .filter(|n| (1..=500).contains(n))
                .ok_or_else(|| {
                    ApiError::bad_request("limit は1から500までの整数で入れてください。")
                        .with_field("limit")
                })?,
        };
        let offset = match self.int("offset")? {
            None => 0,
            Some(n) => u64::try_from(n).map_err(|_| {
                ApiError::bad_request("offset は0以上の整数で入れてください。").with_field("offset")
            })?,
        };
        Ok((limit, offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_and_plus() {
        assert_eq!(percent_decode("a%20b+c", true).as_deref(), Some("a b c"));
        assert_eq!(percent_decode("a+b", false).as_deref(), Some("a+b"));
        assert_eq!(percent_decode("%E3%81%82", true).as_deref(), Some("あ"));
        assert_eq!(percent_decode("%zz", true), None);
        assert_eq!(percent_decode("%e3", true), None);
        assert_eq!(percent_decode("%", true), None);
    }

    #[test]
    fn query_values_and_pages() {
        let q = Query::parse("a=1&a=2&b=&limit=500&q=%E3%81%82").unwrap();
        assert_eq!(q.get("a"), Some("1"));
        assert_eq!(q.get("b"), None);
        assert_eq!(q.get("q"), Some("あ"));
        assert_eq!(q.page().unwrap(), (500, 0));
        assert!(Query::parse("limit=501").unwrap().page().is_err());
        assert!(Query::parse("limit=0").unwrap().page().is_err());
        assert!(Query::parse("offset=-1").unwrap().page().is_err());
        assert!(Query::parse("x=%zz").is_err());
    }
}
