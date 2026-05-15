# Qalam — Design Document

**Version:** v0.1.0 (Pre-alpha)
**Status:** Phase 1 in scaffolding
**Last updated:** 2026-05-15

This document is the canonical architectural reference. Code comments and individual crate docs should not duplicate this; they should link to the relevant section here.

---

## Table of contents

1. [Mission](#1-mission)
2. [Non-goals](#2-non-goals)
3. [The compiler analogy, taken seriously](#3-the-compiler-analogy-taken-seriously)
4. [Cross-cutting determinism contract](#4-cross-cutting-determinism-contract)
5. [Phase 1 — Lexical & morphological foundation](#5-phase-1--lexical--morphological-foundation)
6. [Phase plan overview](#6-phase-plan-overview)
7. [Locked decisions log](#7-locked-decisions-log)
8. [Open questions](#8-open-questions)
9. [Glossary](#9-glossary)

---

## 1. Mission

Qalam is a **deterministic Arabic linguistic intelligence platform** built as a layered compiler pipeline:

```
Text -> Unicode IR -> Token IR -> Clitic IR -> Morph IR -> Lex IR
     -> Syntax IR -> Semantic IR -> KG -> Search -> Learning
```

Initial targets: Modern Standard Arabic (MSA) and Egyptian Arabic.

The project's core promise:

> For fixed `(input, lexicon_hash, config_hash)`, the serialized output bytes are identical across runs, operating systems, and CPU architectures — with a full provenance trace of every inference step.

Three secondary goals follow from this:

- **Explainability:** every analysis includes a `Provenance` record naming rules fired and lexicon entries consulted.
- **Arabic-first design:** morphology-aware, root-aware, dialect-aware, semantic-family-aware data structures throughout.
- **Strong systems engineering:** modular crates, layered IRs, property + fuzz + reproducibility tests, criterion benchmarks.

---

## 2. Non-goals

To set expectations honestly:

- **Not a neural Arabic model.** Statistical components, where used, are hash-stamped frozen-weight artifacts — not live inference. Most components are rule-based.
- **Not a translation system.** Egyptian ↔ MSA normalization is a lossy syntactic transform, not translation.
- **Not state-of-the-art accuracy** on every benchmark. We prioritize explainability and reproducibility over raw accuracy. Where we hit accuracy ceilings (e.g. ~70% UAS on undiacritized parsing in strict mode), we document them.
- **Not bundled with restrictive lexicons.** No BAMA, no CALIMA-Star, no LDC-licensed data in the shipping artifact.
- **No neural diacritization in v1.0.** Rule-based diacritization only in Phase 1.5.

---

## 3. The compiler analogy, taken seriously

Most "NLP as compiler" framings are loose metaphors. Qalam takes the analogy literally:

| Compiler concept | Qalam analog | Engineering consequence |
|---|---|---|
| Lexing | Unicode IR + Token IR | Normalization fold-table is normative and hash-stamped |
| HIR -> MIR desugaring | Clitic splitting | One surface form decomposes into ≤4 proclitics + stem + ≤2 enclitics |
| Parsing with ambiguity | Morph IR is an **ambiguity set**, not a single value | All downstream passes operate over forests, not trees |
| Dataflow analysis | Confidence propagation | `Conf` is a lattice with explicit AND/OR rules |
| SSA | Semantic IR (Phase 3) | Each predicate gets a content-addressed stable ID |
| Linker | KG resolution (Phase 4) | Resolves senses across documents to shared entity IDs |
| `-Werror` | Strict mode | Refuses to lower past unresolved ambiguity |

The decisive move is row 3: treating Morph IR as an ambiguity set forces honesty downstream. Collapsing to a single best-guess at this stage embeds a statistical decision into the deterministic core. Keeping the forest until the syntax stage is the architecturally honest call.

---

## 4. Cross-cutting determinism contract

This contract is enforced everywhere. Violation = no merge.

### 4.1 The four invariants

1. **Bitwise reproducibility:** for fixed `(input, lexicon_hash, config_hash)`, serialized output bytes are identical across runs, OSes, and CPU architectures.
2. **Stable ordering:** ambiguity sets are sorted by `(confidence DESC, canonical_id ASC)` where `canonical_id` is content-derived (BLAKE3).
3. **No wall-clock, no unseeded RNG, no `HashMap` iteration in outputs.** Internal hot loops may use `FxHashMap` (data-deterministic) provided the API boundary materializes into a deterministic structure.
4. **Trace-equivalence:** running the same input twice yields identical *traces* (sequence of inference steps), not just identical final answers.

### 4.2 Mechanism inventory

| Concern | Rust mechanism | Rationale |
|---|---|---|
| Map iteration order in outputs | `BTreeMap`, `IndexMap` | `HashMap` iteration is randomized by design |
| Set membership w/ stable order | `BTreeSet`, `IndexSet` | Same |
| Float comparison | `ordered_float::NotNan<f32>` via `qalam_core::Conf` | f32 default `PartialOrd` is non-total on NaN |
| Content hashing | BLAKE3 via `qalam_core::ContentHash` | Fast, deterministic, well-tested |
| Serialization | `serde_json` w/ `BTreeMap` keys; canonical CBOR for binary | Stable JSON is the OSS-friendly debug format |
| Concurrency | Rayon w/ `.collect::<BTreeMap<_,_>>()` | Never `par_iter().collect::<HashMap<_,_>>()` |

### 4.3 Confidence lattice (formal)

`Conf` is `NotNan<f32>` in `[0, 1]`.

```
AND-combination (clitic + stem must both apply):
    conf(n) = ∏ conf(ci)
    Impl: Conf::and(self, other) -> Conf::clamp(self.value() * other.value())

OR-combination (alternatives compete; deterministic noisy-or):
    conf(n) = 1 - ∏(1 - conf(ai))
    Impl: Conf::or(self, other) -> Conf::clamp(1 - (1-a) * (1-b))

Tie-breaking on equal confidence:
    sort by canonical_id ASC
```

These rules are stable across versions. Changing them = semver-major.

---

## 5. Phase 1 — Lexical & morphological foundation

The phase being built right now. Everything else is downstream.

### 5.1 Pipeline

```
UTF-8 input
  -> qalam-text::unicode::normalize    (NFC + Arabic folds)
  -> qalam-text::tokenize              (graphemes + spans)
  -> qalam-text::clitics::split        (proclitic/enclitic FST)
  -> qalam-morph::patterns::match      (templatic patterns)
  -> qalam-morph::roots::extract       (3- or 4-consonant roots)
  -> qalam-morph::lexicon::lookup      (filter against built lexicon)
  -> MorphForest
```

### 5.2 Core type — `MorphAnalysis`

Defined in `qalam-core::morph_ir`. See `qalam-core/src/morph_ir.rs` for the authoritative source. Key design choices:

- **`SmolStr` over `String`:** Arabic tokens are short; inline storage avoids hot-path allocation.
- **`SmallVec<[T; N]>`:** N tuned to 99th percentile (4 proclitics, 2 enclitics, 4 root radicals); allocation only on outliers.
- **`Option<Root>`, `Option<PatternId>`:** foreign words and particles have neither. Forcing them would be a lie.
- **`Provenance` separated from the analysis itself:** keeps analyses readable; trace bloat lives in `Provenance::decisions` only when explicitly requested.
- **`Conf` as `NotNan<f32>` (not `f64`):** f32 is precise enough for [0,1] confidence; f64 doubles memory for no gain.

### 5.3 Determinism guarantees for Phase 1

1. NFC + Arabic fold table is normative — published as `qalam-text/folds.toml`, hash-stamped in the build.
2. Pattern firing order is fixed — patterns sorted by `(specificity DESC, id ASC)`, all firings recorded in trace.
3. OOV backoff is rule-based, not statistical — letter n-gram patterns derived from observed roots.
4. Confidence ties broken by `canonical_id` (content-addressed BLAKE3 of the analysis structure).
5. Trace is itself bit-reproducible.

### 5.4 Crate boundaries

- **`qalam-core`** — IR types, `Conf`, traces, errors. No I/O. Minimal dependencies. The whole API surface.
- **`qalam-text`** — Unicode normalization + tokenization + clitics. Depends on `qalam-core`.
- **`qalam-morph`** — Pattern matching + root extraction + lexicon access. Depends on `qalam-text` and `qalam-core`.
- **`qalam-lexicon-builder`** — Build-time tool: YAML/CSV sources → FST artifact. Heavy build-time deps stay here, away from runtime.
- **`qalam-cli`** — Demo binary (`qalam`).

Boundaries mark **stability and substitutability** boundaries, not module organization. Crates were collapsed from 8 to 5 in v0.2 because most splits were organizational, not substitutability-driven.

### 5.5 Performance targets

- Throughput: ≥50k tokens/sec single-threaded (2024 laptop class).
- Peak RSS on full Arabic Wikipedia: ≤2GB.
- p50 single-token latency: ≤20µs.
- p99 single-token latency: ≤200µs.

### 5.6 Test discipline

- **Property tests** (`proptest`): determinism, NFC idempotency, span coverage, reassembly, confidence bounds, ordering stability.
- **Fuzz tests** (`cargo-fuzz`): random bytes never panic; differential properties.
- **Reproducibility tests**: golden file comparisons, lexicon hash verification.
- **Cross-OS CI gate**: byte-identical analyzer output on Linux + macOS + Windows.

### 5.7 Lexicon sources (Phase 1 bootstrap)

All sources are redistributable; lexicon artifact ships separately under CC-BY-SA 4.0.

| Source | License | What we get | Caveats |
|---|---|---|---|
| UD Arabic-NYUAD 2.13 | CC BY-SA 4.0 | ~600k tokens MSA, parallel ATB | Lemmas -> lexicon. |
| Arabic Wiktionary dumps | CC BY-SA 3.0 | ~150k lemmas with definitions, POS tags | Coverage uneven; needs cleaning. |
| Tatoeba Arabic sentences | CC BY 2.0 | Aligned sentences, EGY-MSA pairs | High quality, small. |
| MADAR-26 open sample | CC BY 4.0 | Egyptian colloquial data | Phase 2 only; open sample only. |
| AraVec word lists | Apache 2.0 | Frequency-sorted lemma lists | Used for OOV backoff. |

UD Arabic-PADT (CC BY-NC-SA 4.0, non-commercial) is used for **test/eval only**, never bundled.

---

## 6. Phase plan overview

| Phase | What | Key deliverable | Status |
|---|---|---|---|
| 1 | Lexical & morphological | `MorphForest` API, Aramorph-class coverage | Scaffolding |
| 1.5 | Rule-based diacritization | Concise/Full trace of diacritization decisions | Planned |
| 2 | EGY↔MSA normalization | `NormalizedForest` with dropped-feature tracing | Designed |
| 3 | Syntax & Semantic IR | Constraint-grammar parser, AMR-style semantic IR | Designed (strict mode only in v1.0) |
| 4 | Knowledge graph | Merkle-hashed KG with deterministic incremental updates | Designed |
| 5 | Semantic search | Morphology-aware retrieval w/ explainable ranking | Designed |
| 6 | Adaptive learning | KG-driven spaced repetition over patterns + roots | Designed |

Phases 2-6 designs live in this document's history (v0.1 design pass) and will be expanded into their own design sections as work begins on each phase.

---

## 7. Locked decisions log

Decisions that should not be re-derived without an explicit `arch-change` proposal.

| ID | Decision | Date | Rationale |
|---|---|---|---|
| P1 | License: Apache-2.0 OR MIT (dual) | 2026-05-15 | Rust ecosystem standard; permissive; OSS-redistributable |
| P2 | Lexicon built from open sources, shipped separately under CC-BY-SA 4.0 | 2026-05-15 | Avoids LDC license restrictions |
| P3 | Strict bit-reproducibility | 2026-05-15 | Core promise; constrains data structures |
| P4 | Phase 1 first | 2026-05-15 | Lowest-risk demonstration of the pattern |
| P5 | Library + CLI + future WASM form factors | 2026-05-15 | Library-first keeps API honest |
| P6 | Diacritization: input-variation only in P1; rule-based in P1.5; no neural in v1.0 | 2026-05-15 | Determinism-honest |
| Q1 | Repo name: `qalam` | 2026-05-15 | Memorable, evocative, available |
| Q3 | Phase 3 single strict mode in v1.0; API leaves room for future assist mode | 2026-05-15 | OSS clarity over accuracy |
| C1 | `MorphAnalysis` schema with `SmolStr`, `NotNan<f32>`, `SmallVec`, `Provenance` | 2026-05-15 | Hot-path allocation discipline + explainability |
| C2 | Lexicon sources: UD-NYUAD, Wiktionary, Tatoeba, MADAR open sample, AraVec | 2026-05-15 | All redistributable; license-compatible |
| C3 | 5 crates in Phase 1 (collapsed from 8) | 2026-05-15 | Boundaries = stability/substitutability, not org |
| C4 | Strict cross-OS CI determinism gate from v0.1 | 2026-05-15 | Cheaper than retroactive |
| C5 | JSONL trace format, Concise/Full levels | 2026-05-15 | Streamable, diff-able, jq-friendly |

---

## 8. Open questions

To be answered as Phase 1 work progresses:

- Should we support 4-consonant roots fully in v0.1? (Default: yes; cost is low, ~5% of vocabulary.)
- WASM target tier: Phase 3 or earlier?
- Should we build a web playground for demo purposes (à la Rust Playground)?
- Coordination with CAMeL Lab or QCRI for evaluation parity?
- Phase 2: consume Phase 1's `MorphForest` directly, or operate on surface text?
- KG storage (Phase 4): SQLite primary; offer in-memory-only mode for ephemeral use?

---

## 9. Glossary

- **MSA** — Modern Standard Arabic
- **EGY** — Egyptian Arabic
- **ATB** — Arabic Treebank (LDC-licensed, restricted)
- **UD** — Universal Dependencies (open)
- **PADT** — Prague Arabic Dependency Treebank (CC BY-NC-SA 4.0; test/eval only)
- **NYUAD** — NYU Abu Dhabi treebank (CC BY-SA 4.0)
- **MADAR** — Multi-Arabic Dialect Applications and Resources corpus
- **Conf** — confidence value, `NotNan<f32>` in `[0, 1]`
- **Forest** — ambiguity set of analyses for the same input
- **Provenance** — record of rules fired + lexicon entries consulted that produced an analysis
- **Trace** — JSONL stream of inference steps; bit-reproducible
- **Clitic** — bound morpheme attaching to a host word (و, ف, ب, ـه, ـها, etc.)
- **Proclitic** — clitic prefix; **Enclitic** — clitic suffix
- **Root** — usually 3-consonant semantic core (ك-ت-ب for "writing")
- **Pattern** — templatic morphological pattern (e.g. فِعال for nominal)
- **Stem** — root + pattern interleaved; the morphological core of a token
