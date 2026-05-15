//! Clitic splitting — proclitic and enclitic morphemes.
//!
//! Operates on Arabic tokens; recognizes:
//! - Proclitics: و, ف, ب, ل, ك, ال, س, لِ
//! - Enclitics: pronominal suffixes (ـه ـها ـك ـكم ـنا etc.)
//!
//! Returns the set of plausible splits (ambiguity preserved as a forest), not
//! a single best split. Multiple splits arise when a surface form admits more
//! than one segmentation (e.g. ب could be the preposition or part of the stem).

use crate::tokenize::Token;
use qalam_core::{Clitic, Conf, Stem};
use smallvec::SmallVec;

/// One plausible decomposition of a token into clitics + stem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliticSplit {
    pub proclitics: SmallVec<[Clitic; 4]>,
    pub stem: Stem,
    pub enclitics: SmallVec<[Clitic; 2]>,
    pub confidence: Conf,
}

/// Compute the set of plausible clitic splits for a token.
///
/// Ordering: returned splits are sorted by `(confidence DESC, canonical_id ASC)`.
pub fn split(token: &Token) -> SmallVec<[CliticSplit; 4]> {
    // TODO(Phase 1, Stage 1): FST-backed clitic matcher with stable iteration.
    let _ = token;
    todo!("clitics::split: implemented in next PR")
}
