//! Unicode normalization for Arabic text.
//!
//! Applies NFC plus an Arabic-specific normative fold table:
//! - Alef variants (أ إ آ ٱ) folded to ا
//! - Tatweel ـ stripped
//! - Hamza variants normalized
//! - Teh marbuta ة preserved (not folded)
//! - Alef maqsura ى preserved as distinct from ي
//!
//! The fold table is normative — its hash participates in every analysis's
//! provenance. See `DESIGN.md` §5.3.

use qalam_core::ContentHash;

/// Normalize an Arabic input string.
///
/// Applies NFC and the Arabic normative fold table. Idempotent:
/// `normalize(normalize(s)) == normalize(s)` for all valid UTF-8 inputs.
pub fn normalize(input: &str) -> String {
    // TODO(Phase 1, Stage 1): implement NFC + Arabic-specific folds.
    let _ = input;
    todo!("normalize: implemented in next PR")
}

/// The hash of the active fold table. Changes to the fold table invalidate
/// cached outputs.
pub fn fold_table_hash() -> ContentHash {
    // TODO(Phase 1, Stage 1): the fold table will be a TOML data file embedded
    // via `include_bytes!`. The hash is computed at startup.
    todo!("fold_table_hash: implemented in next PR")
}
