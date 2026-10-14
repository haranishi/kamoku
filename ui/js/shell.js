// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 共通の枠（screens-main.md）: 左のサイドバー、768px未満の上の帯と下の5項目のバー、全項目のメニュー（引き出し）。
// サイドバーの並びは「ホーム」「取引（登録・未確認・取込・仕訳帳）」「帳票」「登録簿（口座・勘定科目・取引先）」「設定」。

import { h } from './dom.js';
import { icon } from './icons.js';
import { iconButton } from './components/fields.js';
import { openDrawer } from './components/drawer.js';

/** サイドバーの項目。short は 768〜1023px の細いサイドバーで出す短い名前。 */
export const NAV = [
  { id: 'home', label: 'ホーム', href: '#/', icon: 'home', kbd: 'g h' },
  {
    group: '取引',
    items: [
      { id: 'entry', label: '取引を登録', short: '登録', href: '#/entry', icon: 'plus-square', kbd: 'n' },
      { id: 'inbox', label: '未確認', href: '#/inbox', icon: 'inbox', kbd: 'g i' },
      { id: 'import', label: '取込', href: '#/import', icon: 'import' },
      { id: 'journal', label: '仕訳帳', href: '#/journal', icon: 'book', kbd: 'g j' },
    ],
  },
  { id: 'reports', label: '帳票', href: '#/reports', icon: 'chart', kbd: 'g r' },
  {
    group: '登録簿',
    items: [
      { id: 'wallets', label: '口座', href: '#/wallets', icon: 'wallet', kbd: 'g w' },
      { id: 'accounts', label: '勘定科目', short: '科目', href: '#/accounts', icon: 'list' },
      { id: 'partners', label: '取引先', href: '#/partners', icon: 'building' },
    ],
  },
  { id: 'settings', label: '設定', href: '#/settings', icon: 'sliders', kbd: 'g s' },
];

const BOTTOM = ['home', 'entry', 'inbox', 'journal'];
const RAIL_KEY = 'kamoku.rail';

function flatNav() {
  return NAV.flatMap((n) => (n.group ? n.items : [n]));
}

function brandMark() {
  // 帳簿を表した印（自作のアイコンを、藍の四角に白抜きで置く）。
  return h('span', { class: 'brand-mark', 'aria-hidden': 'true' }, icon('book', { size: 18 }));
}

/**
 * 枠を組み立てる。
 * @param {{ app: HTMLElement, onPalette: () => void, onHelp: () => void, onTheme: () => void, themeText: () => string }} opts
 */
export function buildShell(opts) {
  const countValues = new Map();
  const countEls = new Map();
  let currentNav = null;

  function paintCount(el, n) {
    el.hidden = !n;
    el.textContent = n ? String(n) : '';
    if (n) el.setAttribute('aria-label', `${n}件`);
    else el.removeAttribute('aria-label');
  }

  function navLink(item, { withKbd = true, track = true } = {}) {
    const count = h('span', { class: 'nav-count', hidden: true });
    paintCount(count, countValues.get(item.id) || 0);
    if (track) countEls.set(item.id, (countEls.get(item.id) || []).concat(count));
    const a = h(
      'a',
      { class: 'nav-item', href: item.href, dataset: { nav: item.id } },
      icon(item.icon),
      h(
        'span',
        { class: 'nav-label' },
        h('span', { class: 'nav-label-long' }, item.label),
        item.short ? h('span', { class: 'nav-label-short' }, item.short) : null,
      ),
      count,
      withKbd && item.kbd ? h('kbd', { class: 'nav-kbd', 'aria-hidden': 'true' }, item.kbd) : null,
    );
    if (!item.short) a.querySelector('.nav-label-long').classList.remove('nav-label-long');
    if (item.id === currentNav) a.setAttribute('aria-current', 'page');
    return a;
  }

  function navList(withKbd, track) {
    const link = (i) => h('li', null, navLink(i, { withKbd, track }));
    return h(
      'ul',
      { class: 'nav' },
      NAV.map((n) =>
        n.group
          ? h('li', { class: 'nav-group' }, h('span', { class: 'nav-group-label' }, n.group), h('ul', { class: 'nav' }, n.items.map(link)))
          : link(n),
      ),
    );
  }

  const businessName = h('p', { class: 'sidebar-business' });
  const themeText = h('span', null, opts.themeText());
  const railToggle = iconButton({ iconName: 'panel', label: 'サイドバーを広げる', className: 'rail-toggle', 'aria-expanded': 'false' });
  // 外側（背景と境目の線）はページの高さいっぱいに伸ばし、内側だけを画面に付いてこさせる。
  const sidebar = h(
    'nav',
    { class: 'sidebar', 'aria-label': 'メインメニュー' },
    h(
      'div',
      { class: 'sidebar-inner' },
      h('div', { class: 'sidebar-head' }, h('a', { class: 'brand', href: '#/' }, brandMark(), h('span', { class: 'brand-text' }, 'kamoku')), railToggle),
      businessName,
      navList(true, true),
      h(
        'div',
        { class: 'sidebar-foot' },
        h('button', { type: 'button', class: 'nav-item', onclick: opts.onPalette, dataset: { action: 'palette' } }, icon('search'), h('span', { class: 'nav-label' }, 'コマンド'), h('kbd', { class: 'nav-kbd' }, opts.modLabel, ' K')),
        h(
          'button',
          { type: 'button', class: 'nav-item', onclick: opts.onHelp },
          icon('keyboard'),
          h('span', { class: 'nav-label' }, h('span', { class: 'nav-label-long' }, 'ショートカット'), h('span', { class: 'nav-label-short' }, 'キー')),
          h('kbd', { class: 'nav-kbd' }, '?'),
        ),
        h(
          'button',
          { type: 'button', class: 'nav-item', onclick: opts.onTheme, dataset: { action: 'theme' } },
          icon('monitor'),
          h('span', { class: 'nav-label' }, h('span', { class: 'nav-label-long' }, themeText), h('span', { class: 'nav-label-short' }, '表示')),
        ),
      ),
    ),
  );

  let rail = 'closed';
  try {
    rail = localStorage.getItem(RAIL_KEY) === 'open' ? 'open' : 'closed';
  } catch {
    rail = 'closed';
  }
  function setRail(next) {
    rail = next;
    opts.app.dataset.rail = rail;
    railToggle.setAttribute('aria-expanded', rail === 'open' ? 'true' : 'false');
    railToggle.setAttribute('aria-label', rail === 'open' ? 'サイドバーを細くする' : 'サイドバーを広げる');
    railToggle.title = railToggle.getAttribute('aria-label');
    try {
      localStorage.setItem(RAIL_KEY, rail);
    } catch {
      // 保存できなくても、この画面の間は効く。
    }
  }
  railToggle.addEventListener('click', () => setRail(rail === 'open' ? 'closed' : 'open'));
  setRail(rail);

  const topbar = h(
    'header',
    { class: 'topbar' },
    h('a', { class: 'brand', href: '#/' }, brandMark(), h('span', { class: 'brand-text' }, 'kamoku')),
    h(
      'div',
      { class: 'topbar-actions' },
      iconButton({ iconName: 'search', label: 'コマンド一覧を開く', onClick: opts.onPalette }),
      iconButton({ iconName: 'monitor', label: '表示の明るさを切り替える', onClick: opts.onTheme, dataset: { action: 'theme' } }),
    ),
  );

  const byId = new Map(flatNav().map((i) => [i.id, i]));
  const menuButton = h('button', { type: 'button', class: 'bottomnav-item', 'aria-haspopup': 'dialog' }, icon('menu'), h('span', null, 'メニュー'));
  const bottomnav = h(
    'nav',
    { class: 'bottomnav', 'aria-label': 'よく使う画面' },
    BOTTOM.map((id) => {
      const item = byId.get(id);
      return h('a', { class: 'bottomnav-item', href: item.href, dataset: { nav: id } }, icon(item.icon), h('span', null, item.short || item.label));
    }),
    menuButton,
  );
  menuButton.addEventListener('click', () => {
    const list = navList(false, false);
    const d = openDrawer({ title: 'メニュー', side: 'left', className: 'menu-drawer', body: list });
    list.addEventListener('click', (e) => {
      if (e.target.closest('a')) d.close();
    });
  });

  opts.app.prepend(topbar, sidebar);
  opts.app.append(bottomnav);

  return {
    /** いまの画面の項目に印を付ける。 */
    setActive(navId) {
      currentNav = navId;
      document.querySelectorAll('[data-nav]').forEach((a) => {
        if (a.dataset.nav === navId) a.setAttribute('aria-current', 'page');
        else a.removeAttribute('aria-current');
      });
    },
    /** 項目の横の件数（未確認の件数など）。0なら隠す。 */
    setCount(navId, n) {
      countValues.set(navId, n);
      for (const el of countEls.get(navId) || []) paintCount(el, n);
    },
    setBusinessName(text) {
      businessName.textContent = text || '';
    },
    setThemeText(text) {
      themeText.textContent = text;
    },
  };
}
