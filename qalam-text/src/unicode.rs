//! Unicode normalization for Arabic text.
//!
//! `normalize` applies NFC (Unicode Normalization Form C) followed by the
//! Arabic normative fold table embedded from [`data/folds.toml`]. The fold
//! table's bytes are hashed with BLAKE3; that hash is exposed via
//! [`fold_table_hash`] and is included in every analysis's provenance.
//!
//! Determinism: same input -> same output, byte-for-byte, on every platform.
//! See `DESIGN.md` §4 and §5.3 for the surrounding contract.
//!
//! Scope for v0.1 (Phase 1, Stage 1.1):
//! - Fold: أ إ آ ٱ -> ا
//! - Strip: ـ (tatweel)
//! - Preserve: ة, ى, ء, ؤ, ئ, all diacritics, all digits, all non-Arabic.

use qalam_core::ContentHash;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

/// The fold table source. Embedded at compile time; the runtime never reads
/// from disk.
const FOLDS_TOML: &str = include_str!("../data/folds.toml");

/// On-disk fold-table schema. Mirrored deserialization target.
///
/// `deny_unknown_fields` on both structs is intentional: a stray field is
/// almost always a bug (typo, or a top-level key accidentally falling inside
/// a `[[map]]` table due to TOML's "everything after a header belongs to that
/// table" rule). We want it to be a loud parse error, not silent data loss.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FoldFile {
    #[serde(default)]
    map: Vec<FoldEntry>,
    #[serde(default)]
    strip: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FoldEntry {
    from: String,
    to: String,
}

/// Parsed fold table held in static memory after first access.
#[derive(Debug)]
struct FoldTable {
    /// Source char -> target char. `BTreeMap` for deterministic iteration if
    /// any caller ever needs to enumerate the table.
    map: BTreeMap<char, char>,
    /// Characters to drop entirely. `BTreeSet` for the same reason.
    strip: BTreeSet<char>,
    /// `BLAKE3(FOLDS_TOML.as_bytes())`. Computed once at startup.
    hash: ContentHash,
}

/// Parse `FOLDS_TOML` into a `FoldTable`, validating invariants.
///
/// Panics if:
/// - The TOML is malformed.
/// - Any `from`, `to`, or `strip` entry is not exactly one Unicode scalar value.
/// - The idempotency invariant is violated: a fold target appears as a fold
///   source or in the strip set (which would make `normalize(normalize(s))`
///   non-idempotent).
fn build_fold_table() -> FoldTable {
    let parsed: FoldFile =
        toml::from_str(FOLDS_TOML).expect("folds.toml: malformed TOML (compile-time embed)");

    let mut map = BTreeMap::new();
    for entry in parsed.map {
        let from = single_char(&entry.from, "map.from");
        let to = single_char(&entry.to, "map.to");
        let prior = map.insert(from, to);
        assert!(
            prior.is_none(),
            "folds.toml: duplicate map entry for source {:?}",
            from
        );
    }

    let mut strip = BTreeSet::new();
    for s in parsed.strip {
        let ch = single_char(&s, "strip");
        strip.insert(ch);
    }

    // Idempotency invariant: a second pass of `normalize` must produce the
    // same output as the first. This requires that no fold target appears as
    // a fold source (otherwise it would re-fold) or in the strip set
    // (otherwise it would disappear on the second pass).
    for to in map.values() {
        assert!(
            !map.contains_key(to),
            "folds.toml: fold target {:?} appears as a fold source; breaks idempotency",
            to
        );
        assert!(
            !strip.contains(to),
            "folds.toml: fold target {:?} appears in strip set; breaks idempotency",
            to
        );
    }

    let hash = ContentHash::of(FOLDS_TOML.as_bytes());
    FoldTable { map, strip, hash }
}

/// Extract the single Unicode scalar from `s`, panicking with `context` if
/// `s` is not exactly one char.
fn single_char(s: &str, context: &str) -> char {
    let mut chars = s.chars();
    let first = chars
        .next()
        .unwrap_or_else(|| panic!("folds.toml: empty {context} entry"));
    assert!(
        chars.next().is_none(),
        "folds.toml: {context} must be exactly one char, got {:?}",
        s
    );
    first
}

/// Access the singleton fold table, building it on first call.
fn fold_table() -> &'static FoldTable {
    static TABLE: OnceLock<FoldTable> = OnceLock::new();
    TABLE.get_or_init(build_fold_table)
}

/// Normalize an Arabic input string.
///
/// Applies NFC and the Arabic normative fold table. Idempotent:
/// `normalize(normalize(s)) == normalize(s)` for all valid UTF-8 inputs.
///
/// # Determinism
///
/// For fixed input bytes, this function returns the same output bytes on
/// every platform and on every run. The fold table's contribution to the
/// output is captured by `fold_table_hash()`.
pub fn normalize(input: &str) -> String {
    let table = fold_table();
    // NFC pre-pass; then a single linear scan applying folds and strips.
    let mut out = String::with_capacity(input.len());
    for ch in input.nfc() {
        if table.strip.contains(&ch) {
            continue;
        }
        match table.map.get(&ch) {
            Some(&folded) => out.push(folded),
            None => out.push(ch),
        }
    }
    out
}

/// The BLAKE3 hash of the embedded fold table's bytes.
///
/// Changes to the fold table — including whitespace or comments — change
/// this hash, which propagates into every analysis's `Provenance`. This is
/// how downstream consumers detect that they were built with a different
/// normalization regime than they expected.
pub fn fold_table_hash() -> ContentHash {
    fold_table().hash.clone()
}

/// Remove Arabic diacritics (tashkīl) and tatweel, leaving the consonant +
/// long-vowel skeleton used for morphological pattern matching.
///
/// This is deliberately *separate* from [`normalize`], which preserves
/// diacritics (P6: diacritics are input variation, not noise). The morphology
/// layer needs a diacritic-free view to align stems against templatic patterns;
/// the rest of the pipeline does not.
///
/// Strips: harakat / tanwin / shadda / sukun (U+064B–U+065F), superscript
/// (dagger) alef U+0670, and tatweel U+0640.
pub fn strip_tashkil(input: &str) -> String {
    input.chars().filter(|c| !is_tashkil(*c)).collect()
}

fn is_tashkil(c: char) -> bool {
    matches!(
        c,
        '\u{0640}'              // tatweel
        | '\u{064B}'
            ..='\u{065F}' // tanwin, harakat, shadda, sukun, extended
        | '\u{0670}' // superscript (dagger) alef
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Specific-case unit tests ------------------------------------------

    #[test]
    fn folds_alef_with_hamza_above() {
        assert_eq!(normalize("أحمد"), "احمد");
    }

    #[test]
    fn folds_alef_with_hamza_below() {
        assert_eq!(normalize("إسلام"), "اسلام");
    }

    #[test]
    fn folds_alef_with_madda() {
        assert_eq!(normalize("آدم"), "ادم");
    }

    #[test]
    fn folds_alef_wasla() {
        // U+0671 alef wasla, used in classical Arabic and Quran orthography.
        assert_eq!(normalize("\u{0671}بن"), "ابن");
    }

    #[test]
    fn strips_tatweel() {
        assert_eq!(normalize("كــــتاب"), "كتاب");
    }

    #[test]
    fn preserves_teh_marbuta() {
        // ة must NOT be folded to ه — it carries grammatical information.
        let s = "مدرسة";
        assert_eq!(normalize(s), s);
    }

    #[test]
    fn preserves_alef_maqsura_distinct_from_yeh() {
        // ى (alef maqsura) and ي (yeh) are distinct codepoints with distinct
        // grammatical roles. Folding them collapses real morphology.
        assert_ne!(normalize("ى"), normalize("ي"));
        assert_eq!(normalize("ى"), "ى");
    }

    #[test]
    fn preserves_hamza_carrier_letters() {
        // ء ؤ ئ are NOT folded in v0.1; they carry phonological information
        // distinct from the carrier letter alone.
        for s in ["ء", "ؤ", "ئ", "بئر", "مؤمن"] {
            assert_eq!(
                normalize(s),
                s,
                "expected hamza-bearing letter preserved in {:?}",
                s
            );
        }
    }

    #[test]
    fn preserves_diacritics() {
        // Diacritics (fatha, damma, kasra, sukun, shadda, tanwin) are handled
        // as "input variation" in v0.1 — never stripped.
        let s = "كَتَبَ";
        assert_eq!(normalize(s), s);
    }

    #[test]
    fn preserves_non_arabic() {
        // Pass-through for Latin, digits, punctuation, whitespace.
        assert_eq!(normalize("Hello 123 world."), "Hello 123 world.");
    }

    #[test]
    fn strip_tashkil_removes_diacritics_only() {
        // كَتَبَ -> كتب (harakat removed, consonants kept)
        assert_eq!(strip_tashkil("كَتَبَ"), "كتب");
        // No diacritics: unchanged.
        assert_eq!(strip_tashkil("كتاب"), "كتاب");
        // Tatweel also stripped.
        assert_eq!(strip_tashkil("كــتاب"), "كتاب");
    }

    #[test]
    fn empty_input() {
        assert_eq!(normalize(""), "");
    }

    // --- Determinism / idempotency tests -----------------------------------

    #[test]
    fn idempotent_on_curated_examples() {
        for s in [
            "",
            "كتاب",
            "أهلاً وسهلاً",
            "Hello",
            "ـ",       // bare tatweel
            "ـــــــ", // many tatweels
            "123 السلام",
            "أحمد إسلام آدم",
            "كــــتاب",
        ] {
            let once = normalize(s);
            let twice = normalize(&once);
            assert_eq!(
                once, twice,
                "not idempotent on {:?}: once={:?} twice={:?}",
                s, once, twice
            );
        }
    }

    #[test]
    fn fold_table_hash_is_deterministic_across_calls() {
        assert_eq!(fold_table_hash(), fold_table_hash());
    }

    #[test]
    fn fold_table_hash_matches_raw_blake3_of_file_bytes() {
        // The hash must be exactly BLAKE3 of the on-disk folds.toml bytes.
        // This test guards against accidental changes to the hashing formula.
        let expected = ContentHash::of(FOLDS_TOML.as_bytes());
        assert_eq!(fold_table_hash(), expected);
    }

    #[test]
    fn fold_table_hash_has_expected_length() {
        // BLAKE3 hex output is 64 chars.
        assert_eq!(fold_table_hash().as_str().len(), 64);
    }

    // --- Property tests ----------------------------------------------------

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_idempotent(s in ".{0,200}") {
            let once = normalize(&s);
            let twice = normalize(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn prop_never_panics(s in ".{0,200}") {
            let _ = normalize(&s);
        }

        #[test]
        fn prop_output_is_valid_utf8(s in ".{0,200}") {
            // String guarantees this structurally; the test documents intent.
            let out = normalize(&s);
            prop_assert!(std::str::from_utf8(out.as_bytes()).is_ok());
        }

        #[test]
        fn prop_tatweel_never_in_output(s in ".{0,200}") {
            let out = normalize(&s);
            // Explicit message: avoids prop_assert!'s default behavior of
            // using stringify!(cond) as a format string, which would treat
            // the `{0640}` inside the char literal as a positional arg.
            prop_assert!(!out.contains('\u{0640}'), "tatweel must never appear in normalize output");
        }

        #[test]
        fn prop_alef_variants_never_in_output(s in ".{0,200}") {
            let out = normalize(&s);
            for ch in ['\u{0623}', '\u{0625}', '\u{0622}', '\u{0671}'] {
                prop_assert!(
                    !out.contains(ch),
                    "found {:?} in normalize({:?}) = {:?}", ch, s, out
                );
            }
        }
    }
}
