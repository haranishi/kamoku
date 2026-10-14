// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認（下書き）の一覧・直し方・確定（api-inbox.md）。一時の道具で、本物は kamoku-app が持つ。
// 状態の移り方: draft → ignore / confirm、ignored → restore、confirmed → 仕訳が取消済みのときだけ restore。

import { ApiFail, TAX_CODES } from './compose.mjs';

const STATUSES = ['draft', 'ignored', 'confirmed'];
const isDate = (s) => typeof s === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(s) && !Number.isNaN(Date.parse(`${s}T00:00:00Z`)) && new Date(`${s}T00:00:00Z`).toISOString().slice(0, 10) === s;

function view(store, d) {
  const w = store.db.accounts.find((a) => a.code === d.wallet);
  return {
    id: d.id,
    batch_id: d.batch_id,
    wallet: d.wallet,
    wallet_name: w ? w.name : d.wallet,
    date: d.date,
    description: d.description,
    direction: d.direction,
    amount: d.amount,
    account: d.account,
    tax_code: d.tax_code,
    partner: d.partner,
    memo: d.memo,
    business_pct: d.business_pct,
    status: d.status,
    entry_id: d.entry_id,
  };
}

function find(store, id) {
  const d = store.db.drafts.find((x) => x.id === Number(id));
  if (!d) throw new ApiFail(404, 'not_found', '未確認の明細が見つかりません。一覧を読み込み直してください。', null);
  return d;
}

const confirmedFail = () => new ApiFail(409, 'already_confirmed', 'この明細は、ほかの場所で確定されています。一覧を読み込み直します。', null);

/** GET /drafts。件数（counts）は、絞り込みに関係なく帳簿全体の状態ごとの数（本物のサーバーと同じ）。 */
export function listDrafts(store, q) {
  const status = q.status || 'draft';
  if (!STATUSES.includes(status)) throw new ApiFail(400, 'bad_request', '状態は draft・ignored・confirmed のどれかにしてください。', 'status');
  const needle = (q.q || '').trim();
  const digits = needle.replace(/[,，円¥]/g, '');
  const filtered = store.db.drafts.filter((d) => {
    if (q.wallet && d.wallet !== q.wallet) return false;
    if (q.from && d.date < q.from) return false;
    if (q.to && d.date > q.to) return false;
    if (needle) {
      const hay = [d.description, d.partner, d.memo].join(' ');
      const hit = hay.includes(needle) || (/^\d+$/.test(digits) && String(d.amount).includes(digits));
      if (!hit) return false;
    }
    return true;
  });
  const counts = { draft: 0, ignored: 0, confirmed: 0 };
  for (const d of store.db.drafts) counts[d.status] += 1;
  const rows = filtered.filter((d) => d.status === status).sort((a, b) => (a.date === b.date ? a.id - b.id : a.date < b.date ? -1 : 1));
  const limit = Math.min(Math.max(Number(q.limit) || 50, 1), 500);
  const offset = Math.max(Number(q.offset) || 0, 0);
  return { items: rows.slice(offset, offset + limit).map((d) => view(store, d)), total: rows.length, counts };
}

/** PATCH /drafts/{id}。本文は date account tax_code partner memo business_pct のうち変えるもの。 */
export function patchDraft(store, id, body) {
  const d = find(store, id);
  if (d.status === 'confirmed') throw confirmedFail();
  if (d.status === 'ignored') throw new ApiFail(422, 'bad_request', '取り込まない行は直せません。「無視した行」から戻してから直してください。', null);
  const b = body || {};
  const next = { ...d };
  if ('date' in b) {
    if (!isDate(b.date)) throw new ApiFail(400, 'bad_request', '日付は YYYY-MM-DD の形の、実在する日にしてください。', 'date');
    next.date = b.date;
  }
  if ('account' in b) {
    if (b.account === null || b.account === '') next.account = null;
    else {
      const a = store.db.accounts.find((x) => x.code === b.account);
      if (!a) throw new ApiFail(422, 'unknown_account', `科目「${b.account}」が見つかりません。候補から選び直してください。`, 'account');
      if (!a.enabled) throw new ApiFail(422, 'disabled_account', `「${a.name}」は使わない設定です。勘定科目の画面で有効にするか、別の科目を選んでください。`, 'account');
      if (a.code === d.wallet) throw new ApiFail(422, 'bad_request', '取り込んだ口座と同じ科目は選べません。別の科目を選んでください。', 'account');
      next.account = a.code;
    }
  }
  if ('tax_code' in b) {
    if (b.tax_code !== null && b.tax_code !== '' && !TAX_CODES.includes(b.tax_code)) throw new ApiFail(422, 'bad_tax_code', `税区分「${b.tax_code}」は使えません。一覧から選んでください。`, 'tax_code');
    next.tax_code = b.tax_code || null;
  }
  if ('partner' in b) next.partner = String(b.partner ?? '').trim();
  if ('memo' in b) next.memo = String(b.memo ?? '');
  if ('business_pct' in b) {
    if (!Number.isInteger(b.business_pct) || b.business_pct < 0 || b.business_pct > 100) throw new ApiFail(400, 'bad_request', '事業の割合は0から100の整数にしてください。', 'business_pct');
    next.business_pct = b.business_pct;
  }
  Object.assign(d, next);
  return view(store, d);
}

/** POST /drafts/{id}/ignore */
export function ignoreDraft(store, id) {
  const d = find(store, id);
  if (d.status === 'confirmed') throw confirmedFail();
  d.status = 'ignored';
  // 応えの形は本物のサーバーと同じ（番号と状態だけ）。
  return { id: d.id, status: d.status, entry_id: null };
}

/** POST /drafts/{id}/restore */
export function restoreDraft(store, id) {
  const d = find(store, id);
  if (d.status === 'confirmed') {
    const e = d.entry_id ? store.state(d.entry_id) : null;
    if (!e || !e.voided) throw confirmedFail();
    d.entry_id = null;
  }
  d.status = 'draft';
  return { id: d.id, status: d.status, entry_id: null };
}

/**
 * POST /drafts/confirm。検査は1件ずつ、通った行だけ仕訳にする（AC-05.4、AC-05.5）。
 * 本物は通った行を1つのトランザクションで書く。模擬サーバーはメモリの中だけなので、1件ずつ書く。
 */
export function confirmDrafts(store, body) {
  const ids = Array.isArray(body?.ids) ? body.ids : null;
  if (!ids) throw new ApiFail(400, 'bad_request', '確定する明細の id を ids に入れてください。', 'ids');
  const confirmed = [];
  const failed = [];
  for (const id of ids) {
    const d = store.db.drafts.find((x) => x.id === Number(id));
    const fail = (code, message) => failed.push({ id, error: { code, message } });
    if (!d) fail('not_found', '明細が見つかりません。');
    else if (d.status === 'confirmed') fail('already_confirmed', 'この明細は、すでに確定されています。');
    else if (d.status === 'ignored') fail('bad_request', '取り込まない行は確定できません。');
    else if (!d.account) fail('missing_account', '科目が選ばれていません。');
    else {
      const input = {
        kind: d.direction === 'out' ? 'expense' : 'income',
        date: d.date,
        amount: d.amount,
        account: d.account,
        wallet: d.wallet,
        tax_code: d.tax_code,
        partner: d.partner,
        description: d.description,
        business_pct: d.direction === 'out' ? d.business_pct : 100,
        memo: d.memo,
      };
      try {
        const e = store.post(input, { source: 'import', meta: { draft: String(d.id) } });
        d.status = 'confirmed';
        d.entry_id = e.id;
        confirmed.push({ id: d.id, entry_id: e.id, no: e.no });
      } catch (err) {
        if (!(err instanceof ApiFail)) throw err;
        fail(err.code, err.message);
      }
    }
  }
  return { confirmed, failed };
}
