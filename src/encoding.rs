// SPDX-License-Identifier: GPL-3.0-only

//! WMDE: text encodings and line endings of the files the notepad opens and saves.
//!
//! Upstream reads and writes UTF-8 only, and a file in any other encoding opened as an empty
//! untitled tab. Here a file is decoded from what it is and written back the same way:
//! UTF-8 with or without a byte order mark, UTF-16 marked by its BOM, and the two Cyrillic
//! single-byte code pages that are still met in the wild, Windows-1251 and KOI8-U.

use std::fmt;

/// An encoding the notepad can read and write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1251,
    Koi8U,
}

impl TextEncoding {
    /// Every encoding, in the order the choosers list them.
    pub const ALL: [Self; 6] = [
        Self::Utf8,
        Self::Utf8Bom,
        Self::Utf16Le,
        Self::Utf16Be,
        Self::Windows1251,
        Self::Koi8U,
    ];

    /// The name shown in the status bar and the choosers. Encoding names are not translated;
    /// the only word in them, the BOM mark, is the same in every language.
    pub fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Windows1251 => "Windows-1251",
            Self::Koi8U => "KOI8-U",
        }
    }

    fn bom(self) -> &'static [u8] {
        match self {
            Self::Utf8Bom => &[0xEF, 0xBB, 0xBF],
            Self::Utf16Le => &[0xFF, 0xFE],
            Self::Utf16Be => &[0xFE, 0xFF],
            Self::Utf8 | Self::Windows1251 | Self::Koi8U => &[],
        }
    }
}

impl fmt::Display for TextEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The line ending a document is saved with.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineEnding {
    #[default]
    Lf,
    CrLf,
}

impl LineEnding {
    pub const ALL: [Self; 2] = [Self::Lf, Self::CrLf];

    pub fn name(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

/// Why bytes could not be opened as text.
#[derive(Debug, Eq, PartialEq)]
pub enum DecodeError {
    /// A NUL byte outside UTF-16: an executable, an image, an archive.
    Binary,
}

/// A decoded file.
#[derive(Debug, Eq, PartialEq)]
pub struct Decoded {
    pub text: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    /// Some bytes did not decode and were replaced with U+FFFD. Saving writes the
    /// replacement characters, not the original bytes.
    pub had_errors: bool,
}

/// Characters that the target encoding has no code for.
#[derive(Debug, Eq, PartialEq)]
pub struct Unmappable;

/// Picks the encoding of `bytes`: a byte order mark wins, then valid UTF-8, then the
/// Cyrillic code page whose lowercase range the bytes fall into.
pub fn detect(bytes: &[u8]) -> Result<TextEncoding, DecodeError> {
    for encoding in [
        TextEncoding::Utf8Bom,
        TextEncoding::Utf16Le,
        TextEncoding::Utf16Be,
    ] {
        if bytes.starts_with(encoding.bom()) {
            return Ok(encoding);
        }
    }

    // UTF-16 without a BOM is not guessed: every Windows tool writes the mark, and a file
    // full of NULs without one is far more likely to be binary.
    if bytes.contains(&0) {
        return Err(DecodeError::Binary);
    }

    if std::str::from_utf8(bytes).is_ok() {
        return Ok(TextEncoding::Utf8);
    }

    // Running text is mostly lowercase. Windows-1251 keeps lowercase Cyrillic in 0xE0-0xFF
    // and uppercase in 0xC0-0xDF; KOI8-U has it the other way round.
    let (mut upper_half, mut lower_half) = (0usize, 0usize);
    for &byte in bytes {
        match byte {
            0xC0..=0xDF => upper_half += 1,
            0xE0..=0xFF => lower_half += 1,
            _ => {}
        }
    }
    if upper_half > lower_half {
        Ok(TextEncoding::Koi8U)
    } else {
        Ok(TextEncoding::Windows1251)
    }
}

/// Decodes `bytes` as `forced`, or as what [`detect`] finds when nothing is forced.
pub fn decode(bytes: &[u8], forced: Option<TextEncoding>) -> Result<Decoded, DecodeError> {
    let encoding = match forced {
        Some(encoding) => encoding,
        None => detect(bytes)?,
    };
    let body = bytes.strip_prefix(encoding.bom()).unwrap_or(bytes);
    let (text, had_errors) = match encoding {
        TextEncoding::Utf8 | TextEncoding::Utf8Bom => {
            let (text, had_errors) = encoding_rs::UTF_8.decode_without_bom_handling(body);
            (text.into_owned(), had_errors)
        }
        TextEncoding::Utf16Le => {
            let (text, had_errors) = encoding_rs::UTF_16LE.decode_without_bom_handling(body);
            (text.into_owned(), had_errors)
        }
        TextEncoding::Utf16Be => {
            let (text, had_errors) = encoding_rs::UTF_16BE.decode_without_bom_handling(body);
            (text.into_owned(), had_errors)
        }
        TextEncoding::Windows1251 => {
            let (text, had_errors) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(body);
            (text.into_owned(), had_errors)
        }
        TextEncoding::Koi8U => {
            let (text, had_errors) = encoding_rs::KOI8_U.decode_without_bom_handling(body);
            (text.into_owned(), had_errors)
        }
    };
    let line_ending = detect_line_ending(&text);
    Ok(Decoded {
        text,
        encoding,
        line_ending,
        had_errors,
    })
}

/// Encodes `text` as `encoding`, BOM included. Fails rather than writing HTML numeric
/// character references when a single-byte code page has no code for a character.
pub fn encode(text: &str, encoding: TextEncoding) -> Result<Vec<u8>, Unmappable> {
    let mut bytes = encoding.bom().to_vec();
    match encoding {
        TextEncoding::Utf8 | TextEncoding::Utf8Bom => bytes.extend_from_slice(text.as_bytes()),
        // encoding_rs only decodes UTF-16; encoding it is a plain code unit walk.
        TextEncoding::Utf16Le => {
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
        }
        TextEncoding::Utf16Be => {
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&unit.to_be_bytes());
            }
        }
        TextEncoding::Windows1251 | TextEncoding::Koi8U => {
            let target = if encoding == TextEncoding::Windows1251 {
                encoding_rs::WINDOWS_1251
            } else {
                encoding_rs::KOI8_U
            };
            let (encoded, _, had_errors) = target.encode(text);
            if had_errors {
                return Err(Unmappable);
            }
            bytes.extend_from_slice(&encoded);
        }
    }
    Ok(bytes)
}

/// The ending most lines of `text` use; LF when there are no line breaks at all.
pub fn detect_line_ending(text: &str) -> LineEnding {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count() - crlf;
    if crlf > lf {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

/// Rewrites every LF and CRLF line break in `text` as `ending`. A text that did not end in
/// a line break still does not, and a lone CR is left alone.
pub fn normalize_line_endings(text: &str, ending: LineEnding) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 32);
    let mut rest = text;
    while let Some(i) = rest.find('\n') {
        let line = &rest[..i];
        out.push_str(line.strip_suffix('\r').unwrap_or(line));
        out.push_str(ending.as_str());
        rest = &rest[i + 1..];
    }
    out.push_str(rest);
    out
}

/// The encoding to read a file again with, after it changed on disk or was reverted.
///
/// A byte order mark or valid UTF-8 is a fact about the new bytes and wins. The choice
/// between the two code pages is only a guess, so a tab that is already in one of them -
/// perhaps because the user picked it - stays in it. Plain ASCII reads the same in every
/// encoding without a BOM, so it changes nothing either: a Windows-1251 file that holds
/// only ASCII after a save is still a Windows-1251 file.
pub fn reload_encoding(bytes: &[u8], current: TextEncoding) -> Result<TextEncoding, DecodeError> {
    let detected = detect(bytes)?;
    if detected == TextEncoding::Utf8 && bytes.is_ascii() && current.bom().is_empty() {
        return Ok(current);
    }
    let guessed = |encoding| matches!(encoding, TextEncoding::Windows1251 | TextEncoding::Koi8U);
    if guessed(detected) && guessed(current) {
        Ok(current)
    } else {
        Ok(detected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UK: &str = "Привіт, світе! Ґанок і їжак єднає.\n";

    fn round_trip(encoding: TextEncoding) {
        let bytes = encode(UK, encoding).unwrap();
        let decoded = decode(&bytes, None).unwrap();
        assert_eq!(
            decoded.encoding, encoding,
            "detected as {}",
            decoded.encoding
        );
        assert_eq!(decoded.text, UK);
        assert!(!decoded.had_errors);
        assert_eq!(encode(&decoded.text, decoded.encoding).unwrap(), bytes);
    }

    #[test]
    fn every_encoding_round_trips_ukrainian_text() {
        for encoding in TextEncoding::ALL {
            round_trip(encoding);
        }
    }

    #[test]
    fn plain_ascii_is_utf8() {
        assert_eq!(detect(b"hello\n"), Ok(TextEncoding::Utf8));
        assert_eq!(detect(b""), Ok(TextEncoding::Utf8));
    }

    #[test]
    fn utf8_bom_is_not_part_of_the_text() {
        let decoded = decode(b"\xEF\xBB\xBFabc", None).unwrap();
        assert_eq!(decoded.encoding, TextEncoding::Utf8Bom);
        assert_eq!(decoded.text, "abc");
    }

    #[test]
    fn utf16_needs_its_bom() {
        let mut bytes = Vec::new();
        for unit in "abc".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(detect(&bytes), Err(DecodeError::Binary));
    }

    #[test]
    fn nul_bytes_are_binary() {
        assert_eq!(detect(b"\x7fELF\x02\x01\x01\x00"), Err(DecodeError::Binary));
    }

    #[test]
    fn uppercase_heavy_text_still_tells_the_code_pages_apart() {
        // A heading in capitals followed by ordinary prose.
        let text = "ЗМІСТ\nЦе звичайний текст українською мовою, набагато довший за заголовок.\n";
        for encoding in [TextEncoding::Windows1251, TextEncoding::Koi8U] {
            let bytes = encode(text, encoding).unwrap();
            assert_eq!(detect(&bytes), Ok(encoding));
        }
    }

    #[test]
    fn forced_encoding_overrides_detection() {
        let bytes = encode(UK, TextEncoding::Windows1251).unwrap();
        let decoded = decode(&bytes, Some(TextEncoding::Koi8U)).unwrap();
        assert_eq!(decoded.encoding, TextEncoding::Koi8U);
        assert_ne!(decoded.text, UK);
    }

    #[test]
    fn forced_utf8_on_other_bytes_reports_errors() {
        let bytes = encode(UK, TextEncoding::Windows1251).unwrap();
        let decoded = decode(&bytes, Some(TextEncoding::Utf8)).unwrap();
        assert!(decoded.had_errors);
        assert!(decoded.text.contains('\u{FFFD}'));
    }

    #[test]
    fn code_pages_refuse_characters_they_cannot_hold() {
        assert_eq!(encode("日本", TextEncoding::Windows1251), Err(Unmappable));
        assert_eq!(encode("日本", TextEncoding::Koi8U), Err(Unmappable));
        assert!(encode("日本", TextEncoding::Utf16Le).is_ok());
    }

    #[test]
    fn line_ending_follows_the_majority() {
        assert_eq!(detect_line_ending("a\r\nb\r\nc\n"), LineEnding::CrLf);
        assert_eq!(detect_line_ending("a\nb\nc\r\n"), LineEnding::Lf);
        assert_eq!(detect_line_ending("no breaks"), LineEnding::Lf);
        assert_eq!(
            decode(b"a\r\nb\r\n", None).unwrap().line_ending,
            LineEnding::CrLf
        );
    }

    #[test]
    fn reload_keeps_a_chosen_code_page_but_follows_facts() {
        let cp1251 = encode(UK, TextEncoding::Windows1251).unwrap();
        assert_eq!(
            reload_encoding(&cp1251, TextEncoding::Koi8U),
            Ok(TextEncoding::Koi8U)
        );
        assert_eq!(
            reload_encoding(&cp1251, TextEncoding::Utf8),
            Ok(TextEncoding::Windows1251)
        );
        let bom = encode(UK, TextEncoding::Utf8Bom).unwrap();
        assert_eq!(
            reload_encoding(&bom, TextEncoding::Koi8U),
            Ok(TextEncoding::Utf8Bom)
        );
        assert_eq!(
            reload_encoding(UK.as_bytes(), TextEncoding::Windows1251),
            Ok(TextEncoding::Utf8)
        );
        assert_eq!(
            reload_encoding(b"ascii only\n", TextEncoding::Koi8U),
            Ok(TextEncoding::Koi8U)
        );
        assert_eq!(
            reload_encoding(b"ascii only\n", TextEncoding::Utf16Le),
            Ok(TextEncoding::Utf8)
        );
    }

    #[test]
    fn normalizing_keeps_the_trailing_state() {
        assert_eq!(
            normalize_line_endings("a\nb\r\nc", LineEnding::CrLf),
            "a\r\nb\r\nc"
        );
        assert_eq!(normalize_line_endings("a\r\nb\n", LineEnding::Lf), "a\nb\n");
        assert_eq!(normalize_line_endings("", LineEnding::CrLf), "");
        assert_eq!(normalize_line_endings("a\rb", LineEnding::CrLf), "a\rb");
    }
}
