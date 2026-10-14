// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// `node --test ui/tests/` の入口。Node 22 以降は、引数のディレクトリを試験ファイルに展開しないので、
// ここで同じディレクトリの *.test.mjs を読み込む。`node --test ui/tests/*.test.mjs` でも同じ結果になる。

import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);
const files = readdirSync(dir)
  .filter((name) => name.endsWith('.test.mjs'))
  .sort();
for (const name of files) {
  await import(new URL(name, dir).href);
}
