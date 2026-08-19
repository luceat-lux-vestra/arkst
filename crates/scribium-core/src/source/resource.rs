//! Backend-neutral resource-reference classification.
//!
//! Resource references remain logical strings until a native host supplies a
//! resource context. This module deliberately does not resolve paths or touch
//! the filesystem.

use crate::source::{SourceId, VirtualPathBuf, VirtualPathError};

/// Failure while resolving or reading a logical project resource.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourceAccessError {
    /// The reference is not a project-relative local path.
    #[error("resource reference is not a local project path: {reference}")]
    UnsupportedReference { reference: String },
    /// The source identity used as the relative base is not present.
    #[error("source identity is not present in the project: {0:?}")]
    UnknownSource(SourceId),
    /// The normalized reference leaves the virtual project root.
    #[error("resource path leaves the project boundary: {0}")]
    Boundary(VirtualPathError),
    /// No source or asset exists at the normalized path.
    #[error("resource not found: {0}")]
    NotFound(VirtualPathBuf),
    /// A text-oriented builtin received bytes that are not valid UTF-8.
    #[error("resource is not valid UTF-8: {path}: {message}")]
    InvalidUtf8 {
        path: VirtualPathBuf,
        message: String,
    },
}

/// The semantic class of a source-language resource reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceReference {
    /// A project-relative path such as `./assets/logo.svg` or `../shared/logo.svg`.
    LocalPath(String),
    /// A root-relative, absolute, or platform-specific filesystem path.
    AbsolutePath(String),
    /// A URI with an explicit scheme, such as `https`, `data`, or `custom`.
    Uri { scheme: String, value: String },
}

impl ResourceReference {
    /// Classifies a logical resource reference without resolving it.
    pub fn classify(value: &str) -> Self {
        if is_absolute_or_platform_path(value) {
            return Self::AbsolutePath(value.to_string());
        }

        if let Some(scheme) = uri_scheme(value) {
            return Self::Uri {
                scheme,
                value: value.to_string(),
            };
        }

        Self::LocalPath(value.to_string())
    }

    /// Returns the original logical reference string.
    pub fn value(&self) -> &str {
        match self {
            Self::LocalPath(value) | Self::AbsolutePath(value) => value,
            Self::Uri { value, .. } => value,
        }
    }

    /// Returns whether the reference is safe to pass to a project-rooted
    /// local filesystem backend.
    pub fn is_local_path(&self) -> bool {
        matches!(self, Self::LocalPath(_))
    }
}

fn is_absolute_or_platform_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('\\')
        || value.contains('\\')
        || (value.len() >= 2
            && value.as_bytes()[1] == b':'
            && value.as_bytes()[0].is_ascii_alphabetic())
}

fn uri_scheme(value: &str) -> Option<String> {
    let (scheme, _) = value.split_once(':')?;
    if scheme.is_empty()
        || !scheme.as_bytes()[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::ResourceReference;

    #[test]
    fn classifies_local_absolute_and_uri_references_without_resolution() {
        assert_eq!(
            ResourceReference::classify("./assets/my logo.png"),
            ResourceReference::LocalPath("./assets/my logo.png".into())
        );
        assert_eq!(
            ResourceReference::classify("../shared/logo.svg"),
            ResourceReference::LocalPath("../shared/logo.svg".into())
        );
        assert_eq!(
            ResourceReference::classify("/etc/passwd"),
            ResourceReference::AbsolutePath("/etc/passwd".into())
        );
        assert_eq!(
            ResourceReference::classify(r"C:\\Users\\foo\\a.png"),
            ResourceReference::AbsolutePath(r"C:\\Users\\foo\\a.png".into())
        );
        assert_eq!(
            ResourceReference::classify("HTTPS://example.com/a.png"),
            ResourceReference::Uri {
                scheme: "https".into(),
                value: "HTTPS://example.com/a.png".into()
            }
        );
        assert_eq!(
            ResourceReference::classify("data:image/svg+xml;base64,abc"),
            ResourceReference::Uri {
                scheme: "data".into(),
                value: "data:image/svg+xml;base64,abc".into()
            }
        );
    }
}
