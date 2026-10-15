// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { field, shortcutKeys } from '../js/components/fields.js';

// 偽の DOM。要素の出し入れ（add / remove）と、文字の節の書き換え（data）を、起きた順に log に残す。
function fakeDom(t) {
  const log = [];
  class Node {
    parent = null;
    remove() {
      if (!this.parent) return;
      this.parent.children = this.parent.children.filter((c) => c !== this);
      log.push(['remove', this]);
      this.parent = null;
    }
  }
  class Text extends Node {
    #data;
    constructor(data) {
      super();
      this.#data = String(data);
    }
    get data() {
      return this.#data;
    }
    set data(v) {
      this.#data = String(v);
      log.push(['data', this.#data]);
    }
    get textContent() {
      return this.#data;
    }
  }
  class Element extends Node {
    children = [];
    attrs = new Map();
    dataset = {};
    listeners = {};
    constructor(tag) {
      super();
      this.tagName = tag.toUpperCase();
    }
    setAttribute(k, v) {
      this.attrs.set(k, String(v));
    }
    getAttribute(k) {
      return this.attrs.has(k) ? this.attrs.get(k) : null;
    }
    removeAttribute(k) {
      this.attrs.delete(k);
    }
    get id() {
      return this.getAttribute('id') || '';
    }
    set id(v) {
      this.setAttribute('id', v);
    }
    addEventListener(type, fn) {
      (this.listeners[type] ||= []).push(fn);
    }
    append(...nodes) {
      for (const n of nodes) {
        const node = typeof n === 'string' ? new Text(n) : n;
        node.remove();
        node.parent = this;
        this.children.push(node);
        log.push(['add', node]);
      }
    }
    replaceChildren(...nodes) {
      for (const c of [...this.children]) c.remove();
      this.append(...nodes);
    }
    get textContent() {
      return this.children.map((c) => c.textContent).join('');
    }
  }
  const before = Object.getOwnPropertyDescriptor(globalThis, 'document');
  Object.defineProperty(globalThis, 'document', {
    configurable: true,
    writable: true,
    value: { createElement: (tag) => new Element(tag), createElementNS: (_ns, tag) => new Element(tag), createTextNode: (s) => new Text(s) },
  });
  t.after(() => {
    if (before) Object.defineProperty(globalThis, 'document', before);
    else delete globalThis.document;
  });
  const errorOf = (f) => f.el.children.find((c) => c.getAttribute && c.getAttribute('class') === 'field-error');
  return { log, Element, errorOf };
}

test('AC-10.6 欄の誤りの入れ物は、空のうちから読み上げの領域（aria-live="polite"）。文を入れると欄に結び、消すと外す', (t) => {
  const dom = fakeDom(t);
  const input = document.createElement('input');
  const f = field({ label: '金額', control: input });
  const err = dom.errorOf(f);
  assert.equal(err.getAttribute('aria-live'), 'polite', '空のうちから読み上げの領域にしておく');
  assert.equal(err.textContent, '');
  f.setError('使えない文字「a」があります。');
  assert.equal(err.textContent, '使えない文字「a」があります。');
  assert.equal(err.getAttribute('aria-live'), 'polite');
  assert.equal(input.getAttribute('aria-invalid'), 'true');
  assert.ok(input.getAttribute('aria-describedby').split(' ').includes(err.id));
  f.setError('');
  assert.equal(err.textContent, '');
  assert.equal(input.getAttribute('aria-invalid'), null);
  assert.equal(input.getAttribute('aria-describedby'), null);
});

test('AC-10.6 同じ文を入れ直しても、誤りの要素を作り直さない（保存の検査が消してから同じ文を入れ直しても、読み上げ直さない）。違う文は書き換える', (t) => {
  const dom = fakeDom(t);
  const input = document.createElement('input');
  const f = field({ label: '科目', control: input });
  const err = dom.errorOf(f);
  f.setError('科目を選んでください。');
  const text = err.children.at(-1).children[0];
  // 同じ文をもう一度: 何も起きない。
  let mark = dom.log.length;
  f.setError('科目を選んでください。');
  assert.deepEqual(dom.log.slice(mark), []);
  // 消してから同じ文（entry-simple-check.js の pick）: 要素の出し入れは無く、同じ文字の節が元の文に戻る。
  mark = dom.log.length;
  f.setError('');
  assert.equal(err.textContent, '');
  f.setError('科目を選んでください。');
  assert.deepEqual(dom.log.slice(mark).map(([kind]) => kind), ['data', 'data']);
  assert.equal(err.children.at(-1).children[0], text, '文字の節は同じもの');
  assert.equal(input.getAttribute('aria-invalid'), 'true');
  // 違う文は、同じ節を書き換えて出す。
  mark = dom.log.length;
  f.setError('打った文字に合う科目がありません。候補から選んでください。');
  assert.deepEqual(dom.log.slice(mark), [['data', '打った文字に合う科目がありません。候補から選んでください。']]);
  assert.equal(err.textContent, '打った文字に合う科目がありません。候補から選んでください。');
  assert.equal(err.children.filter((c) => c instanceof dom.Element).length, 2, '印と文の2つだけ');
});

test('SC-02 打ち直したら誤りを消す欄（clearOnInput）は、誤りが出ているときだけ消す', (t) => {
  const dom = fakeDom(t);
  const input = document.createElement('input');
  const f = field({ label: '名前', control: input, clearOnInput: true });
  const err = dom.errorOf(f);
  const typed = () => f.el.listeners.input.forEach((fn) => fn());
  f.setError('名前を入れてください。');
  typed();
  assert.equal(err.textContent, '');
  assert.equal(input.getAttribute('aria-invalid'), null);
  const mark = dom.log.length;
  typed();
  assert.deepEqual(dom.log.slice(mark), [], '出ていないときは何もしない');
});

test('AC-10.2 キーの表示を aria-keyshortcuts の値にする。続けて押すキーは表せないので付けない', () => {
  assert.equal(shortcutKeys('⌘ Enter'), 'Meta+Enter');
  assert.equal(shortcutKeys('Ctrl Enter'), 'Control+Enter');
  assert.equal(shortcutKeys('Ctrl K'), 'Control+K');
  assert.equal(shortcutKeys('Esc'), 'Escape');
  assert.equal(shortcutKeys('n'), 'n');
  assert.equal(shortcutKeys('?'), '?');
  assert.equal(shortcutKeys('e'), 'e');
  // 「g のあと i」は、同時に押すキーではないので、aria-keyshortcuts では表せない。
  assert.equal(shortcutKeys('g i'), null);
  assert.equal(shortcutKeys('Ctrl'), null);
  assert.equal(shortcutKeys(''), null);
  assert.equal(shortcutKeys(undefined), null);
});
