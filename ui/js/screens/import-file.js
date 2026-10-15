// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込の(1): 口座を選び、CSVを置く（ドラッグか選択）。口座が選ばれるまでは置けない（SC-04）。

import { h, uid } from '../dom.js';
import { icon } from '../icons.js';
import { field, selectInput, button } from '../components/fields.js';
import { WALLET_TYPE_LABEL } from './entry-options.js';
import { phrase } from '../components/phrase.js';

/** 送れるファイルの大きさの上限（api-common.md の413）。 */
export const CSV_LIMIT = 10 * 1024 * 1024;

/**
 * @param {{ wallets: any[], wallet: string, onWallet: (code: string) => void, onFile: (file: File) => void }} o
 */
export function createFileStep(o) {
  const walletSel = selectInput({ value: o.wallet || '' }, [{ value: '', label: '口座を選んでください' }, ...o.wallets.map((w) => ({ value: w.code, label: `${w.name}（${WALLET_TYPE_LABEL[w.wallet.type] || ''}）` }))]);
  const walletF = field({ label: '取り込む口座', control: walletSel, required: true, hint: phrase('明細を出した銀行やカードの口座です。口座ごとに、列の対応を覚えます。') });
  // 選ぶのは「ファイルを選ぶ」のボタンから。この欄は描かない（screens-import.css）。
  const fileIn = h('input', { type: 'file', accept: '.csv,.txt,.tsv,text/csv,text/plain', class: 'visually-hidden file-input', id: uid('csv'), tabindex: '-1' });
  // 押せない理由（先に口座を選ぶ）は、ボタンのすぐ上の一言に常に見せ、ボタンの説明にも結ぶ（E4 第2回の19）。
  const zoneHint = h('p', { class: 'drop-hint', id: uid('drop-hint') });
  const pick = button({ label: 'ファイルを選ぶ', iconName: 'import', 'aria-describedby': zoneHint.id, onClick: () => fileIn.click() });
  const zoneText = h('p', { class: 'drop-title' }, 'CSVのファイルをここに置く');
  const error = h('p', { class: 'field-error', role: 'alert' });
  const busy = h('p', { class: 'drop-busy', role: 'status' });
  const zone = h('div', { class: 'drop-zone' }, h('span', { class: 'drop-icon' }, icon('import', { size: 28 })), zoneText, zoneHint, pick, busy);

  function sync() {
    const ready = Boolean(walletSel.value);
    zone.classList.toggle('is-disabled', !ready);
    pick.disabled = !ready;
    zoneHint.replaceChildren(phrase(ready ? 'ドラッグして置くか、「ファイルを選ぶ」から選んでください。文字コード（UTF-8・Shift_JIS）は自動で判定します。10MBまで。' : '先に、上で口座を選んでください。口座が決まると、ファイルを置けます。'));
  }

  function take(file) {
    error.textContent = '';
    if (!walletSel.value) {
      error.textContent = '先に口座を選んでください。';
      walletSel.focus();
      return;
    }
    if (!file) return;
    if (file.size > CSV_LIMIT) {
      error.textContent = `ファイルが大きすぎます（${Math.ceil(file.size / 1024 / 1024)}MB）。10MBまでのCSVにしてください。期間を分けて書き出すと小さくなります。`;
      return;
    }
    o.onFile(file);
  }

  walletSel.addEventListener('change', () => {
    error.textContent = '';
    o.onWallet(walletSel.value);
    sync();
  });
  fileIn.addEventListener('change', () => {
    take(fileIn.files && fileIn.files[0]);
    fileIn.value = '';
  });
  // ドラッグで置く。口座が決まっていないときは、置けないことを伝えて受け取らない。
  zone.addEventListener('dragover', (e) => {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = walletSel.value ? 'copy' : 'none';
    zone.classList.add('is-over');
  });
  zone.addEventListener('dragleave', () => zone.classList.remove('is-over'));
  zone.addEventListener('drop', (e) => {
    e.preventDefault();
    zone.classList.remove('is-over');
    take(e.dataTransfer && e.dataTransfer.files ? e.dataTransfer.files[0] : null);
  });
  sync();

  const el = h('div', { class: 'import-step stack' }, walletF.el, zone, fileIn, error);
  return {
    el,
    /** 読み込み中の表示（ファイルの名前）。空なら消す。 */
    setBusy(text) {
      busy.textContent = text || '';
      pick.disabled = Boolean(text) || !walletSel.value;
    },
    setError(text) {
      error.textContent = text || '';
    },
    focus: () => (walletSel.value ? pick : walletSel).focus(),
    get fileInput() {
      return fileIn;
    },
  };
}
