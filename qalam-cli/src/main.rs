//! `qalam` — the Qalam command-line interface.
//!
//! Subcommands:
//! - `analyze` — run the morphological analyzer over input text
//! - `normalize` — apply Unicode normalization only
//! - `freq` — word-frequency list grouped by normalized form
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

    /// Print version information.
    Version,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Analyze { .. } => anyhow::bail!("analyze: implemented in a future PR"),
        Cmd::Normalize { input } => run_normalize(&input),
        Cmd::Freq { input, jsonl } => run_freq(&input, jsonl),
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

fn read_input(path: &str) -> anyhow::Result<String> {
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        Ok(std::fs::read_to_string(path)?)
    }
}
