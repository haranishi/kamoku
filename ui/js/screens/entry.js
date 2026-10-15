// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-02 取引を登録 `#/entry`、訂正 `#/entry/<id>`。左に入力、右に仕訳のプレビュー。
// 保存には client_id（再送防止の印）を付け、成功したら作り直す。訂正には理由と expected_revision を付け、
// ほかの場所で更新されていたら（409 stale_revision / stale）最新を読み直し、入力は残して差分を見せる。
// 開いたら、新規は日付、訂正は金額の欄へフォーカスを移す（n・e は打ち始めるために開くため。見出しはフォームの名前で読み上げる）。
// 保存していない入力があるうちは、どの道で画面を離れても（リンク・戻る・g・⌘K・タブを閉じる）、先に確認する。

import { h, replace } from '../dom.js';
import { newClientId } from '../api.js';
import { shouldIgnoreKey } from '../ime.js';
import { field, textInput, button } from '../components/fields.js';
import { createTabs } from '../components/tabs.js';
import { banner } from '../components/banner.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { createPreviewPanel } from './entry-preview.js';
import { createSimpleForm } from './entry-simple.js';
import { createJournalForm } from './entry-journal.js';
import { diffSimple, diffJournal, confirmCorrection, journalInputOf, staleNotice } from './entry-correct.js';
import { pickTemplate, saveTemplateDialog } from './entry-templates.js';
import { rememberWallet } from './entry-options.js';
import { createSaveBar } from './entry-savebar.js';
import { createLeaveGuard, dateLooksRight } from './entry-confirm.js';
import { detailHash, fromBackParam } from './journal-return.js';
import { lockedMessage } from './entry-hints.js';

const TITLE_ID = 'entry-title';

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const id = ctx.params.id || null;
  const correcting = Boolean(id);
  // 訂正の後に移る詳細。訂正の前に開いていた一覧の条件（検索など）や元帳を、アドレスの back から引き継ぐ（journal-return.js）。
  const afterHash = correcting ? detailHash(id, fromBackParam(ctx.query.back)) : null;
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
  const title = h('h1', { tabindex: '-1', id: TITLE_ID }, correcting ? '取引を訂正' : '取引を登録');
  root.append(h('div', { class: 'page-head entry-head' }, h('div', { class: 'page-head-text' }, title, subtitle), h('div', { class: 'page-head-actions' }, templateBtn)));
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
  // 保存の帯の「N か所を直してください：…」は、保存を試みたときの結果。欄を直したら、古い一言は消す（直した欄を指したまま残っていた）。
  body.addEventListener('input', () => bar.clearAnnounced());
  body.addEventListener('change', () => bar.clearAnnounced());
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

  /** 締めた日の断り（欄を確定したとき・保存で断られたとき）。締めていなければ空。 */
  const lockedNote = (date, onSave = false) => lockedMessage({ date, lockedUntil: data && data.settings && data.settings.locked_until, correcting, originalDate: entry ? entry.date : null, onSave });

  function showError(err, input) {
    // 締めた期間の断りは、締めた日を入れた文に組み替える（サーバーの文には日付が無い）。
    const message = (err.code === 'locked_period' && lockedNote(input && input.date, true)) || err.message;
    if (err.isStale) {
      setError('ほかの場所で台帳が更新されたため、保存できませんでした。最新の状態を読み込みました。入力はそのままです。もう一度保存してください。');
      state.load('status', { force: true }).catch(() => {});
    } else if (err.isReadOnly) {
      setError(state.readOnlyReason());
    } else if (!(err.field && form().showFieldError(err.field, message))) {
      setError(message);
    }
  }

  // 入力がある（初期値から変わった）か。仕訳で入力に切り替えた後も、かんたん入力の欄の中身は残っているので両方を見る。
  const hasInput = () => Boolean((simple && simple.dirty) || (journal && journal.dirty) || (correcting && reasonIn.value.trim()));
  const guard = createLeaveGuard({ ctx, body, hasInput, fallback: correcting ? afterHash : '#/', disposed: () => disposed });
  const cancel = () => guard.cancel();

  // 年の打ち間違いは、保存の前に1回だけ確かめる。訂正で日付を変えないときは聞かない。
  const dateChecked = (date) =>
    dateLooksRight({ date, openedOn: data.settings && data.settings.opened_on, unchangedFrom: correcting && entry ? entry.date : null, onFix: () => !disposed && form() && form().focusDate() });

  async function submit({ cont }) {
    if (busy || !form()) return;
    setError('');
    if (state.isReadOnly()) return setError(state.readOnlyReason());
    const f = form();
    const input = f.collect();
    if (!input) {
      bar.announce(f.problems);
      return undefined;
    }
    if (f.previewFailed) return setError('仕訳のプレビューを作れないため、保存を止めています。サーバーが動いているかを確かめて、プレビューの「もう一度試す」を押してください。');
    if (correcting) return submitCorrection(input);
    if (!(await dateChecked(input.date)) || disposed) return undefined;
    setBusy(true);
    try {
      const res = await api.post('/entries', { ...input, client_id: clientId });
      clientId = newClientId();
      if (input.wallet) rememberWallet(input.wallet);
      state.invalidate('partners');
      state.load('status', { force: true }).catch(() => {});
      toast.success(`No.${res.no} を登録しました。`, { action: { label: '仕訳を見る', href: `#/journal/${res.id}` } });
      if (!cont) {
        guard.go(`#/journal/${res.id}`);
      } else if (active === 'simple') {
        simple.resetForNext();
        simple.focusAmount();
      } else {
        // 「かんたん入力」の欄の中身は、仕訳の表の元にしたものなので、一緒に空にする（保存した後に、離れる確認が出ないように）。
        if (simple) simple.resetForNext();
        showJournal({ date: input.date });
      }
    } catch (err) {
      showError(err, input);
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
    if (!(await dateChecked(input.date)) || disposed) return undefined;
    if (!(await confirmCorrection({ no: entry.no, revision: entry.revision, diff, reason }))) return undefined;
    setBusy(true);
    try {
      const res = await api.put(`/entries/${id}`, { ...input, reason, expected_revision: entry.revision });
      state.invalidate('partners');
      state.load('status', { force: true }).catch(() => {});
      // 通知のリンクも、保存の後の行き先（afterHash）と同じにする。条件なしの詳細にすると、元帳や検索の条件への戻り先（back）が外れる。
      toast.success(`No.${res.no} を訂正しました（第${res.revision}版）。`, { action: { label: '仕訳を見る', href: afterHash } });
      guard.go(afterHash);
    } catch (err) {
      if (err.isStale) await reloadAfterStale(input);
      else showError(err, input);
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
    labelledBy: TITLE_ID,
    preview,
    api,
    onSubmit: (o) => submit(o),
    onCancel: () => cancel(),
    onPreviewState: () => updateSaveState(),
    onBalance: () => updateSaveState(),
    // 日付を確定したときの一言（締めた日以前なら、欄の下に出す）。
    dateProblem: (date) => lockedNote(date),
  });

  function showJournal(initial) {
    if (journal) journal.destroy();
    journal = createJournalForm({ ...common(), initial });
    formHost.replaceChildren(journal.el);
    if (simple) simple.setPreviewActive(false);
    journal.setPreviewActive(true);
    // 「入力の形」のタブを ← → で選んだときは、フォーカスをタブに残す（入力欄へ移ると、タブを順に聞いて選べなかった）。
    // マウスで押したとき（Chrome はタブにフォーカスを置くが、:focus-visible にはならない）と、保存して続けた後は、最初の欄へ。
    if (!keyOnTab()) journal.focusFirst();
    updateSaveState();
  }

  /** キーで動かしたフォーカスが、タブの上にあるか。 */
  function keyOnTab() {
    const a = document.activeElement;
    if (!a || a.getAttribute('role') !== 'tab') return false;
    try {
      return a.matches(':focus-visible');
    } catch {
      return false;
    }
  }

  function switchMode(next) {
    active = next;
    // 切り替えた先に無い欄（かんたん入力の「事業の割合」など）を指す一言は残さない。
    bar.clearAnnounced();
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
    // 並びは「入力 → プレビュー → 保存のボタン」（screens-main.md の図）。広い画面（1280px以上）では、プレビューと保存を右の列に重ね、
    // 狭い画面では1列にして、保存の帯を画面の下に付ける（右の列の入れ物は、そのとき箱を作らない。screens-entry.css）。
    // 「テンプレートに登録」は保存の組の後ろに置く（Tab で最後の欄から保存の組へ直に進む。見た目の位置は CSS が今のままにする）。
    replace(body, main, h('div', { class: 'entry-rail' }, h('aside', { class: 'entry-side' }, preview.el), bar.el, bar.tplEl));
    updateSaveState();
    // 最初の欄へ（router.js は、この画面では見出しへ移さない）。新規は日付から、訂正は直すことの多い金額から。
    if (document.querySelector('dialog[open]')) return;
    if (correcting) form().focusAmount();
    else simple.focusDate();
  }

  // 欄を作れなかったときは、見出しへ（読み上げが画面の名前から始まるように）。
  const focusTitle = () => {
    if (!document.querySelector('dialog[open]')) title.focus({ preventScroll: true });
  };

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
        body.replaceChildren(banner({ kind: 'danger', title: 'この仕訳は取消済みです', body: '取消済みの仕訳は訂正できません。', actions: h('a', { href: afterHash }, '仕訳の詳細に戻る') }));
        focusTitle();
        return;
      }
      build();
    })
    .catch((err) => {
      stop();
      if (disposed) return;
      body.replaceChildren(banner({ kind: 'danger', title: '入力欄を準備できませんでした', body: err.message }));
      focusTitle();
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
  // 離れる前の確認（router.js）。Esc と同じ小窓で聞き、「捨てて離れる」なら行こうとした先へ進む。
  cleanup.canLeave = (to) => guard.canLeave(to);
  cleanup.managesFocus = true;
  return cleanup;
}
