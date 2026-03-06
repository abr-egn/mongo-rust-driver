//! Utility functions for FFI operations.

#[cfg(test)]
mod tests;

use std::{
    ffi::{c_void, CStr},
    os::raw::c_char,
    time::Duration,
};

use crate::{
    client::auth::AuthMechanism,
    error::{Error, Result},
    options::ReadPreference,
};

#[cfg(any(
    feature = "zstd-compression",
    feature = "zlib-compression",
    feature = "snappy-compression"
))]
use crate::options::Compressor;

/// Convert a C string pointer to a Rust String.
///
/// Returns None if the pointer is null.
/// Returns an error if the string is not valid UTF-8.
///
/// # Safety
///
/// The pointer must be valid and point to a null-terminated C string.
pub(super) unsafe fn c_char_to_string(ptr: *const c_char) -> Result<Option<String>> {
    if ptr.is_null() {
        return Ok(None);
    }

    let c_str = CStr::from_ptr(ptr);
    let str_slice = c_str
        .to_str()
        .map_err(|e| Error::invalid_argument(format!("Invalid UTF-8 in C string: {}", e)))?;

    Ok(Some(str_slice.to_string()))
}

/// Convert a C string pointer to a Rust &str.
///
/// Returns None if the pointer is null.
/// Returns an error if the string is not valid UTF-8.
///
/// # Safety
///
/// The pointer must be valid and point to a null-terminated C string.
/// The returned &str borrows from the C string, so the C string must remain valid.
pub(super) unsafe fn c_char_to_str<'a>(ptr: *const c_char) -> Result<Option<&'a str>> {
    if ptr.is_null() {
        return Ok(None);
    }

    let c_str = CStr::from_ptr(ptr);
    let str_slice = c_str
        .to_str()
        .map_err(|e| Error::invalid_argument(format!("Invalid UTF-8 in C string: {}", e)))?;

    Ok(Some(str_slice))
}

/// Convert an i64 milliseconds value to a Duration.
///
/// Returns None if the value is -1 (indicating "not set").
pub(super) fn i64_to_duration_ms(ms: i64) -> Option<Duration> {
    if ms < 0 {
        None
    } else {
        Some(Duration::from_millis(ms as u64))
    }
}

/// Convert an i32 value to an Option<u32>.
///
/// Returns None if the value is -1 (indicating "not set").
pub(super) fn i32_to_option_u32(value: i32) -> Option<u32> {
    if value < 0 {
        None
    } else {
        Some(value as u32)
    }
}

/// Convert an i8 tri-state to Option<bool>: -1 = None, 0 = false, 1 = true
pub(super) fn i8_to_option_bool(value: i8) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// Parse a comma-separated list of host:port pairs.
///
/// # Safety
///
/// `hosts_ptr` must be a valid null-terminated C string or null.
pub(super) unsafe fn parse_hosts(hosts_ptr: *const c_char) -> Result<Vec<String>> {
    let hosts_str = match c_char_to_str(hosts_ptr)? {
        Some(s) => s,
        None => return Err(Error::invalid_argument("No hosts provided")),
    };

    let hosts: Vec<String> = hosts_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if hosts.is_empty() {
        return Err(Error::invalid_argument("No hosts provided"));
    }

    Ok(hosts)
}

/// Parse compressors from a comma-separated list.
///
/// # Safety
///
/// `compressors_ptr` must be a valid null-terminated C string or null.
#[cfg(any(
    feature = "zstd-compression",
    feature = "zlib-compression",
    feature = "snappy-compression"
))]
pub(super) unsafe fn parse_compressors(
    compressors_ptr: *const c_char,
) -> Result<Option<Vec<Compressor>>> {
    use std::str::FromStr;

    let compressors_str = match c_char_to_str(compressors_ptr)? {
        Some(s) => s,
        None => return Ok(None),
    };

    if compressors_str.is_empty() {
        return Ok(None);
    }

    let mut compressors = Vec::new();
    for compressor_name in compressors_str.split(',') {
        let compressor_name = compressor_name.trim();
        if !compressor_name.is_empty() {
            let compressor = Compressor::from_str(compressor_name)?;
            compressors.push(compressor);
        }
    }

    if compressors.is_empty() {
        Ok(None)
    } else {
        Ok(Some(compressors))
    }
}

/// Parse an authentication mechanism string.
///
/// # Safety
///
/// `mechanism_ptr` must be a valid null-terminated C string or null.
pub(super) unsafe fn parse_auth_mechanism(
    mechanism_ptr: *const c_char,
) -> Result<Option<AuthMechanism>> {
    let mechanism = match c_char_to_str(mechanism_ptr)? {
        Some(s) => s,
        None => return Ok(None),
    };

    let auth_mechanism = match mechanism {
        "SCRAM-SHA-1" => AuthMechanism::ScramSha1,
        "SCRAM-SHA-256" => AuthMechanism::ScramSha256,
        "MONGODB-CR" => AuthMechanism::MongoDbCr,
        "MONGODB-X509" => AuthMechanism::MongoDbX509,
        "PLAIN" => AuthMechanism::Plain,
        "MONGODB-OIDC" => AuthMechanism::MongoDbOidc,
        #[cfg(feature = "gssapi-auth")]
        "GSSAPI" => AuthMechanism::Gssapi,
        #[cfg(feature = "aws-auth")]
        "MONGODB-AWS" => AuthMechanism::MongoDbAws,
        _ => {
            return Err(Error::invalid_argument(format!(
                "Unknown or unsupported authentication mechanism: {}",
                mechanism
            )))
        }
    };

    Ok(Some(auth_mechanism))
}

/// Parse read preference mode from a u8 value.
/// 0 = Primary, 1 = PrimaryPreferred, 2 = Secondary, 3 = SecondaryPreferred, 4 = Nearest
/// 255 = Not set (returns None)
pub(super) fn parse_read_preference_mode(mode: u8) -> Result<Option<ReadPreference>> {
    match mode {
        0 => Ok(Some(ReadPreference::Primary)),
        1 => Ok(Some(ReadPreference::PrimaryPreferred { options: None })),
        2 => Ok(Some(ReadPreference::Secondary { options: None })),
        3 => Ok(Some(ReadPreference::SecondaryPreferred { options: None })),
        4 => Ok(Some(ReadPreference::Nearest { options: None })),
        255 => Ok(None),
        _ => Err(Error::invalid_argument(format!(
            "Invalid read preference mode: {}. Valid values are 0-4 or 255 for not set.",
            mode
        ))),
    }
}

pub(super) fn with_callback<Out>(
    callback: extern "C" fn(*mut c_void, *const Out, *const super::error::Error),
    userdata: *mut c_void,
    body: impl FnOnce() -> std::result::Result<Out, super::error::Error>,
) {
    match body() {
        Ok(out) => callback(userdata, &out, std::ptr::null()),
        Err(e) => callback(userdata, std::ptr::null(), &e),
    }
}

pub(super) fn with_err_callback_internal<COut, ROut>(
    callback: extern "C" fn(*mut c_void, *const COut, *const super::error::Error),
    userdata: *mut c_void,
    body: impl FnOnce() -> Result<ROut>,
) -> Option<ROut> {
    match body() {
        Ok(v) => Some(v),
        Err(e) => {
            callback(userdata, std::ptr::null(), &super::error::Error::from(&e));
            None
        }
    }
}

macro_rules! with_err_callback {
    ($callback:expr, $userdata:expr, $body:expr) => {{
        match $crate::ffi::utils::with_err_callback_internal($callback, $userdata, $body) {
            Some(v) => v,
            None => return,
        }
    }};
}
pub(super) use with_err_callback;
