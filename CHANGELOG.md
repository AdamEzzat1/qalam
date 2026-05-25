# Changelog

All notable changes to Qalam are documented here. The format loosely follows
[Keep a Changelog](https://keepachangelog.com/). This project is pre-1.0, so
breaking changes may occur in any minor release; they are called out under
**Changed**.

## [Unreleased]

### Added
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
