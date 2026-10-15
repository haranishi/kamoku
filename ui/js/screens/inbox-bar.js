// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の下の固定バー（SC-03）: 「確定する行 N件／入金 …円／出金 …円　[確定する Ctrl+Enter]」と、保存の状態。
// 確定の前の確認（件数・出金の合計・入金の合計）もここに置く。金額は折り返しても切らない（幅390・768・1440で確かめる）。
// 件数と合計が変わったら、読み上げにも伝える（見えない aria-live の領域）。Enter で続けて印を付けると、そのたびに
// 読み上げが重なるので、変わってから少し待ち、最後の値だけを読む。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { modLabel } from '../keys.js';
import { button } from '../components/fields.js';
import { openModal } from '../components/dialog.js';
import { phrase } from '../components/phrase.js';
import { money } from '../components/money.js';

const NOTE_ID = 'inbox-block-note';
/** 件数の読み上げを待つ時間（ミリ秒）。この間に変わった分は、まとめて1回だけ読む。 */
export const ANNOUNCE_MS = 1000;

/** 読み上げる文「確定する行 3件、入金 0円、出金 3,300円」。 */
export function barSpeech({ count, inTotal, outTotal }) {
  return `確定する行 ${formatYen(count)}件、入金 ${formatYen(inTotal)}円、出金 ${formatYen(outTotal)}円`;
}

function sum(label, valueEl, extra) {
  return h('div', { class: ['inbox-sum', extra] }, h('dt', null, label), h('dd', null, valueEl));
}

/** money() と同じ形（小さい「円」）で、数字だけを後から書き換えられるかたまり。 */
function liveMoney() {
  const el = money(0);
  return { el, num: el.querySelector('.num') };
}

/**
 * @param {{ onConfirm: () => void }} opts
 */
export function createInboxBar(opts) {
  const count = h('span', { class: 'num' }, '0');
  const inMoney = liveMoney();
  const outMoney = liveMoney();
  const inTotal = inMoney.num;
  const outTotal = outMoney.num;
  const saveState = h('p', { class: 'inbox-save', role: 'status' });
  const note = h('p', { class: 'field-hint inbox-note', id: NOTE_ID });
  const live = h('p', { class: 'visually-hidden inbox-live', 'aria-live': 'polite' });
  const confirmBtn = button({ label: '確定する', variant: 'primary', kbd: `${modLabel()} Enter`, className: 'inbox-confirm', onClick: () => opts.onConfirm() });
  const el = h(
    'section',
    { class: 'inbox-bar', 'data-bottom-bar': true, 'aria-label': '確定する行' },
    h(
      'div',
      { class: 'inbox-bar-main' },
      h(
        'dl',
        { class: 'inbox-sums' },
        sum('確定する行', h('span', { class: 'money' }, count, h('span', { class: 'unit' }, '件')), 'inbox-sum--count'),
        sum('入金', inMoney.el),
        sum('出金', outMoney.el),
      ),
      confirmBtn,
    ),
    h('div', { class: 'inbox-bar-foot' }, saveState, note),
    live,
  );
  let saveTimer = null;
  let liveTimer = null;
  // 最後に読んだ文。最初の表示（0件）は読まない。
  let spoken = null;
  function announce(text) {
    clearTimeout(liveTimer);
    if (spoken === null) {
      spoken = text;
      return;
    }
    liveTimer = setTimeout(() => {
      if (text === spoken) return;
      spoken = text;
      live.textContent = text;
    }, ANNOUNCE_MS);
  }
  return {
    el,
    confirmButton: confirmBtn,
    /**
     * 件数と合計を出す。reason があれば確定のボタンを止めて、その理由を出す。
     * narrow を渡すと、幅640px未満ではその文を出す（指で操作する幅に、Enter の案内は通じない）。
     */
    update({ count: n, inTotal: i, outTotal: o }, reason = '', narrow = '') {
      count.textContent = formatYen(n);
      inTotal.textContent = formatYen(i);
      outTotal.textContent = formatYen(o);
      if (narrow) note.replaceChildren(h('span', { class: 'note-wide' }, reason), h('span', { class: 'note-narrow' }, narrow));
      else note.textContent = reason;
      // 狭い幅で、確定する行が0件のときは、帯を案内の1行に縮める（screens-inbox-bar.css）。
      el.classList.toggle('is-empty', n === 0);
      confirmBtn.disabled = Boolean(reason);
      if (reason) confirmBtn.setAttribute('aria-describedby', NOTE_ID);
      else confirmBtn.removeAttribute('aria-describedby');
      announce(barSpeech({ count: n, inTotal: i, outTotal: o }));
    },
    /** 画面を離れるときに呼ぶ（待っている読み上げを捨てる）。 */
    destroy() {
      clearTimeout(liveTimer);
      clearTimeout(saveTimer);
    },
    /** 保存の状態（保存中・保存済み・失敗）を小さく出す。 */
    setSaveState(kind) {
      clearTimeout(saveTimer);
      const text = { saving: '保存しています…', saved: '保存済み', failed: '保存できなかった欄があります。行の下の理由を見てください。' }[kind] || '';
      const iconName = { saving: 'clock', saved: 'check-circle', failed: 'alert' }[kind];
      saveState.replaceChildren(...(text ? [icon(iconName, { size: 16 }), h('span', null, text.length > 20 ? phrase(text) : text)] : []));
      saveState.dataset.state = kind || '';
      // 「保存済み」は少し経ったら消す（ずっと出ていると、次の保存との区別がつかない）。
      if (kind === 'saved') saveTimer = setTimeout(() => saveState.replaceChildren(), 4000);
    },
  };
}

/**
 * 確定の前の確認。件数と、入金の合計・出金の合計を、下のバーと同じ順に見せる。押した結果を返す。
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
          h('div', null, h('dt', null, '件数'), h('dd', null, h('span', { class: 'money' }, h('span', { class: 'num' }, formatYen(count)), h('span', { class: 'unit' }, '件')))),
          h('div', null, h('dt', null, '入金の合計'), h('dd', null, money(inTotal))),
          h('div', null, h('dt', null, '出金の合計'), h('dd', null, money(outTotal))),
        ),
        h('p', { class: 'small muted' }, phrase('確定すると、仕訳帳と帳票の集計に入ります。確定した後は、訂正と取消の手続きでしか変えられません。')),
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
