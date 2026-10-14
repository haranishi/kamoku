// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! 標準の科目の一覧（`docs/accounts.md`）を標準出力に書く。
//!
//! ```text
//! cargo run -p kamoku-books --example accounts_doc > docs/accounts.md
//! ```

fn main() {
    print!("{}", kamoku_books::standard_accounts_markdown());
}
