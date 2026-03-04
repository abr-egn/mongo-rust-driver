//! FFI command operations.
//!
//! This module provides C-compatible APIs for database commands.

#[cfg(test)]
mod tests;

use std::{ffi::c_void, os::raw::c_char};

use crate::bson::RawDocumentBuf;

use super::{
    client::MongoClient,
    error::{Error, InvalidArgumentError},
    types::{Bson, ClientSession, OperationContext, OwnedBson},
    utils::c_char_to_str,
};

/// Callback type for run_command results.
///
/// - `userdata`: The userdata pointer passed to `mongo_run_command`
/// - `result`: The command result as a BSON document (null on error)
/// - `error`: Error details (null on success)
///
/// Result and error pointers are valid only during callback invocation.
/// Copy any data you need before returning from the callback.
pub type RunCommandCallback =
    extern "C" fn(userdata: *mut c_void, result: *const OwnedBson, error: *const Error);

/// Run a database command asynchronously.
///
/// # Safety
///
/// - `client` must be a valid pointer returned from `mongo_client_new`
/// - `db_name` must be a valid null-terminated C string
/// - `command` must be a valid pointer to a BSON document
/// - `command` must remain valid until the callback is invoked
/// - `session` can be null (no session) or a valid pointer to a Session
/// - `callback` must be a valid function pointer
/// - `userdata` can be any value and will be passed to the callback
///
/// The `read_preference_mode` parameter:
/// - 0 = Primary
/// - 1 = PrimaryPreferred
/// - 2 = Secondary
/// - 3 = SecondaryPreferred
/// - 4 = Nearest
/// - 255 = Not set (use default)
#[no_mangle]
pub unsafe extern "C" fn mongo_run_command(
    client: *mut MongoClient,
    context: *mut OperationContext,
    db_name: *const c_char,
    command: *const Bson,
    callback: RunCommandCallback,
    userdata: *mut c_void,
) {
    if client.is_null() {
        let error = Error::from(InvalidArgumentError::new("client cannot be null"));
        callback(userdata, std::ptr::null(), &error);
        return;
    }

    if command.is_null() {
        let error = Error::from(InvalidArgumentError::new("command cannot be null"));
        callback(userdata, std::ptr::null(), &error);
        return;
    }

    let db_name_str = match c_char_to_str(db_name) {
        Ok(Some(s)) => s,
        Ok(None) => {
            let error = Error::from(InvalidArgumentError::new("db_name cannot be null"));
            callback(userdata, std::ptr::null(), &error);
            return;
        }
        Err(e) => {
            let error = Error::from(&e);
            callback(userdata, std::ptr::null(), &error);
            return;
        }
    };

    let selection_criteria = if context.is_null() {
        None
    } else {
        let context = &*context;
        if context.read_preference.is_null() {
            None
        } else {
            let read_pref = &*context.read_preference;
            Some(crate::selection_criteria::SelectionCriteria::ReadPreference(read_pref.clone()))
        }
    };

    let command_bson = &*command;
    let command_bytes = std::slice::from_raw_parts(command_bson.data, command_bson.len);

    let command_doc = match RawDocumentBuf::from_bytes(command_bytes.to_vec()) {
        Ok(doc) => doc,
        Err(e) => {
            let error = Error::from(InvalidArgumentError::new(&format!(
                "Invalid BSON command: {}",
                e
            )));
            callback(userdata, std::ptr::null(), &error);
            return;
        }
    };

    let client_ref = &*client;
    let db = client_ref.client.database(db_name_str);

    let userdata_ptr = userdata as usize;
    let session_ref: Option<&'static mut ClientSession> = if context.is_null() {
        None
    } else {
        let context = &*context;
        if context.session.is_null() {
            None
        } else {
            Some(&mut *context.session)
        }
    };

    client_ref.runtime.spawn(async move {
        let mut action = db.run_raw_command(command_doc);
        if let Some(criteria) = selection_criteria {
            action = action.selection_criteria(criteria);
        }
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(doc) => {
                let result_bson = OwnedBson::from_doc(&doc);
                callback(userdata, &result_bson, std::ptr::null());
            }
            Err(e) => {
                let error = Error::from(&e);
                callback(userdata, std::ptr::null(), &error);
            }
        }
    });
}
