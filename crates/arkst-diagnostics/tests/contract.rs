use arkst_diagnostics::{Diagnostic, Severity};
use arkst_source::{SourceId, SourceSpan};
use serde_json::json;

#[test]
fn severity_display_preserves_existing_strings() {
    assert_eq!(Severity::Error.to_string(), "error");
    assert_eq!(Severity::Warning.to_string(), "warning");
    assert_eq!(Severity::Hint.to_string(), "hint");
}

#[test]
fn diagnostic_display_and_error_contract_are_preserved() {
    let diagnostic = Diagnostic {
        code: "E1234".to_string(),
        severity: Severity::Error,
        message: "message".to_string(),
        primary: None,
        secondary: Vec::new(),
        hints: Vec::new(),
    };

    assert_eq!(diagnostic.to_string(), "[error] E1234: message");
    assert!(std::error::Error::source(&diagnostic).is_none());
}

#[test]
fn diagnostic_serialization_preserves_fields_and_source_spans() {
    let primary = SourceSpan::new(SourceId(7), 2, 5);
    let secondary = SourceSpan::new(SourceId(7), 9, 12);
    let diagnostic = Diagnostic {
        code: "E1234".to_string(),
        severity: Severity::Warning,
        message: "message".to_string(),
        primary: Some(primary),
        secondary: vec![secondary],
        hints: vec!["try again".to_string()],
    };

    let encoded = serde_json::to_value(&diagnostic).expect("diagnostic serializes");
    assert_eq!(
        encoded,
        json!({
            "code": "E1234",
            "severity": "Warning",
            "message": "message",
            "primary": {"source_id": 7, "start": 2, "end": 5},
            "secondary": [{"source_id": 7, "start": 9, "end": 12}],
            "hints": ["try again"]
        })
    );

    let decoded: Diagnostic = serde_json::from_value(encoded).expect("diagnostic deserializes");
    assert_eq!(decoded.code, diagnostic.code);
    assert!(matches!(decoded.severity, Severity::Warning));
    assert_eq!(decoded.message, diagnostic.message);
    assert_eq!(decoded.primary, Some(primary));
    assert_eq!(decoded.secondary, vec![secondary]);
    assert_eq!(decoded.hints, diagnostic.hints);
}
