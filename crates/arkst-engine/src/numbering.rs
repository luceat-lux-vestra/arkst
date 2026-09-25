use crate::builtins;
use crate::value_conversion::InvocationValue;
use arkst_ir::{
    IrDictionary, IrDocumentType, IrNumberingFormat, IrNumberingLayer, IrNumberingToken, IrValue,
};

/// Parses the bounded v2.5.1 numbering-format grammar recorded by #153.
///
/// Counter markers are single unescaped characters. A backslash quotes the
/// following character. A trailing backslash is intentionally rejected
/// fail-closed because the canonical audit records only the "escapes the next
/// character" contract.
pub(crate) fn parse_numbering_format(text: &str) -> Result<IrNumberingFormat, String> {
    if text == "none" {
        return Ok(IrNumberingFormat { tokens: Vec::new() });
    }

    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut chars = text.chars();

    let flush_literal = |literal: &mut String, tokens: &mut Vec<IrNumberingToken>| {
        if !literal.is_empty() {
            tokens.push(IrNumberingToken::Literal(std::mem::take(literal)));
        }
    };

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let Some(escaped) = chars.next() else {
                return Err("numbering format ends with an incomplete escape".to_string());
            };
            literal.push(escaped);
            continue;
        }

        let token = match ch {
            '1' => Some(IrNumberingToken::Decimal),
            'a' => Some(IrNumberingToken::LowerAlpha),
            'A' => Some(IrNumberingToken::UpperAlpha),
            'i' => Some(IrNumberingToken::LowerRoman),
            'I' => Some(IrNumberingToken::UpperRoman),
            _ => None,
        };
        if let Some(token) = token {
            flush_literal(&mut literal, &mut tokens);
            tokens.push(token);
        } else {
            literal.push(ch);
        }
    }
    flush_literal(&mut literal, &mut tokens);
    Ok(IrNumberingFormat { tokens })
}

fn format_from_value(value: &IrValue) -> Result<IrNumberingFormat, String> {
    let text = builtins::scalar_string_conversion(&InvocationValue::static_value(value.clone()))
        .map_err(|_| "numbering format must convert to String".to_string())?;
    parse_numbering_format(&text)
}

/// Converts one fully evaluated Dictionary candidate into an atomic numbering
/// mutation. Every input pair is retained in extra, including built-in keys,
/// matching the canonical #153 audit.
pub(crate) fn layer_from_dictionary(
    dictionary: &IrDictionary,
    merge: bool,
    document_type: IrDocumentType,
) -> Result<IrNumberingLayer, String> {
    let mut layer = IrNumberingLayer {
        merge,
        document_type,
        ..IrNumberingLayer::default()
    };

    for pair in &dictionary.entries {
        let IrValue::String(key) = pair.first.as_ref() else {
            return Err("numbering keys must remain typed strings".to_string());
        };
        let format = format_from_value(pair.second.as_ref())?;

        match key.as_str() {
            "headings" => layer.headings = Some(format.clone()),
            "figures" => layer.figures = Some(format.clone()),
            "tables" => layer.tables = Some(format.clone()),
            "equations" => layer.equations = Some(format.clone()),
            "code" => layer.code = Some(format.clone()),
            "footnotes" => layer.footnotes = Some(format.clone()),
            _ => {}
        }
        layer.extra.push((key.clone(), format));
    }

    Ok(layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkst_ir::{IrPair, IrValue};
    use arkst_source::{SourceId, SourceSpan};

    #[test]
    fn numbering_format_preserves_counter_kinds_literals_and_escapes() {
        let parsed = parse_numbering_format("1.\\a-A/iI").unwrap();
        assert_eq!(
            parsed.tokens,
            vec![
                IrNumberingToken::Decimal,
                IrNumberingToken::Literal(".a-".to_string()),
                IrNumberingToken::UpperAlpha,
                IrNumberingToken::Literal("/".to_string()),
                IrNumberingToken::LowerRoman,
                IrNumberingToken::UpperRoman,
            ]
        );
    }

    #[test]
    fn none_is_an_empty_non_counting_format_and_trailing_escape_fails_closed() {
        assert!(parse_numbering_format("none").unwrap().tokens.is_empty());
        assert!(parse_numbering_format("1\\").is_err());
    }

    #[test]
    fn built_in_keys_are_also_retained_in_extra() {
        let span = SourceSpan::new(SourceId(175), 0, 1);
        let dictionary = IrDictionary {
            entries: vec![
                IrPair {
                    first: Box::new(IrValue::String("headings".to_string())),
                    second: Box::new(IrValue::String("1".to_string())),
                    span,
                },
                IrPair {
                    first: Box::new(IrValue::String("custom".to_string())),
                    second: Box::new(IrValue::String("A".to_string())),
                    span,
                },
            ],
            span,
        };
        let layer = layer_from_dictionary(&dictionary, true, IrDocumentType::Paged).unwrap();
        assert!(layer.headings.is_some());
        assert_eq!(layer.extra.len(), 2);
        assert_eq!(layer.extra[0].0, "headings");
        assert_eq!(layer.extra[1].0, "custom");
    }
}
