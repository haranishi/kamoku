// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::fmt::Write;

fn address(hash: &str) -> (&str, &str) {
    hash.trim_start_matches('#')
        .split_once('?')
        .unwrap_or((hash.trim_start_matches('#'), ""))
}
fn decode(raw: &str, plus: bool) -> Option<String> {
    let mut out = Vec::new();
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let pair = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
                out.push(u8::from_str_radix(pair, 16).ok()?);
                index += 3;
            }
            b'+' if plus => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}
pub(crate) fn encode(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}
fn query(raw: &str) -> BTreeMap<String, String> {
    raw.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
        .map(|(key, value)| {
            (
                decode(key, true).unwrap_or_else(|| key.into()),
                decode(value, true).unwrap_or_else(|| value.into()),
            )
        })
        .collect()
}
pub(crate) fn is_return_target(hash: &str) -> bool {
    if !hash.starts_with("#/") {
        return false;
    }
    let path = address(hash).0;
    !matches!(path, "/entry" | "/journal" | "/setup")
        && !path.starts_with("/entry/")
        && !path.starts_with("/journal/")
}
pub(crate) fn same_address(left: &str, right: &str) -> bool {
    let (lp, lq) = address(left);
    let (rp, rq) = address(right);
    lp == rp && query(lq) == query(rq)
}
pub(crate) fn to_back_param(hash: &str) -> String {
    if hash.is_empty() || same_address(hash, "#/journal") {
        String::new()
    } else {
        hash.trim_start_matches('#').into()
    }
}
pub(crate) fn from_back_param(value: &str) -> String {
    if !value.starts_with('/') || value.starts_with("//") {
        return String::new();
    }
    let hash = format!("#{value}");
    let Some(path) = decode(address(value).0, false) else {
        return String::new();
    };
    if path.starts_with("//")
        || matches!(path.as_str(), "/entry" | "/setup")
        || path.starts_with("/entry/")
        || path.starts_with("/journal/")
    {
        return String::new();
    }
    if is_return_target(&hash) || path == "/journal" {
        hash
    } else {
        String::new()
    }
}
pub(crate) fn correct_hash(id: &str, back: &str) -> String {
    let path = format!("#/entry/{}", encode(id));
    let back = to_back_param(back);
    if back.is_empty() {
        path
    } else {
        format!("{path}?back={}", encode(&back))
    }
}
pub(crate) fn detail_hash(id: &str, back: &str) -> String {
    let path = format!("#/journal/{}", encode(id));
    if back.is_empty() {
        return path;
    }
    let (back_path, raw) = address(back);
    if back_path == "/journal" {
        if raw.is_empty() {
            path
        } else {
            format!("{path}?{raw}")
        }
    } else {
        let back = to_back_param(back);
        if back.is_empty() {
            path
        } else {
            format!("{path}?back={}", encode(&back))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_return_preserves_query_and_rejects_external_or_unsafe_paths() {
        for hash in ["#/reports/ledger?account=x", "#/"] {
            assert!(is_return_target(hash));
        }
        for hash in [
            "#/journal",
            "#/journal?q=a",
            "#/journal/e_1",
            "#/entry",
            "#/entry/e_1",
            "#/setup",
            "",
            "https://example.com/#/",
            "/reports",
        ] {
            assert!(!is_return_target(hash));
        }
        assert!(same_address(
            "#/journal?q=%E6%90%BA%E5%B8%AF&from=2026-10-01",
            "#/journal?from=2026-10-01&q=携帯"
        ));
        assert!(!same_address("#/journal?q=a", "#/journal?q=b"));
        assert_eq!(to_back_param("#/journal"), "");
        for value in [
            "//example.com",
            "https://example.com",
            "javascript:alert(1)",
            "",
            "/entry",
            "/entry/e_1",
            "/journal/e_1",
            "/setup",
            "/reports/%",
            "/reports/%E0%A4%A",
            "/%65ntry",
        ] {
            assert_eq!(from_back_param(value), "");
        }
        assert_eq!(
            from_back_param("/reports/ledger?account=x"),
            "#/reports/ledger?account=x"
        );
        assert_eq!(from_back_param("/"), "#/");
    }
    #[test]
    fn correction_and_detail_round_trip_the_list_or_ledger() {
        assert_eq!(correct_hash("e_1", "#/journal"), "#/entry/e_1");
        assert_eq!(
            detail_hash("e_1", "#/journal?q=%E6%90%BA%E5%B8%AF&page=2"),
            "#/journal/e_1?q=%E6%90%BA%E5%B8%AF&page=2"
        );
        let hash = correct_hash("e_1", "#/journal?q=携帯");
        let back = query(address(&hash).1).get("back").cloned().unwrap();
        assert!(same_address(&from_back_param(&back), "#/journal?q=携帯"));
        let ledger = "#/reports/ledger?account=x&month=2026-10";
        let hash = detail_hash("e_1", ledger);
        let back = query(address(&hash).1).get("back").cloned().unwrap();
        assert_eq!(from_back_param(&back), ledger);
    }

    #[test]
    fn legacy_address_equality_ignores_encoding_but_preserves_paths_and_values() {
        assert!(same_address("#/journal", "#/journal"));
        assert!(!same_address("#/journal?q=a", "#/journal/e_1?q=a"));
        assert!(!is_return_target("#/entry/e_1?back=%2Fjournal"));
        assert!(is_return_target(
            "#/reports/ledger?account=exp_supplies&month=2026-10"
        ));
    }

    #[test]
    fn legacy_back_parameter_omits_plain_list_and_keeps_valid_list_queries() {
        assert_eq!(to_back_param("#/journal?q=a"), "/journal?q=a");
        assert_eq!(to_back_param(""), "");
        assert_eq!(from_back_param("/journal?q=a"), "#/journal?q=a");
    }

    #[test]
    fn legacy_correction_without_back_is_direct_and_filtered_back_round_trips() {
        assert_eq!(correct_hash("e_1", ""), "#/entry/e_1");
        let hash = correct_hash("e_1", "#/journal?q=携帯");
        assert!(hash.starts_with("#/entry/e_1?back="));
        let back = query(address(&hash).1).get("back").cloned().unwrap();
        assert!(same_address(
            &from_back_param(&back),
            "#/journal?q=%E6%90%BA%E5%B8%AF"
        ));
    }

    #[test]
    fn legacy_detail_without_back_is_direct_and_ledger_back_round_trips() {
        assert_eq!(detail_hash("e_1", ""), "#/journal/e_1");
        let ledger = "#/reports/ledger?account=exp_supplies&month=2026-10";
        let hash = detail_hash("e_1", ledger);
        assert!(hash.starts_with("#/journal/e_1?back="));
        let back = query(address(&hash).1).get("back").cloned().unwrap();
        assert_eq!(from_back_param(&back), ledger);
    }
}
