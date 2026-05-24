//! Reproducibility & raw-span integration tests.
//!
//! These run the full `normalize -> tokenize -> word_frequencies` pipeline over
//! the workspace golden corpus and assert the determinism contract locally.
//! The CI cross-OS gate proves byte-equality *across platforms*; these tests
//! prove stability *within* a build and verify the raw-span anchoring invariant
//! that the whole pipeline now depends on.

use qalam_text::{freq, tokenize, unicode};

/// The shared golden corpus, embedded at compile time from the workspace root.
const CORPUS: &str = include_str!("../../tests/fixtures/golden_corpus.txt");

#[test]
fn normalize_is_deterministic() {
    assert_eq!(unicode::normalize(CORPUS), unicode::normalize(CORPUS));
}

#[test]
fn normalize_is_idempotent_on_corpus() {
    let once = unicode::normalize(CORPUS);
    assert_eq!(unicode::normalize(&once), once);
}

#[test]
fn tokenize_is_deterministic() {
    assert_eq!(tokenize::tokenize(CORPUS), tokenize::tokenize(CORPUS));
}

#[test]
fn freq_is_deterministic() {
    let toks = tokenize::tokenize(CORPUS);
    assert_eq!(freq::word_frequencies(&toks), freq::word_frequencies(&toks));
}

/// The load-bearing invariant of this PR: every span maps back into the RAW
/// corpus, and the tokens tile the input with no gaps or overlaps.
#[test]
fn spans_are_raw_anchored_and_cover_corpus() {
    let toks = tokenize::tokenize(CORPUS);
    let mut cursor = 0u32;
    for t in &toks {
        assert_eq!(t.span.start, cursor, "gap or overlap before token {t:?}");
        assert_eq!(
            &CORPUS[t.span.start as usize..t.span.end as usize],
            t.raw.as_str(),
            "span does not slice the raw corpus to the token's raw surface",
        );
        cursor = t.span.end;
    }
    assert_eq!(
        cursor as usize,
        CORPUS.len(),
        "tokens do not cover the whole corpus"
    );
}

/// Demonstrates the user-visible value: normalization variants collapse, and
/// the corpus's alef-variant line contributes to bare-alef groups.
#[test]
fn frequencies_group_by_normalized_form() {
    let toks = tokenize::tokenize(CORPUS);
    let entries = freq::word_frequencies(&toks);

    // Every entry's normalized form must be free of the folded letters.
    for e in &entries {
        for ch in ['\u{0623}', '\u{0625}', '\u{0622}', '\u{0671}', '\u{0640}'] {
            assert!(
                !e.normalized.contains(ch),
                "normalized key {:?} still contains a foldable/strippable char",
                e.normalized
            );
        }
        assert!(e.count >= 1);
        assert!(!e.variants.is_empty());
    }
}
