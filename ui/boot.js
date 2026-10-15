// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// The generated adapter loads the Rust application. UI behavior lives in kamoku-ui.
import init from './rust/pkg/kamoku_ui.js';

init().catch(() => {
  const main = document.getElementById('main');
  const message = document.createElement('p');
  message.setAttribute('role', 'alert');
  message.textContent = '画面を読み込めませんでした。WebAssemblyに対応したブラウザで、ページを再読み込みしてください。';
  main?.replaceChildren(message);
});
