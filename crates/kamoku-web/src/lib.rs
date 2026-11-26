// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! ブラウザのお試し版の橋。会計の処理は [`kamoku_app::web`]。ここは WASM へ渡すだけ。
//! ネイティブでは空で、`cargo test --workspace` が wasm なしで通るようにしてある。

#[cfg(target_arch = "wasm32")]
mod binding;

#[cfg(target_arch = "wasm32")]
pub use binding::DemoBook;
