use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    bson::doc,
    ffi::{
        client::{mongo_client_destroy, mongo_client_new},
        error::{Error, ErrorType},
        find_one::{
            mongo_find_one_and_delete, mongo_find_one_and_replace, mongo_find_one_and_update,
        },
        types::{Bson, BsonArray, ConnectionSettings, OwnedBson},
    },
};

extern "C" fn find_one_callback(
    userdata: *mut c_void,
    _result: *const OwnedBson,
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

fn make_bson(d: crate::bson::Document) -> (Vec<u8>, Bson) {
    let mut bytes = Vec::new();
    d.to_writer(&mut bytes).unwrap();
    (bytes.clone(), Bson { data: bytes.as_ptr(), len: bytes.len() })
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
fn test_find_one_and_delete_null_client() {
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let (_fb, filter) = make_bson(doc! { "_id": 1 });
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_find_one_and_delete(
            ptr::null_mut(),
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            &filter,
            ptr::null(),
            find_one_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst));
}

#[test]
fn test_find_one_and_delete_null_filter() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_find_one_and_delete(
            client,
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            ptr::null(),
            ptr::null(),
            find_one_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_find_one_and_update_both_update_null() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let (_fb, filter) = make_bson(doc! { "_id": 1 });
    let empty_pipeline = BsonArray { data: ptr::null(), len: 0 };
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_find_one_and_update(
            client,
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            &filter,
            ptr::null(),
            empty_pipeline,
            ptr::null(),
            find_one_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_find_one_and_replace_null_replacement() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let (_fb, filter) = make_bson(doc! { "_id": 1 });
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_find_one_and_replace(
            client,
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            &filter,
            ptr::null(),
            ptr::null(),
            find_one_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}
