//! Build-time tool: read open lexicon sources, emit the Qalam lexicon FST artifact.
//!
//! This is intentionally a separate crate from runtime — its dependencies (YAML
//! parsers, FST builders, network code) never reach the runtime. See
//! `DESIGN.md` §5.4 for the build-vs-runtime split rationale, and §5.7 for the
//! list of supported source formats.

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "qalam-lexicon-builder",
    version,
    about = "Build a Qalam lexicon FST from open sources."
)]
struct Args {
    /// Input directory containing source data files.
    #[arg(short, long)]
    input: std::path::PathBuf,

    /// Output path for the built lexicon FST.
    #[arg(short, long)]
    output: std::path::PathBuf,
}

fn main() -> anyhow::Result<()> {
    let _args = Args::parse();
    anyhow::bail!("qalam-lexicon-builder: implemented in a future PR")
}
