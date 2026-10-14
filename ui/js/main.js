// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 起動。枠を組み、ルーター・キー・コマンド一覧・通知をつなぎ、状態（読み取り専用・デモ）を帯に出す。

import { installImeTracking } from './ime.js';
import { applyAppearance, cycleTheme, setTheme, setDensity, getDensity, themeLabel, onAppearanceChange } from './theme.js';
import { createRouter, buildHash } from './router.js';
import { createKeys, modLabel } from './keys.js';
import { api } from './api.js';
import * as state from './state.js';
import { buildShell } from './shell.js';
import { ROUTES, NOT_FOUND } from './routes.js';
import { initToasts, toast } from './components/toast.js';
import { banner } from './components/banner.js';
import { createCommandPalette } from './components/command-palette.js';
import { openShortcutsHelp } from './components/shortcuts-help.js';

applyAppearance();
installImeTracking(document);
initToasts(document.getElementById('toasts'));

const app = document.getElementById('app');
const outlet = document.getElementById('main');
const banners = document.getElementById('banners');

function navigate(hash, opts) {
  router.navigate(hash, opts);
}

function themeText() {
  return `表示: ${themeLabel()}`;
}

const palette = createCommandPalette({
  commands: () => commands(),
  accounts: () =>
    (state.peek('accounts') || [])
      .filter((a) => a.enabled)
      .map((a) => ({ value: a.code, label: a.name, reading: a.reading })),
  openAccount: (code) => navigate(buildHash('/journal', { account: code })),
  searchJournal: (q) => navigate(buildHash('/journal', { q })),
});

const shell = buildShell({
  app,
  modLabel: modLabel(),
  onPalette: () => palette.open(),
  onHelp: () => openShortcutsHelp(),
  onTheme: () => {
    cycleTheme();
    toast.info(`表示を「${themeLabel()}」にしました。`);
  },
  themeText,
});
onAppearanceChange(() => shell.setThemeText(themeText()));

function commands() {
  const go = (label, reading, hash, iconName, hint) => ({ id: `go-${hash}`, label, reading, group: '移動', icon: iconName, hint, run: () => navigate(hash) });
  return [
    go('ホーム', 'ほーむ', '#/', 'home', 'g h'),
    go('取引を登録', 'とりひきをとうろく', '#/entry', 'plus-square', 'n'),
    go('未確認', 'みかくにん', '#/inbox', 'inbox', 'g i'),
    go('取込（明細のCSV）', 'とりこみ めいさい', '#/import', 'import'),
    go('仕訳帳', 'しわけちょう', '#/journal', 'book', 'g j'),
    go('帳票', 'ちょうひょう', '#/reports', 'chart', 'g r'),
    go('帳票: 損益計算書', 'そんえきけいさんしょ ちょうひょう', '#/reports/income-statement', 'chart'),
    go('帳票: 貸借対照表', 'たいしゃくたいしょうひょう ちょうひょう', '#/reports/balance-sheet', 'chart'),
    go('帳票: 試算表', 'しさんひょう ちょうひょう', '#/reports/trial-balance', 'chart'),
    go('帳票: 総勘定元帳', 'そうかんじょうもとちょう ちょうひょう', '#/reports/ledger', 'chart'),
    go('帳票: 消費税', 'しょうひぜい ちょうひょう', '#/reports/tax-summary', 'chart'),
    go('口座', 'こうざ', '#/wallets', 'wallet', 'g w'),
    go('勘定科目', 'かんじょうかもく', '#/accounts', 'list'),
    go('取引先', 'とりひきさき', '#/partners', 'building'),
    go('設定', 'せってい', '#/settings', 'sliders', 'g s'),
    go('台帳を検査する（設定のデータ）', 'だいちょうをけんさ はっしゅ しょうごう', '#/settings?section=data', 'shield-check'),
    go('期首残高を見直す（設定）', 'きしゅざんだか もといれきん', '#/settings?section=opening', 'wallet'),
    go('口座を足す（口座の画面）', 'こうざをたす', '#/wallets', 'plus'),
    { id: 'theme-light', label: '表示をライトにする', reading: 'ひょうじ らいと あかるい', group: '操作', icon: 'sun', run: () => setTheme('light') },
    { id: 'theme-dark', label: '表示をダークにする', reading: 'ひょうじ だーく くらい', group: '操作', icon: 'moon', run: () => setTheme('dark') },
    { id: 'theme-os', label: '表示をOSに合わせる', reading: 'ひょうじ おーえす', group: '操作', icon: 'monitor', run: () => setTheme('os') },
    {
      id: 'density',
      label: getDensity() === 'compact' ? '行の密度を標準にする' : '行の密度を狭くする',
      reading: 'ぎょうのみつど',
      group: '操作',
      icon: 'list',
      run: () => setDensity(getDensity() === 'compact' ? 'standard' : 'compact'),
    },
    { id: 'help', label: 'ショートカットの一覧', reading: 'しょーとかっと', group: '操作', icon: 'keyboard', hint: '?', run: () => openShortcutsHelp() },
  ];
}

const keys = createKeys({
  navigate,
  openPalette: () => palette.open(),
  togglePalette: () => palette.toggle(),
  openHelp: () => openShortcutsHelp(),
  focusSearch: () => {
    const el = outlet.querySelector('[data-search]');
    if (!el) return false;
    el.focus();
    if (typeof el.select === 'function') el.select();
    return true;
  },
});

// 初期設定が済むまでは、どの画面に入っても初期設定へ移す（SC-00）。状態をまだ読めていないときは止めない。
function setupGuard(path) {
  const st = state.peek('status');
  return st && st.setup_done === false && path !== '/setup' ? '#/setup' : null;
}

const router = createRouter({
  routes: ROUTES,
  outlet,
  notFound: NOT_FOUND,
  makeContext: (base) => ({ ...base, api, state, toast, keys, navigate, router: { navigate, buildHash } }),
  onChange: (cur) => shell.setActive(cur.route.nav),
  guard: setupGuard,
});

// 帯: デモの表示と、読み取り専用（台帳の検証の失敗）。読み取り専用の帯の id は、書き込みのボタンの説明に結ぶ。
function paintBanners(st) {
  banners.replaceChildren();
  if (!st) return;
  if (st.mode === 'demo') {
    banners.append(banner({ kind: 'info', title: 'デモのデータです', body: '架空の帳簿です。本物の帳簿の置き場所には書き込みません。' }));
  }
  if (state.isReadOnly()) {
    const loaded = st.ledger && Number.isInteger(st.ledger.events_loaded) && Number.isInteger(st.ledger.events_in_db) && st.ledger.events_loaded < st.ledger.events_in_db
      ? `表示しているのは、全${st.ledger.events_in_db}件の記録のうち、壊れた手前までの${st.ledger.events_loaded}件です。`
      : '';
    banners.append(
      banner({
        kind: 'danger',
        id: 'banner-readonly',
        title: '読み取り専用で開いています',
        body: `${state.readOnlyReason()}${loaded}設定の「データ」で、検査の結果を確かめてください。`,
      }),
    );
  }
}

state.subscribe('status', (st) => {
  paintBanners(st);
  shell.setCount('inbox', st && st.inbox ? st.inbox.draft : 0);
});
state.subscribe('settings', (s) => shell.setBusinessName(s ? s.business_name : ''));

// 「本文へ移動」: アドレスの #/ を壊さないよう、リンクの移動は止めて、見出しへフォーカスを移す。
document.getElementById('skip-link').addEventListener('click', (e) => {
  e.preventDefault();
  const target = outlet.querySelector('h1') || outlet;
  if (!target.hasAttribute('tabindex')) target.setAttribute('tabindex', '-1');
  target.focus();
});

state.load('settings').catch(() => {});
state.load('accounts').catch(() => {});
// 状態（初期設定が済んだか）を読んでから、最初の画面を出す。読めなくても画面は出す（理由は通知で伝える）。
state
  .load('status')
  .catch((err) => toast.error(err.message))
  .finally(() => router.start());
// 初期設定が済んでいないと分かったら（別のタブで帳簿を作り直したときなど）、いまの画面から移す。
state.subscribe('status', (st) => {
  if (st && st.setup_done === false && !location.hash.startsWith('#/setup')) navigate('#/setup', { replace: true });
});
