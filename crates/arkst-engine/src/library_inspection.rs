//! Clean-room Quarkdown v2.6 library-inspection ordering evidence.
//!
//! The sequence below was observed from the official v2.6.0 Linux x64 release
//! in disposable black-box probe PR #360. It is used only to order names that
//! Arkst actually exposes through its evaluator; presence is never inferred
//! from this list.

pub(crate) const V260_STDLIB_FUNCTION_ORDER: &[&str] = &[
    "islower",
    "functionexists",
    "paragraphstyle",
    "multiply",
    "numbering",
    "pair",
    "appended",
    "heading",
    "abs",
    "clip",
    "libexists",
    "listfiles",
    "match",
    "font",
    "equals",
    "loremipsum",
    "markdown",
    "doclang",
    "landscape",
    "tan",
    "plaintext",
    "filename",
    "tablebyrows",
    "uppercase",
    "round",
    "sumall",
    "todo",
    "sin",
    "grid",
    "not",
    "ifpresent",
    "htmloptions",
    "csv",
    "subdocument",
    "groupvalues",
    "doctype",
    "truncate",
    "css",
    "let",
    "dockeywords",
    "includeall",
    "captionposition",
    "cos",
    "collapse",
    "get",
    "code",
    "docauthor",
    "bibliography",
    "concatenate",
    "getat",
    "keybinding",
    "iseven",
    "cite",
    "link",
    "sum",
    "first",
    "speakernote",
    "textcollapse",
    "align",
    "var",
    "pagebreak",
    "texmacro",
    "noautopagebreak",
    "emoji",
    "rem",
    "br",
    "extend",
    "tablefilter",
    "container",
    "html",
    "debug",
    "fragment",
    "subdocumentgraph",
    "pathtoroot",
    "docdescription",
    "tablesort",
    "whitespace",
    "pow",
    "capitalize",
    "env",
    "range",
    "libraries",
    "isgreater",
    "currentpage",
    "codespan",
    "sqrt",
    "navigation",
    "dictionary",
    "foreach",
    "docauthors",
    "autopagebreak",
    "subtract",
    "read",
    "log",
    "repeat",
    "none",
    "second",
    "docname",
    "theme",
    "takeif",
    "footer",
    "isnone",
    "allemojis",
    "pagemargin",
    "logn",
    "reversed",
    "localization",
    "icon",
    "string",
    "llmstxt",
    "pageformat",
    "lastheading",
    "function",
    "math",
    "third",
    "lowercase",
    "tablecolumn",
    "table",
    "error",
    "float",
    "isnotempty",
    "localize",
    "libfunctions",
    "divide",
    "pi",
    "slides",
    "cssproperties",
    "marker",
    "xychart",
    "size",
    "row",
    "ref",
    "tablecolumns",
    "average",
    "totalpages",
    "if",
    "json",
    "node",
    "box",
    "resetpagenumber",
    "paragraph",
    "figure",
    "filetree",
    "prepended",
    "formatpagenumber",
    "otherwise",
    "negate",
    "text",
    "tablecompute",
    "ifnot",
    "include",
    "image",
    "last",
    "fullspan",
    "isempty",
    "center",
    "column",
    "sorted",
    "tableofcontents",
    "distinct",
    "mermaid",
    "nonumbering",
    "numbered",
    "startswith",
];

pub(crate) const STDLIB_LIBRARY: &str = "stdlib";
pub(crate) const FUNCTION_LIBRARY_PREFIX: &str = "__func__";

/// One evaluator-visible registration after the implicit `stdlib` entry.
///
/// Container libraries and function pseudo-libraries share one ordered stream
/// because the v2.6 oracle exposes them in registration order. This is runtime
/// state only; it does not add a second provider/package registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RegistryEntry {
    Container(String),
    Function(String),
}

impl RegistryEntry {
    pub(crate) fn visible_name(&self) -> String {
        match self {
            Self::Container(name) => name.clone(),
            Self::Function(name) => function_library_name(name),
        }
    }

    pub(crate) fn function_name(&self) -> Option<&str> {
        match self {
            Self::Container(_) => None,
            Self::Function(name) => Some(name),
        }
    }

    pub(crate) fn matches_visible_name(&self, name: &str) -> bool {
        match self {
            Self::Container(container) => container == name,
            Self::Function(function) => function_library_name(function) == name,
        }
    }
}

pub(crate) fn function_library_name(name: &str) -> String {
    format!("{FUNCTION_LIBRARY_PREFIX}{name}")
}

pub(crate) fn function_name_from_library(library: &str) -> Option<&str> {
    library.strip_prefix(FUNCTION_LIBRARY_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_v260_order_is_complete_and_unique() {
        use std::collections::BTreeSet;

        assert_eq!(V260_STDLIB_FUNCTION_ORDER.len(), 164);
        assert_eq!(
            V260_STDLIB_FUNCTION_ORDER
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
            164
        );
        assert_eq!(
            &V260_STDLIB_FUNCTION_ORDER[..8],
            &[
                "islower",
                "functionexists",
                "paragraphstyle",
                "multiply",
                "numbering",
                "pair",
                "appended",
                "heading",
            ]
        );
        assert_eq!(
            &V260_STDLIB_FUNCTION_ORDER[V260_STDLIB_FUNCTION_ORDER.len() - 8..],
            &[
                "column",
                "sorted",
                "tableofcontents",
                "distinct",
                "mermaid",
                "nonumbering",
                "numbered",
                "startswith",
            ]
        );
    }

    #[test]
    fn registry_entry_names_match_oracle_shapes() {
        let container = RegistryEntry::Container("alpha".into());
        let function = RegistryEntry::Function("hello".into());
        assert_eq!(container.visible_name(), "alpha");
        assert_eq!(container.function_name(), None);
        assert!(container.matches_visible_name("alpha"));
        assert_eq!(function.visible_name(), "__func__hello");
        assert_eq!(function.function_name(), Some("hello"));
        assert!(function.matches_visible_name("__func__hello"));
        assert_eq!(function_name_from_library("__func__hello"), Some("hello"));
    }
}