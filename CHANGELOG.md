# Changelog

All notable changes to Qalam are documented here. The format loosely follows
[Keep a Changelog](https://keepachangelog.com/). This project is pre-1.0, so
breaking changes may occur in any minor release; they are called out under
**Changed**.

## [Unreleased]

### Added
- **Weak-root morphology** in `qalam_morph::patterns`: hollow (قال → ق-و-ل /
  ق-ي-ل, enumerated), defective (دعا → د-ع-و, رمى → ر-م-ي), and assimilated/
  mithāl (وصل → و-ص-ل) Form-I patterns via a new `Slot::Weak` + `WeakResolution`.
  `try_match` now returns multiple roots when the surface is ambiguous; the
  lexicon disambiguates (only confirmed candidates are promoted).
- An `[irregulars]` lexicon table (surface → root) for forms not recoverable by
  rule — chiefly hollow/lafīf imperatives. Includes قِ → و-ق-ي (the famous
  one-letter imperative), handled honestly as a lexical exception, not a rule.
- `qalam_morph::lexicon::BootstrapLexicon` — a curated bootstrap lexicon (~65
  strong roots + particles) embedded from `data/lexicon.toml`, hash-stamped;
  implements a `Lexicon` trait so an FST-backed lexicon can replace it later
  without analyzer changes. (The full open-source ingestion is Stage 1.5b.)
- `qalam_text::unicode::strip_tashkil` — diacritic/tatweel stripping for the
  morphological matching skeleton (separate from `normalize`, which preserves
  diacritics).
- `qalam_morph::patterns` — templatic patterns in ف/ع/ل measure notation with a
  skeleton matcher; v0.1 strong-roots-only (rejects matches whose radical is a
  long vowel / hamza-carrier).
- `qalam_morph::roots` — root extraction (normalize + strip-tashkil + pattern
  match) producing (root, pattern, confidence) candidates.
- `qalam_morph::BasicAnalyzer` — composes clitic splitting × pattern/root
  matching into a ranked, top-k-capped `MorphForest`; provenance records fired
  pattern IDs with a documented `qalam:no-lexicon` sentinel `lexicon_hash`.
- **`qalam analyze` is now real** — emits a `MorphForest` per token
  (text / `--jsonl` / `--strict` / `--reproducibility-mode`).
- CI cross-OS determinism gate now also covers `qalam analyze`.

### Changed
- Analyzer now consults the bootstrap lexicon: lexicon-confirmed roots are
  promoted, unconfirmed (likely spurious) pattern matches are down-weighted,
  and recognized particles are tagged `POS=Part`. `Provenance.lexicon_hash` is
  now the real bootstrap-lexicon hash (the `qalam:no-lexicon` sentinel is
  retired); confirmed analyses record their lexicon entry in `lex_entries`.
- `qalam_text::tokenize` — raw-anchored tokenizer that segments text at
  script-class boundaries (Arabic / Latin / Digit / Punct / Whitespace / Other)
  and normalizes each token's surface separately.
- `qalam_text::freq` — word-frequency aggregation grouping tokens by normalized
  form, recording raw variants and first position; deterministic ranking.
- `qalam_text::clitics` — proclitic/enclitic segmentation producing a *forest*
  of candidate splits (always incl. the identity "no split"); raw-anchored
  clitic spans; deterministic ranking with the identity ranked highest pending
  lexicon-based re-ranking. Over-generates by design.
- `qalam freq` and `qalam segment` CLI subcommands (human text and `--jsonl`).
- `qalam-text/benches/text.rs` — criterion benchmarks for normalize / tokenize /
  freq / clitics, measured against the throughput targets in DESIGN.md §5.5.
- `qalam-text/tests/reproducibility.rs` — determinism + raw-span-coverage tests
  over the golden corpus.
- CI cross-OS determinism gate now covers `normalize`, `freq`, and `segment`.

### Changed
- **Span contract (breaking, pre-1.0):** every `ByteSpan` now anchors to the
  **raw** input bytes, never the normalized form. `Token` carries both `raw`
  and `normalized` surfaces plus a raw `span`. `tokenize` consumes raw text and
  normalizes per token, so downstream consumers can always slice the original
  document. See DESIGN.md and `ByteSpan` docs.

## [0.1.0] - 2026-05-15

### Added
- Cargo workspace scaffold: `qalam-core`, `qalam-text`, `qalam-morph`,
  `qalam-lexicon-builder`, `qalam-cli`.
- `qalam_core` IR types: `MorphAnalysis`, `MorphForest`, `Provenance`, `Conf`
  (identity-preserving AND/OR lattice), `ContentHash`, `Trace`.
- `qalam_text::unicode::normalize` — NFC + Arabic normative fold table
  (Phase 1 Stage 1.1); hash-stamped via `fold_table_hash`.
- `qalam normalize` and `qalam version` CLI subcommands.
- Cross-OS determinism CI gate (Linux + macOS + Windows, byte-equality).
- Dual Apache-2.0 / MIT license; determinism contract in CONTRIBUTING.md.

### Changed
- MSRV set to 1.85 (edition2024 stabilization required by transitive deps).
