// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// Only initializes generated bindings. UI, networking, and state live in Rust.
import('./pkg/kamoku_ui.js').then(({ default: init }) => init()).catch(() => {
  document.getElementById('app').hidden = true;
  document.getElementById('boot-error').hidden = false;
});
