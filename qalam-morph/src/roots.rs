//! Root extraction — recovering the 3- or 4-consonant root from a stem given a
//! matched pattern's slot positions.
//!
//! Weak roots (containing و, ي, or ا) interact with patterns nonlinearly; this
//! module is responsible for the surface-to-root mapping that handles those
//! cases deterministically.

use qalam_core::Root;

/// Extract the root from a stem given the matched pattern's template.
///
/// Returns `None` if the stem and template are incompatible (i.e. the pattern
/// match was false-positive).
pub fn extract_root(stem: &str, pattern_template: &str) -> Option<Root> {
    // TODO(Phase 1, Stage 2): handle weak roots (و/ي/ا interactions),
    // gemination (شد), and hamza variants deterministically.
    let _ = (stem, pattern_template);
    todo!("root extraction: implemented in next PR")
}
