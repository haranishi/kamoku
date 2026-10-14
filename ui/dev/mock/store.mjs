// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 模擬の台帳。登録・訂正・取消をイベントとして追記し、現在の状態はイベントの列から計算する。
// ハッシュは本物の符号化ではなく、JSONのSHA-256を連ねただけ（画面の確認用）。

import { createHash } from 'node:crypto';
import { compose, ApiFail } from './compose.mjs';

const ZERO_HASH = '0'.repeat(64);

export function createStore({ settings, accounts, partners, templates, drafts, idSource, clock }) {
  const db = {
    settings,
    accounts,
    partners,
    templates,
    drafts,
    events: [],
    nextPartnerId: partners.length + 1,
    nextTemplateId: templates.length + 1,
    nextDraftId: Math.max(0, ...drafts.map((d) => d.id)) + 1,
    nextBatchId: Math.max(0, ...drafts.map((d) => d.batch_id)) + 1,
    // 取込の途中のファイル（30分で消える）、口座ごとに覚えた列の対応、取込の記録。
    uploads: new Map(),
    profiles: new Map(),
    batches: [],
    broken: null,
  };
  const ctx = () => ({ settings: db.settings, accounts: new Map(db.accounts.map((a) => [a.code, a])) });

  function append(kind, entryId, revision, recordedAt, reason, entry) {
    const prev = db.events.length ? db.events[db.events.length - 1].hash : ZERO_HASH;
    const seq = db.events.length + 1;
    const body = JSON.stringify({ seq, kind, entryId, revision, recordedAt, reason, entry });
    const hash = createHash('sha256').update(prev).update(body).digest('hex');
    const ev = { seq, kind, entry_id: entryId, revision, recorded_at: recordedAt, reason, hash, entry };
    db.events.push(ev);
    return ev;
  }

  function rememberPartner(name) {
    if (!name || db.partners.some((p) => p.name === name)) return;
    db.partners.push({ id: db.nextPartnerId++, name, reading: '', invoice_number: null, note: '', enabled: true });
  }

  /** 入力から仕訳を作る（source は manual / demo / opening / import）。meta は付随情報に足す（取込の draft など）。 */
  function post(input, { source = 'manual', recordedAt = clock(), opening = false, meta = {} } = {}) {
    const c = opening ? input : compose(input, ctx());
    const entry = {
      date: c.date,
      kind: opening ? 'opening' : c.kind,
      description: c.description,
      partner: c.partner || '',
      lines: c.lines,
      meta: { ...c.meta, ...meta, kind: opening ? 'opening' : c.kind, source },
      input: opening || c.kind === 'journal' ? null : simpleInput(input, c),
    };
    rememberPartner(entry.partner);
    const id = idSource();
    append('posted', id, 1, recordedAt, null, entry);
    return represent(id, true);
  }

  // 画面が見ていた版と、いまの版が同じかを確かめる（AC-06.6）。古い画面からの上書きを防ぐ。
  function requireRevision(cur, expected) {
    if (!Number.isInteger(expected)) {
      throw new ApiFail(400, 'bad_request', '読み込んだときの版（expected_revision）が付いていません。画面を読み込み直してください。', 'expected_revision');
    }
    if (expected !== cur.revision) {
      throw new ApiFail(409, 'stale_revision', `この仕訳は、ほかの場所で更新されています（いまは第${cur.revision}版）。読み込み直してから、もう一度操作してください。`, null);
    }
  }

  // 期首残高の仕訳は、仕訳の訂正・取消では直さない（PUT /opening から見直す。api-entries.md）。
  function guardOpening(cur) {
    if (cur.entry.meta && cur.entry.meta.kind === 'opening') {
      throw new ApiFail(422, 'bad_request', '期首残高は、設定の「期首残高」から見直してください。', null);
    }
  }

  function revise(id, input, reason, { recordedAt = clock(), expectedRevision } = {}) {
    const cur = state(id);
    if (!cur) throw new ApiFail(404, 'not_found', '仕訳が見つかりません。一覧から選び直してください。', null);
    guardOpening(cur);
    requireRevision(cur, expectedRevision);
    if (cur.voided) throw new ApiFail(409, 'already_voided', 'この仕訳は取消済みのため、訂正できません。', null);
    requireReason(reason);
    const locked = db.settings.locked_until;
    if (locked && cur.entry.date <= locked) {
      throw new ApiFail(422, 'locked_period', `${locked} 以前は締めてあるため、この仕訳は直せません。`, 'date');
    }
    const c = compose(input, ctx());
    const entry = {
      date: c.date,
      kind: c.kind,
      description: c.description,
      partner: c.partner || '',
      lines: c.lines,
      meta: { ...c.meta, source: cur.entry.meta.source || 'manual' },
      input: c.kind === 'journal' ? null : simpleInput(input, c),
    };
    rememberPartner(entry.partner);
    append('revised', id, cur.revision + 1, recordedAt, reason.trim(), entry);
    return represent(id, true);
  }

  function voidEntry(id, reason, { recordedAt = clock(), expectedRevision, allowOpening = false } = {}) {
    const cur = state(id);
    if (!cur) throw new ApiFail(404, 'not_found', '仕訳が見つかりません。一覧から選び直してください。', null);
    if (!allowOpening) guardOpening(cur);
    requireRevision(cur, expectedRevision);
    if (cur.voided) throw new ApiFail(409, 'already_voided', 'この仕訳はすでに取消済みです。', null);
    requireReason(reason);
    const locked = db.settings.locked_until;
    if (locked && cur.entry.date <= locked) {
      throw new ApiFail(422, 'locked_period', `${locked} 以前は締めてあるため、この仕訳は取り消せません。`, 'date');
    }
    append('voided', id, cur.revision + 1, recordedAt, reason.trim(), null);
    return represent(id, true);
  }

  /** 期首残高の仕訳を作るか、見直す（PUT /opening から。理由は「期首残高の見直し」）。 */
  function writeOpening(date, lines, { recordedAt = clock() } = {}) {
    const entry = { date, kind: 'opening', description: '期首残高', partner: '', lines, meta: { kind: 'opening', source: 'opening' }, input: null };
    const cur = openingState();
    if (!cur) {
      const id = idSource();
      append('posted', id, 1, recordedAt, null, entry);
      return id;
    }
    append('revised', cur.id, cur.revision + 1, recordedAt, '期首残高の見直し', entry);
    return cur.id;
  }

  /** いま有効な期首残高の仕訳（無ければ null）。 */
  function openingState() {
    return all().find((s) => !s.voided && s.entry.meta && s.entry.meta.kind === 'opening') || null;
  }

  function requireReason(reason) {
    if (typeof reason !== 'string' || reason.trim() === '') {
      throw new ApiFail(422, 'reason_required', '理由を入れてください。あとで履歴を見たときに、直した訳が分かるようにするためです。', 'reason');
    }
  }

  /** 1つの仕訳の、いまの状態。 */
  function state(id) {
    let s = null;
    let no = 0;
    for (const ev of db.events) {
      if (ev.kind === 'posted') no += 1;
      if (ev.entry_id !== id) continue;
      if (ev.kind === 'posted') s = { no, entry: ev.entry, revision: 1, voided: false, recordedAt: ev.recorded_at, lastReason: null };
      else if (ev.kind === 'revised') Object.assign(s, { entry: ev.entry, revision: ev.revision, recordedAt: ev.recorded_at, lastReason: ev.reason });
      else Object.assign(s, { voided: true, revision: ev.revision, recordedAt: ev.recorded_at, lastReason: ev.reason });
    }
    return s;
  }

  /** すべての仕訳のいまの状態（登録順）。 */
  function all() {
    const map = new Map();
    let no = 0;
    for (const ev of db.events) {
      if (ev.kind === 'posted') {
        no += 1;
        map.set(ev.entry_id, { id: ev.entry_id, no, entry: ev.entry, revision: 1, voided: false, recordedAt: ev.recorded_at, lastReason: null });
      } else {
        const s = map.get(ev.entry_id);
        if (ev.kind === 'revised') Object.assign(s, { entry: ev.entry, revision: ev.revision, recordedAt: ev.recorded_at, lastReason: ev.reason });
        else Object.assign(s, { voided: true, revision: ev.revision, recordedAt: ev.recorded_at, lastReason: ev.reason });
      }
    }
    return [...map.values()];
  }

  function toRepr(s, withDetail) {
    const e = s.entry;
    const total = e.lines.filter((l) => l.side === 'debit').reduce((a, l) => a + l.amount, 0);
    const repr = {
      id: s.id,
      no: s.no,
      date: e.date,
      kind: e.kind,
      description: e.description,
      partner: e.partner,
      total,
      revision: s.revision,
      state: s.voided ? 'voided' : 'active',
      recorded_at: s.recordedAt,
      last_reason: s.lastReason,
      lines: e.lines.map((l) => ({ ...l, account_name: nameOf(l.account) })),
      meta: e.meta,
    };
    if (withDetail) repr.input = e.input;
    return repr;
  }

  function represent(id, withDetail) {
    const s = state(id);
    if (!s) return null;
    return toRepr({ ...s, id }, withDetail);
  }

  function nameOf(code) {
    const a = db.accounts.find((x) => x.code === code);
    return a ? a.name : code;
  }

  function history(id) {
    return db.events
      .filter((ev) => ev.entry_id === id)
      .map((ev) => ({
        seq: ev.seq,
        kind: ev.kind,
        revision: ev.revision,
        recorded_at: ev.recorded_at,
        reason: ev.reason,
        hash: ev.hash,
        lines: ev.entry ? ev.entry.lines.map((l) => ({ ...l, account_name: nameOf(l.account) })) : null,
      }));
  }

  return { db, ctx, post, revise, voidEntry, writeOpening, openingState, state, all, toRepr, represent, history, nameOf };
}

// 単純な形の入力を、保存した明細に合わせて残す（訂正の画面の初期値）。
function simpleInput(input, c) {
  return {
    kind: c.kind,
    date: c.date,
    amount: input.amount,
    account: c.kind === 'transfer' ? null : input.account,
    wallet: input.wallet,
    to_wallet: c.kind === 'transfer' ? input.to_wallet : null,
    tax_code: c.kind === 'transfer' ? null : input.tax_code ?? null,
    partner: c.partner || '',
    description: c.description,
    business_pct: c.kind === 'expense' ? Number(c.meta.business_pct) : 100,
    memo: String(input.memo ?? ''),
  };
}
