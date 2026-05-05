//! FFI command monitoring types.
//!
//! This module provides C-compatible types for monitoring command events.

#[cfg(test)]
mod tests;

use std::{ffi::{c_void, CString}, os::raw::c_char};

use crate::{
    bson::oid::ObjectId,
    event::{
        EventHandler,
        command::{CommandEvent, CommandFailedEvent, CommandStartedEvent, CommandSucceededEvent},
    },
    options::ServerAddress,
};

use super::{error::Error, types::{Bson, OwnedBson}};

/// C-visible representation of a CommandStartedEvent.
///
/// All pointer fields (`command_name`, `db`, `connection_address`, `command`) are valid
/// only for the duration of the callback invocation. Copy any data you need before returning.
#[repr(C)]
pub struct FfiCommandStartedEvent {
    /// The name of the command, e.g. "find" or "insert". Valid during callback.
    pub command_name: *const c_char,
    /// Driver-generated request identifier. Matches the corresponding succeeded/failed event.
    pub request_id: i32,
    /// Driver-generated connection identifier.
    pub connection_id: u32,
    /// Server-generated connection identifier. -1 if not provided by the server.
    pub connection_server_id: i64,
    /// Server address as "host:port" or "/unix/socket/path". Valid during callback.
    pub connection_address: *const c_char,
    /// ObjectId bytes for the service_id, if the deployment is a load balancer.
    pub service_id: [u8; 12],
    /// True if service_id is populated; false if the deployment is not a load balancer.
    pub has_service_id: bool,
    /// Database name. Valid during callback.
    pub db: *const c_char,
    /// The command document as raw BSON. Valid during callback.
    pub command: *const Bson,
}

/// C-visible representation of a CommandSucceededEvent.
///
/// All pointer fields are valid only for the duration of the callback invocation.
#[repr(C)]
pub struct FfiCommandSucceededEvent {
    /// The name of the command.
    pub command_name: *const c_char,
    /// Driver-generated request identifier.
    pub request_id: i32,
    /// Driver-generated connection identifier.
    pub connection_id: u32,
    /// Server-generated connection identifier. -1 if not set.
    pub connection_server_id: i64,
    /// Server address. Valid during callback.
    pub connection_address: *const c_char,
    /// ObjectId bytes for the service_id.
    pub service_id: [u8; 12],
    /// True if service_id is populated.
    pub has_service_id: bool,
    /// Total round-trip duration of the command in nanoseconds.
    pub duration_nanos: u64,
    /// The server's reply document. Valid during callback.
    pub reply: *const Bson,
}

/// C-visible representation of a CommandFailedEvent.
///
/// All pointer fields are valid only for the duration of the callback invocation.
#[repr(C)]
pub struct FfiCommandFailedEvent {
    /// The name of the command.
    pub command_name: *const c_char,
    /// Driver-generated request identifier.
    pub request_id: i32,
    /// Driver-generated connection identifier.
    pub connection_id: u32,
    /// Server-generated connection identifier. -1 if not set.
    pub connection_server_id: i64,
    /// Server address. Valid during callback.
    pub connection_address: *const c_char,
    /// ObjectId bytes for the service_id.
    pub service_id: [u8; 12],
    /// True if service_id is populated.
    pub has_service_id: bool,
    /// Total round-trip duration in nanoseconds.
    pub duration_nanos: u64,
    /// The error that caused the command to fail. Valid during callback; do not free.
    pub failure: *const Error,
}

/// C-compatible command event handler.
///
/// Pass to `mongo_client_new` to receive command monitoring events. The struct may be
/// stack-allocated; `mongo_client_new` copies the function pointers and userdata before
/// returning.
///
/// Any callback may be null — that event type is silently ignored.
///
/// # Thread Safety
///
/// Callbacks may be invoked concurrently from multiple threads (one per in-flight command).
/// The `userdata` object must be thread-safe. All function pointers must remain valid for
/// the lifetime of the `MongoClient`.
#[repr(C)]
pub struct MongoCommandEventHandler {
    /// Called when a command is sent to the server. May be null.
    pub started: Option<unsafe extern "C" fn(*mut c_void, *const FfiCommandStartedEvent)>,
    /// Called when a command completes successfully. May be null.
    pub succeeded: Option<unsafe extern "C" fn(*mut c_void, *const FfiCommandSucceededEvent)>,
    /// Called when a command fails. May be null.
    pub failed: Option<unsafe extern "C" fn(*mut c_void, *const FfiCommandFailedEvent)>,
    /// Passed unchanged to every callback. May be null.
    pub userdata: *mut c_void,
}

// Safety: fn pointers are Send+Sync; userdata thread-safety is the caller's responsibility
// (documented in the struct's safety contract).
unsafe impl Send for MongoCommandEventHandler {}
unsafe impl Sync for MongoCommandEventHandler {}

/// Build a Rust `EventHandler<CommandEvent>` from an FFI handler reference.
///
/// Called only after a null-check on the incoming pointer. Copies fn pointers and
/// userdata out of the C struct into a closure so the C struct does not need to
/// outlive the client.
///
/// # Safety
///
/// `handler` must be a valid reference for the duration of this call. The returned
/// `EventHandler` captures the fn pointers and userdata by value and is safe to use
/// for the lifetime of the client.
pub(super) unsafe fn build_command_event_handler(
    handler: &MongoCommandEventHandler,
) -> EventHandler<CommandEvent> {
    let started = handler.started;
    let succeeded = handler.succeeded;
    let failed = handler.failed;
    // Convert to usize so the closure is Send (raw pointers are not Send).
    // Safety: reconverted to *mut c_void inside the closure on each invocation.
    let userdata = handler.userdata as usize;

    EventHandler::callback(move |ev| {
        let userdata = userdata as *mut c_void;
        match ev {
            CommandEvent::Started(e) => {
                let Some(cb) = started else { return };
                unsafe { fire_started(cb, userdata, &e); }
            }
            CommandEvent::Succeeded(e) => {
                let Some(cb) = succeeded else { return };
                unsafe { fire_succeeded(cb, userdata, &e); }
            }
            CommandEvent::Failed(e) => {
                let Some(cb) = failed else { return };
                unsafe { fire_failed(cb, userdata, &e); }
            }
        }
    })
}

unsafe fn fire_started(
    cb: unsafe extern "C" fn(*mut c_void, *const FfiCommandStartedEvent),
    userdata: *mut c_void,
    e: &CommandStartedEvent,
) {
    let command_name = CString::new(e.command_name.as_str()).unwrap();
    let db = CString::new(e.db.as_str()).unwrap();
    let address = CString::new(format_address(&e.connection.address)).unwrap();
    let command_bson = OwnedBson::from_doc(&e.command);
    let (service_id, has_service_id) = service_id_bytes(e.service_id.as_ref());

    let ffi_event = FfiCommandStartedEvent {
        command_name: command_name.as_ptr(),
        request_id: e.request_id,
        connection_id: e.connection.id,
        connection_server_id: e.connection.server_id.unwrap_or(-1),
        connection_address: address.as_ptr(),
        service_id,
        has_service_id,
        db: db.as_ptr(),
        command: &command_bson.0 as *const Bson,
    };

    unsafe { cb(userdata, &ffi_event) };
}

unsafe fn fire_succeeded(
    cb: unsafe extern "C" fn(*mut c_void, *const FfiCommandSucceededEvent),
    userdata: *mut c_void,
    e: &CommandSucceededEvent,
) {
    let command_name = CString::new(e.command_name.as_str()).unwrap();
    let address = CString::new(format_address(&e.connection.address)).unwrap();
    let reply_bson = OwnedBson::from_doc(&e.reply);
    let (service_id, has_service_id) = service_id_bytes(e.service_id.as_ref());

    let ffi_event = FfiCommandSucceededEvent {
        command_name: command_name.as_ptr(),
        request_id: e.request_id,
        connection_id: e.connection.id,
        connection_server_id: e.connection.server_id.unwrap_or(-1),
        connection_address: address.as_ptr(),
        service_id,
        has_service_id,
        duration_nanos: e.duration.as_nanos() as u64,
        reply: &reply_bson.0 as *const Bson,
    };

    unsafe { cb(userdata, &ffi_event) };
}

unsafe fn fire_failed(
    cb: unsafe extern "C" fn(*mut c_void, *const FfiCommandFailedEvent),
    userdata: *mut c_void,
    e: &CommandFailedEvent,
) {
    let command_name = CString::new(e.command_name.as_str()).unwrap();
    let address = CString::new(format_address(&e.connection.address)).unwrap();
    let (service_id, has_service_id) = service_id_bytes(e.service_id.as_ref());
    let failure = Error::from(&e.failure);

    let ffi_event = FfiCommandFailedEvent {
        command_name: command_name.as_ptr(),
        request_id: e.request_id,
        connection_id: e.connection.id,
        connection_server_id: e.connection.server_id.unwrap_or(-1),
        connection_address: address.as_ptr(),
        service_id,
        has_service_id,
        duration_nanos: e.duration.as_nanos() as u64,
        failure: &failure as *const Error,
    };

    unsafe { cb(userdata, &ffi_event) };
}

fn format_address(address: &ServerAddress) -> String {
    address.to_string()
}

fn service_id_bytes(oid: Option<&ObjectId>) -> ([u8; 12], bool) {
    match oid {
        Some(id) => (id.bytes(), true),
        None => ([0u8; 12], false),
    }
}
