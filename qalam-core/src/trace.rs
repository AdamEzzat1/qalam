//! Trace types — JSONL events recording inference steps.
//!
//! Traces are part of the determinism contract: for fixed input + lexicon +
//! config, the sequence of trace events is bit-identical. See `DESIGN.md` §4.1.

use crate::conf::Conf;
use crate::morph_ir::{ByteSpan, PatternId, Root};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use smol_str::SmolStr;

/// How detailed traces should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceLevel {
    /// No tracing.
    #[default]
    None,
    /// Top-level events only: tokenize, clitic_split, final.
    Concise,
    /// Every inference step including pattern matches and lexicon lookups.
    Full,
}

/// A single trace event.
///
/// Stable schema: new variants are semver-minor; renaming or removing existing
/// variants is semver-major. JSONL serialization uses the `t` discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum TraceEvent {
    Tokenize {
        span: ByteSpan,
        text: SmolStr,
    },
    CliticSplit {
        token_id: u32,
        proclitic_ids: SmallVec<[u16; 4]>,
        stem: SmolStr,
        enclitic_ids: SmallVec<[u16; 2]>,
    },
    PatternMatch {
        stem: SmolStr,
        pattern: PatternId,
        root: Option<Root>,
        conf: Conf,
        rule_id: u32,
    },
    LexLookup {
        stem: SmolStr,
        entry_id: u32,
        confirms: bool,
    },
    Final {
        token: SmolStr,
        analysis_count: u32,
        best_conf: Conf,
    },
}

/// A sequence of trace events emitted at a configured level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub level: TraceLevel,
    pub events: Vec<TraceEvent>,
}

impl Trace {
    pub fn new(level: TraceLevel) -> Self {
        Self {
            level,
            events: Vec::new(),
        }
    }

    /// Record an event if the trace level allows it.
    ///
    /// `None` level discards everything; `Concise` keeps only `Tokenize`,
    /// `CliticSplit`, and `Final`; `Full` keeps everything.
    pub fn record(&mut self, event: TraceEvent) {
        let keep = match (self.level, &event) {
            (TraceLevel::None, _) => false,
            (TraceLevel::Concise, TraceEvent::Tokenize { .. }) => true,
            (TraceLevel::Concise, TraceEvent::CliticSplit { .. }) => true,
            (TraceLevel::Concise, TraceEvent::Final { .. }) => true,
            (TraceLevel::Concise, _) => false,
            (TraceLevel::Full, _) => true,
        };
        if keep {
            self.events.push(event);
        }
    }
}
