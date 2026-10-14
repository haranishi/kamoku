// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 画面の一覧。画面を足すときは、screens/ にファイルを置き、ここに1行足す（ui/README.md）。
// nav は、サイドバーで印を付ける項目（shell.js の NAV の id）。

import * as home from './screens/home.js';
import * as entry from './screens/entry.js';
import * as journal from './screens/journal.js';
import * as inbox from './screens/inbox.js';
import * as importScreen from './screens/import.js';
import * as reports from './screens/reports.js';
import * as wallets from './screens/wallets.js';
import * as accounts from './screens/accounts.js';
import * as partners from './screens/partners.js';
import * as settings from './screens/settings.js';
import * as setup from './screens/setup.js';
import * as placeholder from './screens/placeholder.js';

// 帳票のタブの名前を、ページの題名にも出す（読み上げとブラウザの履歴で区別できるように）。
function reportTitle(ctx) {
  const t = reports.REPORT_TABS.find((x) => x.id === ctx.params.name);
  return t ? `${t.label}（帳票）` : '帳票';
}

export const ROUTES = [
  { path: '/', screen: home, title: 'ホーム', nav: 'home' },
  { path: '/entry', screen: entry, title: '取引を登録', nav: 'entry' },
  { path: '/entry/:id', screen: entry, title: '取引を訂正', nav: 'journal' },
  { path: '/journal', screen: journal, title: '仕訳帳', nav: 'journal' },
  { path: '/journal/:id', screen: journal, title: '仕訳帳', nav: 'journal' },
  { path: '/inbox', screen: inbox, title: '未確認', nav: 'inbox' },
  { path: '/import', screen: importScreen, title: '明細を取り込む', nav: 'import' },
  { path: '/reports', screen: reports, title: '帳票', nav: 'reports' },
  { path: '/reports/:name', screen: reports, title: reportTitle, nav: 'reports' },
  { path: '/wallets', screen: wallets, title: '口座', nav: 'wallets' },
  { path: '/accounts', screen: accounts, title: '勘定科目', nav: 'accounts' },
  { path: '/partners', screen: partners, title: '取引先', nav: 'partners' },
  { path: '/settings', screen: settings, title: '設定', nav: 'settings' },
  { path: '/setup', screen: setup, title: '初期設定', nav: null },
];

/** どの経路にも当たらないとき。 */
export const NOT_FOUND = { path: '*', screen: placeholder, title: 'ページが見つかりません', nav: null, notFound: true };
