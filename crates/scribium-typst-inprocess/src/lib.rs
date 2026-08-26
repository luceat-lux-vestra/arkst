//! In-process Typst backend spike for issue #187.
//!
//! This adapter intentionally sits after the scribium-typst lowering. It maps a
//! completed VirtualProject to Typst's public World contract, compiles the
//! generated Typst source, and exports a PDF. Typst types stay inside this
//! native adapter; Scribium semantic IR and the platform-neutral lowering
//! crate never depend on them.

use scribium_diagnostics::{Diagnostic, Severity};
use scribium_project::{VirtualPathBuf, VirtualProject};
use scribium_typst::{TypstBackend, TypstInput, TypstOutput};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use typst::diag::{FileError, FileResult, PackageError, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::World;
use typst::{Library, LibraryExt};
use typst_layout::PagedDocument;
use typst_pdf::PdfOptions;

/// A native in-process adapter bound to one completed virtual project.
///
/// The project is the only resource authority. This adapter does not inspect
/// the host filesystem, discover system fonts, consult the process clock, or
/// resolve packages over the network.
pub struct InProcessBackend<'a> {
    project: &'a VirtualProject,
}

impl<'a> InProcessBackend<'a> {
    /// Creates an adapter over an existing in-memory project.
    pub fn new(project: &'a VirtualProject) -> Self {
        Self { project }
    }

    /// Returns the project used as the resource authority.
    pub fn project(&self) -> &'a VirtualProject {
        self.project
    }
}

impl TypstBackend for InProcessBackend<'_> {
    type Error = InProcessError;

    fn compile(&self, input: &TypstInput) -> Result<TypstOutput, Self::Error> {
        let start = Instant::now();
        let world = ProjectWorld::new(self.project, input)?;
        let compile_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            typst::compile::<PagedDocument>(&world)
        }))
        .map_err(panic_error)?;

        let warnings = compile_result
            .warnings
            .iter()
            .map(|diagnostic| render_diagnostic(&world, diagnostic))
            .collect::<Vec<_>>();

        let document = compile_result.output.map_err(|diagnostics| {
            InProcessError::Compilation(
                diagnostics
                    .iter()
                    .map(|diagnostic| to_scribium_diagnostic(&world, diagnostic))
                    .collect(),
            )
        })?;

        let pdf_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            typst_pdf::pdf(&document, &PdfOptions::default())
        }))
        .map_err(panic_error)?;
        let pdf = pdf_result.map_err(|diagnostics| {
            InProcessError::PdfExport(
                diagnostics
                    .iter()
                    .map(|diagnostic| to_scribium_diagnostic(&world, diagnostic))
                    .collect(),
            )
        })?;

        if !pdf.starts_with(b"%PDF-") {
            return Err(InProcessError::InvalidOutput(
                "Typst PDF exporter returned data without a %PDF- header".to_string(),
            ));
        }

        Ok(TypstOutput {
            pdf: Some(pdf),
            html: None,
            svg: None,
            png: None,
            diagnostics: warnings,
            duration: start.elapsed(),
        })
    }

    fn version(&self) -> Result<String, Self::Error> {
        Ok(format!("typst {}", typst::utils::version().raw()))
    }
}

/// A stable, adapter-owned error. Typst diagnostics are converted before they
/// cross this boundary; no Typst compiler type is exposed by the error API.
#[derive(Debug)]
pub enum InProcessError {
    InvalidInput(String),
    Compilation(Vec<Diagnostic>),
    PdfExport(Vec<Diagnostic>),
    InvalidOutput(String),
    CompilerPanic(String),
}

impl InProcessError {
    /// Returns converted structured diagnostics, when the failure came from
    /// Typst compilation or PDF export.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Compilation(diagnostics) | Self::PdfExport(diagnostics) => diagnostics,
            Self::InvalidInput(_) | Self::InvalidOutput(_) | Self::CompilerPanic(_) => &[],
        }
    }
}

impl fmt::Display for InProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(formatter, "invalid in-process Typst input: {message}")
            }
            Self::Compilation(diagnostics) => {
                write_diagnostics(formatter, "Typst compilation failed", diagnostics)
            }
            Self::PdfExport(diagnostics) => {
                write_diagnostics(formatter, "Typst PDF export failed", diagnostics)
            }
            Self::InvalidOutput(message) => {
                write!(formatter, "invalid Typst PDF output: {message}")
            }
            Self::CompilerPanic(message) => {
                write!(formatter, "Typst compiler panicked: {message}")
            }
        }
    }
}

impl std::error::Error for InProcessError {}

/// The in-memory implementation of Typst's public compiler environment.
///
/// VirtualProject remains the source of truth. The maps are immutable for a
/// compile, which gives repeated source/resource loads stable identity and
/// deterministic results without a global cache.
struct ProjectWorld<'a> {
    project: &'a VirtualProject,
    library: LazyHash<Library>,
    font_book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main_id: FileId,
    main_path: VirtualPathBuf,
    main_source: Arc<str>,
    sources: BTreeMap<String, Source>,
    files: BTreeMap<String, Bytes>,
    today: Datetime,
}

impl<'a> ProjectWorld<'a> {
    fn new(project: &'a VirtualProject, input: &TypstInput) -> Result<Self, InProcessError> {
        let requested_entry = parse_document_path(&input.entry_path)?;
        if requested_entry != *project.entry() {
            return Err(InProcessError::InvalidInput(format!(
                "entry path '{}' does not match VirtualProject entry '{}'",
                requested_entry.as_str(),
                project.entry().as_str()
            )));
        }

        let main_path = generated_typst_path(project, &requested_entry)?;
        let main_id = file_id(&main_path)?;
        let main_source: Arc<str> = Arc::from(input.source.as_str());

        let mut sources = BTreeMap::new();
        let mut files = BTreeMap::new();
        sources.insert(
            main_path.as_str().to_string(),
            Source::new(main_id, input.source.clone()),
        );

        for (_, path, source) in project.sources().iter() {
            let path = path.as_str().to_string();
            let bytes = Bytes::from_string(source.to_string());
            files.insert(path.clone(), bytes);
            if path.ends_with(".typ") && path != main_path.as_str() {
                let id = file_id_from_str(&path)?;
                sources.insert(path, Source::new(id, source.to_string()));
            }
        }

        for (path, data) in project.assets().iter() {
            files
                .entry(path.as_str().to_string())
                .or_insert_with(|| Bytes::new(data.to_vec()));
        }

        let fonts = embedded_fonts()
            .into_iter()
            .chain(project_fonts(project))
            .collect::<Vec<_>>();
        let font_book = LazyHash::new(FontBook::from_fonts(fonts.iter()));

        // The spike deliberately does not read the host clock. A future host
        // capability can replace this with an explicit project option.
        let today = Datetime::from_ymd(1970, 1, 1).ok_or_else(|| {
            InProcessError::InvalidInput("fixed deterministic date is invalid".to_string())
        })?;

        Ok(Self {
            project,
            library: LazyHash::new(Library::default()),
            font_book,
            fonts,
            main_id,
            main_path,
            main_source,
            sources,
            files,
            today,
        })
    }

    fn file_error(&self, id: FileId) -> FileError {
        match id.root() {
            VirtualRoot::Project => FileError::NotFound(PathBuf::from(format!(
                "/{}",
                id.vpath().get_without_slash()
            ))),
            VirtualRoot::Package(spec) => FileError::Package(PackageError::NotFound(spec.clone())),
        }
    }

    fn project_path(&self, id: FileId) -> Option<String> {
        if id.root() == &VirtualRoot::Project {
            Some(id.vpath().get_without_slash().to_string())
        } else {
            None
        }
    }
}

impl World for ProjectWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            return self
                .sources
                .get(self.main_path.as_str())
                .cloned()
                .ok_or_else(|| self.file_error(id));
        }

        let Some(path) = self.project_path(id) else {
            return Err(self.file_error(id));
        };
        self.sources
            .get(&path)
            .cloned()
            .ok_or_else(|| self.file_error(id))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main_id {
            return Ok(Bytes::from_string(self.main_source.to_string()));
        }

        let Some(path) = self.project_path(id) else {
            return Err(self.file_error(id));
        };
        self.files
            .get(&path)
            .cloned()
            .ok_or_else(|| self.file_error(id))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        offset.map_or(Some(self.today), |offset| Some(self.today + offset))
    }
}

fn parse_document_path(raw: &str) -> Result<VirtualPathBuf, InProcessError> {
    let path = VirtualPathBuf::parse(raw)
        .map_err(|error| InProcessError::InvalidInput(format!("invalid entry path: {error}")))?;
    if path.is_root() {
        return Err(InProcessError::InvalidInput(
            "entry path must name a document".to_string(),
        ));
    }
    Ok(path)
}

fn generated_typst_path(
    project: &VirtualProject,
    entry: &VirtualPathBuf,
) -> Result<VirtualPathBuf, InProcessError> {
    let file_name = entry
        .file_name()
        .ok_or_else(|| InProcessError::InvalidInput("entry path has no file name".to_string()))?;
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let parent = entry.parent().unwrap_or_else(VirtualPathBuf::root);
    let preferred = parent
        .join(format!("{stem}.typ"))
        .map_err(|error| InProcessError::InvalidInput(error.to_string()))?;
    if !project.sources().contains(&preferred) && !project.assets().contains(&preferred) {
        return Ok(preferred);
    }

    let mut suffix = 0_u64;
    loop {
        let name = if suffix == 0 {
            format!("{stem}.scribium.typ")
        } else {
            format!("{stem}.scribium-{suffix}.typ")
        };
        let candidate = parent
            .join(name)
            .map_err(|error| InProcessError::InvalidInput(error.to_string()))?;
        if !project.sources().contains(&candidate) && !project.assets().contains(&candidate) {
            return Ok(candidate);
        }
        suffix = suffix.checked_add(1).ok_or_else(|| {
            InProcessError::InvalidInput("could not select generated Typst path".to_string())
        })?;
    }
}

fn file_id(path: &VirtualPathBuf) -> Result<FileId, InProcessError> {
    file_id_from_str(path.as_str())
}

fn file_id_from_str(path: &str) -> Result<FileId, InProcessError> {
    let path = VirtualPath::new(path).map_err(|error| {
        InProcessError::InvalidInput(format!("invalid Typst virtual path: {error}"))
    })?;
    Ok(RootedPath::new(VirtualRoot::Project, path).intern())
}

fn embedded_fonts() -> Vec<Font> {
    typst_assets::fonts()
        .flat_map(|data| Font::iter(Bytes::new(data)))
        .collect()
}

fn project_fonts(project: &VirtualProject) -> Vec<Font> {
    project
        .assets()
        .iter()
        .filter(|(path, _)| is_font_path(path.as_str()))
        .flat_map(|(_, data)| Font::iter(Bytes::new(data.to_vec())))
        .collect()
}

fn is_font_path(path: &str) -> bool {
    matches!(
        path.rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase()),
        Some(extension) if matches!(extension.as_str(), "otf" | "ttf" | "otc" | "ttc")
    )
}

fn to_scribium_diagnostic(world: &ProjectWorld<'_>, diagnostic: &SourceDiagnostic) -> Diagnostic {
    let (location, range) = diagnostic_location(world, diagnostic);
    let message = match (&location, &range) {
        (Some(path), Some(range)) => {
            format!(
                "{path}:{}..{}: {}",
                range.start, range.end, diagnostic.message
            )
        }
        (Some(path), None) => format!("{path}: {}", diagnostic.message),
        (None, _) => diagnostic.message.to_string(),
    };

    let primary = match (diagnostic.span.id(), range) {
        (Some(id), Some(range)) if id.root() == &VirtualRoot::Project => {
            let path = VirtualPathBuf::parse(id.vpath().get_without_slash()).ok();
            path.and_then(|path| world.project.sources().get_id(&path))
                .map(|source_id| {
                    scribium_source::SourceSpan::new(source_id, range.start, range.end)
                })
        }
        _ => None,
    };

    Diagnostic {
        code: "E5001".to_string(),
        severity: match diagnostic.severity {
            typst::diag::Severity::Error => Severity::Error,
            typst::diag::Severity::Warning => Severity::Warning,
        },
        message,
        primary,
        secondary: vec![],
        hints: diagnostic
            .hints
            .iter()
            .map(|hint| hint.v.to_string())
            .collect(),
    }
}

fn diagnostic_location(
    world: &ProjectWorld<'_>,
    diagnostic: &SourceDiagnostic,
) -> (Option<String>, Option<std::ops::Range<usize>>) {
    let Some(id) = diagnostic.span.id() else {
        return (None, None);
    };
    let location = match id.root() {
        VirtualRoot::Project => format!("/{}", id.vpath().get_without_slash()),
        VirtualRoot::Package(spec) => format!("{spec}{}", id.vpath().get_with_slash()),
    };
    let range = typst::WorldExt::range(world, diagnostic.span);
    (Some(location), range)
}

fn render_diagnostic(world: &ProjectWorld<'_>, diagnostic: &SourceDiagnostic) -> String {
    to_scribium_diagnostic(world, diagnostic).to_string()
}

fn write_diagnostics(
    formatter: &mut fmt::Formatter<'_>,
    prefix: &str,
    diagnostics: &[Diagnostic],
) -> fmt::Result {
    write!(formatter, "{prefix}")?;
    for diagnostic in diagnostics {
        write!(formatter, "\n{diagnostic}")?;
    }
    Ok(())
}

fn panic_error(payload: Box<dyn std::any::Any + Send>) -> InProcessError {
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic payload")
        .to_string();
    InProcessError::CompilerPanic(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_entry_avoids_typst_source_collision() {
        let project = scribium_project::VirtualProjectBuilder::new()
            .entry("docs/main.qd")
            .expect("entry")
            .add_source("docs/main.qd", "content")
            .expect("source")
            .add_source("docs/main.typ", "#let helper = 1")
            .expect("typst source")
            .build()
            .expect("project");

        let path = generated_typst_path(
            &project,
            &VirtualPathBuf::parse("docs/main.qd").expect("path"),
        )
        .expect("generated path");
        assert_eq!(path.as_str(), "docs/main.scribium.typ");
    }

    #[test]
    fn font_paths_are_selected_case_insensitively() {
        assert!(is_font_path("fonts/example.OTF"));
        assert!(is_font_path("fonts/example.ttc"));
        assert!(!is_font_path("images/example.png"));
    }
}
