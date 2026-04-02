use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::bson::doc;

use crate::ffi::{
    client::{mongo_client_destroy, mongo_client_new},
    error::{Error, ErrorType},
    insert::{mongo_insert_many, mongo_insert_one, InsertManyResult, InsertOneResult},
    types::{Bson, BsonArray, ConnectionSettings},
};

// Callback for insert_one tests
extern "C" fn insert_one_callback(
    userdata: *mut c_void,
    result: *const InsertOneResult,
    error: *const Error,
) {
    unsafe {
        let callback_invoked = &*(userdata as *const AtomicBool);
        callback_invoked.store(true, Ordering::SeqCst);

        // For null input tests, we expect an error
        if !error.is_null() {
            let error_ref = &*error;
            assert_eq!(
                error_ref.error_type,
                ErrorType::InvalidArgument as u8,
                "Error should be InvalidArgument for null inputs"
            );
            assert!(result.is_null(), "Result should be null when error is set");
        }
    }
}

#[test]
fn test_insert_one_null_client() {
    let db_name = CString::new("test").unwrap();
    let coll_name = CString::new("test_coll").unwrap();
    let document = doc! { "foo": "bar" };
    let mut doc_bytes = Vec::new();
    document
        .to_writer(&mut doc_bytes)
        .expect("encode should work");

    let bson_doc = Bson {
        data: doc_bytes.as_ptr(),
        len: doc_bytes.len(),
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        mongo_insert_one(
            ptr::null_mut(), // null client
            ptr::null(),     // no context
            db_name.as_ptr(),
            coll_name.as_ptr(),
            &bson_doc,
            -1,          // bypass_document_validation: None
            ptr::null(), // no comment
            insert_one_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );
    }

    assert!(
        callback_invoked.load(Ordering::SeqCst),
        "Callback should be invoked for null client"
    );
}

#[test]
fn test_insert_one_null_document() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db_name = CString::new("test").unwrap();
    let coll_name = CString::new("test_coll").unwrap();

    let conn_settings = ConnectionSettings {
        hosts: hosts.as_ptr(),
        app_name: ptr::null(),
        compressors: ptr::null(),
        direct_connection: false,
        load_balanced: false,
        max_pool_size: -1,
        min_pool_size: -1,
        max_idle_time_ms: -1,
        connect_timeout_ms: -1,
        socket_timeout_ms: -1,
        server_selection_timeout_ms: -1,
        local_threshold_ms: -1,
        heartbeat_frequency_ms: -1,
        replica_set: ptr::null(),
        read_preference_mode: 0,
        srv_service_name: ptr::null(),
        srv_max_hosts: -1,
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        mongo_insert_one(
            client,
            ptr::null(), // no context
            db_name.as_ptr(),
            coll_name.as_ptr(),
            ptr::null(), // null document
            -1,
            ptr::null(),
            insert_one_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null document"
        );

        mongo_client_destroy(client);
    }
}

#[test]
fn test_insert_one_null_db_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let coll_name = CString::new("test_coll").unwrap();
    let document = doc! { "foo": "bar" };
    let mut doc_bytes = Vec::new();
    document
        .to_writer(&mut doc_bytes)
        .expect("encode should work");

    let bson_doc = Bson {
        data: doc_bytes.as_ptr(),
        len: doc_bytes.len(),
    };

    let conn_settings = ConnectionSettings {
        hosts: hosts.as_ptr(),
        app_name: ptr::null(),
        compressors: ptr::null(),
        direct_connection: false,
        load_balanced: false,
        max_pool_size: -1,
        min_pool_size: -1,
        max_idle_time_ms: -1,
        connect_timeout_ms: -1,
        socket_timeout_ms: -1,
        server_selection_timeout_ms: -1,
        local_threshold_ms: -1,
        heartbeat_frequency_ms: -1,
        replica_set: ptr::null(),
        read_preference_mode: 0,
        srv_service_name: ptr::null(),
        srv_max_hosts: -1,
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        mongo_insert_one(
            client,
            ptr::null(), // no context
            ptr::null(), // null db_name
            coll_name.as_ptr(),
            &bson_doc,
            -1,
            ptr::null(),
            insert_one_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null db_name"
        );

        mongo_client_destroy(client);
    }
}

#[test]
fn test_insert_one_null_coll_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db_name = CString::new("test").unwrap();
    let document = doc! { "foo": "bar" };
    let mut doc_bytes = Vec::new();
    document
        .to_writer(&mut doc_bytes)
        .expect("encode should work");

    let bson_doc = Bson {
        data: doc_bytes.as_ptr(),
        len: doc_bytes.len(),
    };

    let conn_settings = ConnectionSettings {
        hosts: hosts.as_ptr(),
        app_name: ptr::null(),
        compressors: ptr::null(),
        direct_connection: false,
        load_balanced: false,
        max_pool_size: -1,
        min_pool_size: -1,
        max_idle_time_ms: -1,
        connect_timeout_ms: -1,
        socket_timeout_ms: -1,
        server_selection_timeout_ms: -1,
        local_threshold_ms: -1,
        heartbeat_frequency_ms: -1,
        replica_set: ptr::null(),
        read_preference_mode: 0,
        srv_service_name: ptr::null(),
        srv_max_hosts: -1,
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        mongo_insert_one(
            client,
            ptr::null(), // no context
            db_name.as_ptr(),
            ptr::null(), // null coll_name
            &bson_doc,
            -1,
            ptr::null(),
            insert_one_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null coll_name"
        );

        mongo_client_destroy(client);
    }
}

// Callback for insert_many tests
extern "C" fn insert_many_callback(
    userdata: *mut c_void,
    result: *const InsertManyResult,
    error: *const Error,
) {
    unsafe {
        let callback_invoked = &*(userdata as *const AtomicBool);
        callback_invoked.store(true, Ordering::SeqCst);

        if !error.is_null() {
            let error_ref = &*error;
            assert_eq!(
                error_ref.error_type,
                ErrorType::InvalidArgument as u8,
                "Error should be InvalidArgument for invalid inputs"
            );
            assert!(result.is_null(), "Result should be null when error is set");
        }
    }
}

#[test]
fn test_insert_many_null_client() {
    let db_name = CString::new("test").unwrap();
    let coll_name = CString::new("test_coll").unwrap();

    // Create a document array with one document
    let document = doc! { "foo": "bar" };
    let mut doc_bytes = Vec::new();
    document
        .to_writer(&mut doc_bytes)
        .expect("encode should work");
    let doc_ptr: *const u8 = doc_bytes.as_ptr();
    let doc_ptrs = [doc_ptr];

    let bson_array = BsonArray {
        data: doc_ptrs.as_ptr(),
        len: 1,
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        mongo_insert_many(
            ptr::null_mut(), // null client
            ptr::null(),     // no context
            db_name.as_ptr(),
            coll_name.as_ptr(),
            bson_array,
            -1,          // bypass_document_validation: None
            true,        // ordered
            ptr::null(), // no comment
            insert_many_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );
    }

    assert!(
        callback_invoked.load(Ordering::SeqCst),
        "Callback should be invoked for null client"
    );
}

#[test]
fn test_insert_many_empty_documents() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db_name = CString::new("test").unwrap();
    let coll_name = CString::new("test_coll").unwrap();

    let conn_settings = ConnectionSettings {
        hosts: hosts.as_ptr(),
        app_name: ptr::null(),
        compressors: ptr::null(),
        direct_connection: false,
        load_balanced: false,
        max_pool_size: -1,
        min_pool_size: -1,
        max_idle_time_ms: -1,
        connect_timeout_ms: -1,
        socket_timeout_ms: -1,
        server_selection_timeout_ms: -1,
        local_threshold_ms: -1,
        heartbeat_frequency_ms: -1,
        replica_set: ptr::null(),
        read_preference_mode: 0,
        srv_service_name: ptr::null(),
        srv_max_hosts: -1,
    };

    // Empty documents array
    let bson_array = BsonArray {
        data: ptr::null(),
        len: 0,
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        mongo_insert_many(
            client,
            ptr::null(), // no context
            db_name.as_ptr(),
            coll_name.as_ptr(),
            bson_array,
            -1,
            true,
            ptr::null(),
            insert_many_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for empty documents"
        );

        mongo_client_destroy(client);
    }
}

#[test]
fn test_insert_many_null_db_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let coll_name = CString::new("test_coll").unwrap();

    let document = doc! { "foo": "bar" };
    let mut doc_bytes = Vec::new();
    document
        .to_writer(&mut doc_bytes)
        .expect("encode should work");
    let doc_ptr: *const u8 = doc_bytes.as_ptr();
    let doc_ptrs = [doc_ptr];

    let bson_array = BsonArray {
        data: doc_ptrs.as_ptr(),
        len: 1,
    };

    let conn_settings = ConnectionSettings {
        hosts: hosts.as_ptr(),
        app_name: ptr::null(),
        compressors: ptr::null(),
        direct_connection: false,
        load_balanced: false,
        max_pool_size: -1,
        min_pool_size: -1,
        max_idle_time_ms: -1,
        connect_timeout_ms: -1,
        socket_timeout_ms: -1,
        server_selection_timeout_ms: -1,
        local_threshold_ms: -1,
        heartbeat_frequency_ms: -1,
        replica_set: ptr::null(),
        read_preference_mode: 0,
        srv_service_name: ptr::null(),
        srv_max_hosts: -1,
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        mongo_insert_many(
            client,
            ptr::null(), // no context
            ptr::null(), // null db_name
            coll_name.as_ptr(),
            bson_array,
            -1,
            true,
            ptr::null(),
            insert_many_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null db_name"
        );

        mongo_client_destroy(client);
    }
}

#[test]
fn test_insert_many_null_coll_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db_name = CString::new("test").unwrap();

    let document = doc! { "foo": "bar" };
    let mut doc_bytes = Vec::new();
    document
        .to_writer(&mut doc_bytes)
        .expect("encode should work");
    let doc_ptr: *const u8 = doc_bytes.as_ptr();
    let doc_ptrs = [doc_ptr];

    let bson_array = BsonArray {
        data: doc_ptrs.as_ptr(),
        len: 1,
    };

    let conn_settings = ConnectionSettings {
        hosts: hosts.as_ptr(),
        app_name: ptr::null(),
        compressors: ptr::null(),
        direct_connection: false,
        load_balanced: false,
        max_pool_size: -1,
        min_pool_size: -1,
        max_idle_time_ms: -1,
        connect_timeout_ms: -1,
        socket_timeout_ms: -1,
        server_selection_timeout_ms: -1,
        local_threshold_ms: -1,
        heartbeat_frequency_ms: -1,
        replica_set: ptr::null(),
        read_preference_mode: 0,
        srv_service_name: ptr::null(),
        srv_max_hosts: -1,
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        mongo_insert_many(
            client,
            ptr::null(), // no context
            db_name.as_ptr(),
            ptr::null(), // null coll_name
            bson_array,
            -1,
            true,
            ptr::null(),
            insert_many_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null coll_name"
        );

        mongo_client_destroy(client);
    }
}
