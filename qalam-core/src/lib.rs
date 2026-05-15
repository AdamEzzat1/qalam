//! Core IR types and determinism primitives for Qalam.
//!
//! This crate has no I/O and minimal dependencies. Every other Qalam crate
//! depends on it. The types defined here form the public API of the entire
//! pipeline; they are deliberately stable.
//!
//! See `DESIGN.md` for the full architectural context.

pub mod conf;
pub mod error;
pub mod morph_ir;
pub mod provenance;
pub mod trace;

pub use conf::Conf;
pub use error::{QalamError, Result};
pub use morph_ir::{
    Aspect, ByteSpan, Case, Clitic, CliticId, FeatureSet, Gender, Mood, MorphAnalysis, MorphForest,
    Number, PatternId, Person, Pos, Register, Root, State, Stem, Voice,
};
pub use provenance::{ContentHash, LexEntryId, Provenance, RuleId};
pub use trace::{Trace, TraceEvent, TraceLevel};
