//! Arabic text preprocessing: Unicode normalization, tokenization, clitic splitting.
//!
//! Phase 1, Stage 1 of the Qalam pipeline. See `DESIGN.md` §5.

pub mod clitics;
pub mod tokenize;
pub mod unicode;
