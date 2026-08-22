pub mod span;

// Compatibility facade: implementation ownership lives in scribium-project.
pub use scribium_project::{asset_store, resource, source_store, virtual_path};
pub use scribium_project::{
    AssetStore, AssetStoreError, ResourceAccessError, ResourceReference, SourceStore,
    SourceStoreError, VirtualPathBuf, VirtualPathError,
};
pub use span::*;
