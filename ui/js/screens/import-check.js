// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込の(3): 取り込む前に確かめる（SC-04）。各行に「新規」「重複」「読めない」を付け、件数を上に出す。
// 重複の行は理由つきで見せ、行ごとに「この行も取り込む」を選べる（既定は取り込まない。force_lines）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatSigned, formatYen } from '../format.js';
import { countsText } from '../import-map.js';
import { createTable } from '../components/table.js';
import { button } from '../components/fields.js';
import { emptyState } from '../components/empty.js';

const STATUS = {
  new: ['新規', 'badge--ok', 'plus'],
  duplicate: ['重複', 'badge--warn', 'copy'],
  invalid: ['読めない', 'badge--danger', 'alert'],
};

function statusBadge(s) {
  const [label, cls, iconName] = STATUS[s] || [s, 'badge--muted', 'info'];
  return h('span', { class: ['badge', cls] }, icon(iconName), label);
}

/**
 * @param {{ preview: { rows: any[], counts: object, truncated: boolean }, walletName: string,
 *           onBack: () => void, onCommit: (opts: { force: number[], saveProfile: boolean }) => void }} o
 */
export function createCheckStep(o) {
  const { rows, counts, truncated } = o.preview;
  const force = new Set();
  const saveProfile = h('input', { type: 'checkbox', checked: true });
  const commitBtn = button({ label: '', variant: 'primary', iconName: 'import', onClick: () => o.onCommit({ force: [...force], saveProfile: saveProfile.checked }) });

  function paintCommit() {
    const take = counts.new + force.size;
    commitBtn.querySelector('span').textContent = `${take}件を取り込む`;
    commitBtn.disabled = take === 0;
  }

  const forceBox = (r) => {
    if (r.status !== 'duplicate' || !r.forceable) return null;
    const box = h('input', { type: 'checkbox', 'aria-label': `${r.line}行目も取り込む（${r.description}）` });
    box.addEventListener('change', () => {
      if (box.checked) force.add(r.line);
      else force.delete(r.line);
      paintCommit();
    });
    return h('label', { class: 'check force-check' }, box, h('span', null, 'この行も取り込む'));
  };

  const table = createTable({
    caption: '取り込む前の確認（行ごとの判定）',
    className: 'check-table',
    rowKey: (r) => r.line,
    rowClass: (r) => `is-${r.status}`,
    columns: [
      { key: 'line', label: '行', num: true, render: (r) => String(r.line) },
      { key: 'status', label: '判定', render: (r) => statusBadge(r.status) },
      { key: 'date', label: '日付', render: (r) => r.date || '—' },
      { key: 'description', label: '摘要', className: 'wrap', render: (r) => r.description || '—' },
      { key: 'amount', label: '金額', num: true, render: (r) => (r.amount ? h('span', { class: ['amount', `amount--${r.direction}`] }, formatSigned(r.direction === 'in' ? r.amount : -r.amount)) : '—') },
      { key: 'reason', label: '理由・選択', className: 'wrap reason-cell', render: (r) => [r.reason ? h('span', { class: 'reason-text' }, r.reason) : null, forceBox(r)] },
    ],
    rows,
  });

  const head = h(
    'div',
    { class: 'check-head' },
    h(
      'ul',
      { class: 'check-counts', 'aria-label': '判定の件数' },
      h('li', null, statusBadge('new'), h('span', { class: 'num' }, `${formatYen(counts.new)}件`)),
      h('li', null, statusBadge('duplicate'), h('span', { class: 'num' }, `${formatYen(counts.duplicate)}件`)),
      h('li', null, statusBadge('invalid'), h('span', { class: 'num' }, `${formatYen(counts.invalid)}件`)),
    ),
    h('p', { class: 'visually-hidden' }, countsText(counts)),
  );
  const notes = [];
  if (counts.duplicate) notes.push('「重複」は、同じ口座・日付・金額・摘要の明細が、すでに取り込んであるものです。期間が重なる明細を取り込んだときに出ます。本当に別の取引なら、行ごとに「この行も取り込む」を選んでください。');
  if (counts.invalid) notes.push('「読めない」行は取り込みません。理由を見て、列の対応を直すと読めることがあります（「戻る」で直せます）。');
  if (truncated) notes.push('表に出しているのは先頭の500行です。件数はファイル全体の数です。');
  const el = h(
    'div',
    { class: 'import-step stack' },
    head,
    notes.length ? h('ul', { class: 'check-notes' }, notes.map((t) => h('li', null, icon('info', { size: 16 }), h('span', null, t)))) : null,
    rows.length ? table.el : h('div', { class: 'card' }, emptyState({ iconName: 'alert', title: '読める行がありません', body: '列の対応か、読み飛ばす行の数が合っていないかもしれません。「戻る」で直してください。' })),
    h('label', { class: 'check' }, saveProfile, h('span', null, `この列の対応を「${o.walletName}」に覚えさせる（次からの初期値になります）`)),
    h('div', { class: 'import-actions' }, button({ label: '戻る', variant: 'ghost', iconName: 'chevron-left', onClick: () => o.onBack() }), commitBtn),
  );
  paintCommit();
  return { el, focus: () => (commitBtn.disabled ? el.querySelector('button') : commitBtn).focus(), setBusy: (b) => { commitBtn.disabled = b || counts.new + force.size === 0; } };
}

/**
 * 取り込んだ後の結果。「未確認を開く」と「別のファイルを取り込む」を出す（SC-04）。
 * @param {{ result: { created: number, duplicates: number, invalid: number }, wallet: string, onAgain: () => void }} o
 */
export function resultView(o) {
  const { created, duplicates, invalid } = o.result;
  const skipped = [];
  if (duplicates) skipped.push(`重複 ${duplicates}件`);
  if (invalid) skipped.push(`読めない ${invalid}件`);
  return h(
    'div',
    { class: 'card import-done' },
    emptyState({
      iconName: 'check-circle',
      title: `${created}件を未確認に入れました`,
      body: `${skipped.length ? `${skipped.join('と')}は取り込んでいません。` : ''}未確認の画面で科目を決めて、確定してください。確定するまでは、仕訳帳と帳票には出ません。`,
      actions: [
        { label: '未確認を開く', href: `#/inbox?wallet=${encodeURIComponent(o.wallet)}`, primary: true, kbd: 'g i' },
        { label: '別のファイルを取り込む', onClick: o.onAgain },
      ],
    }),
  );
}
