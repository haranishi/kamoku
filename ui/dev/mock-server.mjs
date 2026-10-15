// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 一時の模擬サーバー。本物のサーバー（kamoku-app）ができたら、ui/dev/mock/ ごと捨てる。
// Node標準の部品だけで、ui/ の静的配信と /api/v1 を返す。認証（合言葉とCookie）は省く。
//
//   node ui/dev/mock-server.mjs --port 8766
//   オプション: --today 2026-10-14  --tax-mode exempt|taxable  --demo  --broken  --empty  --fresh  --delay 400  --quiet
//   --fresh は初期設定の前の帳簿（口座なし、仕訳なし、setup_done が偽）。

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { seed } from './mock/seed.mjs';
import { handleApi } from './mock/routes.mjs';
import { ApiFail } from './mock/compose.mjs';

const UI_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const JSON_LIMIT = 1024 * 1024;
const CSV_LIMIT = 10 * 1024 * 1024;
const CSP =
  "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self' 'wasm-unsafe-eval'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";
const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.json': 'application/json; charset=utf-8',
};

function parseArgs(argv) {
  const opts = { port: 8766, host: '127.0.0.1', today: null, taxMode: 'taxable', demo: false, broken: false, empty: false, fresh: false, delay: 0, quiet: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => argv[++i];
    if (a === '--port') opts.port = Number(next());
    else if (a === '--host') opts.host = next();
    else if (a === '--today') opts.today = next();
    else if (a === '--tax-mode') opts.taxMode = next();
    else if (a === '--delay') opts.delay = Number(next());
    else if (a === '--demo') opts.demo = true;
    else if (a === '--broken') opts.broken = true;
    else if (a === '--empty') opts.empty = true;
    else if (a === '--fresh') opts.fresh = true;
    else if (a === '--quiet') opts.quiet = true;
  }
  return opts;
}

function localToday() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

function baseHeaders(extra = {}) {
  return {
    'Content-Security-Policy': CSP,
    'X-Content-Type-Options': 'nosniff',
    'Referrer-Policy': 'no-referrer',
    ...extra,
  };
}

function sendJson(res, status, body) {
  if (status === 204) {
    res.writeHead(204, baseHeaders({ 'Cache-Control': 'no-store' }));
    res.end();
    return;
  }
  const text = JSON.stringify(body);
  res.writeHead(status, baseHeaders({ 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' }));
  res.end(text);
}

function sendError(res, fail) {
  const error = { code: fail.code, message: fail.message };
  if (fail.field) error.field = fail.field;
  if (fail.line) error.line = fail.line;
  sendJson(res, fail.status, { error });
}

// 書き出し（CSV・JSON）は、ファイルとして保存されるように Content-Disposition を付ける。
function sendFile(res, file) {
  const name = encodeURIComponent(file.filename);
  res.writeHead(
    200,
    baseHeaders({ 'Content-Type': file.contentType, 'Cache-Control': 'no-store', 'Content-Disposition': `attachment; filename="${file.filename}"; filename*=UTF-8''${name}`, ...file.headers }),
  );
  res.end(file.__raw);
}

function readRaw(req, limit, what) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on('data', (c) => {
      size += c.length;
      if (size > limit) {
        reject(new ApiFail(413, 'too_large', `送る内容が大きすぎます（${what}まで）。`, null));
        req.destroy();
      } else chunks.push(c);
    });
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', reject);
  });
}

async function readBody(req) {
  const text = (await readRaw(req, JSON_LIMIT, '1MB')).toString('utf8');
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    throw new ApiFail(400, 'bad_request', '本文がJSONとして読めません。', null);
  }
}

async function serveStatic(res, urlPath) {
  const rel = urlPath === '/' ? 'index.html' : decodeURIComponent(urlPath.slice(1));
  const allowed = ['index.html', 'favicon.svg', 'boot.js', 'rust/index.html', 'rust/boot.js', 'rust/preview.css', 'rust/pkg/kamoku_ui.js', 'rust/pkg/kamoku_ui_bg.wasm'].includes(rel) || /^css\/[a-z][a-z0-9-]*\.css$/.test(rel);
  const file = path.resolve(UI_ROOT, rel);
  if (!allowed || !file.startsWith(UI_ROOT + path.sep)) {
    res.writeHead(404, baseHeaders({ 'Content-Type': 'text/plain; charset=utf-8' }));
    res.end('not found');
    return;
  }
  try {
    const data = await readFile(file);
    const type = MIME[path.extname(file)] || 'application/octet-stream';
    // 手元で編集して再読込で確かめられるように、キャッシュさせない。
    res.writeHead(200, baseHeaders({ 'Content-Type': type, 'Cache-Control': 'no-cache' }));
    res.end(data);
  } catch {
    res.writeHead(404, baseHeaders({ 'Content-Type': 'text/plain; charset=utf-8' }));
    res.end('not found');
  }
}

/** サーバーを作って待ち受ける。試験からも呼べるように関数にしてある。 */
export function startMockServer(argv = []) {
  const opts = parseArgs(argv);
  const today = opts.today || localToday();
  const store = seed({ today, taxMode: opts.taxMode, empty: opts.empty, fresh: opts.fresh });
  if (opts.broken) store.db.broken = { seq: Math.min(55, store.db.events.length), kind: 'bad_hash', rule: null };

  const server = createServer(async (req, res) => {
    const [pathname, search = ''] = String(req.url || '/').split('?');
    const url = { pathname, searchParams: new URLSearchParams(search) };
    const host = req.headers.host || '';
    if (host !== `127.0.0.1:${opts.port}` && host !== `localhost:${opts.port}`) {
      return sendError(res, new ApiFail(421, 'bad_host', '開いたアドレスが違います。起動のときに出たアドレスから開いてください。', null));
    }
    if (!url.pathname.startsWith('/api/v1/')) return serveStatic(res, url.pathname);

    const method = req.method || 'GET';
    if (method !== 'GET' && req.headers.origin) {
      let sameOrigin = false;
      try {
        const o = new URL(req.headers.origin);
        sameOrigin = o.protocol === 'http:' && o.host === host;
      } catch {
        sameOrigin = false;
      }
      if (!sameOrigin) return sendError(res, new ApiFail(403, 'forbidden_origin', 'ほかのページからの操作は受け付けません。', null));
    }
    try {
      const apiPath = url.pathname.slice('/api/v1'.length);
      // ファイルの送信だけはバイト列のまま受け取る（application/octet-stream、10MBまで）。
      const isUpload = method === 'POST' && apiPath === '/imports/upload';
      const raw = isUpload ? await readRaw(req, CSV_LIMIT, '10MB') : null;
      const body = method === 'GET' || isUpload ? null : await readBody(req);
      if (opts.delay) await new Promise((r) => setTimeout(r, opts.delay));
      const q = Object.fromEntries(url.searchParams);
      const [status, json] = handleApi({ store, opts, method, path: apiPath, q, body, raw, today });
      if (!opts.quiet) console.log(`${method} ${url.pathname} -> ${status}`);
      if (json && json.__raw) sendFile(res, json);
      else sendJson(res, status, json);
    } catch (e) {
      if (e instanceof ApiFail) return sendError(res, e);
      console.error(e);
      sendError(res, new ApiFail(500, 'internal', 'サーバーで想定外の失敗がありました。もう一度試してください。', null));
    }
  });
  server.listen(opts.port, opts.host, () => {
    if (!opts.quiet) console.log(`kamoku 模擬サーバー: ${opts.host}:${opts.port}（今日=${today}、仕訳 ${store.all().length}件）`);
  });
  return server;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  startMockServer(process.argv.slice(2));
}
