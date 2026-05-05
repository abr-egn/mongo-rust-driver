use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::ffi::{
    client::{mongo_client_destroy, mongo_client_new},
    error::{error_free, Error, ErrorType},
    session::{
        mongo_session_abort_transaction,
        mongo_session_commit_transaction,
        mongo_session_end,
        mongo_session_start,
        mongo_session_start_transaction,
        SessionOptions,
        TransactionOptions,
    },
    types::ConnectionSettings,
};

extern "C" fn noop_callback(_userdata: *mut c_void) {}

// Callback for session transaction tests
extern "C" fn transaction_callback(userdata: *mut c_void, error: *const Error) {
    unsafe {
        let callback_invoked = &*(userdata as *const AtomicBool);
        callback_invoked.store(true, Ordering::SeqCst);

        // For null client/session tests, we expect an error
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
fn test_session_start_null_client() {
    unsafe {
        let mut error: *mut Error = ptr::null_mut();
        let session = mongo_session_start(ptr::null_mut(), ptr::null(), &mut error);

        assert!(session.is_null(), "Session should be null for null client");
        assert!(!error.is_null(), "Error should be set when client is null");

        if !error.is_null() {
            let error_ref = &*error;
            assert_eq!(
                error_ref.error_type,
                ErrorType::InvalidArgument as u8,
                "Error type should be InvalidArgument"
            );
            error_free(error);
        }
    }
}

#[test]
fn test_session_start_and_end() {
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

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        let session = mongo_session_start(client, ptr::null(), ptr::null_mut());
        assert!(!session.is_null(), "Session should be created");

        mongo_session_end(session);
        mongo_client_destroy(client, noop_callback, ptr::null_mut());
    }
}

#[test]
fn test_session_start_with_options() {
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

    let session_options = SessionOptions {
        causal_consistency: 1,
        snapshot: -1,
        default_transaction_options: ptr::null(),
    };

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        let session = mongo_session_start(client, &session_options, ptr::null_mut());
        assert!(!session.is_null(), "Session with options should be created");

        mongo_session_end(session);
        mongo_client_destroy(client, noop_callback, ptr::null_mut());
    }
}

#[test]
fn test_session_end_null() {
    // Should be a no-op, not crash
    unsafe {
        mongo_session_end(ptr::null_mut());
    }
}

#[test]
fn test_start_transaction_null_client() {
    let callback_invoked = AtomicBool::new(false);

    unsafe {
        mongo_session_start_transaction(
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            transaction_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );
    }

    assert!(
        callback_invoked.load(Ordering::SeqCst),
        "Callback should be invoked for null client"
    );
}

#[test]
fn test_start_transaction_null_session() {
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

        mongo_session_start_transaction(
            client,
            ptr::null_mut(),
            ptr::null(),
            transaction_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );

        assert!(
            callback_invoked.load(Ordering::SeqCst),
            "Callback should be invoked for null session"
        );

        mongo_client_destroy(client, noop_callback, ptr::null_mut());
    }
}

#[test]
fn test_commit_transaction_null_client() {
    let callback_invoked = AtomicBool::new(false);

    unsafe {
        mongo_session_commit_transaction(
            ptr::null_mut(),
            ptr::null_mut(),
            transaction_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );
    }

    assert!(
        callback_invoked.load(Ordering::SeqCst),
        "Callback should be invoked for null client"
    );
}

#[test]
fn test_abort_transaction_null_client() {
    let callback_invoked = AtomicBool::new(false);

    unsafe {
        mongo_session_abort_transaction(
            ptr::null_mut(),
            ptr::null_mut(),
            transaction_callback,
            &callback_invoked as *const AtomicBool as *mut c_void,
        );
    }

    assert!(
        callback_invoked.load(Ordering::SeqCst),
        "Callback should be invoked for null client"
    );
}

#[test]
fn test_transaction_options_parsing() {
    let hosts = CString::new("localhost:27017").unwrap();
    let read_concern_level = CString::new("majority").unwrap();
    let w_tag = CString::new("majority").unwrap();

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

    let tx_options = TransactionOptions {
        read_concern_level: read_concern_level.as_ptr(),
        write_concern_w: -1,
        write_concern_w_tag: w_tag.as_ptr(),
        write_concern_j: 1,
        write_concern_w_timeout_ms: 5000,
        read_preference_mode: 2, // Secondary
        max_commit_time_ms: 30000,
    };

    let session_options = SessionOptions {
        causal_consistency: 1,
        snapshot: 0,
        default_transaction_options: &tx_options,
    };

    unsafe {
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        let session = mongo_session_start(client, &session_options, ptr::null_mut());
        assert!(
            !session.is_null(),
            "Session with transaction options should be created"
        );

        mongo_session_end(session);
        mongo_client_destroy(client, noop_callback, ptr::null_mut());
    }
}
