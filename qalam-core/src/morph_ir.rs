//! Morphological IR — the central data structures for Phase 1.
//!
//! See `DESIGN.md` §5.2 for the architectural rationale behind these types.

use crate::conf::Conf;
use crate::provenance::Provenance;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use smol_str::SmolStr;

/// A byte span into the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start: u32,
    pub end: u32,
}

impl ByteSpan {
    pub fn new(start: u32, end: u32) -> Self {
        debug_assert!(
            start <= end,
            "ByteSpan: start ({start}) must be <= end ({end})"
        );
        Self { start, end }
    }

    pub fn len(self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// A 3- or 4-consonant Arabic root (e.g. ك-ت-ب).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Root {
    pub radicals: SmallVec<[char; 4]>,
}

/// A stable identifier for a morphological pattern in the static pattern table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PatternId(pub u32);

/// A clitic identifier referencing the static clitic table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CliticId(pub u16);

/// A pro-clitic or en-clitic with its source span.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Clitic {
    pub form: SmolStr,
    pub clitic_id: CliticId,
    pub span: ByteSpan,
}

/// The morphological core of a token: stem surface and (optionally) lemma.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stem {
    pub surface: SmolStr,
    pub lemma: Option<SmolStr>,
}

/// Morpho-syntactic features. All optional — many tokens (foreign words,
/// particles) carry only some.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeatureSet {
    pub pos: Option<Pos>,
    pub gender: Option<Gender>,
    pub number: Option<Number>,
    pub person: Option<Person>,
    pub case: Option<Case>,
    pub state: Option<State>,
    pub voice: Option<Voice>,
    pub mood: Option<Mood>,
    pub aspect: Option<Aspect>,
    pub register: Option<Register>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pos {
    Noun,
    Verb,
    Adj,
    Adv,
    Pron,
    Prep,
    Conj,
    Part,
    Num,
    Det,
    Punct,
    Foreign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Masc,
    Fem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Number {
    Sg,
    Du,
    Pl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Person {
    First,
    Second,
    Third,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Case {
    Nom,
    Acc,
    Gen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum State {
    Definite,
    Indefinite,
    Construct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Voice {
    Active,
    Passive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mood {
    Indicative,
    Subjunctive,
    Jussive,
    Imperative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Aspect {
    Perfective,
    Imperfective,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Register {
    Msa,
    Colloquial,
}

/// A single morphological analysis of a token.
///
/// This is the central data structure of Phase 1.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MorphAnalysis {
    pub surface: SmolStr,
    pub span: ByteSpan,
    pub proclitics: SmallVec<[Clitic; 4]>,
    pub stem: Stem,
    pub enclitics: SmallVec<[Clitic; 2]>,
    pub root: Option<Root>,
    pub pattern: Option<PatternId>,
    pub features: FeatureSet,
    pub confidence: Conf,
    pub provenance: Provenance,
}

/// An ambiguity set of analyses for a single token.
///
/// Analyses are stored sorted by `(confidence DESC, canonical_id ASC)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MorphForest {
    pub token: SmolStr,
    pub span: ByteSpan,
    pub analyses: SmallVec<[MorphAnalysis; 4]>,
    /// True if the analyzer hit the configured `max_analyses_per_token` cap
    /// and dropped lower-confidence candidates.
    pub truncated: bool,
}

impl MorphForest {
    pub fn new(token: SmolStr, span: ByteSpan) -> Self {
        Self {
            token,
            span,
            analyses: SmallVec::new(),
            truncated: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.analyses.is_empty()
    }

    /// The highest-confidence analysis, or `None` if the forest is empty.
    pub fn best(&self) -> Option<&MorphAnalysis> {
        self.analyses.first()
    }
}
