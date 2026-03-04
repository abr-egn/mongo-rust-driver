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
    session::{
        mongo_session_abort_transaction,
        mongo_session_commit_transaction,
        mongo_session_end,
        mongo_session_start,
        mongo_session_start_transaction,
        SessionOptions,
        TransactionOptions,
    },
    types::{
        mongo_read_preference_create,
        mongo_read_preference_destroy,
        AuthSettings,
        Bson,
        ConnectionSettings,
        OwnedBson,
        ReadPreferenceOptions,
        TlsSettings,
    },
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
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        let session = mongo_session_start(client, ptr::null(), ptr::null_mut());
        assert!(!session.is_null(), "Session should be created");

        mongo_session_end(session);
        mongo_client_destroy(client);
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
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        let session = mongo_session_start(client, &session_options, ptr::null_mut());
        assert!(!session.is_null(), "Session with options should be created");

        mongo_session_end(session);
        mongo_client_destroy(client);
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
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
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

        mongo_client_destroy(client);
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
        let client = mongo_client_new(&conn_settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null(), "Client should be created");

        let session = mongo_session_start(client, &session_options, ptr::null_mut());
        assert!(
            !session.is_null(),
            "Session with transaction options should be created"
        );

        mongo_session_end(session);
        mongo_client_destroy(client);
    }
}

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
