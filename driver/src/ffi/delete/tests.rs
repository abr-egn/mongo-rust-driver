use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    bson::doc,
    ffi::{
        client::{mongo_client_destroy, mongo_client_new},
        delete::{mongo_delete_many, mongo_delete_one, DeleteResult},
        error::{Error, ErrorType},
        types::{Bson, ConnectionSettings},
    },
};

extern "C" fn delete_callback(
    userdata: *mut c_void,
    result: *const DeleteResult,
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
                "Expected InvalidArgument error"
            );
            assert!(result.is_null(), "Result must be null when error is set");
        }
    }
}

fn make_filter_bson() -> (Vec<u8>, Bson) {
    let filter = doc! { "_id": 1 };
    let mut bytes = Vec::new();
    filter.to_writer(&mut bytes).unwrap();
    let bson = Bson { data: bytes.as_ptr(), len: bytes.len() };
    (bytes, bson)
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
fn test_delete_one_null_client() {
    let db_name = CString::new("test").unwrap();
    let coll_name = CString::new("test_coll").unwrap();
    let (_bytes, bson) = make_filter_bson();
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_delete_one(
            ptr::null_mut(),
            ptr::null(),
            db_name.as_ptr(),
            coll_name.as_ptr(),
            &bson,
            ptr::null(),
            delete_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst), "Callback must be invoked");
}

#[test]
fn test_delete_one_null_filter() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db_name = CString::new("test").unwrap();
    let coll_name = CString::new("test_coll").unwrap();
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_delete_one(
            client,
            ptr::null(),
            db_name.as_ptr(),
            coll_name.as_ptr(),
            ptr::null(), // null filter
            ptr::null(),
            delete_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst), "Callback must be invoked");
        mongo_client_destroy(client);
    }
}

#[test]
fn test_delete_one_null_db_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let coll_name = CString::new("test_coll").unwrap();
    let (_bytes, bson) = make_filter_bson();
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_delete_one(
            client,
            ptr::null(),
            ptr::null(), // null db_name
            coll_name.as_ptr(),
            &bson,
            ptr::null(),
            delete_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst), "Callback must be invoked");
        mongo_client_destroy(client);
    }
}

#[test]
fn test_delete_one_null_coll_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db_name = CString::new("test").unwrap();
    let (_bytes, bson) = make_filter_bson();
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_delete_one(
            client,
            ptr::null(),
            db_name.as_ptr(),
            ptr::null(), // null coll_name
            &bson,
            ptr::null(),
            delete_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst), "Callback must be invoked");
        mongo_client_destroy(client);
    }
}

#[test]
fn test_delete_many_null_client() {
    let db_name = CString::new("test").unwrap();
    let coll_name = CString::new("test_coll").unwrap();
    let (_bytes, bson) = make_filter_bson();
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_delete_many(
            ptr::null_mut(),
            ptr::null(),
            db_name.as_ptr(),
            coll_name.as_ptr(),
            &bson,
            ptr::null(),
            delete_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst), "Callback must be invoked");
}
