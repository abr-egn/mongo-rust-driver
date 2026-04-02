use std::{
    ffi::{c_void, CString},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::ffi::{
    client::{mongo_client_destroy, mongo_client_new},
    count::{mongo_count_documents, mongo_estimated_document_count},
    error::{Error, ErrorType},
    types::ConnectionSettings,
};

extern "C" fn count_callback(userdata: *mut c_void, _count: u64, error: *const Error) {
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
fn test_count_documents_null_client() {
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_count_documents(
            ptr::null_mut(),
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            ptr::null(),
            ptr::null(),
            count_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst));
}

#[test]
fn test_count_documents_null_db_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let coll = CString::new("c").unwrap();
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_count_documents(
            client,
            ptr::null(),
            ptr::null(),
            coll.as_ptr(),
            ptr::null(),
            ptr::null(),
            count_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_count_documents_null_coll_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let db = CString::new("test").unwrap();
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_count_documents(
            client,
            ptr::null(),
            db.as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            count_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}

#[test]
fn test_estimated_document_count_null_client() {
    let db = CString::new("test").unwrap();
    let coll = CString::new("c").unwrap();
    let invoked = AtomicBool::new(false);
    unsafe {
        mongo_estimated_document_count(
            ptr::null_mut(),
            ptr::null(),
            db.as_ptr(),
            coll.as_ptr(),
            ptr::null(),
            count_callback,
            &invoked as *const _ as *mut c_void,
        );
    }
    assert!(invoked.load(Ordering::SeqCst));
}

#[test]
fn test_estimated_document_count_null_db_name() {
    let hosts = CString::new("localhost:27017").unwrap();
    let coll = CString::new("c").unwrap();
    let settings = make_conn_settings(&hosts);
    let invoked = AtomicBool::new(false);
    unsafe {
        let client = mongo_client_new(&settings, ptr::null(), ptr::null(), ptr::null(), ptr::null_mut());
        assert!(!client.is_null());
        mongo_estimated_document_count(
            client,
            ptr::null(),
            ptr::null(),
            coll.as_ptr(),
            ptr::null(),
            count_callback,
            &invoked as *const _ as *mut c_void,
        );
        assert!(invoked.load(Ordering::SeqCst));
        mongo_client_destroy(client);
    }
}
