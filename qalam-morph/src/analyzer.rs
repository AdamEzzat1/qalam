//! The top-level morphological analyzer.
//!
//! Composes the lower stages (clitic splitting, pattern matching, root
//! extraction, lexicon lookup) into a single `analyze_token` API that returns
//! a `MorphForest` of ranked analyses.

use qalam_core::{ContentHash, MorphForest, TraceLevel};

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
