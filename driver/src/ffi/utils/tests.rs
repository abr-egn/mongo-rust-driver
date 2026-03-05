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
    let hosts1 = CString::new("localhost:27017").unwrap();
    let result = unsafe { parse_hosts(hosts1.as_ptr()) }.unwrap();
    assert_eq!(result, vec!["localhost:27017"]);

    let hosts2 = CString::new("host1:27017,host2:27018,host3:27019").unwrap();
    let result = unsafe { parse_hosts(hosts2.as_ptr()) }.unwrap();
    assert_eq!(result, vec!["host1:27017", "host2:27018", "host3:27019"]);

    let hosts3 = CString::new("").unwrap();
    let result = unsafe { parse_hosts(hosts3.as_ptr()) };
    assert!(result.is_err());

    let result = unsafe { parse_hosts(std::ptr::null()) };
    assert!(result.is_err());
}
