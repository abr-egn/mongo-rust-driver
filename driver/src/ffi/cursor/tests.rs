//! Tests for FFI cursor operations.

use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::ffi::{
    client::{mongo_client_destroy, mongo_client_new},
    cursor::{mongo_cursor_close, mongo_cursor_get_more, FfiCursor},
    error::{Error, ErrorType},
    types::{BsonArray, ConnectionSettings},
};

extern "C" fn get_more_callback(
    userdata: *mut c_void,
    _exhausted: bool,
    _data: BsonArray,
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
fn test_cursor_get_more_null_client() {
    let callback_invoked = AtomicBool::new(false);

    unsafe {
        // Create a fake cursor pointer (we won't actually use it)
        let fake_cursor = 0x1234 as *mut FfiCursor;

        mongo_cursor_get_more(
            ptr::null_mut(), // null client
            fake_cursor,
            ptr::null_mut(), // no session
            &callback_invoked as *const AtomicBool as *mut c_void,
            get_more_callback,
        );
    }

    assert!(
        callback_invoked.load(Ordering::SeqCst),
        "Callback should be invoked for null client"
    );
}

#[test]
fn test_cursor_get_more_null_cursor() {
    let hosts = CString::new("localhost:27017").unwrap();

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

        mongo_cursor_get_more(
            client,
            ptr::null_mut(), // null cursor
            ptr::null_mut(),
            &callback_invoked as *const AtomicBool as *mut c_void,
            get_more_callback,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null cursor"
        );

        mongo_client_destroy(client);
    }
}

#[test]
fn test_cursor_close_null() {
    // Should not panic or crash
    unsafe {
        mongo_cursor_close(ptr::null_mut());
    }
}
