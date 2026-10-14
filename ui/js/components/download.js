// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 書き出しのリンク（<a href download>）を押したとき、中身を受け取ってから保存させる。
// ブラウザに直接保存させると、サーバーの警告（Shift_JIS に無い文字を ? にした数。X-Kamoku-Warning）も、
// 失敗の理由も画面に出せないため。href は残すので、リンクとして読まれ、新しいタブで開くこともできる。

import { h } from '../dom.js';
import { api } from '../api.js';

/** 受け取った中身を、ファイルとして保存させる。 */
function save(blob, filename) {
  const url = URL.createObjectURL(blob);
  const a = h('a', { href: url, download: filename, hidden: true });
  document.body.append(a);
  a.click();
  a.remove();
  // 保存が始まる前に消さないよう、少し待ってから手放す。
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/**
 * 書き出しのリンクに、押したときの受け取りと知らせを付ける。
 * @param {HTMLAnchorElement} link
 * @param {{ toast: any }} ctx
 */
export function enhanceDownload(link, { toast }) {
  let busy = false;
  link.addEventListener('click', async (e) => {
    const href = link.getAttribute('href');
    // 修飾キーつき（新しいタブ・別の窓）と、使えない状態のリンクは、ブラウザに任せる。
    if (!href || e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    if (busy) return;
    busy = true;
    link.setAttribute('aria-busy', 'true');
    try {
      const file = await api.file(href);
      save(file.blob, file.filename || link.getAttribute('download') || 'kamoku-export');
      if (file.unmappable > 0) {
        toast.info(`Shift_JIS に無い文字が${file.unmappable}か所あったので、「?」に置き換えて書き出しました。元の文字のまま残すときは、UTF-8（BOMつき）で書き出してください。`);
      }
    } catch (err) {
      toast.error(err.message);
    } finally {
      busy = false;
      link.removeAttribute('aria-busy');
    }
  });
}
