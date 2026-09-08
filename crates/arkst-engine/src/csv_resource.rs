//! Platform-neutral loading and parsing for Quarkdown's CSV resource prerequisite.
//!
//! This module intentionally stops before the `.csv` language-level table producer.
//! It owns only the #189 resource boundary: resolve validated UTF-8 text through a
//! `ResourceProvider`, parse the pinned kotlin-csv 1.10.0 default CSV shape, and
//! retain raw ordered headers/cells plus canonical logical resource identity.

use std::collections::BTreeSet;
use std::mem;

use arkst_source::SourceId;

use crate::{ResourceAccessError, ResourceProvider, ResourceText};

/// Raw parsed CSV resource data before Quarkdown table/content transformation.
///
/// Header and cell strings are deliberately not trimmed here. Pinned Quarkdown
/// performs `trim()` only while constructing table cells, which is #183-owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvResource {
    /// Canonical project-logical path returned by the resource provider.
    pub path: String,
    /// Raw first-record fields, in source order.
    pub headers: Vec<String>,
    /// Raw data records, each with exactly `headers.len()` fields.
    pub rows: Vec<Vec<String>>,
}

/// Deterministic malformed-input classes for the bounded CSV parser.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CsvMalformedKind {
    #[error("a quote inside an unquoted field must be doubled")]
    InvalidUnquotedQuote,
    #[error("unexpected character `{character}` after a closing quote")]
    UnexpectedAfterQuote { character: char },
    #[error("quoted field is not terminated")]
    UnterminatedQuotedField,
    #[error("header `{header}` is duplicated")]
    DuplicateHeader { header: String },
    #[error("record has {actual} fields but header has {expected}")]
    FieldCount { expected: usize, actual: usize },
}

/// Resource or deterministic CSV-format failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CsvResourceError {
    #[error(transparent)]
    Resource(#[from] ResourceAccessError),
    #[error("malformed CSV resource {path} at record {record}: {kind}")]
    Malformed {
        path: String,
        /// One-based logical CSV record index.
        record: usize,
        kind: CsvMalformedKind,
    },
}

/// Loads and parses one source-relative logical CSV resource.
///
/// The provider remains the sole resource authority. This function performs no host
/// path resolution, filesystem/network/process access, table construction, Markdown
/// parsing, caption/reference binding, or output lowering.
pub fn load_csv_resource<P: ResourceProvider + ?Sized>(
    provider: &P,
    source_id: SourceId,
    reference: &str,
) -> Result<CsvResource, CsvResourceError> {
    let ResourceText { path, text } = provider.read_text(source_id, reference)?;
    let records = parse_records(&text).map_err(|error| CsvResourceError::Malformed {
        path: path.clone(),
        record: error.record,
        kind: error.kind,
    })?;

    let Some((headers, rows)) = records.split_first() else {
        return Ok(CsvResource {
            path,
            headers: Vec::new(),
            rows: Vec::new(),
        });
    };

    let mut seen = BTreeSet::new();
    for header in headers {
        if !seen.insert(header.clone()) {
            return Err(CsvResourceError::Malformed {
                path,
                record: 1,
                kind: CsvMalformedKind::DuplicateHeader {
                    header: header.clone(),
                },
            });
        }
    }

    for (index, row) in rows.iter().enumerate() {
        if row.len() != headers.len() {
            return Err(CsvResourceError::Malformed {
                path,
                record: index + 2,
                kind: CsvMalformedKind::FieldCount {
                    expected: headers.len(),
                    actual: row.len(),
                },
            });
        }
    }

    Ok(CsvResource {
        path,
        headers: headers.clone(),
        rows: rows.to_vec(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    Start,
    Field,
    AfterDelimiter,
    Quoted,
    AfterQuote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParseError {
    record: usize,
    kind: CsvMalformedKind,
}

fn parse_records(text: &str) -> Result<Vec<Vec<String>>, ParseError> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut records = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut state = ParseState::Start;
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        match state {
            ParseState::Start => match ch {
                '\u{feff}' => {}
                ',' => {
                    row.push(mem::take(&mut field));
                    state = ParseState::AfterDelimiter;
                }
                '"' => state = ParseState::Quoted,
                _ if is_line_terminator(ch) => {
                    row.push(mem::take(&mut field));
                    finish_record(&mut records, &mut row);
                    consume_lf_after_cr(ch, next, &mut index);
                }
                _ => {
                    field.push(ch);
                    state = ParseState::Field;
                }
            },
            ParseState::Field => match ch {
                '"' => {
                    if next == Some('"') {
                        field.push('"');
                        index += 1;
                    } else {
                        return Err(ParseError {
                            record: records.len() + 1,
                            kind: CsvMalformedKind::InvalidUnquotedQuote,
                        });
                    }
                }
                ',' => {
                    row.push(mem::take(&mut field));
                    state = ParseState::AfterDelimiter;
                }
                _ if is_line_terminator(ch) => {
                    row.push(mem::take(&mut field));
                    finish_record(&mut records, &mut row);
                    state = ParseState::Start;
                    consume_lf_after_cr(ch, next, &mut index);
                }
                _ => field.push(ch),
            },
            ParseState::AfterDelimiter => match ch {
                '"' => state = ParseState::Quoted,
                ',' => row.push(mem::take(&mut field)),
                _ if is_line_terminator(ch) => {
                    row.push(mem::take(&mut field));
                    finish_record(&mut records, &mut row);
                    state = ParseState::Start;
                    consume_lf_after_cr(ch, next, &mut index);
                }
                _ => {
                    field.push(ch);
                    state = ParseState::Field;
                }
            },
            ParseState::Quoted => {
                if ch == '"' {
                    if next == Some('"') {
                        field.push('"');
                        index += 1;
                    } else {
                        state = ParseState::AfterQuote;
                    }
                } else {
                    // Delimiters and every supported line terminator are literal data
                    // while a quoted field is open. CRLF is therefore preserved as two
                    // characters, matching the pinned reader/parser composition.
                    field.push(ch);
                }
            }
            ParseState::AfterQuote => match ch {
                ',' => {
                    row.push(mem::take(&mut field));
                    state = ParseState::AfterDelimiter;
                }
                _ if is_line_terminator(ch) => {
                    row.push(mem::take(&mut field));
                    finish_record(&mut records, &mut row);
                    state = ParseState::Start;
                    consume_lf_after_cr(ch, next, &mut index);
                }
                _ => {
                    return Err(ParseError {
                        record: records.len() + 1,
                        kind: CsvMalformedKind::UnexpectedAfterQuote { character: ch },
                    });
                }
            },
        }
        index += 1;
    }

    match state {
        ParseState::Start => {}
        ParseState::Field | ParseState::AfterQuote => {
            row.push(field);
            finish_record(&mut records, &mut row);
        }
        ParseState::AfterDelimiter => {
            row.push(field);
            finish_record(&mut records, &mut row);
        }
        ParseState::Quoted => {
            return Err(ParseError {
                record: records.len() + 1,
                kind: CsvMalformedKind::UnterminatedQuotedField,
            });
        }
    }

    Ok(records)
}

fn finish_record(records: &mut Vec<Vec<String>>, row: &mut Vec<String>) {
    records.push(mem::take(row));
}

fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}' | '\u{0085}')
}

fn consume_lf_after_cr(ch: char, next: Option<char>, index: &mut usize) {
    if ch == '\r' && next == Some('\n') {
        *index += 1;
    }
}
