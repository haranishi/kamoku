// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 候補つきの入力欄（ARIA 1.2 のコンボボックス）。科目・口座・取引先の欄と、未確認の表の科目の欄で使う。
// 変換中の文字でも候補を絞るが、選ぶのは変換が終わってからのEnter・Tab・クリックだけ（keyboard.md）。
// 候補の一覧は position: fixed で出す。表の入れ物（横スクロール）の中でも、一覧が切れないようにするため。

import { h, uid, selectAllOnFocus } from '../dom.js';
import { icon } from '../icons.js';
import { classifyKey, KEY } from '../ime.js';
import { filterItems, exactMatches, fold } from '../kana.js';
import { phrase } from './phrase.js';

/** 打った文字で候補を絞る（DOM不要。試験用に外へ出している）。前方一致が上。 */
export function filterOptions(items, query, limit = 100) {
  return filterItems(items, query).slice(0, limit);
}

/** 欄を離れたときに自動で選ぶ候補。名前か読みがちょうど一致する候補が1つだけのとき、それを返す。 */
export function pickOnBlur(items, text) {
  const m = exactMatches(items, text);
  return m.length === 1 ? m[0] : null;
}

/** 名前（label）が、打った文字とちょうど同じ候補（全角と半角、かなの違いは同じとみなす）。 */
export function sameName(items, text) {
  const q = fold(text);
  return q ? items.filter((i) => fold(i.label) === q) : [];
}

/**
 * 新しい名前を足せる欄（createLabel あり）の候補の並び。名前がちょうど同じ候補を先頭にし、
 * どの候補ともちょうど同じでなければ、末尾に「新しい名前として足す」の行（isNew）を足す。DOM不要（試験用に外へ出している）。
 * @returns {Array<object>}
 */
export function optionsWithNew(items, text, createLabel, limit = 100) {
  const found = filterOptions(items, text, limit);
  const same = sameName(found, text);
  const list = [...same, ...found.filter((i) => !same.includes(i))];
  const name = String(text || '').trim();
  if (name && !sameName(items, name).length) list.push({ value: name, label: name, isNew: true, optionLabel: createLabel(name) });
  return list;
}

/** 一覧の高さの上限（6件ぶん。指で押す幅の1件は44px）。 */
const LIST_MAX_H = 288;

/**
 * 候補の一覧を、入力欄の下に出すか上に出すかと、その大きさ（DOM不要。試験用に外へ出している）。
 * 下の余白が6件ぶんに足りず、上のほうが広いときは上に出す（幅390で、画面の下寄りの欄の一覧が約5件しか見えなかった）。
 * 上に出すときは、一覧の下の端を欄の上の端より上に置き、欄を隠さない。
 * @param {{ top: number, bottom: number, left: number, width: number }} r 入力欄の四角
 * @returns {{ above: boolean, width: number, left: number, top: number|null, bottom: number|null, maxHeight: number }}
 */
export function listPlacement(r, vw, vh) {
  const width = Math.min(Math.max(r.width, 240), 420, vw - 16);
  const left = Math.max(8, Math.min(r.left, vw - width - 8));
  const below = vh - r.bottom - 8;
  const above = r.top - 8;
  if (below < LIST_MAX_H && above > below) {
    return { above: true, width, left, top: null, bottom: vh - r.top + 4, maxHeight: Math.min(LIST_MAX_H, above) };
  }
  return { above: false, width, left, top: r.bottom + 4, bottom: null, maxHeight: Math.min(LIST_MAX_H, Math.max(below, 120)) };
}

/**
 * 候補つきの入力欄を作る。
 * @param {object} opts
 * @param {Array<{ value: string, label: string, reading?: string, hint?: string, keywords?: string }>} opts.items 候補（並びは呼び出し側が決めた優先順）
 * @param {string|null} [opts.value] 最初に選んでおく候補の value（自由入力なら文字）
 * @param {boolean} [opts.allowFree] 候補に無い文字も値にする（取引先）
 * @param {(text: string) => string} [opts.createLabel] allowFree の欄で、候補の末尾に「新しい名前として足す」の行を出す（行の文字を返す）。
 *   あるときは、科目の欄と同じく先頭の候補に印を付け、Enterで決める。打った文字は、その行を選んだときだけ新しい名前になる（item.isNew）
 * @param {string|((text: string) => string)} [opts.noMatch] 候補が0件のとき、一覧を閉じて、欄の下にこの一言を出す（無ければ「候補がありません」の行）
 * @param {boolean} [opts.enterNeedsMatch] 打った文字に合う候補が無いまま Enter を押したら、次の欄へ進ませず、noMatch の一言を出し直す（記帳の欄。保存まで気づけないため）
 * @param {boolean} [opts.arrowOpens] 閉じているときの上下の矢印で一覧を開く（既定は真）。偽なら矢印は外へ回す（未確認の表）。Alt+↓ は常に開く
 * @param {(item: object|null, info: { via: 'enter'|'tab'|'click'|'blur'|'input'|'api', event?: Event }) => (void|'stop')} [opts.onSelect]
 *   候補が決まったとき（または選択が外れたとき null）に呼ぶ。Enterで選んだときに 'stop' を返すと、そのEnterは外へ伝わらない。
 *   返さなければ、Enterは外の処理（「次の欄へ」「確定候補にして次の行へ」）にそのまま伝わる。
 * @param {(text: string) => void} [opts.onInput] 文字が変わるたびに呼ぶ
 */
export function createCombobox(opts) {
  const id = opts.id || uid('combo');
  const allowFree = Boolean(opts.allowFree);
  const createLabel = allowFree && typeof opts.createLabel === 'function' ? opts.createLabel : null;
  const arrowOpens = opts.arrowOpens !== false;
  // 自由入力の欄では、先頭の候補に自動で印を付けない（Enterで打った文字が候補に置き換わるのを防ぐ）。
  // 「新しい名前として足す」の行がある欄は、打った文字を使う道がその行にあるので、科目の欄と同じく先頭に印を付ける。
  const autoMark = opts.autoMark ?? (!allowFree || Boolean(createLabel));
  let items = opts.items || [];
  let selected = null;
  let shown = [];
  let active = -1;
  let open = false;

  // 候補に無い名前（取引先のマスタから外れた名前など）は、入っていた値として、選んだものとみなす（新しい名前ではない）。
  const keptItem = (value) => ({ value: String(value), label: String(value), kept: true });
  if (opts.value !== undefined && opts.value !== null && opts.value !== '') {
    selected = items.find((i) => i.value === opts.value) || (createLabel ? keptItem(opts.value) : null);
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
  // 候補が0件のときの一言（noMatch）。見た目は一覧と同じ場所に出し、読み上げは、いつも在る見えない欄（aria-live）で伝える。
  // 一覧（listbox）は閉じたままにする（候補の無い一覧が開いていると、読み上げで「一覧が開いた」のに何も無い）。
  const msgBox = opts.noMatch ? h('div', { class: 'combo-msg', 'aria-hidden': 'true', hidden: true }) : null;
  const msgLive = opts.noMatch ? h('span', { class: 'visually-hidden', 'aria-live': 'polite' }) : null;
  const el = h('div', { class: ['combo', opts.className] }, input, icon('chevron-down', { size: 16, className: 'combo-chevron' }), list, msgBox, msgLive);

  function render() {
    list.replaceChildren();
    if (!shown.length && !opts.noMatch) list.append(h('li', { class: 'combo-empty', role: 'presentation' }, opts.emptyText || '候補がありません'));
    shown.forEach((item, i) => {
      list.append(
        h(
          'li',
          { id: `${id}-opt-${i}`, class: ['combo-option', item.isNew && 'combo-option--new'], role: 'option', 'aria-selected': 'false', dataset: { index: i } },
          h('span', { class: 'combo-option-label' }, item.isNew ? [icon('plus', { size: 16 }), h('span', null, item.optionLabel)] : item.label),
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
    const limit = opts.limit || 100;
    if (selected && text === selected.label) {
      // 選んである状態で開いたときは、全部の候補を見せて、いまの候補に印を付ける。
      shown = items.slice(0, limit);
      active = shown.indexOf(selected);
    } else {
      shown = createLabel ? optionsWithNew(items, text, createLabel, limit) : filterOptions(items, text, limit);
      active = shown.length && autoMark && text.trim() ? 0 : -1;
    }
    render();
    if (open) position(list);
  }

  function position(target) {
    const p = listPlacement(input.getBoundingClientRect(), window.innerWidth, window.innerHeight);
    target.style.width = `${Math.round(p.width)}px`;
    target.style.left = `${Math.round(p.left)}px`;
    target.style.top = p.above ? 'auto' : `${Math.round(p.top)}px`;
    target.style.bottom = p.above ? `${Math.round(p.bottom)}px` : 'auto';
    target.style.maxHeight = `${Math.round(p.maxHeight)}px`;
  }

  const msgShown = () => Boolean(msgBox && !msgBox.hidden);
  let watching = false;
  const onViewportChange = () => {
    if (open) position(list);
    if (msgShown()) position(msgBox);
  };
  // 一覧か一言が出ている間だけ、画面のスクロールと大きさの変化に合わせて位置を直す。
  function watch() {
    const need = open || msgShown();
    if (need === watching) return;
    watching = need;
    if (need) {
      window.addEventListener('scroll', onViewportChange, { capture: true, passive: true });
      window.addEventListener('resize', onViewportChange);
    } else {
      window.removeEventListener('scroll', onViewportChange, { capture: true });
      window.removeEventListener('resize', onViewportChange);
    }
  }

  function setMsg(text) {
    if (!msgBox) return;
    if (msgLive.textContent !== text) msgLive.textContent = text;
    if (msgBox.textContent !== text) msgBox.replaceChildren(text ? phrase(text) : '');
    msgBox.hidden = !text;
    if (text) position(msgBox);
    watch();
  }

  const noMatchText = () => (typeof opts.noMatch === 'function' ? opts.noMatch(input.value.trim()) : opts.noMatch);

  function openList() {
    // 候補が無ければ、空の一覧を開かない（noMatch か「新しい名前として足す」の行がある欄）。打った文字があれば一言を出す。
    if (!shown.length && (opts.noMatch || createLabel)) {
      closeList();
      setMsg(opts.noMatch && input.value.trim() ? noMatchText() : '');
      return;
    }
    setMsg('');
    if (open) return;
    open = true;
    list.hidden = false;
    input.setAttribute('aria-expanded', 'true');
    position(list);
    sync(true);
    watch();
  }

  function closeList() {
    if (!open) return;
    open = false;
    list.hidden = true;
    active = -1;
    input.setAttribute('aria-expanded', 'false');
    input.removeAttribute('aria-activedescendant');
    watch();
  }

  function choose(item, via, event) {
    selected = item;
    input.value = item ? item.label : '';
    closeList();
    setMsg('');
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
      } else if (opts.enterNeedsMatch && !selected && !e.ctrlKey && !e.metaKey && input.value.trim()) {
        refilter();
        if (!shown.length) {
          e.preventDefault();
          e.stopPropagation();
          openList();
        }
      }
    } else if (e.key === 'Tab') {
      if (open && active >= 0 && shown[active]) choose(shown[active], 'tab', e);
      else closeList();
    } else if (e.key === 'Escape' && (open || msgShown())) {
      // 開いた一覧（か候補が無いときの一言）を閉じるだけにする。ダイアログや画面の「やめる」には伝えない。
      e.preventDefault();
      e.stopPropagation();
      closeList();
      setMsg('');
    }
  });

  input.addEventListener('blur', () => {
    closeList();
    setMsg('');
    const text = input.value.trim();
    // 新しい名前を足せる欄: 名前がちょうど同じ候補が1つなら、それを選ぶ。違えば打った文字のまま（新しい名前にはしない）。
    if (createLabel) {
      const m = !selected && text ? sameName(items, text) : [];
      if (m.length === 1) choose(m[0], 'blur');
      return;
    }
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
    /** 選んだ候補（無ければ null）。新しい名前として足す行を選んだときは isNew が真、入っていた候補に無い名前は kept が真。 */
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
        selected = items.find((i) => i.value === selected.value) || (selected.isNew || selected.kept ? selected : null);
        if (selected) input.value = selected.label;
      }
      if (open) {
        refilter();
        openList();
      }
    },
    /** 値を入れる。notify が真なら onSelect も呼ぶ。 */
    setValue(value, { notify = false } = {}) {
      const item = items.find((i) => i.value === value) || (createLabel && value ? keptItem(value) : null);
      selected = item;
      input.value = item ? item.label : allowFree ? String(value || '') : '';
      closeList();
      setMsg('');
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
      setMsg('');
    },
  };
}
