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
pub mod dispatch;
pub mod error;
pub mod insert;
mod runtime;
pub mod session;
pub mod types;
mod utils;
