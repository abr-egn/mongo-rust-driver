use std::ops::Deref;

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
