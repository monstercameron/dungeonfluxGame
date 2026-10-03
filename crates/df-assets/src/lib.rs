//! Verified native byte publication. Metadata visibility belongs to the caller's metadata store.

mod publication;
pub use publication::{
    AssetManifest, AssetMetadataStore, AssetStore, DurableObject, MetadataFailure, Publication,
    PublicationError, PublicationReceipt, PublicationStatus, PublishedBinding, StoreError, publish,
};

#[cfg(not(target_arch = "wasm32"))]
mod native_file_store;
#[cfg(not(target_arch = "wasm32"))]
pub use native_file_store::{NativeFileStore, StagedUpload};

mod range;
pub use range::{
    AccessFailure, AssetReadAuthority, AssetReadStore, AuthorizedBinding, AuthorizedRange,
    ByteRange, ChunkOutcome, MAX_CHUNK_BYTES, RangeError, open_range,
};
