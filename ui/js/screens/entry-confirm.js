// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録の確認の小窓: 入力を捨てて離れる・戻る、日付の取り違え。
// どちらも、押し間違えても入力が残る側（「入力を続ける」「直す」）を先に置き、最初のフォーカスもそこにする（dialog.js の決まり）。

import { h, uid } from '../dom.js';
import { todayIso } from '../input-parse.js';
import { openModal } from '../components/dialog.js';
import { phrase } from '../components/phrase.js';
import { dateConcern } from './entry-hints.js';

// いま開いている確認の小窓。1枚だけにする。Esc の確認が開いたまま、ブラウザの「戻る」を押すと、2枚目が重なり、
// 1枚目は移動した後も残って、閉じるとフォーカスが BODY に落ちた（2026-10-15 の体験の評価 E4）。新しく開くときは、前のを「続ける」側で閉じる。
let openAsk = null;

function ask({ title, message, cancelLabel, confirmLabel }) {
  if (openAsk) openAsk.close('cancel');
  return new Promise((resolve) => {
    let ok = false;
    const msgId = uid('dlg-msg');
    const modal = openModal({
      title,
      body: h('p', { id: msgId }, phrase(message)),
      // 説明の文を小窓に結ぶ（読み上げで、題名に続けて読まれる）。dialog.js が aria-describedby にする。
      describedBy: msgId,
      actions: [
        { label: cancelLabel, onClick: (c) => c.close('cancel') },
        {
          label: confirmLabel,
          variant: 'primary',
          onClick: (c) => {
            ok = true;
            c.close('ok');
          },
        },
      ],
      onClose: () => {
        if (openAsk === modal) openAsk = null;
        resolve(ok);
      },
    });
    openAsk = modal;
  });
}

/**
 * 保存していない入力を捨てるか聞く。捨てるなら真。
 * @param {{ leaving: boolean }} p leaving が真なら「捨てて離れる」（行こうとした画面へ）、偽なら「捨てて戻る」（来た画面へ。Esc とやめる）
 */
export function confirmDiscard({ leaving }) {
  return ask({
    title: '入力中の内容を捨てますか',
    message: 'まだ保存していない入力があります。捨てると、元に戻せません。',
    cancelLabel: '入力を続ける',
    confirmLabel: leaving ? '捨てて離れる' : '捨てて戻る',
  });
}

/**
 * 年の打ち間違い（今日より31日を超えて先・開業日より前。entry-hints.js の dateConcern）を、保存の前に1回だけ確かめる。
 * このまま保存するなら真。「直す」なら onFix を呼んで偽。
 * @param {{ date: string, openedOn?: string, unchangedFrom?: string|null, onFix: () => void }} p
 *   unchangedFrom: 訂正の元の日付。日付を変えない訂正では聞かない（いまの帳簿にある日付なので）
 */
export async function dateLooksRight({ date, openedOn, unchangedFrom, onFix }) {
  if (unchangedFrom && date === unchangedFrom) return true;
  const message = dateConcern(date, { today: todayIso(), openedOn });
  if (!message || (await ask({ title: '日付は合っていますか', message, cancelLabel: '直す', confirmLabel: 'このまま保存' }))) return true;
  onFix();
  return false;
}

/**
 * 入力のある画面を離れる前の確認。リンク・戻る・g・⌘K（router.js の canLeave）でも、Esc（やめる）でも、同じ小窓で聞く。
 * 「入力を続ける」なら、最後にいた欄へフォーカスを戻す（離れようとして押したリンクやボタンに残さない）。
 * @param {{ ctx: any, body: HTMLElement, hasInput: () => boolean, fallback: string, disposed: () => boolean }} p
 *   fallback: 来た画面が無いとき（アドレスを直に開いた）の、やめた後の行き先
 */
export function createLeaveGuard({ ctx, body, hasInput, fallback, disposed }) {
  let leaving = false;
  let lastField = null;
  body.addEventListener('focusin', (e) => {
    if (e.target.closest('form, .entry-main')) lastField = e.target;
  });
  const backToField = () => {
    if (lastField && document.contains(lastField)) lastField.focus();
  };
  return {
    /** 保存した後の移動。確認を出さない。 */
    go(hash) {
      leaving = true;
      ctx.navigate(hash);
    },
    /** やめる（Esc）: 捨ててよければ、来た画面へ戻る。 */
    async cancel() {
      if (hasInput() && !(await confirmDiscard({ leaving: false }))) {
        backToField();
        return;
      }
      if (disposed()) return;
      leaving = true;
      ctx.back(fallback);
    },
    /** router.js から呼ばれる。to が null なら、タブを閉じる・読み込み直すとき（真偽だけ）。 */
    canLeave(to) {
      if (leaving || !hasInput()) return true;
      if (!to) return false;
      if (to.path === ctx.path) return true;
      return confirmDiscard({ leaving: true }).then((ok) => {
        if (ok) leaving = true;
        else backToField();
        return ok;
      });
    },
  };
}
