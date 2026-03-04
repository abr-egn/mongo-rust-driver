//! FFI type definitions for the MongoDB Rust driver.
//!
//! This module contains C-compatible type definitions for passing data across the FFI boundary.

#[cfg(test)]
mod tests;

use std::os::raw::c_char;

pub use crate::{
    concern::{ReadConcern, WriteConcern},
    options::ReadPreference,
    ClientSession,
};

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

/*
impl Deref for Session {
    type Target = crate::client::session::ClientSession;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
    */

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
/// Options for configuring read preference behavior.
#[repr(C)]
pub struct ReadPreferenceOptions {
    /// Tag sets as BSON array wrapped in doc, nullable. Example: [{"dc": "east"}, {"dc": "west"}]
    pub tags: *const Bson,

    /// Max staleness in seconds. -1 = not set
    pub max_staleness_seconds: i64,

    /// Hedge options as BSON document, nullable. Example: {"enabled": true}
    pub hedge: *const Bson,
}

/// Read preference mode constants.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadPreferenceType {
    /// Read from the primary only.
    Primary = 0,
    /// Read from the primary if available, otherwise a secondary.
    PrimaryPreferred = 1,
    /// Read from a secondary only.
    Secondary = 2,
    /// Read from a secondary if available, otherwise the primary.
    SecondaryPreferred = 3,
    /// Read from the nearest member.
    Nearest = 4,
}

/// Create a read preference. Returns handle (non-null), or null on error.
///
/// # Safety
///
/// - `options` can be null for Primary mode, but must be valid for other modes.
/// - If `options` is non-null, any BSON pointers in it must point to valid BSON data.
#[no_mangle]
pub unsafe extern "C" fn mongo_read_preference_create(
    // Mode: 0=primary, 1=primaryPreferred, 2=secondary, 3=secondaryPreferred, 4=nearest
    mode: u8,
    // May be null for mode=0 (Primary)
    options: *const ReadPreferenceOptions,
) -> *mut ReadPreference {
    if mode > ReadPreferenceType::Nearest as u8 {
        return std::ptr::null_mut();
    }

    let mode: ReadPreferenceType = std::mem::transmute(mode);
    if mode == ReadPreferenceType::Primary {
        return Box::into_raw(Box::new(crate::options::ReadPreference::Primary));
    }

    // For non-Primary modes, parse options
    let rust_options = if options.is_null() {
        None
    } else {
        match parse_read_preference_options(&*options) {
            Ok(opts) => opts,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let read_pref = match mode {
        ReadPreferenceType::Primary => unreachable!(),
        ReadPreferenceType::PrimaryPreferred => crate::options::ReadPreference::PrimaryPreferred {
            options: rust_options,
        },
        ReadPreferenceType::Secondary => crate::options::ReadPreference::Secondary {
            options: rust_options,
        },
        ReadPreferenceType::SecondaryPreferred => {
            crate::options::ReadPreference::SecondaryPreferred {
                options: rust_options,
            }
        }
        ReadPreferenceType::Nearest => crate::options::ReadPreference::Nearest {
            options: rust_options,
        },
    };

    Box::into_raw(Box::new(read_pref))
}

/// Parse FFI ReadPreferenceOptions into Rust ReadPreferenceOptions.
unsafe fn parse_read_preference_options(
    options: &ReadPreferenceOptions,
) -> Result<Option<crate::options::ReadPreferenceOptions>, ()> {
    use crate::bson::RawDocumentBuf;
    use std::{collections::HashMap, time::Duration};

    let mut rust_options = crate::options::ReadPreferenceOptions::default();
    let mut has_any = false;

    // Parse tag_sets from BSON array
    if !options.tags.is_null() {
        let bson = &*options.tags;
        let bytes = std::slice::from_raw_parts(bson.data, bson.len);
        let doc = RawDocumentBuf::from_bytes(bytes.to_vec()).map_err(|_| ())?;

        // The BSON array is wrapped in a document, e.g. {"": [...]}
        // Get the array from the first field
        let mut tag_sets: Vec<HashMap<String, String>> = Vec::new();
        if let Some(arr) = doc
            .iter()
            .next()
            .and_then(|r| r.ok())
            .and_then(|(_, v)| v.as_array())
        {
            for item in arr {
                let item = item.map_err(|_| ())?;
                if let Some(tag_doc) = item.as_document() {
                    let mut tag_set: HashMap<String, String> = HashMap::new();
                    for field in tag_doc {
                        let (key, value) = field.map_err(|_| ())?;
                        if let Some(s) = value.as_str() {
                            tag_set.insert(key.to_string(), s.to_string());
                        }
                    }
                    tag_sets.push(tag_set);
                }
            }
        }

        if !tag_sets.is_empty() {
            rust_options.tag_sets = Some(tag_sets);
            has_any = true;
        }
    }

    // Parse max_staleness
    if options.max_staleness_seconds >= 0 {
        rust_options.max_staleness =
            Some(Duration::from_secs(options.max_staleness_seconds as u64));
        has_any = true;
    }

    // Parse hedge options from BSON document
    if !options.hedge.is_null() {
        let bson = &*options.hedge;
        let bytes = std::slice::from_raw_parts(bson.data, bson.len);
        let doc = RawDocumentBuf::from_bytes(bytes.to_vec()).map_err(|_| ())?;

        if let Some(enabled) = doc.get("enabled").ok().flatten().and_then(|v| v.as_bool()) {
            #[allow(deprecated)]
            {
                rust_options.hedge = Some(crate::options::HedgedReadOptions { enabled });
            }
            has_any = true;
        }
    }

    if has_any {
        Ok(Some(rust_options))
    } else {
        Ok(None)
    }
}

/// Destroy a read preference handle.
///
/// # Safety
///
/// - `handle` must be a valid pointer returned from `mongo_read_preference_create`, or null.
/// - `handle` must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn mongo_read_preference_destroy(handle: *mut ReadPreference) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

/// Options for creating a read concern.
#[repr(C)]
pub struct ReadConcernOptions {
    /// Level: null-terminated string (e.g., "local", "majority", "snapshot", "linearizable")
    pub level: *const c_char,
}

/// Create a read concern. Returns handle (non-null), or null on error.
///
/// # Safety
///
/// - `options` must be a valid pointer to a ReadConcernOptions struct.
/// - `options.level` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn mongo_read_concern_create(
    options: *const ReadConcernOptions,
) -> *mut ReadConcern {
    if options.is_null() {
        return std::ptr::null_mut();
    }

    let options = &*options;

    if options.level.is_null() {
        return std::ptr::null_mut();
    }

    let level_str = match std::ffi::CStr::from_ptr(options.level).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let level = crate::concern::ReadConcernLevel::from_str(level_str);
    let read_concern = crate::concern::ReadConcern::from(level);
    Box::into_raw(Box::new(read_concern))
}

/// Destroy a read concern handle.
///
/// # Safety
///
/// - `handle` must be a valid pointer returned from `mongo_read_concern_create`, or null.
/// - `handle` must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn mongo_read_concern_destroy(handle: *mut ReadConcern) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

/// Options for creating a write concern.
#[repr(C)]
pub struct WriteConcernOptions {
    /// W value. -1 = not set, 0 = unacknowledged, 1+ = w value
    /// Use w_tag for string values like "majority"
    pub w: i32,

    /// W tag (for w:"majority" etc), null-terminated, nullable
    /// If set, w field is ignored
    pub w_tag: *const c_char,

    /// Journal. -1 = not set, 0 = false, 1 = true
    pub journal: i8,

    /// Write timeout in milliseconds. -1 = not set
    pub w_timeout_ms: i64,
}

/// Create a write concern. Returns handle (non-null), or null on error.
///
/// # Safety
///
/// - `options` must be a valid pointer to a WriteConcernOptions struct.
/// - If `options.w_tag` is non-null, it must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn mongo_write_concern_create(
    options: *const WriteConcernOptions,
) -> *mut WriteConcern {
    if options.is_null() {
        return std::ptr::null_mut();
    }

    let options = &*options;

    // Parse w / w_tag - w_tag takes precedence if set
    let w = if !options.w_tag.is_null() {
        let w_tag_str = match std::ffi::CStr::from_ptr(options.w_tag).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };
        Some(crate::concern::Acknowledgment::from(w_tag_str))
    } else if options.w >= 0 {
        Some(crate::concern::Acknowledgment::Nodes(options.w as u32))
    } else {
        None
    };

    // Parse journal
    let journal = if options.journal >= 0 {
        Some(options.journal != 0)
    } else {
        None
    };

    // Parse w_timeout
    let w_timeout = if options.w_timeout_ms >= 0 {
        Some(std::time::Duration::from_millis(
            options.w_timeout_ms as u64,
        ))
    } else {
        None
    };

    let write_concern = crate::concern::WriteConcern {
        w,
        w_timeout,
        journal,
    };

    Box::into_raw(Box::new(write_concern))
}

/// Destroy a write concern handle.
///
/// # Safety
///
/// - `handle` must be a valid pointer returned from `mongo_write_concern_create`, or null.
/// - `handle` must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn mongo_write_concern_destroy(handle: *mut WriteConcern) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

/*
#[repr(C)]
pub struct OperationContext {
    /// Session handle (null = no session)
    pub session_handle: *const Session,

    /// Read preference handle (null = use default/inherit from session)
    pub read_preference_handle: *const ReadPreference,

    /// Write concern handle (null = use default/inherit from session)
    pub write_concern_handle: *const WriteConcern,

    /// Read concern handle (null = use default/inherit from session)
    pub read_concern_handle: *const ReadConcern,

    /// Timeout in milliseconds (CSOT). -1 = not set (use client default)
    pub timeout_ms: i64,
}
*/
