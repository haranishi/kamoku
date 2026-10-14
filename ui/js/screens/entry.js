// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-02 取引を登録 `#/entry`、訂正 `#/entry/<id>`。左に入力、右に仕訳のプレビュー。
// 保存には client_id（再送防止の印）を付け、成功したら作り直す。訂正には理由と expected_revision を付け、
// ほかの場所で更新されていたら（409 stale_revision / stale）最新を読み直し、入力は残して差分を見せる。

import { h, replace } from '../dom.js';
import { newClientId } from '../api.js';
import { shouldIgnoreKey } from '../ime.js';
import { field, textInput, button } from '../components/fields.js';
import { createTabs } from '../components/tabs.js';
import { banner } from '../components/banner.js';
import { confirmDialog } from '../components/dialog.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { createPreviewPanel } from './entry-preview.js';
import { createSimpleForm } from './entry-simple.js';
import { createJournalForm } from './entry-journal.js';
import { diffSimple, diffJournal, confirmCorrection, journalInputOf, staleNotice } from './entry-correct.js';
import { pickTemplate, saveTemplateDialog } from './entry-templates.js';
import { rememberWallet } from './entry-options.js';
import { createSaveBar } from './entry-savebar.js';

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const id = ctx.params.id || null;
  const correcting = Boolean(id);
  const { api, state, toast } = ctx;
  let disposed = false;
  let simple = null;
  let journal = null;
  let active = 'simple';
  let clientId = newClientId();
  let busy = false;
  let entry = null;
  let data = null;
  const cleanups = [];

  const subtitle = h('p', { class: 'muted small' });
  const templateBtn = correcting ? null : button({ label: 'テンプレートから入力', iconName: 'bookmark', onClick: () => chooseTemplate() });
  root.append(
    h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, correcting ? '取引を訂正' : '取引を登録'), subtitle), h('div', { class: 'page-head-actions' }, templateBtn)),
  );
  const body = h('div', { class: 'entry-layout' });
  root.append(body);

  const preview = createPreviewPanel();
  const formHost = h('div', { class: 'entry-form-host' });
  const staleBox = h('div', { class: 'stale-box' });
  const reasonIn = textInput({ placeholder: '例: 金額の入力ミス', maxlength: 200 });
  const reasonF = field({ label: '訂正の理由', control: reasonIn, required: true, hint: '履歴に残ります。あとで見たときに、直した訳が分かるように書いてください。' });
  // 理由の欄は、訂正のときの最後の欄。Enter（と Ctrl/Cmd+Enter）で確認へ進む。
  reasonIn.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e)) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      submit({ cont: true });
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancel();
    }
  });
  const bar = createSaveBar({ correcting, onSave: (o) => submit(o), onCancel: () => cancel(), onTemplate: () => saveTemplate() });
  let voidedElsewhere = false;

  const form = () => (active === 'journal' ? journal : simple);
  const nameOf = (code) => {
    const a = (data && data.accounts.find((x) => x.code === code)) || null;
    return a ? a.name : code;
  };

  const setError = (msg) => bar.setError(msg);

  function updateSaveState() {
    const ro = state.isReadOnly();
    const f = form();
    const failed = Boolean(f && f.previewFailed);
    const unbalanced = active === 'journal' && journal && !journal.balanced;
    let reason = '';
    if (ro) reason = state.readOnlyReason();
    else if (voidedElsewhere) reason = 'この仕訳は取消済みのため、訂正できません。';
    else if (failed) reason = 'プレビューを作れないため、保存を止めています。';
    else if (unbalanced) reason = '貸借が合うまで、保存できません。';
    bar.update({ reason, readOnly: ro, busy });
  }

  function setBusy(b) {
    busy = b;
    updateSaveState();
  }

  function showError(err) {
    if (err.isStale) {
      setError('ほかの場所で台帳が更新されたため、保存できませんでした。最新の状態を読み込みました。入力はそのままです。もう一度保存してください。');
      state.load('status', { force: true }).catch(() => {});
    } else if (err.isReadOnly) {
      setError(state.readOnlyReason());
    } else if (!(err.field && form().showFieldError(err.field, err.message))) {
      setError(err.message);
    }
  }

  async function submit({ cont }) {
    if (busy || !form()) return;
    setError('');
    if (state.isReadOnly()) return setError(state.readOnlyReason());
    const f = form();
    const input = f.collect();
    if (!input) return undefined;
    if (f.previewFailed) return setError('仕訳のプレビューを作れないため、保存を止めています。サーバーが動いているかを確かめて、プレビューの「もう一度試す」を押してください。');
    if (correcting) return submitCorrection(input);
    setBusy(true);
    try {
      const res = await api.post('/entries', { ...input, client_id: clientId });
      clientId = newClientId();
      if (input.wallet) rememberWallet(input.wallet);
      state.invalidate('partners');
      state.load('status', { force: true }).catch(() => {});
      toast.success(`No.${res.no} を登録しました。`, { action: { label: '仕訳を見る', href: `#/journal/${res.id}` } });
      if (!cont) {
        ctx.navigate(`#/journal/${res.id}`);
      } else if (active === 'simple') {
        simple.resetForNext();
        simple.focusAmount();
      } else {
        showJournal({ date: input.date });
      }
    } catch (err) {
      showError(err);
    } finally {
      if (!disposed) setBusy(false);
    }
    return undefined;
  }

  async function submitCorrection(input) {
    const reason = reasonIn.value.trim();
    const diff = active === 'journal' ? diffJournal(journalInputOf(entry), input, nameOf) : diffSimple(entry.input, input, nameOf);
    if (!diff.length) return setError('変わった欄がありません。直したい欄を変えてから、保存してください。');
    if (!reason) {
      reasonF.setError('訂正の理由を入れてください。');
      reasonIn.focus();
      return undefined;
    }
    reasonF.setError('');
    if (!(await confirmCorrection({ no: entry.no, revision: entry.revision, diff, reason }))) return undefined;
    setBusy(true);
    try {
      const res = await api.put(`/entries/${id}`, { ...input, reason, expected_revision: entry.revision });
      state.invalidate('partners');
      state.load('status', { force: true }).catch(() => {});
      toast.success(`No.${res.no} を訂正しました（第${res.revision}版）。`, { action: { label: '仕訳を見る', href: `#/journal/${id}` } });
      ctx.navigate(`#/journal/${id}`);
    } catch (err) {
      if (err.isStale) await reloadAfterStale(input);
      else showError(err);
    } finally {
      if (!disposed) setBusy(false);
    }
    return undefined;
  }

  // ほかの場所で更新された: 最新の版を読み直し、入力は残したまま、最新の版との違いを見せる。
  async function reloadAfterStale(input) {
    toast.info('他の場所で更新されました。読み込み直します。');
    try {
      entry = await api.get(`/entries/${id}`);
    } catch (err) {
      setError(err.message);
      return;
    }
    subtitle.textContent = `No.${entry.no} の仕訳（いまは第${entry.revision}版）`;
    const diff = active === 'journal' ? diffJournal(journalInputOf(entry), input, nameOf) : diffSimple(entry.input, input, nameOf);
    staleBox.replaceChildren(staleNotice(entry, diff));
    staleBox.scrollIntoView({ block: 'nearest' });
    if (entry.state === 'voided') {
      voidedElsewhere = true;
      updateSaveState();
    }
  }

  async function cancel() {
    const f = form();
    if (f && f.dirty && !(await confirmDialog({ title: '入力中の内容を捨てますか', message: 'まだ保存していない入力があります。', confirmLabel: '捨てて戻る', cancelLabel: '入力を続ける' }))) return;
    ctx.navigate(correcting ? `#/journal/${id}` : '#/');
  }

  async function chooseTemplate() {
    if (!data) return;
    const templates = await state.load('templates', { force: true }).catch(() => data.templates);
    const t = await pickTemplate(templates, nameOf);
    if (!t || !simple) return;
    if (active !== 'simple') switchMode('simple');
    simple.applyTemplate(t.body);
    simple.focusAmount();
  }

  async function saveTemplate() {
    const snap = active === 'simple' && simple ? simple.snapshot() : null;
    if (!snap) return setError('テンプレートにするには、金額・科目・口座を入れてください（かんたん入力のときだけ登録できます）。');
    if (await saveTemplateDialog({ api, body: snap, toast })) state.invalidate('templates');
    return undefined;
  }

  const common = () => ({
    accounts: data.accounts,
    partners: data.partners,
    settings: data.settings,
    preview,
    api,
    onSubmit: (o) => submit(o),
    onCancel: () => cancel(),
    onPreviewState: () => updateSaveState(),
    onBalance: () => updateSaveState(),
  });

  function showJournal(initial) {
    if (journal) journal.destroy();
    journal = createJournalForm({ ...common(), initial });
    formHost.replaceChildren(journal.el);
    if (simple) simple.setPreviewActive(false);
    journal.setPreviewActive(true);
    journal.focusFirst();
    updateSaveState();
  }

  function switchMode(next) {
    active = next;
    if (next === 'journal') {
      const seed = simple ? simple.journalSeed() : {};
      showJournal(seed.lines && seed.lines.length >= 2 ? seed : { date: seed.date });
    } else {
      if (journal) {
        journal.destroy();
        journal = null;
      }
      formHost.replaceChildren(simple.el);
      simple.setPreviewActive(true);
    }
    bar.showTemplate(next === 'simple');
    updateSaveState();
  }

  function build() {
    const tabs = correcting ? null : createTabs({ label: '入力の形', tabs: [{ id: 'simple', label: 'かんたん入力' }, { id: 'journal', label: '仕訳で入力' }], selected: 'simple', onChange: switchMode });
    if (correcting) {
      subtitle.textContent = `No.${entry.no} の仕訳（いまは第${entry.revision}版）。元の記録は履歴に残ります。`;
      if (entry.input) {
        simple = createSimpleForm({ ...common(), initial: entry.input });
        formHost.append(simple.el);
      } else {
        active = 'journal';
        journal = createJournalForm({ ...common(), initial: journalInputOf(entry) });
        formHost.append(journal.el);
        journal.setPreviewActive(true);
      }
    } else {
      simple = createSimpleForm(common());
      formHost.append(simple.el);
    }
    if (tabs) {
      tabs.panel.append(formHost);
    }
    const main = h(
      'section',
      { class: 'card entry-main', 'aria-label': correcting ? '訂正の入力' : '取引の入力' },
      tabs ? tabs.el : null,
      staleBox,
      tabs ? tabs.panel : formHost,
      correcting ? reasonF.el : null,
    );
    // 並びは「入力 → プレビュー → 保存のボタン」（screens-main.md の図）。広い画面では、プレビューを右の列に置く。
    replace(body, main, h('aside', { class: 'entry-side' }, preview.el), bar.el);
    updateSaveState();
  }

  const stop = delayedSkeleton(body, () => skeletonBlock({ lines: 6, label: '入力欄を準備しています' }));
  // 開くたびに読み直す。設定（課税の区分）や科目が、ほかの画面や別のタブで変わっていることがあるため。
  const fresh = { force: true };
  Promise.all([state.load('settings', fresh), state.load('accounts', fresh), state.load('partners', fresh), state.load('templates', fresh), correcting ? api.get(`/entries/${id}`) : Promise.resolve(null)])
    .then(([settings, accounts, partners, templates, e]) => {
      stop();
      if (disposed) return;
      data = { settings, accounts, partners, templates };
      entry = e;
      if (correcting && entry.state === 'voided') {
        body.replaceChildren(banner({ kind: 'danger', title: 'この仕訳は取消済みです', body: '取消済みの仕訳は訂正できません。', actions: h('a', { href: `#/journal/${id}` }, '仕訳の詳細に戻る') }));
        return;
      }
      build();
    })
    .catch((err) => {
      stop();
      if (!disposed) body.replaceChildren(banner({ kind: 'danger', title: '入力欄を準備できませんでした', body: err.message }));
    });

  // 種別のキー（文字入力の欄の外で）。1 2 3 で支出・収入・振替。
  const kindKey = (k) => () => {
    if (!simple || active !== 'simple') return false;
    simple.setKind(k);
    simple.focusKind();
    return true;
  };
  cleanups.push(ctx.keys.register({ 1: kindKey('expense'), 2: kindKey('income'), 3: kindKey('transfer'), n: () => true }));
  cleanups.push(state.subscribe('status', () => updateSaveState()));

  const cleanup = () => {
    disposed = true;
    cleanups.forEach((fn) => fn());
    if (simple) simple.destroy();
    if (journal) journal.destroy();
  };
  // 同じ画面のまま（サイドバーの「取引を登録」をもう一度押したなど）は、入力を残す。訂正と新規の行き来は作り直す。
  cleanup.update = (next) => (next.params.id || null) === id;
  return cleanup;
}
