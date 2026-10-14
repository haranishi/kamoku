// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 設定の「データ」（SC-10、AC-09.1〜AC-09.3）: 台帳の検査、末尾のハッシュの写し取り、控えたハッシュの照合、書き出し。
// 照合は、控えたハッシュが台帳の何番目の記録にあたるかを返す。末尾の削除や作り替えに気づくための手段（architecture.md）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { apiUrl } from '../api.js';
import { normalizeHash, verifyText, notedText } from '../ledger-check.js';
import { field, textInput, selectInput, button } from '../components/fields.js';
import { enhanceDownload } from '../components/download.js';

function result(kind, title, body) {
  return h('div', { class: ['check-result', `check-result--${kind}`], role: 'status' }, icon(kind === 'ok' ? 'shield-check' : kind === 'ng' ? 'shield-alert' : 'info'), h('div', null, h('p', { class: 'banner-title' }, title), body ? h('p', null, body) : null));
}

async function copyText(text, host, toast) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success('末尾のハッシュを写し取りました。紙やメモなど、このパソコンの外に控えておいてください。');
  } catch {
    const box = h('input', { class: 'input mono', readonly: true, value: text, 'aria-label': '末尾のハッシュ（全文）' });
    host.replaceChildren(box, h('p', { class: 'field-hint' }, '自動で写せませんでした。選んである文字を Ctrl+C（Macは ⌘+C）で写してください。'));
    box.focus();
    box.select();
  }
}

/**
 * @param {{ api: any, state: any, toast: any }} ctx
 */
export function dataSection(ctx) {
  const { api, state, toast } = ctx;
  const verifyOut = h('div', { class: 'check-out' });
  const verifyBtn = button({ label: '台帳を検査する', iconName: 'shield-check', onClick: () => verify() });
  async function verify() {
    verifyBtn.disabled = true;
    verifyOut.replaceChildren(h('p', { class: 'muted', role: 'status' }, '最初の記録から順に、たどっています…'));
    try {
      const res = await api.post('/verify', { noted_hash: null });
      const t = verifyText(res);
      verifyOut.replaceChildren(result(t.ok ? 'ok' : 'ng', t.title, t.body));
      state.load('status', { force: true }).catch(() => {});
    } catch (err) {
      verifyOut.replaceChildren(result('ng', '検査できませんでした', err.message));
    } finally {
      verifyBtn.disabled = false;
    }
  }

  const headCode = h('code', { class: 'mono hash-full' }, '読み込んでいます…');
  const manual = h('div', { class: 'hash-manual' });
  const copyBtn = button({ label: '写し取る', iconName: 'copy', small: true, onClick: () => copyText(headCode.dataset.hash || '', manual, toast) });
  const headFacts = h('p', { class: 'small muted' });
  const paintHead = (st) => {
    if (!st || !st.ledger) return;
    headCode.textContent = st.ledger.head_hash;
    headCode.dataset.hash = st.ledger.head_hash;
    headFacts.textContent = `${st.ledger.events}件の記録の、末尾（${st.ledger.head_seq}番目）のハッシュです。`;
  };
  paintHead(state.peek('status'));

  const notedIn = textInput({ class: 'mono', placeholder: '64文字の英数字（0〜9 と a〜f）', maxlength: 100, spellcheck: 'false' });
  const notedF = field({ label: '控えたハッシュ', control: notedIn, hint: '前に写し取ったハッシュを貼り付けると、それが台帳の何番目の記録にあたるかを調べます。' });
  const notedOut = h('div', { class: 'check-out' });
  async function compare() {
    const r = normalizeHash(notedIn.value);
    notedF.setError(r.ok ? '' : r.error);
    if (!r.ok) {
      notedIn.focus();
      return;
    }
    try {
      const res = await api.post('/verify', { noted_hash: r.value });
      if (!res.ok) {
        const t = verifyText(res);
        notedOut.replaceChildren(result('ng', t.title, t.body));
        return;
      }
      const t = notedText(res.noted, res.events);
      notedOut.replaceChildren(result(t.found ? 'ok' : 'ng', t.found ? '一致する記録がありました' : '一致する記録がありません', t.text));
    } catch (err) {
      notedOut.replaceChildren(result('ng', '照合できませんでした', err.message));
    }
  }
  notedIn.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e) || e.key !== 'Enter') return;
    e.preventDefault();
    compare();
  });

  const enc = selectInput({ value: 'utf8bom', 'aria-label': '仕訳帳のCSVの文字コード' }, [
    { value: 'utf8bom', label: 'UTF-8（BOMつき）' },
    { value: 'sjis', label: 'Shift_JIS' },
  ]);
  const scope = selectInput({ value: 'active', 'aria-label': '仕訳帳のCSVに入れる仕訳' }, [
    { value: 'active', label: '有効な仕訳だけ' },
    { value: 'all', label: '取消済みも入れる' },
  ]);
  const journalLink = h('a', { class: 'btn', download: '' }, icon('import', { size: 18 }), h('span', null, '仕訳帳のCSV'));
  const backupLink = h('a', { class: 'btn', href: apiUrl('/export/backup.db'), download: '' }, icon('import', { size: 18 }), 'バックアップ（DBの複製）');
  const jsonLink = h('a', { class: 'btn', href: apiUrl('/export/all.json'), download: '' }, icon('import', { size: 18 }), '全件のJSON');
  // 押したときに受け取ってから保存させる（Shift_JIS で置き換えた文字の数や、失敗の理由を知らせるため）。
  [journalLink, backupLink, jsonLink].forEach((link) => enhanceDownload(link, ctx));
  const paintLink = () => {
    journalLink.href = apiUrl('/export/journal.csv', { state: scope.value, encoding: enc.value });
  };
  enc.addEventListener('change', paintLink);
  scope.addEventListener('change', paintLink);
  paintLink();

  const el = h(
    'div',
    { class: 'settings-data stack' },
    h('div', { class: 'settings-block' }, h('h3', null, '台帳の整合性'), h('p', { class: 'small muted' }, '記録は追記だけで、1つずつハッシュでつながっています。最初からたどって、壊れていないかを確かめます。'), h('div', null, verifyBtn), verifyOut),
    h('div', { class: 'settings-block' }, h('h3', null, '末尾のハッシュ'), headFacts, headCode, h('div', null, copyBtn), manual),
    h('div', { class: 'settings-block' }, h('h3', null, '控えたハッシュと照合する'), notedF.el, h('div', null, button({ label: '照合する', iconName: 'search', onClick: () => compare() })), notedOut),
    h(
      'div',
      { class: 'settings-block' },
      h('h3', null, '書き出し'),
      h('p', { class: 'small muted' }, 'このパソコンの中に、ファイルとして保存します。外へは送りません。'),
      h('div', { class: 'export-row' }, backupLink, jsonLink),
      h('div', { class: 'export-row' }, scope, enc, journalLink),
    ),
  );
  return { el, paintHead };
}
