//! Asset store for VirtualProject.
//!
//! Provides in-memory storage for binary assets (fonts, images, etc.).

use crate::source::virtual_path::VirtualPathBuf;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Errors from inserting into an `AssetStore`.
///
/// Contains storage-level errors (duplicate path) distinct from path parsing
/// and validation errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssetStoreError {
    #[error("duplicate asset: {0}")]
    DuplicateAsset(VirtualPathBuf),
}

/// In-memory store for binary assets (fonts, images, etc.).
///
/// Uses `BTreeMap` for deterministic iteration order.
#[derive(Debug, Clone, Default)]
pub struct AssetStore {
    assets: BTreeMap<VirtualPathBuf, Arc<[u8]>>,
}

impl AssetStore {
    /// Creates a new empty asset store.
    pub fn new() -> Self {
        Self {
            assets: BTreeMap::new(),
        }
    }

    /// Inserts an asset into the store.
    ///
    /// Returns an error if the path already exists.
    pub fn insert(&mut self, path: VirtualPathBuf, data: Arc<[u8]>) -> Result<(), AssetStoreError> {
        if self.assets.contains_key(&path) {
            return Err(AssetStoreError::DuplicateAsset(path));
        }
        self.assets.insert(path, data);
        Ok(())
    }

    /// Inserts or updates an asset in the store.
    ///
    /// If the path already exists, updates the data.
    /// If the path doesn't exist, inserts a new asset.
    pub fn upsert(&mut self, path: VirtualPathBuf, data: Arc<[u8]>) {
        self.assets.insert(path, data);
    }

    /// Gets an asset by virtual path (borrowed slice).
    pub fn get(&self, path: &VirtualPathBuf) -> Option<&[u8]> {
        self.assets.get(path).map(|arc| arc.as_ref())
    }

    /// Gets an asset by virtual path (owned Arc).
    pub fn get_owned(&self, path: &VirtualPathBuf) -> Option<Arc<[u8]>> {
        self.assets.get(path).cloned()
    }

    /// Checks if an asset exists.
    pub fn contains(&self, path: &VirtualPathBuf) -> bool {
        self.assets.contains_key(path)
    }

    /// Removes an asset.
    pub fn remove(&mut self, path: &VirtualPathBuf) -> bool {
        self.assets.remove(path).is_some()
    }

    /// Returns the number of assets.
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Checks if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Returns an iterator over all assets.
    pub fn iter(&self) -> impl Iterator<Item = (&VirtualPathBuf, &[u8])> {
        self.assets.iter().map(|(k, v)| (k, v.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::virtual_path::VirtualPathBuf;

    fn path(s: &str) -> VirtualPathBuf {
        VirtualPathBuf::parse(s).unwrap()
    }

    // 1. insert/get
    #[test]
    fn insert_and_get() {
        let mut store = AssetStore::new();
        let p = path("font.ttf");
        let data = Arc::from(b"font data".as_ref());

        store.insert(p.clone(), data).unwrap();

        assert_eq!(store.get(&p), Some(b"font data".as_ref()));
        assert!(store.contains(&p));
    }

    // 2. get_owned는 Arc 공유
    #[test]
    fn get_owned_shares_arc() {
        let mut store = AssetStore::new();
        let p = path("font.ttf");
        let data: Arc<[u8]> = Arc::from(b"font data".as_ref());

        store.insert(p.clone(), data.clone()).unwrap();

        let owned = store.get_owned(&p).unwrap();
        assert!(
            Arc::ptr_eq(&data, &owned),
            "get_owned must share the same Arc"
        );
    }

    // 3. duplicate path 거부
    #[test]
    fn duplicate_path_is_rejected() {
        let mut store = AssetStore::new();
        let p = path("font.ttf");
        store.insert(p.clone(), Arc::from(b"v1".as_ref())).unwrap();

        let err = store
            .insert(p.clone(), Arc::from(b"v2".as_ref()))
            .unwrap_err();
        assert_eq!(err, AssetStoreError::DuplicateAsset(p));
    }

    // 4. remove
    #[test]
    fn remove_asset() {
        let mut store = AssetStore::new();
        let p = path("font.ttf");
        store
            .insert(p.clone(), Arc::from(b"data".as_ref()))
            .unwrap();

        assert!(store.remove(&p));
        assert!(!store.remove(&p)); // Already removed
        assert_eq!(store.get(&p), None);
    }

    // 5. deterministic iteration
    #[test]
    fn iteration_is_deterministic_by_path() {
        let mut store = AssetStore::new();
        store
            .insert(path("z.ttf"), Arc::from(b"z".as_ref()))
            .unwrap();
        store
            .insert(path("a.ttf"), Arc::from(b"a".as_ref()))
            .unwrap();
        store
            .insert(path("m.ttf"), Arc::from(b"m".as_ref()))
            .unwrap();

        let paths: Vec<&str> = store.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths, vec!["a.ttf", "m.ttf", "z.ttf"]);
    }

    // 6. clone 후 데이터 보존
    #[test]
    fn clone_preserves_data() {
        let mut store = AssetStore::new();
        let p = path("font.ttf");
        let data = Arc::from(b"font data".as_ref());
        store.insert(p.clone(), data).unwrap();

        let cloned = store.clone();
        assert_eq!(cloned.get(&p), Some(b"font data".as_ref()));
    }
}
