/// `scribium-cli` — Scribium command-line interface.
///
/// Commands:
/// - `scribium build <input>` — compile to output format(s)
/// - `scribium check <input>` — validate without producing output
/// - `scribium inspect <input>` — show intermediate representations
/// - `scribium --version` — print version
/// - `scribium --help` — print help
mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "scribium",
    version,
    about = "Scribium — Quarkdown-compatible compiler and toolchain"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile input document(s) to output format(s)
    Build {
        /// Input file (.qd, .scrib, .md)
        input: String,
        /// Output format(s): typst, pdf (html, svg, png are not yet implemented)
        #[arg(short, long, default_value = "typst")]
        format: Vec<String>,
        /// Output file path (defaults to .typ for typst and .pdf for pdf)
        #[arg(long)]
        output: Option<PathBuf>,
        /// Path to the Typst executable used for PDF output (defaults to `typst` on PATH)
        #[arg(long, default_value = "typst")]
        typst_path: PathBuf,
    },
    /// Validate input without producing output
    Check {
        /// Input Scribium or Markdown file
        input: String,
    },
    /// Show intermediate representation(s)
    Inspect {
        /// Input Scribium or Markdown file
        input: String,
        /// What to emit: ast, semantic, ir, typst, source-map
        #[arg(long, default_value = "typst")]
        emit: String,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            input,
            format,
            output,
            typst_path,
        } => commands::build(&input, &format, output.as_deref(), &typst_path),
        Commands::Check { input } => commands::check(&input),
        Commands::Inspect { input, emit } => commands::inspect(&input, &emit),
    }
}
