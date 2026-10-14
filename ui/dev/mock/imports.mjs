// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込（api-inbox.md）: ファイルを受け取り、列の対応を推測し、取り込む前に確かめ、下書きに入れる。
// 重複の判定は data-model.md の決まり4（指紋と出現番号）。一時の道具で、本物は kamoku-app が持つ。

import { createHash, randomBytes } from 'node:crypto';
import { ApiFail } from './compose.mjs';
import { decode, detectDelimiter, parseCsv } from './csvio.mjs';
import { guessMapping, readRow } from './importvalues.mjs';

const UPLOAD_TTL_MS = 30 * 60 * 1000;
const PREVIEW_LIMIT = 500;
const SAMPLE_ROWS = 20;
const ENCODINGS = ['auto', 'utf-8', 'shift_jis'];

/** 指紋（口座・日付・符号つき金額・摘要。取引番号 ref があれば口座と取引番号）。 */
export function fingerprint(wallet, row) {
  const body = row.ref ? `${wallet}\u0000ref\u0000${row.ref}` : `${wallet}\u0000${row.date}\u0000${row.direction === 'in' ? row.amount : -row.amount}\u0000${row.description}`;
  return createHash('sha256').update(body).digest('hex');
}

function requireWallet(store, code) {
  const a = store.db.accounts.find((x) => x.code === code);
  if (!a || !a.wallet) throw new ApiFail(422, 'unknown_account', '取り込む口座を選んでください。', 'wallet');
  if (!a.enabled) throw new ApiFail(422, 'disabled_account', `「${a.name}」は使わない設定です。口座の画面で有効にしてください。`, 'wallet');
  return a;
}

function getUpload(store, id) {
  const now = Date.now();
  for (const [k, u] of store.db.uploads) if (now - u.at > UPLOAD_TTL_MS) store.db.uploads.delete(k);
  const u = store.db.uploads.get(id);
  if (!u) throw new ApiFail(404, 'not_found', '取り込むファイルの期限が切れました。もう一度ファイルを選んでください。', 'upload_id');
  return u;
}

function checkMapping(mapping, width) {
  const ok = (c) => Number.isInteger(c) && c >= 0 && c < width;
  const m = mapping || {};
  const a = m.amount || {};
  const amountOk = a.mode === 'split' ? ok(a.out_col) && ok(a.in_col) && a.out_col !== a.in_col : a.mode === 'signed' && ok(a.col) && ['positive', 'negative'].includes(a.inflow);
  if (!Number.isInteger(m.skip_rows) || m.skip_rows < 0 || !ok(m.date_col) || !Array.isArray(m.desc_cols) || !m.desc_cols.every(ok) || !amountOk || (m.id_col !== null && m.id_col !== undefined && !ok(m.id_col))) {
    throw new ApiFail(400, 'bad_request', '列の対応が正しくありません。日付と金額の列を選び直してください。', 'mapping');
  }
  return { skip_rows: m.skip_rows, date_col: m.date_col, desc_cols: m.desc_cols, id_col: m.id_col ?? null, amount: a };
}

/** POST /imports/upload?name=&encoding=&wallet= */
export function upload(store, q, raw) {
  if (!raw || !raw.length) throw new ApiFail(400, 'invalid_csv', 'ファイルが空です。明細のCSVを選んでください。', null);
  const encoding = q.encoding || 'auto';
  if (!ENCODINGS.includes(encoding)) throw new ApiFail(400, 'bad_request', '文字コードは auto・utf-8・shift_jis のどれかにしてください。', 'encoding');
  const d = decode(raw, encoding);
  const delimiter = detectDelimiter(d.text);
  const rows = parseCsv(d.text, delimiter);
  if (!rows.length) throw new ApiFail(400, 'invalid_csv', 'ファイルに読める行がありません。明細のCSVを選んでください。', null);
  const guess = guessMapping(rows);
  const id = `u_${randomBytes(8).toString('hex')}`;
  store.db.uploads.set(id, { at: Date.now(), rows, filename: String(q.name || 'meisai.csv') });
  const skip = guess ? guess.skip_rows : 0;
  return {
    upload_id: id,
    filename: String(q.name || 'meisai.csv'),
    encoding: d.encoding,
    delimiter,
    rows: rows.length,
    header: guess ? rows[skip - 1].cells : null,
    sample: rows.slice(skip, skip + SAMPLE_ROWS).map((r) => r.cells),
    guess,
    warnings: d.warnings,
    saved: q.wallet ? store.db.profiles.get(q.wallet) || null : null,
  };
}

// 読めた行に、ファイルの中の出現番号を振り、すでにある下書きと突き合わせる。
function evaluate(store, wallet, u, mapping) {
  const width = Math.max(...u.rows.map((r) => r.cells.length));
  const m = checkMapping(mapping, width);
  const seen = new Map();
  const existing = new Set(store.db.drafts.map((d) => `${d.fingerprint}#${d.occurrence}`));
  const out = u.rows.slice(m.skip_rows).map((row) => {
    const r = readRow(row, m);
    if (r.reason) return { line: r.line, date: null, description: null, direction: null, amount: null, status: 'invalid', reason: r.reason, forceable: false };
    const fp = fingerprint(wallet, r);
    const occurrence = (seen.get(fp) || 0) + 1;
    seen.set(fp, occurrence);
    const dup = existing.has(`${fp}#${occurrence}`);
    return {
      line: r.line,
      date: r.date,
      description: r.description,
      direction: r.direction,
      amount: r.amount,
      status: dup ? 'duplicate' : 'new',
      reason: dup ? '同じ口座・日付・金額・摘要の明細が、すでに取り込んであります。別の取引なら「この行も取り込む」を選んでください。' : null,
      forceable: dup,
      fp,
      occ: occurrence,
    };
  });
  return { mapping: m, rows: out };
}

const publicRow = ({ fp: _fp, occ: _occ, ...r }) => r;

/** POST /imports/preview */
export function preview(store, body) {
  const u = getUpload(store, body?.upload_id);
  requireWallet(store, body?.wallet);
  const { rows } = evaluate(store, body.wallet, u, body.mapping);
  const counts = { new: 0, duplicate: 0, invalid: 0 };
  for (const r of rows) counts[r.status] += 1;
  return { rows: rows.slice(0, PREVIEW_LIMIT).map(publicRow), counts, truncated: rows.length > PREVIEW_LIMIT };
}

/** POST /imports/commit */
export function commit(store, body) {
  const u = getUpload(store, body?.upload_id);
  const wallet = requireWallet(store, body?.wallet);
  const { mapping, rows } = evaluate(store, body.wallet, u, body.mapping);
  const force = new Set(Array.isArray(body.force_lines) ? body.force_lines : []);
  const batchId = store.db.nextBatchId++;
  let created = 0;
  const invalid = rows.filter((r) => r.status === 'invalid').length;
  const duplicates = rows.filter((r) => r.status === 'duplicate' && !force.has(r.line)).length;
  // 新規の行を先に入れ、選んだ重複の行は後に入れる。重複の出現番号は、その指紋の最大の番号+1
  // （data-model.md の決まり4）なので、同じファイルの新規の行の番号と重ならないようにするため。
  const take = [...rows.filter((r) => r.status === 'new'), ...rows.filter((r) => r.status === 'duplicate' && force.has(r.line))];
  for (const r of take) {
    const same = store.db.drafts.filter((d) => d.fingerprint === r.fp).map((d) => d.occurrence);
    const occurrence = r.status === 'duplicate' ? Math.max(0, ...same) + 1 : r.occ;
    store.db.drafts.push({
      id: store.db.nextDraftId++,
      batch_id: batchId,
      wallet: wallet.code,
      date: r.date,
      description: r.description,
      direction: r.direction,
      amount: r.amount,
      account: null,
      tax_code: null,
      partner: '',
      memo: '',
      business_pct: 100,
      status: 'draft',
      entry_id: null,
      fingerprint: r.fp,
      occurrence,
    });
    created += 1;
  }
  if (body.save_profile) store.db.profiles.set(wallet.code, mapping);
  store.db.batches.push({ id: batchId, wallet: wallet.code, filename: u.filename, recorded_at: Math.floor(Date.now() / 1000), created, duplicates, invalid });
  return { batch_id: batchId, created, duplicates, invalid };
}
