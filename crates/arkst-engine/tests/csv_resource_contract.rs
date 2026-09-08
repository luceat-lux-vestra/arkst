use std::cell::RefCell;

use arkst_engine::csv_resource::{load_csv_resource, CsvMalformedKind, CsvResourceError};
use arkst_engine::{IncludedSource, ResourceAccessError, ResourceProvider, ResourceText};
use arkst_source::SourceId;

struct StubProvider {
    result: Result<ResourceText, ResourceAccessError>,
    calls: RefCell<Vec<(SourceId, String)>>,
}

impl StubProvider {
    fn text(text: &str) -> Self {
        Self {
            result: Ok(ResourceText {
                path: "docs/data/table.csv".to_string(),
                text: text.to_string(),
            }),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn error(error: ResourceAccessError) -> Self {
        Self {
            result: Err(error),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl ResourceProvider for StubProvider {
    fn source_path(&self, _source_id: SourceId) -> Option<String> {
        None
    }

    fn read_text(
        &self,
        source_id: SourceId,
        reference: &str,
    ) -> Result<ResourceText, ResourceAccessError> {
        self.calls
            .borrow_mut()
            .push((source_id, reference.to_string()));
        self.result.clone()
    }

    fn read_source(
        &self,
        _source_id: SourceId,
        _reference: &str,
    ) -> Result<IncludedSource, ResourceAccessError> {
        Err(ResourceAccessError::UnsupportedOperation {
            operation: "read_source",
        })
    }
}

fn malformed(text: &str) -> (usize, CsvMalformedKind) {
    let provider = StubProvider::text(text);
    match load_csv_resource(&provider, SourceId(7), "data/table.csv") {
        Err(CsvResourceError::Malformed { record, kind, .. }) => (record, kind),
        other => panic!("expected malformed CSV, got {other:?}"),
    }
}

#[test]
fn loader_uses_only_the_injected_provider_and_preserves_raw_ordered_cells() {
    let provider = StubProvider::text(" name ,note\r\n alice , two \r\n");
    let csv = load_csv_resource(&provider, SourceId(41), "../data/table.csv")
        .expect("bounded CSV resource loads");

    assert_eq!(
        provider.calls.into_inner(),
        vec![(SourceId(41), "../data/table.csv".to_string())]
    );
    assert_eq!(csv.path, "docs/data/table.csv");
    assert_eq!(csv.headers, [" name ", "note"]);
    assert_eq!(csv.rows, [vec![" alice ".to_string(), " two ".to_string()]]);
}

#[test]
fn pinned_default_quotes_delimiters_multiline_and_escaped_quotes_are_preserved() {
    let provider = StubProvider::text(
        "name,note\r\nalpha,\"one,two\"\r\nbeta,\"line1\r\nline2\"\r\nquote,\"a\"\"b\"\r\n",
    );
    let csv = load_csv_resource(&provider, SourceId(1), "table.csv").unwrap();

    assert_eq!(csv.headers, ["name", "note"]);
    assert_eq!(
        csv.rows,
        [
            vec!["alpha".to_string(), "one,two".to_string()],
            vec!["beta".to_string(), "line1\r\nline2".to_string()],
            vec!["quote".to_string(), "a\"b".to_string()],
        ]
    );
}

#[test]
fn pinned_default_unicode_line_terminators_and_leading_bom_are_recognized() {
    let provider = StubProvider::text("\u{feff}a,b\u{2028}1,2\u{2029}3,4\u{0085}");
    let csv = load_csv_resource(&provider, SourceId(1), "table.csv").unwrap();
    assert_eq!(csv.headers, ["a", "b"]);
    assert_eq!(
        csv.rows,
        [
            vec!["1".to_string(), "2".to_string()],
            vec!["3".to_string(), "4".to_string()],
        ]
    );

    for text in ["", "\u{feff}"] {
        let provider = StubProvider::text(text);
        let csv = load_csv_resource(&provider, SourceId(1), "table.csv").unwrap();
        assert!(csv.headers.is_empty(), "{text:?}");
        assert!(csv.rows.is_empty(), "{text:?}");
    }
}

#[test]
fn default_header_and_row_shape_failures_are_explicit_and_one_based() {
    assert_eq!(
        malformed("a,a\n1,2\n"),
        (
            1,
            CsvMalformedKind::DuplicateHeader {
                header: "a".to_string()
            }
        )
    );
    assert_eq!(
        malformed("a,b\n1\n"),
        (
            2,
            CsvMalformedKind::FieldCount {
                expected: 2,
                actual: 1
            }
        )
    );
    assert_eq!(
        malformed("a,b\n1,2,3\n"),
        (
            2,
            CsvMalformedKind::FieldCount {
                expected: 2,
                actual: 3
            }
        )
    );
    // kotlin-csv 1.10.0 defaults skipEmptyLine=false, so a blank data
    // record is parsed as one empty field and is shape-checked.
    assert_eq!(
        malformed("a,b\n\n"),
        (
            2,
            CsvMalformedKind::FieldCount {
                expected: 2,
                actual: 1
            }
        )
    );
}

#[test]
fn malformed_quote_classes_fail_closed_without_partial_rows() {
    assert_eq!(
        malformed("a\n\"unterminated"),
        (2, CsvMalformedKind::UnterminatedQuotedField)
    );
    assert_eq!(
        malformed("a\nvalue\"x\n"),
        (2, CsvMalformedKind::InvalidUnquotedQuote)
    );
    assert_eq!(
        malformed("a\n\"value\"x\n"),
        (2, CsvMalformedKind::UnexpectedAfterQuote { character: 'x' })
    );
}

#[test]
fn trailing_delimiters_and_unquoted_doubled_quotes_follow_pinned_default_rules() {
    let provider = StubProvider::text("a,b,c\nleft,,\nquoted,a\"\"b,last\n");
    let csv = load_csv_resource(&provider, SourceId(1), "table.csv").unwrap();
    assert_eq!(
        csv.rows,
        [
            vec!["left".to_string(), "".to_string(), "".to_string()],
            vec!["quoted".to_string(), "a\"b".to_string(), "last".to_string()],
        ]
    );
}

#[test]
fn provider_failures_propagate_without_csv_reclassification() {
    for error in [
        ResourceAccessError::NotFound {
            path: "docs/data/missing.csv".to_string(),
        },
        ResourceAccessError::Boundary {
            message: "escape".to_string(),
        },
        ResourceAccessError::InvalidUtf8 {
            path: "docs/data/table.csv".to_string(),
            message: "invalid byte".to_string(),
        },
    ] {
        let provider = StubProvider::error(error.clone());
        assert_eq!(
            load_csv_resource(&provider, SourceId(9), "data/table.csv"),
            Err(CsvResourceError::Resource(error))
        );
        assert_eq!(provider.calls.borrow().len(), 1);
    }
}
