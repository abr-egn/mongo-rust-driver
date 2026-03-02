//! Utility functions for FFI operations.

use std::{ffi::CStr, os::raw::c_char, time::Duration};

use crate::error::{Error, Result};

/// Convert a C string pointer to a Rust String.
///
/// Returns None if the pointer is null.
/// Returns an error if the string is not valid UTF-8.
///
/// # Safety
///
/// The pointer must be valid and point to a null-terminated C string.
pub(crate) unsafe fn c_char_to_string(ptr: *const c_char) -> Result<Option<String>> {
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
#[allow(dead_code)]
pub(crate) unsafe fn c_char_to_str<'a>(ptr: *const c_char) -> Result<Option<&'a str>> {
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
pub(crate) fn i64_to_duration_ms(ms: i64) -> Option<Duration> {
    if ms < 0 {
        None
    } else {
        Some(Duration::from_millis(ms as u64))
    }
}

/// Convert an i32 value to an Option<u32>.
///
/// Returns None if the value is -1 (indicating "not set").
pub(crate) fn i32_to_option_u32(value: i32) -> Option<u32> {
    if value < 0 {
        None
    } else {
        Some(value as u32)
    }
}

/// Parse a comma-separated list of host:port pairs.
///
/// Returns an error if any host:port pair is malformed.
pub(crate) fn parse_hosts(hosts_str: &str) -> Result<Vec<String>> {
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

/// Parse a comma-separated list of compressor names.
#[allow(dead_code)]
pub(crate) fn parse_compressors(compressors_str: &str) -> Vec<String> {
    compressors_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_c_char_to_string() {
        let c_string = CString::new("hello").unwrap();
        let result = unsafe { c_char_to_string(c_string.as_ptr()) };
        assert_eq!(result.unwrap(), Some("hello".to_string()));

        let result = unsafe { c_char_to_string(std::ptr::null()) };
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_i64_to_duration_ms() {
        assert_eq!(i64_to_duration_ms(-1), None);
        assert_eq!(i64_to_duration_ms(0), Some(Duration::from_millis(0)));
        assert_eq!(i64_to_duration_ms(1000), Some(Duration::from_millis(1000)));
    }

    #[test]
    fn test_i32_to_option_u32() {
        assert_eq!(i32_to_option_u32(-1), None);
        assert_eq!(i32_to_option_u32(0), Some(0));
        assert_eq!(i32_to_option_u32(100), Some(100));
    }

    #[test]
    fn test_parse_hosts() {
        let result = parse_hosts("localhost:27017").unwrap();
        assert_eq!(result, vec!["localhost:27017"]);

        let result = parse_hosts("host1:27017,host2:27018,host3:27019").unwrap();
        assert_eq!(result, vec!["host1:27017", "host2:27018", "host3:27019"]);

        let result = parse_hosts("");
        assert!(result.is_err());
    }
}
