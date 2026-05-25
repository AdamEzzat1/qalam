//! Word-frequency aggregation.
//!
//! Groups word tokens (Arabic + Latin) by their **normalized** form and counts
//! occurrences, recording which raw surface variants collapsed into each group.
//! This is the smallest end-to-end artifact the pipeline can produce that a
//! human finds directly useful: a frequency list where, e.g., `أحمد` and `احمد`
//! are correctly recognized as the same word.
//!
//! Punctuation, whitespace, digits, and [`TokenKind::Other`] are excluded.
//!
//! [`TokenKind::Other`]: crate::tokenize::TokenKind::Other

use crate::tokenize::{Token, TokenKind};
use qalam_core::ByteSpan;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::{BTreeMap, BTreeSet};

/// One normalized word group with its count, raw variants, and first position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreqEntry {
    /// The normalized form shared by every occurrence in this group.
    pub normalized: String,
    /// Number of tokens that normalized to this form.
    pub count: usize,
    /// Distinct raw surface forms, sorted by UTF-8 byte order — which for
    /// UTF-8 equals codepoint order, so `أ` (U+0623) precedes `ا` (U+0627),
    /// e.g. `["أحمد", "احمد"]`.
    pub variants: Vec<String>,
    /// Span of the earliest occurrence (smallest raw start offset).
    pub first_span: ByteSpan,
}

/// Compute word frequencies over `tokens`, grouping by normalized form.
///
/// # Determinism
///
/// Output ordering is `(count DESC, normalized ASC)`, with `variants` sorted
/// ascending. Grouping uses `BTreeMap`/`BTreeSet`, so iteration order is fixed.
/// For identical input, the returned `Vec` is byte-identical when serialized.
pub fn word_frequencies(tokens: &[Token]) -> Vec<FreqEntry> {
    struct Acc {
        count: usize,
        variants: BTreeSet<SmolStr>,
        first_span: ByteSpan,
    }

    let mut groups: BTreeMap<SmolStr, Acc> = BTreeMap::new();

    for t in tokens {
        if !matches!(t.kind, TokenKind::Arabic | TokenKind::Latin) {
            continue;
        }
        let acc = groups.entry(t.normalized.clone()).or_insert_with(|| Acc {
            count: 0,
            variants: BTreeSet::new(),
            first_span: t.span,
        });
        acc.count += 1;
        acc.variants.insert(t.raw.clone());
        if t.span.start < acc.first_span.start {
            acc.first_span = t.span;
        }
    }

    let mut entries: Vec<FreqEntry> = groups
        .into_iter()
        .map(|(normalized, acc)| FreqEntry {
            normalized: normalized.to_string(),
            count: acc.count,
            variants: acc.variants.into_iter().map(|s| s.to_string()).collect(),
            first_span: acc.first_span,
        })
        .collect();

    // Rank by descending count, then ascending normalized form for ties.
    // `sort_by` is stable, but the secondary key makes the order total anyway.
    entries.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.normalized.cmp(&b.normalized))
    });

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenize::tokenize;

    #[test]
    fn empty_input_yields_no_entries() {
        assert!(word_frequencies(&tokenize("")).is_empty());
    }

    #[test]
    fn counts_repeated_word() {
        let entries = word_frequencies(&tokenize("كتاب كتاب كتاب"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].count, 3);
        assert_eq!(entries[0].normalized, "كتاب");
    }

    #[test]
    fn collapses_normalization_variants() {
        // The whole point: hamza'd and bare alef forms are ONE word.
        let entries = word_frequencies(&tokenize("أحمد احمد"));
        assert_eq!(entries.len(), 1, "variants must collapse to one group");
        assert_eq!(entries[0].count, 2);
        assert_eq!(entries[0].normalized, "احمد");
        // Sorted by UTF-8 byte order: أ (U+0623) sorts before ا (U+0627).
        assert_eq!(
            entries[0].variants,
            vec!["أحمد".to_string(), "احمد".to_string()]
        );
    }

    #[test]
    fn excludes_punct_digits_whitespace() {
        let entries = word_frequencies(&tokenize("كتاب، ١٢٣ كتاب"));
        // Only the two "كتاب" count; the comma and digits are excluded.
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].count, 2);
    }

    #[test]
    fn ranks_by_count_then_alpha() {
        // "ب" appears twice, "ا" once -> ب first by count.
        let entries = word_frequencies(&tokenize("ا ب ب"));
        assert_eq!(entries[0].normalized, "ب");
        assert_eq!(entries[0].count, 2);
        assert_eq!(entries[1].normalized, "ا");
    }

    #[test]
    fn deterministic_across_calls() {
        let toks = tokenize("كتاب مدرسة كتاب مكتبة مدرسة كتاب");
        assert_eq!(word_frequencies(&toks), word_frequencies(&toks));
    }
}
