//! Virtual path abstraction for platform-independent paths.
//!
//! `VirtualPathBuf` provides OS-independent path handling for the Scribium
//! compilation pipeline. It uses forward slashes as separators and is
//! case-sensitive. All paths are in canonical form: no leading slash, no
//! `.` or `..` components, no trailing slash (except root).

use std::fmt;
use std::hash::Hash;
use std::ops::Deref;

/// Error type for virtual path validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VirtualPathError {
    #[error("absolute path not allowed: {0}")]
    Absolute(String),
    #[error("path escapes project root: {0}")]
    EscapesRoot(String),
    #[error("invalid component: {0}")]
    InvalidComponent(String),
    #[error("trailing slash not allowed: {0}")]
    TrailingSlash(String),
    #[error("windows path not allowed: {0}")]
    WindowsPath(String),
}

/// An owned, platform-independent virtual path in canonical form.
///
/// Uses forward slashes as separators. Case-sensitive.
/// Does not contain `.` or `..` components (normalized).
/// Does not have a leading slash (project-relative).
/// Does not have a trailing slash (except root sentinel).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualPathBuf {
    path: String,
}

impl VirtualPathBuf {
    /// Creates a new virtual path from a string with validation.
    ///
    /// Rules:
    /// - No leading slash (project-relative paths)
    /// - No `..` that escapes project root
    /// - No trailing slash (except for root sentinel)
    /// - No Windows backslash paths
    /// - No drive letters
    /// - Empty string creates root sentinel
    pub fn parse(path: impl AsRef<str>) -> Result<Self, VirtualPathError> {
        let input = path.as_ref();

        if input.is_empty() || input == "/" {
            return Ok(Self::root());
        }

        // Reject Windows paths
        if input.contains('\\') {
            return Err(VirtualPathError::WindowsPath(input.to_string()));
        }
        if input.len() >= 2
            && input.as_bytes()[1] == b':'
            && input.as_bytes()[0].is_ascii_alphabetic()
        {
            return Err(VirtualPathError::WindowsPath(input.to_string()));
        }

        // Reject absolute paths (leading slash)
        if input.starts_with('/') {
            return Err(VirtualPathError::Absolute(input.to_string()));
        }

        // Reject trailing slash (except root)
        if input.ends_with('/') && input != "/" {
            return Err(VirtualPathError::TrailingSlash(input.to_string()));
        }

        // Normalize and validate
        let mut components = Vec::new();

        for part in input.split('/') {
            match part {
                "" | "." => {
                    // Skip empty components (from multiple slashes) and "."
                    continue;
                }
                ".." => {
                    // Pop last component if not at root
                    if !components.is_empty() {
                        components.pop();
                    } else {
                        return Err(VirtualPathError::EscapesRoot(input.to_string()));
                    }
                }
                part => {
                    // Validate component (no control chars, no colon for windows)
                    if part.chars().any(|c| c.is_ascii_control() || c == ':') {
                        return Err(VirtualPathError::InvalidComponent(part.to_string()));
                    }
                    components.push(part);
                }
            }
        }

        let path = if components.is_empty() {
            "/".to_string()
        } else {
            components.join("/")
        };

        Ok(Self { path })
    }

    /// Creates the root sentinel path.
    pub fn root() -> Self {
        Self {
            path: "/".to_string(),
        }
    }

    /// Returns true if this is the root sentinel.
    pub fn is_root(&self) -> bool {
        self.path == "/"
    }

    /// Returns the path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Returns the path as a string.
    pub fn into_string(self) -> String {
        self.path
    }

    /// Joins another path component, returning a new VirtualPathBuf.
    ///
    /// The joined component must be a valid relative path component.
    pub fn join(&self, path: impl AsRef<str>) -> Result<Self, VirtualPathError> {
        let input = path.as_ref();

        if input.is_empty() {
            return Ok(self.clone());
        }

        if input.contains('\\') {
            return Err(VirtualPathError::WindowsPath(input.to_string()));
        }

        if input.starts_with('/') {
            return Err(VirtualPathError::Absolute(input.to_string()));
        }

        if input.ends_with('/') && input != "/" {
            return Err(VirtualPathError::TrailingSlash(input.to_string()));
        }

        // If self is root, just use the component
        if self.is_root() {
            return Self::parse(input);
        }

        // Join and re-parse for validation
        let joined = format!("{}/{}", self.path, input);
        Self::parse(joined)
    }

    /// Appends a path component in place.
    ///
    /// Returns error if the component would violate invariants.
    pub fn push(&mut self, component: &str) -> Result<(), VirtualPathError> {
        let new = self.join(component)?;
        self.path = new.path;
        Ok(())
    }

    /// Removes the last component, returns false if at root.
    pub fn pop(&mut self) -> bool {
        if self.is_root() {
            return false;
        }

        if let Some(pos) = self.path.rfind('/') {
            if pos == 0 {
                self.path.truncate(1);
            } else {
                self.path.truncate(pos);
            }
        } else {
            self.path = "/".to_string();
        }
        true
    }

    /// Returns the parent path.
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }

        if let Some(pos) = self.path.rfind('/') {
            if pos == 0 {
                Some(Self::root())
            } else {
                Some(Self {
                    path: self.path[..pos].to_string(),
                })
            }
        } else {
            Some(Self::root())
        }
    }

    /// Returns the final component (file name).
    pub fn file_name(&self) -> Option<&str> {
        if self.is_root() {
            return None;
        }
        self.path.split('/').next_back()
    }
}

impl AsRef<str> for VirtualPathBuf {
    fn as_ref(&self) -> &str {
        &self.path
    }
}

impl Deref for VirtualPathBuf {
    type Target = str;

    fn deref(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for VirtualPathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}

impl fmt::Debug for VirtualPathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VirtualPathBuf").field(&self.path).finish()
    }
}

impl TryFrom<String> for VirtualPathBuf {
    type Error = VirtualPathError;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        Self::parse(path)
    }
}

impl TryFrom<&str> for VirtualPathBuf {
    type Error = VirtualPathError;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        Self::parse(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hasher;

    #[test]
    fn test_root() {
        let root = VirtualPathBuf::root();
        assert!(root.is_root());
        assert_eq!(root.as_str(), "/");
    }

    #[test]
    fn test_parse_normal() {
        let p = VirtualPathBuf::parse("a/b/c").unwrap();
        assert_eq!(p.as_str(), "a/b/c");
        assert!(!p.is_root());
    }

    #[test]
    fn test_parse_empty() {
        let p = VirtualPathBuf::parse("").unwrap();
        assert!(p.is_root());
        assert_eq!(p.as_str(), "/");
    }

    #[test]
    fn test_parse_root_string() {
        // parse("/") must return the root sentinel (round-trip of root's Display)
        let root = VirtualPathBuf::root();
        assert_eq!(VirtualPathBuf::parse("/").unwrap(), root);
        assert_eq!(VirtualPathBuf::parse(VirtualPathBuf::root()).unwrap(), root);
    }

    #[test]
    fn test_root_display_round_trip() {
        // A VirtualPathBuf parses back to itself (canonical form is stable).
        let root = VirtualPathBuf::root();
        assert_eq!(VirtualPathBuf::parse(VirtualPathBuf::root()).unwrap(), root);
    }

    #[test]
    fn test_try_from_str_success() {
        let p = VirtualPathBuf::try_from("a/b").unwrap();
        assert_eq!(p.as_str(), "a/b");
    }

    #[test]
    fn test_try_from_str_trailing_slash() {
        assert!(matches!(
            VirtualPathBuf::try_from("a/b/"),
            Err(VirtualPathError::TrailingSlash(_))
        ));
    }

    #[test]
    fn test_try_from_string_success() {
        let p = VirtualPathBuf::try_from("a/b".to_string()).unwrap();
        assert_eq!(p.as_str(), "a/b");
    }

    #[test]
    fn test_try_from_string_root() {
        let p = VirtualPathBuf::try_from("/".to_string()).unwrap();
        assert!(p.is_root());
    }

    #[test]
    fn test_try_from_string_invalid() {
        assert!(matches!(
            VirtualPathBuf::try_from("a\\b".to_string()),
            Err(VirtualPathError::WindowsPath(_))
        ));
    }

    #[test]
    fn test_reject_absolute() {
        assert!(matches!(
            VirtualPathBuf::parse("/a/b"),
            Err(VirtualPathError::Absolute(_))
        ));
    }

    #[test]
    fn test_reject_root_escape() {
        assert!(matches!(
            VirtualPathBuf::parse("../a"),
            Err(VirtualPathError::EscapesRoot(_))
        ));
        assert!(matches!(
            VirtualPathBuf::parse("a/../../b"),
            Err(VirtualPathError::EscapesRoot(_))
        ));
    }

    #[test]
    fn test_reject_trailing_slash() {
        assert!(matches!(
            VirtualPathBuf::parse("a/b/"),
            Err(VirtualPathError::TrailingSlash(_))
        ));
    }

    #[test]
    fn test_reject_windows_backslash() {
        assert!(matches!(
            VirtualPathBuf::parse("a\\b"),
            Err(VirtualPathError::WindowsPath(_))
        ));
    }

    #[test]
    fn test_reject_windows_drive() {
        assert!(matches!(
            VirtualPathBuf::parse("C:/a"),
            Err(VirtualPathError::WindowsPath(_))
        ));
    }

    #[test]
    fn test_normalize_repeated_slashes() {
        let p = VirtualPathBuf::parse("a//b///c").unwrap();
        assert_eq!(p.as_str(), "a/b/c");
    }

    #[test]
    fn test_normalize_dot() {
        let p = VirtualPathBuf::parse("a/./b").unwrap();
        assert_eq!(p.as_str(), "a/b");
    }

    #[test]
    fn test_normalize_internal_dotdot() {
        let p = VirtualPathBuf::parse("a/b/../c").unwrap();
        assert_eq!(p.as_str(), "a/c");
    }

    #[test]
    fn test_join() {
        let base = VirtualPathBuf::parse("a/b").unwrap();
        let joined = base.join("c/d").unwrap();
        assert_eq!(joined.as_str(), "a/b/c/d");
    }

    #[test]
    fn test_join_root() {
        let base = VirtualPathBuf::root();
        let joined = base.join("a/b").unwrap();
        assert_eq!(joined.as_str(), "a/b");
    }

    #[test]
    fn test_join_rejects_absolute() {
        let base = VirtualPathBuf::parse("a/b").unwrap();
        assert!(matches!(
            base.join("/c"),
            Err(VirtualPathError::Absolute(_))
        ));
    }

    #[test]
    fn test_join_rejects_trailing_slash() {
        let base = VirtualPathBuf::parse("a/b").unwrap();
        assert!(matches!(
            base.join("c/"),
            Err(VirtualPathError::TrailingSlash(_))
        ));
    }

    #[test]
    fn test_push() {
        let mut p = VirtualPathBuf::parse("a").unwrap();
        p.push("b").unwrap();
        assert_eq!(p.as_str(), "a/b");
    }

    #[test]
    fn test_pop() {
        let mut p = VirtualPathBuf::parse("a/b").unwrap();
        assert!(p.pop());
        assert_eq!(p.as_str(), "a");
        assert!(p.pop());
        assert!(p.is_root());
        assert!(!p.pop());
    }

    #[test]
    fn test_parent() {
        let p = VirtualPathBuf::parse("a/b/c").unwrap();
        assert_eq!(p.parent().unwrap().as_str(), "a/b");

        let p = VirtualPathBuf::parse("a").unwrap();
        assert_eq!(p.parent().unwrap().as_str(), "/");

        assert!(VirtualPathBuf::root().parent().is_none());
    }

    #[test]
    fn test_file_name() {
        assert_eq!(
            VirtualPathBuf::parse("a/b/c").unwrap().file_name(),
            Some("c")
        );
        assert_eq!(VirtualPathBuf::parse("a").unwrap().file_name(), Some("a"));
        assert_eq!(VirtualPathBuf::root().file_name(), None);
    }

    #[test]
    fn test_equality_hash() {
        let a = VirtualPathBuf::parse("a/b").unwrap();
        let b = VirtualPathBuf::parse("a//b").unwrap();
        assert_eq!(a, b);

        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        let mut ha = DefaultHasher::new();
        let mut hb = DefaultHasher::new();
        a.hash(&mut ha);
        b.hash(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
    }

    #[test]
    fn test_canonical_form() {
        // Same logical path always produces same canonical form
        let paths = ["a/b", "a//b", "a/./b", "a/b/../c/../b"];
        let parsed: Vec<_> = paths
            .iter()
            .filter_map(|p| VirtualPathBuf::parse(*p).ok())
            .collect();
        for p in &parsed {
            assert_eq!(p.as_str(), "a/b");
        }

        // Paths that normalize to different forms
        assert_eq!(VirtualPathBuf::parse("a/b/..").unwrap().as_str(), "a");
    }
}
