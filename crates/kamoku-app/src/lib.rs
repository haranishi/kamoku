// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 手元で動くkamokuの本体。ブラウザで開く画面と、それに応えるサーバー（実行ファイル名は `kamoku`）。
//!
//! - 待ち受けは `127.0.0.1` だけ。起動ごとに変わる合言葉（128ビット）でセッションのCookieを渡し、
//!   `Host` と `Origin` を検査する（`docs/spec/api-common.md`）。外へは何も送らない。
//! - HTTP の受け口は [`http`] に閉じ込めた、標準ライブラリだけの小さな実装。自作にした理由と、
//!   扱わない範囲は [`http`] に書いてある。
//! - 仕訳の組み立てと帳票の計算は `kamoku-books`、保存は `kamoku-store`、CSVの読み取りは
//!   `kamoku-import` を呼ぶだけで、規則を自前で持たない。
//! - コマンドは [`cli::run`]。`serve` `demo` `verify` `backup` がある。
//! - wasm32 ではサーバーの受け口を外し、[`web`] が同じ API をメモリ上の帳簿で動かす。

#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
pub mod demo;
#[cfg(not(target_arch = "wasm32"))]
pub mod http;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod web;

mod api;
#[cfg(not(target_arch = "wasm32"))]
mod app;
#[cfg(not(target_arch = "wasm32"))]
mod browser;
mod clock;
mod csv;
mod dispatch;
#[cfg(not(target_arch = "wasm32"))]
mod gate;
mod ids;
mod method;
mod mode;
mod mono;
#[cfg(not(target_arch = "wasm32"))]
mod paths;
mod query;
mod router;
mod state;
#[cfg(not(target_arch = "wasm32"))]
mod statics;
#[cfg(not(target_arch = "wasm32"))]
mod ui_rules;
