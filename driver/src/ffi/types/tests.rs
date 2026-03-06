use std::{ffi::CString, ptr};

use crate::bson::doc;

use crate::ffi::types::{
    mongo_read_concern_create,
    mongo_read_concern_destroy,
    mongo_read_preference_create,
    mongo_read_preference_destroy,
    mongo_write_concern_create,
    mongo_write_concern_destroy,
    Bson,
    ReadConcernOptions,
    ReadPreferenceOptions,
    WriteConcernOptions,
};

// Read Preference Tests

#[test]
fn test_read_preference_all_modes() {
    unsafe {
        // Test all valid modes: 0=Primary, 1=PrimaryPreferred, 2=Secondary, 3=SecondaryPreferred,
        // 4=Nearest
        for mode in 0..=4u8 {
            let rp = mongo_read_preference_create(mode, ptr::null());
            assert!(
                !rp.is_null(),
                "Mode {} should create valid read preference",
                mode
            );
            mongo_read_preference_destroy(rp);
        }
    }
}

#[test]
fn test_read_preference_invalid_mode() {
    unsafe {
        // Mode > 4 should return null
        let rp = mongo_read_preference_create(5, ptr::null());
        assert!(rp.is_null(), "Invalid mode 5 should return null");

        let rp = mongo_read_preference_create(255, ptr::null());
        assert!(rp.is_null(), "Invalid mode 255 should return null");
    }
}

#[test]
fn test_read_preference_destroy_null() {
    // Should be a no-op, not crash
    unsafe {
        mongo_read_preference_destroy(ptr::null_mut());
    }
}

#[test]
fn test_read_preference_with_all_options() {
    // Tag sets
    let tag_sets_doc = doc! {
        "": [{"dc": "east"}]
    };
    let mut tag_bytes = Vec::new();
    tag_sets_doc
        .to_writer(&mut tag_bytes)
        .expect("encode should work");

    let tags_bson = Bson {
        data: tag_bytes.as_ptr(),
        len: tag_bytes.len(),
    };

    // Hedge options
    let hedge_doc = doc! { "enabled": false };
    let mut hedge_bytes = Vec::new();
    hedge_doc
        .to_writer(&mut hedge_bytes)
        .expect("encode should work");

    let hedge_bson = Bson {
        data: hedge_bytes.as_ptr(),
        len: hedge_bytes.len(),
    };

    let options = ReadPreferenceOptions {
        tags: &tags_bson,
        max_staleness_seconds: 120,
        hedge: &hedge_bson,
    };

    unsafe {
        let rp = mongo_read_preference_create(1, &options); // PrimaryPreferred
        assert!(
            !rp.is_null(),
            "Read preference with all options should be created"
        );
        mongo_read_preference_destroy(rp);
    }
}

// Read Concern Tests

#[test]
fn test_read_concern_create_majority() {
    let level = CString::new("majority").unwrap();
    let options = ReadConcernOptions {
        level: level.as_ptr(),
    };

    unsafe {
        let rc = mongo_read_concern_create(&options);
        assert!(
            !rc.is_null(),
            "Read concern with 'majority' should be created"
        );
        mongo_read_concern_destroy(rc);
    }
}

#[test]
fn test_read_concern_create_local() {
    let level = CString::new("local").unwrap();
    let options = ReadConcernOptions {
        level: level.as_ptr(),
    };

    unsafe {
        let rc = mongo_read_concern_create(&options);
        assert!(!rc.is_null(), "Read concern with 'local' should be created");
        mongo_read_concern_destroy(rc);
    }
}

#[test]
fn test_read_concern_create_snapshot() {
    let level = CString::new("snapshot").unwrap();
    let options = ReadConcernOptions {
        level: level.as_ptr(),
    };

    unsafe {
        let rc = mongo_read_concern_create(&options);
        assert!(
            !rc.is_null(),
            "Read concern with 'snapshot' should be created"
        );
        mongo_read_concern_destroy(rc);
    }
}

#[test]
fn test_read_concern_create_linearizable() {
    let level = CString::new("linearizable").unwrap();
    let options = ReadConcernOptions {
        level: level.as_ptr(),
    };

    unsafe {
        let rc = mongo_read_concern_create(&options);
        assert!(
            !rc.is_null(),
            "Read concern with 'linearizable' should be created"
        );
        mongo_read_concern_destroy(rc);
    }
}

#[test]
fn test_read_concern_create_available() {
    let level = CString::new("available").unwrap();
    let options = ReadConcernOptions {
        level: level.as_ptr(),
    };

    unsafe {
        let rc = mongo_read_concern_create(&options);
        assert!(
            !rc.is_null(),
            "Read concern with 'available' should be created"
        );
        mongo_read_concern_destroy(rc);
    }
}

#[test]
fn test_read_concern_create_custom() {
    let level = CString::new("customLevel").unwrap();
    let options = ReadConcernOptions {
        level: level.as_ptr(),
    };

    unsafe {
        let rc = mongo_read_concern_create(&options);
        assert!(
            !rc.is_null(),
            "Read concern with custom level should be created"
        );
        mongo_read_concern_destroy(rc);
    }
}

#[test]
fn test_read_concern_null_options() {
    unsafe {
        let rc = mongo_read_concern_create(ptr::null());
        assert!(
            rc.is_null(),
            "Read concern should be null with null options"
        );
    }
}

#[test]
fn test_read_concern_null_level() {
    let options = ReadConcernOptions { level: ptr::null() };

    unsafe {
        let rc = mongo_read_concern_create(&options);
        assert!(rc.is_null(), "Read concern should be null with null level");
    }
}

#[test]
fn test_read_concern_destroy_null() {
    unsafe {
        mongo_read_concern_destroy(ptr::null_mut());
    }
}

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
        bson::{rawdoc, RawArrayBuf, RawDocument},
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
        let ptr_slice = std::slice::from_raw_parts(bson_array.data, bson_array.len);

        // First document: {"a": 1}
        let raw_doc1 = RawDocument::from_bytes(std::slice::from_raw_parts(
            ptr_slice[0],
            doc1.as_bytes().len(),
        ))
        .unwrap();
        assert_eq!(raw_doc1.get_i32("a").unwrap(), 1);

        // Second document: {"b": 2}
        let raw_doc2 = RawDocument::from_bytes(std::slice::from_raw_parts(
            ptr_slice[1],
            doc2.as_bytes().len(),
        ))
        .unwrap();
        assert_eq!(raw_doc2.get_i32("b").unwrap(), 2);

        // Third document: {"c": 3}
        let raw_doc3 = RawDocument::from_bytes(std::slice::from_raw_parts(
            ptr_slice[2],
            doc3.as_bytes().len(),
        ))
        .unwrap();
        assert_eq!(raw_doc3.get_i32("c").unwrap(), 3);
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
