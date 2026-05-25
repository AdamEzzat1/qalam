//! The top-level morphological analyzer.
//!
//! `BasicAnalyzer` composes the lower stages into a `MorphForest`:
//! clitic splitting (qalam-text) × pattern/root matching (this crate). Each
//! `(clitic split, root candidate)` pair becomes one `MorphAnalysis`; their
//! confidences combine via `Conf::and`; the forest is ranked and top-k capped.
//!
//! Provenance: `lexicon_hash` is the hash of the bootstrap lexicon. An analysis
//! whose extracted root is confirmed by the lexicon records the entry id in
//! `provenance.lex_entries` and is promoted; unconfirmed (likely spurious)
//! pattern matches are down-weighted; recognized particles are promoted.

use qalam_core::{
    ByteSpan, Conf, ContentHash, FeatureSet, MorphAnalysis, MorphForest, Pos, Provenance, RuleId,
    Stem, TraceLevel,
};
use qalam_text::clitics::{self, CliticSplit};
use qalam_text::tokenize::{tokenize, Token, TokenKind};
use qalam_text::unicode;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::lexicon::{BootstrapLexicon, Lexicon};
use crate::patterns::{PatternMatch, PatternTable};
use crate::roots;

/// Confidence floor contributed by a lexicon-confirmed root (via noisy-or).
/// Confirming an extracted root against the lexicon is strong positive evidence.
const LEXICON_CONFIRM: f32 = 0.7;
/// Confidence factor for an extracted root NOT in the lexicon — likely a
/// spurious pattern match, so down-weighted.
const UNCONFIRMED_PENALTY: f32 = 0.4;
/// Confidence floor for a recognized particle (closed-class function word).
const PARTICLE_CONFIRM: f32 = 0.85;
/// Confidence factor for an analysis that found neither a root nor a particle.
///
/// Without it, AND-combining `clitic × pattern` (both < 1) would make rooted
/// analyses rank *below* their rootless siblings — surfacing spurious readings.
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
    lexicon: BootstrapLexicon,
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
            lexicon: BootstrapLexicon::load(),
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
        let mut provenance = Provenance::new(self.lexicon_hash());
        let (root, pattern, pos, confidence) = match rc {
            Some(m) => {
                provenance.rules.push(RuleId(m.pattern.0));
                let base = split.confidence.and(m.confidence);
                let pos = self.patterns.pos_of(m.pattern);
                match self.lexicon.root_id(&m.root) {
                    Some(eid) => {
                        // Confirmed root: strong positive evidence -> promote.
                        provenance.lex_entries.push(eid);
                        (
                            Some(m.root.clone()),
                            Some(m.pattern),
                            pos,
                            base.or(Conf::clamp(LEXICON_CONFIRM)),
                        )
                    }
                    None => {
                        // Extracted but unconfirmed: likely spurious -> demote.
                        (
                            Some(m.root.clone()),
                            Some(m.pattern),
                            pos,
                            base.and(Conf::clamp(UNCONFIRMED_PENALTY)),
                        )
                    }
                }
            }
            None => {
                let norm = unicode::normalize(split.stem.surface.as_str());
                // 1. Irregular form (e.g. قِ -> و-ق-ي): a lexically-known root
                //    with no recoverable pattern. Promoted like a confirmed root.
                if let Some((eid, root)) = self.lexicon.irregular(&norm) {
                    provenance.lex_entries.push(eid);
                    (
                        Some(root),
                        None,
                        Some(Pos::Verb),
                        split.confidence.or(Conf::clamp(LEXICON_CONFIRM)),
                    )
                } else {
                    // 2. Particle, else 3. genuinely unanalyzed.
                    let key = unicode::strip_tashkil(&norm);
                    match self.lexicon.particle_id(&key) {
                        Some(eid) => {
                            provenance.lex_entries.push(eid);
                            (
                                None,
                                None,
                                Some(Pos::Part),
                                split.confidence.or(Conf::clamp(PARTICLE_CONFIRM)),
                            )
                        }
                        None => (
                            None,
                            None,
                            None,
                            split.confidence.and(Conf::clamp(UNANALYZED_PENALTY)),
                        ),
                    }
                }
            }
        };
        let features = FeatureSet {
            pos,
            ..FeatureSet::default()
        };
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
        self.lexicon.hash()
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
    fn hollow_weak_root_is_confirmed_and_ranks_first() {
        let a = BasicAnalyzer::default();
        // قال: hollow enumerates ق-و-ل / ق-ي-ل; only ق-و-ل is in the lexicon,
        // so it is confirmed and ranks first. ا is never claimed as a radical.
        let top = a
            .analyze_token("قال")
            .best()
            .cloned()
            .expect("non-empty forest");
        assert_eq!(
            top.root.as_ref().map(|r| r.radicals.as_slice()),
            Some(['ق', 'و', 'ل'].as_slice())
        );
        assert!(
            !top.provenance.lex_entries.is_empty(),
            "confirmed -> lex entry"
        );
    }

    #[test]
    fn irregular_imperative_resolves_to_root() {
        let a = BasicAnalyzer::default();
        // قِ (with kasra) -> و-ق-ي via the irregulars table — the cited case.
        let top = a
            .analyze_token("قِ")
            .best()
            .cloned()
            .expect("non-empty forest");
        assert_eq!(
            top.root.as_ref().map(|r| r.radicals.as_slice()),
            Some(['و', 'ق', 'ي'].as_slice())
        );
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
    fn lexicon_hash_is_the_bootstrap_hash() {
        let a = BasicAnalyzer::default();
        assert_eq!(
            a.lexicon_hash(),
            crate::lexicon::BootstrapLexicon::load().hash()
        );
        assert_eq!(a.lexicon_hash().as_str().len(), 64);
    }

    #[test]
    fn lexicon_confirmed_root_ranks_first() {
        let a = BasicAnalyzer::default();
        // كتاب: root ك-ت-ب is in the bootstrap lexicon, so the rooted analysis
        // must be promoted above the spurious ك- proclitic split.
        let best = a.analyze_token("كتاب");
        let top = best.best().expect("non-empty forest");
        assert_eq!(
            top.root.as_ref().map(|r| r.radicals.as_slice()),
            Some(['ك', 'ت', 'ب'].as_slice()),
            "lexicon-confirmed root should be the top analysis",
        );
        // And its provenance records a lexicon entry.
        assert!(!top.provenance.lex_entries.is_empty());
    }

    #[test]
    fn particle_is_recognized() {
        let a = BasicAnalyzer::default();
        // في is a particle; its top analysis should be POS=Part with no root.
        let forest = a.analyze_token("في");
        let top = forest.best().expect("non-empty forest");
        assert_eq!(top.features.pos, Some(Pos::Part));
        assert!(top.root.is_none());
    }
}
