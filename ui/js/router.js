// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// アドレスの #/ 以降で画面を切り替える。画面は mount(root, ctx) を持ち、片付けの関数を返す。
// 片付けの関数に update(ctx) を生やしておくと、同じ画面のまま条件だけ変わったとき（絞り込み、引き出しの開閉）に
// 作り直さずにそれを呼ぶ。入力中の値やフォーカスを失わないため。

/**
 * 「#/journal/e_1?from=2026-10-01」を { path: '/journal/e_1', query: { from: '2026-10-01' } } に分ける。
 * @param {string} hash
 */
export function parseHash(hash) {
  const raw = String(hash || '').replace(/^#/, '');
  const [pathPart, search = ''] = raw.split('?');
  const path = pathPart.startsWith('/') ? pathPart : `/${pathPart}`;
  const query = {};
  for (const [k, v] of new URLSearchParams(search)) query[k] = v;
  return { path: path === '' ? '/' : path, query };
}

/** parseHash の逆。空の値は書かない。 */
export function buildHash(path, query = {}) {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(query)) {
    if (v === undefined || v === null || v === '') continue;
    q.set(k, String(v));
  }
  const text = q.toString();
  return `#${path}${text ? `?${text}` : ''}`;
}

/**
 * 経路の型（'/journal/:id' など）と、アドレスの道を突き合わせる。
 * @returns {{ route: object, params: Record<string,string> } | null}
 */
export function matchRoute(routes, path) {
  const segs = path.split('/').filter(Boolean);
  for (const route of routes) {
    const pat = route.path.split('/').filter(Boolean);
    if (pat.length !== segs.length) continue;
    const params = {};
    let ok = true;
    for (let i = 0; i < pat.length; i += 1) {
      if (pat[i].startsWith(':')) params[pat[i].slice(1)] = decodeURIComponent(segs[i]);
      else if (pat[i] !== segs[i]) {
        ok = false;
        break;
      }
    }
    if (ok) return { route, params };
  }
  return null;
}

/**
 * 画面の切り替え役を作る。
 * guard(path) が移り先（'#/setup' など）を返したら、その画面へ置き換える（初期設定が済むまで、ほかの画面に入れないため）。
 * @param {{ routes: object[], outlet: HTMLElement, notFound: object, makeContext: (base: object) => object, onChange?: (cur: object) => void,
 *           guard?: (path: string) => string|null }} opts
 */
export function createRouter({ routes, outlet, notFound, makeContext, onChange, guard }) {
  let current = null;

  function titleOf(route, ctx) {
    return typeof route.title === 'function' ? route.title(ctx) : route.title;
  }

  function setDocumentTitle(text) {
    document.title = text ? `${text} - kamoku` : 'kamoku';
  }

  function render() {
    const { path, query } = parseHash(location.hash || '#/');
    const redirect = guard ? guard(path) : null;
    if (redirect && redirect !== location.hash) {
      history.replaceState(null, '', redirect);
      render();
      return;
    }
    const m = matchRoute(routes, path) || { route: notFound, params: {} };
    const ctx = makeContext({ route: m.route, params: m.params, query, path, setTitle: setDocumentTitle });

    if (current && current.route.screen === m.route.screen && current.cleanup && typeof current.cleanup.update === 'function') {
      if (current.cleanup.update(ctx) !== false) {
        current = { ...current, route: m.route, params: m.params, query, path };
        setDocumentTitle(titleOf(m.route, ctx));
        if (onChange) onChange(current);
        return;
      }
    }
    if (current && typeof current.cleanup === 'function') current.cleanup();
    outlet.replaceChildren();
    window.scrollTo(0, 0);
    setDocumentTitle(titleOf(m.route, ctx));
    const cleanup = m.route.screen.mount(outlet, ctx);
    current = { route: m.route, params: m.params, query, path, cleanup };
    // 画面を替えたら、本文の見出しへフォーカスを移す（keyboard.md）。読み上げが新しい画面の名前から始まる。
    const h1 = outlet.querySelector('h1');
    if (h1) {
      if (!h1.hasAttribute('tabindex')) h1.setAttribute('tabindex', '-1');
      h1.focus({ preventScroll: true });
    }
    if (onChange) onChange(current);
  }

  return {
    start() {
      window.addEventListener('hashchange', render);
      render();
    },
    /** 画面を移る。replace なら履歴を足さない（戻る操作で戻らない）。 */
    navigate(hash, { replace = false } = {}) {
      const target = hash.startsWith('#') ? hash : `#${hash}`;
      if (target === location.hash) return render();
      if (replace) {
        history.replaceState(null, '', target);
        render();
      } else {
        location.hash = target;
      }
      return undefined;
    },
    current: () => current,
    rerender: render,
  };
}
