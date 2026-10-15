// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-06 帳票 `#/reports/<name>`。タブ6つ（月次推移・損益計算書・貸借対照表・試算表・総勘定元帳・消費税）。
// 選んだ期間（と元帳の科目）はアドレスに持つ。並びは「タブ → 期間 → 中身 → 書き出し」。最初の1画面を数字に使うため、
// 「CSVで書き出す」（文字コードを選べる。AC-08.6、AC-08.7）は中身の下に置く。
// タブは1段のまま横に動かせる（狭い幅で2段に折り返さない。選んだタブを見える位置に寄せ、続きは端の影で示す）。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { apiUrl } from '../api.js';
import { todayIso } from '../input-parse.js';
import { readPeriod, periodQuery, periodLabel, carryPeriod } from '../period.js';
import { chartCompare } from '../report-tree.js';
import { createTabs } from '../components/tabs.js';
import { createCombobox } from '../components/combobox.js';
import { selectInput, field } from '../components/fields.js';
import { banner } from '../components/banner.js';
import { enhanceDownload } from '../components/download.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { phrase } from '../components/phrase.js';
import { createPeriodPicker } from './report-period.js';
import { countPending, pendingNote } from './report-pending.js';
import { monthlyView, incomeView, balanceView } from './report-views.js';
import { trialView, ledgerView, taxView } from './report-views2.js';

export const REPORT_TABS = [
  { id: 'monthly', label: '月次推移', view: monthlyView, yearOnly: true },
  { id: 'income-statement', label: '損益計算書', view: incomeView },
  { id: 'balance-sheet', label: '貸借対照表', view: balanceView, dateMode: true },
  { id: 'trial-balance', label: '試算表', view: trialView },
  { id: 'ledger', label: '総勘定元帳', view: ledgerView },
  { id: 'tax-summary', label: '消費税', view: taxView },
];
const KIND_LABEL = { asset: '資産', liability: '負債', equity: '資本', revenue: '収益', expense: '費用' };
const ENC_KEY = 'kamoku.csv.encoding';
// 元帳の科目の欄で、口座を口座の種類の言葉でも探せるようにする（「よきん」「ふつう」で、動きの無い親の科目だけが出て、
// 動きのある口座が出なかった）。読みは辞書を持たないので、種類ごとの言葉を候補の keywords に入れる。
const WALLET_WORDS = {
  cash: 'げんきん 現金',
  bank: 'よきん ふつうよきん 預金 普通預金',
  card: 'かーど くれじっとかーど カード クレジットカード',
  ewallet: 'でんしまねー 電子マネー',
};

function savedEncoding() {
  try {
    return localStorage.getItem(ENC_KEY) === 'sjis' ? 'sjis' : 'utf8bom';
  } catch {
    return 'utf8bom';
  }
}

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const today = todayIso();
  let tab = REPORT_TABS.find((t) => t.id === ctx.params.name) || null;
  if (!tab) {
    // 組み立ての途中で移ると、ルーターが覚える片付けの関数が食い違うので、次の番で移る。
    queueMicrotask(() => ctx.navigate(ctx.router.buildHash('/reports/monthly', ctx.query), { replace: true }));
    return () => {};
  }
  let query = ctx.query;
  let period = readPeriod(query, { today, dateMode: tab.dateMode, yearOnly: tab.yearOnly });
  let data = null;
  let view = null;
  let seq = 0;
  let disposed = false;
  let picker = null;
  let accountCb = null;

  const go = (t, p, extra = {}) => ctx.navigate(ctx.router.buildHash(`/reports/${t.id}`, { ...periodQuery(p), ...extra }));
  const tabs = createTabs({ label: '帳票の種類', tabs: REPORT_TABS.map((t) => ({ id: t.id, label: t.label })), selected: tab.id, onChange: (id) => {
    const t = REPORT_TABS.find((x) => x.id === id);
    go(t, carryPeriod(period, { today, dateMode: t.dateMode, yearOnly: t.yearOnly }), t.id === 'ledger' ? { account: query.account } : {});
  } });
  tabs.panel.classList.add('report-panel');
  tabs.el.classList.add('report-tabs');
  // 横に動かせる帯は、帳票のタブだけの例外（AC-11.6 の試験は data-allow-hscroll の中を数えない）。
  tabs.el.dataset.allowHscroll = '';
  const pickerHost = h('div', { class: 'report-period' });
  const accountHost = h('div', { class: 'report-account' });
  const encSel = selectInput({ value: savedEncoding(), 'aria-label': 'CSVの文字コード', class: 'csv-enc' }, [
    { value: 'utf8bom', label: 'UTF-8（BOMつき）' },
    { value: 'sjis', label: 'Shift_JIS' },
  ]);
  const exportLink = h('a', { class: 'btn csv-link', download: '' }, icon('import', { size: 18 }), h('span', null, 'CSVで書き出す'));
  enhanceDownload(exportLink, ctx);
  const exportBox = h(
    'div',
    { class: 'report-export' },
    h('span', { class: 'report-export-label', 'aria-hidden': 'true' }, 'この帳票を書き出す'),
    h('div', { class: 'report-export-row' }, encSel, exportLink),
  );
  const title = h('h2', { class: 'report-title', 'aria-live': 'polite', tabindex: '-1' });
  // 自由な期間の欄に誤りがあるあいだ、表がどの期間のままかを見出しの横に書く（欄の日付と見出しの期間が食い違って見えた）。
  const stale = h('p', { class: 'report-stale', hidden: true });
  const content = h('div', { class: 'report-content' });
  // 期間のすぐ下に、この期間の未確認（集計に入っていない明細）の件数を1行で出す。0件なら隠す。
  const pendingHost = h('div', { class: 'report-pending', hidden: true });
  root.append(h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '帳票'))), tabs.el, tabs.panel);
  tabs.panel.append(h('div', { class: 'card report-toolbar' }, pickerHost, accountHost), pendingHost, h('div', { class: 'report-head' }, title, stale), content, exportBox);

  function paintStale(bad) {
    stale.hidden = !bad;
    stale.replaceChildren(...(bad ? [icon('info', { size: 16 }), phrase(`前の期間（${periodLabel(period)}）を表示中です。`)] : []));
  }

  /** 選んだタブを、横に動く帯の中で見える位置（真ん中寄り）に寄せる。ページは縦に動かさない。 */
  function revealTab() {
    const btn = tabs.el.querySelector('[aria-selected="true"]');
    if (!btn || tabs.el.scrollWidth <= tabs.el.clientWidth) return;
    tabs.el.scrollLeft = Math.max(0, btn.offsetLeft - (tabs.el.clientWidth - btn.offsetWidth) / 2);
  }
  requestAnimationFrame(revealTab);

  function paintExport(csv) {
    if (!csv) {
      // href の無い <a> はリンクとして読まれなくなるので、役割を明示して「使えない」状態で残す。
      exportLink.removeAttribute('href');
      exportLink.setAttribute('role', 'link');
      exportLink.setAttribute('aria-disabled', 'true');
      exportLink.title = '科目を選ぶと、書き出せます。';
      return;
    }
    exportLink.removeAttribute('role');
    exportLink.removeAttribute('aria-disabled');
    exportLink.removeAttribute('title');
    exportLink.href = apiUrl(csv.path, { ...csv.params, encoding: encSel.value });
  }
  let lastCsv = null;
  encSel.addEventListener('change', () => {
    try {
      localStorage.setItem(ENC_KEY, encSel.value);
    } catch {
      // 覚えられなくても、この画面の間は効く。
    }
    paintExport(lastCsv);
  });

  function buildControls() {
    paintStale(false);
    picker = createPeriodPicker({ period, years: years(), today, dateMode: tab.dateMode, yearOnly: tab.yearOnly, onChange: (p) => go(tab, p, tab.id === 'ledger' ? { account: query.account } : {}), onInvalid: paintStale });
    pickerHost.replaceChildren(picker.el);
    if (accountCb) accountCb.destroy();
    accountCb = null;
    accountHost.replaceChildren();
    if (tab.id === 'ledger' && data) {
      // 口座を先に並べる。同じ名前の親の科目（現金・普通預金など）は、直接の記帳しか出ず、0行になりやすい。「げんきん」で
      // Enter を押したとき、先頭の動きの無い親の科目が選ばれていた（体験の評価 第3回 E3）。
      const sorted = [...data.accounts].sort(chartCompare);
      const items = [...sorted.filter((a) => a.wallet), ...sorted.filter((a) => !a.wallet)].map((a) => ({ value: a.code, label: a.name, reading: a.reading, hint: a.wallet ? '口座' : KIND_LABEL[a.kind], keywords: a.wallet ? WALLET_WORDS[a.wallet.type] : undefined }));
      accountCb = createCombobox({
        items,
        value: query.account || null,
        listLabel: '科目の候補',
        placeholder: '例: げんきん、通信費',
        // 合う候補が無いときに、空の一覧だけが出て、何が起きたか分からなかった。
        noMatch: '合う科目がありません。ひらがなの読みか、科目の名前で探せます。',
        onSelect: (item) => item && item.value !== query.account && go(tab, period, { account: item.value }),
      });
      accountHost.append(field({ label: '科目', control: accountCb.el, labelFor: accountCb.input }).el);
    }
  }

  function years() {
    const first = data && data.settings && /^\d{4}/.test(data.settings.opened_on || '') ? Number(data.settings.opened_on.slice(0, 4)) : Number(today.slice(0, 4)) - 2;
    const last = Number(today.slice(0, 4));
    const out = [];
    for (let y = Math.min(first, period.year); y <= Math.max(last, period.year); y += 1) out.push(y);
    return out;
  }

  /**
   * この帳票の期間の未確認を数えて、期間の下に出す。口座の元帳なら、その口座の未確認だけ。
   * 数え終わるまでは前の行を残す（消してから出し直すと、下の数字が上下に揺れる）。数えられなければ出さない。
   */
  async function paintPending(my) {
    let note = null;
    if (tab.id !== 'ledger' || query.account) {
      const range = tab.dateMode ? { to: period.asOf } : { from: period.from, to: period.to };
      const wallet = tab.id === 'ledger' && data.accounts.some((a) => a.code === query.account && a.wallet) ? query.account : null;
      if (wallet) range.wallet = wallet;
      try {
        note = pendingNote(await countPending(ctx.api, range), wallet ? `#/inbox?wallet=${encodeURIComponent(wallet)}` : '#/inbox');
      } catch {
        // 未確認を数えられなくても、帳票そのものは出す（注記が無いだけ）。
      }
    }
    if (disposed || my !== seq) return;
    pendingHost.replaceChildren(...(note ? [note] : []));
    pendingHost.hidden = !note;
  }

  /**
   * 見出し。元帳は科目の名前も入れる（読み上げの知らせが「総勘定元帳 2026年」だけで、どの科目か分からなかった）。
   * 期間は1つの塊にする（幅390で「2026-06-／30」と日付の途中で折れた）。
   */
  function paintTitle() {
    const acc = tab.id === 'ledger' && query.account ? data.accounts.find((a) => a.code === query.account) : null;
    title.replaceChildren(h('span', null, acc ? `${tab.label}（${acc.name}）` : tab.label), '　', h('span', { class: 'nw' }, periodLabel(period)));
  }

  /**
   * 中身を作り直す前に、フォーカスが中身の中にあったか（元帳の口座へのリンクを押した、など）を覚える。
   * 作り直すと、その要素が消えてフォーカスが本文の外（body）に落ちるので、作り直した後に、同じ仕訳の行、
   * 無ければ行の入口、それも無ければ見出しへ移す。
   */
  function focusMemo() {
    const el = document.activeElement;
    if (!el || !content.contains(el)) return null;
    const tr = el.closest('tr[data-entry-id]');
    return { entry: tr ? tr.dataset.entryId : null };
  }

  function refocus(memo) {
    if (!memo || (document.activeElement && document.activeElement !== document.body)) return;
    const rows = [...content.querySelectorAll('tr[data-entry-id]')];
    const target = rows.find((tr) => memo.entry && tr.dataset.entryId === memo.entry) || content.querySelector('tr[data-entry-id][tabindex="0"]') || title;
    target.focus({ preventScroll: true });
  }

  async function load() {
    const my = ++seq;
    if (view && view.destroy) view.destroy();
    view = null;
    paintTitle();
    const memo = focusMemo();
    content.replaceChildren();
    paintPending(my);
    const stop = delayedSkeleton(content, () => skeletonBlock({ lines: 6, label: `${tab.label}を作っています` }));
    try {
      const ledgerHref = (code) => ctx.router.buildHash('/reports/ledger', { ...periodQuery(period), account: code });
      const v = await tab.view({ api: ctx.api, period, accounts: data.accounts, settings: data.settings, today, account: query.account || null, navigate: ctx.navigate, ledgerHref });
      stop();
      if (disposed || my !== seq) {
        if (v.destroy) v.destroy();
        return;
      }
      view = v;
      lastCsv = v.csv;
      paintExport(v.csv);
      replace(content, v.node);
      refocus(memo);
    } catch (err) {
      stop();
      if (disposed || my !== seq) return;
      lastCsv = null;
      paintExport(null);
      replace(content, banner({ kind: 'danger', title: `${tab.label}を作れませんでした`, body: err.message }));
    }
  }

  const unkeys = ctx.keys.register({
    '[': () => (picker ? picker.shift(-1) : false),
    ']': () => (picker ? picker.shift(1) : false),
    // 元帳の表の行を j・k で動かす（仕訳帳と同じ）。元帳でなければ、キーを全体に回す。
    j: () => (view && view.moveRow ? view.moveRow(1) : false),
    k: () => (view && view.moveRow ? view.moveRow(-1) : false),
  });
  const fresh = { force: true };
  Promise.all([ctx.state.load('settings', fresh), ctx.state.load('accounts', fresh)])
    .then(([settings, accounts]) => {
      if (disposed) return;
      data = { settings, accounts };
      buildControls();
      load();
    })
    .catch((err) => {
      if (!disposed) replace(content, banner({ kind: 'danger', title: '帳票の画面を準備できませんでした', body: err.message }));
    });

  const cleanup = () => {
    disposed = true;
    unkeys();
    if (view && view.destroy) view.destroy();
    if (accountCb) accountCb.destroy();
  };
  cleanup.update = (next) => {
    const nextTab = REPORT_TABS.find((t) => t.id === next.params.name);
    if (!nextTab) return false;
    const tabChanged = nextTab.id !== tab.id;
    tab = nextTab;
    query = next.query;
    period = readPeriod(query, { today, dateMode: tab.dateMode, yearOnly: tab.yearOnly });
    tabs.select(tab.id);
    revealTab();
    if (data) {
      // 同じタブのままなら、欄を作り直さずに値だけ合わせる（フォーカスを失わないため）。
      if (tabChanged) buildControls();
      else {
        picker.set(period);
        if (accountCb && accountCb.value !== (query.account || null)) accountCb.setValue(query.account || null);
      }
      load();
    }
    return true;
  };
  return cleanup;
}
