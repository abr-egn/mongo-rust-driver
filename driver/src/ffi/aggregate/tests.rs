use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::ffi::{
    aggregate::{mongo_aggregate_collection, mongo_aggregate_database},
    client::{mongo_client_destroy, mongo_client_new},
    cursor::CursorResult,
    error::{Error, ErrorType},
    types::{BsonArray, ConnectionSettings},
};

extern "C" fn aggregate_callback(
    userdata: *mut c_void,
    _result: *const CursorResult,
    error: *const Error,
) {
    unsafe {
        let invoked = &*(userdata as *const AtomicBool);
        invoked.store(true, Ordering::SeqCst);
        if !error.is_null() {
            assert_eq!((&*error).error_type, ErrorType::InvalidArgument as u8);
        }
    }
}

fn make_conn_settings(hosts: &CString) -> ConnectionSettings {
    ConnectionSettings {
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
    }
}

#[test]
fn test_aggregate_collection_null_client() {
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let pipeline = BsonArray { data: ptr::null(), len: 0 };
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_aggregate_collection(
            ptr::null_mut(),
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            pipeline,
            ptr::null(),
            aggregate_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst));
}

#[test]
fn test_aggregate_collection_null_db_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let coll = CString::new("c").unwrap();
    let pipeline = BsonArray { data: ptr::null(), len: 0 };
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_aggregate_collection(
            client,
            ptr::null(),
            ptr::null(),
            coll.as_ptr(),
            pipeline,
            ptr::null(),
            aggregate_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_aggregate_collection_null_coll_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db = CString::new("test").unwrap();
    let pipeline = BsonArray { data: ptr::null(), len: 0 };
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_aggregate_collection(
            client,
            ptr::null(),
            db.as_ptr(),
            ptr::null(),
            pipeline,
            ptr::null(),
            aggregate_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_aggregate_database_null_client() {
    let db = CString::new("test").unwrap();
    let pipeline = BsonArray { data: ptr::null(), len: 0 };
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_aggregate_database(
            ptr::null_mut(),
            ptr::null(),
            db.as_ptr(),
            pipeline,
            ptr::null(),
            aggregate_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst));
}

#[test]
fn test_aggregate_database_null_db_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let pipeline = BsonArray { data: ptr::null(), len: 0 };
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_aggregate_database(
            client,
            ptr::null(),
            ptr::null(),
            pipeline,
            ptr::null(),
            aggregate_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}
