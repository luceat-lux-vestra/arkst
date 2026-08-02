use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

static NEXT_SOURCE_ID: AtomicU32 = AtomicU32::new(1);

/// Generates unique source IDs.
pub fn fresh_source_id() -> super::SourceId {
    super::SourceId(NEXT_SOURCE_ID.fetch_add(1, Ordering::Relaxed))
}
