// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::IdError;
use std::fmt;

pub(crate) fn valid_code(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn validate(value: &str, max: usize) -> Result<(), IdError> {
    if value.is_empty() {
        return Err(IdError::Empty);
    }
    if value.len() > max {
        return Err(IdError::TooLong);
    }
    if !valid_code(value, max) {
        return Err(IdError::BadChar);
    }
    Ok(())
}
/// 1〜64バイトのASCII英数字・ハイフン・下線による仕訳識別子。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId(String);
impl EntryId {
    /// 文字列を検証して仕訳識別子を作ります。
    ///
    /// # Errors
    /// 空・長さ超過・許可されていない文字を拒否します。
    pub fn new(value: &str) -> Result<Self, IdError> {
        validate(value, 64)?;
        Ok(Self(value.to_owned()))
    }
    /// 保存された文字列を返します。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
/// 1〜16バイトのASCII英数字・ハイフン・下線による勘定科目コード。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountCode(String);
impl AccountCode {
    /// 文字列を検証して勘定科目コードを作ります。
    ///
    /// # Errors
    /// 空・長さ超過・許可されていない文字を拒否します。
    pub fn new(value: &str) -> Result<Self, IdError> {
        validate(value, 16)?;
        Ok(Self(value.to_owned()))
    }
    /// 保存された文字列を返します。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for AccountCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
