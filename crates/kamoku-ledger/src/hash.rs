// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::HashError;
use std::fmt;

/// SHA-256 の32バイトのハッシュ。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash(pub(crate) [u8; 32]);
impl Hash {
    /// 空台帳のハッシュ（全32バイトが零）。
    pub const GENESIS: Self = Self([0; 32]);
    /// ハッシュのバイト列。
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// 小文字64桁の16進表現。
    #[must_use]
    pub fn to_hex(self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(64);
        for b in self.0 {
            if let (Some(hi), Some(lo)) = (
                DIGITS.get(usize::from(b >> 4)),
                DIGITS.get(usize::from(b & 15)),
            ) {
                s.push(char::from(*hi));
                s.push(char::from(*lo));
            }
        }
        s
    }
    /// 64桁の16進表現から復元します。大文字も受け付けます。
    ///
    /// # Errors
    /// 長さまたは16進文字の不正を返します。
    pub fn from_hex(s: &str) -> Result<Self, HashError> {
        if s.len() != 64 {
            return Err(HashError::InvalidLength);
        }
        let mut bytes = [0; 32];
        for (out, pair) in bytes.iter_mut().zip(s.as_bytes().as_chunks::<2>().0) {
            let mut digits = pair.iter().copied();
            let hi = digits
                .next()
                .and_then(hex_digit)
                .ok_or(HashError::InvalidHex)?;
            let lo = digits
                .next()
                .and_then(hex_digit)
                .ok_or(HashError::InvalidHex)?;
            *out = (hi << 4) | lo;
        }
        Ok(Self(bytes))
    }
}
fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}
impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
