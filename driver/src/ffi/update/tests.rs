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
        types::{Bson, BsonArray, ConnectionSettings},
        update::{mongo_replace_one, mongo_update_one, UpdateResult},
    },
};

extern "C" fn update_callback(
    userdata: *mut c_void,
    result: *const UpdateResult,
    error: *const Error,
) {
    unsafe {
        let callback_invoked = &*(userdata as *const AtomicBool);
        callback_invoked.store(true, Ordering::SeqCst);
        if !error.is_null() {
            let error_ref = &*error;
            assert_eq!(error_ref.error_type, ErrorType::InvalidArgument as u8);
            assert!(result.is_null());
        }
    }
}

fn make_bson(d: crate::bson::Document) -> (Vec<u8>, Bson) {
    let mut bytes = Vec::new();
    d.to_writer(&mut bytes).unwrap();
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
fn test_update_one_null_client() {
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let (_fb, filter) = make_bson(doc! { "_id": 1 });
    let (_ub, update) = make_bson(doc! { "$set": { "x": 1 } });
    let empty_pipeline = BsonArray { data: ptr::null(), len: 0 };
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_update_one(
            ptr::null_mut(),
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            &filter,
            &update,
            empty_pipeline,
            ptr::null(),
            update_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst));
}

#[test]
fn test_update_one_null_filter() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let (_ub, update) = make_bson(doc! { "$set": { "x": 1 } });
    let empty_pipeline = BsonArray { data: ptr::null(), len: 0 };
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_update_one(
            client,
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            ptr::null(),
            &update,
            empty_pipeline,
            ptr::null(),
            update_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_update_one_both_update_null() {
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
        mongo_update_one(
            client,
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            &filter,
            ptr::null(),
            empty_pipeline,
            ptr::null(),
            update_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_update_one_both_update_set() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let (_fb, filter) = make_bson(doc! { "_id": 1 });
    let (_ub, update) = make_bson(doc! { "$set": { "x": 1 } });
    let (_pb, pipeline_doc) = make_bson(doc! { "$set": { "x": 2 } });
    let pipeline_ptr = pipeline_doc.data;
    let pipeline_ptrs = [pipeline_ptr];
    let pipeline = BsonArray { data: pipeline_ptrs.as_ptr(), len: 1 };
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_update_one(
            client,
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            &filter,
            &update,
            pipeline,
            ptr::null(),
            update_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_replace_one_null_client() {
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let (_fb, filter) = make_bson(doc! { "_id": 1 });
    let (_rb, replacement) = make_bson(doc! { "x": 2 });
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_replace_one(
            ptr::null_mut(),
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            &filter,
            &replacement,
            ptr::null(),
            update_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst));
}
