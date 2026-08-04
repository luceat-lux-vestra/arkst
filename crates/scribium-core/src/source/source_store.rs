//! Source store for VirtualProject.
//!
//! Provides in-memory storage for source files with efficient lookup.
//! Uses existing SourceId for source identification.

use crate::source::span::SourceId;
use crate::source::virtual_path::VirtualPathBuf;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Errors from inserting into a `SourceStore`.
///
/// Contains storage-level errors (duplicate path, exhausted ID space) that are
/// distinct from path parsing/validation errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceStoreError {
    #[error("duplicate source: {0}")]
    DuplicateSource(VirtualPathBuf),
    #[error("source id space exhausted")]
    SourceIdExhausted,
}

/// In-memory store for source files.
///
/// Maps `VirtualPathBuf` to source strings with associated `SourceId`.
/// Uses `BTreeMap` for deterministic iteration order.
#[derive(Debug, Clone)]
pub struct SourceStore {
    by_path: BTreeMap<VirtualPathBuf, SourceEntry>,
    by_id: Vec<Option<SourceEntry>>,
    next_id: u32,
}

impl Default for SourceStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SourceEntry {
    source: Arc<str>,
    id: SourceId,
}

impl SourceStore {
    /// Creates a new empty source store.
    pub fn new() -> Self {
        Self {
            by_path: BTreeMap::new(),
            by_id: Vec::new(),
            next_id: 1,
        }
    }

    /// Adds a source to the store, assigning a new SourceId.
    ///
    /// Returns the assigned SourceId.
    /// Errors if the path already exists or the ID space is exhausted.
    pub fn insert(
        &mut self,
        path: VirtualPathBuf,
        source: String,
    ) -> Result<SourceId, SourceStoreError> {
        if self.by_path.contains_key(&path) {
            return Err(SourceStoreError::DuplicateSource(path));
        }

        let id = self.allocate_id()?;

        let entry = SourceEntry {
            source: Arc::from(source),
            id,
        };
        self.by_path.insert(path, entry.clone());
        self.by_id[id.0 as usize] = Some(entry);

        Ok(id)
    }

    /// Inserts or updates a source in the store.
    ///
    /// If the path already exists, updates the content while preserving the FileId.
    /// If the path doesn't exist, inserts a new source with a new FileId.
    ///
    /// Returns `None` when the ID space is exhausted.
    pub fn upsert(&mut self, path: VirtualPathBuf, source: String) -> Option<SourceId> {
        if let Some(entry) = self.by_path.get_mut(&path) {
            // Update existing source, preserve ID
            entry.source = Arc::from(source);
            Some(entry.id)
        } else {
            // Insert new source
            let id = self.allocate_id().ok()?;

            let entry = SourceEntry {
                source: Arc::from(source),
                id,
            };
            self.by_path.insert(path, entry.clone());
            self.by_id[id.0 as usize] = Some(entry);
            Some(id)
        }
    }

    fn allocate_id(&mut self) -> Result<SourceId, SourceStoreError> {
        if self.next_id == 0 {
            // The ID space is exhausted: next_id wrapped around past the u32 max.
            return Err(SourceStoreError::SourceIdExhausted);
        }
        let id = SourceId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);

        // Ensure by_id vec has space
        while self.by_id.len() <= id.0 as usize {
            self.by_id.push(None);
        }
        Ok(id)
    }

    /// Gets a source by virtual path.
    pub fn get(&self, path: &VirtualPathBuf) -> Option<&str> {
        self.by_path.get(path).map(|e| &*e.source)
    }

    /// Gets a source by SourceId.
    pub fn get_by_id(&self, id: SourceId) -> Option<&str> {
        self.by_id
            .get(id.0 as usize)
            .and_then(|e| e.as_ref().map(|e| &*e.source))
    }

    /// Gets the SourceId for a path.
    pub fn get_id(&self, path: &VirtualPathBuf) -> Option<SourceId> {
        self.by_path.get(path).map(|e| e.id)
    }

    /// Checks if a source exists at the given path.
    pub fn contains(&self, path: &VirtualPathBuf) -> bool {
        self.by_path.contains_key(path)
    }

    /// Returns an iterator over all sources in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (&VirtualPathBuf, &str, SourceId)> {
        self.by_path
            .iter()
            .map(|(path, entry)| (path, &*entry.source, entry.id))
    }

    /// Returns the number of sources.
    pub fn len(&self) -> usize {
        self.by_path.len()
    }

    /// Checks if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    /// Removes a source by path.
    pub fn remove(&mut self, path: &VirtualPathBuf) -> bool {
        if let Some(entry) = self.by_path.remove(path) {
            self.by_id[entry.id.0 as usize] = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::virtual_path::VirtualPathBuf;

    fn path(s: &str) -> VirtualPathBuf {
        VirtualPathBuf::parse(s).unwrap()
    }

    // 1. new()와 default()에서 동일한 최초 SourceId
    #[test]
    fn new_and_default_assign_same_first_id() {
        let mut from_new = SourceStore::new();
        let mut from_default = SourceStore::default();

        let p1 = path("a.qd");
        let id1 = from_new.insert(p1, "content".to_string()).unwrap();
        let p2 = path("b.qd");
        let id2 = from_default.insert(p2, "content".to_string()).unwrap();

        assert_eq!(id1, id2, "new() and default() must start with the same ID");
    }

    // 11. ID 0 정책 검증
    #[test]
    fn first_id_is_one_not_zero() {
        let mut store = SourceStore::new();
        let id = store.insert(path("a.qd"), "content".to_string()).unwrap();
        assert_eq!(id.0, 1, "first SourceId must be 1, never 0");
    }

    // 2. 여러 source 삽입 시 ID가 순차적
    #[test]
    fn ids_are_assigned_sequentially() {
        let mut store = SourceStore::new();
        for i in 1..=5 {
            let p = path(&format!("file_{}.qd", i));
            let id = store.insert(p, format!("content {}", i)).unwrap();
            assert_eq!(id.0, i as u32, "IDs must be sequential in insertion order");
        }
    }

    // 3. path → ID → source round-trip
    #[test]
    fn path_to_id_to_source_round_trip() {
        let mut store = SourceStore::new();
        let p = path("a/b.qd");
        let id = store.insert(p.clone(), "content".to_string()).unwrap();

        let id_by_path = store.get_id(&p).unwrap();
        assert_eq!(id_by_path, id);
        let source_by_id = store.get_by_id(id).unwrap();
        assert_eq!(source_by_id, "content");
    }

    // 4. ID → source 조회
    #[test]
    fn lookup_source_by_id() {
        let mut store = SourceStore::new();
        let p = path("a/b.qd");
        let id = store.insert(p, "content".to_string()).unwrap();
        assert_eq!(store.get_by_id(id), Some("content"));
    }

    // 5. duplicate path 거부
    #[test]
    fn duplicate_path_is_rejected() {
        let mut store = SourceStore::new();
        let p = path("a/b.qd");
        store.insert(p.clone(), "first".to_string()).unwrap();

        let err = store.insert(p.clone(), "second".to_string()).unwrap_err();
        assert_eq!(err, SourceStoreError::DuplicateSource(p));
    }

    // 6. remove 후 path 조회 실패
    #[test]
    fn removed_path_is_not_found() {
        let mut store = SourceStore::new();
        let p = path("a/b.qd");
        store.insert(p.clone(), "content".to_string()).unwrap();

        assert!(store.remove(&p));
        assert_eq!(store.get(&p), None);
        assert!(!store.contains(&p));
    }

    // 7. remove 후 ID 조회 실패
    #[test]
    fn removed_id_is_not_found() {
        let mut store = SourceStore::new();
        let p = path("a/b.qd");
        let id = store.insert(p.clone(), "content".to_string()).unwrap();

        store.remove(&p);
        assert_eq!(store.get_by_id(id), None);
    }

    // 8. remove 후 다른 source index가 손상되지 않음
    #[test]
    fn remove_does_not_corrupt_other_entries() {
        let mut store = SourceStore::new();
        let p1 = path("a.qd");
        let p2 = path("b.qd");
        let p3 = path("c.qd");
        let id1 = store.insert(p1.clone(), "one".to_string()).unwrap();
        let id2 = store.insert(p2.clone(), "two".to_string()).unwrap();
        let id3 = store.insert(p3.clone(), "three".to_string()).unwrap();

        store.remove(&p2);

        assert_eq!(store.get_by_id(id1), Some("one"));
        assert_eq!(store.get_by_id(id2), None);
        assert_eq!(store.get_by_id(id3), Some("three"));
        assert_eq!(store.get(&p1), Some("one"));
        assert_eq!(store.get(&p3), Some("three"));
    }

    // 9. deterministic iteration
    #[test]
    fn iteration_is_deterministic_by_path() {
        let mut store = SourceStore::new();
        store.insert(path("z.qd"), "z".to_string()).unwrap();
        store.insert(path("a.qd"), "a".to_string()).unwrap();
        store.insert(path("m.qd"), "m".to_string()).unwrap();

        let collected: Vec<(&VirtualPathBuf, &str, SourceId)> = store.iter().collect();
        let paths: Vec<&str> = collected.iter().map(|(p, _, _)| p.as_str()).collect();
        assert_eq!(paths, vec!["a.qd", "m.qd", "z.qd"]);
    }

    // 10. clone 후 ID/path/source 보존
    #[test]
    fn clone_preserves_ids_paths_and_sources() {
        let mut store = SourceStore::new();
        let p = path("a/b.qd");
        let id = store.insert(p.clone(), "content".to_string()).unwrap();

        let cloned = store.clone();
        assert_eq!(cloned.get(&p), Some("content"));
        assert_eq!(cloned.get_id(&p), Some(id));
        assert_eq!(cloned.get_by_id(id), Some("content"));
    }

    // 12. ID overflow 처리 내부 테스트
    #[test]
    fn id_space_exhausted_is_detected() {
        let mut store = SourceStore::new();
        store.next_id = 0; // simulate wrap-around / exhaustion
        let p = path("overflow.qd");

        let err = store.insert(p, "content".to_string()).unwrap_err();
        assert_eq!(err, SourceStoreError::SourceIdExhausted);
    }
}
