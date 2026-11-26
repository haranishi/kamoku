// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// お試し版の起動。WASM の帳簿を開き、/api/v1 の fetch だけを差し替えてから画面を始める。
// 帳簿の WASM も動的に読む。静的な import だと、読めなかったときに下の案内まで届かない。

const originalFetch = window.fetch.bind(window);

function aborted(signal) {
  if (signal?.aborted) {
    throw new DOMException('The operation was aborted.', 'AbortError');
  }
}

async function bytesOf(body, signal) {
  aborted(signal);
  if (body == null) return new Uint8Array();
  if (typeof body === 'string') return new TextEncoder().encode(body);
  if (body instanceof Uint8Array) return body;
  if (body instanceof ArrayBuffer) return new Uint8Array(body);
  if (ArrayBuffer.isView(body)) {
    return new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
  }
  if (typeof Blob !== 'undefined' && body instanceof Blob) {
    const buffer = await body.arrayBuffer();
    aborted(signal);
    return new Uint8Array(buffer);
  }
  throw new TypeError('対応していない本文です');
}

// 帳簿の WASM が例外（panic）を投げたら、以後は呼ばない。止まった帳簿は中身の保証が無いため。
const STOPPED_MESSAGE = 'お試し版の帳簿が止まりました。ページを再読み込みしてください。';
let stopped = false;

function stoppedResponse() {
  // サーバーの想定外の失敗（ApiError::internal）と同じ形の本文。
  const body = JSON.stringify({ error: { code: 'internal', message: STOPPED_MESSAGE } });
  return new Response(body, {
    status: 500,
    headers: { 'Content-Type': 'application/json; charset=utf-8' },
  });
}

function installFetch(book) {
  window.fetch = async (input, init) => {
    const request = input instanceof Request ? input : null;
    const raw = request ? request.url : input instanceof URL ? input.href : String(input);
    const url = new URL(raw, window.location.href);
    // 画面は同じオリジンの絶対パス /api/v1/... を呼ぶ。サブパスに置いてもこの形のまま。
    if (url.origin !== window.location.origin || !url.pathname.startsWith('/api/v1/')) {
      return originalFetch(input, init);
    }
    const initObj = init ?? {};
    const signal = initObj.signal ?? request?.signal;
    aborted(signal);
    const method = (initObj.method || request?.method || 'GET').toUpperCase();
    const headers = new Headers(initObj.headers ?? (request ? request.headers : undefined));
    const contentType = headers.has('content-type') ? headers.get('content-type') : null;
    let body = new Uint8Array();
    if (initObj.body != null) {
      body = await bytesOf(initObj.body, signal);
    } else if (request && request.method !== 'GET' && request.method !== 'HEAD') {
      body = await bytesOf(await request.blob(), signal);
    }
    const query = url.search.startsWith('?') ? url.search.slice(1) : '';
    if (stopped) return stoppedResponse();
    let reply;
    try {
      reply = book.handle(method, url.pathname, query, contentType, body);
    } catch (error) {
      stopped = true;
      console.error('kamoku: お試し版の帳簿が止まりました', error);
      return stoppedResponse();
    }
    aborted(signal);
    const outHeaders = new Headers();
    for (const key of Object.keys(reply.headers)) {
      outHeaders.set(key, reply.headers[key]);
    }
    // HEAD と、本文を持てない状態には本文を付けない（Response が例外を投げるため）。
    const empty = method === 'HEAD' || [101, 204, 205, 304].includes(reply.status);
    return new Response(empty ? null : reply.body, { status: reply.status, headers: outHeaders });
  };
}

// ダウンロードのリンクは /api/v1/ を指す。修飾キーつきのクリックや中ボタンは、画面の処理を通らずに
// ブラウザが本物の要求（新しいタブ・ダウンロード）を出す。配信先に /api/ は無いので、通常のクリックに置き換える。
const replayed = new WeakSet();

function apiLink(event) {
  const target = event.target instanceof Element ? event.target.closest('a[href^="/api/v1/"]') : null;
  return target instanceof HTMLAnchorElement ? target : null;
}

function replayAsPlainClick(link) {
  const plain = new MouseEvent('click', { bubbles: true, cancelable: true, composed: true, button: 0 });
  replayed.add(plain);
  link.dispatchEvent(plain);
}

function guardApiLinks() {
  document.addEventListener(
    'click',
    (event) => {
      if (replayed.has(event)) return;
      const link = apiLink(event);
      if (!link) return;
      if (event.button === 0 && !(event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      replayAsPlainClick(link);
    },
    true,
  );
  document.addEventListener(
    'auxclick',
    (event) => {
      const link = apiLink(event);
      if (!link) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (event.button === 1) replayAsPlainClick(link);
    },
    true,
  );
  // 置き換えたクリックを画面が処理しなかったときも、リンク先へは移らない。
  window.addEventListener('click', (event) => {
    if (replayed.has(event)) event.preventDefault();
  });
}

function fail() {
  const main = document.getElementById('main');
  const message = document.createElement('p');
  message.setAttribute('role', 'alert');
  message.textContent = '画面を読み込めませんでした。WebAssemblyに対応したブラウザで、ページを再読み込みしてください。';
  main?.replaceChildren(message);
  document.getElementById('boot-status')?.remove();
}

try {
  const { default: initBook, DemoBook } = await import('./pkg/kamoku_web.js');
  await initBook();
  const book = new DemoBook();
  installFetch(book);
  guardApiLinks();
  const ui = await import('../rust/pkg/kamoku_ui.js');
  await ui.default();
  document.getElementById('boot-status')?.remove();
} catch (error) {
  console.error('kamoku: お試し版を起動できませんでした', error);
  fail();
}
