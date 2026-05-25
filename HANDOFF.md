# Qalam — Handoff & Project Status

> Purpose: read this (plus `DESIGN.md`) to resume work in a fresh session with
> full context. Last updated 2026-05-25, `main` at the Stage 1.4b merge.

## What Qalam is

A **deterministic Arabic morphological analyzer** in Rust, structured as a
compiler pipeline (text → normalize → tokenize → clitics → morphology →
lexicon → ranked analysis forest). Core promise: same input + same lexicon +
same config → **byte-identical output across OS/architecture**, with a
provenance trail. Targets MSA (Egyptian dialect planned). Dual Apache-2.0/MIT,
MSRV 1.85.

- GitHub: https://github.com/AdamEzzat1/qalam
- Local: `C:\Users\adame\Qalam`

## Current state (Phase 1 morphology engine ~95% complete)

All of Phase 1 except the full lexicon is **merged to `main`** (PRs #1–#5):

| Stage | What | 
|---|---|
| 1.1 | Unicode normalization (NFC + Arabic fold table) |
| 1.2 | Raw-anchored tokenizer + `qalam freq` |
| 1.3 | Clitic splitting + `qalam segment` |
| 1.4 | Pattern matching + strong-root extraction; `qalam analyze` became real |
| 1.5 | Bootstrap lexicon (real ranking + provenance) |
| 1.4b | Weak-root morphology (hollow / defective / mithāl) + irregulars |

- **Working CLI** (`qalam`): `normalize`, `freq`, `segment`, `analyze`, `version` (most take `--jsonl`).
- **98 tests** passing; `clippy --all-targets -D warnings` clean; `fmt` clean.
- **Cross-OS determinism gate green** (CI diffs `normalize`/`freq`/`segment`/`analyze` output across Linux + macOS-ARM + Windows; byte-identical).
- **Lexicon**: bootstrap only — **99 roots, 37 particles, 5 irregulars**. Real-text coverage ≈ **25%** of content words (measured: 4 of 17 tokens on a neutral MSA sentence).
- No open PRs; no dangling branches.

## Crate map

- **`qalam-core`** — IR types: `MorphAnalysis`, `MorphForest`, `Conf` (NotNan<f32> lattice), `ByteSpan` (raw-anchored), `Provenance`, `ContentHash` (BLAKE3), `Trace`. No I/O.
- **`qalam-text`** — `unicode` (`normalize`, `strip_tashkil`, `fold_table_hash`; fold table in `data/folds.toml`), `tokenize`, `clitics`, `freq`.
- **`qalam-morph`** — `patterns` (ف/ع/ل measure notation, `Slot` incl. `Weak`, skeleton matcher), `roots` (`analyze_stem`), `analyzer` (`BasicAnalyzer`), `lexicon` (`BootstrapLexicon` + `Lexicon` trait; data in `data/lexicon.toml`).
- **`qalam-lexicon-builder`** — **stub** (the FST builder; built in Stage 1.5b).
- **`qalam-cli`** — the `qalam` binary.

## Key locked decisions (full log in `DESIGN.md` §7)

- **Determinism contract** (`CONTRIBUTING.md`): `BTreeMap`/`IndexMap` not `HashMap` in outputs; `Conf` (NotNan<f32>) for confidences; BLAKE3 `ContentHash`; LF enforced via `.gitattributes`; no wall-clock/unseeded-RNG in outputs.
- **Raw-anchored spans**: every `ByteSpan` indexes the *raw* input (tokenize consumes raw text, normalizes per-token).
- **Forest, not guess**: ambiguity is enumerated + ranked, never silently collapsed.
- **Weak roots** via *enumerate-then-disambiguate*: hollow `قال` yields both ق-و-ل and ق-ي-ل; the lexicon promotes the real one. `قِ`-class lafīf imperatives handled via the `[irregulars]` table (documented lexical exception, **not** a rule).
- **Lexicon behind a trait**: `BootstrapLexicon` now; an FST lexicon swaps in with **no analyzer change**.
- **Confidence priors are deterministic placeholders, not calibrated**: `LEXICON_CONFIRM 0.7`, `UNCONFIRMED_PENALTY 0.4`, `PARTICLE_CONFIRM 0.85`, `UNANALYZED_PENALTY 0.3` (in `qalam-morph/src/analyzer.rs`).

## Build / test / run

```sh
cargo build --workspace
cargo test --workspace                                   # 98 tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run -p qalam-cli -- analyze --input <file>         # also: normalize, freq, segment, version
```

**Windows gotcha:** don't pipe Arabic via PowerShell `|` (it injects a BOM and mangles encoding). Write input to a file (e.g. `printf '...' > f.txt` in git-bash) and use `--input f.txt`, or `--input -` from a clean stdin.

## Workflow conventions

- Feature branch → PR → **squash-merge** after CI is green (build ×3 OS + cross-OS determinism gate). One stage per PR.
- Every PR updates: tests, `CHANGELOG.md`, and `README.md`/`DESIGN.md` if behavior/architecture changed.

## Remaining work (prioritized)

1. **Stage 1.5b — lexicon ingestion pipeline (RECOMMENDED NEXT).** Highest leverage by far: it lifts coverage from ~25% to most words and makes ranking/provenance "real" (retiring placeholder priors). Build `qalam-lexicon-builder`: read a defined input schema (TSV or CoNLL-U of `lemma · root · features`) → emit a **content-hashed, mmap-able FST artifact**; add an FST-backed `Lexicon` impl to `qalam-morph`. **Split of labor:** the *pipeline* is code (can be built without data); the *data* (open sources: UD-NYUAD, Wiktionary, Tatoeba, MADAR, AraVec) needs sourcing/licensing decisions from Adam. A good first sub-task is researching the cleanest license-compatible, fetchable open root/lemma list to seed it.
2. **Accuracy evaluation harness** — there is *no* gold-standard eval yet (a real gap; blocks any accuracy claim). Consider UD Arabic-PADT (eval-only, NC license).
3. Phase 1.5 — rule-based diacritization.
4. Phase 2 — Egyptian↔MSA dialect normalization (designed, unbuilt).
5. Phases 3–6 — syntax & semantic IR, knowledge graph, semantic search, adaptive learning (all designed in `DESIGN.md`, none built).

## Known limitations / deferred

- **Coverage** is the dominant limitation (99-root bootstrap lexicon).
- **No accuracy benchmark.**
- Clitic splitting **degrades on diacritized text** (raw-surface matching is interrupted by diacritics between clitic letters; needs a diacritic-stripped matching view + offset map).
- Weak-root coverage: lafīf and irregulars beyond the 5-entry table aren't handled; mithāl over-generates (lexicon prunes it).
- No diacritization *generation*; no dialect/syntax/KG/search/learning yet.

## Pointers

- `DESIGN.md` — canonical architecture + locked-decision log (§7).
- `CONTRIBUTING.md` — the determinism contract in full.
- `CHANGELOG.md` — per-stage history.
- Blog post (public status writeup): https://adamezzat1.github.io/blog/posts/qalam-v0.1/

## Resume prompt (paste into a new session)

> "Resuming Qalam (deterministic Arabic morphology engine in Rust at
> `C:\Users\adame\Qalam`). Read `HANDOFF.md` and `DESIGN.md` first. Phase 1
> morphology is ~95% done and on `main`. I want to work on **[Stage 1.5b
> lexicon ingestion pipeline / accuracy eval harness / Phase 2 dialect / …]**.
> Keep the feature-branch→PR→squash-merge workflow and the determinism contract."
