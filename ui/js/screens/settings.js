// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-10 設定 `#/settings`。事業者・締め・期首残高・データ・見た目。?section= で、その節へ移る（コマンド一覧と仕訳の詳細から）。
// 事業者と締めは PUT /settings（全項目を置き換え）。見た目は、ブラウザの中にだけ保存する。

import { h, replace, uid, addDescribedBy } from '../dom.js';
import { formatYen } from '../format.js';
import { addDays } from '../input-parse.js';
import { normalizeInvoice } from '../invoice.js';
import { getTheme, setTheme, getDensity, setDensity, onAppearanceChange } from '../theme.js';
import { field, textInput, selectInput, button } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { createDateInput } from '../components/date-input.js';
import { banner } from '../components/banner.js';
import { confirmDialog } from '../components/dialog.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { createOpeningForm } from './opening-form.js';
import { dataSection } from './settings-data.js';
import { phrase } from '../components/phrase.js';

/** 免税か課税かの判断の目安（数字は年で変わるので書かない）。初期設定と同じ文。 */
export const TAX_MODE_HINT = '免税事業者かどうかは、前々年の課税売上などで決まります。分からなければ、税務署か税理士に確かめてください。';

/**
 * 開業日の説明（初期設定と設定で同じ文。1文ずつ行を分ける）。開業日は、これより前の日付で登録するときの確認（取引の登録）と、
 * 期首残高の日付の初期値（期首残高の入力・GET /opening）に使う。途中から記帳を始めるときの勧めは、別の行に分ける。
 */
export const OPENED_HINT = ['この日より前の日付で取引を登録するときは、確認が出ます。期首残高の日付の初期値にもなります。', '年の途中から記帳を始めるときは、その年の1月1日にすると、年の帳票の「期首」の列に、期首残高が入ります。'];

const SECTIONS = [
  ['business', '事業者'],
  ['lock', '締め'],
  ['opening', '期首残高'],
  ['data', 'データ'],
  ['appearance', '見た目'],
];

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  let settings = null;
  let disposed = false;
  let opening = null;
  // この画面で出した失敗の通知。保存に成功したら閉じる（直したあとも、古い失敗が残っていた）。
  const failures = [];
  const fail = (message) => failures.push(toast.error(message));
  const cleared = () => failures.splice(0).forEach((close) => close());
  const body = h('div', { class: 'settings' });
  // 目次は、画面の上に付いてくるタブの並び。今いる節に aria-current を付ける（押せる大きさで、どこにいるかが分かるように）。
  const tabs = SECTIONS.map(([id, label]) => h('button', { type: 'button', class: 'settings-tab', dataset: { section: id }, onclick: () => jump(id) }, label));
  const markCurrent = (id) => tabs.forEach((t) => (t.dataset.section === id ? t.setAttribute('aria-current', 'true') : t.removeAttribute('aria-current')));
  const jump = (id) => {
    const sec = body.querySelector(`#set-${id}`);
    if (!sec) return;
    sec.scrollIntoView({ block: 'start' });
    sec.querySelector('h2').focus({ preventScroll: true });
    markCurrent(id);
  };
  const nav = h('nav', { class: 'settings-nav', 'aria-label': '設定の節' }, tabs);
  root.append(h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '設定'))), nav, body);
  markCurrent(SECTIONS[0][0]);

  // スクロールに合わせて、今いる節（目次の下の端より上に見出しが来た、最後の節）を示す。いちばん下まで来たら最後の節。
  let ticking = false;
  const onScroll = () => {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(() => {
      ticking = false;
      const line = nav.getBoundingClientRect().bottom + 24;
      const secs = [...body.querySelectorAll('.settings-section')];
      if (!secs.length) return;
      const atEnd = window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2;
      const passed = secs.filter((s) => s.getBoundingClientRect().top <= line);
      const cur = atEnd ? secs[secs.length - 1] : passed[passed.length - 1] || secs[0];
      markCurrent(cur.id.replace(/^set-/, ''));
    });
  };
  window.addEventListener('scroll', onScroll, { passive: true });

  const ro = () => state.isReadOnly();
  /** 節の枠。side は見出しの横に出すもの（締めの状態など）。lead は文の配列なら、1文ずつ行を分ける。 */
  function section(id, title, lead, side, ...children) {
    const heading = h('h2', { id: `set-${id}-title`, class: 'card-title', tabindex: '-1' }, title);
    const leadText = Array.isArray(lead) ? lead.map((t) => h('span', { class: 'hint-line' }, phrase(t))) : lead ? phrase(lead) : null;
    return h('section', { class: 'card settings-section', id: `set-${id}`, 'aria-labelledby': `set-${id}-title` }, side ? h('div', { class: 'settings-head' }, heading, side) : heading, leadText ? h('p', { class: 'small muted settings-lead' }, leadText) : null, children);
  }

  /**
   * 広い幅（1280px以上）で、節の中を左右2列に並べる入れ物（screens-settings.css の .settings-cols）。狭い幅では今までどおり縦に並ぶ。
   * カードは全幅なのに入力が左の約560pxに集まり、右が空いて、ページも3,000px超と長かった（見た目の採点 第2回 幅1440）。
   */
  const cols = (left, right) => h('div', { class: 'settings-cols' }, h('div', { class: 'settings-col' }, left), h('div', { class: 'settings-col' }, right));

  /** 設定を保存する。fields は、サーバーが field で返した誤りを出す欄。done は、保存できたときの知らせ。保存できたら真。 */
  async function saveSettings(patch, fields, btn, done = '設定を保存しました。') {
    btn.disabled = true;
    Object.values(fields).forEach((f) => f.setError(''));
    try {
      settings = await api.put('/settings', { ...settings, ...patch });
      state.put('settings', settings);
      cleared();
      toast.success(done);
      return true;
    } catch (err) {
      const f = err.field && fields[err.field];
      if (f) f.setError(err.message);
      else fail(err.message);
      return false;
    } finally {
      btn.disabled = ro();
    }
  }

  function businessSection() {
    const nameIn = textInput({ value: settings.business_name, maxlength: 80, placeholder: '例: サンプル事務所' });
    const opened = createDateInput({ value: settings.opened_on || null, onError: (m) => fields.opened_on.setError(m) });
    const mode = createSegmented({ legend: '消費税の課税の区分', options: [{ value: 'exempt', label: '免税事業者' }, { value: 'taxable', label: '課税事業者' }], value: settings.tax_mode, onChange: () => paintMode() });
    const invIn = textInput({ value: settings.invoice_number || '', maxlength: 20, placeholder: '例: T1234567890123' });
    const rounding = selectInput({ value: settings.rounding }, [
      { value: 'floor', label: '切捨て' },
      { value: 'round', label: '四捨五入' },
      { value: 'ceil', label: '切上げ' },
    ]);
    const fields = {
      // 狭い幅ではメニューを開いた中に出るので、幅に依存しない案内にする。初期設定と同じく必須（空で保存でき、メニューから名前が消えた）。
      business_name: field({ label: '事業者名', control: nameIn, required: true, hint: phrase('メニューの上に出ます。'), clearOnInput: true }),
      opened_on: field({ label: '開業日', control: opened.el, required: true, clearOnInput: true, hint: OPENED_HINT.map((t) => h('span', { class: 'hint-line' }, phrase(t))) }),
      invoice_number: field({ label: '登録番号（適格請求書発行事業者）', control: invIn, hint: phrase('課税事業者のときだけ入れられます。形式（T と13桁の数字）だけを確かめます。'), clearOnInput: true }),
      rounding: field({ label: '消費税の端数処理', control: rounding, hint: phrase('記帳の目安の税額を出すときの、1円未満の扱いです。') }),
    };
    // 免税か課税かの目安は、選択の組に結び付ける（組に入ったときに、legend と一緒に読まれる）。
    const modeHint = h('p', { class: 'field-hint', id: uid('mode-hint') }, phrase(TAX_MODE_HINT));
    addDescribedBy(mode.el, modeHint.id);
    const paintMode = () => {
      fields.invoice_number.el.hidden = mode.value !== 'taxable';
    };
    paintMode();
    const save = button({ label: '事業者の設定を保存', variant: 'primary', onClick: () => submit() });
    save.disabled = ro();
    function submit() {
      const name = nameIn.value.trim();
      fields.business_name.setError(name ? '' : '事業者名を入れてください。');
      if (!name) return nameIn.focus();
      if (!opened.commit()) return;
      let invoice = null;
      if (mode.value === 'taxable') {
        const r = normalizeInvoice(invIn.value);
        fields.invoice_number.setError(r.ok ? '' : r.error);
        if (!r.ok) return invIn.focus();
        invoice = r.value;
      }
      return saveSettings({ business_name: name, opened_on: opened.value, tax_mode: mode.value, invoice_number: invoice, rounding: rounding.value }, fields, save);
    }
    return section(
      'business',
      '事業者',
      '課税の区分を「免税事業者」にすると、取引の登録と未確認で税区分の欄を隠します。',
      null,
      cols(h('div', { class: 'settings-grid' }, fields.business_name.el, fields.opened_on.el), [mode.el, modeHint, fields.invoice_number.el, fields.rounding.el]),
      h('div', { class: 'settings-actions' }, save),
    );
  }

  /** 締めの状態の文（見出しの横に、いつも出す）。 */
  const lockState = (until) => (until ? `${until} まで締め済み` : '締めていません');

  /** 日付の塊（「2026-03-31 」）。途中で折らず、後ろの空白も見せる（phrase に通すと、塊の末尾の空白が見えなくなる）。 */
  const day = (iso) => [h('span', { class: 'nw' }, `${iso}\u00a0`), h('wbr')];

  /** 締める・外す前に、何ができなくなる（できるようになる）かを確かめる。承認されたら真。 */
  function confirmLock(before, next) {
    const id = uid('lock-msg');
    if (!next) {
      return confirmDialog({ title: '締めを外しますか', describedBy: id, message: h('p', { id }, phrase('締めを外すと、'), day(before), phrase('以前の仕訳を、また直せるようになります。')), confirmLabel: '外す' });
    }
    // 締めた日を前へ戻すときは、外れる期間も書く（その間の仕訳は、また直せるようになる）。
    const from = before && next < before ? addDays(next, 1) : null;
    const reopened = from ? [from < before ? [h('span', { class: 'nw' }, `${from}〜`), h('wbr')] : null, h('span', { class: 'nw' }, `${before}\u00a0の仕訳は、`), h('wbr'), phrase('また直せるようになります。')] : null;
    return confirmDialog({ title: `${next} まで締めますか`, describedBy: id, message: h('p', { id }, day(next), phrase('以前の、登録・訂正・取消ができなくなります。'), reopened), confirmLabel: '締める' });
  }

  function lockSection() {
    // 空は「締めない」なので誤りにしない（日付の欄は、空で離れると「日付を入れてください」を出す。それが出ると、
    // 外そうとして空にし、保存を押した瞬間にボタンが下へずれて、押したことにならなかった）。
    const lock = createDateInput({ value: settings.locked_until || null, onError: (m) => f.setError(lock.input.value.trim() === '' ? '' : m) });
    // 例は、ほかの画面と同じ日付の形で（「1231」は年が無く、日付として読めなかった）。去年の年末を例にする。
    lock.input.placeholder = `例: ${new Date().getFullYear() - 1}-12-31（締めないなら空）`;
    const f = field({ label: 'この日以前は変更しない', control: lock.el, hint: phrase('決算や申告が済んだ期間を締めると、その期間の登録・訂正・取消を断ります。空にすると締めません。'), clearOnInput: true });
    const stateEl = h('span', { class: 'badge badge--muted lock-state' }, lockState(settings.locked_until));
    const save = button({ label: '締めた日を保存', variant: 'primary', onClick: async () => {
      const empty = lock.input.value.trim() === '';
      if (!empty && !lock.commit()) return;
      const before = settings.locked_until || null;
      const next = empty ? null : lock.value;
      if (next === before) {
        toast.info(`締めた日は変わっていません（${lockState(before)}）。`);
        return;
      }
      if (!(await confirmLock(before, next))) return;
      if (await saveSettings({ locked_until: next }, { locked_until: f }, save, next ? `${next} まで締めました。` : '締めを外しました。')) {
        stateEl.textContent = lockState(settings.locked_until);
      }
    } });
    save.disabled = ro();
    return section('lock', '締め', null, stateEl, f.el, h('div', { class: 'settings-actions' }, save));
  }

  function openingSection() {
    const accounts = state.peek('accounts') || [];
    const form = createOpeningForm({ accounts, opening, defaultDate: settings.opened_on || `${new Date().getFullYear()}-01-01` });
    const save = button({ label: '期首残高を保存', variant: 'primary', onClick: async () => {
      const input = form.collect();
      if (!input) return;
      save.disabled = true;
      try {
        opening = await api.put('/opening', input);
        cleared();
        toast.success(opening.entry_id ? `期首残高を保存しました（元入金 ${formatYen(opening.capital)}円）。` : '期首残高をすべて0にしました（仕訳はありません）。');
        state.load('status', { force: true }).catch(() => {});
      } catch (err) {
        // 別のプロセスが台帳に書いた（409 stale）。入力は消さずに、最新の期首残高を読み直してから押し直してもらう。
        if (err.isStale) {
          toast.info('他の場所で更新されました。読み込み直しました。入力はそのままです。もう一度「期首残高を保存」を押してください。');
          opening = await api.get('/opening').catch(() => opening);
        } else if (!form.showError(err)) fail(err.message);
      } finally {
        save.disabled = ro();
      }
    } });
    save.disabled = ro();
    return section('opening', '期首残高', ['開業日（または記帳を始めた日）の、口座などの残高です。', '見直すと、期首残高の仕訳が「期首残高の見直し」という理由で訂正されます。'], null, form.el, h('div', { class: 'settings-actions' }, save));
  }

  let stopAppearance = () => {};
  function appearanceSection() {
    const theme = createSegmented({ legend: '明るさ', options: [{ value: 'light', label: 'ライト' }, { value: 'dark', label: 'ダーク' }, { value: 'os', label: 'OSに合わせる' }], value: getTheme(), className: 'seg--grid', onChange: (v) => setTheme(v) });
    const density = createSegmented({ legend: '表の行の密度', options: [{ value: 'standard', label: '標準' }, { value: 'compact', label: '狭い' }], value: getDensity(), onChange: (v) => setDensity(v) });
    // 左下の「表示」ボタンやコマンドで変えたときも、ここの選択を合わせる（OSに合わせる、に回しても「ダーク」のまま残った。
    // 体験の評価 第3回 E3）。画面を離れるときに外す（cleanup）。
    stopAppearance();
    stopAppearance = onAppearanceChange(() => {
      theme.setValue(getTheme());
      density.setValue(getDensity());
    });
    // 保存のボタンは無い（選んだ時点で変わる）。ほかの節と保存のしかたが違うので、そのことを書く。
    return section('appearance', '見た目', '選ぶと、すぐに変わります（保存のボタンはありません）。このブラウザの中にだけ覚え、帳簿には入りません。', null, cols(theme.el, density.el));
  }

  async function load() {
    const stop = delayedSkeleton(body, () => skeletonBlock({ lines: 6, label: '設定を読み込んでいます' }));
    try {
      const [s, , o] = await Promise.all([state.load('settings', { force: true }), state.load('accounts', { force: true }), api.get('/opening'), state.load('status', { force: true })]);
      stop();
      if (disposed) return;
      settings = s;
      opening = o;
      const data = dataSection(ctx);
      replace(
        body,
        ro() ? banner({ kind: 'danger', title: '読み取り専用のため、保存はできません', body: state.readOnlyReason() }) : null,
        businessSection(),
        lockSection(),
        openingSection(),
        section('data', 'データ', null, null, data.el),
        appearanceSection(),
      );
      unsub = state.subscribe('status', (st) => data.paintHead(st));
      if (ctx.query.section) jump(ctx.query.section);
    } catch (err) {
      stop();
      if (!disposed) replace(body, banner({ kind: 'danger', title: '設定を読み込めませんでした', body: err.message }));
    }
  }

  let unsub = () => {};
  load();
  const cleanup = () => {
    disposed = true;
    window.removeEventListener('scroll', onScroll);
    unsub();
    stopAppearance();
    cleared();
  };
  cleanup.update = (next) => {
    if (next.query.section) jump(next.query.section);
    return true;
  };
  return cleanup;
}

