//! FFI drop operations.
//!
//! This module provides C-compatible APIs for dropping databases and collections.

#[cfg(test)]
mod tests;

use std::{ffi::c_void, os::raw::c_char};

use crate::bson::RawDocumentBuf;

use super::{
    client::MongoClient,
    error::Error,
    types::{ContextExt, OperationContext},
    utils::{c_char_to_str, with_void_err_callback},
};

/// Callback type for drop operations.
///
/// - `userdata`: The userdata pointer passed to the drop function
/// - `error`: Error details (null on success)
pub type DropCallback = extern "C" fn(userdata: *mut c_void, error: *const Error);

/// Drop a database asynchronously.
///
/// # Safety
///
/// - `client` must be a valid pointer returned from `mongo_client_new`
/// - `db_name` must be a valid null-terminated C string
/// - `callback` must be a valid function pointer
/// - `userdata` can be any value and will be passed to the callback
#[no_mangle]
pub unsafe extern "C" fn mongo_drop_database(
    client: *mut MongoClient,
    context: *mut OperationContext,
    db_name: *const c_char,
    callback: DropCallback,
    userdata: *mut c_void,
) {
    use crate::error::Error;

    let db = with_void_err_callback!(callback, userdata, || {
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        let db_name_str = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        Ok((*client).client.database(db_name_str))
    });

    let client_ref = &*client;
    let userdata_ptr = userdata as usize;
    let session_ref = context.session();
    let write_concern = context.write_concern();
    client_ref.runtime.spawn(async move {
        let mut action = db.drop();
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        if let Some(wc) = write_concern {
            action = action.write_concern(wc);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(()) => callback(userdata, std::ptr::null()),
            Err(e) => callback(userdata, &super::error::Error::from(&e)),
        }
    });
}

/// Drop a collection asynchronously.
///
/// # Safety
///
/// - `client` must be a valid pointer returned from `mongo_client_new`
/// - `db_name` must be a valid null-terminated C string
/// - `coll_name` must be a valid null-terminated C string
/// - `callback` must be a valid function pointer
/// - `userdata` can be any value and will be passed to the callback
#[no_mangle]
pub unsafe extern "C" fn mongo_drop_collection(
    client: *mut MongoClient,
    context: *mut OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    callback: DropCallback,
    userdata: *mut c_void,
) {
    use crate::error::Error;

    let coll = with_void_err_callback!(callback, userdata, || {
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        let db_name_str = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        Ok((*client)
            .client
            .database(db_name_str)
            .collection::<RawDocumentBuf>(coll_name_str))
    });

    let client_ref = &*client;
    let userdata_ptr = userdata as usize;
    let session_ref = context.session();
    let write_concern = context.write_concern();
    client_ref.runtime.spawn(async move {
        let mut action = coll.drop();
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        if let Some(wc) = write_concern {
            action = action.write_concern(wc);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(()) => callback(userdata, std::ptr::null()),
            Err(e) => callback(userdata, &super::error::Error::from(&e)),
        }
    });
}
