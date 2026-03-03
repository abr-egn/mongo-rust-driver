//! FFI (Foreign Function Interface) layer for the MongoDB Rust driver.
//!
//! This module provides a C-compatible API for using the MongoDB driver from other languages.
//! The design follows these principles:
//!
//! 1. **Options as FFI structs, not BSON** - Type-safe contracts, no serialization overhead
//! 2. **Documents/filters/pipelines as BSON** - User data stays as raw BSON
//! 3. **Results as FFI structs** - Except embedded documents which stay as BSON
//! 4. **Async with callbacks** - Operations spawn onto Tokio, invoke callback on completion
//! 5. **Opaque session handles** - Sessions are fully managed in Rust, including transaction state
//!
//! See `docs/ffi.md` for the complete design specification.

pub mod client;
pub mod command;
pub mod error;
mod runtime;
pub mod types;
mod utils;

#[cfg(test)]
mod tests;

pub use client::{mongo_client_destroy, mongo_client_new, MongoClient};
pub use command::{mongo_run_command, RunCommandCallback};
pub use error::{error_free, Error};
pub use types::{
    AuthSettings,
    Bson,
    ChangeStream,
    ConnectionSettings,
    Cursor,
    OwnedBson,
    ReadConcern,
    ReadPreference,
    Session,
    TlsSettings,
    WriteConcern,
};
