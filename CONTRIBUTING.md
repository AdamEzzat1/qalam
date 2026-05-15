# Contributing to Qalam

Thanks for your interest. Qalam has stricter contribution rules than most Rust projects because **determinism is a core promise**, not a nice-to-have. Code that violates the determinism contract will not be merged, regardless of how clever or fast it is.

---

## The determinism contract (mandatory reading)

### What it means

For any code path that produces user-visible output:
> Same input + same lexicon hash + same config hash -> byte-identical output. Across OS, CPU architecture, and time.

This is enforced in CI by running the analyzer on Linux, macOS, and Windows and comparing outputs byte-by-byte. If your PR breaks this gate, it cannot merge.

### What's allowed vs. forbidden at API boundaries

| Category | Use | Avoid |
|---|---|---|
| Maps in outputs | `BTreeMap`, `IndexMap` | `HashMap` (iteration order randomized) |
| Sets in outputs | `BTreeSet`, `IndexSet` | `HashSet` |
| Floats | `ordered_float::NotNan<f32>` (use `qalam_core::Conf` for `[0,1]`) | Bare `f32`/`f64` (NaN breaks total order) |
| Time | none in outputs | `SystemTime::now`, `Instant::now` |
| RNG | seeded with explicit, recorded seed | `thread_rng`, `OsRng` |
| Parallelism | `rayon` collecting to a deterministic structure | `par_iter().collect::<HashMap<_,_>>` |
| Content hashing | BLAKE3 (via `qalam_core::ContentHash`) | Don't depend on `std::hash::Hash` order |

**Internal hot loops may use `FxHashMap`** (FxHash is data-deterministic, not seed-dependent) as long as the API boundary materializes into a deterministic structure (`BTreeMap`, `IndexMap`, or a sorted `Vec`).

### Confidence lattice

`Conf` (in `qalam-core`) is `NotNan<f32>` in `[0, 1]`. Combinations:

- AND-combination (e.g. clitic + stem): `a.and(b) = a * b`
- OR-combination (e.g. multiple analyses): `a.or(b) = 1 - (1-a)(1-b)`  (deterministic noisy-or)
- Tie-breaking on equal confidence: sort by `canonical_id` ascending.

These rules are stable across versions. Changing them is a semver-major event.

### How to check your PR

Before opening a PR:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The CI workflow runs the above on Linux, macOS, and Windows and additionally runs the cross-OS reproducibility gate.

---

## Style

- `rustfmt` (default config) — `cargo fmt --check` passes
- `clippy --all-targets --all-features -- -D warnings` passes
- Public APIs documented with `///` (one-line minimum)
- Tests live alongside the code (`#[cfg(test)] mod tests`) or in a sibling `tests/` directory

## PR process

1. Open an issue first if the change is non-trivial.
2. Branch from `main`.
3. One logical change per PR; rebase to a clean history.
4. CI must pass.
5. Update `CHANGELOG.md` if user-visible behavior changes.
6. Update `DESIGN.md` if architectural decisions change.

## Architectural decisions

If your PR changes a locked decision in `DESIGN.md` §7, open an issue tagged `arch-change` first. Architectural changes require explicit sign-off and a written rationale.

## License

By contributing, you agree your contributions are dual-licensed under Apache-2.0 OR MIT.
