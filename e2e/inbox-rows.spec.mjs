// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認（SC-03）の行の直し（2026-10-15 の体験の評価への対応 g2）: 科目を選び直したときの税区分、向きと逆の科目の注意、
// 「科目のある行をすべて選ぶ」、行の読み上げの名前、下の帯の読み上げ、狭い幅の帯、印の列の見出しと表の名前。
// 各試験は、自分の名前（uniqueText）を摘要に入れた行を取り込み、その名前で絞って使う（ほかの試験と混ざらない）。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, walletOf, importRows, withAccountDefaults } from './helpers.mjs';

const rowsOf = (page) => page.locator('tr.inbox-row');
const accountOf = (row) => row.locator('td.c-account input[role="combobox"]');
const taxOf = (row) => row.locator('td.c-tax select');
const msgOf = async (page, row) => page.locator(`#draft-${await row.getAttribute('data-id')}-msg`);

/**
 * 行を取り込み、その名前で絞った未確認を開く。specs は [摘要の後ろ, 出金, 入金, 日付（省略可）]。account があれば、全部の行に入れる。
 * 1月の日付にする（ほかの試験が10月に作る仕訳を、仕訳帳の1ページ目から押し出さないように）。
 */
async function prepare(page, prefix, specs, { account = null } = {}) {
  await open(page, '#/');
  const token = uniqueText(prefix).replace(/\s+/g, '-');
  const card = await walletOf(page, 'card');
  const rows = specs.map(([suffix, out, inn, date], i) => [date || `2026-01-${String(i + 1).padStart(2, '0')}`, `${token} ${suffix}`.trim(), out, inn]);
  await importRows(page, card.code, rows);
  if (account) {
    const items = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}&limit=100`)).json.items;
    for (const d of items) await apiCall(page, 'PATCH', `/drafts/${d.id}`, { account });
  }
  await open(page, `#/inbox?q=${encodeURIComponent(token)}`);
  await expect(rowsOf(page)).toHaveCount(specs.length);
  return token;
}

/** 科目の欄に読みを打ち、候補を押して選ぶ。 */
async function pick(page, row, reading, name) {
  await accountOf(row).click();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText(reading);
  await page.getByRole('option', { name: new RegExp(name) }).first().click();
}

test('SC-03 g2-1 科目を選び直すと、税区分が新しい科目の既定へ追従する（入金の行の通信費→売上高）。手で選んだ税区分は残り、「違います」と出る', async ({ page }) => {
  await open(page, '#/');
  await withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 100 }, () =>
    withAccountDefaults(page, 'sales', { default_tax: 'S10', default_pct: 100 }, async () => {
      const token = await prepare(page, 'E2E 税区分追従', [['入金', '', '93438'], ['出金', '1100', '']]);
      const [income, out] = [rowsOf(page).nth(0), rowsOf(page).nth(1)];
      // 入金の行では、通信費の既定（課税仕入10%）は選べない。手で選んでいないので、仮の値（科目で決まる）。
      await pick(page, income, 'つうしん', '通信費');
      await expect(taxOf(income)).toHaveValue('__unset');
      await pick(page, income, 'うりあげ', '売上高');
      await expect(taxOf(income)).toHaveValue('S10');
      // 2回目の選び直しも追従する（売上高→雑収入（既定なし）→売上高）。
      await pick(page, income, 'ざっしゅう', '雑収入');
      await expect(taxOf(income)).toHaveValue('__unset');
      await pick(page, income, 'うりあげ', '売上高');
      await expect(taxOf(income)).toHaveValue('S10');
      // 手で選んだ税区分は、科目を2回変えても残り、科目の既定と違えば行に一言出る。
      await pick(page, out, 'しょうもう', '消耗品費');
      await expect(taxOf(out)).toHaveValue('P10');
      await taxOf(out).selectOption('P8');
      await pick(page, out, 'つうしん', '通信費');
      await pick(page, out, 'ちだい', '地代家賃');
      await expect(taxOf(out)).toHaveValue('P8');
      await expect(await msgOf(page, out)).toContainText('税区分が、科目の既定（対象外）と違います。');
      await expect.poll(async () => {
        const items = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}`)).json.items;
        return items.map((d) => [d.account, d.tax_code]);
      }).toEqual([['sales', 'S10'], ['exp_rent', 'P8']]);
    }),
  );
});

test('SC-03 g2-4 出金の行に収益、入金の行に費用の科目を選ぶと、行に注意を出す（印と文字。確定する行にはできる）', async ({ page }) => {
  await prepare(page, 'E2E 向き', [['出金', '1100', ''], ['入金', '', '93438']]);
  const [out, income] = [rowsOf(page).nth(0), rowsOf(page).nth(1)];
  await pick(page, out, 'うりあげ', '売上高');
  const outWarn = (await msgOf(page, out)).locator('[data-note="warn"]');
  await expect(outWarn).toHaveText('出金の行に、収益の科目が選ばれています。返金などでなければ、科目を見直してください。');
  await expect(outWarn.locator('svg')).toHaveCount(1);
  // 止めない（返金などは正しい）: Enter で確定する行になる。
  await accountOf(out).focus();
  await page.keyboard.press('Enter');
  await expect(out).toHaveClass(/is-candidate/);
  await pick(page, income, 'しょうもう', '消耗品費');
  await expect((await msgOf(page, income)).locator('[data-note="warn"]')).toHaveText('入金の行に、費用の科目が選ばれています。返金などでなければ、科目を見直してください。');
  // 向きに合う科目に直すと、注意は消える。
  await pick(page, out, 'しょうもう', '消耗品費');
  await expect((await msgOf(page, out)).locator('[data-note="warn"]')).toHaveCount(0);
});

test('SC-03 g2-5 一覧の上に「科目のある行をすべて選ぶ（N行）」をどの幅でも見せる。科目のある行が無いときは、押すと一言出す', async ({ page }) => {
  await prepare(page, 'E2E 全選択', [['001', '1100', ''], ['002', '1200', ''], ['003', '1300', '']]);
  const rows = rowsOf(page);
  const all = page.getByRole('checkbox', { name: /^科目のある行をすべて選ぶ/ });
  const line = page.locator('.inbox-all');
  await expect(all).toBeVisible();
  // 件数の文は、2026-10-15 の第3回で「（N行）」から「（N件のうちM件を選択中）」にした（下の帯の「確定する行 M件」との関係が
  // 読み分けにくかった。見た目の採点 第2回）。科目のある行が無ければ「（0件）」。
  await expect(line).toContainText('科目のある行をすべて選ぶ（0件）');
  await all.click();
  await expect(page.locator('.cand-all-note')).toHaveText('科目のある行がありません。');
  await expect(all).not.toBeChecked();
  await expect(rows.locator('.cand-check:checked')).toHaveCount(0);
  // 3行のうち1行に科目を入れる。押すとその1行だけを選び、「すべて」がチェック済みになる（対象は科目のある行）。
  await pick(page, rows.nth(0), 'しょうもう', '消耗品費');
  await expect(page.locator('.cand-all-note')).toBeHidden();
  await expect(line).toContainText('（1件のうち0件を選択中）');
  await all.click();
  await expect(all).toBeChecked();
  await expect(rows.nth(0)).toHaveClass(/is-candidate/);
  await expect(rows.nth(1)).not.toHaveClass(/is-candidate/);
  // どの幅でも、文字が見えている（チェックだけの幅ではない）。
  for (const width of [1440, 768, 390]) {
    await page.setViewportSize({ width, height: 844 });
    await expect(line.locator('.cand-all')).toContainText('科目のある行をすべて選ぶ（1件のうち1件を選択中）');
    expect((await line.locator('.cand-all').boundingBox()).width).toBeGreaterThan(150);
  }
});

test('SC-03 g2-5 「科目のある行をすべて選ぶ」は、科目のある行すべてに印を付け、もう一度押すと全部外す（先頭の1行だけにならない）', async ({ page }) => {
  // 行に印を付けるたびに表が描き直され、押した欄の checked が「一部」に戻る。ループの中でそれを読むと、2行目から外す側に回る（体験の評価 第2回 E2）。
  await prepare(page, 'E2E 全選択N', [['a', '1100', ''], ['b', '1200', ''], ['c', '1300', ''], ['d', '1400', '']], { account: 'exp_supplies' });
  const rows = rowsOf(page);
  const all = page.getByRole('checkbox', { name: /^科目のある行をすべて選ぶ/ });
  const count = page.locator('.inbox-all .cand-all-count');
  await expect(count).toHaveText('（4件のうち0件を選択中）');
  await all.click();
  await expect(rows.locator('.cand-check:checked')).toHaveCount(4);
  await expect(all).toBeChecked();
  await expect(page.locator('.inbox-sum--count')).toContainText('4件');
  await expect(count).toHaveText('（4件のうち4件を選択中）');
  await all.click();
  await expect(rows.locator('.cand-check:checked')).toHaveCount(0);
  await expect(all).not.toBeChecked();
  // 1行だけ印が付いた「一部」の状態から押すと、残りも全部選ぶ（外さない）。
  await rows.nth(1).locator('.cand-check').check();
  await expect(all).toHaveJSProperty('indeterminate', true);
  await expect(count).toHaveText('（4件のうち1件を選択中）');
  await all.click();
  await expect(rows.locator('.cand-check:checked')).toHaveCount(4);
  await expect(all).toBeChecked();
});

test('SC-03 g2-10 行の欄の読み上げの名前に、日付と金額を足す（同じ摘要の行を区別できる）', async ({ page }) => {
  const token = await prepare(page, 'E2E 名前', [['', '7470', '', '2026-01-05'], ['', '1200', '', '2026-01-06']]);
  const thisYear = String(new Date().getFullYear());
  const day = (d) => (thisYear === '2026' ? `01/${d}` : `2026/01/${d}`);
  for (const [d, amount] of [['05', '7,470'], ['06', '1,200']]) {
    const name = `${day(d)} ${token} ${amount}円`;
    await expect(page.getByRole('combobox', { name: `科目 ${name}` })).toHaveCount(1);
    await expect(page.getByRole('checkbox', { name: `確定する ${name}` })).toHaveCount(1);
    await expect(page.getByRole('textbox', { name: `日付 ${name}` })).toHaveCount(1);
    await expect(page.getByRole('combobox', { name: `税区分 ${name}` })).toHaveCount(1);
    // キー（x）は名前に入れず、aria-keyshortcuts で伝える（2026-10-15 の第3回で「無視する（x）: …」から変えた。E4 第2回の9）。
    const ignore = page.getByRole('button', { name: `無視する: ${name}`, exact: true });
    await expect(ignore).toHaveCount(1);
    await expect(ignore).toHaveAttribute('aria-keyshortcuts', 'x');
  }
});

test('SC-03 r3-4 税区分の選択肢は登録・訂正の画面と同じ名前で、幅1440の表の欄に入りきる。既定が対象外の科目では、空の税区分は「対象外（既定）」', async ({ page }) => {
  await open(page, '#/');
  await withAccountDefaults(page, 'exp_travel', { default_tax: null, default_pct: 100 }, async () => {
    await prepare(page, 'E2E 税区分名', [['出金', '1100', ''], ['入金', '', '5000']]);
    const [out, income] = [rowsOf(page).nth(0), rowsOf(page).nth(1)];
    // 登録・訂正の画面の名前（TAX_LABELS）。経過措置だけ、後ろの「（免税事業者等から）」を省く（欄の title に全体を出す）。
    expect(await taxOf(out).locator('option:not([hidden])').allTextContents()).toEqual(['対象外', '課税仕入10%', '課税仕入8%（軽減）', '経過措置の仕入10%', '経過措置の仕入8%', '非課税']);
    expect(await taxOf(income).locator('option:not([hidden])').allTextContents()).toEqual(['対象外', '課税売上10%', '課税売上8%（軽減）', '非課税']);
    await taxOf(out).selectOption('P10X');
    await expect(taxOf(out)).toHaveAttribute('title', '経過措置の仕入10%（免税事業者等から）');
    // どの選択肢も、矢印の手前で切れない（選択欄の中の文字の幅と、欄の中の幅を比べる）。
    const over = await page.evaluate(() => {
      const ctx = document.createElement('canvas').getContext('2d');
      return [...document.querySelectorAll('tr.inbox-row td.c-tax select')].flatMap((s) => {
        const cs = getComputedStyle(s);
        ctx.font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
        const room = s.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
        return [...s.options].filter((o) => ctx.measureText(o.textContent).width > room).map((o) => `${o.textContent} > ${room}px`);
      });
    });
    expect(over).toEqual([]);
    // 科目が無い行は、今までどおり「科目で決まる」。
    await expect(taxOf(income)).toHaveValue('__unset');
    await expect(taxOf(income).locator('option:checked')).toHaveText('科目で決まる');
  });
});

test('SC-03 r3-4 手で選んでいない税区分は、既定が対象外の科目なら「対象外（既定）」、既定が行の向きで使えない科目なら「対象外」と出る', async ({ page }) => {
  await open(page, '#/');
  await withAccountDefaults(page, 'exp_travel', { default_tax: null, default_pct: 100 }, () =>
    withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 100 }, async () => {
      await prepare(page, 'E2E 仮の値', [['出金', '1100', ''], ['入金', '', '5000']]);
      const [out, income] = [rowsOf(page).nth(0), rowsOf(page).nth(1)];
      await pick(page, out, 'りょひ', '旅費交通費');
      await expect(taxOf(out)).toHaveValue('__unset');
      await expect(taxOf(out).locator('option:checked')).toHaveText('対象外（既定）');
      await expect(taxOf(out)).toHaveAttribute('title', 'この科目の既定の税区分は「対象外」です。このまま確定すると「対象外」になります。');
      await expect(taxOf(out)).toHaveClass(/is-placeholder/);
      // 入金の行の通信費: 既定（課税仕入10%）は入金の行で選べないので、既定とは言わずに「対象外」。
      await pick(page, income, 'つうしん', '通信費');
      await expect(taxOf(income)).toHaveValue('__unset');
      await expect(taxOf(income).locator('option:checked')).toHaveText('対象外');
      await expect(taxOf(income)).toHaveAttribute('title', 'この科目の既定の税区分（課税仕入10%）は、入金の行では使えません。このまま確定すると「対象外」になります。');
      // 確定すると、どちらも対象外（税区分なし）で記帳される。
      await out.locator('.cand-check').check();
      await income.locator('.cand-check').check();
      await page.keyboard.press('ControlOrMeta+Enter');
      await page.getByRole('dialog', { name: '2件を確定しますか' }).getByRole('button', { name: '確定する' }).click();
      await expect(page.locator('.toast').filter({ hasText: '2件を確定しました' })).toBeVisible();
    }),
  );
});

test('SC-03 r3-4 割合の列の見出しは「事業の割合」。説明（残りは事業主貸に入る）を見出しの title と、各行の割合の欄の説明に添える。金額の見出しは単位つき', async ({ page }) => {
  await prepare(page, 'E2E 事業の割合', [['出金', '1100', '']]);
  const th = page.locator('.inbox-table th.c-pct');
  await expect(th).toHaveText('事業の割合');
  await expect(th).toHaveAttribute('title', '仕事に使った分の割合。残りは事業主貸に入ります。');
  await expect(page.locator('.inbox-table th.c-amount')).toHaveText('金額（円）');
  const pct = rowsOf(page).nth(0).locator('td.c-pct input');
  await expect(pct).toHaveAccessibleName(/^事業の割合 /);
  await expect(pct).toHaveAccessibleDescription('事業の割合は、仕事に使った分の割合。残りは事業主貸に入ります。');
});

test('SC-03 r3-5 幅390: 「くわしく」の名前に行の名前（日付・摘要・金額）を足し、開くと文字も名前も「閉じる」に変わる', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const token = await prepare(page, 'E2E くわしく', [['', '7470', '', '2026-01-05'], ['', '1200', '', '2026-01-06']]);
  const thisYear = String(new Date().getFullYear());
  const day = (d) => (thisYear === '2026' ? `01/${d}` : `2026/01/${d}`);
  const first = rowsOf(page).nth(0);
  const more = first.locator('.more-btn');
  await expect(more).toHaveAccessibleName(`くわしく: ${day('05')} ${token} 7,470円`);
  await expect(rowsOf(page).nth(1).locator('.more-btn')).toHaveAccessibleName(`くわしく: ${day('06')} ${token} 1,200円`);
  await expect(more).toHaveText('くわしく');
  await expect(more).toHaveAttribute('aria-expanded', 'false');
  await more.click();
  await expect(more).toHaveText('閉じる');
  await expect(more).toHaveAttribute('aria-expanded', 'true');
  await expect(more).toHaveAccessibleName(`閉じる: ${day('05')} ${token} 7,470円`);
  await expect(first.locator('td.c-tax select')).toBeVisible();
  await more.click();
  await expect(more).toHaveText('くわしく');
  await expect(more).toHaveAttribute('aria-expanded', 'false');
  await expect(first.locator('td.c-tax select')).toBeHidden();
});

test('SC-03 r3-5 1件だけ確定した通知の番号は「No.N」だけ（範囲の形にしない）', async ({ page }) => {
  await prepare(page, 'E2E 1件確定', [['001', '1100', ''], ['002', '1200', '']], { account: 'exp_supplies' });
  await rowsOf(page).nth(0).locator('.cand-check').check();
  await page.keyboard.press('ControlOrMeta+Enter');
  await page.getByRole('dialog', { name: '1件を確定しますか' }).getByRole('button', { name: '確定する' }).click();
  await expect(page.locator('.toast').filter({ hasText: '1件を確定しました' })).toHaveText(/1件を確定しました（No\.\d+）。/);
  await expect(page.locator('.toast').filter({ hasText: '1件を確定しました' })).not.toContainText('〜');
});

test('SC-03 g2-5 下の帯の件数と合計が変わると、少し待ってから読み上げの領域に入れる（続けて変えたら最後の値だけ）', async ({ page }) => {
  await prepare(page, 'E2E 読み上げ', [['001', '1100', ''], ['002', '1000', ''], ['003', '1200', '']], { account: 'exp_supplies' });
  const live = page.locator('.inbox-bar .inbox-live');
  await expect(live).toHaveAttribute('aria-live', 'polite');
  // 最初の表示（0件）は読まない。
  await expect(live).toHaveText('');
  await page.evaluate(() => {
    window.__spoken = [];
    new MutationObserver(() => window.__spoken.push(document.querySelector('.inbox-live').textContent)).observe(document.querySelector('.inbox-live'), { childList: true, characterData: true, subtree: true });
  });
  // 人が続けて印を付ける速さ（150ms おき）で3行に付ける。試験の道具の遅れに左右されないよう、ページの中で押す。
  await page.evaluate(async () => {
    const boxes = document.querySelectorAll('tr.inbox-row .cand-check');
    for (let i = 0; i < 3; i += 1) {
      boxes[i].click();
      await new Promise((resolve) => setTimeout(resolve, 150));
    }
  });
  await expect(live).toHaveText('確定する行 3件、入金 0円、出金 3,300円');
  expect(await page.evaluate(() => window.__spoken)).toEqual(['確定する行 3件、入金 0円、出金 3,300円']);
});

test('SC-03 g2-9 幅390: 確定する行が0件の帯は、指の操作の案内の1行だけ。印を付けると、件数とボタンが出る', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await prepare(page, 'E2E 狭い帯', [['001', '1100', ''], ['002', '1200', '']], { account: 'exp_supplies' });
  const bar = page.locator('.inbox-bar');
  await expect(bar.locator('.note-narrow')).toHaveText('左の印を押すと、確定する行になります。');
  await expect(bar.locator('.note-wide')).toBeHidden();
  await expect(bar.locator('.inbox-bar-main')).toBeHidden();
  const empty = await bar.boundingBox();
  expect(empty.height).toBeLessThan(844 * 0.1);
  await rowsOf(page).nth(0).locator('.cand-check').check();
  await expect(bar.locator('.inbox-bar-main')).toBeVisible();
  await expect(bar).toContainText('確定する行1件');
  await expect(bar.getByRole('button', { name: /確定する/ })).toBeEnabled();
  // 広い幅では、キーボードの案内を出す。
  await rowsOf(page).nth(0).locator('.cand-check').uncheck();
  await page.setViewportSize({ width: 1440, height: 900 });
  await expect(bar.locator('.note-wide')).toHaveText('確定する行は、まだありません。行で Enter を押すと、確定する行になります。');
  await expect(bar.locator('.note-narrow')).toBeHidden();
  await expect(bar.locator('.inbox-bar-main')).toBeVisible();
});

test('SC-03 g2-6 印の列の見出しは「確定する」。表の名前のキーは端末に合わせる（Mac は ⌘）。無視の通知は「無視しました」', async ({ page }) => {
  const token = await prepare(page, 'E2E 言葉', [['001', '1100', ''], ['002', '1200', '']]);
  await expect(page.locator('.inbox-table th.c-cand')).toHaveText('確定する');
  const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent));
  await expect(page.getByRole('table', { name: `未確認の明細（Enterで確定する行にして次の行へ、${mac ? '⌘' : 'Ctrl'}+Enterで確定）` })).toHaveCount(1);
  await rowsOf(page).nth(0).getByRole('button', { name: /^無視する: / }).click();
  const notice = page.locator('.toast').filter({ hasText: `「${token} 001」を無視しました。` });
  await expect(notice).toBeVisible();
  await expect(notice.getByRole('button', { name: '元に戻す' })).toBeVisible();
  await page.getByRole('tab', { name: /^無視した行/ }).click();
  await expect(page.getByRole('table', { name: '無視した行（「戻す」で未確認に戻る）' })).toHaveCount(1);
});
