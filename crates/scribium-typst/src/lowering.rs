/// Typst lowering — converts the Scribium IR into Typst source code.
///
/// Each IR node type maps to a Typst construct. The lowering pass
/// records source map entries as it generates code.
use scribium_core::ir::IrDocument;

/// Lower a Scribium IR document to Typst source code.
pub fn lower_to_typst(doc: &IrDocument) -> String {
    let _ = doc;
    // TODO: implement IR → Typst lowering
    String::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn lower_empty_document() {
        let doc = scribium_core::ir::IrDocument {};
        let result = super::lower_to_typst(&doc);
        assert_eq!(result, "");
    }
}