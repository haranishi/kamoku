// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::input::{normalize_width, parse_amount};
pub(crate) fn read_balance(text: &str) -> Result<i64, String> {
    let clean: String = normalize_width(text)
        .chars()
        .filter(|c| !c.is_whitespace() && !['円', '¥', ','].contains(c))
        .collect();
    if clean.is_empty() || clean.chars().all(|c| c == '0') {
        return Ok(0);
    }
    let first = parse_amount(text);
    if first.is_ok() {
        return first;
    }
    if parse_amount(&format!("0-({clean})")).is_ok() {
        return Err("マイナスの金額は入れられません（0円は可）。".into());
    }
    if parse_amount(&format!("{clean}+1")) == Ok(1) {
        return Ok(0);
    }
    first
}
pub(crate) fn normalize_hash(text: &str) -> Result<String, String> {
    let text: String = normalize_width(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if text.is_empty() {
        return Err("控えたハッシュを入れてください。".into());
    }
    if !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("ハッシュに使えるのは、0〜9 と a〜f の文字だけです。写し間違いがないか確かめてください。".into());
    }
    if text.len() != 64 {
        return Err(format!(
            "ハッシュは64文字です（いまは{}文字）。全文を貼り付けてください。",
            text.len()
        ));
    }
    Ok(text)
}

pub(crate) fn ledger_error_kind(kind: &str) -> &'static str {
    match kind {
        "bad_hash" => "ハッシュの不一致",
        "broken_link" => "前の記録とのつながりの切れ",
        "wrong_sequence" | "bad_seq" => "連番の飛びや入れ違い",
        "rule_violation" | "bad_rule" => "記録の決まりへの違反",
        "quick_check" => "データベースのファイルの傷み",
        _ => "記録の不整合",
    }
}
pub(crate) struct Verification {
    pub ok: bool,
    pub title: &'static str,
    pub body: String,
}
pub(crate) fn verify_text(value: &serde_json::Value) -> Verification {
    if value["ok"].as_bool() == Some(true) {
        Verification {
            ok: true,
            title: "台帳は壊れていません",
            body: format!(
                "{}件の記録を最初から順にたどり、すべてのハッシュがつながっていることを確かめました。",
                value["events"].as_i64().unwrap_or(0)
            ),
        }
    } else {
        let e = &value["error"];
        let rule = e["rule"]
            .as_str()
            .filter(|r| !r.is_empty())
            .map_or(String::new(), |r| format!("（決まり: {r}）"));
        Verification {
            ok: false,
            title: "台帳に壊れた記録があります",
            body: format!(
                "{}番目の記録に、{}が見つかりました{rule}。それより後の記録は信用できません。自動では直さないので、バックアップと見比べてください。",
                e["seq"].as_i64().unwrap_or(0),
                ledger_error_kind(e["kind"].as_str().unwrap_or(""))
            ),
        }
    }
}
pub(crate) fn noted_text(noted: &serde_json::Value, events: i64) -> (bool, String) {
    if noted["found"].as_bool() != Some(true) {
        return(false,"この台帳には、控えたハッシュと一致する記録がありません。写し間違いがないか、別の帳簿のハッシュでないかを確かめてください。どちらでもなければ、控えた後に記録が書き換えられたおそれがあります。".into());
    }
    let seq = noted["seq"].as_i64().unwrap_or(0);
    (
        true,
        if noted["is_head"].as_bool() == Some(true) {
            format!(
                "台帳の{seq}番目の記録と一致しました（末尾と同じです）。控えた後に、記録は増えていません。"
            )
        } else {
            format!(
                "台帳の{seq}番目の記録と一致しました（末尾ではありません）。控えた後に、{}件の記録が足されています。",
                events.saturating_sub(seq).max(0)
            )
        },
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn balances_allow_zero_expressions_but_not_negative() {
        for t in [
            "", "0", "000", "０円", "20-20", "100/200", "5-5", "0*7", "-0",
        ] {
            assert_eq!(read_balance(t), Ok(0), "{t}");
        }
        assert_eq!(read_balance("(1200+300)*2"), Ok(3000));
        for text in ["-1", "-500", "ー500", "−1,200", "100-200"] {
            assert_eq!(
                read_balance(text),
                Err("マイナスの金額は入れられません（0円は可）。".into())
            );
        }
        assert_eq!(read_balance("1,650"), Ok(1650));
        assert_eq!(read_balance("１０００"), Ok(1000));
        assert!(read_balance("abc").unwrap_err().contains("使えない文字"));
        assert!(read_balance("1,2,3").is_err());
    }
    #[test]
    fn hashes_normalize_width_case_and_space() {
        assert_eq!(
            normalize_hash(&format!(" {} ", "Ａ".repeat(64))),
            Ok("a".repeat(64))
        );
        for t in ["", "bad!", &"a".repeat(63)] {
            assert!(normalize_hash(t).is_err());
        }
    }
    #[test]
    fn verification_reports_the_first_sequence_rule_and_japanese_kind() {
        use serde_json::json;
        assert!(verify_text(&json!({"ok":true,"events":80})).ok);
        let bad =
            verify_text(&json!({"ok":false,"error":{"seq":55,"kind":"bad_hash","rule":null}}));
        assert!(!bad.ok);
        assert!(bad.body.contains("55番目の記録に、ハッシュの不一致"));
        assert!(
            verify_text(
                &json!({"ok":false,"error":{"seq":7,"kind":"bad_rule","rule":"unbalanced"}})
            )
            .body
            .contains("決まり: unbalanced")
        );
        for (kind, label) in [
            ("bad_hash", "ハッシュの不一致"),
            ("broken_link", "前の記録とのつながりの切れ"),
            ("wrong_sequence", "連番の飛びや入れ違い"),
            ("rule_violation", "記録の決まりへの違反"),
            ("quick_check", "データベースのファイルの傷み"),
            ("unknown", "記録の不整合"),
        ] {
            assert_eq!(ledger_error_kind(kind), label);
        }
        let unknown = verify_text(&json!({"ok":false,"error":{"seq":3,"kind":"something_new"}}));
        assert!(!unknown.body.contains("something_new"));
    }
    #[test]
    fn hash_match_distinguishes_head_prefix_and_missing() {
        use serde_json::json;
        let head = noted_text(&json!({"found":true,"seq":128,"is_head":true}), 128);
        assert!(
            head.0
                && head
                    .1
                    .contains("128番目の記録と一致しました（末尾と同じです）")
        );
        let middle = noted_text(&json!({"found":true,"seq":100,"is_head":false}), 128);
        assert!(
            middle.1.contains("末尾ではありません")
                && middle.1.contains("28件の記録が足されています")
        );
        assert!(!noted_text(&json!({"found":false}), 128).0);
    }
}
