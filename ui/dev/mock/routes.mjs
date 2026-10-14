// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// /api/v1 の応え。形は docs/spec/api-*.md と同じにする（本物に付け替えたとき、画面を直さずに済むように）。
// U2 は、ここに未確認・取込・帳票の応えを足す。

import { compose, ApiFail } from './compose.mjs';
import { listEntries, walletBalances, dashboard } from './reports.mjs';
import { MORE_ROUTES, MORE_NOT_WRITES } from './routes-more.mjs';

const READ_ONLY_MESSAGE = '台帳の検証で不正が見つかったため、読み取り専用で開いています。書き込みはできません。';
const isDate = (s) => typeof s === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(s);

function statusBody(store, opts) {
  const ev = store.db.events;
  const head = ev[ev.length - 1];
  const drafts = store.db.drafts;
  return {
    app: 'kamoku',
    version: '0.1.0-alpha',
    schema: 1,
    mode: opts.demo ? 'demo' : 'normal',
    setup_done: store.db.settings.setup_done,
    ledger: {
      events: ev.length,
      // 読み取り専用のときは、壊れた連番の手前までを読み込んだことにする（api-common.md）。
      events_in_db: ev.length,
      events_loaded: store.db.broken ? Math.max(store.db.broken.seq - 1, 0) : ev.length,
      entries: ev.filter((e) => e.kind === 'posted').length,
      head_seq: head ? head.seq : 0,
      head_hash: head ? head.hash : '0'.repeat(64),
      last_recorded_at: head ? head.recorded_at : null,
      verified: !store.db.broken,
      error: store.db.broken,
    },
    inbox: {
      draft: drafts.filter((d) => d.status === 'draft').length,
      ignored: drafts.filter((d) => d.status === 'ignored').length,
    },
  };
}

function putSettings(store, body) {
  const s = body || {};
  if (!['exempt', 'taxable'].includes(s.tax_mode)) throw new ApiFail(400, 'bad_request', '課税の区分は exempt か taxable にしてください。', 'tax_mode');
  if (!['floor', 'round', 'ceil'].includes(s.rounding)) throw new ApiFail(400, 'bad_request', '端数処理は floor・round・ceil のどれかにしてください。', 'rounding');
  if (s.invoice_number !== null && s.invoice_number !== undefined && s.invoice_number !== '') {
    if (s.tax_mode !== 'taxable') throw new ApiFail(422, 'bad_invoice_number', '登録番号は、課税事業者の設定のときだけ入れられます。', 'invoice_number');
    if (!/^T\d{13}$/.test(s.invoice_number)) {
      throw new ApiFail(422, 'bad_invoice_number', '登録番号は、半角の T と13桁の数字で入れてください（例: T1234567890123）。', 'invoice_number');
    }
  }
  if (!isDate(s.opened_on)) throw new ApiFail(400, 'bad_request', '開業日は YYYY-MM-DD の形にしてください。', 'opened_on');
  if (s.locked_until !== null && s.locked_until !== undefined && !isDate(s.locked_until)) {
    throw new ApiFail(400, 'bad_request', '締めた日は YYYY-MM-DD の形にしてください。', 'locked_until');
  }
  store.db.settings = {
    business_name: String(s.business_name ?? ''),
    owner_name: String(s.owner_name ?? ''),
    tax_mode: s.tax_mode,
    invoice_number: s.invoice_number || null,
    rounding: s.rounding,
    opened_on: s.opened_on,
    locked_until: s.locked_until || null,
    setup_done: Boolean(s.setup_done),
  };
  return store.db.settings;
}

// 再送防止の印（client_id）。同じ値が10分以内に来たら、作らずに最初の結果を返す（AC-02.7）。
const CLIENT_ID_TTL_MS = 10 * 60 * 1000;
const recentClientIds = new Map();

function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.keys(value)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canonical(value[k])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function createEntry(store, body) {
  const clientId = body && typeof body.client_id === 'string' && body.client_id ? body.client_id : null;
  const now = Date.now();
  for (const [k, v] of recentClientIds) if (now - v.at > CLIENT_ID_TTL_MS) recentClientIds.delete(k);
  if (clientId && recentClientIds.has(clientId)) {
    const seen = recentClientIds.get(clientId);
    if (seen.key !== canonical(body)) {
      throw new ApiFail(409, 'client_id_reused', '同じ送信の印で、違う内容が届きました。画面を読み込み直してから、もう一度保存してください。', null);
    }
    return [200, store.represent(seen.id, true)];
  }
  const created = store.post(body);
  if (clientId) recentClientIds.set(clientId, { at: now, key: canonical(body), id: created.id });
  return [201, created];
}

function previewBody(store, body) {
  const c = compose(body, store.ctx());
  return { lines: c.lines, total: c.total, warnings: c.warnings };
}

function detail(store, id) {
  const repr = store.represent(id, true);
  if (!repr) throw new ApiFail(404, 'not_found', '仕訳が見つかりません。一覧から選び直してください。', null);
  return { ...repr, history: store.history(id) };
}

const ROUTES = [
  ['GET', /^\/status$/, ({ store, opts }) => [200, statusBody(store, opts)]],
  [
    'POST',
    /^\/verify$/,
    ({ store, body }) => {
      if (store.db.broken) return [200, { ok: false, error: store.db.broken }];
      const ev = store.db.events;
      const noted = body && body.noted_hash ? ev.find((e) => e.hash === body.noted_hash) : undefined;
      return [
        200,
        {
          ok: true,
          events: ev.length,
          head_hash: ev.length ? ev[ev.length - 1].hash : '0'.repeat(64),
          noted: body && body.noted_hash ? { found: Boolean(noted), seq: noted ? noted.seq : null, is_head: noted ? noted.seq === ev.length : false } : null,
        },
      ];
    },
  ],
  ['GET', /^\/settings$/, ({ store }) => [200, store.db.settings]],
  ['PUT', /^\/settings$/, ({ store, body }) => [200, putSettings(store, body)]],
  [
    'GET',
    /^\/accounts$/,
    ({ store, q }) => {
      const items = store.db.accounts
        .filter((a) => (q.include_disabled === '1' ? true : a.enabled))
        .filter((a) => (q.kind ? a.kind === q.kind : true))
        .filter((a) => (q.wallet === '1' ? Boolean(a.wallet) : true))
        .sort((a, b) => a.sort - b.sort);
      return [200, { items, total: items.length }];
    },
  ],
  ['GET', /^\/wallets$/, ({ store, q, today }) => [200, walletBalances(store, isDate(q.as_of) ? q.as_of : today)]],
  [
    'GET',
    /^\/partners$/,
    ({ store, q }) => {
      const needle = (q.q || '').trim();
      const items = store.db.partners.filter((p) => !needle || p.name.includes(needle) || p.reading.includes(needle));
      return [200, { items, total: items.length }];
    },
  ],
  ['GET', /^\/templates$/, ({ store }) => [200, { items: store.db.templates, total: store.db.templates.length }]],
  [
    'POST',
    /^\/templates$/,
    ({ store, body }) => {
      const name = String(body?.name ?? '').trim();
      if (!name) throw new ApiFail(400, 'bad_request', 'テンプレートの名前を入れてください。', 'name');
      if (!body.body || typeof body.body !== 'object') throw new ApiFail(400, 'bad_request', 'テンプレートの中身がありません。', 'body');
      if (store.db.templates.some((t) => t.name === name)) {
        throw new ApiFail(409, 'duplicate_name', `「${name}」という名前のテンプレートがすでにあります。別の名前にしてください。`, 'name');
      }
      const { date: _date, ...rest } = body.body;
      const t = { id: store.db.nextTemplateId++, name, body: rest };
      store.db.templates.push(t);
      return [201, t];
    },
  ],
  [
    'DELETE',
    /^\/templates\/(\d+)$/,
    ({ store, m }) => {
      const before = store.db.templates.length;
      store.db.templates = store.db.templates.filter((t) => t.id !== Number(m[1]));
      if (store.db.templates.length === before) throw new ApiFail(404, 'not_found', 'テンプレートが見つかりません。', null);
      // 本物のサーバーは、応えに必ず長さを付けるため 204 でなく 200 を返す（spec-issues の R2b の5）。
      return [200, { id: Number(m[1]), deleted: true }];
    },
  ],
  ['GET', /^\/entries$/, ({ store, q }) => [200, listEntries(store, q)]],
  ['POST', /^\/entries\/preview$/, ({ store, body }) => [200, previewBody(store, body)]],
  ['POST', /^\/entries$/, ({ store, body }) => createEntry(store, body)],
  ['GET', /^\/entries\/(e_[0-9a-f]+)$/, ({ store, m }) => [200, detail(store, m[1])]],
  [
    'PUT',
    /^\/entries\/(e_[0-9a-f]+)$/,
    ({ store, m, body }) => [200, store.revise(m[1], body, body?.reason, { expectedRevision: body?.expected_revision })],
  ],
  [
    'POST',
    /^\/entries\/(e_[0-9a-f]+)\/void$/,
    ({ store, m, body }) => [200, store.voidEntry(m[1], body?.reason, { expectedRevision: body?.expected_revision })],
  ],
  [
    'GET',
    /^\/reports\/dashboard$/,
    ({ store, q, today }) => {
      const month = /^\d{4}-\d{2}$/.test(q.month || '') ? q.month : today.slice(0, 7);
      return [200, dashboard(store, month, today)];
    },
  ],
];

// 書き込みでも、読み取り専用のときに通すもの（台帳に何も書かない）。
const NOT_WRITES = new Set(['/entries/preview', '/verify', ...MORE_NOT_WRITES]);
const ALL_ROUTES = [...ROUTES, ...MORE_ROUTES];

/**
 * /api/v1 以下の要求に応える。raw は、ファイルの送信（application/octet-stream）のときのバイト列。
 * @returns {[number, any]} 状態と本文（204 は本文なし。書き出しは { __raw, contentType, filename }）
 */
export function handleApi({ store, opts, method, path, q, body, raw, today }) {
  const isWrite = method !== 'GET' && !NOT_WRITES.has(path);
  if (isWrite && store.db.broken) throw new ApiFail(409, 'read_only', READ_ONLY_MESSAGE, null);
  for (const [verb, re, fn] of ALL_ROUTES) {
    const m = re.exec(path);
    if (m && verb === method) return fn({ store, opts, q, body, raw, m, today });
  }
  throw new ApiFail(404, 'not_found', 'その操作はありません。アドレスを確かめてください。', null);
}
