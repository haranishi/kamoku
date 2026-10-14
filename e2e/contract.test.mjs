// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 契約試験: 模擬サーバー（ui/dev/mock-server.mjs）と、本物のサーバーの応えの「形」が食い違っていないかを比べる。
//
// 画面は模擬サーバーを相手に作り、速く回す試験にも使っている。模擬が本物と違う形を返していると、
// 画面の試験は通るのに、本物では動かない、という穴ができる。ここでは値ではなく、キーの有無と型だけを見る
// （合成データの中身は、模擬と本物で違ってよい）。本物は KAMOKU_URL と KAMOKU_TOKEN で渡す。
//   node --test e2e/contract.test.mjs      （e2e/run-real.sh contract が、本物を立ててから呼ぶ）

import assert from 'node:assert/strict';
import net from 'node:net';
import test from 'node:test';
import { startMockServer } from '../ui/dev/mock-server.mjs';

const REAL_URL = process.env.KAMOKU_URL;
const REAL_TOKEN = process.env.KAMOKU_TOKEN;

/**
 * 食い違いを承知で許すもの。キーは「経路」（比べた場所）で、理由を必ず書く。
 * 増やすときは、画面がその違いに依存していないことを確かめてからにする。
 */
const ACCEPTED = new Map([
  // 例: ['GET /xxx .yyy', '理由'],
]);

// --- 形の取り出しと比較 -------------------------------------------------------------------------

/** 値から、型の集合・オブジェクトのキー・配列の要素の形を取り出す。配列は全要素の形を合わせる。 */
function shapeOf(value) {
  if (value === null) return { types: new Set(['null']) };
  if (Array.isArray(value)) {
    const items = value.slice(0, 300).map(shapeOf);
    return { types: new Set(['array']), item: items.length ? items.reduce(merge) : null };
  }
  if (typeof value === 'object') {
    const keys = {};
    for (const [k, v] of Object.entries(value)) keys[k] = { shape: shapeOf(v), optional: false };
    return { types: new Set(['object']), keys };
  }
  return { types: new Set([typeof value]) };
}

/** 2つの形を合わせる。片方にしか無いキーは「無いこともある」印を付ける。 */
function merge(a, b) {
  const out = { types: new Set([...a.types, ...b.types]) };
  if (a.keys || b.keys) {
    out.keys = {};
    const names = new Set([...Object.keys(a.keys || {}), ...Object.keys(b.keys || {})]);
    for (const k of names) {
      const x = a.keys?.[k];
      const y = b.keys?.[k];
      out.keys[k] = x && y ? { shape: merge(x.shape, y.shape), optional: x.optional || y.optional } : { shape: (x || y).shape, optional: true };
    }
  }
  if (a.item || b.item) out.item = a.item && b.item ? merge(a.item, b.item) : a.item || b.item;
  return out;
}

/** 本物を基準に、模擬との違いを集める。null は「中身が空だった」だけなので、どんな型とも合うことにする。 */
function diff(real, mock, path, out) {
  const rt = [...real.types].filter((t) => t !== 'null').sort();
  const mt = [...mock.types].filter((t) => t !== 'null').sort();
  if (rt.length === 0 || mt.length === 0) return;
  if (rt.join() !== mt.join()) {
    out.push(`${path}  型が違う（本物=${rt.join('|')} 模擬=${mt.join('|')}）`);
    return;
  }
  if (real.keys && mock.keys) {
    for (const [k, r] of Object.entries(real.keys)) {
      const m = mock.keys[k];
      if (!m) {
        if (!r.optional) out.push(`${path}.${k}  模擬が返さない`);
        continue;
      }
      diff(r.shape, m.shape, `${path}.${k}`, out);
    }
    for (const [k, m] of Object.entries(mock.keys)) {
      if (!real.keys[k] && !m.optional) out.push(`${path}.${k}  本物が返さない（模擬だけが返す）`);
    }
  }
  if (real.item && mock.item) diff(real.item, mock.item, `${path}[]`, out);
}

// --- 2つのサーバーを呼ぶ ------------------------------------------------------------------------

async function freePort() {
  return new Promise((resolve, reject) => {
    const s = net.createServer();
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
    s.on('error', reject);
  });
}

/** 本物のサーバーへ入る。合言葉つきのアドレスを開いて、Cookie を受け取る。 */
async function signIn(url, token) {
  const res = await fetch(`${url}/?t=${token}`, { redirect: 'manual' });
  const cookie = res.headers.getSetCookie().map((c) => c.split(';')[0]).join('; ');
  assert.ok(cookie, '本物のサーバーが Cookie を返しませんでした（KAMOKU_TOKEN が違うかもしれません）');
  return cookie;
}

function client(base, cookie) {
  return async (method, path, body) => {
    const headers = cookie ? { Cookie: cookie } : {};
    if (method !== 'GET') Object.assign(headers, { Origin: new URL(base).origin, 'Content-Type': 'application/json' });
    const res = await fetch(`${base}/api/v1${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
    const text = await res.text();
    let json = null;
    try {
      json = text ? JSON.parse(text) : null;
    } catch {
      json = { __text: text.slice(0, 80) };
    }
    return { status: res.status, json };
  };
}

if (!REAL_URL || !REAL_TOKEN) {
  test('契約試験（KAMOKU_URL と KAMOKU_TOKEN が無いので省く）', { skip: '本物のサーバーを立てて、アドレスと合言葉を渡してください（e2e/run-real.sh contract）' }, () => {});
} else {
  const mockPort = await freePort();
  const mockServer = startMockServer(['--port', String(mockPort), '--quiet', '--today', '2026-10-14']);
  await new Promise((r) => mockServer.once('listening', r));
  const real = client(REAL_URL, await signIn(REAL_URL, REAL_TOKEN));
  const mock = client(`http://127.0.0.1:${mockPort}`, '');

  test.after(() => mockServer.close());

  // 画面が実際に読む問い合わせ（ui/js の api.get）。期間は固定にして、本物のデモの今日に左右されないようにする。
  const Y = 'from=2026-01-01&to=2026-12-31';
  const READS = [
    '/status',
    '/settings',
    '/accounts?include_disabled=1',
    '/wallets',
    '/partners?limit=500',
    '/templates',
    '/opening',
    '/drafts?status=draft&limit=50',
    '/drafts?status=ignored&limit=50',
    `/entries?${Y}&limit=50`,
    '/reports/dashboard?month=2026-09',
    `/reports/trial-balance?${Y}`,
    '/reports/balance-sheet?as_of=2026-09-30',
    `/reports/income-statement?${Y}&by=month`,
    `/reports/income-statement?${Y}&by=total`,
    `/reports/ledger?account=exp_comm&${Y}`,
    `/reports/tax-summary?${Y}`,
  ];

  for (const path of READS) {
    test(`形が同じ: GET ${path}`, async () => {
      const [r, m] = await Promise.all([real('GET', path), mock('GET', path)]);
      assert.equal(m.status, r.status, `状態コードが違う（本物=${r.status} 模擬=${m.status}）`);
      const problems = [];
      diff(shapeOf(r.json), shapeOf(m.json), `GET ${path.split('?')[0]} `, problems);
      const unexpected = problems.filter((p) => ![...ACCEPTED.keys()].some((k) => p.startsWith(k)));
      assert.deepEqual(unexpected, [], `模擬と本物の形が食い違っています:\n  ${unexpected.join('\n  ')}`);
    });
  }

  // 仕訳の詳細は、仕訳の種類によって付くキーが違う（按分の割合など）。先頭30件を重ねて、どちらも出しうるキーを揃える。
  test('形が同じ: GET /entries/{id}（一覧の先頭30件を重ねる）', async () => {
    const shapes = await Promise.all(
      [real, mock].map(async (call) => {
        const items = (await call('GET', `/entries?${Y}&limit=30`)).json.items;
        const details = await Promise.all(items.map((e) => call('GET', `/entries/${e.id}`)));
        for (const d of details) assert.equal(d.status, 200);
        return details.map((d) => shapeOf(d.json)).reduce(merge);
      }),
    );
    const problems = [];
    diff(shapes[0], shapes[1], 'GET /entries/{id} ', problems);
    assert.deepEqual(problems, [], `模擬と本物の形が食い違っています:\n  ${problems.join('\n  ')}`);
  });

  // 一括確定は、行ごとの失敗を 200 の中の failed に入れて返す（api-inbox.md）。その形も揃っていること。
  test('形が同じ: POST /drafts/confirm（無い下書きの行は failed に入る）', async () => {
    const body = { ids: [999999] };
    const [r, m] = await Promise.all([real('POST', '/drafts/confirm', body), mock('POST', '/drafts/confirm', body)]);
    assert.equal(r.status, 200);
    assert.equal(m.status, r.status);
    assert.equal(r.json.failed.length, 1, '本物は、無い下書きを failed に入れるはずです');
    assert.equal(m.json.failed.length, 1, '模擬は、無い下書きを failed に入れるはずです');
    assert.equal(m.json.failed[0].error.code, r.json.failed[0].error.code);
    const problems = [];
    diff(shapeOf(r.json), shapeOf(m.json), 'POST /drafts/confirm ', problems);
    assert.deepEqual(problems, [], `模擬と本物の形が食い違っています:\n  ${problems.join('\n  ')}`);
  });

  // 失敗の応え。画面は error.code と error.field で欄の下に出す文を決めるので、状態コードとコードが一致していること。
  const FAILS = [
    ['GET', '/entries/no-such-entry', undefined],
    ['GET', '/reports/trial-balance?from=2026-12-01&to=2027-01-31', undefined],
    ['GET', '/reports/balance-sheet?as_of=not-a-date', undefined],
    ['PUT', '/settings', { tax_mode: 'nope' }],
    ['POST', '/entries/preview', {}],
    ['PATCH', '/accounts/no_such_account', { enabled: false }],
  ];
  for (const [method, path, body] of FAILS) {
    test(`失敗の応えが同じ: ${method} ${path}`, async () => {
      const [r, m] = await Promise.all([real(method, path, body), mock(method, path, body)]);
      assert.ok(r.status >= 400 && r.status < 500, `本物が失敗を返すはずの要求です（${r.status}）`);
      assert.equal(m.status, r.status, `状態コードが違う（本物=${r.status} 模擬=${m.status}）`);
      assert.equal(m.json?.error?.code, r.json?.error?.code, `error.code が違う（本物=${r.json?.error?.code} 模擬=${m.json?.error?.code}）`);
      assert.equal(typeof m.json?.error?.message, 'string');
      assert.equal(typeof r.json?.error?.message, 'string');
      assert.equal(m.json?.error?.field ?? null, r.json?.error?.field ?? null, 'error.field が違う');
    });
  }
}
