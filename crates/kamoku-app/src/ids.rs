// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 乱数と、識別子の作り方。

use std::fmt::Write as _;

/// 乱数を `bytes` バイト取り、小文字の16進にします。
///
/// OSの乱数を読めないときは失敗します（合言葉やセッション値を、弱い乱数で作らないため）。
pub(crate) fn random_hex(bytes: usize) -> Result<String, getrandom::Error> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf)?;
    Ok(hex(&buf))
}

/// バイト列を小文字の16進にします。
pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// 仕訳ID（`e_` + 時刻（ミリ秒）の16進12桁 + 乱数の16進8桁）。`EntryId` の規則（英数字と下線）に合う。
pub(crate) fn entry_id(millis: u64, random: u32) -> String {
    format!("e_{:012x}{random:08x}", millis & 0xffff_ffff_ffff)
}

/// 定数時間の比較。合言葉とセッション値の比較で、一致した長さを時間から推し量らせないため。
pub(crate) fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    std::hint::black_box(diff) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_id_has_the_documented_shape() {
        let id = entry_id(0x0192_ab3c_4d5e, 0x6f7a_8b9c);
        assert_eq!(id, "e_0192ab3c4d5e6f7a8b9c");
        assert!(kamoku_ledger::EntryId::new(&id).is_ok());
    }

    #[test]
    fn secrets_compare_exactly() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
        assert_eq!(random_hex(16).unwrap().len(), 32);
    }
}
