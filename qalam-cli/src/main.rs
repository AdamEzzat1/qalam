//! `qalam` — the Qalam command-line interface.
//!
//! Subcommands:
//! - `analyze` — run the morphological analyzer over input text
//! - `normalize` — apply Unicode normalization only
//! - `version` — print the version and lexicon hash

use clap::{Parser, Subcommand};

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

    /// Print version information.
    Version,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Analyze { .. } => anyhow::bail!("analyze: implemented in next PR"),
        Cmd::Normalize { .. } => anyhow::bail!("normalize: implemented in next PR"),
        Cmd::Version => {
            println!("qalam {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
