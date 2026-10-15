// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// コマンド一覧（Ctrl/Cmd+K）。画面の移動・主な操作・科目で探す・仕訳を文字で探す（keyboard.md、AC-10.1）。
// 一覧の並びは、打った文字の前方一致が上。仕訳を探す項目は、何か打ったときだけ最後に出す。

import { h, uid } from '../dom.js';
import { icon } from '../icons.js';
import { classifyKey, KEY } from '../ime.js';
import { filterItems } from '../kana.js';
import { openModal } from './dialog.js';

/**
 * @typedef {object} Command
 * @property {string} id
 * @property {string} label 画面に出す名前
 * @property {string} group まとまりの名前（「移動」「操作」）
 * @property {string} [reading] ひらがなの読み（探すため）
 * @property {string} [keywords] ほかに探せる言葉
 * @property {string} [icon]
 * @property {string} [hint] 右に出すキー（「g h」など）
 * @property {() => void} run
 */

/**
 * 例: const palette = createCommandPalette({ commands: () => list, accounts: () => items, searchJournal: (q) => ... })
 * @param {{ commands: () => Command[], accounts?: () => Array<{ value: string, label: string, reading?: string }>,
 *           openAccount?: (code: string) => void, searchJournal: (q: string) => void }} opts
 */
export function createCommandPalette(opts) {
  let ctrl = null;
  // 開いているかは、小窓そのもので見る。Esc で閉じた直後（閉じた知らせが届く前）に Ctrl/Cmd+K を押すと、
  // 開いたままとみなして閉じる側に回り、何も開かなかった。
  const isOpen = () => Boolean(ctrl && ctrl.el.open);

  function compute(q) {
    const text = q.trim();
    let res = filterItems(opts.commands(), text);
    if (text && opts.accounts && opts.openAccount) {
      const acc = filterItems(opts.accounts(), text)
        .slice(0, 5)
        .map((a) => ({ id: `acct-${a.value}`, label: `科目「${a.label}」の仕訳を見る`, group: '科目で探す', icon: 'list', run: () => opts.openAccount(a.value) }));
      res = res.concat(acc);
    }
    if (text) {
      res.push({ id: 'search', label: `仕訳を「${text}」で探す`, group: '仕訳を探す', icon: 'search', run: () => opts.searchJournal(text) });
    }
    return res;
  }

  function open() {
    if (isOpen()) return;
    const listId = uid('palette-list');
    const input = h('input', {
      type: 'text',
      class: 'input palette-input',
      role: 'combobox',
      'aria-expanded': 'true',
      'aria-controls': listId,
      'aria-autocomplete': 'list',
      'aria-label': 'コマンドを探す',
      autocomplete: 'off',
      spellcheck: 'false',
      placeholder: '画面・操作・科目の名前、または仕訳を探す言葉',
    });
    const list = h('div', { id: listId, role: 'listbox', class: 'palette-list', 'aria-label': 'コマンド' });
    let shown = [];
    let active = 0;

    function render() {
      shown = compute(input.value);
      active = Math.min(active, Math.max(shown.length - 1, 0));
      list.replaceChildren();
      let group = null;
      let groupEl = null;
      shown.forEach((c, i) => {
        if (c.group !== group) {
          group = c.group;
          const gid = uid('pg');
          groupEl = h('div', { role: 'group', 'aria-labelledby': gid }, h('div', { id: gid, class: 'palette-group', role: 'presentation' }, group));
          list.append(groupEl);
        }
        groupEl.append(
          h(
            'div',
            { id: `${listId}-${i}`, role: 'option', class: 'palette-option', 'aria-selected': 'false', dataset: { index: i } },
            icon(c.icon || 'chevron-right'),
            h('span', { class: 'palette-option-label' }, c.label),
            c.hint ? h('kbd', { 'aria-hidden': 'true' }, c.hint) : null,
          ),
        );
      });
      if (!shown.length) list.append(h('p', { class: 'combo-empty', role: 'presentation' }, '当てはまるコマンドがありません。'));
      sync();
    }

    function sync() {
      list.querySelectorAll('[role="option"]').forEach((o) => {
        const on = Number(o.dataset.index) === active;
        o.setAttribute('aria-selected', on ? 'true' : 'false');
        if (on) {
          input.setAttribute('aria-activedescendant', o.id);
          o.scrollIntoView({ block: 'nearest' });
        }
      });
      if (!shown.length) input.removeAttribute('aria-activedescendant');
    }

    function run(i) {
      const c = shown[i];
      if (!c) return;
      ctrl.close('run');
      c.run();
    }

    input.addEventListener('input', () => {
      active = 0;
      render();
    });
    input.addEventListener('keydown', (e) => {
      const kind = classifyKey(e);
      if (kind === KEY.COMPOSING) return;
      if (kind !== KEY.NORMAL) {
        e.preventDefault();
        return;
      }
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        if (!shown.length) return;
        active = e.key === 'ArrowDown' ? (active + 1) % shown.length : (active - 1 + shown.length) % shown.length;
        sync();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        run(active);
      }
    });
    list.addEventListener('mousedown', (e) => e.preventDefault());
    list.addEventListener('click', (e) => {
      const o = e.target.closest('[role="option"]');
      if (o) run(Number(o.dataset.index));
    });

    // 前の小窓の閉じた知らせが、あとから開いた小窓の記録を消さないよう、自分の小窓のときだけ外す。
    const mine = openModal({
      title: 'コマンド',
      className: 'palette',
      body: h('div', null, input, list),
      initialFocus: input,
      onClose: () => {
        if (ctrl === mine) ctrl = null;
      },
    });
    ctrl = mine;
    render();
  }

  return {
    open,
    close() {
      if (isOpen()) ctrl.close('cancel');
    },
    toggle() {
      if (isOpen()) ctrl.close('cancel');
      else open();
    },
    get isOpen() {
      return isOpen();
    },
  };
}
