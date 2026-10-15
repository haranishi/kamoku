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
import { phrase } from '../components/phrase.js';

function result(kind, title, body) {
  return h('div', { class: ['check-result', `check-result--${kind}`], role: 'status' }, icon(kind === 'ok' ? 'shield-check' : kind === 'ng' ? 'shield-alert' : 'info'), h('div', null, h('p', { class: 'banner-title' }, title), body ? h('p', null, body) : null));
}

async function copyText(text, host, toast) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success('照合用の控えを写し取りました。紙やメモなど、このパソコンの外に残しておいてください。');
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
    headFacts.replaceChildren(phrase(`いちばん新しい記録（${st.ledger.head_seq}番目）までの中身から作った、64文字の控えです。このパソコンの外に書き残しておくと、あとで照合できます。`));
  };
  paintHead(state.peek('status'));

  const notedIn = textInput({ class: 'mono', placeholder: '写し取った64文字を貼り付け', maxlength: 100, spellcheck: 'false' });
  const notedF = field({ label: '控えたハッシュ', control: notedIn, hint: phrase('前に写し取った控えを貼り付けると、台帳の何番目の記録にあたるかを調べます。'), clearOnInput: true });
  const notedOut = h('div', { class: 'check-out' });
  async function compare() {
    // 照合し直すときは、前の結果を先に消す（形の誤りの下に、前の「一致する記録がありました」が残り、食い違って見えた）。
    notedOut.replaceChildren();
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
  const journalLink = h('a', { class: 'btn export-wide', download: '' }, icon('import', { size: 18 }), h('span', null, '仕訳帳のCSV'));
  // 名前は、何に使うファイルかで呼ぶ（「DBの複製」「全件のJSON」は、使う人に通じにくかった）。
  // 文字は塊で折る（幅1280〜1383pxの2列の設定では、ボタンが細くなり「バックア／ップ」と語の途中で折れた）。
  const backupLink = h('a', { class: 'btn', href: apiUrl('/export/backup.db'), download: '' }, icon('import', { size: 18 }), phrase('帳簿の控え（バックアップ）'));
  const jsonLink = h('a', { class: 'btn', href: apiUrl('/export/all.json'), download: '' }, icon('import', { size: 18 }), phrase('全件のデータ（JSON）'));
  // 押したときに受け取ってから保存させる（Shift_JIS で置き換えた文字の数や、失敗の理由を知らせるため）。
  [journalLink, backupLink, jsonLink].forEach((link) => enhanceDownload(link, ctx));
  const paintLink = () => {
    journalLink.href = apiUrl('/export/journal.csv', { state: scope.value, encoding: enc.value });
  };
  enc.addEventListener('change', paintLink);
  scope.addEventListener('change', paintLink);
  paintLink();

  const small = (text) => h('p', { class: 'small muted' }, phrase(text));
  const el = h(
    'div',
    { class: 'settings-data stack' },
    h('div', { class: 'settings-block' }, h('h3', null, '台帳の整合性'), small('記録は書き足すだけで、前の記録と鎖のようにつながっています。最初からたどって、壊れていないかを確かめます。'), h('div', null, verifyBtn), verifyOut),
    h('div', { class: 'settings-block' }, h('h3', null, '照合用の控え（末尾のハッシュ）'), headFacts, headCode, h('div', null, copyBtn), manual),
    h('div', { class: 'settings-block' }, h('h3', null, '控えと照合する'), notedF.el, h('div', null, button({ label: '照合する', iconName: 'search', onClick: () => compare() })), notedOut),
    // 書き出しは、ボタンと選択欄を同じ幅の格子に並べる（幅がばらばらで、凸凹の列になっていた）。
    h(
      'div',
      { class: 'settings-block' },
      h('h3', null, '書き出し'),
      small('このパソコンの中に、ファイルとして保存します。外へは送りません。'),
      h('p', { class: 'export-title' }, '帳簿の控え・全件のデータ'),
      // 長い1段落を、控えとJSONの2つに分ける（説明・補足・操作のまとまりが続き、目的の操作を拾いにくかった。見た目の採点 第2回 幅768）。
      small('控えは、取った時点の帳簿を、ファイルごと複製したものです。戻すには、ファイルを置き換えます（手順は「よくある質問」にあります）。'),
      small('全件のデータ（JSON）は、このソフト独自の形式です。確定した記録と設定が入り、未確認の明細・テンプレート・取込の設定は入りません。ほかのソフトで使うには、変換が必要です。'),
      h('div', { class: 'export-grid' }, backupLink, jsonLink),
      h('p', { class: 'export-title' }, '仕訳帳（表計算ソフト向けのCSV）'),
      h('div', { class: 'export-grid' }, scope, enc, journalLink),
    ),
  );
  return { el, paintHead };
}
