// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(crate) const PCT_ERROR: &str = "事業の割合は、0から100の整数で入れてください。";
pub(crate) const NO_ACCOUNT: &str =
    "合う科目がありません。ひらがなの読みか、科目の名前で探せます。";
pub(crate) const NO_WALLET: &str = "合う口座がありません。ひらがなの読みか、口座の名前で探せます。";
pub(crate) const TRANSITIONAL_NOTE: &str = "課税仕入れの日（商品の引渡し・役務の提供が完了した日）にしてください。経過措置の割合は、この日付で決まります。";

pub(crate) fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

pub(crate) fn integer(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

pub(crate) fn list(value: &Value) -> Vec<Value> {
    value
        .as_array()
        .cloned()
        .or_else(|| value.get("items").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

pub(crate) fn parse_pct(raw: &str) -> Option<i64> {
    let normalized = crate::input::normalize_width(raw);
    let trimmed = normalized.trim();
    if trimmed.is_empty() || trimmed.len() > 3 || !trimmed.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    trimmed.parse::<i64>().ok().filter(|pct| *pct <= 100)
}

pub(crate) fn tax_label(code: &str) -> &str {
    match code {
        "S10" => "課税売上10%",
        "S8" => "課税売上8%（軽減）",
        "P10" => "課税仕入10%",
        "P8" => "課税仕入8%（軽減）",
        "P10X" => "経過措置の仕入10%（免税事業者等から）",
        "P8X" => "経過措置の仕入8%（免税事業者等から）",
        "NT" => "非課税",
        "" => "対象外",
        _ => code,
    }
}

pub(crate) fn tax_options(kind: &str) -> Vec<(&'static str, &'static str)> {
    let codes: &[&str] = match kind {
        "income" => &["", "S10", "S8", "NT"],
        "journal" => &["", "S10", "S8", "P10", "P8", "P10X", "P8X", "NT"],
        _ => &["", "P10", "P8", "P10X", "P8X", "NT"],
    };
    codes.iter().map(|code| (*code, tax_label(code))).collect()
}

pub(crate) fn is_transitional(code: &str) -> bool {
    matches!(code, "P10X" | "P8X")
}

pub(crate) fn is_wallet(account: &Value) -> bool {
    account
        .get("wallet")
        .is_some_and(|wallet| !wallet.is_null() && wallet != &Value::Bool(false))
}

pub(crate) fn next_defaults(previous: &Value, next: &Value, tax: &str, pct: i64) -> (String, i64) {
    if is_wallet(next) {
        return (String::new(), 100);
    }
    let previous_tax = if is_wallet(previous) {
        ""
    } else {
        text(previous, "default_tax")
    };
    let previous_pct = if is_wallet(previous) {
        100
    } else {
        previous
            .get("default_pct")
            .and_then(Value::as_i64)
            .unwrap_or(100)
    };
    (
        if tax == previous_tax {
            text(next, "default_tax")
        } else {
            tax
        }
        .to_owned(),
        if pct == previous_pct {
            next.get("default_pct")
                .and_then(Value::as_i64)
                .unwrap_or(100)
        } else {
            pct
        },
    )
}

pub(crate) fn account_options(
    accounts: &[Value],
    kind: &str,
    for_entry: bool,
    keep: &[String],
) -> Vec<Value> {
    let mut hidden = wallet_parent_codes(accounts);
    for code in keep {
        hidden.remove(code);
    }
    let mut out: Vec<Value> = accounts
        .iter()
        .filter(|account| {
            account["enabled"] == true && (!for_entry || !hidden.contains(text(account, "code")))
        })
        .cloned()
        .collect();
    let rank = |account: &Value| -> u8 {
        if kind == "income" {
            if account["kind"] == "revenue" {
                0
            } else if account["code"] == "owner_inv" {
                1
            } else if is_wallet(account) {
                3
            } else if account["kind"] == "expense" {
                4
            } else {
                2
            }
        } else if account["kind"] == "expense" {
            0
        } else if account["code"] == "owner_draw" {
            1
        } else if is_wallet(account) {
            3
        } else if account["kind"] == "revenue" {
            4
        } else {
            2
        }
    };
    out.sort_by_key(|account| (rank(account), integer(account, "sort")));
    out
}

pub(crate) fn wallet_parent_codes(accounts: &[Value]) -> BTreeSet<String> {
    accounts
        .iter()
        .filter(|account| {
            account["enabled"] == true && is_wallet(account) && account["wallet"]["type"] != "card"
        })
        .map(|account| text(account, "parent"))
        .filter(|code| !code.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn wallet_options(accounts: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = accounts
        .iter()
        .filter(|account| is_wallet(account) && account["enabled"] == true)
        .cloned()
        .collect();
    out.sort_by_key(|account| integer(account, "sort"));
    out
}

pub(crate) fn kind_label(kind: &str) -> &str {
    match kind {
        "expense" => "支出",
        "income" => "収入",
        "transfer" => "振替",
        "journal" => "仕訳の形",
        "opening" => "期首残高",
        _ => kind,
    }
}

pub(crate) fn wallet_label(kind: &str) -> &'static str {
    match kind {
        "income" => "入金先の口座",
        "transfer" => "移動元の口座",
        _ => "支払った口座",
    }
}

pub(crate) fn pct_source_note(account: &Value) -> String {
    let pct = account["default_pct"].as_i64().unwrap_or(100);
    if !account.is_null() && !is_wallet(account) && pct < 100 {
        format!("この科目の既定の割合は、事業{pct}%です。「勘定科目」の画面で変えられます。")
    } else {
        String::new()
    }
}

pub(crate) fn tax_source_note(
    taxable: bool,
    kind: &str,
    account: &Value,
    tax: &str,
) -> &'static str {
    if taxable
        && kind == "expense"
        && account["kind"] == "expense"
        && !is_wallet(account)
        && text(account, "default_tax").is_empty()
        && tax.is_empty()
    {
        "この科目の既定は「対象外」です。課税の取引なら、税区分を選んでください。"
    } else {
        ""
    }
}

pub(crate) fn jp_date(iso: &str) -> String {
    if !iso_shape(iso) {
        return iso.into();
    }
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() == 3
        && let (Ok(year), Ok(month), Ok(day)) = (
            parts[0].parse::<u32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        )
    {
        return format!("{year}年{month}月{day}日");
    }
    iso.to_owned()
}

pub(crate) fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub(crate) fn add_days(iso: &str, count: i32) -> String {
    let parts: Vec<i32> = iso
        .split('-')
        .filter_map(|part| part.parse().ok())
        .collect();
    if parts.len() != 3 {
        return iso.to_owned();
    }
    let (mut year, mut month, mut day) = (parts[0], parts[1], parts[2] + count);
    while day > days_in_month(year, month) {
        day -= days_in_month(year, month);
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    while day < 1 {
        month -= 1;
        if month < 1 {
            month = 12;
            year -= 1;
        }
        day += days_in_month(year, month);
    }
    format!("{year:04}-{month:02}-{day:02}")
}

pub(crate) fn date_concern(date: &str, today: &str, opened: &str, original: &str) -> String {
    if date == original || !iso_shape(date) {
        return String::new();
    }
    if iso_shape(today) && date > add_days(today, 31).as_str() {
        return format!(
            "{}の取引として保存します。今日より31日を超えて先の日付です。年は合っていますか。",
            jp_date(date)
        );
    }
    if iso_shape(opened) && date < opened {
        return format!(
            "{}の取引として保存します。開業日（{}）より前の日付です。年は合っていますか。",
            jp_date(date),
            jp_date(opened)
        );
    }
    String::new()
}

pub(crate) fn locked_note(
    date: &str,
    locked: &str,
    correcting: bool,
    original: &str,
    on_save: bool,
) -> String {
    if !iso_shape(locked) {
        return String::new();
    }
    let prefix = format!("{}まで締めてあるため、", jp_date(locked));
    if correcting && iso_shape(original) && original <= locked {
        return format!(
            "{prefix}{}の仕訳は訂正できません。{}",
            jp_date(original),
            if on_save {
                "締めた日は、設定の「締め」で見直せます。"
            } else {
                ""
            }
        );
    }
    if !iso_shape(date) || date > locked {
        return String::new();
    }
    format!(
        "{prefix}この日付には{}できません。{}",
        if correcting { "訂正" } else { "登録" },
        if on_save {
            "日付を直すか、設定の「締め」で締めた日を見直してください。"
        } else {
            ""
        }
    )
}

pub(crate) fn locked_detail(date: &str, locked: &str) -> String {
    if !iso_shape(locked) || !iso_shape(date) || date > locked {
        String::new()
    } else {
        format!(
            "{}まで締めてあるため、この仕訳は訂正も取消もできません。締めた日は、設定の「締め」で見直せます。",
            jp_date(locked)
        )
    }
}

pub(crate) fn line_memo(line: &Value) -> String {
    let memo = text(line, "memo");
    let pct = memo
        .strip_prefix("家事按分 ")
        .and_then(|memo| memo.strip_suffix('%'));
    if line["account"] == "owner_draw"
        && pct.is_some_and(|pct| {
            !pct.is_empty() && pct.len() <= 3 && pct.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        format!("私用の分 {}%", pct.unwrap_or_default())
    } else {
        memo.to_owned()
    }
}

fn iso_shape(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

pub(crate) fn journal_input(entry: &Value) -> Value {
    json!({"kind":"journal", "date":entry["date"], "description":entry["description"], "partner":entry["partner"], "lines":list(&entry["lines"]).iter().map(|line|json!({"account":line["account"],"side":line["side"],"amount":line["amount"],"tax_code":line["tax_code"],"memo":line["memo"]})).collect::<Vec<_>>()})
}

pub(crate) fn changed_fields(before: &Value, after: &Value) -> Vec<(&'static str, &'static str)> {
    let fields = [
        ("kind", "種別"),
        ("date", "日付"),
        ("amount", "金額"),
        ("account", "科目"),
        ("wallet", "口座"),
        ("to_wallet", "移動先の口座"),
        ("tax_code", "税区分"),
        ("partner", "取引先"),
        ("description", "摘要"),
        ("business_pct", "事業の割合"),
        ("memo", "メモ"),
        ("lines", "明細"),
    ];
    fields
        .into_iter()
        .filter(|(key, _)| {
            let normalize = |value: &Value| {
                let value = value.get(*key).cloned().unwrap_or(Value::Null);
                if *key == "business_pct" && value.is_null() {
                    json!(100)
                } else if value == "" {
                    Value::Null
                } else {
                    value
                }
            };
            normalize(before) != normalize(after)
        })
        .collect()
}

#[cfg(test)]
#[path = "legacy_tests.rs"]
mod legacy_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_03_3_defaults_follow_the_old_default_and_preserve_explicit_values() {
        let old = json!({"default_tax":"P10", "default_pct":70});
        let next = json!({"default_tax":"P8", "default_pct":30});
        assert_eq!(next_defaults(&old, &next, "P10", 70), ("P8".into(), 30));
        assert_eq!(next_defaults(&old, &next, "NT", 50), ("NT".into(), 50));
        assert_eq!(
            next_defaults(&next, &json!({"wallet":{"type":"card"}}), "P8", 30),
            (String::new(), 100)
        );
    }

    #[test]
    fn percentage_is_an_integer_including_full_width_digits() {
        for value in ["", "150", "-10", "1.2", "5e1", "abc"] {
            assert_eq!(parse_pct(value), None);
        }
        assert_eq!(parse_pct("１００"), Some(100));
        assert_eq!(parse_pct(" ０ "), Some(0));
    }

    #[test]
    fn ac_02_9_hide_wallet_parents_except_card_and_preserve_legacy_edit_accounts() {
        let accounts = vec![
            json!({"code":"cash","enabled":true}),
            json!({"code":"accrued","enabled":true}),
            json!({"code":"w1","enabled":true,"parent":"cash","wallet":{"type":"cash"}}),
            json!({"code":"w2","enabled":true,"parent":"accrued","wallet":{"type":"card"}}),
        ];
        assert!(
            !account_options(&accounts, "expense", true, &[])
                .iter()
                .any(|account| account["code"] == "cash")
        );
        assert!(
            account_options(&accounts, "expense", true, &[])
                .iter()
                .any(|account| account["code"] == "accrued")
        );
        assert!(
            account_options(&accounts, "expense", true, &["cash".into()])
                .iter()
                .any(|account| account["code"] == "cash")
        );
    }

    #[test]
    fn ac_06_3_locked_period_includes_the_boundary_and_original_correction_date() {
        assert_eq!(
            locked_note("2026-10-01", "2026-09-30", false, "", false),
            ""
        );
        assert!(locked_note("2026-09-30", "2026-09-30", false, "", true).contains("2026年9月30日"));
        assert!(
            locked_note("2026-10-01", "2026-09-30", true, "2026-09-01", false)
                .contains("2026年9月1日の仕訳")
        );
        assert!(locked_detail("2026-09-30", "2026-09-30").contains("訂正も取消もできません"));
    }

    #[test]
    fn date_concerns_cross_month_year_and_leap_boundaries() {
        assert_eq!(add_days("2024-02-28", 1), "2024-02-29");
        assert_eq!(add_days("2026-01-01", -1), "2025-12-31");
        assert!(date_concern("2026-11-15", "2026-10-14", "", "").contains("31日を超えて"));
        assert_eq!(date_concern("2026-11-14", "2026-10-14", "", ""), "");
        assert!(date_concern("2025-12-31", "2026-10-14", "2026-01-01", "").contains("開業日"));
        assert_eq!(date_concern("", "2026-10-14", "2026-01-01", ""), "");
        assert_eq!(
            date_concern("2026/10/14", "2026-10-14", "2026-01-01", ""),
            ""
        );
        assert_eq!(date_concern("2025-12-31", "2026-10-14", "invalid", ""), "");
        assert_eq!(
            date_concern("2030-01-01", "2026-10-14", "2026-01-01", "2030-01-01"),
            ""
        );
    }

    #[test]
    fn correction_diff_normalizes_empty_and_default_percentage_but_keeps_integer_precision() {
        assert_eq!(
            changed_fields(
                &json!({"partner":null}),
                &json!({"partner":"", "business_pct":100})
            ),
            [] as [(&str, &str); 0]
        );
        assert_eq!(
            changed_fields(
                &json!({"amount":9_007_199_254_740_992_i64}),
                &json!({"amount":9_007_199_254_740_993_i64})
            ),
            vec![("amount", "金額")]
        );
        assert_eq!(
            line_memo(&json!({"account":"owner_draw", "memo":"家事按分 30%"})),
            "私用の分 30%"
        );
        for memo in [
            "家事按分 abc%",
            "家事按分 1000%",
            "家事按分 %",
            "家事按分 30% 以外",
        ] {
            assert_eq!(
                line_memo(&json!({"account":"owner_draw","memo":memo})),
                memo
            );
        }
    }

    #[test]
    fn source_notes_explain_only_real_defaults_and_never_change_values() {
        let account = json!({"kind":"expense","default_pct":30,"default_tax":null});
        assert!(pct_source_note(&account).contains("事業30%"));
        assert_eq!(pct_source_note(&json!({"default_pct":100})), "");
        assert_eq!(pct_source_note(&json!({"default_pct":null})), "");
        assert_eq!(
            pct_source_note(&json!({"default_pct":30,"wallet":{"type":"card"}})),
            ""
        );
        assert_ne!(tax_source_note(true, "expense", &account, ""), "");
        assert_eq!(tax_source_note(false, "expense", &account, ""), "");
        assert_eq!(tax_source_note(true, "income", &account, ""), "");
        assert_eq!(tax_source_note(true, "expense", &account, "P10"), "");
        assert_eq!(
            tax_source_note(true, "expense", &json!({"kind":"asset"}), ""),
            ""
        );
        assert_eq!(
            tax_source_note(
                true,
                "expense",
                &json!({"kind":"expense","default_tax":"P10"}),
                ""
            ),
            ""
        );
    }

    #[test]
    fn malformed_dates_never_produce_a_locked_period_notice() {
        for date in ["", "2026/09/30", "2026-9-30", "not-a-date"] {
            assert_eq!(locked_note("2026-09-15", date, false, "", false), "");
            assert_eq!(locked_note(date, "2026-09-30", false, "", false), "");
            assert_eq!(locked_detail(date, "2026-09-30"), "");
        }
    }
}
