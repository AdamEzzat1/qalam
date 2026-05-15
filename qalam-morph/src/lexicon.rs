//! Lexicon access — read-only lookup against the built FST artifact.
//!
//! The runtime lexicon is a memory-mapped FST built offline by
//! `qalam-lexicon-builder` from open sources. See `DESIGN.md` §5.7 for the
//! source list and licensing.

use qalam_core::{ContentHash, FeatureSet, LexEntryId};
use smol_str::SmolStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexEntry {
    pub id: LexEntryId,
    pub lemma: SmolStr,
    pub features: FeatureSet,
}

/// Read-only lexicon, typically backed by a memory-mapped FST artifact.
pub trait Lexicon: Send + Sync {
    /// Look up entries matching a stem. The returned entries are in
    /// `LexEntryId ASC` order.
    fn lookup_stem(&self, stem: &str) -> Vec<LexEntry>;

    /// The content hash of this lexicon. Every analysis carries this hash in
    /// its `Provenance` so downstream consumers can verify they were built
    /// with the expected lexicon.
    fn hash(&self) -> ContentHash;
}
