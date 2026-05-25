//! Templatic Arabic morphology engine.
//!
//! Phase 1, Stage 2 of the Qalam pipeline. See `DESIGN.md` §5.

pub mod analyzer;
pub mod lexicon;
pub mod patterns;
pub mod roots;

pub use analyzer::{Analyzer, AnalyzerConfig, BasicAnalyzer, DialectHint};
pub use lexicon::{LexEntry, Lexicon};
pub use patterns::{Pattern, PatternMatch, PatternTable, Slot};
pub use roots::{analyze_stem, best_root, RootCandidate};
