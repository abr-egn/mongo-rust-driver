//! Tests for FFI find operations.

use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::ffi::{
    client::{mongo_client_destroy, mongo_client_new},
    cursor::CursorResult,
    error::{Error, ErrorType},
    find::mongo_find,
    types::ConnectionSettings,
};

extern "C" fn find_callback(
    userdata: *mut c_void,
    _result: *const CursorResult,
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
                "Error should be InvalidArgument for null inputs"
            );
        }
    }
}

#[test]
fn test_find_null_client() {
    let db_name = CString::new("test").unwrap();
    let coll_name = CString::new("test_coll").unwrap();

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        mongo_find(
            ptr::null_mut(), // null client
            ptr::null(),     // no context
            db_name.as_ptr(),
            coll_name.as_ptr(),
            ptr::null(), // no filter
            ptr::null(), // no options
            find_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );
    }

    assert!(
        callback_invoked.load(Ordering::SeqCst),
        "Callback should be invoked for null client"
    );
}

#[test]
fn test_find_null_db_name() {
    let hosts = CString::new("localhost:27017").unwrap();
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

        mongo_find(
            client,
            ptr::null(), // no context
            ptr::null(), // null db_name
            coll_name.as_ptr(),
            ptr::null(), // no filter
            ptr::null(), // no options
            find_callback,
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
fn test_find_null_coll_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db_name = CString::new("test").unwrap();

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

        mongo_find(
            client,
            ptr::null(), // no context
            db_name.as_ptr(),
            ptr::null(), // null coll_name
            ptr::null(), // no filter
            ptr::null(), // no options
            find_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null coll_name"
        );

        mongo_client_destroy(client);
    }
}
