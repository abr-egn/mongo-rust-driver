use std::{ffi::CString, ops::Deref, ptr};

use crate::ffi::types::{
    mongo_write_concern_create,
    mongo_write_concern_destroy,
    WriteConcernOptions,
};

// Write Concern Tests

#[test]
fn test_write_concern_create_with_all_options() {
    let w_tag = CString::new("majority").unwrap();
    let options = WriteConcernOptions {
        w: 1, // ignored because w_tag is set
        w_tag: w_tag.as_ptr(),
        journal: 1,
        w_timeout_ms: 10000,
    };

    unsafe {
        let wc = mongo_write_concern_create(&options);
        assert!(
            !wc.is_null(),
            "Write concern with all options should be created"
        );
        mongo_write_concern_destroy(wc);
    }
}

#[test]
fn test_write_concern_create_custom_tag() {
    let w_tag = CString::new("myDataCenter").unwrap();
    let options = WriteConcernOptions {
        w: -1,
        w_tag: w_tag.as_ptr(),
        journal: -1,
        w_timeout_ms: -1,
    };

    unsafe {
        let wc = mongo_write_concern_create(&options);
        assert!(
            !wc.is_null(),
            "Write concern with custom tag should be created"
        );
        mongo_write_concern_destroy(wc);
    }
}

#[test]
fn test_write_concern_null_options() {
    unsafe {
        let wc = mongo_write_concern_create(ptr::null());
        assert!(
            wc.is_null(),
            "Write concern should be null with null options"
        );
    }
}

#[test]
fn test_write_concern_destroy_null() {
    unsafe {
        mongo_write_concern_destroy(ptr::null_mut());
    }
}

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
