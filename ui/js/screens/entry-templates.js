// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// よく使う取引（テンプレート）。選ぶと入力欄に入り、「テンプレートに登録」で名前を付けて保存する。
// 中身は日付を除いた取引の入力（api-masters.md）。過去の入力からの推測はしない（利用者が自分で登録したものだけ）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { formatYen } from '../format.js';
import { field, textInput } from '../components/fields.js';
import { openModal } from '../components/dialog.js';
import { emptyState } from '../components/empty.js';
import { phrase } from '../components/phrase.js';

const KIND = { expense: '支出', income: '収入', transfer: '振替' };

function summary(body, nameOf) {
  const parts = [KIND[body.kind] || ''];
  if (body.amount) parts.push(`${formatYen(body.amount)}円`);
  if (body.account) parts.push(nameOf(body.account));
  if (body.wallet) parts.push(nameOf(body.wallet));
  if (body.partner) parts.push(body.partner);
  return parts.filter(Boolean).join('・');
}

/**
 * テンプレートを選ぶ。選んだものを返す（やめたら null）。
 * @returns {Promise<object|null>}
 */
export function pickTemplate(templates, nameOf) {
  return new Promise((resolve) => {
    let chosen = null;
    const list = templates.length
      ? h(
          'ul',
          { class: 'tpl-list' },
          templates.map((t) =>
            h(
              'li',
              null,
              h(
                'button',
                {
                  type: 'button',
                  class: 'tpl-item',
                  onclick: () => {
                    chosen = t;
                    ctrl.close('ok');
                  },
                },
                icon('bookmark'),
                h('span', { class: 'tpl-text' }, h('span', { class: 'tpl-name' }, t.name), h('span', { class: 'tpl-sum' }, summary(t.body, nameOf))),
              ),
            ),
          ),
        )
      : emptyState({ iconName: 'bookmark', title: 'テンプレートはまだありません', body: '取引を入れてから「テンプレートに登録」を押すと、ここから呼び出せます。' });
    const ctrl = openModal({
      title: 'テンプレートから入力',
      body: list,
      actions: [{ label: '閉じる', onClick: (c) => c.close('cancel') }],
      initialFocus: list.querySelector ? list.querySelector('button') || undefined : undefined,
      onClose: () => resolve(chosen),
    });
  });
}

/**
 * いまの入力を、名前を付けてテンプレートに登録する。登録できたら真。
 * @param {{ api: any, body: object, toast: any }} p
 * @returns {Promise<boolean>}
 */
export function saveTemplateDialog({ api, body, toast }) {
  return new Promise((resolve) => {
    let saved = false;
    const nameIn = textInput({ placeholder: '例: 携帯電話の利用料', maxlength: 60 });
    const nameF = field({ label: 'テンプレートの名前', control: nameIn, required: true, hint: phrase('日付は入りません。呼び出すと、それ以外の欄が入ります。') });
    const submit = async (c) => {
      const name = nameIn.value.trim();
      if (!name) {
        nameF.setError('名前を入れてください。');
        nameIn.focus();
        return;
      }
      c.setBusy(true);
      try {
        const { date: _date, client_id: _cid, ...rest } = body;
        await api.post('/templates', { name, body: rest });
        saved = true;
        toast.success(`テンプレート「${name}」を登録しました。`);
        c.setBusy(false);
        c.close('ok');
      } catch (err) {
        c.setBusy(false);
        if (err.field === 'name') nameF.setError(err.message);
        else c.setError(err.message);
      }
    };
    const ctrl = openModal({
      title: 'テンプレートに登録',
      body: nameF.el,
      actions: [
        { label: 'やめる', onClick: (c) => c.close('cancel') },
        { label: '登録する', variant: 'primary', onClick: submit },
      ],
      onClose: () => resolve(saved),
    });
    nameIn.addEventListener('keydown', (e) => {
      if (shouldIgnoreKey(e)) return;
      if (e.key === 'Enter') {
        e.preventDefault();
        submit(ctrl);
      }
    });
  });
}
