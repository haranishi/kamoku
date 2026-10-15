// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳帳（SC-05）: キーで行を動かして詳細を開く、訂正（理由と差分）、取消（理由と赤系の確認、Ctrl/Cmd+Enter）、古い版からの訂正、
// 読み上げの見出しと区切り、検索欄の Enter と Esc。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, entryFields, ensureEntryWith, withSettings } from './helpers.mjs';

async function createEntry(page, desc, amount = 3300) {
  const res = await apiCall(page, 'POST', '/entries', {
    kind: 'expense',
    date: '2026-10-02',
    amount,
    account: 'exp_supplies',
    wallet: 'w01',
    tax_code: 'P10',
    partner: 'サンプル文具',
    description: desc,
    business_pct: 100,
    memo: '',
  });
  expect(res.status).toBe(201);
  return res.json;
}

test('SC-05 j k で行を動かし、Enterで詳細を開き、Escで閉じると一覧と同じ行に戻る', async ({ page }) => {
  await open(page, '#/journal');
  const rows = page.locator('.journal-table tbody tr');
  await expect(rows.first()).toBeVisible();
  await page.keyboard.press('j');
  await expect(rows.nth(0)).toBeFocused();
  await page.keyboard.press('j');
  await expect(rows.nth(1)).toBeFocused();
  await page.keyboard.press('k');
  await page.keyboard.press('j');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/journal\/e_[0-9a-f]+/);
  const drawer = page.getByRole('dialog');
  await expect(drawer).toContainText('の仕訳');
  await expect(drawer.getByRole('heading', { name: '履歴' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(drawer).toHaveCount(0);
  await expect(page).toHaveURL(/#\/journal$/);
  await expect(rows.nth(1)).toBeFocused();
});

test('SC-05 絞り込みはアドレスに入り、戻る操作で前の条件に戻る', async ({ page }) => {
  await open(page, '#/journal');
  // 本物のサーバーの合成データには「家賃」の仕訳が無いことがあるので、無ければ作る。
  await ensureEntryWith(page, '家賃');
  await page.reload();
  await page.locator('main h1').focus();
  await page.keyboard.press('/');
  const search = page.getByRole('searchbox', { name: '文字で探す' });
  await expect(search).toBeFocused();
  await search.fill('家賃');
  await search.press('Enter');
  await expect(page).toHaveURL(/#\/journal\?q=%E5%AE%B6%E8%B3%83/);
  await expect(page.locator('.journal-table tbody tr').first()).toContainText('家賃');
  await page.locator('main h1').focus();
  await page.keyboard.press(']');
  await expect(page).toHaveURL(/from=\d{4}-\d{2}-01/);
  await page.goBack();
  await expect(page).toHaveURL(/#\/journal\?q=%E5%AE%B6%E8%B3%83$/);
  await expect(search).toHaveValue('家賃');
});

test('AC-06.4 詳細から訂正に移ると現在の値が入っていて、理由と差分を確かめて第2版になる', async ({ page }) => {
  const desc = uniqueText('E2E 訂正');
  const e = await createEntry(page, desc);
  // 詳細は、仕訳帳の一覧から開く（板を閉じると、開く前の画面へ戻るため。ホームから開くと、閉じたときホームへ戻る）。
  await open(page, '#/journal');
  await page.goto(`/#/journal/${e.id}`);
  const drawer = page.getByRole('dialog');
  await expect(drawer).toContainText(desc);
  await page.keyboard.press('e');
  await expect(page).toHaveURL(new RegExp(`#/entry/${e.id}$`));
  const f = entryFields(page);
  await expect(page.locator('main h1')).toHaveText('取引を訂正');
  await expect(f.amount).toHaveValue('3,300');
  await expect(f.account).toHaveValue('消耗品費');
  await expect(f.wallet).toHaveValue('現金');
  await expect(f.description).toHaveValue(desc);
  await expect(f.date).toHaveValue('2026-10-02 (金)');

  // 理由が無いと保存できない。
  await f.amount.fill('3850');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.getByText('訂正の理由を入れてください。')).toBeVisible();
  await page.getByLabel('訂正の理由').fill('金額の入力ミス');
  await page.keyboard.press('ControlOrMeta+Enter');
  const confirm = page.getByRole('dialog', { name: /を訂正します/ });
  await expect(confirm).toContainText('3,300円');
  await expect(confirm).toContainText('3,850円');
  await expect(confirm.getByRole('button', { name: '訂正を保存' })).toBeFocused();
  // 保存の画面から Ctrl/Cmd+Enter で開いた確認は、同じキーで進める（取消の確認と同じ。体験の評価 第2回 E1）。ボタンにもキーを書く。
  await expect(confirm.getByRole('button', { name: '訂正を保存' })).toContainText(/(⌘|Ctrl)\s*Enter/);
  await page.keyboard.press('ControlOrMeta+Enter');

  await expect(page).toHaveURL(new RegExp(`#/journal/${e.id}$`));
  await expect(page.getByRole('dialog')).toContainText('訂正 第2版');
  await expect(page.getByRole('dialog')).toContainText('理由: 金額の入力ミス');
  const after = (await apiCall(page, 'GET', `/entries/${e.id}`)).json;
  expect(after.revision).toBe(2);
  expect(after.total).toBe(3850);
});

test('AC-06.6 ほかの場所で更新されていたら、読み直して差分を見せ、入力は残す', async ({ page }) => {
  const desc = uniqueText('E2E 競合');
  const e = await createEntry(page, desc);
  await open(page, `#/entry/${e.id}`);
  const f = entryFields(page);
  await expect(f.description).toHaveValue(desc);
  await f.description.fill(`${desc} 画面で直した`);
  // 画面が読み込んだ後に、別の場所（ここではAPI）で第2版にする。
  const other = await apiCall(page, 'PUT', `/entries/${e.id}`, { ...e.input, amount: 4400, reason: '別の場所での訂正', expected_revision: 1 });
  expect(other.status).toBe(200);

  await page.getByLabel('訂正の理由').fill('摘要を直した');
  await page.getByRole('button', { name: /訂正を確かめる/ }).click();
  await page.getByRole('button', { name: '訂正を保存' }).click();
  await expect(page.locator('.toast').filter({ hasText: '他の場所で更新されました。読み込み直します。' })).toBeVisible();
  const box = page.locator('.stale-box');
  await expect(box).toContainText('最新の版（第2版）を読み込みました');
  await expect(box).toContainText('4,400円');
  await expect(f.description).toHaveValue(`${desc} 画面で直した`);

  // もう一度保存すると、最新の版を元に第3版になる。
  await page.getByRole('button', { name: /訂正を確かめる/ }).click();
  await page.getByRole('button', { name: '訂正を保存' }).click();
  await expect(page).toHaveURL(new RegExp(`#/journal/${e.id}$`));
  const after = (await apiCall(page, 'GET', `/entries/${e.id}`)).json;
  expect(after.revision).toBe(3);
  expect(after.description).toBe(`${desc} 画面で直した`);
});

test('SC-05 取消は理由が要り、赤系の確認ボタンで取り消す', async ({ page }) => {
  const desc = uniqueText('E2E 取消');
  const e = await createEntry(page, desc);
  // 一覧から開く（閉じたとき一覧へ戻る。AC-06.4 と同じ）。
  await open(page, '#/journal');
  await page.goto(`/#/journal/${e.id}`);
  await expect(page.getByRole('dialog')).toContainText(desc);
  await page.keyboard.press('v');
  const dialog = page.getByRole('dialog', { name: '仕訳を取り消しますか' });
  await expect(dialog).toBeVisible();
  const confirm = dialog.getByRole('button', { name: '取り消す' });
  await expect(confirm).toHaveClass(/btn--danger/);
  await confirm.click();
  await expect(dialog).toContainText('取消の理由を入れてください。');
  await dialog.getByLabel('取消の理由').fill('二重に登録したため');
  await confirm.click();
  await expect(dialog).toHaveCount(0);
  const drawer = page.getByRole('dialog');
  await expect(drawer).toContainText('取消済み');
  await expect(drawer.getByRole('button', { name: /訂正する/ })).toBeDisabled();
  await expect(drawer.getByRole('button', { name: /取り消す/ })).toBeDisabled();
  expect((await apiCall(page, 'GET', `/entries/${e.id}`)).json.state).toBe('voided');
  await page.keyboard.press('Escape');
  await expect(page.locator('.journal-table tbody tr', { hasText: desc })).toContainText('取消済み');
});

test('SC-05 一覧の札は取消と訂正の行だけ。合計は何の合計かを文で書き、すべての期間では前後の矢印を出さない', async ({ page }) => {
  const desc = uniqueText('E2E 札');
  const e = await createEntry(page, desc);
  const revised = await apiCall(page, 'PUT', `/entries/${e.id}`, { ...e.input, amount: 4400, reason: '札の確かめ', expected_revision: 1 });
  expect(revised.status).toBe(200);
  await createEntry(page, `${desc} 2件目`);
  await open(page, `#/journal?q=${encodeURIComponent(desc)}`);
  const rows = page.locator('.journal-table tbody tr');
  await expect(rows).toHaveCount(2);
  // 有効な行（第1版）には札も「有効」の文字も出さない。訂正の行は朱の淡い札（色の決まり）。
  await expect(rows.filter({ hasText: '2件目' }).locator('.c-state')).toHaveText('');
  const badge = rows.filter({ hasNotText: '2件目' }).locator('.c-state .badge');
  await expect(badge).toHaveText('訂正 第2版');
  await expect(badge).toHaveClass(/badge--danger/);
  // 合計は、条件に合う全件の合計（取消済みも含む、が既定）。
  await expect(page.locator('.list-summary')).toContainText('条件に合う2件の合計（取消済みも含む）');
  await expect(page.locator('.list-summary')).toContainText('7,700円');
  // すべての期間: 前後の矢印は出さず、「月ごとに見る」から今月へ。
  await expect(page.getByRole('button', { name: /前の月/ })).toBeHidden();
  await expect(page.getByRole('button', { name: /次の月/ })).toBeHidden();
  await page.getByRole('button', { name: '月ごとに見る' }).click();
  await expect(page).toHaveURL(/from=\d{4}-\d{2}-01/);
  await expect(page.getByRole('button', { name: /前の月/ })).toBeVisible();
  await expect(page.getByRole('button', { name: '月ごとに見る' })).toBeHidden();
});

test('SC-05 詳細: 金額は大きく、訂正のある仕訳は最新の訂正と理由を上に出す。取り消すは朱の枠のボタン', async ({ page }) => {
  const desc = uniqueText('E2E 詳細');
  const e = await createEntry(page, desc);
  await apiCall(page, 'PUT', `/entries/${e.id}`, { ...e.input, amount: 5500, reason: '詳細の確かめ', expected_revision: 1 });
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, `#/journal/${e.id}`);
  const drawer = page.getByRole('dialog');
  await expect(drawer.locator('.detail-total')).toContainText('5,500円');
  const latest = drawer.locator('.detail-latest');
  await expect(latest).toContainText('訂正 第2版');
  await expect(latest).toContainText('理由: 詳細の確かめ');
  // 最初の1画面（高さ844px）に、最新の訂正と、訂正・取消の操作が入る。
  for (const el of [latest, drawer.getByRole('button', { name: /訂正する/ }), drawer.getByRole('button', { name: /取り消す/ })]) {
    const box = await el.boundingBox();
    expect(box.y + box.height).toBeLessThanOrEqual(844);
  }
  await expect(drawer.getByRole('button', { name: /取り消す/ })).toHaveClass(/btn-void/);
  // 狭い幅の明細は、借方と貸方の2列（表は隠す）。
  await expect(drawer.locator('.detail-sides')).toBeVisible();
  await expect(drawer.locator('.detail-lines')).toBeHidden();
  await expect(drawer.locator('.detail-side-col').first()).toContainText('借方');
  await expect(drawer.locator('.detail-side-col').first()).toContainText('消耗品費');
});

test('SC-05 取消の確認は、Ctrl/Cmd+Enter でも取り消せる。取り消すと元に戻せないことを書く', async ({ page }) => {
  const desc = uniqueText('E2E 取消のキー');
  const e = await createEntry(page, desc);
  await open(page, `#/journal/${e.id}`);
  await expect(page.getByRole('dialog')).toContainText(desc);
  await page.keyboard.press('v');
  const dialog = page.getByRole('dialog', { name: '仕訳を取り消しますか' });
  await expect(dialog).toContainText('取り消すと、元に戻せません。');
  await expect(dialog).toContainText('記録は履歴に残ります。');
  // 理由が無ければ、キーでも取り消さない。
  await dialog.getByLabel('取消の理由').focus();
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(dialog).toContainText('取消の理由を入れてください。');
  await dialog.getByLabel('取消の理由').fill('キーで取り消す');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(dialog).toHaveCount(0);
  expect((await apiCall(page, 'GET', `/entries/${e.id}`)).json.state).toBe('voided');
});

test('SC-05 一覧: 最後の列の見出しは読み上げで「詳細を開く」、摘要と取引先の間に見えない区切り。検索欄は Enter で探し、Esc で欄から出る', async ({ page }) => {
  const desc = uniqueText('E2E 区切り');
  await createEntry(page, desc);
  await open(page, `#/journal?q=${encodeURIComponent(desc)}`);
  const table = page.locator('.journal-table');
  await expect(table.getByRole('columnheader', { name: '詳細を開く' })).toHaveCount(1);
  await expect(table.locator('tbody tr').first().locator('.c-desc')).toHaveText(`${desc}、取引先 サンプル文具`);
  const search = page.getByRole('searchbox', { name: '文字で探す' });
  await expect(search).toHaveAttribute('placeholder', /Enterで探します/);
  // 打ちかけで Esc: 欄から出て、探している条件の文字に戻る。続けて押した g h は、文字でなく移動として働く。
  await search.click();
  await page.keyboard.type('abc');
  await page.keyboard.press('Escape');
  await expect(search).not.toBeFocused();
  await expect(search).toHaveValue(desc);
  await expect(page).toHaveURL(new RegExp(`#/journal\\?q=`));
  await page.keyboard.press('g');
  await page.keyboard.press('h');
  await expect(page).toHaveURL(/#\/$/);
});

// ---- 取消・訂正・詳細を閉じた後の現在地（2026-10-15 の体験の評価 E4 の不具合1・2、E1 の B5、E3 の7） ----

/** 総勘定元帳（科目と月）のアドレス。 */
const ledgerHash = (account, month) => `#/reports/ledger?account=${account}&month=${month}`;

test('SC-05 一覧で v で取り消すと、一覧を読み直した後も、取り消した行にフォーカスが残る（⌘Enter でも「取り消す」のボタンでも）', async ({ page }) => {
  const desc = uniqueText('E2E 取消の後のフォーカス');
  await createEntry(page, `${desc} 1`);
  await createEntry(page, `${desc} 2`);
  await open(page, `#/journal?q=${encodeURIComponent(desc)}`);
  const rows = page.locator('.journal-table tbody tr');
  await expect(rows).toHaveCount(2);
  const dialog = page.getByRole('dialog', { name: '仕訳を取り消しますか' });

  // 1行目: 理由を入れて Ctrl/Cmd+Enter。
  await page.keyboard.press('j');
  await expect(rows.nth(0)).toBeFocused();
  const firstDesc = (await rows.nth(0).locator('.c-desc span').first().textContent()).trim();
  await page.keyboard.press('v');
  await dialog.getByLabel('取消の理由').fill('E2E フォーカスの確かめ');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('.toast').filter({ hasText: 'を取り消しました。' }).first()).toBeVisible();
  const voided = rows.filter({ hasText: firstDesc });
  await expect(voided).toContainText('取消済み');
  await expect(voided).toBeFocused();

  // 2行目: 「取り消す」のボタンを押す。
  await page.keyboard.press('j');
  await expect(rows.nth(1)).toBeFocused();
  const secondDesc = (await rows.nth(1).locator('.c-desc span').first().textContent()).trim();
  await page.keyboard.press('v');
  await dialog.getByLabel('取消の理由').fill('E2E ボタンで取り消す');
  await dialog.getByRole('button', { name: '取り消す' }).click();
  await expect(dialog).toHaveCount(0);
  const voided2 = rows.filter({ hasText: secondDesc });
  await expect(voided2).toContainText('取消済み');
  await expect(voided2).toBeFocused();
});

test('SC-05 検索した一覧から開いた仕訳を訂正して保存し、板を閉じると、検索の条件のまま一覧に戻り、その行にフォーカスがある', async ({ page }) => {
  const desc = uniqueText('E2E 訂正の後の戻り先');
  const e = await createEntry(page, desc);
  await open(page, `#/journal?q=${encodeURIComponent(desc)}`);
  const rows = page.locator('.journal-table tbody tr');
  await expect(rows).toHaveCount(1);
  await page.keyboard.press('j');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('dialog', { name: `No.${e.no} の仕訳` })).toBeVisible();
  await page.keyboard.press('e');
  // 訂正の画面のアドレスに、詳細を閉じたときの戻り先（検索の条件の一覧）を持たせる。
  await expect(page).toHaveURL(new RegExp(`#/entry/${e.id}\\?back=`));
  const f = entryFields(page);
  await expect(f.amount).toHaveValue('3,300');
  await f.amount.fill('3850');
  await page.getByLabel('訂正の理由').fill('戻り先の確かめ');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.getByRole('dialog', { name: /を訂正します/ })).toBeVisible();
  await page.keyboard.press('ControlOrMeta+Enter');
  // 保存した後の詳細は、検索の条件の一覧の上に開く。
  await expect(page).toHaveURL(new RegExp(`#/journal/${e.id}\\?q=`));
  const drawer = page.getByRole('dialog', { name: `No.${e.no} の仕訳` });
  await expect(drawer).toContainText('訂正 第2版');
  await page.keyboard.press('Escape');
  await expect(drawer).toHaveCount(0);
  await expect(page).toHaveURL(new RegExp(`#/journal\\?q=`));
  await expect(page.getByRole('searchbox', { name: '文字で探す' })).toHaveValue(desc);
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText('訂正 第2版');
  await expect(rows.first()).toBeFocused();
});

test('SC-05 一覧の e で訂正して保存した後も、詳細を閉じると検索の条件の一覧のその行に戻る。板の中で取り消して「閉じる」でも、その行に戻る', async ({ page }) => {
  const desc = uniqueText('E2E 一覧から訂正');
  const e = await createEntry(page, desc);
  await open(page, `#/journal?q=${encodeURIComponent(desc)}`);
  const rows = page.locator('.journal-table tbody tr');
  await expect(rows).toHaveCount(1);
  await page.keyboard.press('j');
  await page.keyboard.press('e');
  await expect(page).toHaveURL(new RegExp(`#/entry/${e.id}\\?back=`));
  const f = entryFields(page);
  await expect(f.amount).toHaveValue('3,300');
  await f.amount.fill('4400');
  await page.getByLabel('訂正の理由').fill('一覧から訂正');
  await page.keyboard.press('ControlOrMeta+Enter');
  await page.getByRole('dialog', { name: /を訂正します/ }).getByRole('button', { name: '訂正を保存' }).click();
  const drawer = page.getByRole('dialog', { name: `No.${e.no} の仕訳` });
  await expect(drawer).toContainText('訂正 第2版');
  await expect(page).toHaveURL(new RegExp(`#/journal/${e.id}\\?q=`));

  // 板の中の「取り消す」のボタンで取り消す。ボタンは止まり、中身も作り直されるが、フォーカスは板の中（題名）に残る。
  await drawer.getByRole('button', { name: /取り消す/ }).click();
  const dialog = page.getByRole('dialog', { name: '仕訳を取り消しますか' });
  await dialog.getByLabel('取消の理由').fill('板の中で取り消す');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(dialog).toHaveCount(0);
  await expect(drawer).toContainText('取消済み');
  await expect(drawer.getByRole('heading', { name: `No.${e.no} の仕訳` })).toBeFocused();
  await drawer.getByRole('button', { name: '閉じる', exact: true }).click();
  await expect(drawer).toHaveCount(0);
  await expect(page).toHaveURL(new RegExp(`#/journal\\?q=`));
  await expect(rows.first()).toContainText('取消済み');
  await expect(rows.first()).toBeFocused();
});

test('SC-05 総勘定元帳の行から開いた仕訳を Esc・「閉じる」で閉じると、同じ科目・期間の元帳へ戻り、元帳の表の行にフォーカスがある。戻るの履歴は増えない', async ({ page }) => {
  const desc = uniqueText('E2E 元帳から開く');
  const e = await createEntry(page, desc);
  await open(page, '#/');
  const ledger = ledgerHash('exp_supplies', '2026-10');
  await page.evaluate((h) => {
    location.hash = h;
  }, ledger);
  const row = page.locator('.ledger-table tbody tr', { hasText: desc });
  await expect(row).toBeVisible();

  for (const how of ['Escape', '閉じる']) {
    await row.click();
    await expect(page).toHaveURL(new RegExp(`#/journal/${e.id}`));
    const drawer = page.getByRole('dialog', { name: `No.${e.no} の仕訳` });
    await expect(drawer).toBeVisible();
    if (how === 'Escape') await page.keyboard.press('Escape');
    else await drawer.getByRole('button', { name: '閉じる', exact: true }).click();
    await expect(drawer).toHaveCount(0);
    // 元帳（同じ科目・期間）に戻る。仕訳帳の一覧には移らない。
    await expect(page).toHaveURL(/#\/reports\/ledger\?/);
    const q = new URLSearchParams(new URL(page.url()).hash.split('?')[1]);
    expect(q.get('account')).toBe('exp_supplies');
    expect(q.get('month')).toBe('2026-10');
    await expect(row).toBeVisible();
    // フォーカスは BODY に落ちず、元帳の表の行にある。
    await expect(page.locator('.ledger-table tbody tr:focus')).toHaveCount(1);
  }
  // 板を閉じても履歴の行は増えない: 戻るで、元帳の前の画面（ホーム）へ。
  await page.goBack();
  await expect(page).toHaveURL(/#\/$/);
});

// 締めた日以前の仕訳は、訂正も取消もできない。開いた時点で知らせてボタンを止める（理由を書いて確定してから断られ、断る文の
// 「日付を直すか」が取消に合わなかった。2026-10-15 の体験の評価 E3）。
test('SC-05 締めた日以前の仕訳は、詳細を開いた時点で「訂正も取消もできません」と知らせ、ボタンを止める。締めを外すと押せる', async ({ page }) => {
  const e = await createEntry(page, uniqueText('E2E 締めた期間'));
  await open(page, '#/');
  await withSettings(page, { locked_until: '2026-10-10' }, async () => {
    await page.reload();
    await page.evaluate((h) => {
      location.hash = h;
    }, `#/journal/${e.id}`);
    const drawer = page.getByRole('dialog', { name: `No.${e.no} の仕訳` });
    await expect(drawer).toBeVisible();
    await expect(drawer.getByRole('button', { name: /^訂正する/ })).toBeDisabled();
    await expect(drawer.getByRole('button', { name: /^取り消す/ })).toBeDisabled();
    await expect(drawer).toContainText('2026年10月10日まで締めてあるため、この仕訳は訂正も取消もできません。');
  });
  // 締めを外すと、同じ仕訳が訂正・取消できる。
  await page.reload();
  const drawer = page.getByRole('dialog', { name: `No.${e.no} の仕訳` });
  await expect(drawer).toBeVisible();
  await expect(drawer.getByRole('button', { name: /^訂正する/ })).toBeEnabled();
  await expect(drawer.getByRole('button', { name: /^取り消す/ })).toBeEnabled();
});

// 一覧から開いた詳細Aを開いたまま、通知のリンクなどで別の詳細Bへ移ったとき、Bを閉じて「戻る」を使うと、Aが開き直った
// （一覧から開いた印が残っていたため。2026-10-15 の読み取りレビュー）。1つ前の履歴が一覧のときだけ「戻る」を使う。
test('SC-05 一覧から開いた詳細Aを開いたまま別の詳細Bへ移り、Bを閉じると、Aではなく一覧へ戻る', async ({ page }) => {
  const tag = uniqueText('E2E 続けて開く');
  const a = await createEntry(page, `${tag} A`);
  const b = await createEntry(page, `${tag} B`);
  await open(page, `#/journal?q=${encodeURIComponent(tag)}`);
  await page.locator('.journal-table tbody tr', { hasText: `${tag} A` }).click();
  await expect(page).toHaveURL(new RegExp(`#/journal/${a.id}`));
  await expect(page.getByRole('dialog', { name: `No.${a.no} の仕訳` })).toBeVisible();
  // 通知の「仕訳を見る」のように、一覧を経ずに別の詳細へ移る。
  await page.evaluate((h) => {
    location.hash = h;
  }, `#/journal/${b.id}`);
  await expect(page.getByRole('dialog', { name: `No.${b.no} の仕訳` })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page).toHaveURL(/#\/journal(\?[^/]*)?$/);
});

// 約束 C1（元帳の行の data-entry-id）。元帳から開いた詳細を閉じると、開いた行にフォーカスが戻る。
test('SC-05 総勘定元帳から開いた仕訳を閉じると、開いた行（data-entry-id）にフォーカスが戻る（約束 C1）', async ({ page }) => {
  const desc = uniqueText('E2E 元帳の行へ戻る');
  await createEntry(page, `${desc} 前`);
  const e = await createEntry(page, `${desc} 後`);
  await open(page, '#/');
  await page.evaluate((h) => {
    location.hash = h;
  }, ledgerHash('exp_supplies', '2026-10'));
  const target = page.locator('.ledger-table tbody tr', { hasText: `${desc} 後` });
  await expect(target).toBeVisible();
  // 開く行は、表の最初の行ではない（最初の行へ移っただけでは通らないように）。
  await expect(page.locator('.ledger-table tbody tr').first()).not.toContainText(`${desc} 後`);
  await target.click();
  await expect(page.getByRole('dialog', { name: `No.${e.no} の仕訳` })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page).toHaveURL(/#\/reports\/ledger\?/);
  await expect(page.locator(`.ledger-table tbody tr[data-entry-id="${e.id}"]`)).toBeFocused();
});

test('SC-05 検索欄で ↓ を押すと、一覧の先頭の行へ移る。打ちかけ（Enter の前）なら、先に探してから移る', async ({ page }) => {
  const desc = uniqueText('E2E 下の矢印');
  await createEntry(page, `${desc} 甲`);
  await createEntry(page, `${desc} 乙`);
  await open(page, '#/journal');
  await page.locator('main h1').focus();
  await page.keyboard.press('/');
  const search = page.getByRole('searchbox', { name: '文字で探す' });
  await expect(search).toBeFocused();
  await search.fill(desc);
  await search.press('Enter');
  const rows = page.locator('.journal-table tbody tr');
  await expect(rows).toHaveCount(2);
  await page.keyboard.press('ArrowDown');
  await expect(rows.first()).toBeFocused();
  // 打ちかけ: Enter を押さずに ↓。探した結果の先頭の行へ（一覧の作り直しで、フォーカスを落とさない）。
  await search.focus();
  await search.fill(`${desc} 乙`);
  await page.keyboard.press('ArrowDown');
  await expect(page).toHaveURL(/q=/);
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText(`${desc} 乙`);
  await expect(rows.first()).toBeFocused();
});

// ---- 一覧の見た目（2026-10-15 の見た目の採点。幅390・768・1440） ----

/** 色の変数（tokens.css）の値を、computed の rgb(...) の形にする。 */
const tokenRgb = (page, name) =>
  page.evaluate((n) => {
    const hex = getComputedStyle(document.documentElement).getPropertyValue(n).trim().replace('#', '');
    const v = hex.length === 3 ? [...hex].map((c) => c + c).join('') : hex;
    return `rgb(${parseInt(v.slice(0, 2), 16)}, ${parseInt(v.slice(2, 4), 16)}, ${parseInt(v.slice(4, 6), 16)})`;
  }, name);

test('SC-05 一覧（幅1440）: 取引先は摘要の下の行に本文の色で出す。見出しは「金額（円）」と「詳細」。ページ送りは一覧の上にもある', async ({ page }) => {
  const desc = uniqueText('E2E 取引先の行');
  await createEntry(page, desc);
  await open(page, '#/journal');
  const table = page.locator('.journal-table');
  await expect(table.getByRole('columnheader', { name: '金額（円）' })).toHaveCount(1);
  await expect(table.getByRole('columnheader', { name: '詳細', exact: true })).toHaveCount(1);
  await expect(table.getByRole('columnheader', { name: '状態' })).toHaveCount(0);
  // 上と下のページ送りは同じもの（全件が50件を超える帳簿）。
  const top = page.getByRole('navigation', { name: 'ページ送り（一覧の上）' });
  const bottom = page.getByRole('navigation', { name: 'ページ送り（一覧の下）' });
  await expect(top).toContainText('1 / ');
  await expect(top).toHaveText(await bottom.textContent());
  const [topBox, tableBox] = await Promise.all([top.boundingBox(), table.boundingBox()]);
  expect(topBox.y + topBox.height).toBeLessThanOrEqual(tableBox.y);
  await top.getByRole('button', { name: /次の50件/ }).click();
  await expect(page).toHaveURL(/page=2/);
  await expect(top).toContainText('2 / ');
  // 取引先: 摘要の下の行で、摘要と同じ左の位置から始まる。色は本文の色。
  await page.goto(`/#/journal?q=${encodeURIComponent(desc)}`);
  const cell = table.locator('tbody tr').first().locator('.c-desc');
  const pos = await cell.evaluate((td) => {
    const d = td.querySelector('span').getBoundingClientRect();
    const p = td.querySelector('.journal-partner').getBoundingClientRect();
    return { below: p.top >= d.bottom - 1, sameLeft: Math.abs(p.left - d.left) < 1, color: getComputedStyle(td.querySelector('.journal-partner')).color };
  });
  expect(pos.below).toBe(true);
  expect(pos.sameLeft).toBe(true);
  expect(pos.color).toBe(await tokenRgb(page, '--c-text'));
});

test('SC-05 一覧（幅390）: 借方と貸方は別の行に出し、口座の名前は1行に入る。金額には「円」を添える', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/journal');
  const rows = page.locator('.journal-table tbody tr');
  await expect(rows.first()).toBeVisible();
  const checks = await rows.evaluateAll((trs) =>
    trs.slice(0, 20).map((tr) => {
      const d = tr.querySelector('.c-debit').getBoundingClientRect();
      const c = tr.querySelector('.c-credit').getBoundingClientRect();
      const name = tr.querySelector('.c-credit .keep');
      const lh = parseFloat(getComputedStyle(name).lineHeight);
      return { separate: c.top >= d.bottom - 1, oneLine: name.getBoundingClientRect().height < lh * 1.5, unit: getComputedStyle(tr.querySelector('.c-total .amount'), '::after').content };
    }),
  );
  for (const x of checks) {
    expect(x.separate).toBe(true);
    expect(x.oneLine).toBe(true);
    expect(x.unit).toBe('"円"');
  }
});

test('SC-05 詳細の履歴の見出しの下に、「記録のハッシュ」が何かを1行で書く', async ({ page }) => {
  const desc = uniqueText('E2E ハッシュの説明');
  const e = await createEntry(page, desc);
  await open(page, '#/journal');
  await page.goto(`/#/journal/${e.id}`);
  const drawer = page.getByRole('dialog', { name: `No.${e.no} の仕訳` });
  const note = drawer.locator('.detail-hash-note');
  await expect(note).toContainText('「記録のハッシュ」は、記録ごとの印です。');
  await expect(note).toContainText('書き換えられていないか');
  // 見出し「履歴」のすぐ下（履歴の版の上）。
  const [head, n, first] = await Promise.all([drawer.getByRole('heading', { name: '履歴' }).boundingBox(), note.boundingBox(), drawer.locator('.timeline-item').first().boundingBox()]);
  expect(n.y).toBeGreaterThanOrEqual(head.y + head.height - 1);
  expect(n.y + n.height).toBeLessThanOrEqual(first.y);
});
