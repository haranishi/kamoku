// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の下の固定バー（SC-03）: 「確定候補 N件／入金 …円／出金 …円　[確定する Ctrl+Enter]」と、保存の状態。
// 確定の前の確認（件数・出金の合計・入金の合計）もここに置く。金額は折り返しても切らない（幅390・768・1440で確かめる）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { modLabel } from '../keys.js';
import { button } from '../components/fields.js';
import { openModal } from '../components/dialog.js';

const NOTE_ID = 'inbox-block-note';

function sum(label, valueEl, extra) {
  return h('div', { class: ['inbox-sum', extra] }, h('dt', null, label), h('dd', null, valueEl));
}

/**
 * @param {{ onConfirm: () => void }} opts
 */
export function createInboxBar(opts) {
  const count = h('span', { class: 'num' }, '0');
  const inTotal = h('span', { class: 'num' }, '0');
  const outTotal = h('span', { class: 'num' }, '0');
  const saveState = h('p', { class: 'inbox-save', role: 'status' });
  const note = h('p', { class: 'field-hint inbox-note', id: NOTE_ID });
  const confirmBtn = button({ label: '確定する', variant: 'primary', kbd: `${modLabel()} Enter`, className: 'inbox-confirm', onClick: () => opts.onConfirm() });
  const el = h(
    'section',
    { class: 'inbox-bar', 'aria-label': '確定候補' },
    h(
      'div',
      { class: 'inbox-bar-main' },
      h(
        'dl',
        { class: 'inbox-sums' },
        sum('確定候補', [count, h('span', { class: 'unit' }, '件')], 'inbox-sum--count'),
        sum('入金', [inTotal, h('span', { class: 'unit' }, '円')]),
        sum('出金', [outTotal, h('span', { class: 'unit' }, '円')]),
      ),
      confirmBtn,
    ),
    h('div', { class: 'inbox-bar-foot' }, saveState, note),
  );
  let saveTimer = null;
  return {
    el,
    confirmButton: confirmBtn,
    /** 件数と合計を出す。reason があれば確定のボタンを止めて、その理由を出す。 */
    update({ count: n, inTotal: i, outTotal: o }, reason = '') {
      count.textContent = String(n);
      inTotal.textContent = formatYen(i);
      outTotal.textContent = formatYen(o);
      note.textContent = reason;
      confirmBtn.disabled = Boolean(reason);
      if (reason) confirmBtn.setAttribute('aria-describedby', NOTE_ID);
      else confirmBtn.removeAttribute('aria-describedby');
    },
    /** 保存の状態（保存中・保存済み・失敗）を小さく出す。 */
    setSaveState(kind) {
      clearTimeout(saveTimer);
      const text = { saving: '保存しています…', saved: '保存済み', failed: '保存できなかった欄があります。行の下の理由を見てください。' }[kind] || '';
      const iconName = { saving: 'clock', saved: 'check-circle', failed: 'alert' }[kind];
      saveState.replaceChildren(...(text ? [icon(iconName, { size: 16 }), h('span', null, text)] : []));
      saveState.dataset.state = kind || '';
      // 「保存済み」は少し経ったら消す（ずっと出ていると、次の保存との区別がつかない）。
      if (kind === 'saved') saveTimer = setTimeout(() => saveState.replaceChildren(), 4000);
    },
  };
}

/**
 * 確定の前の確認。件数と、出金の合計・入金の合計を見せる。押した結果を返す。
 * @returns {Promise<boolean>}
 */
export function confirmCandidates({ count, inTotal, outTotal }) {
  return new Promise((resolve) => {
    let ok = false;
    openModal({
      title: `${count}件を確定しますか`,
      body: h(
        'div',
        { class: 'stack' },
        h(
          'dl',
          { class: 'confirm-sums' },
          h('div', null, h('dt', null, '件数'), h('dd', null, h('span', { class: 'num' }, String(count)), '件')),
          h('div', null, h('dt', null, '出金の合計'), h('dd', null, h('span', { class: 'num' }, formatYen(outTotal)), '円')),
          h('div', null, h('dt', null, '入金の合計'), h('dd', null, h('span', { class: 'num' }, formatYen(inTotal)), '円')),
        ),
        h('p', { class: 'small muted' }, '確定すると仕訳帳に入り、帳票の集計に入ります。確定した後は、訂正と取消の手続きでしか変えられません。'),
      ),
      actions: [
        { label: 'やめる', onClick: (c) => c.close('cancel') },
        {
          label: '確定する',
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
