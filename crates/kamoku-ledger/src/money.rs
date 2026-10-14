// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

/// 整数で表した円。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Yen(i64);
impl Yen {
    /// 零円。
    pub const ZERO: Self = Self(0);
    /// 円の値を作ります。
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }
    /// 円の整数値を返します。
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
    /// 桁あふれを検査して加算します。
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
    /// 桁あふれを検査して減算します。
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}
impl fmt::Display for Yen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}円", self.0)
    }
}
