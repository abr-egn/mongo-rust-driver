//! FFI type definitions for the MongoDB Rust driver.
//!
//! This module contains C-compatible type definitions for passing data across the FFI boundary.

use std::os::raw::c_char;

/// Connection settings for creating a MongoDB client.
///
/// All string fields are null-terminated C strings. Nullable fields can be null pointers.
/// Integer fields use -1 to indicate "not set" (use default).
#[repr(C)]
pub struct ConnectionSettings {
    /// Hosts as comma-separated "host:port" pairs (null-terminated)
    pub hosts: *const c_char,
    /// Application name (null-terminated, nullable)
    pub app_name: *const c_char,
    /// Compressors as comma-separated list (null-terminated, nullable)
    pub compressors: *const c_char,
    /// Direct connection flag
    pub direct_connection: bool,
    /// Load balanced flag
    pub load_balanced: bool,
    /// Max pool size (-1 = not set)
    pub max_pool_size: i32,
    /// Min pool size (-1 = not set)
    pub min_pool_size: i32,
    /// Max idle time in milliseconds (-1 = not set)
    pub max_idle_time_ms: i64,
    /// Connect timeout in milliseconds (-1 = not set)
    pub connect_timeout_ms: i64,
    /// Socket timeout in milliseconds (-1 = not set)
    pub socket_timeout_ms: i64,
    /// Server selection timeout in milliseconds (-1 = not set)
    pub server_selection_timeout_ms: i64,
    /// Local threshold in milliseconds (-1 = not set)
    pub local_threshold_ms: i64,
    /// Heartbeat frequency in milliseconds (-1 = not set)
    pub heartbeat_frequency_ms: i64,
    /// Replica set name (null-terminated, nullable)
    pub replica_set: *const c_char,
    /// Read preference mode (0=primary, 1=primaryPreferred, etc.)
    pub read_preference_mode: u8,
    /// SRV service name (null-terminated, nullable)
    pub srv_service_name: *const c_char,
    /// SRV max hosts (-1 = not set)
    pub srv_max_hosts: i32,
}

/// Authentication settings for MongoDB client.
///
/// All fields are nullable (can be null pointers).
#[repr(C)]
pub struct AuthSettings {
    /// Auth mechanism (null-terminated, nullable: "SCRAM-SHA-1", "SCRAM-SHA-256", etc.)
    pub mechanism: *const c_char,
    /// Username (null-terminated, nullable)
    pub username: *const c_char,
    /// Password (null-terminated, nullable)
    pub password: *const c_char,
    /// Auth source database (null-terminated, nullable)
    pub source: *const c_char,
}

/// TLS settings for MongoDB client.
#[repr(C)]
pub struct TlsSettings {
    /// Enable TLS
    pub enabled: bool,
    /// Allow invalid certificates
    pub allow_invalid_certificates: bool,
    /// Allow invalid hostnames
    pub allow_invalid_hostnames: bool,
    /// CA file path (null-terminated, nullable)
    pub ca_file: *const c_char,
    /// Certificate file path (null-terminated, nullable)
    pub cert_file: *const c_char,
    /// Certificate key file path (null-terminated, nullable)
    pub cert_key_file: *const c_char,
}

// Re-export Rust types to use as opaque pointers in FFI
// C code will only see these as opaque pointers and cannot access their internals
pub use crate::{
    change_stream::ChangeStream,
    client::session::ClientSession as Session,
    options::{ReadConcern, ReadPreference, WriteConcern},
    Cursor,
};

/// Raw BSON document.
#[repr(C)]
pub struct Bson {
    /// Pointer to the raw BSON data
    pub data: *const u8,
    /// Length of the BSON data in bytes
    pub len: usize,
}

/// Owned BSON document - frees memory on drop.
#[repr(transparent)]
pub struct OwnedBson(pub Bson);

impl OwnedBson {
    /// Create an empty value (null pointer, zero length).
    pub(super) fn empty() -> Self {
        Self(Bson {
            data: std::ptr::null(),
            len: 0,
        })
    }

    /// Create from a Rust Document by serializing to raw bytes.
    pub(super) fn from_doc(doc: &crate::bson::Document) -> Self {
        let mut bytes = Vec::new();
        doc.to_writer(&mut bytes)
            .expect("Document encoding should not fail");
        let boxed = bytes.into_boxed_slice();
        let len = boxed.len();
        let ptr = Box::into_raw(boxed) as *const u8;
        Self(Bson { data: ptr, len })
    }

    /// Create from a Rust RawDocument.
    pub(super) fn from_raw(doc: &crate::bson::RawDocument) -> Self {
        let bytes = doc.as_bytes();
        let boxed: Box<[u8]> = bytes.to_vec().into_boxed_slice();
        let len = boxed.len();
        let ptr = Box::into_raw(boxed) as *const u8;
        Self(Bson { data: ptr, len })
    }
}

impl Drop for OwnedBson {
    fn drop(&mut self) {
        let Bson { data, len } = &self.0;
        if !data.is_null() && *len > 0 {
            unsafe {
                let _ = Vec::from_raw_parts(*data as *mut u8, *len, *len);
            }
        }
    }
}

/// Any BSON value with explicit type byte.
#[repr(C)]
pub struct BsonValue {
    /// Pointer to the raw BSON value data
    pub data: *const u8,
    /// Length of the BSON value data in bytes
    pub len: usize,
    /// BSON type: 0x01=double, 0x02=string, 0x07=objectid, etc.
    pub bson_type: u8,
}

/// Owned BSON value - frees memory on drop.
#[repr(transparent)]
pub struct OwnedBsonValue(pub BsonValue);

impl Drop for OwnedBsonValue {
    fn drop(&mut self) {
        let BsonValue {
            data,
            len,
            bson_type: _,
        } = &self.0;
        if !data.is_null() && *len > 0 {
            unsafe {
                let _ = Vec::from_raw_parts(*data as *mut u8, *len, *len);
            }
        }
    }
}
