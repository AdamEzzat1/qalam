//! Token segmentation.
//!
//! Splits **raw** Arabic text into tokens at script-class boundaries, then
//! normalizes each token's surface separately. Every token carries:
//! - `raw`: the original substring, byte-for-byte from the input;
//! - `normalized`: its NFC + fold-table normalized form (for matching/grouping);
//! - `span`: a [`ByteSpan`] into the **raw** input (see its docs for why).
//!
//! ## Why tokenize raw, not normalized
//!
//! Normalization can change byte length (it strips tatweel, may recompose under
//! NFC). If we tokenized normalized text, every `ByteSpan` would point into a
//! buffer that no longer corresponds to the user's document. So we tokenize raw
//! and normalize per token.
//!
//! ## Invariant this relies on
//!
//! The v0.1 fold set never changes token boundaries: alef-variant folds stay
//! within the Arabic letter class, and tatweel is always intra-word. So
//! `tokenize(raw)` and a hypothetical `tokenize(normalize(raw))` agree on
//! boundaries. **If a future fold can split or merge tokens (e.g. one that
//! introduces or removes whitespace/punctuation), this assumption must be
//! revisited.**

use qalam_core::ByteSpan;
use smol_str::SmolStr;

/// A single token: a maximal run of same-kind characters from the raw input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Surface text exactly as it appears in the raw input.
    pub raw: SmolStr,
    /// Normalized form of `raw` (NFC + Arabic fold table).
    pub normalized: SmolStr,
    /// Byte span into the **raw** input. `&raw_input[span] == raw`.
    pub span: ByteSpan,
    /// Script/character class of this token.
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// Arabic letters and combining marks (incl. tatweel, diacritics).
    Arabic,
    /// Latin (and other alphabetic non-Arabic) letters.
    Latin,
    /// Digits: ASCII, Arabic-Indic (٠-٩), or Extended Arabic-Indic (۰-۹).
    Digit,
    /// Punctuation: ASCII punctuation plus common Arabic marks (، ؛ ؟ ٪ ۔).
    Punct,
    /// Whitespace runs.
    Whitespace,
    /// Anything else (format chars, symbols, emoji, unhandled punctuation).
    Other,
}

/// Tokenize raw input into a sequence of tokens covering the entire input.
///
/// # Guarantees
///
/// - **Coverage:** concatenating `token.raw` in order reproduces the input
///   exactly; spans are contiguous and cover `0..input.len()`.
/// - **Raw-anchored spans:** `&input[token.span.start..token.span.end] ==
///   token.raw` for every token.
/// - **Determinism:** same input bytes -> same tokens, on every platform.
///
/// # Limits
///
/// Input must be < 4 GiB (spans are `u32`). General Unicode punctuation outside
/// ASCII and the listed Arabic marks (e.g. em-dash, smart quotes) currently
/// classifies as [`TokenKind::Other`]; ZWJ/ZWNJ likewise. Tracked for a later
/// fold-table/segmentation pass.
pub fn tokenize(input: &str) -> Vec<Token> {
    debug_assert!(
        input.len() <= u32::MAX as usize,
        "tokenize: input exceeds u32 span range (4 GiB)"
    );

    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((start, c)) = chars.next() {
        let kind = classify(c);
        let mut end = start + c.len_utf8();

        // Extend the run while the next char has the same kind.
        while let Some(&(idx, next_c)) = chars.peek() {
            if classify(next_c) == kind {
                end = idx + next_c.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        let raw_surface = &input[start..end];
        let normalized = crate::unicode::normalize(raw_surface);
        tokens.push(Token {
            raw: SmolStr::new(raw_surface),
            normalized: SmolStr::new(normalized),
            span: ByteSpan::new(start as u32, end as u32),
            kind,
        });
    }

    tokens
}

/// Classify a single character into a [`TokenKind`].
///
/// Order matters: digits and punctuation are checked before the general Arabic
/// block, because Arabic-Indic digits and Arabic punctuation marks live inside
/// the same Unicode block as Arabic letters.
fn classify(c: char) -> TokenKind {
    if c.is_whitespace() {
        TokenKind::Whitespace
    } else if is_digit(c) {
        TokenKind::Digit
    } else if is_punct(c) {
        TokenKind::Punct
    } else if is_arabic_letter(c) {
        TokenKind::Arabic
    } else if c.is_alphabetic() {
        TokenKind::Latin
    } else {
        TokenKind::Other
    }
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
        || ('\u{0660}'..='\u{0669}').contains(&c) // Arabic-Indic
        || ('\u{06F0}'..='\u{06F9}').contains(&c) // Extended Arabic-Indic (Persian)
}

fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c,
            '\u{060C}'   // ، Arabic comma
                | '\u{060D}' // ؍ Arabic date separator
                | '\u{061B}' // ؛ Arabic semicolon
                | '\u{061F}' // ؟ Arabic question mark
                | '\u{066A}' // ٪ Arabic percent sign
                | '\u{066B}' // ٫ Arabic decimal separator
                | '\u{066C}' // ٬ Arabic thousands separator
                | '\u{066D}' // ٭ Arabic five-pointed star
                | '\u{06D4}' // ۔ Arabic full stop
        )
}

/// Arabic letters and combining marks. Digits and punctuation in the Arabic
/// blocks are routed earlier by [`classify`], so this can be a plain block test.
fn is_arabic_letter(c: char) -> bool {
    matches!(c,
        '\u{0600}'..='\u{06FF}'   // Arabic
        | '\u{0750}'..='\u{077F}' // Arabic Supplement
        | '\u{08A0}'..='\u{08FF}' // Arabic Extended-A
        | '\u{FB50}'..='\u{FDFF}' // Arabic Presentation Forms-A
        | '\u{FE70}'..='\u{FEFF}' // Arabic Presentation Forms-B
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(input: &str) -> Vec<TokenKind> {
        tokenize(input).into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn single_arabic_word() {
        let toks = tokenize("كتاب");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, TokenKind::Arabic);
        assert_eq!(toks[0].raw.as_str(), "كتاب");
        assert_eq!(toks[0].span, ByteSpan::new(0, 8)); // 4 chars x 2 bytes
    }

    #[test]
    fn splits_on_whitespace_and_keeps_it() {
        // word, space, word -> 3 tokens (the space is its own Whitespace token).
        assert_eq!(
            kinds("كتاب مفيد"),
            vec![TokenKind::Arabic, TokenKind::Whitespace, TokenKind::Arabic]
        );
    }

    #[test]
    fn splits_on_script_boundary() {
        assert_eq!(kinds("abcمرحبا"), vec![TokenKind::Latin, TokenKind::Arabic]);
    }

    #[test]
    fn arabic_punctuation_is_punct_not_arabic() {
        // ، ؟ are inside the Arabic block but must classify as punctuation.
        assert_eq!(
            kinds("نعم، لا؟"),
            vec![
                TokenKind::Arabic,
                TokenKind::Punct,
                TokenKind::Whitespace,
                TokenKind::Arabic,
                TokenKind::Punct
            ]
        );
    }

    #[test]
    fn arabic_indic_digits_are_digit() {
        assert_eq!(kinds("١٢٣"), vec![TokenKind::Digit]);
    }

    #[test]
    fn tatweel_stays_within_arabic_token() {
        // The tatweel is intra-word; the whole thing is one Arabic token whose
        // normalized form has the tatweel stripped.
        let toks = tokenize("كــتاب");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, TokenKind::Arabic);
        assert_eq!(toks[0].normalized.as_str(), "كتاب");
        assert_eq!(toks[0].raw.as_str(), "كــتاب");
    }

    #[test]
    fn normalized_field_folds_alef_variants() {
        let toks = tokenize("أحمد");
        assert_eq!(toks[0].raw.as_str(), "أحمد");
        assert_eq!(toks[0].normalized.as_str(), "احمد");
    }

    #[test]
    fn spans_are_raw_anchored() {
        let input = "أحمد وكتاب";
        for t in tokenize(input) {
            assert_eq!(
                &input[t.span.start as usize..t.span.end as usize],
                t.raw.as_str()
            );
        }
    }

    #[test]
    fn spans_cover_input_contiguously() {
        let input = "كتاب، abc ١٢٣";
        let toks = tokenize(input);
        let mut cursor = 0u32;
        for t in &toks {
            assert_eq!(t.span.start, cursor, "gap/overlap before {t:?}");
            cursor = t.span.end;
        }
        assert_eq!(cursor as usize, input.len());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_deterministic(s in ".{0,300}") {
            prop_assert_eq!(tokenize(&s), tokenize(&s));
        }

        #[test]
        fn prop_spans_reconstruct_input(s in ".{0,300}") {
            let reconstructed: String = tokenize(&s).iter().map(|t| t.raw.as_str()).collect();
            prop_assert_eq!(reconstructed, s);
        }

        #[test]
        fn prop_never_panics(s in ".{0,300}") {
            let _ = tokenize(&s);
        }
    }
}
