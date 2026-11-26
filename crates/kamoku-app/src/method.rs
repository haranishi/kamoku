// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 受け付けるHTTPメソッド。受け口（`http`）とブラウザ版の両方から使う。

/// 受け付けるメソッド。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// 読む。
    Get,
    /// 見出しだけ読む。
    Head,
    /// 作る。
    Post,
    /// 置き換える。
    Put,
    /// 一部を変える。
    Patch,
    /// 消す。
    Delete,
}
impl Method {
    /// 名前から返します。知らない名前は `None`（大文字・小文字は区別する）。
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "GET" => Some(Self::Get),
            "HEAD" => Some(Self::Head),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            _ => None,
        }
    }
    /// 名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
    /// 状態を変える要求か（`Origin` の検査の対象）。
    #[must_use]
    pub const fn changes_state(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch | Self::Delete)
    }
}
