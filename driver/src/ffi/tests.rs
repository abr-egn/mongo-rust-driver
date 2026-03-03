//! Tests for the FFI layer.

use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::bson::doc;

use super::{
    client::{mongo_client_destroy, mongo_client_new},
    command::mongo_run_command,
    error::{error_free, Error, ErrorType},
    types::{AuthSettings, Bson, ConnectionSettings, OwnedBson, TlsSettings},
};

#[test]
fn test_client_new_minimal() {
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
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client creation should succeed");

        mongo_client_destroy(client);
    }
}

#[test]
fn test_client_new_maximal() {
    // Create client with all available settings
    let hosts = CString::new("localhost:27017,localhost:27018,localhost:27019").unwrap();
    let app_name = CString::new("test_app").unwrap();
    let replica_set = CString::new("rs0").unwrap();
    let srv_service_name = CString::new("mongodb").unwrap();
    let username = CString::new("testuser").unwrap();
    let password = CString::new("testpass").unwrap();
    let source = CString::new("admin").unwrap();
    let mechanism = CString::new("SCRAM-SHA-256").unwrap();
    #[cfg(any(
        feature = "zstd-compression",
        feature = "zlib-compression",
        feature = "snappy-compression"
    ))]
    let compressors = CString::new("snappy").unwrap();

    let conn_settings = ConnectionSettings {
        hosts: hosts.as_ptr(),
        app_name: app_name.as_ptr(),
        #[cfg(any(
            feature = "zstd-compression",
            feature = "zlib-compression",
            feature = "snappy-compression"
        ))]
        compressors: compressors.as_ptr(),
        #[cfg(not(any(
            feature = "zstd-compression",
            feature = "zlib-compression",
            feature = "snappy-compression"
        )))]
        compressors: ptr::null(),
        direct_connection: false,
        load_balanced: false,
        max_pool_size: 100,
        min_pool_size: 10,
        max_idle_time_ms: 60000,
        connect_timeout_ms: 10000,
        socket_timeout_ms: 30000,
        server_selection_timeout_ms: 30000,
        local_threshold_ms: 15,
        heartbeat_frequency_ms: 10000,
        replica_set: replica_set.as_ptr(),
        read_preference_mode: 2, // Secondary
        srv_service_name: srv_service_name.as_ptr(),
        srv_max_hosts: 5,
    };

    let auth_settings = AuthSettings {
        mechanism: mechanism.as_ptr(),
        username: username.as_ptr(),
        password: password.as_ptr(),
        source: source.as_ptr(),
    };

    let tls_settings = TlsSettings {
        enabled: true,
        allow_invalid_certificates: true,
        allow_invalid_hostnames: true,
        ca_file: ptr::null(),
        cert_file: ptr::null(),
        cert_key_file: ptr::null(),
    };

    unsafe {
        let client = mongo_client_new(
            &conn_settings,
            &auth_settings,
            &tls_settings,
            ptr::null_mut(),
        );
        assert!(
            !client.is_null(),
            "Client creation with maximal settings should succeed"
        );

        mongo_client_destroy(client);
    }
}

#[test]
fn test_client_new_multiple() {
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
        let client1 = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
        let client2 = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
        let client3 = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());

        assert!(!client1.is_null(), "First client should be created");
        assert!(!client2.is_null(), "Second client should be created");
        assert!(!client3.is_null(), "Third client should be created");

        // Destroy in different order
        mongo_client_destroy(client2);
        mongo_client_destroy(client1);
        mongo_client_destroy(client3);
    }
}

#[test]
fn test_client_new_error_handling() {
    let invalid_hosts = CString::new("invalid:host:port:format").unwrap();

    let conn_settings = ConnectionSettings {
        hosts: invalid_hosts.as_ptr(),
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
        let mut error: *mut Error = ptr::null_mut();
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), &mut error);

        assert!(
            client.is_null(),
            "Client creation should fail with invalid hosts"
        );
        assert!(
            !error.is_null(),
            "Error should be set when client creation fails"
        );

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
