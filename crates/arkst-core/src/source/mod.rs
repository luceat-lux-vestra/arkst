pub mod span;

// Compatibility facade: implementation ownership lives in arkst-project.
pub use arkst_project::{asset_store, resource, source_store, virtual_path};
pub use arkst_project::{
    AssetStore, AssetStoreError, ResourceAccessError, ResourceReference, SourceStore,
    SourceStoreError, VirtualPathBuf, VirtualPathError,
};
pub use span::*;
