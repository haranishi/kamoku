// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 最初の描画の前に、保存した明暗と密度を <html> に当てる（モジュールではない普通のスクリプト）。
// ES modules は描画の後に動くため、手動でダークを選んだ人の画面が一瞬白くなるのを防ぐ。
// 鍵の名前は theme.js と同じ。
(function bootAppearance() {
  var root = document.documentElement;
  try {
    var theme = localStorage.getItem('kamoku.theme');
    if (theme === 'light' || theme === 'dark') root.setAttribute('data-theme', theme);
    if (localStorage.getItem('kamoku.density') === 'compact') root.setAttribute('data-density', 'compact');
  } catch (e) {
    // 保存を読めない環境では、OSの設定と標準の密度のままにする。
  }
})();
