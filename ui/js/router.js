// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// アドレスの #/ 以降で画面を切り替える。画面は mount(root, ctx) を持ち、片付けの関数を返す。
// 片付けの関数に update(ctx) を生やしておくと、同じ画面のまま条件だけ変わったとき（絞り込み、引き出しの開閉）に
// 作り直さずにそれを呼ぶ。入力中の値やフォーカスを失わないため。
// 片付けの関数に canLeave(to) を生やしておくと、ほかの画面へ移る前に呼ぶ。保存していない入力を、黙って捨てないため。
//   真なら移る。偽なら残る。Promise なら、答えが出るまでアドレスを元に戻しておき、真になったら行こうとした先へ進む。
//   to が null のときは、タブを閉じる・読み込み直すとき（小窓を出せない）。真偽だけを返し、偽ならブラウザの標準の確認が出る。
// 片付けの関数の managesFocus が真なら、見出しへフォーカスを移さない（画面が、準備ができたら最初の欄へ移す）。
// 画面には、1つ前の履歴のアドレス（ctx.prevHash）も渡す。仕訳の詳細の板を閉じたとき、板を開く前の画面へ戻すため（C2）。

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
  // 表示中の画面の、履歴の中の位置（history.state の kamokuIdx）とアドレス。戻る・進むで離れようとしたとき、
  // 何歩動いたかが分かれば、確認の間だけ同じ歩数で戻しておける（履歴の行を書き換えずに済む）。
  let shownIdx = 0;
  let shownHash = null;
  let reverting = false;
  let approved = null;
  let asking = false;
  // 履歴の位置ごとに、出したアドレスを覚える（1つ前の履歴が何の画面かを、画面が知るため）。読み込み直すと消える。
  const seen = new Map();

  function titleOf(route, ctx) {
    return typeof route.title === 'function' ? route.title(ctx) : route.title;
  }

  function setDocumentTitle(text) {
    document.title = text ? `${text} - kamoku` : 'kamoku';
  }

  const idxOf = (st) => (st && Number.isInteger(st.kamokuIdx) ? st.kamokuIdx : null);

  // 印の無い履歴の行は、リンクやアドレスの書き換えで、いまの画面のすぐ後ろに積まれたもの（前に進む移動は、先の行を消す）。
  function stamp() {
    const idx = idxOf(history.state);
    if (idx !== null) shownIdx = idx;
    else {
      shownIdx += 1;
      history.replaceState({ kamokuIdx: shownIdx }, '', location.href);
    }
    shownHash = location.hash || '#/';
    seen.set(shownIdx, shownHash);
  }

  function render() {
    const { path, query } = parseHash(location.hash || '#/');
    const redirect = guard ? guard(path) : null;
    if (redirect && redirect !== location.hash) {
      history.replaceState(history.state, '', redirect);
      render();
      return;
    }
    stamp();
    const m = matchRoute(routes, path) || { route: notFound, params: {} };
    const ctx = makeContext({ route: m.route, params: m.params, query, path, setTitle: setDocumentTitle, back, prevHash: seen.get(shownIdx - 1) || null });

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
    if (h1 && !(cleanup && cleanup.managesFocus)) {
      if (!h1.hasAttribute('tabindex')) h1.setAttribute('tabindex', '-1');
      h1.focus({ preventScroll: true });
    }
    if (onChange) onChange(current);
  }

  function canLeave(to) {
    const fn = current && current.cleanup && current.cleanup.canLeave;
    return typeof fn === 'function' ? fn(to) : true;
  }

  // 確認の間、アドレスを表示中の画面に戻す。戻る・進むなら同じ歩数で逆へ、リンクなら1歩戻る。動かした歩数を返す。
  function putBack() {
    const idx = idxOf(history.state);
    const delta = idx === null ? 1 : idx - shownIdx;
    if (delta === 0) history.replaceState(history.state, '', shownHash);
    else {
      reverting = true;
      history.go(-delta);
    }
    return delta;
  }

  function onHashChange() {
    const target = location.hash || '#/';
    if (reverting) {
      // 自分で戻した移動。行き着いた先が表示中の画面と違う（履歴の印が崩れた）ときは、アドレスだけを合わせる。
      reverting = false;
      if (target !== shownHash) history.replaceState(history.state, '', shownHash);
      return;
    }
    if (approved !== null && target === approved) {
      approved = null;
      render();
      return;
    }
    // 確認の小窓が開いている間の移動（戻るボタンなど）は、元に戻すだけにする（小窓を重ねない）。
    if (asking) {
      putBack();
      return;
    }
    const { path, query } = parseHash(target);
    const verdict = canLeave({ hash: target, path, query });
    if (verdict === true) {
      render();
      return;
    }
    const delta = putBack();
    if (!verdict) return;
    asking = true;
    Promise.resolve(verdict).then(
      (ok) => {
        asking = false;
        if (!ok) return;
        approved = target;
        if (delta === 0) {
          history.replaceState(history.state, '', target);
          onHashChange();
        } else history.go(delta);
      },
      () => {
        asking = false;
      },
    );
  }

  /** 画面を移る。replace なら履歴を足さない（戻る操作で戻らない）。replace の移動は、離れる前の確認をしない（初期設定へ移すなど）。 */
  function navigate(hash, { replace = false } = {}) {
    const target = hash.startsWith('#') ? hash : `#${hash}`;
    if (target === location.hash) return render();
    if (replace) {
      history.replaceState(history.state, '', target);
      render();
    } else {
      location.hash = target;
    }
    return undefined;
  }

  /** 来た画面へ戻る。この画面より前に、アプリの画面が履歴に無ければ（アドレスを直に開いたなど）、fallback へ移る。 */
  function back(fallback) {
    if (shownIdx > 1) history.back();
    else navigate(fallback, { replace: true });
  }

  return {
    start() {
      window.addEventListener('hashchange', onHashChange);
      // タブを閉じる・読み込み直すときは、小窓を出せないので、ブラウザの標準の確認に任せる。
      window.addEventListener('beforeunload', (e) => {
        if (canLeave(null) !== false) return;
        e.preventDefault();
        e.returnValue = '';
      });
      render();
    },
    navigate,
    back,
    current: () => current,
    rerender: render,
  };
}
