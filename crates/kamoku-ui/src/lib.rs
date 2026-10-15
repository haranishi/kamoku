// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rust-authored bookkeeping UI. The browser remains responsible for native
//! text input, semantic HTML, and layout; no accounting calculation lives here.

// Browser state is only instantiated on wasm32; native builds exercise its tests.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod model;

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod input;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod choices;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod interaction;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod notification;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod phrase;

#[cfg(target_arch = "wasm32")]
mod app;

// Compile feature models natively without instantiating browser-only DOM types.
// Their browser callers are compiled and linted separately on wasm32.
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/entry/model.rs"]
mod entry_model;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/home/chart_model.rs"]
mod home_chart_model;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/imports/model.rs"]
mod imports_model;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/inbox/model.rs"]
mod inbox_model;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/journal/model.rs"]
mod journal_model;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/journal/return_path.rs"]
mod journal_return_path;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/masters/model.rs"]
mod master_model;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/reports/period.rs"]
mod report_period;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/reports/tree.rs"]
mod report_tree;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/settings/model.rs"]
mod settings_model;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "app/screens/setup/model.rs"]
mod setup_model;
