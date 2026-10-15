// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引を登録（SC-02）の並び。幅1280px未満は、保存の帯を画面の下（768px未満は下のバーの真上）に付ける（AC-11.8 と同じ決まり）。
// 640px未満のプレビューは1行の要約で、「くわしく」で表を開く。1280px以上は、右の列にプレビューと保存を重ねる。
// 帯を足しても、キーボードの流れ（Ctrl/Cmd+Enter、最後の欄の Enter、Tab の順）は変えない。

import { test, expect } from '@playwright/test';
import { open, entryFields, uniqueText } from './helpers.mjs';

/** 金額・科目・口座・摘要を入れて、プレビューの表ができるまで待つ。 */
async function fill(page, desc = '10月分 携帯電話') {
  const f = entryFields(page);
  await f.amount.fill('11000');
  await f.account.click();
  await page.keyboard.insertText('通信費');
  await page.keyboard.press('Enter');
  await f.wallet.click();
  await page.keyboard.insertText('サンプルカード');
  await page.keyboard.press('Enter');
  await f.description.fill(desc);
  await f.description.blur();
  await expect(f.preview.locator('table')).toBeAttached();
  return f;
}

/** 帯の下の端と、その下（下のバーか画面の端）の隙間。隙間に見えている要素も返す。 */
const barGap = (page) =>
  page.evaluate(() => {
    const bar = document.querySelector('.entry-actions-card');
    const r = bar.getBoundingClientRect();
    const nav = document.querySelector('.bottomnav');
    const navBox = nav && nav.getBoundingClientRect().width > 0 ? nav.getBoundingClientRect() : null;
    const floor = navBox ? navBox.top : window.innerHeight;
    const gap = floor - r.bottom;
    const seen = gap >= 1 ? document.elementFromPoint(r.left + r.width / 2, r.bottom + 0.5) : null;
    return { gap: Math.round(gap * 10) / 10, inView: r.top >= 0 && r.bottom <= window.innerHeight + 0.5, seen: seen ? String(seen.className || seen.tagName) : null, bg: getComputedStyle(bar).backgroundColor };
  });

for (const width of [390, 768]) {
  test(`AC-11.8 取引を登録（幅${width}）: 保存の帯は下の端にぴったり付き、帯の下から本文がのぞかない。下まで送っても、欄に移っても`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await open(page, '#/entry');
    await fill(page);
    const bar = page.locator('.entry-actions-card');
    await expect(bar.getByRole('button', { name: /保存して続ける/ })).toBeInViewport();
    const top = await barGap(page);
    expect(top.inView).toBe(true);
    expect(Math.abs(top.gap), `帯の下に ${top.gap}px の隙間（${top.seen}）`).toBeLessThan(1);
    expect(top.bg).not.toMatch(/rgba\(\d+, \d+, \d+, 0\)|transparent/);
    // 本文のいちばん下まで送っても、帯と下のバー（画面の端）の間に、本文の余白が出ない。
    await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
    const bottom = await barGap(page);
    expect(Math.abs(bottom.gap), `下まで送ると、帯の下に ${bottom.gap}px の隙間（${bottom.seen}）`).toBeLessThan(1);
    // 画面の外にある最後の欄（事業の割合）へ移っても、欄は帯の裏に隠れない。
    await page.evaluate(() => window.scrollTo(0, 0));
    await entryFields(page).pct.focus();
    const [fieldBottom, barTop] = await page.evaluate(() => [document.activeElement.getBoundingClientRect().bottom, document.querySelector('.entry-actions-card').getBoundingClientRect().top]);
    expect(fieldBottom).toBeLessThanOrEqual(barTop);
  });
}

test('SC-02 幅390: プレビューは1行の要約（借方と貸方）で、「くわしく」で表を開ける', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/entry');
  const f = await fill(page);
  const summary = f.preview.locator('.pv-summary');
  await expect(summary).toBeVisible();
  await expect(summary).toHaveText(/借方\s*通信費\s*11,000\s*／\s*貸方\s*サンプルカード\s*11,000/);
  await expect(f.preview.locator('table')).toBeHidden();
  const toggle = f.preview.getByRole('button', { name: 'くわしく' });
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
  await expect(f.preview.locator('table')).toBeVisible();
  await expect(summary).toBeHidden();
});

// 保存の組（やめる・保存・保存して続ける）を、最後の欄のすぐ後の Tab の順にする。「テンプレートに登録」はその後ろ（2026-10-15 の体験の評価 E4 の20）。
test('SC-02 幅390: キーボードだけで保存できる。最後の欄の後の Tab は、くわしく → やめる → 保存 → 保存して続ける → テンプレートに登録', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/entry');
  const f = await fill(page, uniqueText('E2E 帯の上で保存'));
  await f.pct.focus();
  const names = [];
  for (let i = 0; i < 5; i += 1) {
    await page.keyboard.press('Tab');
    // 見えている文字（狭い幅ではキーの手がかりを出さない）。
    names.push(await page.evaluate(() => document.activeElement.innerText.trim().replace(/\s+/g, ' ')));
  }
  expect(names).toEqual(['くわしく', 'やめる', '保存', '保存して続ける', 'テンプレートに登録']);
  // 帯があっても、Ctrl/Cmd+Enter で保存し、金額へ戻る（AC-02.5）。
  await f.description.focus();
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  await expect(f.amount).toBeFocused();
  await expect(f.amount).toHaveValue('');
});

test('SC-02 幅1440: 保存のボタンは、右の列のプレビューのすぐ下にあり、プレビューと一緒に画面の上に付いてくる', async ({ page }) => {
  await open(page, '#/entry');
  const f = await fill(page);
  const pv = await f.preview.boundingBox();
  const bar = await page.locator('.entry-actions-card').boundingBox();
  const main = await page.locator('.entry-main').boundingBox();
  expect(bar.x).toBeGreaterThanOrEqual(main.x + main.width);
  expect(Math.abs(bar.x - pv.x)).toBeLessThan(1);
  expect(bar.y).toBeGreaterThanOrEqual(pv.y + pv.height);
  expect(bar.y - (pv.y + pv.height)).toBeLessThan(40);
  // 主な操作（保存して続ける）は、やめる・保存の下の段の幅いっぱい（並び＝Tabの順と同じ読む順）。
  const actions = page.locator('.entry-actions-card');
  const [cancel, save, cont] = await Promise.all([
    actions.getByRole('button', { name: /^やめる/ }).boundingBox(),
    actions.getByRole('button', { name: '保存', exact: true }).boundingBox(),
    actions.getByRole('button', { name: /^保存して続ける/ }).boundingBox(),
  ]);
  expect(cont.y).toBeGreaterThan(Math.max(cancel.y, save.y));
  expect(cont.width).toBeGreaterThan(save.width + cancel.width);
});

for (const width of [390, 1440]) {
  test(`SC-02 幅${width}: 保存直後の通知が保存の帯に重ならない`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 });
    await open(page, '#/entry');
    const f = await fill(page, uniqueText(`E2E 保存通知${width}`));
    await expect(f.description).toHaveAttribute('enterkeyhint', 'next');
    await expect(f.pct).toHaveAttribute('enterkeyhint', 'done');
    const bar = page.locator('.entry-actions-card');
    await bar.getByRole('button', { name: /^保存して続ける/ }).click();
    const notice = page.locator('.toast').filter({ hasText: 'を登録しました' }).last();
    await expect(notice).toBeVisible();
    const [a, b] = await Promise.all([notice.boundingBox(), bar.boundingBox()]);
    expect(a).not.toBeNull();
    expect(b).not.toBeNull();
    const overlaps = a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
    expect(overlaps).toBe(false);
    if (width === 390) expect(a.y + a.height).toBeLessThanOrEqual(b.y);
    else await expect(page.locator('#toasts')).toHaveCSS('--toast-lift', '0px');
  });
}

test('SC-02 幅390: 下の方から「保存して続ける」と、金額の欄（と欄の名前、引き継いだ日付）が上の帯に潜らない', async ({ page }) => {
  // 高さを低くして、下の方から戻ったときに、ページが金額の欄を上の端まで送れる状態にする。
  await page.setViewportSize({ width: 390, height: 460 });
  await open(page, '#/entry');
  const f = await fill(page, uniqueText('E2E 帯に潜らない'));
  await f.pct.focus();
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  await expect(f.amount).toBeFocused();
  const m = await page.evaluate(() => {
    const top = document.querySelector('.topbar').getBoundingClientRect().bottom;
    const field = (el) => el.closest('.field').querySelector('.field-label').getBoundingClientRect().top;
    const amount = document.activeElement;
    const date = document.querySelector('form.entry-form .field .input');
    return { top, amountLabel: field(amount), amountInput: amount.getBoundingClientRect().top, dateLabel: field(date) };
  });
  expect(m.amountInput).toBeGreaterThanOrEqual(m.top);
  expect(m.amountLabel).toBeGreaterThanOrEqual(m.top);
  expect(m.dateLabel).toBeGreaterThanOrEqual(m.top);
});

test('SC-02 幅1440: 最後の欄の後の Tab は、やめる → 保存 → 保存して続ける → テンプレートに登録。テンプレートに登録の見た目は、今までどおりプレビューの枠の右下', async ({ page }) => {
  await open(page, '#/entry');
  const f = await fill(page);
  await f.pct.focus();
  const names = [];
  for (let i = 0; i < 4; i += 1) {
    await page.keyboard.press('Tab');
    names.push(await page.evaluate(() => document.activeElement.textContent.trim().replace(/\s+/g, ' ')));
  }
  expect(names.map((n) => n.replace(/\s*(Esc|⌘ Enter|Ctrl Enter)$/, ''))).toEqual(['やめる', '保存', '保存して続ける', 'テンプレートに登録']);
  // 見た目: テンプレートに登録は、プレビューの枠の中（右下）にあり、表より下・保存の帯より上。
  const [card, table, tpl, bar] = await Promise.all([
    f.preview.boundingBox(),
    f.preview.locator('.pv-wrap').boundingBox(),
    page.getByRole('button', { name: 'テンプレートに登録' }).boundingBox(),
    page.locator('.entry-actions-card').boundingBox(),
  ]);
  expect(tpl.x).toBeGreaterThanOrEqual(card.x);
  expect(tpl.x + tpl.width).toBeLessThanOrEqual(card.x + card.width);
  expect(tpl.y).toBeGreaterThanOrEqual(table.y + table.height);
  expect(tpl.y + tpl.height).toBeLessThanOrEqual(card.y + card.height);
  expect(tpl.y + tpl.height).toBeLessThan(bar.y);
  // 右の端は、表の入れ物（枠の内側の幅いっぱい）の右の端にそろう（今までと同じ位置）。
  expect(Math.abs(tpl.x + tpl.width - (table.x + table.width))).toBeLessThan(2);
});

test('SC-02 幅390: プレビューの枠は要約のすぐ下で終わり、「テンプレートに登録」は要約のすぐ下（保存の帯より上）にある', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/entry');
  const f = await fill(page);
  const summary = f.preview.locator('.pv-summary');
  await expect(summary).toBeVisible();
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  const [s, card, tpl, bar] = await Promise.all([
    summary.boundingBox(),
    f.preview.boundingBox(),
    page.getByRole('button', { name: 'テンプレートに登録' }).boundingBox(),
    page.locator('.entry-actions-card').boundingBox(),
  ]);
  // 枠の中の要約の下に、空白を残さない（畳んだ表の入れ物と間隔の分が残っていた）。
  expect(card.y + card.height - (s.y + s.height)).toBeLessThan(8);
  // テンプレートに登録は、要約から 32px 以内の下にあり、保存の帯より上。
  expect(tpl.y).toBeGreaterThanOrEqual(s.y + s.height);
  expect(tpl.y - (s.y + s.height)).toBeLessThan(32);
  expect(tpl.y + tpl.height).toBeLessThanOrEqual(bar.y);
  // 見出しの右の「テンプレートから入力」は、見出しの下の行にある。
  const [h1, from] = await Promise.all([page.locator('main h1').boundingBox(), page.getByRole('button', { name: 'テンプレートから入力' }).boundingBox()]);
  expect(from.y).toBeGreaterThanOrEqual(h1.y + h1.height);
});

// 仕訳で入力の明細: 借方か貸方・税区分の選択欄に、選んだ文字（いちばん長い選択肢）が全部入る。2026-10-15 の体験の評価 B7（幅1440で「借方」と「課税仕」が切れた）。
for (const width of [390, 768, 1440]) {
  test(`SC-02 仕訳で入力（幅${width}）: 選択欄の文字が切れない。いちばん長い税区分は、入らない幅（390）では欄の下に全部出す`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await open(page, '#/entry');
    await page.getByRole('tab', { name: '仕訳で入力' }).click();
    const rows = page.locator('form.entry-form .jl-table tbody tr');
    await expect(rows).toHaveCount(2);
    await rows.nth(0).getByRole('combobox', { name: '科目' }).click();
    await page.keyboard.insertText('通信費');
    await page.keyboard.press('Enter');
    await rows.nth(0).getByLabel('税区分').selectOption('P10X');
    await rows.nth(1).getByLabel('借方か貸方').selectOption('credit');
    const fits = await page.evaluate(() => {
      const ctx = document.createElement('canvas').getContext('2d');
      return [...document.querySelectorAll('.jl-table select')].map((sel) => {
        const cs = getComputedStyle(sel);
        ctx.font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
        const room = sel.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
        const longest = Math.max(...[...sel.options].map((o) => ctx.measureText(o.label).width));
        return { name: sel.getAttribute('aria-label'), shown: sel.selectedOptions[0].label, fit: ctx.measureText(sel.selectedOptions[0].label).width <= room + 0.5, longestFits: longest <= room + 0.5 };
      });
    });
    const wrap = await page.locator('.jl-wrap').evaluate((el) => el.scrollWidth - el.clientWidth);
    expect(wrap, '明細の表が横にはみ出さない').toBeLessThanOrEqual(1);
    for (const s of fits.filter((x) => /借方か貸方/.test(x.name))) expect(s.longestFits, `${s.name}`).toBe(true);
    const tax = fits.filter((x) => /税区分/.test(x.name));
    const full = rows.nth(0).locator('.jl-tax-full');
    if (width >= 640) {
      for (const s of tax) expect(s.longestFits, `${s.name}（${s.shown}）`).toBe(true);
      await expect(full).toBeHidden();
    } else {
      // 幅390では、経過措置の名前は欄に入らない（16pxの文字で約355px要る）。欄の下に全部出す。ほかの選択肢は欄に入る。
      expect(tax[0].fit).toBe(false);
      await expect(full).toBeVisible();
      await expect(full).toHaveText('経過措置の仕入10%（免税事業者等から）');
      expect(tax[1].fit).toBe(true);
      await expect(rows.nth(1).locator('.jl-tax-full')).toBeHidden();
    }
  });
}

test('SC-02 入力欄の補足（事業の割合の説明、取引先の足し方）と事業主貸の額は、本文の色で出す', async ({ page }) => {
  await open(page, '#/entry');
  const text = await page.evaluate(() => getComputedStyle(document.querySelector('main h1')).color);
  const colors = await page.locator('form.entry-form .field-hint').evaluateAll((els) => els.filter((el) => el.getClientRects().length).map((el) => [el.textContent.trim(), getComputedStyle(el).color]));
  expect(colors.map(([t]) => t).join(' ')).toContain('残りは事業主貸に入ります');
  for (const [t, c] of colors) expect(c, t).toBe(text);
  await expect(page.locator('form.entry-form .pct-out')).toHaveCSS('color', text);
});
