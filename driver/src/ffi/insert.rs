//! FFI insert operations.
//!
//! This module provides C-compatible APIs for document insertion.

#[cfg(test)]
mod tests;

use std::ffi::{c_char, c_void};

use crate::{
    bson::Document,
    coll::options::InsertOneOptions,
    ffi::{
        types::{BsonArray, ContextExt},
        utils::with_err_callback,
    },
};

use super::{
    client::MongoClient,
    error::Error,
    types::{Bson, BsonValue, OperationContext, OwnedBsonValue},
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
    let (coll, raw_doc, options) = with_err_callback!(callback, userdata, || {
        use crate::error::Error;

        // Validate required arguments
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        if document.is_null() {
            return Err(Error::invalid_argument("document cannot be null"));
        }

        let db_name_str = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        let client_ref = &*client;
        let coll = client_ref
            .client
            .database(db_name_str)
            .collection::<Document>(coll_name_str);

        let doc_bson = &*document;
        let raw_doc = doc_bson.as_raw_doc()?;

        // Build InsertOneOptions
        let mut options = InsertOneOptions::default();
        if bypass_document_validation >= 0 {
            options.bypass_document_validation = Some(bypass_document_validation != 0);
        }
        if !comment.is_null() {
            let comment_val = &*comment;
            options.comment = comment_val.to_bson()?;
        }
        options.write_concern = ctx.write_concern();

        Ok((coll, raw_doc, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let result = coll
            .insert_one_raw(raw_doc, Some(options), session_ref)
            .await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            let result = result?;
            let owned_id = OwnedBsonValue::from_bson(&result.inserted_id)?;
            let out = InsertOneResult {
                inserted_id: owned_id,
            };
            callback(userdata, &out, std::ptr::null());
            Ok(())
        });
    });
}

/// Callback for `mongo_insert_many` results.
pub type InsertManyCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const InsertManyResult, // null on error
    error: *const Error,             // null on success
);

/// Result of an insert_many operation.
#[repr(C)]
pub struct InsertManyResult {
    /// The ids of the inserted documents.
    pub inserted_ids: *const InsertedId,
    /// The length of the inserted_ids array.
    pub inserted_ids_len: usize,
}

/// A single inserted ID as part of a batch.
#[repr(C)]
pub struct InsertedId {
    index: usize,
    id: OwnedBsonValue,
}

/// Insert multiple documents into a collection.
#[no_mangle]
pub unsafe extern "C" fn mongo_insert_many(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    documents: BsonArray,
    // options
    bypass_document_validation: i8, // -1 = None, 0 = false, 1 = true
    ordered: bool,
    comment: *const BsonValue,
    // result
    callback: InsertManyCallback,
    userdata: *mut c_void,
) {
    let (coll, options) = with_err_callback!(callback, userdata, || {
        use crate::error::Error;

        // Validate required arguments
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        if documents.is_empty() {
            return Err(Error::invalid_argument("documents cannot be empty"));
        }

        let db_name_str = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        let client_ref = &*client;
        let coll = client_ref
            .client
            .database(db_name_str)
            .collection::<Document>(coll_name_str);

        // Build options
        let mut options = crate::options::InsertManyOptions::default();
        if bypass_document_validation >= 0 {
            options.bypass_document_validation = Some(bypass_document_validation != 0);
        }
        options.ordered = Some(ordered);
        if !comment.is_null() {
            let comment_val = &*comment;
            options.comment = comment_val.to_bson()?;
        }
        options.write_concern = ctx.write_concern();

        Ok((coll, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let raw_docs = documents.to_raw_docs();
        let result = coll
            .insert_many_raw(&raw_docs, Some(options), session_ref)
            .await;

        let userdata = userdata_ptr as *mut c_void;
        let (_inserted_arr, result) = with_err_callback!(callback, userdata, || {
            let crate::results::InsertManyResult { inserted_ids } = result?;
            let mut inserted_arr = vec![];
            for (index, id) in inserted_ids {
                inserted_arr.push(InsertedId {
                    index,
                    id: OwnedBsonValue::from_bson(&id)?,
                });
            }
            let result = InsertManyResult {
                inserted_ids: inserted_arr.as_ptr(),
                inserted_ids_len: inserted_arr.len(),
            };
            Ok((inserted_arr, result))
        });
        callback(userdata, &result, std::ptr::null());
    });
}
