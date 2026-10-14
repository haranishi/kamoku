// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSONの本文の読み取り。欄ごとに、形の誤り（400）と規則の違反（422）を分け、欄の名前を付けて返す。

use super::ApiError;
use serde_json::{Map, Value};
use time::Date;

/// 本文を、JSONのオブジェクトとして読みます。
pub(crate) fn object(body: &[u8]) -> Result<Map<String, Value>, ApiError> {
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(ApiError::bad_request(
            "本文は JSON のオブジェクト（{ ... }）にしてください。",
        )),
        Err(_) => Err(ApiError::bad_request(
            "本文を JSON として読めません。形を確かめてください。",
        )),
    }
}

/// 欄を読む道具。`prefix` は `lines[0].` のような、入れ子の欄の名前の頭。
pub(crate) struct Fields<'a> {
    map: &'a Map<String, Value>,
    prefix: String,
}
impl<'a> Fields<'a> {
    pub(crate) fn new(map: &'a Map<String, Value>) -> Self {
        Self {
            map,
            prefix: String::new(),
        }
    }
    pub(crate) const fn nested(map: &'a Map<String, Value>, prefix: String) -> Self {
        Self { map, prefix }
    }
    /// 欄の名前（入れ子の頭つき）。
    pub(crate) fn name(&self, key: &str) -> String {
        format!("{}{key}", self.prefix)
    }
    /// 欄があるか（`null` も「ある」に入る）。
    pub(crate) fn has(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }
    /// 欄の値（無いか `null` なら `None`）。
    pub(crate) fn value(&self, key: &str) -> Option<&'a Value> {
        self.map.get(key).filter(|v| !v.is_null())
    }
    fn wrong_type(&self, key: &str, what: &str) -> ApiError {
        ApiError::bad_request(format!("「{}」は{what}で入れてください。", self.name(key)))
            .with_field(self.name(key))
    }
    /// 必須の欄が無いときの失敗。
    pub(crate) fn missing(&self, key: &str, label: &str) -> ApiError {
        ApiError::bad_request(format!("{label}（{}）を入れてください。", self.name(key)))
            .with_field(self.name(key))
    }

    pub(crate) fn opt_str(&self, key: &str) -> Result<Option<String>, ApiError> {
        match self.value(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(self.wrong_type(key, "文字列")),
        }
    }
    /// 文字列（無ければ空）。
    pub(crate) fn text(&self, key: &str) -> Result<String, ApiError> {
        Ok(self.opt_str(key)?.unwrap_or_default())
    }
    /// 必須の文字列。
    pub(crate) fn req_str(&self, key: &str, label: &str) -> Result<String, ApiError> {
        self.opt_str(key)?.ok_or_else(|| self.missing(key, label))
    }
    pub(crate) fn opt_bool(&self, key: &str) -> Result<Option<bool>, ApiError> {
        match self.value(key) {
            None => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(_) => Err(self.wrong_type(key, "true か false")),
        }
    }
    /// 整数（小数や文字列は形の誤り）。
    pub(crate) fn opt_int(&self, key: &str) -> Result<Option<i64>, ApiError> {
        match self.value(key) {
            None => Ok(None),
            Some(Value::Number(n)) => n
                .as_i64()
                .map(Some)
                .ok_or_else(|| self.wrong_type(key, "整数")),
            Some(_) => Err(self.wrong_type(key, "整数")),
        }
    }
    /// 金額。整数でない値は形の誤り、整数だが i64 に収まらない値は `amount_range`。
    pub(crate) fn opt_amount(&self, key: &str) -> Result<Option<i64>, ApiError> {
        match self.value(key) {
            Some(Value::Number(n)) if n.as_i64().is_none() && n.as_u64().is_some() => {
                Err(ApiError::new(
                    422,
                    "amount_range",
                    "金額は1円から9,999,999,999,999円までです。",
                )
                .with_field(self.name(key)))
            }
            _ => self.opt_int(key),
        }
    }
    /// 事業の割合（0〜100の整数）。範囲の外は規則の違反（422）。
    pub(crate) fn opt_pct(&self, key: &str) -> Result<Option<u8>, ApiError> {
        let out_of_range = || {
            ApiError::rule("事業の割合は0から100までの整数で入れてください。")
                .with_field(self.name(key))
        };
        match self.value(key) {
            Some(Value::Number(n)) if n.as_i64().is_none() && n.as_u64().is_some() => {
                Err(out_of_range())
            }
            _ => self
                .opt_int(key)?
                .map(|n| {
                    u8::try_from(n)
                        .ok()
                        .filter(|p| *p <= 100)
                        .ok_or_else(out_of_range)
                })
                .transpose(),
        }
    }
    /// `YYYY-MM-DD` の実在する日。
    pub(crate) fn opt_date(&self, key: &str) -> Result<Option<Date>, ApiError> {
        self.opt_str(key)?
            .filter(|s| !s.is_empty())
            .map(|s| {
                kamoku_books::parse_date(&s).ok_or_else(|| {
                    ApiError::bad_request(format!(
                        "日付「{s}」を読めません。実在する日を YYYY-MM-DD で入れてください。"
                    ))
                    .with_field(self.name(key))
                })
            })
            .transpose()
    }
    pub(crate) fn opt_array(&self, key: &str) -> Result<Option<&'a Vec<Value>>, ApiError> {
        match self.value(key) {
            None => Ok(None),
            Some(Value::Array(a)) => Ok(Some(a)),
            Some(_) => Err(self.wrong_type(key, "配列")),
        }
    }
    pub(crate) fn opt_object(&self, key: &str) -> Result<Option<&'a Map<String, Value>>, ApiError> {
        match self.value(key) {
            None => Ok(None),
            Some(Value::Object(m)) => Ok(Some(m)),
            Some(_) => Err(self.wrong_type(key, "オブジェクト")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fields(v: &Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn separates_shape_from_rules() {
        let map = fields(
            &json!({"a": 1.5, "b": "x", "big": 18_446_744_073_709_551_615_u64, "p": 101, "d": "2026-02-30"}),
        );
        let f = Fields::new(&map);
        assert_eq!(f.opt_int("a").unwrap_err().status, 400);
        assert_eq!(f.opt_int("b").unwrap_err().status, 400);
        assert_eq!(f.opt_amount("big").unwrap_err().code, "amount_range");
        assert_eq!(f.opt_pct("p").unwrap_err().status, 422);
        assert_eq!(f.opt_pct("big").unwrap_err().status, 422);
        assert_eq!(f.opt_date("d").unwrap_err().field.as_deref(), Some("d"));
        assert_eq!(f.opt_int("missing").unwrap(), None);
        assert!(object(b"[1]").is_err() && object(b"{").is_err());
    }
}
