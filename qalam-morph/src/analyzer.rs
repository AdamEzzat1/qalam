//! The top-level morphological analyzer.
//!
//! `BasicAnalyzer` composes the lower stages into a `MorphForest`:
//! clitic splitting (qalam-text) × pattern/root matching (this crate). Each
//! `(clitic split, root candidate)` pair becomes one `MorphAnalysis`; their
//! confidences combine via `Conf::and`; the forest is ranked and top-k capped.
//!
//! Provenance at this depth: no lexicon exists yet, so `lexicon_hash` is a
//! documented sentinel (`qalam:no-lexicon:v0`) meaning "no lexicon consulted",
//! and `provenance.rules` records the pattern IDs that actually fired — the
//! real provenance content available before a lexicon ships.

use qalam_core::{
    ByteSpan, Conf, ContentHash, FeatureSet, MorphAnalysis, MorphForest, Provenance, RuleId, Stem,
    TraceLevel,
};
use qalam_text::clitics::{self, CliticSplit};
use qalam_text::tokenize::{tokenize, Token, TokenKind};
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::patterns::{PatternMatch, PatternTable};
use crate::roots;

/// Sentinel hashed into `Provenance.lexicon_hash` until a real lexicon ships.
const NO_LEXICON: &[u8] = b"qalam:no-lexicon:v0";

/// Confidence factor applied to an analysis that found NO strong root.
///
/// Recovering a known pattern is positive evidence, so a rootless analysis is
/// down-weighted relative to a rooted one on the same clitic split. Without
/// this, AND-combining `clitic × pattern` (both < 1) would make rooted analyses
/// rank *below* their rootless siblings — surfacing the spurious reading. This
/// is a deterministic placeholder prior until lexicon validation arrives.
const UNANALYZED_PENALTY: f32 = 0.3;

/// Runtime configuration for the analyzer.
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Maximum analyses kept per token before truncating to the top-k.
    /// `0` means unlimited (strict mode).
    pub max_analyses_per_token: u8,
    /// If `false`, unrecognized stems return an empty forest rather than
    /// invoking the deterministic OOV backoff.
    pub backoff_enabled: bool,
    /// How much detail to record in traces.
    pub trace_level: TraceLevel,
    /// Hint about which dialect the input is expected to be in.
    pub dialect_hint: DialectHint,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            max_analyses_per_token: 8,
            backoff_enabled: true,
            trace_level: TraceLevel::None,
            dialect_hint: DialectHint::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialectHint {
    Msa,
    Egyptian,
    Auto,
}

/// The top-level morphological analyzer trait.
///
/// Implementors must respect the determinism contract: for fixed
/// `(input, lexicon_hash, config_hash)`, the returned `MorphForest` and any
/// emitted trace events must be byte-identical across runs and platforms.
pub trait Analyzer: Send + Sync {
    fn analyze_token(&self, token: &str) -> MorphForest;
    fn analyze_text(&self, text: &str) -> Vec<MorphForest>;
    fn lexicon_hash(&self) -> ContentHash;
    fn config_hash(&self) -> ContentHash;
}

/// The v0.1 analyzer: clitic splitting + strong-root pattern matching.
#[derive(Debug, Clone)]
pub struct BasicAnalyzer {
    config: AnalyzerConfig,
    patterns: PatternTable,
}

impl Default for BasicAnalyzer {
    fn default() -> Self {
        Self::new(AnalyzerConfig::default())
    }
}

impl BasicAnalyzer {
    pub fn new(config: AnalyzerConfig) -> Self {
        Self {
            config,
            patterns: PatternTable::builtin(),
        }
    }

    /// Analyze a single already-segmented token.
    fn analyze_one(&self, token: &Token) -> MorphForest {
        let mut forest = MorphForest::new(token.raw.clone(), token.span);

        // Non-Arabic tokens get a single trivial analysis (whole token = stem).
        if token.kind != TokenKind::Arabic {
            forest.analyses.push(self.leaf(token));
            return forest;
        }

        let mut analyses: Vec<MorphAnalysis> = Vec::new();
        for split in clitics::split(token) {
            let roots = roots::analyze_stem(split.stem.surface.as_str(), &self.patterns);
            if roots.is_empty() {
                analyses.push(self.build(token, &split, None));
            } else {
                for rc in &roots {
                    analyses.push(self.build(token, &split, Some(rc)));
                }
            }
        }

        // Deterministic ranking: confidence DESC, then a total canonical key.
        analyses.sort_by(|a, b| {
            b.confidence
                .cmp(&a.confidence)
                .then_with(|| canonical_key(a).cmp(&canonical_key(b)))
        });

        let cap = self.config.max_analyses_per_token as usize;
        if cap > 0 && analyses.len() > cap {
            analyses.truncate(cap);
            forest.truncated = true;
        }
        forest.analyses = analyses.into_iter().collect();
        forest
    }

    fn build(
        &self,
        token: &Token,
        split: &CliticSplit,
        rc: Option<&PatternMatch>,
    ) -> MorphAnalysis {
        let (root, pattern, confidence, pos) = match rc {
            Some(m) => (
                Some(m.root.clone()),
                Some(m.pattern),
                split.confidence.and(m.confidence),
                self.patterns.pos_of(m.pattern),
            ),
            None => (
                None,
                None,
                split.confidence.and(Conf::clamp(UNANALYZED_PENALTY)),
                None,
            ),
        };
        let features = FeatureSet {
            pos,
            ..FeatureSet::default()
        };
        let mut provenance = Provenance::new(self.lexicon_hash());
        if let Some(m) = rc {
            provenance.rules.push(RuleId(m.pattern.0));
        }
        MorphAnalysis {
            surface: token.raw.clone(),
            span: token.span,
            proclitics: split.proclitics.clone(),
            stem: split.stem.clone(),
            enclitics: split.enclitics.clone(),
            root,
            pattern,
            features,
            confidence,
            provenance,
        }
    }

    fn leaf(&self, token: &Token) -> MorphAnalysis {
        MorphAnalysis {
            surface: token.raw.clone(),
            span: token.span,
            proclitics: SmallVec::new(),
            stem: Stem {
                surface: token.raw.clone(),
                lemma: None,
            },
            enclitics: SmallVec::new(),
            root: None,
            pattern: None,
            features: FeatureSet::default(),
            confidence: Conf::clamp(1.0),
            provenance: Provenance::new(self.lexicon_hash()),
        }
    }
}

impl Analyzer for BasicAnalyzer {
    fn analyze_token(&self, token: &str) -> MorphForest {
        match tokenize(token).first() {
            Some(t) => self.analyze_one(t),
            None => MorphForest::new(SmolStr::new(token), ByteSpan::new(0, 0)),
        }
    }

    fn analyze_text(&self, text: &str) -> Vec<MorphForest> {
        tokenize(text)
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace)
            .map(|t| self.analyze_one(t))
            .collect()
    }

    fn lexicon_hash(&self) -> ContentHash {
        ContentHash::of(NO_LEXICON)
    }

    fn config_hash(&self) -> ContentHash {
        let c = &self.config;
        let s = format!(
            "max={};backoff={};trace={:?};dialect={:?}",
            c.max_analyses_per_token, c.backoff_enabled, c.trace_level, c.dialect_hint
        );
        ContentHash::of(s.as_bytes())
    }
}

/// A total, deterministic tie-break key for ranking analyses of equal
/// confidence.
fn canonical_key(a: &MorphAnalysis) -> String {
    let root: String = a
        .root
        .as_ref()
        .map(|r| r.radicals.iter().collect())
        .unwrap_or_default();
    let pat = a.pattern.map(|p| p.0).unwrap_or(0);
    let procs: String = a.proclitics.iter().map(|c| c.form.as_str()).collect();
    let encs: String = a.enclitics.iter().map(|c| c.form.as_str()).collect();
    format!("{root}|{pat}|{}|{procs}|{encs}", a.stem.surface)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_active_participle() {
        let a = BasicAnalyzer::default();
        let forest = a.analyze_token("كاتب");
        // Some analysis must recover the ك-ت-ب root via فاعل.
        assert!(forest.analyses.iter().any(|an| {
            an.root
                .as_ref()
                .is_some_and(|r| r.radicals.as_slice() == ['ك', 'ت', 'ب'])
        }));
    }

    #[test]
    fn analyzes_with_clitics() {
        let a = BasicAnalyzer::default();
        // الكتاب -> ال + كتاب (فعال) -> root ك-ت-ب in some analysis.
        let forest = a.analyze_token("الكتاب");
        assert!(forest.analyses.iter().any(|an| {
            an.proclitics.iter().any(|c| c.form.as_str() == "ال")
                && an
                    .root
                    .as_ref()
                    .is_some_and(|r| r.radicals.as_slice() == ['ك', 'ت', 'ب'])
        }));
    }

    #[test]
    fn weak_root_word_still_analyzes_without_root() {
        let a = BasicAnalyzer::default();
        // قال: no strong root, but the forest is non-empty (clitic-level).
        let forest = a.analyze_token("قال");
        assert!(!forest.analyses.is_empty());
        assert!(forest
            .analyses
            .iter()
            .all(|an| an.root.is_none() || !an.root.as_ref().unwrap().radicals.contains(&'ا')));
    }

    #[test]
    fn deterministic() {
        let a = BasicAnalyzer::default();
        assert_eq!(
            a.analyze_text("وبالكتاب مدرسة"),
            a.analyze_text("وبالكتاب مدرسة")
        );
    }

    #[test]
    fn forest_respects_cap() {
        let cfg = AnalyzerConfig {
            max_analyses_per_token: 2,
            ..AnalyzerConfig::default()
        };
        let a = BasicAnalyzer::new(cfg);
        for forest in a.analyze_text("وبالكتاب") {
            assert!(forest.analyses.len() <= 2);
        }
    }

    #[test]
    fn lexicon_hash_is_stable_sentinel() {
        let a = BasicAnalyzer::default();
        assert_eq!(a.lexicon_hash(), ContentHash::of(b"qalam:no-lexicon:v0"));
    }
}
