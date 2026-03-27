//! FFI command operations.
//!
//! This module provides C-compatible APIs for database commands.

#[cfg(test)]
mod tests;

use std::{ffi::c_void, os::raw::c_char};

use crate::{bson::RawDocumentBuf, ffi::utils::with_err_callback, options::RunCommandOptions};

use super::{
    client::MongoClient,
    error::Error,
    types::{Bson, ContextExt, OperationContext, OwnedBson},
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
    let (db, options, command_doc) = with_err_callback!(callback, userdata, || {
        use crate::error::Error;

        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        if command.is_null() {
            return Err(Error::invalid_argument("command cannot be null"));
        }

        let db_name_str = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let client_ref = &*client;
        let db = client_ref.client.database(db_name_str);

        let mut options = RunCommandOptions::default();
        options.selection_criteria = context
            .read_preference()?
            .map(crate::selection_criteria::SelectionCriteria::ReadPreference);

        let command_bson = &*command;
        let command_bytes = std::slice::from_raw_parts(command_bson.data, command_bson.len);
        let command_doc = RawDocumentBuf::from_bytes(command_bytes.to_vec())?;

        Ok((db, options, command_doc))
    });

    let userdata_ptr = userdata as usize;
    let session_ref = context.session();
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = db.run_raw_command(command_doc).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            let result = result?;
            callback(userdata, &OwnedBson::from_doc(&result), std::ptr::null());
            Ok(())
        });
    });
}
