use std::ptr;

use crate::bson::doc;

use crate::ffi::types::{
    mongo_read_preference_create,
    mongo_read_preference_destroy,
    Bson,
    ReadPreferenceOptions,
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
