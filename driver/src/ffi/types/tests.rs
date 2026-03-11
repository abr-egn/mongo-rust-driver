use std::{ffi::CString, ops::Deref, ptr};

// BsonArray Tests

#[test]
fn test_bson_array_from_array() {
    use crate::{
        bson::{rawdoc, RawArrayBuf},
        ffi::types::BsonArray,
    };

    let doc1 = rawdoc! { "a": 1 };
    let doc2 = rawdoc! { "b": 2 };
    let doc3 = rawdoc! { "c": 3 };

    let mut arr = RawArrayBuf::new();
    arr.push(doc1.clone());
    arr.push(doc2.clone());
    arr.push(doc3.clone());

    let (_ptrs, bson_array) = BsonArray::from_array(&arr).expect("from_array should succeed");

    assert_eq!(bson_array.len, 3);
    assert!(!bson_array.data.is_null());

    // Validate that bson_array.data points to raw documents with expected contents
    unsafe {
        let docs = bson_array.to_raw_docs();
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0], doc1.deref());
        assert_eq!(docs[1], doc2.deref());
        assert_eq!(docs[2], doc3.deref());
    }
}

#[test]
fn test_bson_array_from_empty_array() {
    use crate::{bson::RawArrayBuf, ffi::types::BsonArray};

    let arr = RawArrayBuf::new();
    let (ptrs, bson_array) = BsonArray::from_array(&arr).expect("from_array should succeed");

    assert!(bson_array.data.is_null());
    assert_eq!(bson_array.len, 0);
    assert!(ptrs.is_empty());
}

#[test]
fn test_bson_array_null() {
    use crate::ffi::types::BsonArray;

    let bson_array = BsonArray::null();
    assert!(bson_array.data.is_null());
    assert_eq!(bson_array.len, 0);
}

// ReadPreference Tests

#[test]
fn test_read_preference_parse_primary() {
    use crate::ffi::types::{ReadPreference, ReadPreferenceKind};

    let rp = ReadPreference {
        kind: ReadPreferenceKind::Primary as u8,
        tags: ptr::null(),
        max_staleness_seconds: -1,
        hedge: ptr::null(),
    };

    let parsed = unsafe { rp.parse() }.expect("parse should succeed");
    assert!(matches!(parsed, crate::options::ReadPreference::Primary));
}

#[test]
fn test_read_preference_parse_secondary() {
    use crate::ffi::types::{ReadPreference, ReadPreferenceKind};

    let rp = ReadPreference {
        kind: ReadPreferenceKind::Secondary as u8,
        tags: ptr::null(),
        max_staleness_seconds: -1,
        hedge: ptr::null(),
    };

    let parsed = unsafe { rp.parse() }.expect("parse should succeed");
    assert!(matches!(
        parsed,
        crate::options::ReadPreference::Secondary { .. }
    ));
}

#[test]
fn test_read_preference_parse_nearest_with_staleness() {
    use crate::ffi::types::{ReadPreference, ReadPreferenceKind};

    let rp = ReadPreference {
        kind: ReadPreferenceKind::Nearest as u8,
        tags: ptr::null(),
        max_staleness_seconds: 120,
        hedge: ptr::null(),
    };

    let parsed = unsafe { rp.parse() }.expect("parse should succeed");
    if let crate::options::ReadPreference::Nearest { options } = parsed {
        let opts = options.expect("options should be set");
        assert_eq!(
            opts.max_staleness,
            Some(std::time::Duration::from_secs(120))
        );
    } else {
        panic!("Expected Nearest read preference");
    }
}

#[test]
fn test_read_preference_parse_invalid_kind() {
    use crate::ffi::types::ReadPreference;

    let rp = ReadPreference {
        kind: 99, // invalid
        tags: ptr::null(),
        max_staleness_seconds: -1,
        hedge: ptr::null(),
    };

    let result = unsafe { rp.parse() };
    assert!(result.is_err());
}

// WriteConcern Tests

#[test]
fn test_write_concern_parse_w_nodes() {
    use crate::ffi::types::WriteConcern;

    let wc = WriteConcern {
        w: 2,
        w_tag: ptr::null(),
        journal: -1,
        w_timeout_ms: -1,
    };

    let parsed = unsafe { wc.parse() }.expect("parse should succeed");
    assert_eq!(parsed.w, Some(crate::concern::Acknowledgment::Nodes(2)));
    assert!(parsed.journal.is_none());
    assert!(parsed.w_timeout.is_none());
}

#[test]
fn test_write_concern_parse_w_tag_majority() {
    use crate::ffi::types::WriteConcern;

    let tag = CString::new("majority").unwrap();
    let wc = WriteConcern {
        w: 5, // should be ignored when w_tag is set
        w_tag: tag.as_ptr(),
        journal: 1,
        w_timeout_ms: 5000,
    };

    let parsed = unsafe { wc.parse() }.expect("parse should succeed");
    assert_eq!(parsed.w, Some(crate::concern::Acknowledgment::Majority));
    assert_eq!(parsed.journal, Some(true));
    assert_eq!(
        parsed.w_timeout,
        Some(std::time::Duration::from_millis(5000))
    );
}

#[test]
fn test_write_concern_parse_journal_false() {
    use crate::ffi::types::WriteConcern;

    let wc = WriteConcern {
        w: -1,
        w_tag: ptr::null(),
        journal: 0,
        w_timeout_ms: -1,
    };

    let parsed = unsafe { wc.parse() }.expect("parse should succeed");
    assert!(parsed.w.is_none());
    assert_eq!(parsed.journal, Some(false));
}

// ReadConcern Tests

#[test]
fn test_read_concern_parse_majority() {
    use crate::ffi::types::ReadConcern;

    let level = CString::new("majority").unwrap();
    let rc = ReadConcern {
        level: level.as_ptr(),
    };

    let parsed = unsafe { rc.parse() }.expect("parse should succeed");
    assert_eq!(parsed.level, crate::concern::ReadConcernLevel::Majority);
}

#[test]
fn test_read_concern_parse_local() {
    use crate::ffi::types::ReadConcern;

    let level = CString::new("local").unwrap();
    let rc = ReadConcern {
        level: level.as_ptr(),
    };

    let parsed = unsafe { rc.parse() }.expect("parse should succeed");
    assert_eq!(parsed.level, crate::concern::ReadConcernLevel::Local);
}

#[test]
fn test_read_concern_parse_null_level() {
    use crate::ffi::types::ReadConcern;

    let rc = ReadConcern { level: ptr::null() };

    let result = unsafe { rc.parse() };
    assert!(result.is_err());
}
