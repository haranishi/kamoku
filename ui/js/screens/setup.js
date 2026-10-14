// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-00 初期設定 `#/setup`。4段階（事業者名と課税の区分／口座を足す／期首残高／確認して始める）。
// 各段階でサーバーに保存するので、途中で閉じても決めた分は残る（AC-01.1）。いまの段階はアドレスの ?step= に持つ。
// status.setup_done が偽の間は、ほかのどの画面に入っても、ここへ移す（main.js のルーターの見張り）。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { normalizeInvoice } from '../invoice.js';
import { field, textInput, button } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { createDateInput } from '../components/date-input.js';
import { banner } from '../components/banner.js';
import { emptyState } from '../components/empty.js';
import { walletDialog, WALLET_TYPES } from './master-forms.js';
import { createOpeningForm } from './opening-form.js';

const STEPS = ['事業者と課税の区分', '口座', '期首残高', '確認して始める'];
const MODE_LABEL = { exempt: '免税事業者', taxable: '課税事業者' };

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  const thisYear = new Date().getFullYear();
  let step = 1;
  let disposed = false;
  let current = null;
  const stepper = h('ol', { class: 'stepper', 'aria-label': '初期設定の段階' });
  const body = h('div', { class: 'setup-body' });
  // 初期設定の間はサイドバーを隠すので、広い画面でも名前が分かるように、上に印と名前を出す（狭い画面は上の帯に出る）。
  const brand = h('p', { class: 'setup-brand', 'aria-hidden': 'true' }, h('span', { class: 'brand-mark' }, icon('book', { size: 18 })), 'kamoku');
  root.append(brand, h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '初期設定'), h('p', { class: 'muted small' }, '記帳を始める前に、4つだけ決めます。決めた分はその場で保存するので、途中で閉じても続きから再開できます。'))), stepper, body);
  document.documentElement.dataset.setup = 'on';

  const go = (n) => ctx.navigate(ctx.router.buildHash('/setup', { step: String(n) }));
  const readStep = (q) => Math.min(Math.max(Number(q.step) || 1, 1), STEPS.length);

  function paintStepper() {
    stepper.replaceChildren(
      ...STEPS.map((label, i) =>
        h('li', { class: ['step', i + 1 < step && 'is-done', i + 1 === step && 'is-current'], 'aria-current': i + 1 === step ? 'step' : null }, h('span', { class: 'step-no' }, i + 1 < step ? icon('check', { size: 16 }) : String(i + 1)), h('span', { class: 'step-label' }, label)),
      ),
    );
  }

  function card(title, ...children) {
    return h('section', { class: 'card setup-card', 'aria-labelledby': 'setup-title' }, h('h2', { id: 'setup-title', class: 'card-title', tabindex: '-1' }, `${step}. ${title}`), children);
  }

  function actions(back, next) {
    return h('div', { class: 'import-actions' }, back ? button({ label: '戻る', variant: 'ghost', iconName: 'chevron-left', onClick: back }) : h('span'), next);
  }

  async function saveSettings(patch) {
    const s = await api.put('/settings', { ...current.settings, ...patch });
    current.settings = s;
    state.put('settings', s);
    return s;
  }

  function step1() {
    const s = current.settings;
    const nameIn = textInput({ value: s.business_name, maxlength: 80, placeholder: '例: サンプル事務所（屋号が無ければ、お名前）' });
    const mode = createSegmented({ legend: '消費税の課税の区分', options: [{ value: 'exempt', label: '免税事業者' }, { value: 'taxable', label: '課税事業者' }], value: s.tax_mode || 'exempt', onChange: () => paint() });
    const opened = createDateInput({ value: s.opened_on || `${thisYear}-01-01`, onError: (m) => fields.opened_on.setError(m) });
    const invIn = textInput({ value: s.invoice_number || '', maxlength: 20, placeholder: '例: T1234567890123（無ければ空）' });
    const fields = {
      business_name: field({ label: '事業者名', control: nameIn, required: true }),
      opened_on: field({ label: '開業日（記帳を始める日）', control: opened.el, required: true, hint: '年の途中から記帳を始めるときも、その年の1月1日にしておくと、帳票の数字がそろいます。' }),
      invoice_number: field({ label: '登録番号（任意）', control: invIn, hint: '適格請求書発行事業者の番号です。あとから設定で入れられます。' }),
    };
    const modeHint = h('p', { class: 'field-hint' });
    const paint = () => {
      fields.invoice_number.el.hidden = mode.value !== 'taxable';
      modeHint.textContent = mode.value === 'exempt' ? '免税事業者にすると、取引の登録で税区分の欄を隠します（「詳細」で出せます）。分からなければ、このまま進めて、あとで設定で変えられます。' : '課税事業者にすると、取引に税区分を付け、ホームと帳票に消費税の見込み（参考）を出します。';
    };
    paint();
    const next = button({ label: '保存して次へ', variant: 'primary', iconName: 'chevron-right', onClick: async () => {
      const name = nameIn.value.trim();
      fields.business_name.setError(name ? '' : '事業者名を入れてください。');
      if (!name) return nameIn.focus();
      if (!opened.commit()) return opened.input.focus();
      let invoice = null;
      if (mode.value === 'taxable') {
        const r = normalizeInvoice(invIn.value);
        fields.invoice_number.setError(r.ok ? '' : r.error);
        if (!r.ok) return invIn.focus();
        invoice = r.value;
      }
      next.disabled = true;
      try {
        await saveSettings({ business_name: name, tax_mode: mode.value, opened_on: opened.value, invoice_number: invoice, setup_done: false });
        go(2);
      } catch (err) {
        const f = err.field && fields[err.field];
        if (f) f.setError(err.message);
        else toast.error(err.message);
      } finally {
        next.disabled = false;
      }
      return undefined;
    } });
    return card(STEPS[0], fields.business_name.el, mode.el, modeHint, fields.opened_on.el, fields.invoice_number.el, actions(null, next));
  }

  async function step2() {
    let wallets = current.accounts.filter((a) => a.wallet && a.enabled);
    // 現金の口座は、最初から1つ用意する（SC-00）。口座が1つも無いときだけ作るので、読み込み直しても増えない。
    if (!wallets.length) {
      const cash = await api.post('/accounts', { name: '現金', reading: 'げんきん', kind: 'asset', parent: 'cash', wallet: { type: 'cash', institution: '' }, default_tax: null, default_pct: 100 });
      current.accounts = [...current.accounts, cash];
      wallets = [cash];
      state.invalidate('accounts');
    }
    const list = h('ul', { class: 'setup-wallets' }, wallets.map((w) => h('li', { class: 'setup-wallet' }, icon(w.wallet.type === 'card' ? 'card' : w.wallet.type === 'bank' ? 'building' : 'wallet'), h('span', { class: 'setup-wallet-name' }, w.name), h('span', { class: 'badge badge--muted' }, (WALLET_TYPES.find((t) => t.value === w.wallet.type) || { label: '' }).label))));
    const add = button({ label: '口座を足す', iconName: 'plus', onClick: async () => {
      const saved = await walletDialog({ api });
      if (saved) {
        toast.success(`口座「${saved.name}」を足しました。`);
        state.invalidate('accounts');
        render();
      }
    } });
    const next = button({ label: '次へ', variant: 'primary', iconName: 'chevron-right', onClick: () => go(3) });
    return card(STEPS[1], h('p', null, '取引に使うお金の置き場所です。現金の口座は用意してあります。銀行の口座やクレジットカードを使うなら、足してください。'), list, h('div', null, add), h('p', { class: 'small muted' }, '型は「現金」「預金」「クレジットカード」「電子マネー」から選び、名前は自由に付けられます。あとから口座の画面で足せます。'), actions(() => go(1), next));
  }

  function step3() {
    const form = createOpeningForm({ accounts: current.accounts, opening: current.opening, defaultDate: current.settings.opened_on || `${thisYear}-01-01` });
    const next = button({ label: '保存して次へ', variant: 'primary', iconName: 'chevron-right', onClick: async () => {
      const input = form.collect();
      if (!input) return;
      next.disabled = true;
      try {
        current.opening = await api.put('/opening', input);
        go(4);
      } catch (err) {
        // 別のプロセスが台帳に書いた（409 stale）。入力は消さずに、押し直してもらう。
        if (err.isStale) toast.info('他の場所で更新されました。読み込み直しました。入力はそのままです。もう一度「保存して次へ」を押してください。');
        else toast.error(err.message);
      } finally {
        next.disabled = false;
      }
    } });
    return card(STEPS[2], h('p', null, '開業日の時点で、それぞれの口座にいくらあったかを入れます。カードは、まだ払っていない額です。'), form.el, actions(() => go(2), next));
  }

  function step4() {
    const s = current.settings;
    const wallets = current.accounts.filter((a) => a.wallet && a.enabled);
    const o = current.opening;
    const start = button({ label: '記帳を始める', variant: 'primary', iconName: 'check', onClick: async () => {
      start.disabled = true;
      try {
        await saveSettings({ setup_done: true });
        await state.load('status', { force: true });
        toast.success('初期設定が済みました。取引を登録するか、明細を取り込んでください。');
        ctx.navigate('#/');
      } catch (err) {
        toast.error(err.message);
        start.disabled = false;
      }
    } });
    const row = (k, v) => h('div', null, h('dt', null, k), h('dd', null, v));
    return card(
      STEPS[3],
      h('dl', { class: 'setup-summary' }, row('事業者名', s.business_name), row('課税の区分', MODE_LABEL[s.tax_mode] || s.tax_mode), row('開業日', s.opened_on), row('口座', wallets.map((w) => w.name).join('、') || 'なし'), row('期首残高', o && o.entry_id ? `元入金 ${formatYen(o.capital)}円（${o.balances.length}つの科目）` : 'なし（すべて0円）')),
      h('p', { class: 'small muted' }, 'どれも、あとから設定と口座の画面で直せます。'),
      actions(() => go(3), start),
    );
  }

  async function render() {
    paintStepper();
    try {
      const [settings, accounts, opening] = await Promise.all([state.load('settings', { force: true }), state.load('accounts', { force: true }), api.get('/opening')]);
      if (disposed) return;
      current = { settings, accounts, opening };
      const node = step === 1 ? step1() : step === 2 ? await step2() : step === 3 ? step3() : step4();
      if (disposed) return;
      replace(body, node);
      body.querySelector('h2').focus();
    } catch (err) {
      if (!disposed) replace(body, banner({ kind: 'danger', title: '初期設定の画面を準備できませんでした', body: err.message }));
    }
  }

  const st = state.peek('status');
  if (st && st.setup_done) {
    delete document.documentElement.dataset.setup;
    stepper.remove();
    brand.remove();
    body.append(h('div', { class: 'card' }, emptyState({ iconName: 'check-circle', title: '初期設定は済んでいます', body: '事業者・締め・期首残高は設定の画面で、口座は口座の画面で見直せます。', actions: [{ label: '設定を開く', href: '#/settings', primary: true }, { label: 'ホームへ', href: '#/' }] })));
    return () => {};
  }
  step = readStep(ctx.query);
  render();
  const cleanup = () => {
    disposed = true;
    delete document.documentElement.dataset.setup;
  };
  cleanup.update = (next) => {
    step = readStep(next.query);
    render();
    return true;
  };
  return cleanup;
}
