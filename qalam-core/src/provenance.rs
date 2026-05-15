//! Provenance tracking — the chain of evidence that produced an analysis.

use crate::trace::TraceEvent;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// A stable identifier for a rule in the static rule registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RuleId(pub u32);

/// A stable identifier for a lexicon entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LexEntryId(pub u32);

/// A BLAKE3 content hash, stored as a 64-character lowercase hex string.
///
/// Used everywhere a content-addressed identifier or input-fingerprint is
/// required: lexicon hashes, config hashes, canonical IDs for tie-breaking.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContentHash(pub String);

impl ContentHash {
    /// Hash the given bytes with BLAKE3.
    pub fn of(bytes: &[u8]) -> Self {
        let h = blake3::hash(bytes);
        Self(h.to_hex().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Show only the first 12 hex chars for human-readability; full hash is
        // available via `as_str`.
        write!(f, "{}...", &self.0[..self.0.len().min(12)])
    }
}

/// Records the chain of evidence that produced an analysis.
///
/// `decisions` is `None` when traces are disabled; `Some(...)` when
/// `TraceLevel::Full` is active. The `lexicon_hash` is always present so
/// downstream consumers can verify they were built with the expected lexicon.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Provenance {
    pub lexicon_hash: ContentHash,
    pub rules: SmallVec<[RuleId; 8]>,
    pub lex_entries: SmallVec<[LexEntryId; 4]>,
    pub decisions: Option<Vec<TraceEvent>>,
}

impl Provenance {
    pub fn new(lexicon_hash: ContentHash) -> Self {
        Self {
            lexicon_hash,
            rules: SmallVec::new(),
            lex_entries: SmallVec::new(),
            decisions: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_deterministic() {
        let a = ContentHash::of(b"hello");
        let b = ContentHash::of(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn content_hash_differs_on_different_input() {
        let a = ContentHash::of(b"hello");
        let b = ContentHash::of(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn content_hash_hex_length() {
        let h = ContentHash::of(b"anything");
        assert_eq!(h.as_str().len(), 64);
    }
}
