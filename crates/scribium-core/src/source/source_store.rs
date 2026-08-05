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
/// - Lookup by `SourceId`: O(1) via `Vec`
/// - Lookup by virtual path: O(log n) via `BTreeMap`
/// - Iteration: deterministic path-ordered via `BTreeMap`
#[derive(Debug, Clone)]
pub struct SourceStore {
    by_path: BTreeMap<VirtualPathBuf, SourceId>,
    by_id: Vec<Option<SourceRecord>>,
    next_id: u32,
}

impl Default for SourceStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct SourceRecord {
    path: VirtualPathBuf,
    source: Arc<str>,
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

    /// Allocates a new SourceId.
    fn allocate_id(&mut self) -> Result<SourceId, SourceStoreError> {
        if self.next_id == 0 {
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

    /// Adds a source to the store, assigning a new SourceId.
    ///
    /// Returns the assigned SourceId.
    /// Errors if the path already exists.
    pub fn insert(
        &mut self,
        path: VirtualPathBuf,
        source: String,
    ) -> Result<SourceId, SourceStoreError> {
        if self.by_path.contains_key(&path) {
            return Err(SourceStoreError::DuplicateSource(path));
        }

        let id = self.allocate_id()?;

        let record = SourceRecord {
            path: path.clone(),
            source: Arc::from(source),
        };

        self.by_path.insert(path, id);
        self.by_id[id.0 as usize] = Some(record);

        Ok(id)
    }

    /// Inserts or updates a source in the store.
    ///
    /// If the path already exists, updates the content while preserving the SourceId.
    /// If the path doesn't exist, inserts a new source with a new SourceId.
    pub fn upsert(
        &mut self,
        path: VirtualPathBuf,
        source: String,
    ) -> Result<SourceId, SourceStoreError> {
        if let Some(&id) = self.by_path.get(&path) {
            let record = self
                .record_mut(id)
                .expect("SourceStore indexes must remain consistent");

            record.source = Arc::from(source);
            return Ok(id);
        }

        self.insert(path, source)
    }

    /// Gets a source by virtual path.
    pub fn get(&self, path: &VirtualPathBuf) -> Option<&str> {
        let id = *self.by_path.get(path)?;
        self.get_by_id(id)
    }

    /// Gets a source by SourceId.
    pub fn get_by_id(&self, id: SourceId) -> Option<&str> {
        self.record(id).map(|record| record.source.as_ref())
    }

    /// Gets the SourceId for a path.
    pub fn get_id(&self, path: &VirtualPathBuf) -> Option<SourceId> {
        self.by_path.get(path).copied()
    }

    /// Gets the path for a SourceId.
    pub fn path_by_id(&self, id: SourceId) -> Option<&VirtualPathBuf> {
        self.record(id).map(|record| &record.path)
    }

    /// Gets a source and its SourceId by virtual path.
    pub fn get_with_id(&self, path: &VirtualPathBuf) -> Option<(&str, SourceId)> {
        let id = *self.by_path.get(path)?;
        Some((self.get_by_id(id)?, id))
    }

    /// Checks if a source exists at the given path.
    pub fn contains(&self, path: &VirtualPathBuf) -> bool {
        self.by_path.contains_key(path)
    }

    /// Returns an iterator over all sources in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (SourceId, &VirtualPathBuf, &str)> {
        self.by_path.iter().map(|(path, &id)| {
            let source = self
                .record(id)
                .expect("SourceStore indexes must remain consistent")
                .source
                .as_ref();
            (id, path, source)
        })
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
        let Some(id) = self.by_path.remove(path) else {
            return false;
        };

        if let Some(slot) = self.by_id.get_mut(id.0 as usize) {
            *slot = None;
        }

        true
    }

    /// Internal access to a record by SourceId.
    fn record(&self, id: SourceId) -> Option<&SourceRecord> {
        self.by_id.get(id.0 as usize)?.as_ref()
    }

    /// Internal mutable access to a record by SourceId.
    fn record_mut(&mut self, id: SourceId) -> Option<&mut SourceRecord> {
        self.by_id.get_mut(id.0 as usize)?.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::virtual_path::VirtualPathBuf;

    #[test]
    fn insert_and_get_by_path() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("a/b.qd").unwrap();
        let id = store.insert(path.clone(), "content".to_string()).unwrap();

        assert_eq!(store.get(&path), Some("content"));
        assert_eq!(store.get_by_id(id), Some("content"));
        assert_eq!(store.get_id(&path), Some(id));
    }

    #[test]
    fn insert_and_get_by_id() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("a/b.qd").unwrap();
        let id = store.insert(path.clone(), "content".to_string()).unwrap();

        let source = store.get_by_id(id).unwrap();
        assert_eq!(source, "content");
    }

    #[test]
    fn upsert_updates_path_and_id_views() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("main.qd").unwrap();

        let original_id = store.insert(path.clone(), "before".to_string()).unwrap();

        let updated_id = store.upsert(path.clone(), "after".to_string()).unwrap();

        assert_eq!(updated_id, original_id);
        assert_eq!(store.get(&path), Some("after"));
        assert_eq!(store.get_by_id(original_id), Some("after"));
        assert_eq!(store.path_by_id(original_id), Some(&path));
    }

    #[test]
    fn upsert_new_path_allocates_id() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("new.qd").unwrap();

        let id = store.upsert(path.clone(), "content".to_string()).unwrap();

        assert_eq!(store.get(&path), Some("content"));
        assert_eq!(store.get_by_id(id), Some("content"));
    }

    #[test]
    fn upsert_existing_path_preserves_id() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("a.qd").unwrap();

        let id1 = store.insert(path.clone(), "first".to_string()).unwrap();
        let id2 = store.upsert(path.clone(), "second".to_string()).unwrap();

        assert_eq!(id1, id2);
        assert_eq!(store.get(&path), Some("second"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn upsert_existing_path_does_not_increase_len() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("a.qd").unwrap();

        store.insert(path.clone(), "first".to_string()).unwrap();
        assert_eq!(store.len(), 1);

        store.upsert(path.clone(), "second".to_string()).unwrap();
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn remove_clears_path_and_id_indexes() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("a.qd").unwrap();
        let id = store.insert(path.clone(), "content".to_string()).unwrap();

        assert!(store.remove(&path));
        assert!(!store.remove(&path)); // Already removed
        assert_eq!(store.get(&path), None);
        assert_eq!(store.get_by_id(id), None);
        assert_eq!(store.path_by_id(id), None);
    }

    #[test]
    fn clone_after_upsert_preserves_updated_content() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("a.qd").unwrap();

        store.insert(path.clone(), "before".to_string()).unwrap();
        let cloned = store.clone();

        store.upsert(path.clone(), "after".to_string()).unwrap();

        assert_eq!(cloned.get(&path), Some("before"));
        assert_eq!(store.get(&path), Some("after"));
    }

    #[test]
    fn removing_source_removes_both_indexes() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("a/b.qd").unwrap();
        let id = store.insert(path.clone(), "content".to_string()).unwrap();

        assert!(store.remove(&path));
        assert!(!store.remove(&path)); // Already removed
        assert_eq!(store.get(&path), None);
        assert_eq!(store.get_by_id(id), None);
    }

    #[test]
    fn different_sources_receive_different_ids() {
        let mut store = SourceStore::new();
        let path1 = VirtualPathBuf::parse("a/b.qd").unwrap();
        let path2 = VirtualPathBuf::parse("c/d.qd").unwrap();

        let id1 = store.insert(path1, "first".to_string()).unwrap();
        let id2 = store.insert(path2, "second".to_string()).unwrap();

        assert_ne!(id1, id2);
    }

    #[test]
    fn removed_id_is_not_reused() {
        let mut store = SourceStore::new();
        let path = VirtualPathBuf::parse("a/b.qd").unwrap();
        let id1 = store.insert(path.clone(), "first".to_string()).unwrap();

        store.remove(&path);

        let path2 = VirtualPathBuf::parse("c/d.qd").unwrap();
        let id2 = store.insert(path2, "second".to_string()).unwrap();

        assert_ne!(id1, id2, "Removed ID should not be reused");
    }

    #[test]
    fn file_id_lookup_does_not_scan_sources() {
        let mut store = SourceStore::new();
        // Insert many sources
        for i in 0..100 {
            let path = VirtualPathBuf::parse(format!("file_{}.qd", i)).unwrap();
            store.insert(path, format!("content {}", i)).unwrap();
        }

        // Get ID by path - should be O(log n) via BTreeMap
        let path = VirtualPathBuf::parse("file_50.qd").unwrap();
        let id = store.get_id(&path).unwrap();

        // Get by ID - should be O(1) via Vec
        let source = store.get_by_id(id).unwrap();
        assert_eq!(source, "content 50");
    }

    #[test]
    fn store_consistency_after_multiple_operations() {
        let mut store = SourceStore::new();

        // Insert several sources
        for i in 0..10 {
            let path = VirtualPathBuf::parse(format!("file_{}.qd", i)).unwrap();
            store.insert(path, format!("content {}", i)).unwrap();
        }

        // Update some
        for i in 0..5 {
            let path = VirtualPathBuf::parse(format!("file_{}.qd", i)).unwrap();
            store.upsert(path, format!("updated {}", i)).unwrap();
        }

        // Remove some
        store.remove(&VirtualPathBuf::parse("file_2.qd").unwrap());
        store.remove(&VirtualPathBuf::parse("file_7.qd").unwrap());

        // Verify consistency
        for (id, path, source) in store.iter() {
            assert_eq!(store.get(path), Some(source));
            assert_eq!(store.get_by_id(id), Some(source));
            assert_eq!(store.get_id(path), Some(id));
            assert_eq!(store.path_by_id(id), Some(path));
        }
    }
}
