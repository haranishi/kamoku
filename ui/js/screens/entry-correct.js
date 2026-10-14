// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 訂正の差分（AC-06.4 の画面側）。元の値と、いまの入力で変わった欄を「変更前 → 変更後」で並べる。
// 保存の前に確認の画面で見せ、ほかの場所で更新されたとき（409 stale_revision）も、最新の版との差分として見せる。

import { h } from '../dom.js';
import { formatYen, formatDate } from '../format.js';
import { TAX_LABELS } from '../account-defaults.js';
import { openModal } from '../components/dialog.js';
import { banner } from '../components/banner.js';

const KIND = { expense: '支出', income: '収入', transfer: '振替', journal: '仕訳' };

/**
 * 単純な形の入力どうしの差分。
 * @param {object} before 元の input（GET /entries/{id} の input）
 * @param {object} after いまの入力
 * @param {(code: string) => string} nameOf 科目のコードから名前
 * @returns {Array<{ label: string, before: string, after: string }>}
 */
export function diffSimple(before, after, nameOf) {
  const show = {
    kind: (v) => KIND[v] || v || '（なし）',
    date: (v) => (v ? formatDate(v) : '（なし）'),
    amount: (v) => (v ? `${formatYen(v)}円` : '（なし）'),
    account: (v) => (v ? nameOf(v) : '（なし）'),
    wallet: (v) => (v ? nameOf(v) : '（なし）'),
    to_wallet: (v) => (v ? nameOf(v) : '（なし）'),
    tax_code: (v) => (v ? TAX_LABELS[v] || v : '対象外'),
    partner: (v) => v || '（なし）',
    description: (v) => v || '（なし）',
    business_pct: (v) => `${v ?? 100}%`,
    memo: (v) => v || '（なし）',
  };
  const LABEL = { kind: '種別', date: '日付', amount: '金額', account: '科目', wallet: '口座', to_wallet: '移動先の口座', tax_code: '税区分', partner: '取引先', description: '摘要', business_pct: '事業の割合', memo: 'メモ' };
  const out = [];
  for (const key of Object.keys(LABEL)) {
    const b = before ? before[key] ?? null : null;
    const a = after ? after[key] ?? null : null;
    const norm = (v) => (key === 'business_pct' ? v ?? 100 : v === '' ? null : v);
    if (norm(b) === norm(a)) continue;
    out.push({ label: LABEL[key], before: show[key](b), after: show[key](a) });
  }
  return out;
}

function lineText(lines, nameOf) {
  return lines
    .map((l) => `${l.side === 'debit' ? '借方' : '貸方'} ${nameOf(l.account)} ${formatYen(l.amount)}円${l.tax_code ? `（${TAX_LABELS[l.tax_code] || l.tax_code}）` : ''}${l.memo ? ` ${l.memo}` : ''}`)
    .join('\n');
}

/** 仕訳の形の差分（日付・摘要・取引先・明細）。 */
export function diffJournal(before, after, nameOf) {
  const out = [];
  if (before.date !== after.date) out.push({ label: '日付', before: formatDate(before.date), after: formatDate(after.date) });
  if ((before.description || '') !== (after.description || '')) out.push({ label: '摘要', before: before.description || '（なし）', after: after.description || '（なし）' });
  if ((before.partner || '') !== (after.partner || '')) out.push({ label: '取引先', before: before.partner || '（なし）', after: after.partner || '（なし）' });
  const b = lineText(before.lines, nameOf);
  const a = lineText(after.lines, nameOf);
  if (b !== a) out.push({ label: '明細', before: b, after: a });
  return out;
}

/** 差分の表。 */
export function diffTable(diff, caption = '変わる欄') {
  return h(
    'div',
    { class: 'table-wrap' },
    h(
      'table',
      { class: 'table diff-table' },
      h('caption', null, caption),
      h('thead', null, h('tr', null, h('th', { scope: 'col' }, '欄'), h('th', { scope: 'col' }, '変更前'), h('th', { scope: 'col' }, '変更後'))),
      h('tbody', null, diff.map((d) => h('tr', null, h('th', { scope: 'row' }, d.label), h('td', { class: 'wrap diff-before' }, d.before), h('td', { class: 'wrap diff-after' }, d.after)))),
    ),
  );
}

/**
 * 訂正の確認。差分と理由を見せ、押した結果を返す。
 * @returns {Promise<boolean>}
 */
export function confirmCorrection({ no, revision, diff, reason }) {
  return new Promise((resolve) => {
    let ok = false;
    openModal({
      title: `No.${no} を訂正します`,
      body: h(
        'div',
        { class: 'stack' },
        h('p', null, `第${revision}版を残したまま、第${revision + 1}版を追記します。元の記録は履歴に残ります。`),
        diffTable(diff),
        h('p', null, h('strong', null, '理由: '), reason),
      ),
      actions: [
        { label: '戻って直す', onClick: (c) => c.close('cancel') },
        {
          label: '訂正を保存',
          variant: 'primary',
          autofocus: true,
          onClick: (c) => {
            ok = true;
            c.close('ok');
          },
        },
      ],
      onClose: () => resolve(ok),
    });
  });
}

/** 仕訳の表現（応答）から、仕訳の形の入力を作る（訂正の初期値と、差分の「変更前」に使う）。 */
export function journalInputOf(e) {
  return {
    date: e.date,
    description: e.description,
    partner: e.partner,
    lines: e.lines.map((l) => ({ side: l.side, account: l.account, amount: l.amount, tax_code: l.tax_code, memo: l.memo })),
  };
}

/**
 * ほかの場所で更新されたとき（409 stale_revision / stale）の帯。入力は残したまま、最新の版との違いを見せる。
 * 取り消されていたら、訂正できないことだけを伝える。
 */
export function staleNotice(latest, diff) {
  if (latest.state === 'voided') {
    return banner({ kind: 'danger', title: 'この仕訳は、ほかの場所で取り消されました', body: '取消済みの仕訳は訂正できません。仕訳帳で履歴を確かめてください。' });
  }
  return banner({
    kind: 'warn',
    title: `ほかの場所で更新されたため、最新の版（第${latest.revision}版）を読み込みました`,
    body: diff.length ? 'あなたの入力は残しています。下は、最新の版とあなたの入力の違いです。よければ、もう一度保存してください。' : 'あなたの入力は、最新の版と同じです。直す必要はありません。',
    actions: diff.length ? diffTable(diff, '最新の版との違い') : null,
  });
}
