// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳のプレビューの呼び出し役。入力が変わるたびに、150msの待ちの後で POST /entries/preview を送る。
// 新しい入力が来たら、古い要求は取り消す（遅れて届いた古い答えで、表示が戻らないように）。
// 通信そのものが失敗したら failed を立て、画面は保存を止めて理由を出す（u1 の依頼書）。

export const PREVIEW_DELAY_MS = 150;

/**
 * @param {{ api: any, preview: ReturnType<import('./entry-preview.js').createPreviewPanel>, build: () => any|null,
 *           onLines?: (lines: any[]) => void, onFailedChange?: (failed: boolean) => void, idleText: string,
 *           blocked?: () => string }} opts
 *   blocked: 誤りの文を返すと、問い合わせずに、仕訳の代わりにその誤りを出す（範囲外の割合で、前の割合の仕訳を見せないため）。
 */
export function createPreviewRunner(opts) {
  let timer = null;
  let seq = 0;
  let ctrl = null;
  let failed = false;
  let active = true;

  function setFailed(next) {
    if (next === failed) return;
    failed = next;
    if (opts.onFailedChange) opts.onFailedChange(failed);
  }

  async function run() {
    if (!active) return;
    clearTimeout(timer);
    if (ctrl) ctrl.abort();
    const my = ++seq;
    const problem = opts.blocked ? opts.blocked() : '';
    if (problem) {
      setFailed(false);
      opts.preview.blocked(problem);
      if (opts.onLines) opts.onLines([]);
      return;
    }
    const input = opts.build();
    if (!input) {
      setFailed(false);
      opts.preview.idle(opts.idleText);
      if (opts.onLines) opts.onLines([]);
      return;
    }
    ctrl = new AbortController();
    try {
      const res = await opts.api.post('/entries/preview', input, { signal: ctrl.signal });
      if (my !== seq || !active) return;
      setFailed(false);
      opts.preview.show(res);
      if (opts.onLines) opts.onLines(res.lines);
    } catch (err) {
      if ((err && err.name === 'AbortError') || my !== seq || !active) return;
      if (opts.onLines) opts.onLines([]);
      if (err.isNetwork || !err.status || err.status >= 500) {
        setFailed(true);
        opts.preview.failed(err.message, run);
      } else {
        // 入力の誤り（科目が無い、割合の範囲など）は、打っている途中にもよくあるので、止めずに案内だけ出す。
        setFailed(false);
        opts.preview.invalid(err.message);
      }
    }
  }

  return {
    schedule() {
      if (!active) return;
      clearTimeout(timer);
      timer = setTimeout(run, PREVIEW_DELAY_MS);
    },
    run,
    get failed() {
      return failed;
    },
    /** 表示中のフォームだけが送る。切り替えたら、すぐに出し直す。 */
    setActive(on) {
      active = on;
      if (on) run();
      else {
        clearTimeout(timer);
        if (ctrl) ctrl.abort();
      }
    },
    cancel() {
      active = false;
      clearTimeout(timer);
      if (ctrl) ctrl.abort();
    },
  };
}
