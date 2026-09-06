/// `arkst-cli` — Arkst command-line interface.
///
/// Commands:
/// - `arkst build <input>` — compile to output format(s)
/// - `arkst check <input>` — validate without producing output
/// - `arkst inspect <input>` — show intermediate representations
/// - `arkst --version` — print version
/// - `arkst --help` — print help
mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "arkst",
    version,
    about = "Arkst — Quarkdown-compatible compiler and toolchain"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile input document(s) to output format(s)
    Build {
        /// Input file (.qd, .arkst, .md)
        input: String,
        /// Explicit loadable-library directory (`-l` / `--libs`); direct lowercase `.qd` files only
        #[arg(short = 'l', long = "libs", value_name = "DIR")]
        libs: Option<PathBuf>,
        /// Output format(s): typst, pdf (html, svg, png are not yet implemented)
        #[arg(short, long, default_value = "typst")]
        format: Vec<String>,
        /// Output file path (defaults to .typ for typst and .pdf for pdf)
        #[arg(long)]
        output: Option<PathBuf>,
        /// Native PDF backend: subprocess (default) or in-process (explicit native-only opt-in; requires Cargo feature `typst-inprocess`; not browser/WASM rendering)
        #[arg(long, value_enum, default_value = "subprocess")]
        backend: commands::BackendSelection,
        /// Path to the Typst executable used by the subprocess PDF backend (defaults to `typst` on PATH)
        #[arg(long, default_value = "typst")]
        typst_path: PathBuf,
    },
    /// Validate input without producing output
    Check {
        /// Input Arkst or Markdown file
        input: String,
        /// Explicit loadable-library directory (`-l` / `--libs`); direct lowercase `.qd` files only
        #[arg(short = 'l', long = "libs", value_name = "DIR")]
        libs: Option<PathBuf>,
    },
    /// Show intermediate representation(s)
    Inspect {
        /// Input Arkst or Markdown file
        input: String,
        /// Explicit loadable-library directory (`-l` / `--libs`); direct lowercase `.qd` files only
        #[arg(short = 'l', long = "libs", value_name = "DIR")]
        libs: Option<PathBuf>,
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
            libs,
            format,
            output,
            backend,
            typst_path,
        } => commands::build_with_backend_and_libraries(
            &input,
            &format,
            output.as_deref(),
            &typst_path,
            backend,
            libs.as_deref(),
        ),
        Commands::Check { input, libs } => commands::check_with_libraries(&input, libs.as_deref()),
        Commands::Inspect { input, libs, emit } => {
            commands::inspect_with_libraries(&input, &emit, libs.as_deref())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_defaults_to_subprocess() {
        let cli = Cli::try_parse_from(["arkst", "build", "document.qd"]).expect("parse");
        let Commands::Build { backend, .. } = cli.command else {
            panic!("expected build command");
        };
        assert_eq!(backend, commands::BackendSelection::Subprocess);
    }

    #[test]
    fn backend_accepts_explicit_values() {
        for (value, expected) in [
            ("subprocess", commands::BackendSelection::Subprocess),
            ("in-process", commands::BackendSelection::InProcess),
        ] {
            let cli = Cli::try_parse_from(["arkst", "build", "document.qd", "--backend", value])
                .expect("parse");
            let Commands::Build { backend, .. } = cli.command else {
                panic!("expected build command");
            };
            assert_eq!(backend, expected);
        }
    }

    #[test]
    fn backend_rejects_unknown_values() {
        let error =
            match Cli::try_parse_from(["arkst", "build", "document.qd", "--backend", "unknown"]) {
                Ok(_) => panic!("unknown backend must be rejected"),
                Err(error) => error,
            };
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn help_describes_backend_choices_and_default() {
        use clap::CommandFactory;

        let mut command = Cli::command();
        let help = command
            .find_subcommand_mut("build")
            .expect("build subcommand")
            .render_long_help()
            .to_string();
        assert!(help.contains("--backend"));
        assert!(help.contains("subprocess"));
        assert!(help.contains("in-process"));
        assert!(help.contains("default"));
        assert!(help.contains("typst-inprocess"));
    }
}
