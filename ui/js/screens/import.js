// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-04 取込 `#/import`。3段階: (1) 口座を選び、CSVを置く (2) 列の対応を確かめる (3) 結果を見て取り込む。
// サーバーは読み込んだファイルを30分だけ覚えている。期限が切れたら（404）、もう一度ファイルを選んでもらう。
// 口座はアドレスの ?wallet= に持つ（口座の画面から、その口座を選んだ状態で開けるように）。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { banner } from '../components/banner.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { createFileStep } from './import-file.js';
import { createColumnsStep } from './import-columns.js';
import { createCheckStep, resultView } from './import-check.js';

const STEPS = ['口座とファイル', '列の対応', '確かめて取り込む'];
const EXPIRED = '時間が経ったため、読み込んだファイルの期限が切れました（30分）。もう一度ファイルを選んでください。';

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  let disposed = false;
  let wallets = [];
  // 進み具合: wallet、選んだファイル（文字コードを選び直すと送り直す）、サーバーの応え、列の対応、確認の結果。
  const s = { wallet: ctx.query.wallet || '', file: null, bytes: null, encoding: 'auto', upload: null, mapping: null, preview: null };
  const stepper = h('ol', { class: 'stepper', 'aria-label': '取込の段階' });
  const body = h('div', { class: 'import-body' });
  const topError = h('div');
  root.append(h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '明細を取り込む'), h('p', { class: 'muted small' }, '銀行やカードの明細のCSVを、未確認に並べます。確定するまで、仕訳帳と帳票には入りません。'))), stepper, topError, body);

  const walletName = () => (wallets.find((w) => w.code === s.wallet) || { name: '' }).name;

  function paintStepper(i) {
    stepper.replaceChildren(
      ...STEPS.map((label, j) =>
        h('li', { class: ['step', j < i && 'is-done', j === i && 'is-current'], 'aria-current': j === i ? 'step' : null }, h('span', { class: 'step-no' }, j < i ? icon('check', { size: 16 }) : String(j + 1)), h('span', { class: 'step-label' }, label), j < i ? h('span', { class: 'visually-hidden' }, '（済み）') : null),
      ),
    );
  }

  function show(i, node, focus) {
    paintStepper(i);
    topError.replaceChildren();
    replace(body, h('section', { class: 'card import-card', 'aria-label': STEPS[i] }, h('h2', { class: 'card-title', tabindex: '-1' }, `${i + 1}. ${STEPS[i]}`), node.el || node));
    if (focus) focus();
    else body.querySelector('h2').focus();
  }

  function fail(err, i) {
    if (err.status === 404) {
      s.upload = null;
      step1(EXPIRED);
      return;
    }
    topError.replaceChildren(banner({ kind: 'danger', title: i === 0 ? 'ファイルを読めませんでした' : '取り込めませんでした', body: err.message }));
    topError.querySelector('.banner').scrollIntoView({ block: 'nearest' });
  }

  function step1(message) {
    s.file = null;
    s.bytes = null;
    s.encoding = 'auto';
    const f = createFileStep({
      wallets,
      wallet: s.wallet,
      onWallet: (code) => {
        s.wallet = code;
        ctx.navigate(ctx.router.buildHash('/import', { wallet: code }), { replace: true });
      },
      onFile: async (file) => {
        s.file = file;
        f.setBusy(`「${file.name}」を読み込んでいます…`);
        try {
          s.bytes = await file.arrayBuffer();
          await upload();
        } catch (err) {
          if (disposed) return;
          f.setBusy('');
          f.setError(err.message);
        }
      },
    });
    show(0, f, () => f.focus());
    if (message) f.setError(message);
  }

  async function upload() {
    s.upload = await api.upload('/imports/upload', { name: s.file.name, encoding: s.encoding, wallet: s.wallet }, s.bytes);
    if (!disposed) step2();
  }

  function step2() {
    const c = createColumnsStep({
      upload: s.upload,
      filename: s.file ? s.file.name : s.upload.filename,
      encodingChoice: s.encoding,
      onEncoding: async (enc) => {
        s.encoding = enc;
        try {
          await upload();
          toast.info('文字コードを変えて、読み直しました。');
        } catch (err) {
          fail(err, 1);
        }
      },
      onBack: () => step1(),
      onNext: async (mapping) => {
        s.mapping = mapping;
        try {
          s.preview = await api.post('/imports/preview', { upload_id: s.upload.upload_id, wallet: s.wallet, mapping });
          if (!disposed) step3();
        } catch (err) {
          fail(err, 1);
        }
      },
    });
    show(1, c);
  }

  function step3() {
    const c = createCheckStep({
      preview: s.preview,
      walletName: walletName(),
      onBack: () => step2(),
      onCommit: async ({ force, saveProfile }) => {
        c.setBusy(true);
        try {
          const res = await api.post('/imports/commit', { upload_id: s.upload.upload_id, wallet: s.wallet, mapping: s.mapping, save_profile: saveProfile, force_lines: force });
          state.load('status', { force: true }).catch(() => {});
          if (disposed) return;
          toast.success(`${res.created}件を未確認に入れました。`, { action: { label: '未確認を開く', href: `#/inbox?wallet=${encodeURIComponent(s.wallet)}` } });
          paintStepper(3);
          replace(body, resultView({ result: res, wallet: s.wallet, onAgain: () => step1() }));
          body.querySelector('a.btn').focus();
        } catch (err) {
          c.setBusy(false);
          fail(err, 2);
        }
      },
    });
    show(2, c, () => c.focus());
  }

  const stop = delayedSkeleton(body, () => skeletonBlock({ lines: 4, label: '口座を読み込んでいます' }));
  state
    .load('accounts', { force: true })
    .then((accounts) => {
      stop();
      if (disposed) return;
      wallets = accounts.filter((a) => a.wallet && a.enabled);
      if (!wallets.some((w) => w.code === s.wallet)) s.wallet = '';
      if (!wallets.length) {
        replace(body, banner({ kind: 'warn', title: '口座がまだありません', body: '明細を取り込む前に、口座を足してください。', actions: h('a', { class: 'btn btn--sm', href: '#/wallets' }, '口座の画面へ') }));
        return;
      }
      step1();
    })
    .catch((err) => {
      stop();
      if (!disposed) replace(body, banner({ kind: 'danger', title: '取込の画面を準備できませんでした', body: err.message }));
    });

  const cleanup = () => {
    disposed = true;
  };
  // アドレスの口座だけが変わったとき（選び直し）は、作り直さない。
  cleanup.update = () => true;
  return cleanup;
}
