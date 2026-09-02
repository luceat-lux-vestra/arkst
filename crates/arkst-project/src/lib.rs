//! I/O-free, deterministic in-memory project model for Arkst.
//!
//! Native hosts load files and assets and construct a [`VirtualProject`].
//! This crate does not perform filesystem, network, or process I/O and is
//! suitable for platform-neutral and WASM consumers.

pub mod asset_store;
pub mod resource;
pub mod source_store;
pub mod virtual_path;
pub mod virtual_project;

pub use asset_store::*;
pub use resource::*;
pub use source_store::*;
pub use virtual_path::*;
pub use virtual_project::*;
