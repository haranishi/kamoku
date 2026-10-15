// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { initToasts, toast } from '../js/components/toast.js';
import { toastCandidates, overlaps } from '../js/components/toast-place.js';

test('SC-02 SC-03 通知の置き場所: 小窓が無ければ上を先に試し、次に今までの下。狭い幅は上の帯のすぐ下、広い幅は本文の列の上の真ん中', () => {
  const narrow = toastCandidates({ vw: 390, vh: 844, narrow: true, topbarBottom: 56, column: { left: 0, right: 390 }, dialog: null });
  assert.deepEqual(narrow.map((c) => c.key), ['top', 'bottom']);
  assert.deepEqual(narrow[0], { key: 'top', top: 64, left: 16, right: 16, maxWidth: null });
  // 上の帯が無い（初期設定の間など）ときは、画面の上の端から。
  assert.equal(toastCandidates({ vw: 390, vh: 844, narrow: true, topbarBottom: 0, column: null, dialog: null })[0].top, 16);
  const wide = toastCandidates({ vw: 1440, vh: 900, narrow: false, topbarBottom: 0, column: { left: 232, right: 1440 }, dialog: null });
  assert.deepEqual(wide.map((c) => c.key), ['top', 'bottom']);
  // 左右の間（256〜1416）の真ん中に、幅440まで。見出しの横のボタンなど、押せる物に重なるなら使わない。
  assert.deepEqual(wide[0], { key: 'top', top: 16, left: 256, right: 24, maxWidth: 440, avoidControls: true });
});

test('SC-05 通知の置き場所: 小窓が開いていれば、外に余白があれば外、無ければ見出しの下、次に下のボタンの並びの上。見出しと下の並びには重ならない', () => {
  // 幅1440の、右の引き出し（仕訳の詳細）。左の背景に出す。右には余白が無い。
  const drawer = { left: 840, right: 1440, top: 0, bottom: 900, headBottom: 76, footTop: null };
  const d = toastCandidates({ vw: 1440, vh: 900, narrow: false, topbarBottom: 0, column: { left: 232, right: 1440 }, dialog: drawer });
  assert.deepEqual(d.map((c) => c.key), ['out-left', 'in-top', 'in-bottom']);
  assert.deepEqual(d[0], { key: 'out-left', top: 16, right: 624, left: null, maxWidth: 440 });
  assert.equal(d[1].top, 84, '見出しの下');
  assert.equal(d[2].bottom, 8, '下のボタンの並びが無ければ、小窓の下の端の上');
  // 真ん中の小窓（ショートカットの一覧）。左右の両方に余白があれば、右を先に。
  const centered = toastCandidates({ vw: 1440, vh: 900, narrow: false, topbarBottom: 0, column: null, dialog: { left: 440, right: 1000, top: 32, bottom: 868, headBottom: 100, footTop: 788 } });
  assert.deepEqual(centered.map((c) => c.key), ['out-right', 'out-left', 'in-top', 'in-bottom']);
  assert.equal(centered[0].maxWidth, 392, '余白から両側の間を引いた幅まで');
  assert.equal(centered[3].bottom, 900 - 788 + 8, '下のボタンの並びの上');
  // 狭い幅の小窓は、外に余白が無いので中だけ。幅は小窓の中に収める。
  const small = toastCandidates({ vw: 390, vh: 844, narrow: true, topbarBottom: 56, column: null, dialog: { left: 16, right: 374, top: 32, bottom: 812, headBottom: 100, footTop: 732 } });
  assert.deepEqual(small.map((c) => c.key), ['in-top', 'in-bottom']);
  assert.deepEqual(small[0], { key: 'in-top', top: 108, left: 32, right: 32, maxWidth: 326 });
  // 余白が狭すぎる（幅768の引き出し）なら、外には出さない。
  const tablet = toastCandidates({ vw: 768, vh: 1024, narrow: false, topbarBottom: 0, column: null, dialog: { left: 168, right: 768, top: 0, bottom: 1024, headBottom: 76, footTop: null } });
  assert.deepEqual(tablet.map((c) => c.key), ['in-top', 'in-bottom']);
});

test('通知の置き場所: 重なりは、辺が触れるだけなら重ならないとみなす', () => {
  const a = { left: 0, right: 10, top: 0, bottom: 10 };
  assert.equal(overlaps(a, { left: 10, right: 20, top: 0, bottom: 10 }), false);
  assert.equal(overlaps(a, { left: 9, right: 20, top: 9, bottom: 20 }), true);
  assert.equal(overlaps(a, null), false);
});

class Element {
  children = [];
  attrs = {};
  parent = null;
  dataset = {};
  values = new Map();
  style = { getPropertyValue: (k) => this.values.get(k) || '', setProperty: (k, v) => this.values.set(k, v) };
  classList = { contains: (c) => (this.attrs.class || '').split(' ').includes(c) };
  get parentNode() { return this.parent; }
  setAttribute(k, v) { this.attrs[k] = v; }
  getAttribute(k) { return this.attrs[k] ?? null; }
  addEventListener() {}
  contains(node) {
    return node === this || this.children.some((c) => typeof c === 'object' && c.contains(node));
  }
  append(...nodes) {
    for (const n of nodes) {
      if (typeof n === 'object') n.remove();
      this.children.push(n);
      if (typeof n === 'object') n.parent = this;
    }
  }
  remove() {
    if (this.parent) this.parent.children = this.parent.children.filter((n) => n !== this);
    this.parent = null;
  }
  /** 通知の本文（試験で、どの通知かを見分けるため）。 */
  get text() {
    return this.children.map((c) => (typeof c === 'object' ? c.text : String(c))).join('');
  }
}

/** ポップオーバーの出し入れを数える入れ物（ブラウザの showPopover / hidePopover の代わり）。 */
class PopoverElement extends Element {
  open = false;
  calls = [];
  isConnected = true;
  matches(sel) { return sel === ':popover-open' ? this.open : false; }
  showPopover() { this.open = true; this.calls.push('show'); }
  hidePopover() { this.open = false; this.calls.push('hide'); }
}

/** モーダルで開いた小窓（:modal）。 */
class DialogElement extends Element {
  matches(sel) { return sel === ':modal'; }
}

function environment(t) {
  const originals = new Map();
  const put = (key, value) => {
    originals.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
    Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
  };
  const events = new Map();
  const target = (prefix) => ({
    addEventListener: (name, fn) => events.set(`${prefix}:${name}`, fn),
    removeEventListener: (name) => events.delete(`${prefix}:${name}`),
  });
  const observers = [];
  class Observer {
    active = false;
    constructor(fn) { this.fn = fn; observers.push(this); }
    observe() { this.active = true; }
    disconnect() { this.active = false; }
  }
  const frames = new Map();
  let id = 0;
  const state = { bars: [], modal: false, narrow: true, dialogs: [], active: null };
  put('document', {
    ...target('document'), body: {},
    createElement: () => new Element(), createElementNS: () => new Element(),
    querySelectorAll: (sel) => (sel === 'dialog[open]' ? state.dialogs : state.bars),
    querySelector: () => state.modal ? {} : null,
    get activeElement() { return state.active; },
  });
  put('window', { ...target('window'), innerHeight: 844, matchMedia: () => ({ matches: state.narrow }) });
  put('getComputedStyle', (bar) => bar.css);
  put('ResizeObserver', Observer);
  put('MutationObserver', Observer);
  put('requestAnimationFrame', (fn) => { frames.set(++id, fn); return id; });
  put('cancelAnimationFrame', (key) => frames.delete(key));
  t.after(() => {
    initToasts(new Element());
    for (const [key, desc] of originals) {
      if (desc) Object.defineProperty(globalThis, key, desc);
      else delete globalThis[key];
    }
  });
  return {
    state, events, observers, frames,
    change() {
      for (const o of observers) if (o.active) o.fn();
      const pending = [...frames.values()];
      frames.clear();
      for (const fn of pending) fn();
    },
  };
}

const bar = (top, bottom, { position = 'sticky', inset = 60, height = bottom - top } = {}) => ({
  css: { position, bottom: `${inset}px`, visibility: 'visible' },
  getBoundingClientRect: () => ({ top, bottom, height, width: 358 }),
});

test('通知は見えている下端の帯を避け、高さ・小窓・画面移動に追従し、最後に監視を解除する', (t) => {
  const env = environment(t);
  const region = new Element();
  env.state.bars = [bar(700, 784)];
  initToasts(region);
  const dismiss = toast.error('保存できませんでした');
  assert.equal(region.style.getPropertyValue('--toast-lift'), '84px');
  env.state.bars = [bar(650, 784)];
  env.change();
  assert.equal(region.style.getPropertyValue('--toast-lift'), '134px');
  env.state.modal = true;
  env.change();
  assert.equal(region.style.getPropertyValue('--toast-lift'), '0px');
  env.state.modal = false;
  env.change();
  assert.equal(region.style.getPropertyValue('--toast-lift'), '134px');
  env.state.bars = [];
  env.change();
  assert.equal(region.style.getPropertyValue('--toast-lift'), '0px');
  env.state.narrow = false;
  env.state.bars = [bar(750, 844, { inset: 0 })];
  env.change();
  assert.equal(region.style.getPropertyValue('--toast-lift'), '94px');
  dismiss();
  assert.equal(region.style.getPropertyValue('--toast-lift'), '0px');
  assert.equal(env.events.size, 0);
  assert.equal(env.frames.size, 0);
  assert.ok(env.observers.every((o) => !o.active));
});

test('通常配置・非表示・下端から離れた帯は無視し、複数なら一番上の帯を避ける', (t) => {
  const env = environment(t);
  const region = new Element();
  env.state.bars = [bar(700, 784, { position: 'static' }), bar(700, 784, { height: 0 }), bar(300, 384)];
  initToasts(region);
  const dismiss = toast.error('試験');
  assert.equal(region.style.getPropertyValue('--toast-lift'), '0px');
  env.state.bars = [bar(720, 784), bar(660, 784)];
  env.change();
  assert.equal(region.style.getPropertyValue('--toast-lift'), '124px');
  dismiss();
});

const texts = (region) => region.children.map((c) => c.text);
const has = (region, word) => texts(region).some((s) => s.includes(word));

test('SC-03 通知の長さ（未確認の「元に戻す」など）: 成功は4秒、操作つきは ms が無くても10秒、ms を渡せばその長さ。エラーは閉じるまで残る', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  environment(t);
  const region = new Element();
  initToasts(region);
  toast.success('保存しました。');
  toast.success('No.1 を登録しました。', { action: { label: '仕訳を見る', href: '#/journal/e1' } });
  toast.info('取り込まない行にしました。', { action: { label: '元に戻す', onClick() {} }, ms: 6000 });
  toast.error('保存できませんでした。');
  assert.equal(region.children.length, 4);
  t.mock.timers.tick(3999);
  assert.ok(has(region, '保存しました。'));
  t.mock.timers.tick(1);
  assert.ok(!has(region, '保存しました。'), '普通の成功は4秒で消える');
  t.mock.timers.tick(2000);
  assert.ok(!has(region, '取り込まない行'), 'ms を渡したら、その長さで消える');
  assert.ok(has(region, 'No.1 を登録しました'), '操作つきは、6秒ではまだ残る');
  t.mock.timers.tick(4000);
  assert.ok(!has(region, 'No.1 を登録しました'), '操作つきは10秒で消える');
  t.mock.timers.tick(600000);
  assert.deepEqual(texts(region), ['保存できませんでした。']);
});

test('SC-03 フォーカスが乗っている間は、操作つきの知らせを消さない', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const env = environment(t);
  const region = new Element();
  initToasts(region);
  toast.success('No.2 を登録しました。', { action: { label: '仕訳を見る', href: '#/journal/e2' } });
  const item = region.children[0];
  env.state.active = item.children[1];
  t.mock.timers.tick(30000);
  assert.equal(region.children.length, 1);
  env.state.active = null;
  t.mock.timers.tick(10000);
  assert.equal(region.children.length, 0);
});

test('AC-11.3 同じ種類の短い知らせ（表示の切り替えなど）は前の知らせを置き換え、ほかの知らせは別々に出す', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  environment(t);
  const region = new Element();
  initToasts(region);
  toast.info('表示を「ライト」にしました。');
  toast.info('表示を「ダーク」にしました。');
  toast.info('表示を「OSに合わせる」にしました。');
  assert.deepEqual(texts(region), ['表示を「OSに合わせる」にしました。']);
  // 文が違う知らせ、種類（成功と案内）が違う知らせ、操作つきの知らせは、置き換えない。
  toast.info('文字コードを変えて、読み直しました。');
  toast.success('表示を「ダーク」にしました。');
  assert.equal(region.children.length, 3);
  toast.success('No.3 を登録しました。', { action: { label: '仕訳を見る', href: '#/journal/e3' } });
  toast.success('No.4 を登録しました。', { action: { label: '仕訳を見る', href: '#/journal/e4' } });
  assert.ok(has(region, 'No.3') && has(region, 'No.4'));
  // 置き換えた知らせの時計は止まっている（あとで消えるときに、新しい知らせを巻き込まない）。
  t.mock.timers.tick(4000);
  assert.ok(texts(region).every((s) => s.startsWith('No.')), texts(region).join(' / '));
});

test('SC-00 同じ文のエラーは重ねず（初期設定の保存の失敗など）、出ている通知を末尾へ出し直す。返す関数で、その通知を閉じられる', (t) => {
  environment(t);
  const region = new Element();
  initToasts(region);
  const msg = '期首残高の日付は、最初の会計年度（2026年）の中にしてください。';
  const first = toast.error(msg);
  const again = toast.error(msg);
  assert.equal(region.children.length, 1);
  assert.equal(again, first);
  toast.error('通信できませんでした。');
  toast.error(msg);
  assert.deepEqual(texts(region), ['通信できませんでした。', msg]);
  // くわしい文が違えば、別の通知。
  toast.error(msg, { detail: '開業日は 2026-01-01 です。' });
  assert.equal(region.children.length, 3);
  first();
  assert.ok(!texts(region).includes(msg));
  assert.equal(region.children.length, 2);
});

test('SC-05 入れ物はポップオーバーで（仕訳の詳細の引き出しの上にも出す）、出すたびに最前面へ出し直し、開いた小窓の中へ移って押せるようにする', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const env = environment(t);
  const body = new Element();
  const region = new PopoverElement();
  body.append(region);
  initToasts(region);
  assert.equal(region.open, false, '通知が無いうちは、最前面に出さない');
  const saved = toast.success('No.5 を登録しました。', { action: { label: '仕訳を見る', href: '#/journal/e5' } });
  assert.equal(region.open, true);
  assert.equal(region.parentNode, body);
  // あとから仕訳の詳細（引き出し）が開いた。小窓の外は押せなくなるので、入れ物を小窓の中へ移して出し直す。
  const drawer = new DialogElement();
  body.append(drawer);
  env.state.dialogs = [drawer];
  env.state.modal = true;
  env.change();
  assert.equal(region.parentNode, drawer);
  assert.deepEqual(region.calls.slice(-2), ['hide', 'show']);
  // 小窓が開いている間に出した通知も、最前面へ出し直す（あとから開いた小窓より上に来るように）。
  region.calls.length = 0;
  const voided = toast.info('No.5 を取り消しました。');
  assert.deepEqual(region.calls, ['hide', 'show']);
  // 確認の小窓を重ねたら、そちらへ。閉じたら前の小窓へ、全部閉じたら元の場所へ戻る。
  const confirm = new DialogElement();
  body.append(confirm);
  env.state.dialogs = [drawer, confirm];
  env.change();
  assert.equal(region.parentNode, confirm);
  env.state.dialogs = [drawer];
  env.change();
  assert.equal(region.parentNode, drawer);
  env.state.dialogs = [];
  env.state.modal = false;
  env.change();
  assert.equal(region.parentNode, body);
  // 通知のボタンにフォーカスがあるときは、出し直さない（閉じるとフォーカスが動くことがある）。
  region.calls.length = 0;
  env.state.active = region.children[0];
  const reread = toast.info('文字コードを変えて、読み直しました。');
  assert.deepEqual(region.calls, []);
  env.state.active = null;
  // 通知が無くなったら、最前面から下ろし、元の場所に置く（小窓と一緒に文書から外れないように）。
  env.state.dialogs = [drawer];
  env.state.modal = true;
  env.change();
  assert.equal(region.parentNode, drawer);
  for (const close of [saved, voided, reread]) close();
  assert.equal(region.children.length, 0);
  assert.equal(region.open, false);
  assert.equal(region.parentNode, body);
  assert.ok(env.observers.every((o) => !o.active), '通知が無くなったら、監視を解く');
});
