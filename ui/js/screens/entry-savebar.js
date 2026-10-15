// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録の、保存のボタンの帯。保存できない理由（読み取り専用・プレビューの失敗・貸借の不一致）があれば、
// ボタンを止めて、その理由を文字で出し、aria-describedby で結ぶ（読み上げでも止まった訳が分かるように）。
// 帯は、幅1280px未満では画面の下に付けて、いつでも押せるようにする（screens-entry.css）。だから中身は保存の操作だけにし、
// 「テンプレートに登録」は帯の外（プレビューの下）に置く（tplEl）。

import { h } from '../dom.js';
import { modLabel } from '../keys.js';
import { button } from '../components/fields.js';

const NOTE_ID = 'entry-block-note';
/** 誤りの一言を一度空にしてから入れるまでの間（同じ文でも、読み上げに変化として届くように）。 */
export const ANNOUNCE_DELAY_MS = 100;

/**
 * @param {{ correcting: boolean, onSave: (o: { cont: boolean }) => void, onCancel: () => void, onTemplate: () => void }} opts
 */
export function createSaveBar(opts) {
  const formError = h('p', { class: 'field-error form-error', role: 'alert' });
  const blockNote = h('p', { class: 'field-hint block-note', id: NOTE_ID });
  const saveCont = button({ label: opts.correcting ? '訂正を確かめる' : '保存して続ける', variant: 'primary', kbd: `${modLabel()} Enter`, className: 'save-main', onClick: () => opts.onSave({ cont: true }) });
  const saveOnly = opts.correcting ? null : button({ label: '保存', onClick: () => opts.onSave({ cont: false }) });
  const cancelBtn = button({ label: 'やめる', variant: 'ghost', kbd: 'Esc', onClick: () => opts.onCancel() });
  const tplSave = opts.correcting ? null : button({ label: 'テンプレートに登録', iconName: 'bookmark', small: true, variant: 'ghost', onClick: () => opts.onTemplate() });
  // 主な操作（保存して続ける）は右の端（確認の小窓・未確認・取込と同じ）。並び（Tabの順）も見た目と同じにする。
  const el = h(
    'section',
    { class: 'entry-actions-card', 'data-bottom-bar': true, 'aria-label': '保存' },
    formError,
    blockNote,
    h('div', { class: 'entry-actions' }, cancelBtn, saveOnly, saveCont),
  );
  const tplEl = tplSave ? h('div', { class: 'pv-foot' }, tplSave) : null;
  let seq = 0;
  // いま出している文が、announce（誤りの数と欄の名前）のものか。欄を直した後は古くなるので、これだけを消す。
  let announced = false;
  return {
    el,
    /** 帯の外に置く「テンプレートに登録」（訂正のときは無い）。 */
    tplEl,
    /** ボタンの上に、保存できなかった原因と直し方を出す（空なら消す）。 */
    setError(message) {
      seq += 1;
      announced = false;
      formError.textContent = message || '';
    },
    /**
     * 欄を直したときと、入力の形を切り替えたときに、「N か所を直してください：…」を消す。保存を試みたときの結果なので、
     * 直した欄や、切り替えた先に無い欄を指したまま残ると、事実と違う（2026-10-15 の体験の評価 E1）。
     * サーバーの断りなど、ほかの理由（setError）の文は残す。
     */
    clearAnnounced() {
      seq += 1; // 出す順番を待っている announce も取り消す
      if (!announced) return;
      announced = false;
      formError.textContent = '';
    },
    /**
     * 保存の検査で見つかった誤りを、数と欄の名前で知らせる（「2か所を直してください：金額、科目」。role="alert"）。
     * 誤りの欄にフォーカスが残ったまま保存すると、画面には誤りが出ても、読み上げでは何も起きなかった（2026-10-15 の体験の評価 E4）。
     * @param {string[]} names 誤りのあった欄の名前（上から順）
     */
    announce(names) {
      seq += 1;
      announced = false;
      const my = seq;
      formError.textContent = '';
      if (!names || !names.length) return;
      const text = `${names.length}か所を直してください：${names.join('、')}`;
      setTimeout(() => {
        if (my !== seq) return;
        announced = true;
        formError.textContent = text;
      }, ANNOUNCE_DELAY_MS);
    },
    /**
     * ボタンの状態を決める。reason があれば止めて、その理由を出す。
     * @param {{ reason: string, readOnly: boolean, busy: boolean }} p
     */
    update({ reason, readOnly, busy }) {
      blockNote.textContent = reason;
      for (const b of [saveCont, saveOnly, tplSave]) {
        if (!b) continue;
        b.disabled = Boolean(reason) || busy;
        if (reason) b.setAttribute('aria-describedby', readOnly ? `banner-readonly ${NOTE_ID}` : NOTE_ID);
        else b.removeAttribute('aria-describedby');
      }
    },
    /** テンプレートに登録できるのは、かんたん入力のときだけ。 */
    showTemplate(on) {
      if (tplEl) tplEl.hidden = !on;
    },
  };
}
