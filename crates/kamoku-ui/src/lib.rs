// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rust-authored journal preview. The browser remains responsible for native
//! text input, semantic HTML, and layout; no accounting calculation lives here.

// Browser state is only instantiated on wasm32; native builds exercise its tests.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod model;

#[cfg(target_arch = "wasm32")]
mod browser;
