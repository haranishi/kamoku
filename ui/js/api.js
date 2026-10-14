// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// サーバーとの通信は、このファイルの fetch だけで行う（check-policy が、ほかの場所の fetch を落とす）。
// 送り先は同じオリジンの /api/v1 だけ。外へは何も送らない。

const BASE = '/api/v1';

/** 失敗の応え（{ error: { code, message, field } }）、または通信そのものの失敗。 */
export class ApiError extends Error {
  /**
   * @param {{ status: number, code: string, message: string, field?: string|null }} p
   */
  constructor({ status, code, message, field = null }) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
    this.field = field;
  }

  /** 通信が届かなかった（サーバーが止まっているなど）。 */
  get isNetwork() {
    return this.code === 'network';
  }

  /** ほかの場所で台帳が更新された（409 stale / stale_revision）。読み込み直す合図。 */
  get isStale() {
    return this.code === 'stale' || this.code === 'stale_revision';
  }

  /** 読み取り専用で断られた。 */
  get isReadOnly() {
    return this.code === 'read_only';
  }
}

const FALLBACK = {
  400: '送った内容の形が正しくありません。入力を確かめてください。',
  401: '合言葉の期限が切れました。サーバーを起動したときに出たアドレスから、開き直してください。',
  403: 'ほかのページからの操作として断られました。kamokuの画面から操作してください。',
  404: '見つかりませんでした。一覧から選び直してください。',
  409: 'いまの状態と合わないため、断られました。画面を読み込み直してください。',
  413: '送る内容が大きすぎます。',
  421: '開いたアドレスが違います。起動のときに出たアドレスから開いてください。',
  422: '入力が決まりに合いません。入力を確かめてください。',
  500: 'サーバーで想定外の失敗がありました。もう一度試してください。',
};

const listeners = new Set();

/** 失敗のたびに呼ばれる関数を登録する（読み取り専用の帯を出すため）。片付けの関数を返す。 */
export function onApiError(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function query(params) {
  if (!params) return '';
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null || v === '') continue;
    q.set(k, String(v));
  }
  const text = q.toString();
  return text ? `?${text}` : '';
}

async function request(method, path, { params, body, bytes, signal } = {}) {
  const init = { method, headers: { Accept: 'application/json' }, credentials: 'same-origin', signal };
  if (bytes !== undefined) {
    // 取込のファイルは、手を加えずにバイト列のまま送る（文字コードの判定はサーバーの仕事）。
    init.headers['Content-Type'] = 'application/octet-stream';
    init.body = bytes;
  } else if (body !== undefined) {
    init.headers['Content-Type'] = 'application/json';
    init.body = JSON.stringify(body);
  }
  let res;
  try {
    res = await fetch(`${BASE}${path}${query(params)}`, init);
  } catch (e) {
    if (e && e.name === 'AbortError') throw e;
    const err = new ApiError({
      status: 0,
      code: 'network',
      message: 'サーバーにつながりません。kamokuのサーバーが動いているかを確かめて、もう一度試してください。',
    });
    listeners.forEach((fn) => fn(err));
    throw err;
  }
  if (res.status === 204) return null;
  let data = null;
  try {
    data = await res.json();
  } catch {
    data = null;
  }
  if (!res.ok) throw failure(res, data);
  return data;
}

/** 失敗の応えから ApiError を作り、見張りの関数に知らせる。 */
function failure(res, data) {
  const e = data && data.error ? data.error : {};
  const err = new ApiError({
    status: res.status,
    code: e.code || `http_${res.status}`,
    message: e.message || FALLBACK[res.status] || FALLBACK[500],
    field: e.field || null,
  });
  listeners.forEach((fn) => fn(err));
  return err;
}

/** Content-Disposition からファイル名を読む（filename* を優先）。 */
function fileName(disposition) {
  const star = /filename\*=UTF-8''([^;]+)/i.exec(disposition || '');
  if (star) {
    try {
      return decodeURIComponent(star[1]);
    } catch {
      return null;
    }
  }
  const plain = /filename="?([^";]+)"?/i.exec(disposition || '');
  return plain ? plain[1] : null;
}

/**
 * 書き出しのファイルを受け取る。中身・ファイル名・サーバーの警告（Shift_JIS に無い文字を ? にした数）を返す。
 * url は apiUrl() で作ったアドレス。失敗の応えは、ほかの要求と同じ ApiError にする。
 * @returns {Promise<{ blob: Blob, filename: string|null, unmappable: number }>}
 */
async function fetchFile(url) {
  if (!url.startsWith(`${BASE}/`)) throw new Error('書き出しのアドレスではありません。');
  let res;
  try {
    res = await fetch(url, { credentials: 'same-origin' });
  } catch {
    const err = new ApiError({ status: 0, code: 'network', message: 'サーバーにつながりません。kamokuのサーバーが動いているかを確かめて、もう一度試してください。' });
    listeners.forEach((fn) => fn(err));
    throw err;
  }
  if (!res.ok) {
    let data = null;
    try {
      data = await res.json();
    } catch {
      data = null;
    }
    throw failure(res, data);
  }
  const warning = /unmappable=(\d+)/.exec(res.headers.get('X-Kamoku-Warning') || '');
  return { blob: await res.blob(), filename: fileName(res.headers.get('Content-Disposition')), unmappable: warning ? Number(warning[1]) : 0 };
}

/**
 * 例: await api.get('/entries', { from: '2026-10-01', limit: 50 })
 *     await api.post('/entries', input)
 */
export const api = {
  get: (path, params, opts = {}) => request('GET', path, { ...opts, params }),
  post: (path, body, opts = {}) => request('POST', path, { ...opts, body }),
  put: (path, body, opts = {}) => request('PUT', path, { ...opts, body }),
  patch: (path, body, opts = {}) => request('PATCH', path, { ...opts, body }),
  del: (path, opts = {}) => request('DELETE', path, opts),
  /** ファイルを送る。例: await api.upload('/imports/upload', { name: 'a.csv', encoding: 'auto' }, file) */
  upload: (path, params, bytes, opts = {}) => request('POST', path, { ...opts, params, bytes }),
  /** 書き出しのファイルを受け取る。例: await api.file(apiUrl('/export/journal.csv', { encoding: 'sjis' })) */
  file: (url) => fetchFile(url),
};

/**
 * 書き出し（CSV・JSON・バックアップ）のアドレス。画面は <a href download> に入れる。押したときは
 * components/download.js が api.file で受け取って保存させる（警告や失敗を日本語で知らせるため）。
 * 例: apiUrl('/export/journal.csv', { state: 'active', encoding: 'sjis' })
 */
export function apiUrl(path, params) {
  return `${BASE}${path}${query(params)}`;
}

/** 再送防止の印（client_id）を作る。フォームを開いたときに作り、保存に成功したら作り直す。 */
export function newClientId() {
  const bytes = new Uint8Array(12);
  crypto.getRandomValues(bytes);
  return `c_${[...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')}`;
}
