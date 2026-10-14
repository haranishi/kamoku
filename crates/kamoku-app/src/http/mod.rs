// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 手元のブラウザとだけ話す、小さな HTTP/1.1 の受け口。標準ライブラリだけで書いてある。
//!
//! # 自作にした理由
//!
//! - 最初の設計は `tiny_http` 0.12 だった。0.12.0（最新版）は、`Content-Length` に巨大な値
//!   （例: 4611686018427387904）を書いた要求1つで、プロセスごと異常終了する（実測。読み残した本文の
//!   長さぶんを一度に確保するため）。合言葉の検査より前に起きるので、防ぎようがない。
//! - `tiny_http` には、読み書きの時間切れも、同時接続の数の上限も無い。本文を途中で止める接続だけで、
//!   処理のスレッドが止まる（実測）。
//! - 代わりの `axum` は、`tokio` を含めて依存が約50クレートになり、依存を絞る方針に合わない。
//!
//! # 扱う範囲（意図して狭くしてある）
//!
//! - HTTP/1.1 と 1.0。メソッドは GET・HEAD・POST・PUT・PATCH・DELETE だけ（ほかは405）。
//! - 1つの接続で1つの要求だけ。応答には毎回 `Connection: close` と `Content-Length` を付けて閉じる。
//!   後に続く要求（パイプライン）は読まない。
//! - 本文は `Content-Length` のものだけ。`Transfer-Encoding` があれば501で断る。
//! - 本文の長さは、読む前に経路ごとの上限と比べ、超えたら413を返して閉じる。本文は受け取らない
//!   （応答が相手に届くよう、閉じる前に既定で最大1秒・1MiBだけ読み捨てる。[`Limits`] の `linger`）。
//! - `Expect: 100-continue` は、見出しの検査を通ったときだけ `100 Continue` を返してから本文を読む。
//! - 要求行・見出しの大きさと数、読み書きの時間切れ、同時接続の数、処理のスレッドの数に上限がある
//!   （[`Limits`]）。
//!
//! # 扱わないもの
//!
//! TLS、HTTP/2、接続の使い回し（keep-alive）、パイプライン、`chunked` の本文、圧縮、範囲の要求。
//! 相手は同じパソコンのブラウザと試験の道具だけなので、要らない。
//!
//! 仕訳・取込・帳票の処理は、この受け口に依存しない関数として [`Handler`] の向こう側に置く。
//! 受け口を差し替えるときは、このモジュールだけを替える。

mod conn;
mod request;
mod response;
mod server;

pub use request::{Head, Method, Request};
pub use response::{Response, reason_phrase};
pub use server::{Handler, Limits, Running, start};
