use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::bson::doc;

use crate::ffi::{
    client::{mongo_client_destroy, mongo_client_new},
    command::mongo_run_command,
    error::{Error, ErrorType},
    types::{Bson, ConnectionSettings, OwnedBson},
};

// Callback for run_command tests
extern "C" fn run_command_callback(
    userdata: *mut c_void,
    result: *const OwnedBson,
    error: *const Error,
) {
    unsafe {
        let callback_invoked = &*(userdata as *const AtomicBool);
        callback_invoked.store(true, Ordering::SeqCst);

        // For null client test, we expect an error
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
fn test_run_command_null_client() {
    let db_name = CString::new("test").unwrap();
    let ping_command = doc! { "ping": 1 };
    let mut command_bytes = Vec::new();
    ping_command
        .to_writer(&mut command_bytes)
        .expect("encode should work");

    let command = Bson {
        data: command_bytes.as_ptr(),
        len: command_bytes.len(),
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        mongo_run_command(
            ptr::null_mut(),
            ptr::null_mut(), // no session
            db_name.as_ptr(),
            &command,
            255, // not set
            run_command_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );
    }

    assert!(
        callback_invoked.load(Ordering::SeqCst),
        "Callback should be invoked even for null client"
    );
}

#[test]
fn test_run_command_null_db_name() {
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

    let ping_command = doc! { "ping": 1 };
    let mut command_bytes = Vec::new();
    ping_command
        .to_writer(&mut command_bytes)
        .expect("encode should work");

    let command = Bson {
        data: command_bytes.as_ptr(),
        len: command_bytes.len(),
    };

    let callback_invoked = AtomicBool::new(false);

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        mongo_run_command(
            client,
            ptr::null_mut(), // no session
            ptr::null(),
            &command,
            255, // not set
            run_command_callback,
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
fn test_run_command_null_command() {
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
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        mongo_run_command(
            client,
            ptr::null_mut(), // no session
            db_name.as_ptr(),
            ptr::null(),
            255, // not set
            run_command_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null command"
        );

        mongo_client_destroy(client);
    }
}
