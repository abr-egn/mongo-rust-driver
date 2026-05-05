use std::{ffi::{c_void, CString}, ptr};

extern "C" fn noop_callback(_userdata: *mut c_void) {}

use crate::ffi::{
    client::{mongo_client_destroy, mongo_client_new},
    error::{error_free, Error, ErrorType},
    types::{AuthSettings, ConnectionSettings, TlsSettings},
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
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client creation should succeed");

        mongo_client_destroy(client, noop_callback, ptr::null_mut());
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
            ptr::null(),
            ptr::null_mut(),
        );
        assert!(
            !client.is_null(),
            "Client creation with maximal settings should succeed"
        );

        mongo_client_destroy(client, noop_callback, ptr::null_mut());
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
        let client1 = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        let client2 = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        let client3 = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());

        assert!(!client1.is_null(), "First client should be created");
        assert!(!client2.is_null(), "Second client should be created");
        assert!(!client3.is_null(), "Third client should be created");

        // Destroy in different order
        mongo_client_destroy(client2, noop_callback, ptr::null_mut());
        mongo_client_destroy(client1, noop_callback, ptr::null_mut());
        mongo_client_destroy(client3, noop_callback, ptr::null_mut());
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
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null(), &mut error);

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

use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

use crate::ffi::event::MongoCommandEventHandler;

#[test]
fn test_client_new_with_null_event_handler() {
    // Null handler pointer: must succeed and not crash.
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
        let client = mongo_client_new(
            &conn_settings,
            ptr::null(),
            ptr::null(),
            ptr::null(), // null handler = no monitoring
            ptr::null_mut(),
        );
        assert!(!client.is_null(), "Client with null event handler should succeed");
        mongo_client_destroy(client, noop_callback, ptr::null_mut());
    }
}

#[test]
fn test_client_new_with_event_handler() {
    static STARTED_COUNT: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn on_started(
        _userdata: *mut c_void,
        _event: *const crate::ffi::event::FfiCommandStartedEvent,
    ) {
        STARTED_COUNT.fetch_add(1, AtomicOrdering::SeqCst);
    }

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

    let handler = MongoCommandEventHandler {
        started: Some(on_started),
        succeeded: None,
        failed: None,
        userdata: ptr::null_mut(),
    };

    unsafe {
        let client = mongo_client_new(
            &conn_settings,
            ptr::null(),
            ptr::null(),
            &handler,
            ptr::null_mut(),
        );
        assert!(!client.is_null(), "Client with event handler should succeed");
        mongo_client_destroy(client, noop_callback, ptr::null_mut());
    }
}
