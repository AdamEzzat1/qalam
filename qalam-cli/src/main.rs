//! `qalam` — the Qalam command-line interface.
//!
//! Subcommands:
//! - `analyze` — run the morphological analyzer over input text
//! - `normalize` — apply Unicode normalization only
//! - `freq` — word-frequency list grouped by normalized form
//! - `segment` — clitic-splitting forest per Arabic token
//! - `version` — print the version and lexicon hash

use clap::{Parser, Subcommand};
use std::io::{Read, Write};

#[derive(Debug, Parser)]
#[command(
    name = "qalam",
    version,
    about = "Deterministic Arabic linguistic intelligence engine."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run the morphological analyzer over input text.
    Analyze {
        /// Input file path; `-` for stdin.
        #[arg(short, long)]
        input: String,

        /// Emit machine-readable JSONL.
        #[arg(long)]
        jsonl: bool,

        /// Strict mode: fail on unresolved ambiguity.
        #[arg(long)]
        strict: bool,

        /// Reproducibility mode: emit canonical analyses only, no trace.
        #[arg(long)]
        reproducibility_mode: bool,
    },

    /// Apply Unicode normalization to the input.
    Normalize {
        /// Input file path; `-` for stdin.
        #[arg(short, long)]
        input: String,
    },

    /// Word-frequency list grouped by normalized form.
    Freq {
        /// Input file path; `-` for stdin.
        #[arg(short, long)]
        input: String,

        /// Emit machine-readable JSONL (one FreqEntry per line) instead of text.
        #[arg(long)]
        jsonl: bool,
    },

    /// Clitic-splitting forest for each Arabic token.
    Segment {
        /// Input file path; `-` for stdin.
        #[arg(short, long)]
        input: String,

        /// Emit machine-readable JSONL (one token per line) instead of text.
        #[arg(long)]
        jsonl: bool,
    },

    /// Print version information.
    Version,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Analyze {
            input,
            jsonl,
            strict,
            reproducibility_mode,
        } => run_analyze(&input, jsonl, strict, reproducibility_mode),
        Cmd::Normalize { input } => run_normalize(&input),
        Cmd::Freq { input, jsonl } => run_freq(&input, jsonl),
        Cmd::Segment { input, jsonl } => run_segment(&input, jsonl),
        Cmd::Version => {
            println!("qalam {}", env!("CARGO_PKG_VERSION"));
            println!(
                "fold_table_hash: {}",
                qalam_text::unicode::fold_table_hash()
            );
            Ok(())
        }
    }
}

/// Read input from a file path (or stdin if `-`), normalize, write to stdout.
///
/// Writes via `write_all` rather than `print!` to avoid any platform-level
/// line-ending translation. The output bytes match the normalized string
/// exactly on every OS — which is the property the cross-OS CI gate checks.
fn run_normalize(path: &str) -> anyhow::Result<()> {
    let text = read_input(path)?;
    let normalized = qalam_text::unicode::normalize(&text);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(normalized.as_bytes())?;
    Ok(())
}

/// Read input, tokenize, aggregate word frequencies, write the ranked list.
///
/// Output is deterministic: entries are ordered `(count DESC, normalized ASC)`.
/// Lines are written with `\n` on every platform (no CRLF translation), which
/// is what lets the cross-OS CI gate diff this output byte-for-byte.
fn run_freq(path: &str, jsonl: bool) -> anyhow::Result<()> {
    let text = read_input(path)?;
    let tokens = qalam_text::tokenize::tokenize(&text);
    let entries = qalam_text::freq::word_frequencies(&tokens);

    let stdout = std::io::stdout();
    let mut h = stdout.lock();
    if jsonl {
        for e in &entries {
            writeln!(h, "{}", serde_json::to_string(e)?)?;
        }
    } else {
        for e in &entries {
            if e.variants.len() > 1 {
                writeln!(
                    h,
                    "{:>6}  {}  [variants: {}]",
                    e.count,
                    e.normalized,
                    e.variants.join(", ")
                )?;
            } else {
                writeln!(h, "{:>6}  {}", e.count, e.normalized)?;
            }
        }
    }
    Ok(())
}

/// Read input and run the full morphological analyzer, printing a `MorphForest`
/// per non-whitespace token.
///
/// `--jsonl` / `--reproducibility-mode` emit one JSON `MorphForest` per line
/// (the form the cross-OS determinism gate diffs). `--strict` fails if any
/// token has more than one analysis (unresolved ambiguity).
fn run_analyze(path: &str, jsonl: bool, strict: bool, repro: bool) -> anyhow::Result<()> {
    use qalam_morph::Analyzer;
    let text = read_input(path)?;
    let analyzer = qalam_morph::BasicAnalyzer::default();
    let forests = analyzer.analyze_text(&text);

    if strict {
        for f in &forests {
            anyhow::ensure!(
                f.analyses.len() <= 1,
                "strict mode: unresolved ambiguity for {:?} ({} analyses)",
                f.token.as_str(),
                f.analyses.len()
            );
        }
    }

    let stdout = std::io::stdout();
    let mut h = stdout.lock();
    if jsonl || repro {
        for f in &forests {
            writeln!(h, "{}", serde_json::to_string(f)?)?;
        }
    } else {
        for f in &forests {
            let trunc = if f.truncated { " (truncated)" } else { "" };
            writeln!(
                h,
                "{}  [{}..{}]{}",
                f.token, f.span.start, f.span.end, trunc
            )?;
            for an in &f.analyses {
                let root = an
                    .root
                    .as_ref()
                    .map(|r| {
                        r.radicals
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join("-")
                    })
                    .unwrap_or_else(|| "—".to_string());
                let pat = an
                    .pattern
                    .map(|p| p.0.to_string())
                    .unwrap_or_else(|| "—".to_string());
                writeln!(
                    h,
                    "    {:.3}  {}  root={}  pat={}",
                    an.confidence.value(),
                    fmt_analysis(an),
                    root,
                    pat
                )?;
            }
        }
    }
    Ok(())
}

/// Render a MorphAnalysis segmentation as `proc+proc · stem · enc`.
fn fmt_analysis(an: &qalam_core::MorphAnalysis) -> String {
    let mut out = String::new();
    if !an.proclitics.is_empty() {
        let procs: Vec<&str> = an.proclitics.iter().map(|c| c.form.as_str()).collect();
        out.push_str(&procs.join("+"));
        out.push_str(" · ");
    }
    out.push_str(an.stem.surface.as_str());
    if !an.enclitics.is_empty() {
        let encs: Vec<&str> = an.enclitics.iter().map(|c| c.form.as_str()).collect();
        out.push_str(" · ");
        out.push_str(&encs.join("+"));
    }
    out
}

/// Read input, tokenize, and print the clitic-splitting forest for each Arabic
/// token. Non-Arabic tokens are skipped (they carry only the identity split).
///
/// Output is deterministic: tokens in source order, splits already ranked by
/// `(confidence DESC, stem ASC)`. Lines use `\n` on every platform.
fn run_segment(path: &str, jsonl: bool) -> anyhow::Result<()> {
    let text = read_input(path)?;
    let tokens = qalam_text::tokenize::tokenize(&text);

    let stdout = std::io::stdout();
    let mut h = stdout.lock();
    for t in &tokens {
        if t.kind != qalam_text::tokenize::TokenKind::Arabic {
            continue;
        }
        let splits = qalam_text::clitics::split(t);
        if jsonl {
            let line = serde_json::json!({
                "token": t.raw.as_str(),
                "span": [t.span.start, t.span.end],
                "splits": splits.as_slice(),
            });
            writeln!(h, "{}", serde_json::to_string(&line)?)?;
        } else {
            writeln!(h, "{}  [{}..{}]", t.raw, t.span.start, t.span.end)?;
            for s in &splits {
                writeln!(h, "    {:.3}  {}", s.confidence.value(), fmt_split(s))?;
            }
        }
    }
    Ok(())
}

/// Render one clitic split as `proc+proc · stem · enc` for human reading.
fn fmt_split(s: &qalam_text::clitics::CliticSplit) -> String {
    let mut out = String::new();
    if !s.proclitics.is_empty() {
        let procs: Vec<&str> = s.proclitics.iter().map(|c| c.form.as_str()).collect();
        out.push_str(&procs.join("+"));
        out.push_str(" · ");
    }
    out.push_str(s.stem.surface.as_str());
    if !s.enclitics.is_empty() {
        let encs: Vec<&str> = s.enclitics.iter().map(|c| c.form.as_str()).collect();
        out.push_str(" · ");
        out.push_str(&encs.join("+"));
    }
    out
}

fn read_input(path: &str) -> anyhow::Result<String> {
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        Ok(std::fs::read_to_string(path)?)
    }
}
