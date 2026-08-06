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
        /// Input file (.qd, .scrib, .md, .typ)
        input: String,
        /// Output format(s): pdf, html, svg, png, typst
        #[arg(short, long, default_value = "typst")]
        format: Vec<String>,
        /// Output file path (defaults to input with a `.typ` extension)
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Validate input without producing output
    Check {
        /// Input file or directory
        input: String,
    },
    /// Show intermediate representation(s)
    Inspect {
        /// Input file
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
        } => commands::build(&input, &format, output.as_deref()),
        Commands::Check { input } => commands::check(&input),
        Commands::Inspect { input, emit } => commands::inspect(&input, &emit),
    }
}
