//! Token segmentation.
//!
//! Splits normalized Arabic text into tokens. Distinguishes Arabic, Latin,
//! digits, punctuation, and whitespace by Unicode category. Multi-script input
//! is segmented at script boundaries.

use qalam_core::ByteSpan;
use smol_str::SmolStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub surface: SmolStr,
    pub span: ByteSpan,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Arabic,
    Latin,
    Digit,
    Punct,
    Whitespace,
    Other,
}

/// Tokenize a (normalized) input string into a sequence of tokens.
///
/// The input must already be Unicode-normalized via
/// [`crate::unicode::normalize`]. Calling on un-normalized input is not a
/// safety error but may produce unexpected splits.
pub fn tokenize(input: &str) -> Vec<Token> {
    // TODO(Phase 1, Stage 1): grapheme-aware segmentation with span tracking.
    let _ = input;
    todo!("tokenize: implemented in next PR")
}
