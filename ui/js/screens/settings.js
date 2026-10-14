// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-10 設定 `#/settings`。事業者・締め・期首残高・データ・見た目。?section= で、その節へ移る（コマンド一覧と仕訳の詳細から）。
// 事業者と締めは PUT /settings（全項目を置き換え）。見た目は、ブラウザの中にだけ保存する。

import { h, replace } from '../dom.js';
import { formatYen } from '../format.js';
import { normalizeInvoice } from '../invoice.js';
import { getTheme, setTheme, getDensity, setDensity } from '../theme.js';
import { field, textInput, selectInput, button } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { createDateInput } from '../components/date-input.js';
import { banner } from '../components/banner.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { createOpeningForm } from './opening-form.js';
import { dataSection } from './settings-data.js';

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
  const body = h('div', { class: 'settings' });
  const jump = (id) => {
    const sec = body.querySelector(`#set-${id}`);
    if (!sec) return;
    sec.scrollIntoView({ block: 'start' });
    sec.querySelector('h2').focus({ preventScroll: true });
  };
  root.append(
    h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '設定'))),
    h('nav', { class: 'settings-nav', 'aria-label': '設定の節' }, SECTIONS.map(([id, label]) => h('button', { type: 'button', class: 'btn btn--sm btn--ghost', onclick: () => jump(id) }, label))),
    body,
  );

  const ro = () => state.isReadOnly();
  function section(id, title, lead, ...children) {
    return h('section', { class: 'card settings-section', id: `set-${id}`, 'aria-labelledby': `set-${id}-title` }, h('h2', { id: `set-${id}-title`, class: 'card-title', tabindex: '-1' }, title), lead ? h('p', { class: 'small muted' }, lead) : null, children);
  }

  /** 設定を保存する。fields は、サーバーが field で返した誤りを出す欄。 */
  async function saveSettings(patch, fields, btn) {
    btn.disabled = true;
    Object.values(fields).forEach((f) => f.setError(''));
    try {
      settings = await api.put('/settings', { ...settings, ...patch });
      state.put('settings', settings);
      toast.success('設定を保存しました。');
    } catch (err) {
      const f = err.field && fields[err.field];
      if (f) f.setError(err.message);
      else toast.error(err.message);
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
      business_name: field({ label: '事業者名', control: nameIn, hint: 'サイドバーと帳票に出ます。' }),
      opened_on: field({ label: '開業日', control: opened.el, required: true }),
      invoice_number: field({ label: '登録番号（適格請求書発行事業者）', control: invIn, hint: '課税事業者のときだけ入れられます。形式（T と13桁の数字）だけを確かめます。' }),
      rounding: field({ label: '消費税の端数処理', control: rounding, hint: '記帳の目安の税額を出すときの、1円未満の扱いです。' }),
    };
    const paintMode = () => {
      fields.invoice_number.el.hidden = mode.value !== 'taxable';
    };
    paintMode();
    const save = button({ label: '事業者の設定を保存', variant: 'primary', onClick: () => submit() });
    save.disabled = ro();
    function submit() {
      if (!opened.commit()) return;
      let invoice = null;
      if (mode.value === 'taxable') {
        const r = normalizeInvoice(invIn.value);
        fields.invoice_number.setError(r.ok ? '' : r.error);
        if (!r.ok) return invIn.focus();
        invoice = r.value;
      }
      return saveSettings({ business_name: nameIn.value.trim(), opened_on: opened.value, tax_mode: mode.value, invoice_number: invoice, rounding: rounding.value }, fields, save);
    }
    return section('business', '事業者', '課税の区分を「免税事業者」にすると、取引の登録と未確認で税区分の欄を隠します。', h('div', { class: 'settings-grid' }, fields.business_name.el, fields.opened_on.el), mode.el, fields.invoice_number.el, fields.rounding.el, h('div', { class: 'settings-actions' }, save));
  }

  function lockSection() {
    const lock = createDateInput({ value: settings.locked_until || null, onError: (m) => f.setError(m) });
    lock.input.placeholder = '例: 1231（締めないなら空）';
    const f = field({ label: 'この日以前は変更しない', control: lock.el, hint: '決算や申告が済んだ期間を締めると、その期間の登録・訂正・取消を断ります。空にすると締めません。' });
    const save = button({ label: '締めた日を保存', variant: 'primary', onClick: () => {
      const empty = lock.input.value.trim() === '';
      if (!empty && !lock.commit()) return undefined;
      return saveSettings({ locked_until: empty ? null : lock.value }, { locked_until: f }, save);
    } });
    save.disabled = ro();
    return section('lock', '締め', null, f.el, h('div', { class: 'settings-actions' }, save));
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
        toast.success(opening.entry_id ? `期首残高を保存しました（元入金 ${formatYen(opening.capital)}円）。` : '期首残高をすべて0にしました（仕訳はありません）。');
        state.load('status', { force: true }).catch(() => {});
      } catch (err) {
        // 別のプロセスが台帳に書いた（409 stale）。入力は消さずに、最新の期首残高を読み直してから押し直してもらう。
        if (err.isStale) {
          toast.info('他の場所で更新されました。読み込み直しました。入力はそのままです。もう一度「期首残高を保存」を押してください。');
          opening = await api.get('/opening').catch(() => opening);
        } else toast.error(err.message);
      } finally {
        save.disabled = ro();
      }
    } });
    save.disabled = ro();
    return section('opening', '期首残高', '開業日（または記帳を始めた日）の、口座などの残高です。見直すと、期首残高の仕訳が「期首残高の見直し」という理由で訂正されます。', form.el, h('div', { class: 'settings-actions' }, save));
  }

  function appearanceSection() {
    const theme = createSegmented({ legend: '明るさ', options: [{ value: 'light', label: 'ライト' }, { value: 'dark', label: 'ダーク' }, { value: 'os', label: 'OSに合わせる' }], value: getTheme(), className: 'seg--grid', onChange: (v) => setTheme(v) });
    const density = createSegmented({ legend: '表の行の密度', options: [{ value: 'standard', label: '標準' }, { value: 'compact', label: '狭い' }], value: getDensity(), onChange: (v) => setDensity(v) });
    return section('appearance', '見た目', 'この項目は、このブラウザの中にだけ保存します。帳簿には入りません。', theme.el, density.el);
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
        section('data', 'データ', null, data.el),
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
    unsub();
  };
  cleanup.update = (next) => {
    if (next.query.section) jump(next.query.section);
    return true;
  };
  return cleanup;
}

