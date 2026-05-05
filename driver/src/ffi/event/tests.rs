use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::ffi::event::{
    FfiCommandFailedEvent, FfiCommandStartedEvent, FfiCommandSucceededEvent,
    MongoCommandEventHandler,
};

#[test]
fn test_handler_struct_null_callbacks() {
    // Verify the struct can be constructed with all-null callbacks
    let handler = MongoCommandEventHandler {
        started: None,
        succeeded: None,
        failed: None,
        userdata: ptr::null_mut(),
    };
    assert!(handler.started.is_none());
    assert!(handler.succeeded.is_none());
    assert!(handler.failed.is_none());
}

#[test]
fn test_event_structs_are_repr_c() {
    // Verify sizes are non-zero (compile-time check that #[repr(C)] works)
    assert!(std::mem::size_of::<FfiCommandStartedEvent>() > 0);
    assert!(std::mem::size_of::<FfiCommandSucceededEvent>() > 0);
    assert!(std::mem::size_of::<FfiCommandFailedEvent>() > 0);
}

#[test]
fn test_build_handler_started_fires() {
    static INVOKED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn on_started(
        _userdata: *mut c_void,
        event: *const FfiCommandStartedEvent,
    ) {
        unsafe {
            let e = &*event;
            assert!(!e.command_name.is_null());
            let name = std::ffi::CStr::from_ptr(e.command_name).to_str().unwrap();
            assert_eq!(name, "ping");
            assert_eq!(e.request_id, 42);
            assert_eq!(e.connection_id, 1);
            assert_eq!(e.connection_server_id, 100);
            assert!(!e.connection_address.is_null());
            let addr = std::ffi::CStr::from_ptr(e.connection_address).to_str().unwrap();
            assert_eq!(addr, "localhost:27017");
            assert!(!e.has_service_id);
            assert!(!e.db.is_null());
            let db = std::ffi::CStr::from_ptr(e.db).to_str().unwrap();
            assert_eq!(db, "testdb");
            assert!(!e.command.is_null());
            assert!((*e.command).len > 0);
        }
        INVOKED.store(true, Ordering::SeqCst);
    }

    let handler = MongoCommandEventHandler {
        started: Some(on_started),
        succeeded: None,
        failed: None,
        userdata: ptr::null_mut(),
    };

    let event_handler = unsafe { crate::ffi::event::build_command_event_handler(&handler) };

    let started_event = crate::event::command::CommandStartedEvent {
        command: crate::bson::doc! { "ping": 1 },
        db: "testdb".to_string(),
        command_name: "ping".to_string(),
        request_id: 42,
        connection: crate::cmap::ConnectionInfo {
            id: 1,
            server_id: Some(100),
            address: crate::options::ServerAddress::Tcp {
                host: "localhost".to_string(),
                port: Some(27017),
            },
        },
        service_id: None,
    };

    event_handler.handle(crate::event::command::CommandEvent::Started(started_event));

    assert!(
        INVOKED.load(Ordering::SeqCst),
        "started callback should have fired"
    );
}

#[test]
fn test_build_handler_null_started_no_crash() {
    // When started is None, firing a Started event must not crash.
    let handler = MongoCommandEventHandler {
        started: None,
        succeeded: None,
        failed: None,
        userdata: ptr::null_mut(),
    };

    let event_handler = unsafe { crate::ffi::event::build_command_event_handler(&handler) };

    let started_event = crate::event::command::CommandStartedEvent {
        command: crate::bson::doc! { "ping": 1 },
        db: "admin".to_string(),
        command_name: "ping".to_string(),
        request_id: 1,
        connection: crate::cmap::ConnectionInfo {
            id: 1,
            server_id: None,
            address: crate::options::ServerAddress::Tcp {
                host: "localhost".to_string(),
                port: Some(27017),
            },
        },
        service_id: None,
    };

    // Must not panic or crash
    event_handler.handle(crate::event::command::CommandEvent::Started(started_event));
}
