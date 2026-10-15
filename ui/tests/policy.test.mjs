// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { checkSource, uiSources } from '../dev/check-policy.mjs';

test('Rust/HTML/CSS policy rejects executable text, inline handlers and external requests', () => {
  for (const [file, source] of [
    ['index.html', '<script>alert(1)</script>'],
    ['index.html', '<button onclick="save()">save</button>'],
    ['index.html', '<p style="color:red">bad</p>'],
    ['boot.js', 'eval(userText);'],
    ['css/base.css', '.text { color: #fff; }'],
    ['../crates/kamoku-ui/src/app/screens/example.rs', 'element.set_inner_html(&description);'],
    ['../crates/kamoku-ui/src/app/screens/example.rs', 'window.fetch_with_str("/api/v1/status");'],
    ['../crates/kamoku-ui/src/app/screens/example.rs', 'let address = "https://outside.example/";'],
    ['../crates/kamoku-ui/src/app/screens/example.rs', 'event.is_composing();'],
  ]) assert.ok(checkSource(file, source).length > 0, `${file}: ${source}`);
});

test('policy permits the Rust API/IME boundary, text DOM, local modules and CSS tokens', () => {
  for (const [file, source] of [
    ['index.html', '<script type="module" src="boot.js"></script>'],
    ['boot.js', "import init from './rust/pkg/kamoku_ui.js';"],
    ['css/base.css', '.text { color: var(--c-text); }'],
    ['../crates/kamoku-ui/src/app/core/api.rs', 'window.fetch_with_request(&request);'],
    ['../crates/kamoku-ui/src/app/core/keys.rs', 'event.is_composing();'],
    ['../crates/kamoku-ui/src/app/core/dom.rs', 'element.set_text_content(Some(text));'],
    ['../crates/kamoku-ui/src/app/core/dom.rs', 'let namespace = "http://www.w3.org/2000/svg";'],
  ]) assert.deepEqual(checkSource(file, source), [], `${file}: ${source}`);
});

test('native hostile-input regression fixtures are not treated as outgoing network requests', () => {
  const source = 'fn valid() {}\n#[cfg(test)]\nmod tests { const HOSTILE: &str = "https://outside.example/"; }';
  assert.deepEqual(checkSource('../crates/kamoku-ui/src/input.rs', source), []);
  assert.ok(checkSource('../crates/kamoku-ui/src/input.rs', 'let address = "https://outside.example/";\n' + source).length > 0);
});

test('source inventory includes authored Rust and loaders, excludes generated adapters and old UI modules', () => {
  const files = uiSources(fileURLToPath(new URL('../', import.meta.url)));
  assert.ok(files.includes('boot.js'));
  assert.ok(files.includes('../crates/kamoku-ui/src/app/core/api.rs'));
  assert.ok(files.some((file) => file.endsWith('/screens/entry/simple.rs')));
  assert.ok(!files.some((file) => file.startsWith('rust/pkg/') || file.startsWith('js/')));
});
