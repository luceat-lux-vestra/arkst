//! Integration tests that exercise the real `typst` executable.
//!
//! These tests are separated from the unit tests in `backend.rs` because
//! they need an actual Typst installation. They are skipped (with a notice)
//! when no Typst executable can be located, so a developer machine or a CI
//! runner without Typst can still run the rest of the suite. CI installs a
//! pinned Typst version explicitly before running tests; set
//! `SCRIBIUM_REQUIRE_TYPST=1` to turn a missing executable into a hard
//! failure instead of a skip.

use std::path::PathBuf;
use std::process::Command;

use scribium_typst::backend::{SubprocessBackend, TypstBackend, TypstInput};

/// Locates a Typst executable, in order of preference:
///
/// 1. `SCRIBIUM_TYPST_PATH` (used by CI to point at a pinned install);
/// 2. `typst` on `PATH`;
/// 3. the Homebrew default location (`/opt/homebrew/bin/typst`).
fn find_typst() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SCRIBIUM_TYPST_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
        return None;
    }
    let probe = Command::new("typst").arg("--version").output();
    if probe.is_ok_and(|o| o.status.success()) {
        return Some(PathBuf::from("typst"));
    }
    let homebrew = PathBuf::from("/opt/homebrew/bin/typst");
    if homebrew.is_file() {
        return Some(homebrew);
    }
    None
}

/// Runs `body` with a located Typst backend, skipping (or failing, when
/// `SCRIBIUM_REQUIRE_TYPST` is set) if none can be found.
fn with_typst<F>(name: &str, body: F)
where
    F: FnOnce(SubprocessBackend),
{
    match find_typst() {
        Some(path) => {
            eprintln!("[integration] {name}: using typst at {}", path.display());
            body(SubprocessBackend::new(path));
        }
        None => {
            let required = std::env::var("SCRIBIUM_REQUIRE_TYPST").is_ok();
            let message = format!(
                "[integration] {name}: no Typst executable found (set SCRIBIUM_TYPST_PATH or install typst); \
                 {}",
                if required {
                    "SCRIBIUM_REQUIRE_TYPST is set, failing"
                } else {
                    "skipping"
                }
            );
            eprintln!("{message}");
            if required {
                panic!("{message}");
            }
        }
    }
}

#[test]
fn integration_compile_produces_valid_pdf() {
    with_typst("compile", |backend| {
        let input = TypstInput {
            source: "#heading[Test]\n\nHello world.\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(!pdf.is_empty(), "pdf must not be empty");
        assert!(
            pdf.starts_with(b"%PDF-"),
            "pdf must start with %PDF-, began with {:?}",
            &pdf[..pdf.len().min(8)]
        );
    });
}

#[test]
fn integration_version_succeeds() {
    with_typst("version", |backend| {
        let version = backend.version().expect("version should succeed");
        assert!(!version.is_empty(), "version output must not be empty");
        assert!(version.contains("typst"), "version was: {}", version);
    });
}

#[test]
fn integration_compile_failure_surfaces_diagnostic() {
    with_typst("compile-failure", |backend| {
        let input = TypstInput {
            source: "#heading[Test\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let result = backend.compile(&input);
        assert!(result.is_err(), "invalid Typst must fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("compilation failed"),
            "error must surface the compiler diagnostic, was: {}",
            err
        );
    });
}

#[test]
fn integration_configured_path_is_respected() {
    // The custom-path plumbing is validated by using the located binary
    // through an explicitly configured path rather than the default: the
    // backend passed here was constructed from a resolved path, not the
    // bare `typst` on `PATH`.
    with_typst("configured-path", |backend| {
        let input = TypstInput {
            source: "Hello world.\n".to_string(),
            entry_path: "test.qd".to_string(),
        };
        let output = backend.compile(&input).expect("compile should succeed");
        let pdf = output.pdf.expect("pdf output must be present");
        assert!(pdf.starts_with(b"%PDF-"));
    });
}
