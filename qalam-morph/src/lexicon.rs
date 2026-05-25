//! Lexicon access.
//!
//! v0.1 ships a [`BootstrapLexicon`]: a small, hand-curated set of common strong
//! roots + particles embedded from `data/lexicon.toml`. It exists to wire
//! lexicon lookup into the analyzer end-to-end — confirming extracted roots and
//! giving provenance a real `lexicon_hash`. The full open-source lexicon (an
//! mmap'd FST built by `qalam-lexicon-builder`) is Stage 1.5b; when it lands it
//! implements the same [`Lexicon`] trait and the analyzer is unchanged.

use qalam_core::{ContentHash, LexEntryId, Root};
use serde::Deserialize;
use std::collections::BTreeMap;

/// A read-only lexicon. The analyzer depends only on this trait, so the
/// bootstrap can be swapped for an FST-backed lexicon without analyzer changes.
pub trait Lexicon: Send + Sync {
    /// The entry id if `root` is a known root, else `None`.
    fn root_id(&self, root: &Root) -> Option<LexEntryId>;
    /// The entry id if `surface` (normalized) is a known particle, else `None`.
    fn particle_id(&self, surface: &str) -> Option<LexEntryId>;
    /// The (entry id, root) if `surface` (normalized, diacritics preserved) is a
    /// known irregular form whose root is not recoverable by rule, else `None`.
    fn irregular(&self, surface: &str) -> Option<(LexEntryId, Root)>;
    /// Content hash of the lexicon; recorded in every analysis's provenance.
    fn hash(&self) -> ContentHash;
}

/// A lexicon entry (reserved for the richer FST-backed lexicon; the bootstrap
/// only needs ids).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexEntry {
    pub id: LexEntryId,
    pub lemma: smol_str::SmolStr,
}

const LEXICON_TOML: &str = include_str!("../data/lexicon.toml");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LexiconFile {
    #[serde(default)]
    roots: Vec<String>,
    #[serde(default)]
    particles: Vec<String>,
    #[serde(default)]
    irregulars: BTreeMap<String, String>,
}

/// The embedded bootstrap lexicon.
#[derive(Debug, Clone)]
pub struct BootstrapLexicon {
    /// Root (radicals joined, e.g. "كتب") -> entry id.
    roots: BTreeMap<String, LexEntryId>,
    /// Particle surface (normalized) -> entry id.
    particles: BTreeMap<String, LexEntryId>,
    /// Irregular surface (normalized, diacritics preserved) -> (id, root).
    irregulars: BTreeMap<String, (LexEntryId, Root)>,
    hash: ContentHash,
}

impl Default for BootstrapLexicon {
    fn default() -> Self {
        Self::load()
    }
}

impl BootstrapLexicon {
    /// Parse the embedded lexicon. Ids are assigned deterministically: roots
    /// get 1.., particles get 1_000_000.. (disjoint ranges for clarity).
    pub fn load() -> Self {
        let parsed: LexiconFile =
            toml::from_str(LEXICON_TOML).expect("lexicon.toml: malformed (compile-time embed)");
        let mut roots = BTreeMap::new();
        for (i, r) in parsed.roots.iter().enumerate() {
            roots.insert(r.clone(), LexEntryId(i as u32 + 1));
        }
        let mut particles = BTreeMap::new();
        for (i, p) in parsed.particles.iter().enumerate() {
            particles.insert(p.clone(), LexEntryId(1_000_000 + i as u32));
        }
        // BTreeMap iteration is sorted -> deterministic id assignment.
        let mut irregulars = BTreeMap::new();
        for (i, (surface, root_str)) in parsed.irregulars.iter().enumerate() {
            let root = Root {
                radicals: root_str.chars().collect(),
            };
            irregulars.insert(surface.clone(), (LexEntryId(2_000_000 + i as u32), root));
        }
        Self {
            roots,
            particles,
            irregulars,
            hash: ContentHash::of(LEXICON_TOML.as_bytes()),
        }
    }

    fn root_key(root: &Root) -> String {
        root.radicals.iter().collect()
    }
}

impl Lexicon for BootstrapLexicon {
    fn root_id(&self, root: &Root) -> Option<LexEntryId> {
        self.roots.get(&Self::root_key(root)).copied()
    }

    fn particle_id(&self, surface: &str) -> Option<LexEntryId> {
        self.particles.get(surface).copied()
    }

    fn irregular(&self, surface: &str) -> Option<(LexEntryId, Root)> {
        self.irregulars.get(surface).cloned()
    }

    fn hash(&self) -> ContentHash {
        self.hash.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    fn root(s: &str) -> Root {
        Root {
            radicals: s.chars().collect(),
        }
    }

    #[test]
    fn known_roots_are_found() {
        let lex = BootstrapLexicon::load();
        assert!(lex.root_id(&root("كتب")).is_some());
        assert!(lex.root_id(&root("سلم")).is_some());
        assert!(lex.root_id(&root("درس")).is_some());
    }

    #[test]
    fn unknown_root_is_not_found() {
        let lex = BootstrapLexicon::load();
        // A nonsense skeleton not in the bootstrap.
        assert!(lex.root_id(&root("خزق")).is_none());
    }

    #[test]
    fn weak_roots_are_present() {
        let lex = BootstrapLexicon::load();
        assert!(lex.root_id(&root("قول")).is_some()); // hollow
        assert!(lex.root_id(&root("رمي")).is_some()); // defective
        assert!(lex.root_id(&root("وصل")).is_some()); // mithal
                                                      // The spurious hollow alternative for قال is NOT in the lexicon.
        assert!(lex.root_id(&root("قيل")).is_none());
    }

    #[test]
    fn irregular_forms_resolve_to_roots() {
        let lex = BootstrapLexicon::load();
        // قِ (with kasra) -> و-ق-ي, the famous one-letter imperative.
        let (_, r) = lex.irregular("قِ").expect("قِ should be an irregular");
        assert_eq!(r.radicals.as_slice(), ['و', 'ق', 'ي']);
        // قل -> ق-و-ل
        let (_, r2) = lex.irregular("قل").expect("قل should be an irregular");
        assert_eq!(r2.radicals.as_slice(), ['ق', 'و', 'ل']);
        // A normal word is not an irregular.
        assert!(lex.irregular("كتاب").is_none());
    }

    #[test]
    fn particles_are_found_in_normalized_form() {
        let lex = BootstrapLexicon::load();
        assert!(lex.particle_id("في").is_some());
        // إلى normalizes to الى, which is how it is stored.
        assert!(lex.particle_id("الى").is_some());
        assert!(lex.particle_id("كتاب").is_none());
    }

    #[test]
    fn hash_is_stable_and_blake3_of_file() {
        let lex = BootstrapLexicon::load();
        assert_eq!(lex.hash(), ContentHash::of(LEXICON_TOML.as_bytes()));
        assert_eq!(lex.hash().as_str().len(), 64);
    }

    #[test]
    fn root_radicals_smallvec_lookup() {
        // Sanity: a Root built from a SmallVec literal looks up the same.
        let lex = BootstrapLexicon::load();
        let r = Root {
            radicals: smallvec!['ك', 'ت', 'ب'],
        };
        assert!(lex.root_id(&r).is_some());
    }
}
