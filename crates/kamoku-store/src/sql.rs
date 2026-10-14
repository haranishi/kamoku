// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

pub(crate) fn unsigned(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
pub(crate) fn integer(value: u64) -> Result<i64, crate::StoreError> {
    i64::try_from(value).map_err(|_| crate::StoreError::AmountRange)
}
