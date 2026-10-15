// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の中身（2）: 試算表・総勘定元帳・消費税（SC-06、api-reports.md）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen, formatDate } from '../format.js';
import { treeRows, collapseSingleRows, chartCompare } from '../report-tree.js';
import { createTable } from '../components/table.js';
import { banner } from '../components/banner.js';
import { emptyState } from '../components/empty.js';
import { money } from '../components/money.js';
import { phrase } from '../components/phrase.js';
import { reportTable, yen, balanceBadge, reportFigures, reportNote } from './report-table.js';

const KINDS = [
  ['asset', '資産'],
  ['liability', '負債'],
  ['equity', '資本'],
  ['revenue', '収益'],
  ['expense', '費用'],
];

/**
 * 試算表: 期首・借方・貸方・期末の4つの数字の列。行は科目ごと（親の行は子の合計）。狭い枠では科目ごとのカード。
 * 子が1つだけの親は1行に畳む（「現金／現金」と同じ数字が2回並ばないように。貸借対照表と同じ決まり）。
 */
export async function trialView({ api, period, accounts }) {
  const res = await api.get('/reports/trial-balance', { from: period.from, to: period.to });
  const keys = ['opening', 'debit', 'credit', 'closing'];
  const body = [];
  for (const [kind, label] of KINDS) {
    const rows = res.rows.filter((r) => r.kind === kind);
    if (!rows.length) continue;
    body.push({ kind: 'group', name: label });
    for (const r of collapseSingleRows(treeRows(rows, accounts, keys))) body.push({ name: r.name, sub: r.sub, depth: r.depth, parent: r.parent, direct: r.direct, cells: keys.map((k) => yen(r[k])) });
  }
  body.push({ kind: 'grand', name: '動きの合計', cells: ['', yen(res.total_debit), yen(res.total_credit), ''] });
  const node = h(
    'div',
    { class: 'report-body' },
    h('p', { class: 'report-check' }, balanceBadge(res.balanced, '借方と貸方の合計、期末の残高の貸借が一致しています', '貸借が一致していません。設定の「データ」で台帳を検査してください')),
    res.rows.length
      ? reportTable({ caption: '試算表', head: ['科目', '期首', '借方', '貸方', '期末'], body, fold: 'cards', keyCol: 3 })
      : h('div', { class: 'card' }, emptyState({ iconName: 'book', title: 'この期間には、残高も動きもありません', body: '期間を変えるか、取引を登録してください。' })),
    reportNote([
      '期首と期末は、科目のふだんの側から見た残高です。',
      '資産と費用は借方−貸方、負債・資本・収益は貸方−借方で数えます。',
      '前の年までの利益は、元入金の期首に入れています。',
    ]),
  );
  return { node, csv: { path: '/export/trial-balance.csv', params: { from: period.from, to: period.to } } };
}

/**
 * 摘要のセル。2行目に、No（仕訳へのリンク）と相手科目を小さく出す。
 * 幅1024px未満では、No と相手科目の列を畳んで、この2行目で見せる（640px未満は日付もここ）。広い幅では2行目を隠す。
 */
function ledgerDescription(r) {
  const link = h('a', { href: `#/journal/${encodeURIComponent(r.entry_id)}`, tabindex: '-1' }, `No.${r.no}`);
  return [
    h('span', { class: 'lg-desc-text keep' }, r.description || '（摘要なし）'),
    h('span', { class: 'lg-sub' }, h('span', { class: 'lg-sub-date' }, formatDate(r.date), ' · '), link, ' · ', h('span', { class: 'keep' }, r.counter)),
  ];
}

/**
 * 日付のセル。曜日は別の塊にして、640〜1023pxでは日付の下に置く（行は摘要の2行目で2段になっているので高さは
 * 変わらない。日付の列を細くして、摘要の列に幅を回す。幅768で、長い相手科目の行の摘要が窮屈に見えた）。
 */
function ledgerDate(r) {
  const [ymd, dow] = formatDate(r.date).split(' ');
  return dow ? [h('span', { class: 'lg-ymd' }, ymd), ' ', h('span', { class: 'lg-dow' }, dow)] : ymd;
}

/**
 * 口座をまとめる科目（「普通預金・その他の預金」など）を選んだときの一言と、下の口座ごとの元帳へのリンク。
 * 元帳の行は、記帳した科目ごと（口座の取引は口座の側に入る）なので、親の科目だけを見ると動きが無いように見えた
 * （試算表では、同じ見出しの下に金額がある）。リンクの先も、いまと同じ期間。
 * @param {Array<object>} kids 下の科目（口座）
 * @param {boolean} direct この科目への直接の記帳があるか
 * @param {(code: string) => string} hrefFor 元帳のアドレス
 */
function childrenNote(kids, direct, hrefFor) {
  const word = kids.every((a) => a.wallet) ? '口座' : '科目';
  // 名前は名前のまま出す（文ごと phrase に通すと、塊の末尾の空白が見えなくなり「サンプル銀行普通預金」になった）。
  const names = kids.map((a) => h('span', { class: 'keep' }, `「${a.name}」`));
  const lead = direct
    ? [phrase(`この表は、この科目への直接の記帳だけです。${word}`), names, phrase(`の動きは、${kids.length > 1 ? 'それぞれの' : 'その'}${word}の元帳にあります。`)]
    : [phrase(`この科目の動きは、${word}`), names, phrase('の側にあります。')];
  return h(
    'div',
    { class: 'ledger-children', role: 'note' },
    h('p', { class: 'ledger-children-text' }, icon('info', { size: 16 }), h('span', null, lead)),
    h(
      'ul',
      { class: 'ledger-children-links', 'aria-label': `${word}の元帳` },
      kids.map((a) => h('li', null, h('a', { class: 'btn btn--sm ledger-child-link', href: hrefFor(a.code) }, icon('book', { size: 16 }), h('span', { class: 'keep' }, `${a.name}の元帳`)))),
    ),
  );
}

/**
 * 総勘定元帳: 科目を選ぶと、日付順の行と残高の推移。行から仕訳の詳細に飛べる（Enter、またはNoのリンク）。
 * 行（tr）には、仕訳の id を data-entry-id で持たせる（詳細を閉じて元帳へ戻ったとき、開いた行へフォーカスを戻すため）。
 * @param {{ api: any, period: object, account: string|null, accounts: any[], navigate: (hash: string) => void,
 *           ledgerHref: (code: string) => string }} o
 */
export async function ledgerView({ api, period, account, accounts = [], navigate, ledgerHref }) {
  if (!account) {
    return { node: h('div', { class: 'card' }, emptyState({ iconName: 'list', title: '科目を選んでください', body: '上の「科目」の欄で選ぶと、その科目の動きと残高の推移が出ます。口座を選ぶと、口座の出入りが見られます。' })), csv: null };
  }
  const res = await api.get('/reports/ledger', { account, from: period.from, to: period.to });
  const kids = accounts.filter((a) => a.parent === account).sort(chartCompare);
  const table = createTable({
    caption: '総勘定元帳（行で Enter を押すと、その仕訳を開く）',
    className: 'ledger-table',
    rowKey: (r) => `${r.entry_id}-${r.no}-${r.balance}`,
    onOpen: (r) => navigate(`#/journal/${encodeURIComponent(r.entry_id)}`),
    columns: [
      { key: 'date', label: '日付', className: 'lg-date', headClass: 'lg-date', render: ledgerDate },
      { key: 'no', label: 'No', num: true, className: 'col-wide lg-no', headClass: 'col-wide lg-no', render: (r) => h('a', { href: `#/journal/${encodeURIComponent(r.entry_id)}`, tabindex: '-1' }, `No.${r.no}`) },
      { key: 'description', label: '摘要', stackLabel: '', className: 'wrap lg-desc', render: ledgerDescription },
      { key: 'counter', label: '相手科目', className: 'col-wide keep', headClass: 'col-wide', render: (r) => r.counter },
      { key: 'debit', label: '借方', num: true, render: (r) => (r.debit ? formatYen(r.debit) : '') },
      { key: 'credit', label: '貸方', num: true, render: (r) => (r.credit ? formatYen(r.credit) : '') },
      { key: 'balance', label: '残高', num: true, className: 'lg-balance', render: (r) => formatYen(r.balance) },
    ],
    rows: res.rows,
  });
  for (const tr of table.tbody.querySelectorAll('tr[data-index]')) tr.dataset.entryId = res.rows[Number(tr.dataset.index)].entry_id;
  const empty = kids.length
    ? emptyState({ iconName: 'book', title: 'この期間には、この科目への直接の記帳がありません', body: '動きは、上のリンクの元帳で見られます。' })
    : emptyState({ iconName: 'book', title: 'この期間には、この科目の動きがありません', body: '期間を変えるか、別の科目を選んでください。' });
  const node = h(
    'div',
    { class: 'report-body' },
    kids.length && ledgerHref ? childrenNote(kids, res.rows.length > 0, ledgerHref) : null,
    h(
      'dl',
      { class: 'ledger-facts' },
      h('div', null, h('dt', null, '期首の残高'), h('dd', null, money(res.opening))),
      h('div', null, h('dt', null, '期間の行'), h('dd', null, h('span', { class: 'money' }, h('span', { class: 'num' }, String(res.rows.length)), h('span', { class: 'unit' }, '行')))),
      h('div', { class: 'is-main' }, h('dt', null, '期末の残高'), h('dd', null, money(res.closing))),
    ),
    res.rows.length ? table.el : h('div', { class: 'card' }, empty),
  );
  /** j・k で行を動かす（仕訳帳と同じ）。行が無ければ偽（キーを全体に回す）。 */
  const moveRow = (n) => {
    if (!res.rows.length) return false;
    const i = table.focusedIndex();
    table.focusRow(i < 0 ? 0 : i + n);
    return true;
  };
  return { node, csv: { path: '/export/ledger.csv', params: { account, from: period.from, to: period.to } }, moveRow };
}

// 消費税の注意の文（サーバーの文）に出る言葉の、短い説明。割合や上限の額は年で変わるので、数字は書かない。
const TAX_TERMS = [
  ['経過措置', '登録番号の無い仕入先（免税事業者など）からの仕入れでも、決まった割合だけ税額を差し引ける、期間を区切った特例です。'],
  ['一の仕入先ごとの上限', '同じ仕入先からの仕入れが、年に決まった額を超えると、超えた分にはこの特例を使えない決まりです。'],
];

/** 注意の文に出てくる言葉だけを、文のすぐ後で説明する（初めて出たところで分かるように）。 */
function taxTerms(text) {
  const used = TAX_TERMS.filter(([term]) => String(text || '').includes(term));
  if (!used.length) return null;
  return h(
    'dl',
    { class: 'tax-terms', 'aria-label': '言葉の説明' },
    used.map(([term, desc]) => h('div', null, h('dt', null, h('dfn', null, term), '：'), h('dd', null, phrase(desc)))),
  );
}

/**
 * 「参考値」の注意の本文と言葉の説明。どの幅でも「くわしく」に畳み、帯は1行の要点（見出し）だけにする（長い注意書きが
 * 最初に目に入り、答えの納付額の見込みに着くのが遅れた。幅1440でも同じだった）。開くと、本文と言葉の説明が出る。
 */
function disclaimerMore(text) {
  return h('details', { class: 'tax-more' }, h('summary', { class: 'tap' }, icon('chevron-right', { size: 16, className: 'tax-more-icon' }), 'くわしく'), h('div', { class: 'tax-more-body' }, h('p', null, phrase(text)), taxTerms(text)));
}

/**
 * 消費税: 結論（納付額の見込み）を大きく先に出し、税区分ごとの税込の金額と税額、経過措置の内訳を並べる。
 * 広い枠では、税区分の表を左、結論を右に置く。上に「参考値」の注意（藍の情報の帯）を常に出す（1行の要点と「くわしく」）。
 * 帯の文は塊で折る（「反映していま／せん」「保存、一／の仕入先」のような折れ方を避ける）。
 */
export async function taxView({ api, period, settings }) {
  const res = await api.get('/reports/tax-summary', { from: period.from, to: period.to });
  const notices = [];
  if (settings && settings.tax_mode === 'exempt') {
    notices.push(banner({ kind: 'info', title: phrase('設定上は免税事業者として表示しています。申告の要否は、国税庁の案内か税理士に確かめてください'), body: phrase('課税事業者になったときは、設定の「事業者」で課税の区分を変えてください。') }));
  }
  notices.push(banner({ kind: 'info', title: phrase('参考値。簡易課税・特例は反映していません'), extra: disclaimerMore(res.disclaimer) }));
  const byCode = reportTable({
    caption: '税区分ごとの税込の金額と税額',
    head: ['税区分', '税込の金額', '含まれる税額'],
    fold: 'cards',
    keyCol: 1,
    body: res.rows.length ? res.rows.map((r) => ({ name: r.label, cells: [yen(r.gross), yen(r.tax)] })) : [{ name: 'この期間には、税区分のある取引がありません', cells: ['', ''] }],
  });
  const figures = reportFigures([
    { label: '納付額の見込み', hint: '参考・本則課税', value: res.estimate, main: true },
    { label: '売上税額', value: res.sales_tax },
    { label: '仕入税額', hint: '経過措置の控除を含む', value: res.purchase_tax },
  ], { className: 'tax-sums' });
  const trans = res.transitional.length
    ? reportTable({
        caption: '経過措置の内訳（免税事業者等からの課税仕入れ）',
        head: ['控除の割合', '支払額（税込）', '含まれる税額', '控除できる額'],
        fold: 'cards',
        keyCol: 2,
        body: res.transitional.map((t) => ({ name: `${t.percent}%`, cells: [yen(t.gross), yen(t.tax), yen(t.deductible)] })),
      })
    : null;
  const node = h(
    'div',
    { class: 'report-body' },
    notices,
    h(
      'div',
      { class: 'tax-layout' },
      h('section', { class: 'tax-result', 'aria-label': '結果' }, figures),
      h('section', { class: 'tax-detail', 'aria-label': '税区分ごとの内訳' }, byCode, trans ? h('section', { class: 'report-section', 'aria-labelledby': 'tax-trans-title' }, h('h3', { id: 'tax-trans-title' }, '経過措置の内訳'), trans) : null),
    ),
    // サーバーは、仕訳の明細ごとの税額（登録のときに1円未満を処理した額）を足している（kamoku-books の tax_summary）。
    // 経過措置の仕入れだけは、税額に控除できる割合を掛け、0へ向けて切り捨てた額を足す（通常の税額は足さない）。
    // 税込の合計の10/110と数十円ちがい、理由が画面に無かった。
    reportNote('売上税額は、明細ごとの税額の合計です。仕入税額は、通常の仕入れは明細ごとの税額の合計、経過措置の仕入れは、税額に控除できる割合を掛けた額の合計です。どちらも、税込の合計から計算し直すと、1円未満の処理の分だけ違うことがあります。'),
  );
  return { node, csv: { path: '/export/tax-summary.csv', params: { from: period.from, to: period.to } } };
}
