//! Templatic morphological patterns (e.g. فِعال, مَفْعول, اِسْتَفْعَل).
//!
//! Patterns are loaded once at startup from a static table. Iteration order is
//! `(specificity DESC, id ASC)` — this is the canonical firing order recorded
//! in every analysis's provenance.

use qalam_core::{Conf, FeatureSet, PatternId, Root};
use smallvec::SmallVec;
use smol_str::SmolStr;

/// A templatic pattern: a sequence of root-slot and constant-letter positions,
/// plus the morpho-syntactic features it imposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub id: PatternId,
    /// Pattern template using ف ع ل as root-slot placeholders, e.g. "فِعال".
    pub template: SmolStr,
    /// Higher = more specific. Patterns with higher specificity fire first.
    pub specificity: u32,
    /// Features carried by this pattern (e.g. nominal, masculine singular).
    pub features: FeatureSet,
}

/// Result of matching a stem against a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternMatch {
    pub pattern: PatternId,
    pub root: Root,
    pub confidence: Conf,
}

/// The static, deterministically-ordered pattern table.
#[derive(Debug, Clone, Default)]
pub struct PatternTable {
    patterns: Vec<Pattern>,
}

impl PatternTable {
    /// Returns the patterns in their canonical firing order:
    /// `(specificity DESC, id ASC)`.
    pub fn ordered(&self) -> &[Pattern] {
        &self.patterns
    }

    /// Try to match the stem against every pattern, returning all matches in
    /// canonical order.
    pub fn match_all(&self, stem: &str) -> SmallVec<[PatternMatch; 4]> {
        // TODO(Phase 1, Stage 2): templatic match against each pattern in
        // canonical order. Record every firing in the trace.
        let _ = stem;
        todo!("pattern matching: implemented in next PR")
    }
}
