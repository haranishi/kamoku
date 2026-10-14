// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 候補つきの入力欄（ARIA 1.2 のコンボボックス）。科目・口座・取引先の欄と、未確認の表の科目の欄で使う。
// 変換中の文字でも候補を絞るが、選ぶのは変換が終わってからのEnter・Tab・クリックだけ（keyboard.md）。
// 候補の一覧は position: fixed で出す。表の入れ物（横スクロール）の中でも、一覧が切れないようにするため。

import { h, uid, selectAllOnFocus } from '../dom.js';
import { icon } from '../icons.js';
import { classifyKey, KEY } from '../ime.js';
import { filterItems, exactMatches } from '../kana.js';

/** 打った文字で候補を絞る（DOM不要。試験用に外へ出している）。前方一致が上。 */
export function filterOptions(items, query, limit = 100) {
  return filterItems(items, query).slice(0, limit);
}

/** 欄を離れたときに自動で選ぶ候補。名前か読みがちょうど一致する候補が1つだけのとき、それを返す。 */
export function pickOnBlur(items, text) {
  const m = exactMatches(items, text);
  return m.length === 1 ? m[0] : null;
}

/**
 * 候補つきの入力欄を作る。
 * @param {object} opts
 * @param {Array<{ value: string, label: string, reading?: string, hint?: string, keywords?: string }>} opts.items 候補（並びは呼び出し側が決めた優先順）
 * @param {string|null} [opts.value] 最初に選んでおく候補の value（自由入力なら文字）
 * @param {boolean} [opts.allowFree] 候補に無い文字も値にする（取引先）
 * @param {boolean} [opts.arrowOpens] 閉じているときの上下の矢印で一覧を開く（既定は真）。偽なら矢印は外へ回す（未確認の表）。Alt+↓ は常に開く
 * @param {(item: object|null, info: { via: 'enter'|'tab'|'click'|'blur'|'input'|'api', event?: Event }) => (void|'stop')} [opts.onSelect]
 *   候補が決まったとき（または選択が外れたとき null）に呼ぶ。Enterで選んだときに 'stop' を返すと、そのEnterは外へ伝わらない。
 *   返さなければ、Enterは外の処理（「次の欄へ」「確定候補にして次の行へ」）にそのまま伝わる。
 * @param {(text: string) => void} [opts.onInput] 文字が変わるたびに呼ぶ
 */
export function createCombobox(opts) {
  const id = opts.id || uid('combo');
  const allowFree = Boolean(opts.allowFree);
  const arrowOpens = opts.arrowOpens !== false;
  // 自由入力の欄では、先頭の候補に自動で印を付けない（Enterで打った文字が候補に置き換わるのを防ぐ）。
  const autoMark = opts.autoMark ?? !allowFree;
  let items = opts.items || [];
  let selected = null;
  let shown = [];
  let active = -1;
  let open = false;

  if (opts.value !== undefined && opts.value !== null && opts.value !== '') {
    selected = items.find((i) => i.value === opts.value) || null;
  }

  const listId = `${id}-list`;
  const input = h('input', {
    id,
    type: 'text',
    class: ['input', opts.inputClass],
    role: 'combobox',
    'aria-autocomplete': 'list',
    'aria-expanded': 'false',
    'aria-controls': listId,
    autocomplete: 'off',
    autocapitalize: 'off',
    spellcheck: 'false',
    placeholder: opts.placeholder || '',
  });
  if (selected) input.value = selected.label;
  else if (allowFree && opts.value) input.value = String(opts.value);
  const list = h('ul', { id: listId, class: 'combo-list', role: 'listbox', 'aria-label': opts.listLabel || '候補', hidden: true });
  const el = h('div', { class: ['combo', opts.className] }, input, icon('chevron-down', { size: 16, className: 'combo-chevron' }), list);

  function render() {
    list.replaceChildren();
    if (!shown.length) list.append(h('li', { class: 'combo-empty', role: 'presentation' }, opts.emptyText || '候補がありません'));
    shown.forEach((item, i) => {
      list.append(
        h(
          'li',
          { id: `${id}-opt-${i}`, class: 'combo-option', role: 'option', 'aria-selected': 'false', dataset: { index: i } },
          h('span', { class: 'combo-option-label' }, item.label),
          item.hint ? h('span', { class: 'combo-option-hint' }, item.hint) : null,
        ),
      );
    });
    sync(true);
  }

  function sync(scroll) {
    const options = list.querySelectorAll('[role="option"]');
    options.forEach((li, i) => li.setAttribute('aria-selected', i === active ? 'true' : 'false'));
    if (active >= 0 && options[active]) {
      input.setAttribute('aria-activedescendant', options[active].id);
      if (scroll) options[active].scrollIntoView({ block: 'nearest' });
    } else {
      input.removeAttribute('aria-activedescendant');
    }
  }

  function refilter() {
    const text = input.value;
    if (selected && text === selected.label) {
      // 選んである状態で開いたときは、全部の候補を見せて、いまの候補に印を付ける。
      shown = items.slice(0, opts.limit || 100);
      active = shown.indexOf(selected);
    } else {
      shown = filterOptions(items, text, opts.limit || 100);
      active = shown.length && autoMark && text.trim() ? 0 : -1;
    }
    render();
    if (open) position();
  }

  function position() {
    const r = input.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const width = Math.min(Math.max(r.width, 240), 420, vw - 16);
    const below = vh - r.bottom - 8;
    const above = r.top - 8;
    list.style.width = `${Math.round(width)}px`;
    list.style.left = `${Math.round(Math.max(8, Math.min(r.left, vw - width - 8)))}px`;
    if (below < 180 && above > below) {
      list.style.top = 'auto';
      list.style.bottom = `${Math.round(vh - r.top + 4)}px`;
      list.style.maxHeight = `${Math.round(Math.min(288, above))}px`;
    } else {
      list.style.bottom = 'auto';
      list.style.top = `${Math.round(r.bottom + 4)}px`;
      list.style.maxHeight = `${Math.round(Math.min(288, Math.max(below, 120)))}px`;
    }
  }

  const onViewportChange = () => {
    if (open) position();
  };

  function openList() {
    if (open) return;
    open = true;
    list.hidden = false;
    input.setAttribute('aria-expanded', 'true');
    position();
    sync(true);
    window.addEventListener('scroll', onViewportChange, { capture: true, passive: true });
    window.addEventListener('resize', onViewportChange);
  }

  function closeList() {
    if (!open) return;
    open = false;
    list.hidden = true;
    active = -1;
    input.setAttribute('aria-expanded', 'false');
    input.removeAttribute('aria-activedescendant');
    window.removeEventListener('scroll', onViewportChange, { capture: true });
    window.removeEventListener('resize', onViewportChange);
  }

  function choose(item, via, event) {
    selected = item;
    input.value = item ? item.label : '';
    closeList();
    return opts.onSelect ? opts.onSelect(item, { via, event }) : undefined;
  }

  input.addEventListener('input', () => {
    if (selected && input.value !== selected.label) {
      selected = null;
      if (!allowFree && opts.onSelect) opts.onSelect(null, { via: 'input' });
    }
    refilter();
    openList();
    if (opts.onInput) opts.onInput(input.value);
  });

  // 変換中の文字でも絞る。compositionupdate の時点では value がまだ古いので、次の番で読む。
  input.addEventListener('compositionupdate', () => {
    setTimeout(() => {
      if (document.activeElement !== input) return;
      refilter();
      openList();
    }, 0);
  });

  // 入っている名前の上から、そのまま打ち直せるようにする（既定の口座を別の口座に変えるときなど）。
  selectAllOnFocus(input);

  input.addEventListener('click', () => {
    if (open) return;
    refilter();
    openList();
  });

  input.addEventListener('keydown', (e) => {
    const kind = classifyKey(e);
    if (kind === KEY.COMPOSING) return;
    if (kind !== KEY.NORMAL) {
      e.preventDefault();
      return;
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (!open) {
        if (!arrowOpens && !(e.altKey && e.key === 'ArrowDown')) return;
        e.preventDefault();
        refilter();
        openList();
        if (active < 0 && shown.length) active = 0;
        sync(true);
        return;
      }
      e.preventDefault();
      if (!shown.length) return;
      active = e.key === 'ArrowDown' ? Math.min(active + 1, shown.length - 1) : Math.max(active - 1, 0);
      sync(true);
    } else if (e.key === 'Enter') {
      if (open && active >= 0 && shown[active]) {
        if (choose(shown[active], 'enter', e) === 'stop') {
          e.preventDefault();
          e.stopPropagation();
        }
      }
    } else if (e.key === 'Tab') {
      if (open && active >= 0 && shown[active]) choose(shown[active], 'tab', e);
      else closeList();
    } else if (e.key === 'Escape' && open) {
      // 開いた一覧を閉じるだけにする。ダイアログや画面の「やめる」には伝えない。
      e.preventDefault();
      e.stopPropagation();
      closeList();
    }
  });

  input.addEventListener('blur', () => {
    closeList();
    const text = input.value.trim();
    if (allowFree) return;
    if (!selected && text) {
      const m = pickOnBlur(items, text);
      if (m) choose(m, 'blur');
    } else if (selected && !text) {
      choose(null, 'blur');
    }
  });

  // 候補を押したときに、入力欄のフォーカスを失わない。
  list.addEventListener('mousedown', (e) => e.preventDefault());
  list.addEventListener('click', (e) => {
    const li = e.target.closest('[role="option"]');
    if (!li) return;
    const item = shown[Number(li.dataset.index)];
    if (item) {
      choose(item, 'click', e);
      input.focus();
    }
  });
  list.addEventListener('mousemove', (e) => {
    const li = e.target.closest('[role="option"]');
    if (!li) return;
    const i = Number(li.dataset.index);
    if (i !== active) {
      active = i;
      sync(false);
    }
  });

  return {
    el,
    input,
    list,
    /** 選んだ候補の value。自由入力の欄では、打った文字（前後の空白を除く）。 */
    get value() {
      if (allowFree) return input.value.trim();
      return selected ? selected.value : null;
    },
    /** 選んだ候補（無ければ null）。 */
    get item() {
      return selected;
    },
    get isOpen() {
      return open;
    },
    /** 候補を入れ替える（種別が変わって並びが変わるときなど）。選択は value で保つ。 */
    setItems(next) {
      items = next;
      if (selected) {
        selected = items.find((i) => i.value === selected.value) || null;
        if (selected) input.value = selected.label;
      }
      if (open) refilter();
    },
    /** 値を入れる。notify が真なら onSelect も呼ぶ。 */
    setValue(value, { notify = false } = {}) {
      const item = items.find((i) => i.value === value) || null;
      selected = item;
      input.value = item ? item.label : allowFree ? String(value || '') : '';
      closeList();
      if (notify && opts.onSelect) opts.onSelect(item, { via: 'api' });
    },
    open() {
      refilter();
      openList();
    },
    close: closeList,
    focus() {
      input.focus();
    },
    destroy() {
      closeList();
    },
  };
}
