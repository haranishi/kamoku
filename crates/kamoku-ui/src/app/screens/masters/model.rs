// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use unicode_normalization::UnicodeNormalization;
pub(crate) const KINDS: &[(&str, &str)] = &[
    ("asset", "資産"),
    ("liability", "負債"),
    ("equity", "資本"),
    ("revenue", "収益"),
    ("expense", "費用"),
];
pub(crate) const WALLET_TYPES: &[(&str, &str)] = &[
    ("cash", "現金"),
    ("bank", "預金"),
    ("card", "クレジットカード"),
    ("ewallet", "電子マネー"),
];
pub(crate) const TAX: &[(&str, &str)] = &[
    ("", "対象外（なし）"),
    ("S10", "課税売上10%"),
    ("S8", "課税売上8%（軽減）"),
    ("P10", "課税仕入10%"),
    ("P8", "課税仕入8%（軽減）"),
    ("P10X", "経過措置の仕入10%（免税事業者等から）"),
    ("P8X", "経過措置の仕入8%（免税事業者等から）"),
    ("NT", "非課税"),
];
pub(crate) fn kind_label(kind: &str) -> &str {
    KINDS
        .iter()
        .find(|(v, _)| *v == kind)
        .map_or("", |(_, label)| label)
}
pub(crate) fn tax_label(tax: &str) -> &str {
    TAX.iter()
        .find(|(v, _)| *v == tax)
        .map_or(tax, |(_, label)| label)
}
pub(crate) fn wallet_label(kind: &str) -> &str {
    WALLET_TYPES
        .iter()
        .find(|(v, _)| *v == kind)
        .map_or("", |(_, label)| label)
}
pub(crate) fn required(code: &str) -> bool {
    matches!(
        code,
        "cash" | "dep_other" | "accrued" | "owner_draw" | "owner_inv" | "capital" | "sales"
    )
}
pub(crate) fn name_locked(code: &str) -> bool {
    matches!(code, "owner_draw" | "owner_inv" | "capital")
}
pub(crate) fn wallet_parent(kind: &str) -> &str {
    match kind {
        "bank" => "dep_other",
        "card" => "accrued",
        _ => "cash",
    }
}
pub(crate) fn invoice(text: &str) -> Result<Option<String>, String> {
    let value: String = text
        .nfkc()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '‐' | '−' | 'ー'))
        .flat_map(char::to_uppercase)
        .collect();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() == 14 && value.starts_with('T') && value[1..].bytes().all(|c| c.is_ascii_digit())
    {
        Ok(Some(value))
    } else {
        Err("登録番号は、T と13桁の数字で入れてください（例: T1234567890123）。".into())
    }
}
pub(crate) fn percentage(text: &str) -> Result<i64, String> {
    text.nfkc()
        .collect::<String>()
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|v| (0..=100).contains(v))
        .ok_or_else(|| "0から100の整数で入れてください。".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invoice_normalizes_width_separators_and_case_but_does_not_claim_real_validation() {
        assert_eq!(
            invoice("ｔ１２３４５６７８９０１２３").unwrap().as_deref(),
            Some("T1234567890123")
        );
        assert_eq!(
            invoice(" t-1234 5678 90123 ").unwrap().as_deref(),
            Some("T1234567890123")
        );
        assert!(invoice("T123").is_err());
        assert_eq!(invoice(" - ").unwrap(), None);
    }
    #[test]
    fn account_constraints_match_server_required_accounts() {
        assert!(required("sales"));
        assert!(!name_locked("sales"));
        assert!(name_locked("owner_draw"));
        assert!(!required("custom"));
        assert_eq!(wallet_parent("card"), "accrued");
        assert_eq!(percentage("１００").unwrap(), 100);
        for bad in ["", "-1", "101", "1.5"] {
            assert!(percentage(bad).is_err());
        }
    }
}
