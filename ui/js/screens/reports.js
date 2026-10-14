// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-06 帳票 `#/reports/<name>`。タブ6つ（月次推移・損益計算書・貸借対照表・試算表・総勘定元帳・消費税）。
// 選んだ期間（と元帳の科目）はアドレスに持つ。各画面の右上に「CSVで書き出す」（文字コードを選べる。AC-08.6、AC-08.7）。

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
import { createPeriodPicker } from './report-period.js';
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
  const pickerHost = h('div', { class: 'report-period' });
  const accountHost = h('div', { class: 'report-account' });
  const encSel = selectInput({ value: savedEncoding(), 'aria-label': 'CSVの文字コード', class: 'csv-enc' }, [
    { value: 'utf8bom', label: 'UTF-8（BOMつき）' },
    { value: 'sjis', label: 'Shift_JIS' },
  ]);
  const exportLink = h('a', { class: 'btn csv-link', download: '' }, icon('import', { size: 18 }), h('span', null, 'CSVで書き出す'));
  enhanceDownload(exportLink, ctx);
  const exportBox = h('div', { class: 'report-export' }, h('span', { class: 'field-label', 'aria-hidden': 'true' }, '書き出し'), h('div', { class: 'report-export-row' }, encSel, exportLink));
  const title = h('h2', { class: 'report-title', 'aria-live': 'polite' });
  const content = h('div', { class: 'report-content' });
  root.append(h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '帳票'))), tabs.el, tabs.panel);
  tabs.panel.append(h('div', { class: 'card report-toolbar' }, h('div', { class: 'report-toolbar-main' }, pickerHost, accountHost), exportBox), h('div', { class: 'report-head' }, title), content);

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
    picker = createPeriodPicker({ period, years: years(), today, dateMode: tab.dateMode, yearOnly: tab.yearOnly, onChange: (p) => go(tab, p, tab.id === 'ledger' ? { account: query.account } : {}) });
    pickerHost.replaceChildren(picker.el);
    if (accountCb) accountCb.destroy();
    accountCb = null;
    accountHost.replaceChildren();
    if (tab.id === 'ledger' && data) {
      const items = [...data.accounts].sort(chartCompare).map((a) => ({ value: a.code, label: a.name, reading: a.reading, hint: a.wallet ? '口座' : KIND_LABEL[a.kind] }));
      accountCb = createCombobox({ items, value: query.account || null, listLabel: '科目の候補', placeholder: '例: げんきん、通信費', onSelect: (item) => item && item.value !== query.account && go(tab, period, { account: item.value }) });
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

  async function load() {
    const my = ++seq;
    if (view && view.destroy) view.destroy();
    view = null;
    title.textContent = `${tab.label}　${periodLabel(period)}`;
    content.replaceChildren();
    const stop = delayedSkeleton(content, () => skeletonBlock({ lines: 6, label: `${tab.label}を作っています` }));
    try {
      const v = await tab.view({ api: ctx.api, period, accounts: data.accounts, settings: data.settings, today, account: query.account || null, navigate: ctx.navigate });
      stop();
      if (disposed || my !== seq) {
        if (v.destroy) v.destroy();
        return;
      }
      view = v;
      lastCsv = v.csv;
      paintExport(v.csv);
      replace(content, v.node);
    } catch (err) {
      stop();
      if (disposed || my !== seq) return;
      lastCsv = null;
      paintExport(null);
      replace(content, banner({ kind: 'danger', title: `${tab.label}を作れませんでした`, body: err.message }));
    }
  }

  const unkeys = ctx.keys.register({ '[': () => (picker ? picker.shift(-1) : false), ']': () => (picker ? picker.shift(1) : false) });
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
