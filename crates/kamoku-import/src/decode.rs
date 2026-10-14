// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::ImportError;
use encoding_rs::{DecoderResult, Encoding, SHIFT_JIS, UTF_8, UTF_16BE, UTF_16LE};
use serde::{Deserialize, Serialize};

/// 文字コードの指定（`?encoding=auto|utf-8|shift_jis`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum EncodingChoice {
    /// BOMと、UTF-8として正しいかで判定する。
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// UTF-8として読む（BOMがあれば除く）。
    #[serde(rename = "utf-8")]
    Utf8,
    /// シフトJIS（Windows-31J）として読む。
    #[serde(rename = "shift_jis")]
    ShiftJis,
}

/// 読んだ文字コード。JSONでは `utf-8` `utf-16le` `utf-16be` `shift_jis`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextEncoding {
    /// UTF-8。
    #[serde(rename = "utf-8")]
    Utf8,
    /// UTF-16（リトルエンディアン。BOMがあるときだけ）。
    #[serde(rename = "utf-16le")]
    Utf16Le,
    /// UTF-16（ビッグエンディアン。BOMがあるときだけ）。
    #[serde(rename = "utf-16be")]
    Utf16Be,
    /// シフトJIS（`encoding_rs` の `SHIFT_JIS` は Windows-31J の拡張文字を含む）。
    #[serde(rename = "shift_jis")]
    ShiftJis,
}
impl TextEncoding {
    /// 通信で使う名前。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::Utf16Le => "utf-16le",
            Self::Utf16Be => "utf-16be",
            Self::ShiftJis => "shift_jis",
        }
    }
}

/// 文字にしたファイル。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// 中身（BOMは除いてある）。
    pub text: String,
    /// 読んだ文字コード。
    pub encoding: TextEncoding,
    /// BOMがあったか。
    pub bom: bool,
    /// 正しく読めなかったバイト列の数（それぞれ U+FFFD に置き換えた）。0でなければ利用者に警告する。
    pub malformed: usize,
}

/// バイト列を文字にします。`Auto` は、BOM（UTF-8、UTF-16LE/BE）があればそれに従い、
/// 無ければ、UTF-8として正しければUTF-8、そうでなければシフトJISとして読みます。
/// 読めないバイトは U+FFFD に置き換えて数えます（失敗にはしない）。
///
/// # Errors
/// NUL文字を含む（文字のファイルではない）ときに返します。
pub fn decode(bytes: &[u8], choice: EncodingChoice) -> Result<Decoded, ImportError> {
    let utf8_bom = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]);
    let (encoding, kind, bom, body): (&'static Encoding, TextEncoding, bool, &[u8]) = match choice {
        EncodingChoice::Utf8 => match utf8_bom {
            Some(rest) => (UTF_8, TextEncoding::Utf8, true, rest),
            None => (UTF_8, TextEncoding::Utf8, false, bytes),
        },
        EncodingChoice::ShiftJis => (SHIFT_JIS, TextEncoding::ShiftJis, false, bytes),
        EncodingChoice::Auto => {
            if let Some(rest) = utf8_bom {
                (UTF_8, TextEncoding::Utf8, true, rest)
            } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
                (UTF_16LE, TextEncoding::Utf16Le, true, rest)
            } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
                (UTF_16BE, TextEncoding::Utf16Be, true, rest)
            } else if std::str::from_utf8(bytes).is_ok() {
                (UTF_8, TextEncoding::Utf8, false, bytes)
            } else {
                (SHIFT_JIS, TextEncoding::ShiftJis, false, bytes)
            }
        }
    };
    let (text, malformed) = decode_counting(encoding, body);
    if text.contains('\0') {
        return Err(ImportError::NotText);
    }
    Ok(Decoded {
        text,
        encoding: kind,
        bom,
        malformed,
    })
}

fn decode_counting(encoding: &'static Encoding, bytes: &[u8]) -> (String, usize) {
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let room = |decoder: &encoding_rs::Decoder, len: usize| {
        decoder
            .max_utf8_buffer_length_without_replacement(len)
            .unwrap_or(len)
            .max(16)
    };
    let mut text = String::with_capacity(room(&decoder, bytes.len()));
    let mut rest = bytes;
    let mut malformed = 0usize;
    loop {
        let (result, read) = decoder.decode_to_string_without_replacement(rest, &mut text, true);
        rest = rest.get(read..).unwrap_or_default();
        match result {
            DecoderResult::InputEmpty => return (text, malformed),
            DecoderResult::OutputFull => text.reserve(room(&decoder, rest.len())),
            DecoderResult::Malformed(_, _) => {
                malformed = malformed.saturating_add(1);
                text.push('\u{FFFD}');
            }
        }
    }
}
