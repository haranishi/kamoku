// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 共通の枠: コマンド一覧、ショートカットの一覧、ライトとダーク、フォーカスの輪、動きを減らす設定。

import { test, expect } from '@playwright/test';
import { open, ensureEntryWith, addCashExpense, uniqueText, apiCall, entryFields } from './helpers.mjs';

// 色の文字（rgb(...)）を [r, g, b] にして、WCAGのコントラスト比を出す。
function ratio(a, b) {
  const lum = (rgb) => {
    const [r, g, bl] = rgb.match(/\d+(\.\d+)?/g).slice(0, 3).map(Number).map((v) => {
      const c = v / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

const bodyBg = (page) => page.evaluate(() => getComputedStyle(document.body).backgroundColor);

test('AC-10.1 Ctrl/Cmd+K のコマンド一覧から、画面を移り、仕訳を文字で探せる', async ({ page }) => {
  await open(page, '#/');
  // 本物のサーバーの合成データには「家賃」の仕訳が無いことがあるので、無ければ作る。
  await ensureEntryWith(page, '家賃');
  await page.keyboard.press('ControlOrMeta+K');
  const dialog = page.getByRole('dialog', { name: 'コマンド' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('combobox')).toBeFocused();
  await page.keyboard.insertText('しわけちょう');
  await expect(dialog.getByRole('option').first()).toContainText('仕訳帳');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/journal$/);
  await expect(dialog).toHaveCount(0);

  await page.keyboard.press('ControlOrMeta+K');
  await page.keyboard.insertText('家賃');
  const search = page.getByRole('option', { name: /仕訳を「家賃」で探す/ });
  await expect(search).toBeVisible();
  await search.click();
  await expect(page).toHaveURL(/#\/journal\?q=/);
  await expect(page.locator('.journal-table tbody tr').first()).toContainText('家賃');

  // U2 の画面（帳票）へも移れる。「帳票」は月次推移のタブに着く。
  await page.keyboard.press('ControlOrMeta+K');
  await page.keyboard.insertText('帳票');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/reports\/monthly/);
  await expect(page.locator('main h1')).toHaveText('帳票');
  await expect(page.getByRole('tab', { name: '月次推移' })).toHaveAttribute('aria-selected', 'true');

  // 帳票のタブへ直に移るコマンドもある。
  await page.keyboard.press('ControlOrMeta+K');
  await page.keyboard.insertText('しさんひょう');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/reports\/trial-balance/);

  // どの画面にも当たらないアドレスは「見つかりません」の画面に着く。
  await page.goto('/#/nowhere');
  await expect(page.locator('main')).toContainText('このアドレスの画面はありません');
});

// AC-10.1 一般的な言葉（編集・修正・削除）でコマンドを探しても、訂正・取消が見つかる（訂正・取消は仕訳帳で仕訳を開いて行う）。
test('AC-10.1 コマンド一覧で「編集」「修正」「削除」と打つと、訂正・取消のコマンドが出て、仕訳帳へ移れる', async ({ page }) => {
  await open(page, '#/');
  const dialog = page.getByRole('dialog', { name: 'コマンド' });
  for (const [word, label] of [['編集', '仕訳を訂正する（仕訳帳で仕訳を開く）'], ['修正', '仕訳を訂正する（仕訳帳で仕訳を開く）'], ['削除', '仕訳を取り消す（仕訳帳で仕訳を開く）']]) {
    await page.keyboard.press('ControlOrMeta+K');
    await expect(dialog).toBeVisible();
    await page.keyboard.insertText(word);
    await expect(dialog.getByRole('option').first(), word).toHaveText(label);
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
  }
  // 閉じた直後（閉じた知らせが届く前）に Ctrl/Cmd+K を押しても開く（前は、開いたままとみなして何も開かなかった）。
  await page.keyboard.press('ControlOrMeta+K');
  await expect(dialog).toBeVisible();
  const reopened = await page.evaluate(() => {
    document.querySelector('dialog.palette[open]').close('cancel');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true, bubbles: true, cancelable: true }));
    return Boolean(document.querySelector('dialog.palette[open]'));
  });
  expect(reopened).toBe(true);
  await expect(dialog).toHaveCount(1);
  await page.keyboard.insertText('削除');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/journal$/);
  await expect(page.locator('dialog.palette')).toHaveCount(0);
});

test('AC-10.2 ? でショートカットの一覧が出て、Escで閉じる', async ({ page }) => {
  await open(page, '#/');
  await page.keyboard.press('?');
  const dialog = page.getByRole('dialog', { name: 'ショートカット' });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText('仕訳帳へ');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

// WCAG 2.1.4 1文字のショートカット（n・?・/・g＋文字、画面の j k e v など）は、ショートカットの一覧の小窓で切れる。
// 切ると画面が移らず、⌘K（Ctrl+K）は開く。切った設定は読み込み直しても残り、コマンド一覧から戻すと n がまた働く。
test('AC-10.5 1文字のショートカット（WCAG 2.1.4）を切ると n・g＋文字・? が働かず、⌘K は開く。コマンド一覧から戻すと n が働く', async ({ page }) => {
  await open(page, '#/');
  await page.keyboard.press('?');
  const help = page.getByRole('dialog', { name: 'ショートカット' });
  const box = help.getByRole('checkbox', { name: '1文字のショートカットを使う' });
  await expect(box).toBeChecked();
  const whole = help.getByRole('table', { name: '全体のショートカット' });
  const mark = (table, text) => table.locator('tbody tr').filter({ hasText: text }).getByText('いま切ってあります');
  await expect(help.getByText('いま切ってあります').filter({ visible: true })).toHaveCount(0);
  await box.uncheck();
  await expect(page.locator('#toasts')).toContainText('1文字のショートカットを切りました。');
  // 一覧の表にも、切ってあることを出す。組み合わせのキーと Esc の行には出さない。
  await expect(mark(whole, 'ホームへ')).toBeVisible();
  await expect(mark(whole, 'この一覧')).toBeVisible();
  await expect(mark(whole, 'コマンド一覧')).toHaveCount(0);
  await expect(mark(whole, '閉じる、取り消す')).toHaveCount(0);
  await expect(mark(help.getByRole('table', { name: '仕訳帳のショートカット' }), '訂正する')).toBeVisible();
  await expect(mark(help.getByRole('table', { name: '未確認のショートカット' }), '無視する')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(help).toHaveCount(0);

  // 読み込み直しても、切ったまま（見出しにフォーカスを置いて、文字の欄の外で押す）。
  await page.reload();
  await expect(page.locator('main h1')).toBeVisible();
  await page.locator('main h1').focus();
  for (const key of ['n', 'g', 'j', '?', '/']) await page.keyboard.press(key);
  // ⌘K は開く（開いたことで、前に押したキーが処理し終わっていることも分かる）。
  await page.keyboard.press('ControlOrMeta+K');
  const palette = page.getByRole('dialog', { name: 'コマンド' });
  await expect(palette).toBeVisible();
  expect(new URL(page.url()).hash, '1文字のキーで画面が移っていない').toBe('#/');
  await expect(page.getByRole('dialog', { name: 'ショートカット' })).toHaveCount(0);

  // コマンド一覧から戻す。名前に、いまの状態が入る。
  await page.keyboard.insertText('いちもじ');
  await expect(palette.getByRole('option').first()).toHaveText('1文字のショートカットを切り替える（いまは切ってあります）');
  await page.keyboard.press('Enter');
  await expect(palette).toHaveCount(0);
  await expect(page.locator('#toasts')).toContainText('1文字のショートカットを使います。');
  await page.locator('main h1').focus();
  await page.keyboard.press('n');
  await expect(page).toHaveURL(/#\/entry$/);
});

test('AC-11.3 OSの設定に従い、手動でもライトとダークを変えられる', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await open(page, '#/');
  expect(await bodyBg(page)).toBe('rgb(16, 21, 27)');
  await page.emulateMedia({ colorScheme: 'light' });
  expect(await bodyBg(page)).toBe('rgb(246, 245, 241)');

  // 手動の切り替え（OS → ライト → ダーク）。OSがライトでも、ダークを選べばダークになる。
  const toggle = page.locator('.sidebar [data-action="theme"]');
  await toggle.click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await toggle.click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(await bodyBg(page)).toBe('rgb(16, 21, 27)');
  // 再読込しても、選んだ明暗が残る。
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await toggle.click();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /./);
});

for (const scheme of ['light', 'dark']) {
  test(`AC-10.3 AC-11.3 フォーカスの輪が見える（${scheme}、背景との差が3:1以上）`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await open(page, '#/entry');
    await page.locator('main h1').focus();
    await page.keyboard.press('Tab');
    const ring = await page.evaluate(() => {
      const el = document.activeElement;
      const s = getComputedStyle(el);
      let bg = 'rgba(0, 0, 0, 0)';
      for (let n = el.parentElement; n; n = n.parentElement) {
        const c = getComputedStyle(n).backgroundColor;
        if (c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent') {
          bg = c;
          break;
        }
      }
      return { style: s.outlineStyle, width: s.outlineWidth, color: s.outlineColor, bg, tag: el.tagName };
    });
    expect(ring.style).toBe('solid');
    expect(parseFloat(ring.width)).toBeGreaterThanOrEqual(2);
    expect(ratio(ring.color, ring.bg)).toBeGreaterThanOrEqual(3);
  });
}

test('AC-11.4 動きを減らす設定では、ダイアログの動きが止まる', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await open(page, '#/');
  await page.keyboard.press('?');
  const dur = await page.getByRole('dialog').evaluate((el) => getComputedStyle(el).animationDuration);
  expect(dur).toBe('0s');
  await page.keyboard.press('Escape');

  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.keyboard.press('?');
  const moving = await page.getByRole('dialog').evaluate((el) => getComputedStyle(el).animationDuration);
  expect(moving).not.toBe('0s');
});

test('768px未満では、下のバーの「メニュー」から全部の画面へ行ける', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/');
  await page.locator('.bottomnav').getByRole('button', { name: 'メニュー' }).click();
  const menu = page.getByRole('dialog', { name: 'メニュー' });
  await expect(menu).toBeVisible();
  await menu.getByRole('link', { name: '勘定科目' }).click();
  await expect(menu).toHaveCount(0);
  await expect(page).toHaveURL(/#\/accounts$/);
  await expect(page.locator('.bottomnav [aria-current="page"]')).toHaveCount(0);
  // 下のバーに無い画面にいる間は、「メニュー」を選んだ表示にする（今いる場所が分かるように）。
  const menuButton = page.locator('.bottomnav').getByRole('button', { name: 'メニュー' });
  await expect(menuButton).toHaveAttribute('aria-current', 'true');
  await page.locator('.bottomnav').getByRole('link', { name: '仕訳帳' }).click();
  await expect(page.locator('.bottomnav [aria-current="page"]')).toHaveText('仕訳帳');
  await expect(menuButton).not.toHaveAttribute('aria-current', /./);
  for (const [hash, inMenu] of [['#/reports/monthly', true], ['#/wallets', true], ['#/partners', true], ['#/settings', true], ['#/', false], ['#/entry', false], ['#/inbox', false]]) {
    await page.goto(`/${hash}`);
    await expect(page.locator('main h1')).toBeVisible();
    if (inMenu) await expect(menuButton, hash).toHaveAttribute('aria-current', 'true');
    else await expect(menuButton, hash).not.toHaveAttribute('aria-current', /./);
  }
});

test('768〜1023pxでは、細いサイドバーを広げて名前を全部出せる', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 900 });
  await open(page, '#/');
  const app = page.locator('#app');
  const toggle = page.locator('.rail-toggle');
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await expect(page.locator('.sidebar').getByText('取引を登録')).toBeHidden();
  await toggle.click();
  await expect(app).toHaveAttribute('data-rail', 'open');
  await expect(page.locator('.sidebar').getByText('取引を登録')).toBeVisible();
  await toggle.click();
  await expect(app).toHaveAttribute('data-rail', 'closed');
});

test('読み込みが300ms以上かかるときだけ骨組みを出す', async ({ page }) => {
  await page.route('**/api/v1/reports/dashboard*', async (route) => {
    await new Promise((r) => setTimeout(r, 900));
    await route.continue();
  });
  await open(page, '#/');
  await expect(page.locator('.skel-block').first()).toBeVisible();
  await expect(page.locator('.kpis')).toBeVisible({ timeout: 5000 });
  await expect(page.locator('.skel-block')).toHaveCount(0);
  await page.unroute('**/api/v1/reports/dashboard*');
  // すぐ終わる読み込みでは、骨組みは一度も出ない。
  await page.evaluate(() => {
    window.skelSeen = false;
    new MutationObserver(() => {
      if (document.querySelector('.skel-block')) window.skelSeen = true;
    }).observe(document.body, { childList: true, subtree: true });
  });
  await page.locator('.month-picker').getByRole('button', { name: '前の月' }).click();
  await expect(page.locator('.kpis')).toBeVisible();
  expect(await page.evaluate(() => window.skelSeen)).toBe(false);
});

test('デモのときは、画面の上に「デモのデータです」と常に出す', async ({ page }) => {
  await page.route('**/api/v1/status', async (route) => {
    const res = await route.fetch();
    const body = await res.json();
    body.mode = 'demo';
    await route.fulfill({ response: res, json: body });
  });
  await open(page, '#/journal');
  await expect(page.locator('#banners')).toContainText('デモのデータです');
});

test('本文へ移動のリンクが文書の先頭にあり、押すと本文の見出しへフォーカスが移る', async ({ page }) => {
  await open(page, '#/journal');
  const first = await page.evaluate(() => {
    const el = document.querySelector('a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])');
    return el && el.textContent.trim();
  });
  expect(first).toBe('本文へ移動');
  const skip = page.getByRole('link', { name: '本文へ移動' });
  await skip.focus();
  await expect(skip).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page.locator('main h1')).toBeFocused();
  await expect(page).toHaveURL(/#\/journal$/);
});

// AC-11.8 iPhone の下の余白（safe-area-inset-bottom）があっても、下に固定したバーは下のバーの真上に隙間なく載り、
// 通知も下のバーに重ならない（下のバーの高さは余白を含むので、--bar-bottom も同じ式。通知で余白を二重に足さない）。
// 余白は Chromium の開発者向けの命令（CDP）で擬似する。
test('AC-11.8 下の余白がある端末でも、固定バーは下のバーの真上に載り、通知は下のバーの上に出る', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/inbox');
  await expect(page.locator('.inbox-bar')).toBeVisible();
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Emulation.setSafeAreaInsetsOverride', { insets: { top: 0, left: 0, bottom: 34, right: 0 } });
  const m = await page.evaluate(() => {
    const nav = document.querySelector('.bottomnav').getBoundingClientRect();
    return {
      navTop: nav.top,
      navHeight: nav.height,
      barBottom: document.querySelector('.inbox-bar').getBoundingClientRect().bottom,
      toastsBottom: document.querySelector('.toasts').getBoundingClientRect().bottom,
    };
  });
  // 下のバーは 60px ＋ 余白34px。
  expect(m.navHeight).toBe(94);
  expect(Math.abs(m.barBottom - m.navTop), `固定バーの下の端 ${m.barBottom} と下のバーの上の端 ${m.navTop}`).toBeLessThanOrEqual(1);
  // 通知は、下のバーの12px上（余白を二重に足すと、さらに34px上に浮く）。
  expect(Math.round(m.navTop - m.toastsBottom)).toBe(12);
});

/** 通知の真ん中の点で、いちばん手前にある要素が、その通知の中か。小窓の四角の中に通知の真ん中があるかも返す。 */
function frontAt(notice, dialogSelector) {
  return notice.evaluate((el, sel) => {
    const r = el.getBoundingClientRect();
    const x = r.left + r.width / 2;
    const y = r.top + r.height / 2;
    const top = document.elementsFromPoint(x, y)[0];
    const d = document.querySelector(sel).getBoundingClientRect();
    return { front: el.contains(top), overDialog: x >= d.left && x <= d.right && y >= d.top && y <= d.bottom, top: top && `${top.tagName}.${top.className}` };
  }, dialogSelector);
}

// SC-05 の取消は、仕訳の詳細（引き出し）の中で確かめてから行う。取り消した知らせは、開いたままの引き出しより手前に出て、押せる。
// 置き場所: 幅1440は、引き出しの左の背景（引き出しの中身・履歴に重ねない）。幅390は引き出しが画面いっぱいなので、見出しの下に
// 重ねる（閉じるのボタンは隠さない）。どちらも、読めて押せる。
for (const [width, height] of [[1440, 900], [390, 844]]) {
  test(`SC-05 通知は、開いた引き出し（仕訳の詳細）の手前に出て、読めて押せる。中身と閉じるを隠さない（取消の知らせ・幅${width}）`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await open(page, '#/');
    const entry = await addCashExpense(page, { description: uniqueText('E2E 通知の前面') });
    await page.goto(`/#/journal/${entry.id}`);
    const drawer = page.getByRole('dialog', { name: `No.${entry.no} の仕訳` });
    await expect(drawer).toBeVisible();
    await drawer.getByRole('button', { name: '取り消す' }).click();
    const confirm = page.getByRole('dialog', { name: '仕訳を取り消しますか' });
    await confirm.getByLabel('取消の理由').fill('E2E 通知の前面');
    await confirm.getByRole('button', { name: '取り消す' }).click();
    await expect(confirm).toHaveCount(0);
    await expect(drawer).toBeVisible();
    const notice = page.locator('.toast').filter({ hasText: `No.${entry.no} を取り消しました` });
    await expect(notice).toBeVisible();
    const at = await frontAt(notice, 'dialog.drawer[open]');
    expect(at.front, `通知の点のいちばん手前が ${at.top}`).toBe(true);
    const [box, drawerBox, close] = await Promise.all([notice.boundingBox(), drawer.boundingBox(), drawer.locator('.dialog-head').getByRole('button', { name: '閉じる' }).boundingBox()]);
    expect(overlap(box, close), '引き出しの「閉じる」を隠さない').toBe(false);
    if (width >= 1024) expect(overlap(box, drawerBox), '広い幅では、引き出しの外（左の背景）に出す').toBe(false);
    else expect(at.overDialog, '狭い幅では、引き出しの上（見出しの下）に重ねる').toBe(true);
    // 入れ物はポップオーバーとして最前面に出ている（小窓の開く動きに引きずられない）。
    expect(await page.locator('#toasts').evaluate((el) => el.matches(':popover-open'))).toBe(true);
    // 押せる（覆われている・不活性だと、押す前に待ち切れずに落ちる）。
    await notice.getByRole('button', { name: 'この通知を閉じる' }).click({ timeout: 3000 });
    await expect(notice).toHaveCount(0);
    await expect(drawer).toBeVisible();
  });
}

/** 2つの四角（boundingBox の形）が重なるか。 */
function overlap(a, b) {
  return Boolean(a && b) && a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
}

/**
 * 通知と、フォーカスのある要素・上の帯・下に固定した帯（保存・確定）・下のバーの四角。見えない物は null。
 * 通知が出るときの動き（0.2秒、8px 下から上がる）が終わってから測る（その後の数秒間の位置を確かめるため）。
 */
async function aroundNotice(page, notice) {
  await notice.evaluate((el) => Promise.all(el.getAnimations().map((a) => a.finished)));
  return notice.evaluate((el) => {
    const box = (e) => {
      if (!e) return null;
      const r = e.getBoundingClientRect();
      return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== 'hidden' ? { x: r.left, y: r.top, width: r.width, height: r.height } : null;
    };
    const a = document.activeElement;
    const focusEl = a && a !== document.body ? (a.matches('input[type="checkbox"], input[type="radio"]') && a.closest('label')) || a : null;
    const n = box(el);
    const hit = (b) => b && n.x < b.x + b.width && b.x < n.x + n.width && n.y < b.y + b.height && b.y < n.y + n.height;
    return {
      notice: n,
      focus: box(focusEl),
      topbar: box(document.querySelector('.topbar')),
      bars: [...document.querySelectorAll('[data-bottom-bar], .bottomnav')].map(box).filter(Boolean),
      // 本文の押せる物のうち、通知に重なっているもの。
      controls: [...document.querySelectorAll('main a[href], main button, main input, main select, main textarea, main [role="tab"]')]
        .filter((c) => hit(box(c)))
        .map((c) => `${c.tagName.toLowerCase()}「${(c.getAttribute('aria-label') || c.textContent || c.placeholder || '').trim().slice(0, 12)}」`),
      vh: innerHeight,
    };
  });
}

// SC-02 SC-03 通知は、フォーカスのある欄・上の帯・保存と確定の帯・下のバーに重ならない。狭い幅は上の帯のすぐ下に出す
// （下に出すと、保存の帯のすぐ上の、これから入る欄や行に重なった）。広い幅は本文の上（押せる物があれば今までの右下）で、
// 本文の押せる物にも重ならない。
for (const [width, height] of [[390, 844], [768, 1024], [1440, 900]]) {
  test(`SC-02 保存して続けた知らせは、フォーカスのある欄・上の帯・保存の帯・下のバーに重ならない（幅${width}）`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await open(page, '#/entry');
    const f = entryFields(page);
    await f.amount.fill('1100');
    await f.account.click();
    await page.keyboard.insertText('つうしん');
    await page.keyboard.press('Enter');
    await expect(f.account).toHaveValue('通信費');
    await f.description.fill(uniqueText('E2E 通知の置き場所'));
    await f.description.focus();
    await page.keyboard.press('ControlOrMeta+Enter');
    const notice = page.locator('.toast').filter({ hasText: 'を登録しました' });
    await expect(notice).toBeVisible();
    await expect(f.amount).toBeFocused();
    const m = await aroundNotice(page, notice);
    expect(overlap(m.notice, m.focus), 'フォーカスのある金額の欄').toBe(false);
    expect(overlap(m.notice, m.topbar), '上の帯').toBe(false);
    for (const b of m.bars) expect(overlap(m.notice, b), '保存の帯・下のバー').toBe(false);
    if (width >= 768) expect(m.controls, '広い幅では、本文の押せる物に重ならない').toEqual([]);
    if (width < 768) {
      expect(m.notice.y, '狭い幅は上に出す').toBeLessThan(m.vh / 2);
      // 上に出した通知の下になった欄へ戻っても、通知のほうがどいて、欄は隠れない。
      await page.keyboard.press('Shift+Tab');
      await expect(f.date).toBeFocused();
      await expect.poll(async () => {
        const n = await aroundNotice(page, notice);
        return overlap(n.notice, n.focus);
      }).toBe(false);
      // 通知のボタンへフォーカスを移しても、通知は跳ねない（押そうとしている物が動かない）。
      const moved = await notice.boundingBox();
      await notice.getByRole('link', { name: '仕訳を見る' }).focus();
      await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
      expect(await notice.boundingBox()).toEqual(moved);
    }
  });

  test(`SC-03 無視した知らせは、フォーカスのある行・上の帯・確定の帯・下のバーに重ならない（幅${width}）`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await open(page, '#/inbox');
    const rows = page.locator('tr.inbox-row');
    await expect(rows.first()).toBeVisible();
    test.skip((await rows.count()) < 3, '未確認が少ないデータ');
    await rows.nth(1).locator('.cand-check').focus();
    await page.keyboard.press('x');
    const notice = page.locator('.toast').filter({ hasText: 'を無視しました' });
    await expect(notice).toBeVisible();
    const m = await aroundNotice(page, notice);
    expect(m.focus, '無視した後のフォーカスがある').not.toBeNull();
    expect(overlap(m.notice, m.focus), 'フォーカスのある行').toBe(false);
    expect(overlap(m.notice, m.topbar), '上の帯').toBe(false);
    for (const b of m.bars) expect(overlap(m.notice, b), '確定の帯・下のバー').toBe(false);
    if (width < 768) expect(m.notice.y, '狭い幅は上に出す（下に帯が3段重ならない）').toBeLessThan(m.vh / 2);
    // 1024px 以上は、見出しの横のボタンに重ならない本文の上に出せる（768px は見出しの横のボタンがあるので、今までの右下）。
    if (width >= 1024) expect(m.controls, '広い幅では、表の行の欄に重ならない').toEqual([]);
    // 元に戻す（ほかの試験の件数を変えない）。
    await page.keyboard.press('ControlOrMeta+Z');
    await expect(page.locator('.toast').filter({ hasText: '未確認に戻しました' })).toBeVisible();
  });
}

// SC-01 小窓（ショートカットの一覧）の中で切り替えた知らせは、切り替えた欄（フォーカス）と、小窓の見出しと「閉じる」を隠さない。
for (const [width, height] of [[390, 844], [1440, 900]]) {
  test(`SC-01 小窓の中で出た知らせは、フォーカスのある欄と、小窓の見出し・閉じるを隠さない（幅${width}）`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await open(page, '#/');
    await page.keyboard.press('?');
    const help = page.getByRole('dialog', { name: 'ショートカット' });
    const box = help.getByRole('checkbox', { name: '1文字のショートカットを使う' });
    await expect(box).toBeFocused();
    await page.keyboard.press('Space');
    const notice = page.locator('.toast').filter({ hasText: '1文字のショートカットを切りました' });
    await expect(notice).toBeVisible();
    const m = await aroundNotice(page, notice);
    expect(overlap(m.notice, m.focus), '切り替えた欄').toBe(false);
    for (const b of [help.locator('.dialog-head'), help.locator('.dialog-foot')]) expect(overlap(m.notice, await b.boundingBox())).toBe(false);
    expect((await frontAt(notice, 'dialog[open]')).front).toBe(true);
    await page.keyboard.press('Space');
    await expect(box).toBeChecked();
  });
}

// 通知が出たあとで小窓を開いても、通知は小窓の手前に出し直される（狭い幅の全項目のメニューと、ショートカットの一覧）。
test('SC-01 通知が出たあとで小窓（メニュー・ショートカットの一覧）を開いても、通知は小窓の手前にあり、押せる', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/');
  await page.locator('.topbar [data-action="theme"]').click();
  const notice = page.locator('.toast').filter({ hasText: '表示を「' });
  await expect(notice).toBeVisible();
  await page.locator('.bottomnav').getByRole('button', { name: 'メニュー' }).click();
  await expect(page.getByRole('dialog', { name: 'メニュー' })).toBeVisible();
  let at = await frontAt(notice, 'dialog.menu-drawer[open]');
  expect(at.overDialog).toBe(true);
  expect(at.front, `通知の点のいちばん手前が ${at.top}`).toBe(true);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'メニュー' })).toHaveCount(0);

  await page.setViewportSize({ width: 1440, height: 900 });
  await expect(notice).toBeVisible();
  await page.keyboard.press('?');
  const help = page.getByRole('dialog', { name: 'ショートカット' });
  await expect(help).toBeVisible();
  // ショートカットの一覧は真ん中の小窓。通知（右下）と重ならないこともあるので、手前にあることと押せることだけを見る。
  at = await frontAt(notice, 'dialog[open]');
  expect(at.front, `通知の点のいちばん手前が ${at.top}`).toBe(true);
  await notice.getByRole('button', { name: 'この通知を閉じる' }).click({ timeout: 3000 });
  await expect(notice).toHaveCount(0);
  await expect(help).toBeVisible();
});

// 表示の切り替えを続けて押しても、知らせは積み重ならず、最後の1つだけが残る（読み上げの領域に前の文を残さない）。
test('AC-11.3 表示の切り替えの知らせは、前の知らせを置き換える', async ({ page }) => {
  await open(page, '#/');
  const toggle = page.locator('.sidebar [data-action="theme"]');
  for (let i = 0; i < 3; i += 1) await toggle.click();
  const notices = page.locator('#toasts .toast');
  await expect(notices).toHaveCount(1);
  await expect(notices.first()).toContainText('表示を「OSに合わせる」にしました。');
  // 元に戻す（OS → ライト → ダーク → OS の順で、3回で元の OS に戻っている）。
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /./);
});

// 幅390の下のバーの「未確認」にも、サイドバーと同じ件数（同じ数字と、読み上げの名前）を、アイコンの右上に出す。
test('SC-03 幅390の下のバーの「未確認」にも、サイドバーと同じ件数と読み上げの名前を出す', async ({ page }) => {
  await open(page, '#/');
  const n = (await apiCall(page, 'GET', '/status')).json.inbox.draft;
  test.skip(!n, '未確認が無いデータ');
  const name = new RegExp(`^未確認 ?${n}件$`);
  await expect(page.locator('.sidebar').getByRole('link', { name })).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  const tab = page.locator('.bottomnav').getByRole('link', { name });
  await expect(tab).toBeVisible();
  await expect(tab.locator('.nav-count')).toHaveText(String(n));
  // 印はタブの中に収まり、アイコンの右上（アイコンと重なる高さ）にある。
  const [box, badge, ico] = await Promise.all([tab.boundingBox(), tab.locator('.nav-count').boundingBox(), tab.locator('svg').boundingBox()]);
  expect(badge.x).toBeGreaterThanOrEqual(box.x);
  expect(badge.x + badge.width).toBeLessThanOrEqual(box.x + box.width);
  expect(badge.y).toBeLessThan(ico.y + ico.height);
  expect(badge.x).toBeGreaterThan(ico.x);
});

// AC-10.2 キーの表示（kbd）は、ボタンやリンクの読み上げの名前に混ぜない。キーは aria-keyshortcuts で伝える（「g i」のように
// 続けて押すキーは表せないので付けない）。必須は欄の aria-required で伝え、見える「必須」の札は名前に入れない。
// 共通の部品（fields.js）を画面に置いて確かめ、実際の画面（ホーム・サイドバー）でも確かめる。
test('AC-10.2 ボタンとリンクの読み上げの名前にキーの文字が混ざらず、キーは aria-keyshortcuts で伝える。「必須」は名前に入れない', async ({ page }) => {
  await open(page, '#/');
  await page.evaluate(async () => {
    const { button, field } = await import('/js/components/fields.js');
    const host = document.createElement('div');
    host.id = 'e2e-parts';
    const input = document.createElement('input');
    host.append(button({ label: '保存して続ける', kbd: '⌘ Enter' }), button({ label: 'やめる', kbd: 'Esc' }), button({ label: '未確認を開く', kbd: 'g i' }), field({ label: '日付', control: input, required: true }).el);
    document.querySelector('main').append(host);
  });
  const parts = page.locator('#e2e-parts');
  await expect(parts.getByRole('button', { name: '保存して続ける', exact: true })).toHaveAttribute('aria-keyshortcuts', 'Meta+Enter');
  await expect(parts.getByRole('button', { name: 'やめる', exact: true })).toHaveAttribute('aria-keyshortcuts', 'Escape');
  await expect(parts.getByRole('button', { name: '未確認を開く', exact: true })).not.toHaveAttribute('aria-keyshortcuts', /./);
  await expect(parts.locator('kbd').first()).toBeVisible();
  await expect(parts.getByRole('textbox', { name: '日付', exact: true })).toHaveAttribute('aria-required', 'true');
  await expect(parts.locator('.req')).toHaveText('必須');

  await page.reload();
  await expect(page.locator('.next-card')).toBeVisible();
  await expect(page.locator('.page-head').getByRole('link', { name: '取引を登録', exact: true })).toHaveAttribute('aria-keyshortcuts', 'n');
  await expect(page.locator('.next-card').getByRole('link', { name: /^(未確認を開く|帳票を開く|取引を登録)$/ }).first()).toBeVisible();
  const side = page.locator('.sidebar');
  await expect(side.getByRole('button', { name: 'コマンド', exact: true })).toHaveAttribute('aria-keyshortcuts', /^(Meta|Control)\+K$/);
  await expect(side.getByRole('button', { name: 'ショートカット', exact: true })).toHaveAttribute('aria-keyshortcuts', '?');
  await expect(side.getByRole('link', { name: '取引を登録', exact: true })).toHaveAttribute('aria-keyshortcuts', 'n');
  await expect(side.getByRole('link', { name: '仕訳帳', exact: true })).not.toHaveAttribute('aria-keyshortcuts', /./);
});

/** 読み上げの木（Chromium の CDP）で、name の文の節の id。無ければ null。 */
async function axTextId(cdp, name) {
  const { nodes } = await cdp.send('Accessibility.getFullAXTree');
  const n = nodes.find((x) => !x.ignored && x.role && x.role.value === 'StaticText' && x.name && x.name.value === name);
  return n ? n.nodeId : null;
}

// SC-02（WCAG 4.1.3）欄を出た時点の誤りは、空のうちから在る読み上げの領域（aria-live="polite"）に出る。空の入れ物は見えない形で
// 置き（表示しない入れ物に文を入れても読まれないため）、場所は取らない。同じ文を入れ直しても、読み上げの木の節は作り直さない。
test('AC-10.6 欄を出た時点の誤りは、空のうちから在る読み上げの領域に出る。消して同じ文を入れ直しても、読み上げの木の節は同じ', async ({ page }) => {
  await open(page, '#/entry');
  const amount = page.locator('form.entry-form').getByLabel('金額');
  await expect(amount).toBeVisible();
  const errId = `${await amount.getAttribute('id')}-err`;
  const region = page.locator(`[id="${errId}"]`);
  // 空のうちから: 読み上げの領域で、表示はされている（display: none でない）が、見えず、場所も取らない。
  await expect(region).toHaveAttribute('aria-live', 'polite');
  const empty = await region.evaluate((el) => {
    const s = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    return { display: s.display, position: s.position, w: r.width, h: r.height };
  });
  expect(empty.display).not.toBe('none');
  expect(empty.position).toBe('absolute');
  expect(empty.w * empty.h).toBeLessThanOrEqual(1);

  await amount.fill('12a3');
  await page.keyboard.press('Tab');
  await expect(region).toContainText('使えない文字「a」があります');
  await expect(region).toBeVisible();
  await expect(amount).toHaveAttribute('aria-invalid', 'true');
  await expect(amount).not.toBeFocused();
  const box = await region.boundingBox();
  expect(box.height, '誤りが出たら、ふつうの大きさで見える').toBeGreaterThan(10);

  // もう一度欄を出ても（同じ文）、要素は何も変わらない。
  await region.evaluate((el) => {
    window.e2eMutations = [];
    new MutationObserver((list) => window.e2eMutations.push(...list.map((m) => m.type))).observe(el, { childList: true, subtree: true, characterData: true });
  });
  await amount.focus();
  await page.keyboard.press('Tab');
  await expect(amount).not.toBeFocused();
  expect(await page.evaluate(() => window.e2eMutations)).toEqual([]);

  // 保存の検査のように、消してすぐ同じ文を入れ直しても、読み上げの木の文の節は同じもの（作り直すと、新しい文として読み直される）。
  // 部品（fields.js）をその場に置いて確かめる。
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Accessibility.enable');
  await page.evaluate(async () => {
    const { field } = await import('/js/components/fields.js');
    const input = document.createElement('input');
    const f = field({ label: 'E2E の欄', control: input });
    f.el.id = 'e2e-field';
    document.querySelector('main').append(f.el);
    f.setError('E2E の誤りの文です。');
    window.e2eField = f;
  });
  const before = await axTextId(cdp, 'E2E の誤りの文です。');
  expect(before, '誤りの文が読み上げの木にある').not.toBeNull();
  await page.evaluate(() => {
    window.e2eField.setError('');
    window.e2eField.setError('E2E の誤りの文です。');
  });
  expect(await axTextId(cdp, 'E2E の誤りの文です。')).toBe(before);
  // 消したままなら、文は読み上げの木から消え、入れ物は見えない形に戻る。
  await page.evaluate(() => window.e2eField.setError(''));
  await expect.poll(() => axTextId(cdp, 'E2E の誤りの文です。')).toBeNull();
  await expect(page.locator('#e2e-field .field-error')).toHaveAttribute('aria-live', 'polite');
  expect(await page.locator('#e2e-field .field-error').evaluate((el) => getComputedStyle(el).position)).toBe('absolute');
});

// SC-01 グラフ（ホーム）: 「表で見る」は名前だけで状態を伝える（押下の状態 aria-pressed を付けない。名前と一緒に変わると
// 「グラフで見る、押されている」と読まれた）。凡例は名前だけで、見分け方は下の1行。赤字の月には額を添え、額どうしは重ならない。
for (const [width, height] of [[390, 844], [1440, 900]]) {
  test(`SC-01 グラフの切り替えは名前だけで伝え、凡例は名前だけ。赤字の月の額は重ならない（幅${width}）`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await open(page, '#/');
    const chart = page.locator('.chart').first();
    await expect(chart.locator('.chart-svg')).toBeVisible();
    const toggle = chart.getByRole('button', { name: '表で見る', exact: true });
    await expect(toggle).not.toHaveAttribute('aria-pressed', /./);
    await expect(chart.locator('.chart-legend li')).toHaveText(['売上', '経費', '利益', '選んだ月']);
    await expect(chart.locator('.chart-note')).toHaveText(/^塗りの棒が売上、斜線の棒が経費、線が利益です。赤字の月は、点を塗って額を添えます。/);
    const m = await chart.locator('.chart-svg').evaluate((svg) => {
      const box = svg.getBoundingClientRect();
      const labels = [...svg.querySelectorAll('.chart-loss-label')].map((t) => ({ text: t.textContent, r: t.getBoundingClientRect() }));
      const hit = (a, b) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
      // 額は、棒・点・月の名前の上に載らない（斜線の棒の上に載ると読めなかった。2026-10-15 の見た目の採点、幅390・768）。
      const marks = [...svg.querySelectorAll('.chart-bar--revenue, .chart-bar--expense, .chart-dot, .chart-label')].map((e) => e.getBoundingClientRect());
      return {
        losses: svg.querySelectorAll('.chart-dot--loss').length,
        texts: labels.map((l) => l.text),
        overlaps: labels.some((a, i) => labels.some((b, j) => i < j && hit(a.r, b.r))),
        outside: labels.some((l) => l.r.left < box.left || l.r.right > box.right),
        onMarks: labels.some((l) => marks.some((r) => hit(l.r, r))),
      };
    });
    expect(m.losses, 'デモの帳簿には赤字の月がある').toBeGreaterThan(0);
    expect(m.texts.length, '赤字の月ごとに額を添える（隣り合う月は段をずらす）').toBe(m.losses);
    for (const t of m.texts) expect(t).toMatch(/^-[\d,.]+(万|億)?$/);
    expect(m.overlaps, '額どうしが重ならない').toBe(false);
    expect(m.outside, '額がグラフの外にはみ出さない').toBe(false);
    expect(m.onMarks, '額が棒・点・月の名前の上に載らない').toBe(false);
    await toggle.click();
    const back = chart.getByRole('button', { name: 'グラフで見る', exact: true });
    await expect(back).toBeVisible();
    await expect(back).not.toHaveAttribute('aria-pressed', /./);
    await expect(chart.locator('.chart-table table')).toBeVisible();
  });
}

// E4 幅390の上の帯の「表示」の名前に、いまの表示を入れる（サイドバーと同じ「表示: …」。見える名前「表示」で始まる）。
test('SC-01 幅390の上の帯の「表示」の名前に、いまの表示が入り、切り替えると変わる', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/');
  const theme = page.locator('.topbar [data-action="theme"]');
  await expect(theme).toHaveAccessibleName('表示: OSに合わせる');
  await theme.click();
  await expect(theme).toHaveAccessibleName('表示: ライト');
  await theme.click();
  await expect(theme).toHaveAccessibleName('表示: ダーク');
  await theme.click();
  await expect(theme).toHaveAccessibleName('表示: OSに合わせる');
});

// AC-11.6 表の入れ物は、横にスクロールできるときだけ Tab の止まり場所になる（横に動かない入れ物で、Tab が1つ余計に要った）。
test('AC-11.6 表の入れ物は、横にスクロールできるときだけ Tab で止まる', async ({ page }) => {
  await open(page, '#/accounts');
  await expect(page.locator('.accounts-table').first()).toBeVisible();
  const wraps = page.locator('main .table-wrap');
  expect(await wraps.count()).toBeGreaterThan(0);
  await expect.poll(() => wraps.evaluateAll((els) => els.filter((e) => e.hasAttribute('tabindex')).length)).toBe(0);
  // 部品そのもの: 中が広くて横に動くときは止まり、狭くなって動かなくなったら止まらない。
  await page.evaluate(async () => {
    const { focusWhenScrollable } = await import('/js/components/table.js');
    const wrap = document.createElement('div');
    wrap.className = 'table-wrap';
    wrap.id = 'e2e-wrap';
    const inner = document.createElement('div');
    inner.id = 'e2e-inner';
    inner.style.width = '3000px';
    inner.style.height = '10px';
    wrap.append(inner);
    document.querySelector('main').append(wrap);
    focusWhenScrollable(wrap, inner);
  });
  await expect(page.locator('#e2e-wrap')).toHaveAttribute('tabindex', '0');
  await page.evaluate(() => document.getElementById('e2e-inner').style.setProperty('width', '10px'));
  await expect(page.locator('#e2e-wrap')).not.toHaveAttribute('tabindex', /./);
});

// AC-10.2 ショートカットの一覧の「未確認」は、画面の言葉（確定する行・無視）と動き（税区分の ↑↓、検索の欄の Esc、⌘Z で戻す）に合わせる。
test('AC-10.2 ショートカットの一覧の未確認の欄は、「確定する行」「無視する」と、⌘Z（Ctrl+Z）で戻すことを書く', async ({ page }) => {
  await open(page, '#/');
  await page.keyboard.press('?');
  const table = page.getByRole('table', { name: '未確認のショートカット' });
  await expect(table).toBeVisible();
  const row = (key) => table.locator('tbody tr').filter({ has: page.locator('th', { hasText: key }) });
  await expect(row('x')).toContainText('無視する');
  await expect(table.locator('tbody tr').filter({ hasText: '最後に無視した行を戻す' }).locator('th')).toHaveText(/^(⌘|Ctrl) Z$/);
  await expect(table).toContainText('税区分の欄では、行を動かさずに税区分を切り替える');
  await expect(table).toContainText('検索の欄では、欄から抜ける');
  await expect(table).toContainText('確定する行を確定する');
  await expect(table).not.toContainText('確定候補');
  await expect(table).not.toContainText('取り込まない');
  // 表の並びは今までどおり（Enter・確定・↑↓・Esc・x・戻す・Shift Space）。
  await expect(table.locator('tbody th')).toHaveCount(7);
  await page.keyboard.press('Escape');
});
