//! Templatic morphological patterns (أوزان) and skeleton matching.
//!
//! Patterns are written in the classical ف/ع/ل measure notation: `ف` is the
//! first radical, `ع` the second, `ل` the third; every other character is a
//! literal that must align with the stem. Matching is exact-length alignment
//! of a diacritic-stripped stem against each pattern: literal positions must be
//! equal, radical positions capture the root.
//!
//! v0.1 is **strong-roots-only**: a match whose captured radical is a long
//! vowel (ا و ي) or a hamza-carrier is rejected, because that signals a weak
//! root this stage cannot yet analyze correctly (e.g. قال → ق-و-ل, not ق-ا-ل).
//! Weak roots are deferred to a later sub-stage.

use qalam_core::{Conf, PatternId, Pos, Root};
use smallvec::SmallVec;
use smol_str::SmolStr;

/// How a weak radical's hidden identity is recovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeakResolution {
    /// The radical is و (recoverable, e.g. defective ending in ا).
    Waw,
    /// The radical is ي (recoverable, e.g. defective ending in ى).
    Ya,
    /// Surface does not reveal which — enumerate BOTH و and ي candidates
    /// (e.g. hollow perfect قال, whose ا hides either). The lexicon disambiguates.
    WawOrYa,
}

/// One position in a pattern template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// Captures a strong radical at this 0-based index (0=ف, 1=ع, 2=ل).
    /// Rejects long vowels / hamza-carriers.
    Radical(u8),
    /// A literal character that must match the stem exactly.
    Literal(char),
    /// A weak radical: the stem must show `surface` at this position; the
    /// radical's identity is given by `resolves`.
    Weak {
        idx: u8,
        surface: char,
        resolves: WeakResolution,
    },
}

/// A templatic pattern: its measure-notation template, parsed slots, the POS it
/// implies, and a deterministic commonness prior used as match confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub id: PatternId,
    pub template: SmolStr,
    pub slots: SmallVec<[Slot; 8]>,
    pub pos: Pos,
    pub prior: f32,
}

/// A successful match of a stem against a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternMatch {
    pub pattern: PatternId,
    pub root: Root,
    pub confidence: Conf,
}

impl Pattern {
    /// Parse a measure-notation template into slots. Panics if the template is
    /// not a well-formed triliteral measure (exactly one each of ف, ع, ل).
    fn parse(id: u32, template: &str, pos: Pos, prior: f32) -> Pattern {
        let mut slots: SmallVec<[Slot; 8]> = SmallVec::new();
        let (mut f, mut e, mut l) = (0u8, 0u8, 0u8);
        for ch in template.chars() {
            match ch {
                'ف' => {
                    slots.push(Slot::Radical(0));
                    f += 1;
                }
                'ع' => {
                    slots.push(Slot::Radical(1));
                    e += 1;
                }
                'ل' => {
                    slots.push(Slot::Radical(2));
                    l += 1;
                }
                other => slots.push(Slot::Literal(other)),
            }
        }
        assert!(
            f == 1 && e == 1 && l == 1,
            "pattern {template:?} must contain exactly one each of ف ع ل"
        );
        Pattern {
            id: PatternId(id),
            template: SmolStr::new(template),
            slots,
            pos,
            prior,
        }
    }

    /// Construct a pattern from explicit slots (used for weak patterns whose
    /// surface can't be written in ف/ع/ل notation).
    fn weak(id: u32, template: &str, slots: &[Slot], pos: Pos, prior: f32) -> Self {
        Pattern {
            id: PatternId(id),
            template: SmolStr::new(template),
            slots: slots.iter().copied().collect(),
            pos,
            prior,
        }
    }

    /// Try to match a diacritic-stripped stem (as a char slice) against this
    /// pattern. Returns the captured root(s): empty if lengths differ, a literal
    /// or weak-surface mismatches, or a strong radical slot captured a weak
    /// letter. A hollow pattern with a `WawOrYa` slot returns TWO candidates.
    pub fn try_match(&self, chars: &[char]) -> SmallVec<[Root; 2]> {
        let mut out: SmallVec<[Root; 2]> = SmallVec::new();
        if self.slots.len() != chars.len() {
            return out;
        }
        let mut radicals: [Option<char>; 3] = [None; 3];
        let mut ambiguous: Option<usize> = None;
        for (slot, &ch) in self.slots.iter().zip(chars) {
            match slot {
                Slot::Literal(l) => {
                    if *l != ch {
                        return out;
                    }
                }
                Slot::Radical(i) => {
                    if is_weak_radical(ch) {
                        return out; // strong slot must hold a strong consonant
                    }
                    radicals[*i as usize] = Some(ch);
                }
                Slot::Weak {
                    idx,
                    surface,
                    resolves,
                } => {
                    if *surface != ch {
                        return out; // expected weak realization absent
                    }
                    match resolves {
                        WeakResolution::Waw => radicals[*idx as usize] = Some('و'),
                        WeakResolution::Ya => radicals[*idx as usize] = Some('ي'),
                        WeakResolution::WawOrYa => ambiguous = Some(*idx as usize),
                    }
                }
            }
        }

        // Assemble. For an ambiguous (WawOrYa) slot, emit one root per fill.
        let fills: &[char] = if ambiguous.is_some() {
            &['و', 'ي']
        } else {
            &['\u{0}'] // single pass; the placeholder is never consulted
        };
        for &fill in fills {
            let mut rad: SmallVec<[char; 4]> = SmallVec::new();
            let mut complete = true;
            for (i, slot_radical) in radicals.iter().enumerate() {
                let c = if Some(i) == ambiguous {
                    Some(fill)
                } else {
                    *slot_radical
                };
                match c {
                    Some(ch) => rad.push(ch),
                    None => {
                        complete = false;
                        break;
                    }
                }
            }
            if complete && rad.len() == 3 {
                out.push(Root { radicals: rad });
            }
        }
        out
    }
}

/// Long vowels and hamza-carriers cannot be strong radicals; capturing one
/// means the stem has a weak root we defer to a later stage.
fn is_weak_radical(c: char) -> bool {
    matches!(
        c,
        'ا' | 'و' | 'ي' | 'ى' | 'ء' | 'أ' | 'إ' | 'آ' | 'ؤ' | 'ئ' | 'ة'
    )
}

/// The static, deterministically-ordered pattern table.
#[derive(Debug, Clone)]
pub struct PatternTable {
    patterns: Vec<Pattern>,
}

impl Default for PatternTable {
    fn default() -> Self {
        Self::builtin()
    }
}

impl PatternTable {
    /// The built-in v0.1 strong-root pattern set. Ordering is fixed (by id).
    pub fn builtin() -> Self {
        // (id, template, POS, prior). Priors are deterministic commonness
        // placeholders until lexicon-based validation.
        let specs: &[(u32, &str, Pos, f32)] = &[
            (1, "فعل", Pos::Verb, 0.40),      // bare triliteral (ambiguous)
            (2, "فاعل", Pos::Noun, 0.60),     // active participle / agent
            (3, "مفعول", Pos::Noun, 0.60),    // passive participle
            (4, "فعال", Pos::Noun, 0.55),     // verbal noun (kitāb)
            (5, "فعول", Pos::Noun, 0.45),     // fuʿūl
            (6, "فعيل", Pos::Adj, 0.55),      // adjective (jamīl)
            (7, "مفعل", Pos::Noun, 0.50),     // place / instrument (maktab)
            (8, "مفعلة", Pos::Noun, 0.50),    // place fem (maktaba)
            (9, "مفاعل", Pos::Noun, 0.45),    // broken plural (makātib)
            (10, "افتعال", Pos::Noun, 0.45),  // Form VIII verbal noun
            (11, "استفعال", Pos::Noun, 0.45), // Form X verbal noun
        ];
        let mut patterns: Vec<Pattern> = specs
            .iter()
            .map(|&(id, t, pos, prior)| Pattern::parse(id, t, pos, prior))
            .collect();

        // Weak Form-I patterns (built from explicit slots, since their surface
        // realizations replace radical positions and can't be written in plain
        // ف/ع/ل notation). Weak analyses get a slightly lower prior — they are
        // more speculative — but lexicon confirmation promotes the real ones.
        use WeakResolution::{Waw, WawOrYa, Ya};
        patterns.push(Pattern::weak(
            20,
            "فال (hollow)",
            &[
                Slot::Radical(0),
                Slot::Weak {
                    idx: 1,
                    surface: 'ا',
                    resolves: WawOrYa,
                },
                Slot::Radical(2),
            ],
            Pos::Verb,
            0.50,
        )); // قال -> ق-و-ل / ق-ي-ل
        patterns.push(Pattern::weak(
            21,
            "فعا (defective-w)",
            &[
                Slot::Radical(0),
                Slot::Radical(1),
                Slot::Weak {
                    idx: 2,
                    surface: 'ا',
                    resolves: Waw,
                },
            ],
            Pos::Verb,
            0.50,
        )); // دعا -> د-ع-و
        patterns.push(Pattern::weak(
            22,
            "فعى (defective-y)",
            &[
                Slot::Radical(0),
                Slot::Radical(1),
                Slot::Weak {
                    idx: 2,
                    surface: 'ى',
                    resolves: Ya,
                },
            ],
            Pos::Verb,
            0.50,
        )); // رمى -> ر-م-ي
        patterns.push(Pattern::weak(
            23,
            "وفع (mithal-w)",
            &[
                Slot::Weak {
                    idx: 0,
                    surface: 'و',
                    resolves: Waw,
                },
                Slot::Radical(1),
                Slot::Radical(2),
            ],
            Pos::Verb,
            0.45,
        )); // وصل -> و-ص-ل
        patterns.push(Pattern::weak(
            24,
            "يفع (mithal-y)",
            &[
                Slot::Weak {
                    idx: 0,
                    surface: 'ي',
                    resolves: Ya,
                },
                Slot::Radical(1),
                Slot::Radical(2),
            ],
            Pos::Verb,
            0.45,
        )); // يسر -> ي-س-ر

        Self { patterns }
    }

    pub fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }

    /// Match a diacritic-stripped skeleton against every pattern, returning all
    /// strong-root matches. Confidence is the pattern's prior. Results are
    /// sorted by `(confidence DESC, pattern id ASC)`.
    pub fn match_skeleton(&self, skeleton: &str) -> SmallVec<[PatternMatch; 8]> {
        let chars: Vec<char> = skeleton.chars().collect();
        let mut out: SmallVec<[PatternMatch; 8]> = SmallVec::new();
        for p in &self.patterns {
            for root in p.try_match(&chars) {
                out.push(PatternMatch {
                    pattern: p.id,
                    root,
                    confidence: Conf::clamp(p.prior),
                });
            }
        }
        out.sort_by(|a, b| {
            b.confidence
                .cmp(&a.confidence)
                .then_with(|| a.pattern.0.cmp(&b.pattern.0))
        });
        out
    }

    /// Look up a pattern's POS by id (for the analyzer's feature assignment).
    pub fn pos_of(&self, id: PatternId) -> Option<Pos> {
        self.patterns.iter().find(|p| p.id == id).map(|p| p.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_of(s: &str) -> Vec<char> {
        // Helper: best strong-root match for a (clean) skeleton.
        let table = PatternTable::builtin();
        let m = table.match_skeleton(s);
        m.first()
            .map(|m| m.root.radicals.to_vec())
            .unwrap_or_default()
    }

    #[test]
    fn table_parses_without_panic() {
        let t = PatternTable::builtin();
        // 11 strong + 5 weak (hollow, defective-w/y, mithal-w/y).
        assert_eq!(t.patterns().len(), 16);
    }

    #[test]
    fn active_participle_root() {
        // كاتب (فاعل) -> ك ت ب
        assert_eq!(root_of("كاتب"), vec!['ك', 'ت', 'ب']);
    }

    #[test]
    fn passive_participle_root() {
        // مكتوب (مفعول) -> ك ت ب
        assert_eq!(root_of("مكتوب"), vec!['ك', 'ت', 'ب']);
    }

    #[test]
    fn verbal_noun_root() {
        // كتاب (فعال) -> ك ت ب
        assert_eq!(root_of("كتاب"), vec!['ك', 'ت', 'ب']);
    }

    #[test]
    fn place_noun_fem_root() {
        // مدرسة (مفعلة) -> د ر س
        assert_eq!(root_of("مدرسة"), vec!['د', 'ر', 'س']);
    }

    #[test]
    fn form_viii_root() {
        // اكتساب (افتعال) -> ك س ب
        assert_eq!(root_of("اكتساب"), vec!['ك', 'س', 'ب']);
    }

    #[test]
    fn hollow_root_enumerates_waw_and_ya() {
        // قال (hollow): middle weak hides و or ي — enumerate both, never claim ا.
        let table = PatternTable::builtin();
        let m = table.match_skeleton("قال");
        let roots: Vec<Vec<char>> = m.iter().map(|m| m.root.radicals.to_vec()).collect();
        assert!(roots.contains(&vec!['ق', 'و', 'ل']), "should yield ق-و-ل");
        assert!(
            roots.contains(&vec!['ق', 'ي', 'ل']),
            "should also enumerate ق-ي-ل"
        );
        assert!(
            m.iter().all(|m| !m.root.radicals.contains(&'ا')),
            "must never claim ا as a radical"
        );
    }

    #[test]
    fn defective_roots_recover_final_weak() {
        // ا vs ى distinguishes final-waw from final-ya — recoverable.
        assert_eq!(root_of("دعا"), vec!['د', 'ع', 'و']);
        assert_eq!(root_of("رمى"), vec!['ر', 'م', 'ي']);
    }

    #[test]
    fn mithal_root_recovers_initial_waw() {
        assert_eq!(root_of("وصل"), vec!['و', 'ص', 'ل']);
    }

    #[test]
    fn one_stem_can_match_multiple_patterns() {
        // كتب matches فعل; ensure ك-ت-ب is produced.
        let table = PatternTable::builtin();
        let m = table.match_skeleton("كتب");
        assert!(m
            .iter()
            .any(|m| m.root.radicals.as_slice() == ['ك', 'ت', 'ب']));
    }
}
