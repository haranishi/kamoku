// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 動かし方。サーバーの起動とブラウザ版の両方から使う。

/// 動かし方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 本物の帳簿。ブラウザ版は合成データだけを開く。
    #[cfg_attr(
        target_arch = "wasm32",
        allow(dead_code, reason = "ブラウザ版は合成データの帳簿だけを開く")
    )]
    Normal,
    /// 合成データの帳簿（画面の上に「デモのデータです」と出す）。
    Demo,
}
impl Mode {
    /// `/status` の `mode` に入れる名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Demo => "demo",
        }
    }
}
