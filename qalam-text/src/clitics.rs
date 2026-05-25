//! Clitic splitting — proclitic and enclitic segmentation.
//!
//! Peels recognized clitics off a token to expose its stem, emitting a *forest*
//! of candidate splits (always including the identity "no split" candidate)
//! rather than committing to one segmentation. Undiacritized Arabic is
//! genuinely ambiguous here — e.g. «كتابه» is كتاب+ه ("his book"), but the
//! leading «ك» of any word could also be the ka- proclitic ("like/as") — so
//! later stages (pattern matching, lexicon lookup) re-rank these candidates.
//! Until then the identity candidate ranks highest (most conservative: do not
//! over-segment).
//!
//! ## Spans are raw-anchored
//!
//! Clitics are matched on the token's RAW surface and their `span`s are global
//! offsets into the original input (`token.span.start + local offset`). This is
//! sound because no v0.1 proclitic/enclitic surface form contains a foldable or
//! strippable character, so raw-surface matching and normalized-surface
//! matching agree on clitic boundaries.
//!
//! ## v0.1 scope and known gaps
//!
//! - Proclitics, peeled in order: conjunction (و ف), particle/preposition/
//!   future (ب ك ل س), definite article (ال).
//! - Enclitics: common pronominal suffixes; at most one peeled.
//! - The splitter deliberately *over-generates* (it will offer a ك- proclitic
//!   reading for «كتاب»); over-generation is the safe direction for a forest
//!   that a lexicon stage will later prune.
//! - **Diacritized input degrades.** Because matching is on the raw surface
//!   (to keep spans raw-anchored), diacritics sitting *between* clitic letters
//!   break the match: «وَبِكِتابِهِم» peels only «و» and misses the «هم»
//!   enclitic. This is reliable on undiacritized text (the MSA norm); robust
//!   diacritized handling needs a diacritic-stripped matching view with an
//!   offset map back to raw — the same offset-map work deferred elsewhere.
//! - NOT handled yet: the لِ+الـ → لل contraction, stacked object pronouns,
//!   sun-letter assimilation (orthographically invisible anyway).

use crate::tokenize::{Token, TokenKind};
use qalam_core::{ByteSpan, Clitic, CliticId, Conf, Stem};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use smol_str::SmolStr;

/// One candidate decomposition of a token into proclitics + stem + enclitics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliticSplit {
    pub proclitics: SmallVec<[Clitic; 4]>,
    pub stem: Stem,
    pub enclitics: SmallVec<[Clitic; 2]>,
    pub confidence: Conf,
}

// --- Static clitic tables. IDs are stable; changing them is semver-major. ---

/// Conjunction proclitics (position 1).
const CONJ: &[(&str, u16)] = &[("و", 100), ("ف", 101)];
/// Particle / preposition / future proclitics (position 2).
const PARTICLE: &[(&str, u16)] = &[("ب", 110), ("ك", 111), ("ل", 112), ("س", 113)];
/// Definite article (position 3): surface form and stable id.
const ARTICLE_FORM: &str = "ال";
const ARTICLE_ID: u16 = 120;
/// Enclitic pronominal suffixes, longest first for greedy matching.
const ENCLITICS: &[(&str, u16)] = &[
    ("هما", 200),
    ("كما", 201),
    ("هم", 202),
    ("هن", 203),
    ("كم", 204),
    ("كن", 205),
    ("نا", 206),
    ("ها", 207),
    ("ني", 208),
    ("ه", 209),
    ("ك", 210),
    ("ي", 211),
];

/// Minimum stem length (in chars) for a peel candidate to be offered. Prevents
/// degenerate splits like peeling a whole short word down to nothing.
const MIN_STEM_CHARS: usize = 2;

/// Confidence priors (deterministic placeholders until lexicon-based validation
/// in a later stage). Identity ranks highest = conservative default.
const CONF_IDENTITY: f32 = 0.6;
const CONF_SINGLE_AFFIX: f32 = 0.5;
const CONF_BOTH_AFFIXES: f32 = 0.45;

fn char_count(s: &str) -> usize {
    s.chars().count()
}

fn mk_clitic(form: &str, id: u16, abs_start: u32) -> Clitic {
    Clitic {
        form: SmolStr::new(form),
        clitic_id: CliticId(id),
        span: ByteSpan::new(abs_start, abs_start + form.len() as u32),
    }
}

fn mk_stem(slice: &str) -> Stem {
    Stem {
        surface: SmolStr::new(slice),
        lemma: None,
    }
}

/// Match the first table entry that `s` starts with.
///
/// The `'static` on the table's surface strings lets us return a `&'static str`
/// — sound because the only callers pass the `const` clitic tables.
fn match_prefix(s: &str, table: &[(&'static str, u16)]) -> Option<(&'static str, u16)> {
    table
        .iter()
        .find(|(sur, _)| s.starts_with(*sur))
        .map(|&(sur, id)| (sur, id))
}

/// Peel the proclitic sequence (conj?, particle?, article?) off the front of
/// `raw`. Returns the clitics (with absolute spans) and the byte offset where
/// the stem begins.
fn peel_proclitics(raw: &str, base: u32) -> (SmallVec<[Clitic; 4]>, usize) {
    let mut procs: SmallVec<[Clitic; 4]> = SmallVec::new();
    let mut off = 0usize;

    if let Some((form, id)) = match_prefix(&raw[off..], CONJ) {
        procs.push(mk_clitic(form, id, base + off as u32));
        off += form.len();
    }
    if let Some((form, id)) = match_prefix(&raw[off..], PARTICLE) {
        procs.push(mk_clitic(form, id, base + off as u32));
        off += form.len();
    }
    if raw[off..].starts_with(ARTICLE_FORM) {
        procs.push(mk_clitic(ARTICLE_FORM, ARTICLE_ID, base + off as u32));
        off += ARTICLE_FORM.len();
    }

    (procs, off)
}

/// Match one enclitic suffix at the end of `raw[..end]`. Returns the clitic and
/// the byte offset where it begins.
fn peel_enclitic(raw: &str, base: u32, end: usize) -> Option<(Clitic, usize)> {
    let region = &raw[..end];
    for &(sur, id) in ENCLITICS {
        if region.ends_with(sur) {
            let start = end - sur.len();
            return Some((mk_clitic(sur, id, base + start as u32), start));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn push_candidate(
    out: &mut SmallVec<[CliticSplit; 4]>,
    raw: &str,
    procs: SmallVec<[Clitic; 4]>,
    enc: Option<Clitic>,
    stem_start: usize,
    stem_end: usize,
    conf: f32,
) {
    if stem_end <= stem_start {
        return;
    }
    if char_count(&raw[stem_start..stem_end]) < MIN_STEM_CHARS {
        return;
    }
    let mut encs: SmallVec<[Clitic; 2]> = SmallVec::new();
    if let Some(e) = enc {
        encs.push(e);
    }
    let cand = CliticSplit {
        proclitics: procs,
        stem: mk_stem(&raw[stem_start..stem_end]),
        enclitics: encs,
        confidence: Conf::clamp(conf),
    };
    if !out.contains(&cand) {
        out.push(cand);
    }
}

fn identity(token: &Token) -> CliticSplit {
    CliticSplit {
        proclitics: SmallVec::new(),
        stem: Stem {
            surface: token.raw.clone(),
            lemma: None,
        },
        enclitics: SmallVec::new(),
        confidence: Conf::clamp(CONF_IDENTITY),
    }
}

/// Compute the forest of plausible clitic splits for a token.
///
/// Always returns at least the identity split. Candidates are ordered by
/// `(confidence DESC, stem surface ASC)` — deterministic and total.
pub fn split(token: &Token) -> SmallVec<[CliticSplit; 4]> {
    let mut out: SmallVec<[CliticSplit; 4]> = SmallVec::new();
    out.push(identity(token));

    // Only Arabic tokens carry clitics.
    if token.kind != TokenKind::Arabic {
        return out;
    }

    let raw = token.raw.as_str();
    let base = token.span.start;
    let len = raw.len();

    let (procs, proc_off) = peel_proclitics(raw, base);
    let enc = peel_enclitic(raw, base, len);
    let enc_start = enc.as_ref().map(|(_, s)| *s).unwrap_or(len);
    let enc_clitic = enc.as_ref().map(|(c, _)| c.clone());

    // B: proclitics + enclitic (stem is the middle).
    if !procs.is_empty() && enc_clitic.is_some() {
        push_candidate(
            &mut out,
            raw,
            procs.clone(),
            enc_clitic.clone(),
            proc_off,
            enc_start,
            CONF_BOTH_AFFIXES,
        );
    }
    // C: proclitics only.
    if !procs.is_empty() {
        push_candidate(
            &mut out,
            raw,
            procs.clone(),
            None,
            proc_off,
            len,
            CONF_SINGLE_AFFIX,
        );
    }
    // D: enclitic only.
    if enc_clitic.is_some() {
        push_candidate(
            &mut out,
            raw,
            SmallVec::new(),
            enc_clitic,
            0,
            enc_start,
            CONF_SINGLE_AFFIX,
        );
    }

    out.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then_with(|| a.stem.surface.cmp(&b.stem.surface))
    });

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenize::tokenize;

    /// Split the first token of `s`.
    fn split_first(s: &str) -> SmallVec<[CliticSplit; 4]> {
        let toks = tokenize(s);
        split(&toks[0])
    }

    fn has_split(forest: &[CliticSplit], procs: &[&str], stem: &str, encs: &[&str]) -> bool {
        forest.iter().any(|c| {
            c.proclitics
                .iter()
                .map(|p| p.form.as_str())
                .eq(procs.iter().copied())
                && c.stem.surface.as_str() == stem
                && c.enclitics
                    .iter()
                    .map(|e| e.form.as_str())
                    .eq(encs.iter().copied())
        })
    }

    #[test]
    fn identity_always_present_and_first() {
        let forest = split_first("كتاب");
        assert!(has_split(&forest, &[], "كتاب", &[]));
        // Identity (no clitics) must rank first for a clean stem.
        assert!(forest[0].proclitics.is_empty() && forest[0].enclitics.is_empty());
    }

    #[test]
    fn non_arabic_token_only_identity() {
        let forest = split_first("hello");
        assert_eq!(forest.len(), 1);
        assert!(forest[0].proclitics.is_empty());
    }

    #[test]
    fn peels_definite_article_with_conjunction_and_preposition() {
        // وبالكتاب -> و + ب + ال + كتاب
        let forest = split_first("وبالكتاب");
        assert!(has_split(&forest, &["و", "ب", "ال"], "كتاب", &[]));
    }

    #[test]
    fn peels_enclitic_pronoun() {
        // كتابه -> كتاب + ه (his book). Note it also over-generates a ك- reading.
        let forest = split_first("كتابه");
        assert!(has_split(&forest, &[], "كتاب", &["ه"]));
    }

    #[test]
    fn clitic_spans_are_raw_anchored() {
        let input = "وبالكتاب";
        let toks = tokenize(input);
        let forest = split(&toks[0]);
        for cand in &forest {
            for c in cand.proclitics.iter().chain(cand.enclitics.iter()) {
                assert_eq!(
                    &input[c.span.start as usize..c.span.end as usize],
                    c.form.as_str(),
                    "clitic span must slice the raw input to the clitic form",
                );
            }
        }
    }

    #[test]
    fn respects_min_stem_length() {
        // «له» must not be peeled into ل + (empty/1-char) stem; identity stays.
        let forest = split_first("له");
        assert!(forest
            .iter()
            .all(|c| char_count(c.stem.surface.as_str()) >= 1));
        assert!(has_split(&forest, &[], "له", &[]));
    }

    #[test]
    fn forest_is_bounded() {
        // At most 4 candidates (identity + B + C + D).
        let forest = split_first("وبكتابهم");
        assert!(forest.len() <= 4);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_always_has_identity_and_never_panics(s in "\\PC{0,40}") {
            let toks = tokenize(&s);
            for t in &toks {
                let forest = split(t);
                prop_assert!(!forest.is_empty(), "forest must always contain identity");
            }
        }

        #[test]
        fn prop_deterministic(s in "\\PC{0,40}") {
            let toks = tokenize(&s);
            for t in &toks {
                prop_assert_eq!(split(t), split(t));
            }
        }

        #[test]
        fn prop_clitic_spans_within_token(s in "\\PC{0,40}") {
            let toks = tokenize(&s);
            for t in &toks {
                for cand in split(t) {
                    for c in cand.proclitics.iter().chain(cand.enclitics.iter()) {
                        prop_assert!(c.span.start >= t.span.start && c.span.end <= t.span.end);
                    }
                }
            }
        }
    }
}
