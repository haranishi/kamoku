// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-00 初期設定 `#/setup`。4段階（事業者名と課税の区分／口座を追加する／期首残高／確認して始める）。
// 各段階でサーバーに保存するので、途中で閉じても決めた分は残る（AC-01.1）。いまの段階はアドレスの ?step= に持つ。
// ?step= の無いアドレスで開いたとき（閉じて開き直したとき）は、保存した分から続きの段階を決める（savedUpTo）。
// status.setup_done が偽の間は、ほかのどの画面に入っても、ここへ移す（main.js のルーターの見張り）。

import { h, replace, uid, addDescribedBy, srOnly } from '../dom.js';
import { icon } from '../icons.js';
import { formatDate } from '../format.js';
import { normalizeInvoice } from '../invoice.js';
import { field, textInput, button } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { createDateInput } from '../components/date-input.js';
import { banner } from '../components/banner.js';
import { emptyState } from '../components/empty.js';
import { walletDialog, WALLET_TYPES } from './master-forms.js';
import { createOpeningForm, enterAdvances } from './opening-form.js';
import { TAX_MODE_HINT, OPENED_HINT } from './settings.js';
import { money } from '../components/money.js';
import { phrase } from '../components/phrase.js';

const STEPS = ['事業者と課税の区分', '口座', '期首残高', '確認して始める'];
const MODE_LABEL = { exempt: '免税事業者', taxable: '課税事業者' };

/**
 * サーバーに保存した分から、どの段階まで済んだかを読む（0〜3。新しい保存の仕組みは足さない）。1段階目は事業者名と開業日、
 * 3段階目は期首残高の仕訳。2段階目（口座）は保存の押しボタンが無いので、3段階目と一緒に済みとみなす。
 * 期首残高をすべて0で保存したとき（仕訳ができない）は、3段階目を済みと読めない（そのときは2段階目から続ける）。
 */
const savedUpTo = ({ settings, opening }) => (!(settings.business_name && settings.opened_on) ? 0 : opening && opening.entry_id ? 3 : 1);

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  const thisYear = new Date().getFullYear();
  let step = 1;
  let disposed = false;
  let current = null;
  // この画面で出した失敗の通知。保存に成功したら閉じる（直して次へ進んだあとも、ホームまで残っていた）。
  const failures = [];
  const fail = (message) => failures.push(toast.error(message));
  const cleared = () => failures.splice(0).forEach((close) => close());
  const stepper = h('ol', { class: 'stepper', 'aria-label': '初期設定の段階' });
  const body = h('div', { class: 'setup-body' });
  // 初期設定の間はサイドバーを隠すので、広い画面でも名前が分かるように、上に印と名前を出す（狭い画面は上の帯に出る）。
  const brand = h('p', { class: 'setup-brand', 'aria-hidden': 'true' }, h('span', { class: 'brand-mark' }, icon('book', { size: 18 })), 'kamoku');
  root.append(brand, h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '初期設定'), h('p', { class: 'muted small' }, phrase('記帳を始める前に、4つだけ決めます。決めた分はその場で保存するので、途中で閉じても続きから再開できます。')))), stepper, body);
  document.documentElement.dataset.setup = 'on';

  const go = (n) => ctx.navigate(ctx.router.buildHash('/setup', { step: String(n) }));
  /** アドレスの ?step=。無ければ null（保存した分から決める）。 */
  const readStep = (q) => (q.step ? Math.min(Math.max(Number(q.step) || 1, 1), STEPS.length) : null);
  let asked = null;

  /**
   * 段階の表示。いまの段階より前と、保存して済んだ段階には印を付ける（戻るで前の段階を開いても、済んだ印は残す）。
   * 印は目で見る手がかりなので、読み上げには「済み」を添える。
   */
  function paintStepper() {
    const saved = current ? savedUpTo(current) : 0;
    stepper.replaceChildren(
      ...STEPS.map((label, i) => {
        const n = i + 1;
        const done = n !== step && (n < step || n <= saved);
        return h(
          'li',
          { class: ['step', done && 'is-done', n === step && 'is-current'], 'aria-current': n === step ? 'step' : null },
          h('span', { class: 'step-no' }, done ? icon('check', { size: 16 }) : String(n)),
          h('span', { class: 'step-label' }, label, done ? srOnly('（済み）') : null),
        );
      }),
    );
  }

  function card(title, ...children) {
    return h('section', { class: 'card setup-card', 'aria-labelledby': 'setup-title' }, h('h2', { id: 'setup-title', class: 'card-title', tabindex: '-1' }, `${step}. ${title}`), children);
  }

  function actions(back, next) {
    return h('div', { class: 'import-actions setup-actions' }, back ? button({ label: '戻る', variant: 'ghost', iconName: 'chevron-left', onClick: back }) : h('span'), next);
  }

  /** 進むボタン。進む向きの印は文字の後ろに置く（取込の「確かめる」と同じ。戻るの印は前）。 */
  function forward(label, onClick) {
    const b = button({ label, variant: 'primary', onClick });
    b.append(icon('chevron-right', { size: 18 }));
    return b;
  }

  /** 段階の冒頭の説明。ほかの画面の説明と同じ大きさ（13pxの補足の文）。 */
  const lead = (text) => h('p', { class: 'small muted setup-lead' }, phrase(text));

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
      business_name: field({ label: '事業者名', control: nameIn, required: true, clearOnInput: true }),
      // 名前を「開業日（記帳を始める日）」とし、説明で「途中でも1月1日に」と勧めていたので、4月に開業した人がどちらを
      // 入れるか迷った。開業日が何に使われるか（コードの振る舞い）と、途中から記帳を始めるときの勧めを分けて書く。
      opened_on: field({ label: '開業日', control: opened.el, required: true, clearOnInput: true, hint: OPENED_HINT.map((t) => h('span', { class: 'hint-line' }, phrase(t))) }),
      invoice_number: field({ label: '登録番号（任意）', control: invIn, clearOnInput: true, hint: phrase('適格請求書発行事業者の番号です。あとから設定で入れられます。') }),
    };
    // 選んだ区分で何が変わるか（1文）と、どちらを選ぶかの目安。目安は「選び方をくわしく」に畳む（説明が続いて、次の必須の欄まで
    // 遠かった。見た目の採点 第2回 幅390）。どちらも選択の組に結び付ける（組に入ったときに読まれる。畳んでいても読まれる）。
    const modeHint = h('p', { class: 'field-hint', id: uid('mode-hint') });
    const modeGuide = h('p', { class: 'field-hint', id: uid('mode-guide') }, phrase(TAX_MODE_HINT), phrase('このまま進めて、あとで設定で変えることもできます。'));
    const guideBox = h('details', { class: 'setup-more' }, h('summary', { class: 'tap' }, icon('chevron-right', { size: 16, className: 'setup-more-icon' }), '選び方をくわしく'), modeGuide);
    addDescribedBy(mode.el, modeHint.id);
    addDescribedBy(mode.el, modeGuide.id);
    const paint = () => {
      fields.invoice_number.el.hidden = mode.value !== 'taxable';
      modeHint.replaceChildren(phrase(mode.value === 'exempt' ? '免税事業者にすると、取引の登録で税区分の欄を隠します（「詳細」で出せます）。' : '課税事業者にすると、取引に税区分を付け、ホームと帳票に消費税の見込み（参考）を出します。'));
    };
    paint();
    const next = forward('保存して次へ', async () => {
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
        cleared();
        go(2);
      } catch (err) {
        const f = err.field && fields[err.field];
        if (f) f.setError(err.message);
        else fail(err.message);
      } finally {
        next.disabled = false;
      }
      return undefined;
    });
    const node = card(STEPS[0], fields.business_name.el, mode.el, h('div', { class: 'setup-mode-notes' }, modeHint, guideBox), fields.opened_on.el, fields.invoice_number.el, actions(null, next));
    enterAdvances(node, () => [nameIn, mode, opened.input, fields.invoice_number.el.hidden ? null : invIn].filter(Boolean), () => next.disabled || next.click());
    return node;
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
    const add = button({ label: '口座を追加', iconName: 'plus', onClick: async () => {
      const saved = await walletDialog({ api });
      if (saved) {
        toast.success(`口座「${saved.name}」を追加しました。`);
        state.invalidate('accounts');
        render();
      }
    } });
    const next = forward('次へ', () => go(3));
    return card(STEPS[1], lead('取引に使うお金の置き場所です。現金の口座は用意してあります。銀行の口座やクレジットカードを使うなら、追加してください。'), list, h('div', null, add), h('p', { class: 'small muted' }, phrase('種類は「現金」「預金」「クレジットカード」「電子マネー」から選び、名前は自由に付けられます。あとから口座の画面でも追加できます。')), actions(() => go(1), next));
  }

  function step3() {
    // 最後の残高の欄の Enter は「保存して次へ」と同じ（残高の欄の Enter で次の残高の欄へ進むのは、期首残高の部品の中）。
    const form = createOpeningForm({ accounts: current.accounts, opening: current.opening, defaultDate: current.settings.opened_on || `${thisYear}-01-01`, onSubmit: () => next.disabled || next.click() });
    const next = forward('保存して次へ', async () => {
      const input = form.collect();
      if (!input) return;
      next.disabled = true;
      try {
        current.opening = await api.put('/opening', input);
        cleared();
        go(4);
      } catch (err) {
        // 別のプロセスが台帳に書いた（409 stale）。入力は消さずに、押し直してもらう。
        if (err.isStale) toast.info('他の場所で更新されました。読み込み直しました。入力はそのままです。もう一度「保存して次へ」を押してください。');
        // 期首の日付が年度の外、などの断りは、その欄の下に出して、欄へフォーカスを移す（通知だけでは、どこを直すか分からなかった）。
        else if (!form.showError(err)) fail(err.message);
      } finally {
        next.disabled = false;
      }
    });
    return card(STEPS[2], lead('開業日の時点で、それぞれの口座にいくらあったかを入れます。カードは、まだ払っていない額です。'), form.el, actions(() => go(2), next));
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
        cleared();
        // 次にすることまで書いた知らせなので、読み終わる前に消えないよう長めに出す（数秒で消えていた）。
        toast.success('初期設定が済みました。取引を登録するか、明細を取り込んでください。', { ms: 8000 });
        ctx.navigate('#/');
      } catch (err) {
        fail(err.message);
        start.disabled = false;
      }
    } });
    const row = (k, v) => h('div', null, h('dt', null, k), h('dd', null, v));
    // 期首残高は、科目ごとの残高と元入金を並べる（元入金と科目の数だけで、何をいくら入れたか確かめられなかった）。
    const byCode = new Map(current.accounts.map((a) => [a.code, a]));
    const balances = (bals) =>
      h(
        'ul',
        { class: 'setup-balances' },
        bals.map((b) => {
          const a = byCode.get(b.account);
          const kind = a && a.kind === 'liability' ? h('span', { class: 'badge badge--muted' }, '負債') : null;
          return h('li', null, h('span', { class: 'setup-balance-name' }, h('span', { class: 'keep' }, a ? a.name : b.account), kind), money(b.amount));
        }),
      );
    return card(
      STEPS[3],
      // 日付はほかの画面と同じ書き方（曜日つき）。口座の名前は語の途中で折らない。円は money()。
      h(
        'dl',
        { class: 'setup-summary' },
        row('事業者名', s.business_name),
        row('課税の区分', MODE_LABEL[s.tax_mode] || s.tax_mode),
        row('開業日', formatDate(s.opened_on)),
        row('口座', wallets.length ? wallets.map((w, i) => [i ? '、' : null, h('span', { class: 'keep' }, w.name)]) : 'なし'),
        row('期首残高', o && o.entry_id ? [balances(o.balances), h('p', { class: 'setup-capital' }, '元入金 ', money(o.capital), `（${o.balances.length}つの科目）`)] : 'なし（すべて0円）'),
      ),
      h('p', { class: 'small muted' }, phrase('どれも、あとから設定と口座の画面で直せます。')),
      actions(() => go(3), start),
    );
  }

  async function render() {
    // 段階が決まっているときだけ先に描く（開き直したときに、いったん1段階目を見せてから飛ばないように）。
    if (asked) {
      step = asked;
      paintStepper();
    }
    try {
      const [settings, accounts, opening] = await Promise.all([state.load('settings', { force: true }), state.load('accounts', { force: true }), api.get('/opening')]);
      if (disposed) return;
      current = { settings, accounts, opening };
      // 開き直したとき（?step= が無い）は、最後に保存した段階の次から（「続きから再開できます」と書いているのに、1段階目から始まった）。
      step = asked || Math.min(savedUpTo(current) + 1, STEPS.length);
      paintStepper();
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
  asked = readStep(ctx.query);
  render();
  const cleanup = () => {
    disposed = true;
    delete document.documentElement.dataset.setup;
    cleared();
  };
  cleanup.update = (next) => {
    asked = readStep(next.query);
    render();
    return true;
  };
  return cleanup;
}
