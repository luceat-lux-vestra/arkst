//! In-process Typst compilation spike
//!
//! This crate investigates the feasibility of using the Typst crate
//! as an in-process backend for Scribium.
//!
//! It is an investigation spike, NOT production code.
//! It is excluded from the main workspace.
//!
//! ## Purpose
//! - Test if typst::compile can be called in-process
//! - Measure compilation time and binary size
//! - Test WASM target compatibility
//! - Explore World trait implementation complexity
//!
//! ## Used Typst Version
//! - typst 0.15.1
//!
//! ## Run
//! ```bash
//! cargo +1.92.0 run --manifest-path tools/spikes/typst-in-process/Cargo.toml
//! ```

use typst::compile;
use typst::foundations::Datetime;
use typst::foundations::Duration;
use typst::Library;
use typst::LibraryExt;
use typst::text::Font;
use typst::text::FontBook;
use typst::utils::LazyHash;
use typst::syntax::FileId;
use typst::diag::FileError;
use typst::syntax::Source;
use typst::foundations::Bytes;
use typst::World;
use typst_layout::PagedDocument;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant;

struct TestWorld {
    library: LazyHash<Library>,
    font_book: LazyHash<FontBook>,
    main_file_id: FileId,
    source_cache: HashMap<FileId, String>,
    file_cache: HashMap<FileId, Arc<[u8]>>,
    font_cache: HashMap<usize, Font>,
}

impl TestWorld {
    fn new(main_source: &str) -> Self {
        let library = LazyHash::new(Library::default());
        let font_book = LazyHash::new(FontBook::default());
        let main_file_id = FileId::unique(
            typst::syntax::RootedPath::new(
                typst::syntax::VirtualRoot::Project,
                typst::syntax::VirtualPath::new("main.typ").expect("valid path")
            )
        );

        let mut source_cache = HashMap::new();
        source_cache.insert(main_file_id, main_source.to_string());

        Self {
            library: LazyHash::new(Library::default()),
            font_book: LazyHash::new(FontBook::default()),
            main_file_id,
            source_cache,
            file_cache: HashMap::new(),
            font_cache: HashMap::new(),
        }
    }
}

impl World for TestWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_book
    }

    fn main(&self) -> FileId {
        self.main_file_id
    }

    fn source(&self, id: FileId) -> Result<Source, FileError> {
        if let Some(source) = self.source_cache.get(&id) {
            Ok(Source::detached(source.clone()))
        } else {
            Err(FileError::from_io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"), std::path::Path::new("")))
        }
    }

    fn file(&self, id: FileId) -> Result<Bytes, FileError> {
        if let Some(bytes) = self.file_cache.get(&id) {
            Ok(Bytes::new(Arc::clone(bytes)))
        } else if id == self.main_file_id {
            if let Some(source) = self.source_cache.get(&id) {
                let bytes = Arc::from(source.as_bytes());
                Ok(Bytes::new(bytes))
            } else {
                Err(FileError::from_io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"), std::path::Path::new("")))
            }
        } else {
            Err(FileError::from_io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"), std::path::Path::new("")))
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.font_cache.get(&index).cloned()
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        if let Some(offset) = offset {
            Datetime::from_ymd_hms(2024, 1, 1, 0, 0, 0).map(|dt| dt + offset)
        } else {
            Datetime::from_ymd_hms(2024, 1, 1, 0, 0, 0)
        }
    }
}

fn main() {
    println!("=== Typst In-Process Compilation Spike ===");
    println!("Testing Typst 0.15.1 in-process compilation");
    println!();

    // Test Case 1: Simple rect (no fonts)
    println!("=== Test Case 1: Simple rect (no fonts) ===");
    let simple_source = r#"
#set page(width: 100pt, height: 100pt)
#rect(width: 20pt, height: 20pt)
"#;

    let start = Instant::now();
    let world = TestWorld::new(simple_source);
    let result = compile::<PagedDocument>(&world);
    let elapsed = start.elapsed();

    match result.output {
        Ok(doc) => {
            println!("✅ Success!");
            println!("  Pages: {}", doc.pages().len());
            println!("  Time: {:.2?}", elapsed);
        }
        Err(errors) => {
            println!("❌ Failed with {} errors:", errors.len());
            for err in &errors {
                println!("  - {}", err.message);
            }
        }
    }

    println!();

    // Test Case 2: Text with font
    println!("=== Test Case 2: Text with font ===");
    let text_source = r#"
#set page(width: 200pt, height: 100pt)
Hello from Scribium.
"#;

    let start = Instant::now();
    let world = TestWorld::new(text_source);
    let result = compile::<PagedDocument>(&world);
    let elapsed = start.elapsed();

    match result.output {
        Ok(doc) => {
            println!("✅ Success!");
            println!("  Pages: {}", doc.pages().len());
            println!("  Time: {:.2?}", elapsed);
        }
        Err(errors) => {
            println!("❌ Failed with {} errors:", errors.len());
            for err in &errors {
                println!("  - {}", err.message);
            }
        }
    }

    println!();

    // Test Case 3: Error fixture
    println!("=== Test Case 3: Invalid function call ===");
    let error_source = r#"
#unknown-function()
"#;

    let start = Instant::now();
    let world = TestWorld::new(error_source);
    let result = compile::<PagedDocument>(&world);
    let elapsed = start.elapsed();

    match result.output {
        Ok(doc) => {
            println!("⚠️ Unexpected success!");
            println!("  Pages: {}", doc.pages().len());
        }
        Err(errors) => {
            println!("✅ Expected failure with {} errors:", errors.len());
            for err in &errors {
                println!("  - {}", err.message);
            }
            println!("  Time: {:.2?}", elapsed);
        }
    }

    println!();
    println!("=== Spike completed ===");
}