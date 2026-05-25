//! Root extraction — turning a stem into candidate (root, pattern) analyses.
//!
//! Pipeline for one stem:
//! 1. `qalam_text::unicode::normalize` (NFC + fold alef variants, strip tatweel)
//! 2. `qalam_text::unicode::strip_tashkil` (remove diacritics) -> skeleton
//! 3. match the skeleton against the pattern table (strong roots only)
//!
//! The resulting [`Root`] is an abstraction (just its radicals), so it carries
//! no span — unlike clitics/tokens, a root does not correspond to a contiguous
//! slice of the input.

use crate::patterns::{PatternMatch, PatternTable};
use qalam_core::Root;
use qalam_text::unicode;

/// A root candidate with the pattern that produced it and a confidence.
pub type RootCandidate = PatternMatch;

/// Extract strong-root candidates for a stem surface (raw, possibly diacritized
/// and un-normalized). Returns matches sorted by `(confidence DESC, pattern id
/// ASC)`; empty if no strong pattern matches (e.g. weak-root stems).
pub fn analyze_stem(
    stem_surface: &str,
    table: &PatternTable,
) -> smallvec::SmallVec<[RootCandidate; 8]> {
    let skeleton = unicode::strip_tashkil(&unicode::normalize(stem_surface));
    table.match_skeleton(&skeleton)
}

/// Convenience: the single best root for a stem, if any.
pub fn best_root(stem_surface: &str, table: &PatternTable) -> Option<Root> {
    analyze_stem(stem_surface, table)
        .first()
        .map(|m| m.root.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_root_through_normalization_and_tashkil() {
        let table = PatternTable::builtin();
        // Diacritized + normalize-able: كَاتِب (active participle) -> ك ت ب.
        let root = best_root("كَاتِب", &table).expect("should match فاعل");
        assert_eq!(root.radicals.as_slice(), ['ك', 'ت', 'ب']);
    }

    #[test]
    fn weak_root_stem_yields_no_strong_candidate() {
        let table = PatternTable::builtin();
        // باب (door, root ب-و-ب, weak/geminate) should not produce a strong root.
        assert!(best_root("قال", &table).is_none());
    }

    #[test]
    fn unknown_shape_yields_empty() {
        let table = PatternTable::builtin();
        // A 6-letter skeleton matching no pattern.
        assert!(analyze_stem("abcdef", &table).is_empty());
    }

    #[test]
    fn deterministic() {
        let table = PatternTable::builtin();
        assert_eq!(analyze_stem("مكتوب", &table), analyze_stem("مكتوب", &table));
    }
}
