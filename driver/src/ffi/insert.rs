//! FFI insert operations.
//!
//! This module provides C-compatible APIs for document insertion.

#[cfg(test)]
mod tests;

use std::ffi::{c_char, c_void};

use crate::{
    bson::{Document, RawDocument},
    coll::options::InsertOneOptions,
};

use super::{
    client::MongoClient,
    error::{Error, InvalidArgumentError},
    types::{Bson, BsonValue, ClientSession, OperationContext, OwnedBsonValue},
    utils::c_char_to_str,
};

/// Callback type for insert_one results.
///
/// - `userdata`: The userdata pointer passed to `mongo_insert_one`
/// - `result`: The insert result (null on error)
/// - `error`: Error details (null on success)
///
/// Result and error pointers are valid only during callback invocation.
pub type InsertOneCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const InsertOneResult, // null on error
    error: *const Error,            // null on success
);

/// Result of an insert_one operation.
#[repr(C)]
pub struct InsertOneResult {
    /// The `_id` of the inserted document. Any BSON type (ObjectId, String, Int, etc.)
    pub inserted_id: OwnedBsonValue,
}

/// Insert a single document asynchronously.
///
/// # Safety
///
/// - `client` must be a valid pointer returned from `mongo_client_new`
/// - `db_name` must be a valid null-terminated C string
/// - `coll_name` must be a valid null-terminated C string
/// - `document` must be a valid pointer to a BSON document
/// - `ctx` can be null (no session/options) or a valid pointer to OperationContext
/// - `comment` can be null or a valid pointer to a BsonValue
/// - `callback` must be a valid function pointer
/// - `userdata` can be any value and will be passed to the callback
#[no_mangle]
pub unsafe extern "C" fn mongo_insert_one(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    document: *const Bson,
    bypass_document_validation: i8, // -1 = None, 0 = false, 1 = true
    comment: *const BsonValue,
    callback: InsertOneCallback,
    userdata: *mut c_void,
) {
    // Validate required arguments
    if client.is_null() {
        let error = Error::from(InvalidArgumentError::new("client cannot be null"));
        callback(userdata, std::ptr::null(), &error);
        return;
    }

    if document.is_null() {
        let error = Error::from(InvalidArgumentError::new("document cannot be null"));
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

    let coll_name_str = match c_char_to_str(coll_name) {
        Ok(Some(s)) => s,
        Ok(None) => {
            let error = Error::from(InvalidArgumentError::new("coll_name cannot be null"));
            callback(userdata, std::ptr::null(), &error);
            return;
        }
        Err(e) => {
            let error = Error::from(&e);
            callback(userdata, std::ptr::null(), &error);
            return;
        }
    };

    // Parse the document
    let doc_bson = &*document;
    let doc_bytes = std::slice::from_raw_parts(doc_bson.data, doc_bson.len);
    let raw_doc = match RawDocument::from_bytes(doc_bytes) {
        Ok(doc) => doc,
        Err(e) => {
            let error = Error::from(InvalidArgumentError::new(&format!(
                "Invalid BSON document: {}",
                e
            )));
            callback(userdata, std::ptr::null(), &error);
            return;
        }
    };

    // Build InsertOneOptions
    let mut options = InsertOneOptions::default();

    if bypass_document_validation >= 0 {
        options.bypass_document_validation = Some(bypass_document_validation != 0);
    }

    // Parse comment if provided
    if !comment.is_null() {
        let comment_val = &*comment;
        match comment_val.to_bson() {
            Ok(comment) => options.comment = comment,
            Err(e) => {
                callback(userdata, std::ptr::null(), &Error::from(&e));
                return;
            }
        }
    }

    // Extract write concern from context
    if !ctx.is_null() {
        let context = &*ctx;
        if !context.write_concern.is_null() {
            options.write_concern = Some((*context.write_concern).clone());
        }
    }

    // Get session reference if provided
    let session_ref: Option<&'static mut ClientSession> = if ctx.is_null() {
        None
    } else {
        let context = &*ctx;
        if context.session.is_null() {
            None
        } else {
            Some(&mut *context.session)
        }
    };

    let client_ref = &*client;
    let coll = client_ref
        .client
        .database(db_name_str)
        .collection::<Document>(coll_name_str);

    let userdata_ptr = userdata as usize;

    client_ref.runtime.spawn(async move {
        let result = coll
            .insert_one_raw(raw_doc, Some(options), session_ref)
            .await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(insert_result) => {
                let owned_id = OwnedBsonValue::from_bson(&insert_result.inserted_id);
                let ffi_result = InsertOneResult {
                    inserted_id: owned_id,
                };
                callback(userdata, &ffi_result, std::ptr::null());
            }
            Err(e) => {
                let error = Error::from(&e);
                callback(userdata, std::ptr::null(), &error);
            }
        }
    });
}
