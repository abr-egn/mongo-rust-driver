//! FFI drop operations.
//!
//! This module provides C-compatible APIs for dropping databases and collections.

#[cfg(test)]
mod tests;

use std::{ffi::c_void, os::raw::c_char};

use crate::bson::RawDocumentBuf;

use super::{
    client::MongoClient,
    error::{Error, InvalidArgumentError},
    types::{ContextExt, OperationContext},
    utils::c_char_to_str,
};

/// Callback type for drop operations.
///
/// - `userdata`: The userdata pointer passed to the drop function
/// - `result`: Always null (void operation)
/// - `error`: Error details (null on success)
pub type DropCallback =
    extern "C" fn(userdata: *mut c_void, result: *const (), error: *const Error);

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
    if client.is_null() {
        callback(
            userdata,
            std::ptr::null(),
            &InvalidArgumentError::new("client cannot be null").into(),
        );
        return;
    }

    let db_name_str = match c_char_to_str(db_name) {
        Ok(Some(s)) => s,
        Ok(None) => {
            callback(
                userdata,
                std::ptr::null(),
                &InvalidArgumentError::new("db_name cannot be null").into(),
            );
            return;
        }
        Err(e) => {
            callback(userdata, std::ptr::null(), &Error::from(&e));
            return;
        }
    };

    let client_ref = &*client;
    let db = client_ref.client.database(db_name_str);

    let userdata_ptr = userdata as usize;
    let session_ref = context.session();
    client_ref.runtime.spawn(async move {
        let mut action = db.drop();
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(()) => callback(userdata, std::ptr::null(), std::ptr::null()),
            Err(e) => callback(userdata, std::ptr::null(), &Error::from(&e)),
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
    if client.is_null() {
        callback(
            userdata,
            std::ptr::null(),
            &InvalidArgumentError::new("client cannot be null").into(),
        );
        return;
    }

    let db_name_str = match c_char_to_str(db_name) {
        Ok(Some(s)) => s,
        Ok(None) => {
            callback(
                userdata,
                std::ptr::null(),
                &InvalidArgumentError::new("db_name cannot be null").into(),
            );
            return;
        }
        Err(e) => {
            callback(userdata, std::ptr::null(), &Error::from(&e));
            return;
        }
    };

    let coll_name_str = match c_char_to_str(coll_name) {
        Ok(Some(s)) => s,
        Ok(None) => {
            callback(
                userdata,
                std::ptr::null(),
                &InvalidArgumentError::new("coll_name cannot be null").into(),
            );
            return;
        }
        Err(e) => {
            callback(userdata, std::ptr::null(), &Error::from(&e));
            return;
        }
    };

    let client_ref = &*client;
    let coll = client_ref
        .client
        .database(db_name_str)
        .collection::<RawDocumentBuf>(coll_name_str);

    let userdata_ptr = userdata as usize;
    let session_ref = context.session();
    client_ref.runtime.spawn(async move {
        let mut action = coll.drop();
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(()) => callback(userdata, std::ptr::null(), std::ptr::null()),
            Err(e) => callback(userdata, std::ptr::null(), &Error::from(&e)),
        }
    });
}

