# Qalam

**Arabic Cognitive Infrastructure Engine (ACIE)** — a deterministic Arabic linguistic intelligence platform built in Rust.

> Status: **Pre-alpha (v0.1.0)**. Phase 1 (lexical & morphological foundation) in active development.

## What this is

Qalam is a compiler-style pipeline for Arabic text. It lowers natural language through layered IRs — Unicode normalization → tokens → clitics → morphology → roots → semantics → knowledge graph → search — and treats Arabic NLP as a layered IR problem, not a black box.

The core promise: **same input + same lexicon hash + same config → bit-identical output, across operating systems, with a full provenance trace of every inference step.**

Initial targets: Modern Standard Arabic (MSA) and Egyptian Arabic.

## Why it exists

Existing Arabic NLP infrastructure is either:
- **Statistical** (MADAMIRA, AraBERT, Farasa) — non-deterministic, opaque,
- **License-restricted** (BAMA, CALIMA-Star) — bound by LDC licensing, non-redistributable, or
- **Not actively maintained.**

Qalam is OSS-redistributable (Apache-2.0 + MIT), deterministic, and explainable by design. Every analysis carries a `Provenance` record naming the rules that fired and the lexicon entries consulted.

## Documentation

- [`DESIGN.md`](DESIGN.md) — full architectural design, phase plan, determinism contract
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — contributor guide and determinism rules (mandatory reading for contributors)

## Phase status

| Phase | What | Status |
|---|---|---|
| 1   | Lexical & morphological foundation | In progress — normalize, tokenize, clitics, strong + weak-root morphology, `analyze`, bootstrap lexicon shipped; only full open-source lexicon ingestion pending |
| 1.5 | Rule-based diacritization | Planned |
| 2   | Egyptian ↔ MSA normalization | Designed |
| 3   | Syntax & semantic IR | Designed (strict mode only in v1.0) |
| 4   | Knowledge graph | Designed |
| 5   | Semantic search | Designed |
| 6   | Adaptive learning | Designed |

See [`DESIGN.md`](DESIGN.md) for details.

## Workspace layout

```
qalam-core               IR types, Conf, traces, errors. No I/O.
qalam-text               Unicode normalization, tokenization, clitics.
qalam-morph              Pattern matching, root extraction, lexicon access.
qalam-lexicon-builder    Build-time tool: open sources -> FST artifact.
qalam-cli                Command-line interface (binary: `qalam`).
```

## Building

```sh
cargo build --workspace
cargo test --workspace
```

MSRV: Rust 1.85 (bumped from 1.78 on 2026-05-15 — see `DESIGN.md` §7 entry `MSRV-v0.2`).

## License

Dual-licensed under either of:
- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT License ([`LICENSE-MIT`](LICENSE-MIT))

at your option.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). Note: **all contributions must preserve the determinism contract.** CI enforces byte-identical analyzer output across Linux, macOS, and Windows.
