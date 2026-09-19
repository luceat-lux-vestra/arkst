//! Deterministic CSV resource loading for the bounded #189 data contract.
//!
//! This module intentionally stops before Quarkdown table production. It reads
//! one source-relative logical resource through ResourceProvider, parses an
//! independently evidenced CSV subset, and returns trimmed raw string columns.
//! Markdown-cell conversion, captions, references, table IR, numbering, and
//! rendering remain #183/#181-owned concerns.

use crate::{ResourceAccessError, ResourceProvider};
use arkst_source::SourceId;
use std::collections::BTreeSet;

/// Parsed raw CSV data ready for a table producer.
///
/// Upstream Quarkdown creates table columns lazily while iterating data rows.
/// Consequently, an empty resource or a header-only resource has no materialized
/// columns even if a header record was parsed successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvResourceData {
    /// Canonical logical resource path returned by the provider.
    pub path: String,
    /// Ordered parsed columns. Empty when no data row exists.
    pub columns: Vec<CsvResourceColumn>,
}

/// One raw CSV column before any Markdown/content conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvResourceColumn {
    /// Trimmed header text.
    pub header: String,
    /// Trimmed cell text in source row order.
    pub cells: Vec<String>,
}

/// Deterministic failure from the bounded CSV resource layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CsvResourceError {
    #[error(transparent)]
    Resource(#[from] ResourceAccessError),

    #[error("CSV resource contains an unsupported blank record: {path}")]
    BlankRecord { path: String },

    #[error("CSV resource contains unsupported quoting syntax: {path}: {message}")]
    UnsupportedQuoting { path: String, message: String },

    #[error("CSV resource contains an unsupported record terminator: {path}: {message}")]
    UnsupportedRecordTerminator { path: String, message: String },

    #[error("CSV resource contains a duplicate header {header:?}: {path}")]
    DuplicateHeader { path: String, header: String },

    #[error(
        "CSV resource has inconsistent field count: {path}: expected {expected}, got {actual}"
    )]
    UnequalFields {
        path: String,
        expected: usize,
        actual: usize,
    },

    #[error("CSV resource exceeds field limit {limit}: {path} (attempted {attempted})")]
    FieldLimit {
        path: String,
        limit: usize,
        attempted: usize,
    },

    #[error("CSV resource could not be parsed: {path}: {message}")]
    Parse { path: String, message: String },
}

/// Reads and parses one logical CSV resource.
///
/// max_fields counts the parsed header fields plus every data field. The
/// caller must choose the bound explicitly; this function never falls back to
/// host filesystem state or an unbounded materialization policy.
pub fn load_csv_resource<R: ResourceProvider + ?Sized>(
    resources: &R,
    source_id: SourceId,
    reference: &str,
    max_fields: usize,
) -> Result<CsvResourceData, CsvResourceError> {
    let resource = resources.read_text(source_id, reference)?;
    parse_csv_resource_text(resource.path, &resource.text, max_fields)
}

/// Parses validated UTF-8 CSV text into raw trimmed columns.
///
/// The accepted syntax is the independently evidenced standard subset:
/// comma-separated records, RFC-style quoted fields, doubled quote escapes,
/// LF/CRLF record terminators, equal record widths, and unique raw headers.
///
/// Physical blank records and non-standard quote placement fail closed instead
/// of inheriting the more permissive behavior of a particular CSV library.
pub fn parse_csv_resource_text(
    path: impl Into<String>,
    text: &str,
    max_fields: usize,
) -> Result<CsvResourceData, CsvResourceError> {
    let path = path.into();
    validate_bounded_csv_syntax(&path, text)?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(text.as_bytes());

    let headers = reader
        .headers()
        .map_err(|error| map_csv_error(&path, error))?
        .clone();

    ensure_field_limit(&path, max_fields, headers.len())?;

    let mut seen = BTreeSet::new();
    for header in headers.iter() {
        if !seen.insert(header.to_string()) {
            return Err(CsvResourceError::DuplicateHeader {
                path,
                header: header.to_string(),
            });
        }
    }

    let trimmed_headers = headers
        .iter()
        .map(|header| header.trim().to_string())
        .collect::<Vec<_>>();

    let mut columns = Vec::new();
    let mut parsed_fields = headers.len();

    for record in reader.records() {
        let record = record.map_err(|error| map_csv_error(&path, error))?;
        let attempted = parsed_fields.saturating_add(record.len());
        ensure_field_limit(&path, max_fields, attempted)?;
        parsed_fields = attempted;

        if columns.is_empty() {
            columns = trimmed_headers
                .iter()
                .map(|header| CsvResourceColumn {
                    header: header.clone(),
                    cells: Vec::new(),
                })
                .collect();
        }

        for (column, cell) in columns.iter_mut().zip(record.iter()) {
            column.cells.push(cell.trim().to_string());
        }
    }

    Ok(CsvResourceData { path, columns })
}

fn ensure_field_limit(path: &str, limit: usize, attempted: usize) -> Result<(), CsvResourceError> {
    if attempted > limit {
        return Err(CsvResourceError::FieldLimit {
            path: path.to_string(),
            limit,
            attempted,
        });
    }
    Ok(())
}

fn map_csv_error(path: &str, error: csv::Error) -> CsvResourceError {
    match error.kind() {
        csv::ErrorKind::UnequalLengths {
            expected_len, len, ..
        } => CsvResourceError::UnequalFields {
            path: path.to_string(),
            expected: *expected_len as usize,
            actual: *len as usize,
        },
        _ => CsvResourceError::Parse {
            path: path.to_string(),
            message: error.to_string(),
        },
    }
}

fn validate_bounded_csv_syntax(path: &str, text: &str) -> Result<(), CsvResourceError> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut in_quotes = false;
    let mut at_field_start = true;
    let mut after_quoted_field = false;
    let mut record_has_content = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_quotes {
            record_has_content = true;
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    index += 2;
                    continue;
                }
                in_quotes = false;
                after_quoted_field = true;
            }
            index += 1;
            continue;
        }

        let terminator_len = match byte {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => Some(2),
            b'\n' => Some(1),
            b'\r' => {
                return Err(CsvResourceError::UnsupportedRecordTerminator {
                    path: path.to_string(),
                    message: "bare carriage-return record separators are outside the evidenced subset"
                        .to_string(),
                });
            }
            _ => None,
        };
        if let Some(len) = terminator_len {
            if !record_has_content {
                return Err(CsvResourceError::BlankRecord {
                    path: path.to_string(),
                });
            }
            record_has_content = false;
            at_field_start = true;
            after_quoted_field = false;
            index += len;
            continue;
        }

        record_has_content = true;

        if after_quoted_field {
            if byte == b',' {
                after_quoted_field = false;
                at_field_start = true;
                index += 1;
                continue;
            }
            return Err(CsvResourceError::UnsupportedQuoting {
                path: path.to_string(),
                message:
                    "characters follow a closing quote before a delimiter or record terminator"
                        .to_string(),
            });
        }

        if at_field_start {
            if byte == b'"' {
                in_quotes = true;
                at_field_start = false;
                index += 1;
                continue;
            }
            if byte == b',' {
                index += 1;
                continue;
            }
            at_field_start = false;
            index += 1;
            continue;
        }

        match byte {
            b',' => at_field_start = true,
            b'"' => {
                return Err(CsvResourceError::UnsupportedQuoting {
                    path: path.to_string(),
                    message: "quote appears inside an unquoted field".to_string(),
                });
            }
            _ => {}
        }
        index += 1;
    }

    if in_quotes {
        return Err(CsvResourceError::UnsupportedQuoting {
            path: path.to_string(),
            message: "quoted field is not terminated".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IncludedSource, ResourceText};
    use std::cell::RefCell;

    const LIMIT: usize = 128;

    #[test]
    fn parses_trimmed_headers_cells_quotes_crlf_and_multiline() {
        let parsed = parse_csv_resource_text(
            "data.csv",
            " Name , Value\r\n Alice , 1\r\n\"Bob, Jr.\",\"a \"\"quote\"\"\"\r\n",
            LIMIT,
        )
        .expect("bounded CSV");

        assert_eq!(
            parsed.columns,
            vec![
                CsvResourceColumn {
                    header: "Name".to_string(),
                    cells: vec!["Alice".to_string(), "Bob, Jr.".to_string()],
                },
                CsvResourceColumn {
                    header: "Value".to_string(),
                    cells: vec!["1".to_string(), "a \"quote\"".to_string()],
                },
            ]
        );

        let multiline = parse_csv_resource_text("multi.csv", "a,b\n\"one\nline\",two\n", LIMIT)
            .expect("multiline quoted cell");
        assert_eq!(multiline.columns[0].cells, vec!["one\nline"]);
        assert_eq!(multiline.columns[1].cells, vec!["two"]);
    }

    #[test]
    fn rejects_short_long_and_duplicate_header_records() {
        assert!(matches!(
            parse_csv_resource_text("short.csv", "a,b,c\n1,2\n", LIMIT),
            Err(CsvResourceError::UnequalFields {
                expected: 3,
                actual: 2,
                ..
            })
        ));
        assert!(matches!(
            parse_csv_resource_text("long.csv", "a,b\n1,2,3\n", LIMIT),
            Err(CsvResourceError::UnequalFields {
                expected: 2,
                actual: 3,
                ..
            })
        ));
        assert_eq!(
            parse_csv_resource_text("duplicate.csv", "a,a\n1,2\n", LIMIT),
            Err(CsvResourceError::DuplicateHeader {
                path: "duplicate.csv".to_string(),
                header: "a".to_string(),
            })
        );
    }

    #[test]
    fn duplicate_check_precedes_header_trim_like_upstream() {
        let parsed =
            parse_csv_resource_text("headers.csv", " a ,a\n1,2\n", LIMIT).expect("raw unique");
        assert_eq!(parsed.columns[0].header, "a");
        assert_eq!(parsed.columns[1].header, "a");
    }

    #[test]
    fn empty_and_header_only_resources_materialize_no_columns() {
        assert!(parse_csv_resource_text("empty.csv", "", LIMIT)
            .expect("empty")
            .columns
            .is_empty());
        assert!(parse_csv_resource_text("header.csv", "a,b\n", LIMIT)
            .expect("header only")
            .columns
            .is_empty());
    }

    #[test]
    fn blank_records_and_unproven_quote_forms_fail_closed() {
        assert!(matches!(
            parse_csv_resource_text("blank.csv", "a,b\n\n1,2\n", LIMIT),
            Err(CsvResourceError::BlankRecord { .. })
        ));
        assert!(matches!(
            parse_csv_resource_text("quote.csv", "a,b\nleft\"quote,right\n", LIMIT),
            Err(CsvResourceError::UnsupportedQuoting { .. })
        ));
        assert!(matches!(
            parse_csv_resource_text("quote.csv", "a,b\n\"unterminated,right\n", LIMIT),
            Err(CsvResourceError::UnsupportedQuoting { .. })
        ));
    }

    #[test]
    fn bare_carriage_return_record_terminators_fail_closed() {
        assert!(matches!(
            parse_csv_resource_text("bare-cr.csv", "a,b\r1,2\r", LIMIT),
            Err(CsvResourceError::UnsupportedRecordTerminator { .. })
        ));
    }

    #[test]
    fn field_limit_is_checked_before_unbounded_materialization() {
        assert_eq!(
            parse_csv_resource_text("limit.csv", "a,b\n1,2\n", 3),
            Err(CsvResourceError::FieldLimit {
                path: "limit.csv".to_string(),
                limit: 3,
                attempted: 4,
            })
        );
    }

    #[derive(Default)]
    struct Provider {
        calls: RefCell<Vec<(SourceId, String)>>,
        result: Option<Result<ResourceText, ResourceAccessError>>,
    }

    impl ResourceProvider for Provider {
        fn source_path(&self, _source_id: SourceId) -> Option<String> {
            Some("docs/main.qd".to_string())
        }

        fn read_text(
            &self,
            source_id: SourceId,
            reference: &str,
        ) -> Result<ResourceText, ResourceAccessError> {
            self.calls
                .borrow_mut()
                .push((source_id, reference.to_string()));
            self.result.clone().unwrap_or_else(|| {
                Ok(ResourceText {
                    path: "docs/data.csv".to_string(),
                    text: "a,b\n1,2\n".to_string(),
                })
            })
        }

        fn read_source(
            &self,
            source_id: SourceId,
            _reference: &str,
        ) -> Result<IncludedSource, ResourceAccessError> {
            Err(ResourceAccessError::UnknownSource { source_id })
        }
    }

    #[test]
    fn loader_uses_only_the_resource_provider_and_preserves_canonical_path() {
        let provider = Provider::default();
        let source_id = SourceId(1890);

        let parsed =
            load_csv_resource(&provider, source_id, "../data.csv", LIMIT).expect("provider CSV");

        assert_eq!(
            provider.calls.into_inner(),
            vec![(source_id, "../data.csv".to_string())]
        );
        assert_eq!(parsed.path, "docs/data.csv");
        assert_eq!(parsed.columns[0].cells, vec!["1"]);
        assert_eq!(parsed.columns[1].cells, vec!["2"]);
    }

    #[test]
    fn loader_propagates_resource_boundary_failures_without_fallback() {
        let provider = Provider {
            calls: RefCell::default(),
            result: Some(Err(ResourceAccessError::Boundary {
                message: "escape".to_string(),
            })),
        };

        assert_eq!(
            load_csv_resource(&provider, SourceId(1891), "../../secret.csv", LIMIT),
            Err(CsvResourceError::Resource(ResourceAccessError::Boundary {
                message: "escape".to_string(),
            }))
        );
    }
}
