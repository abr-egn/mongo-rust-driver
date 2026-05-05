//! FFI delete operations.

#[cfg(test)]
mod tests;

use std::ffi::{c_char, c_void};

use crate::ffi::{
    client::MongoClient,
    error::Error,
    types::{Bson, BsonValue, ContextExt, OperationContext},
    utils::{c_char_to_str, c_char_to_string, with_err_callback},
};

/// Callback for delete operation results.
pub type DeleteCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const DeleteResult,
    error: *const Error,
);

/// Result of a delete_one or delete_many operation.
#[repr(C)]
pub struct DeleteResult {
    pub deleted_count: u64,
}

/// Options for delete operations.
///
/// All pointer fields are nullable (null = not set).
/// `write_concern` comes from `OperationContext`, not this struct.
#[repr(C)]
pub struct DeleteOptions {
    /// Collation as a serialized BSON document. Nullable.
    pub collation: *const Bson,
    /// Index hint by name. Nullable. Takes precedence over `hint_keys`.
    pub hint_name: *const c_char,
    /// Index hint by key pattern as BSON document. Nullable.
    pub hint_keys: *const Bson,
    /// Variables for MQL expressions (`$let`). Nullable BSON document.
    pub let_vars: *const Bson,
    /// Comment BSON value. Nullable.
    pub comment: *const BsonValue,
}

unsafe fn parse_delete_options(
    opts: *const DeleteOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<crate::coll::options::DeleteOptions> {
    let mut options = crate::coll::options::DeleteOptions::default();
    options.write_concern = ctx.write_concern();

    if opts.is_null() {
        return Ok(options);
    }
    let opts = &*opts;

    options.hint = if let Some(name) = c_char_to_string(opts.hint_name)? {
        Some(crate::options::Hint::Name(name))
    } else if let Some(keys) = Bson::to_doc(opts.hint_keys)? {
        Some(crate::options::Hint::Keys(keys))
    } else {
        None
    };

    if let Some(doc) = Bson::to_doc(opts.collation)? {
        options.collation = Some(crate::bson_compat::deserialize_from_document(doc)?);
    }

    options.let_vars = Bson::to_doc(opts.let_vars)?;

    if !opts.comment.is_null() {
        options.comment = (&*opts.comment).to_bson()?;
    }

    Ok(options)
}

/// Delete up to one document matching `filter`.
///
/// # Safety
///
/// - `client` must be a valid pointer from `mongo_client_new`
/// - `db_name`, `coll_name` must be valid null-terminated C strings
/// - `filter` must be a valid BSON document pointer
/// - `opts` may be null (use defaults)
/// - `ctx` may be null (no session/write concern)
#[no_mangle]
pub unsafe extern "C" fn mongo_delete_one(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    opts: *const DeleteOptions,
    callback: DeleteCallback,
    userdata: *mut c_void,
) {
    let (coll, filter_doc, options) = with_err_callback!(callback, userdata, || {
        use crate::error::Error;
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        if filter.is_null() {
            return Err(Error::invalid_argument("filter cannot be null"));
        }
        let db = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        let coll = (*client).client
            .database(db)
            .collection::<crate::bson::Document>(coll_name_str);
        let filter_doc: crate::bson::Document = (&*filter).as_raw_doc()?.try_into()?;
        let options = parse_delete_options(opts, ctx)?;
        Ok((coll, filter_doc, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll.delete_one(filter_doc).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            let r = result?;
            callback(userdata, &DeleteResult { deleted_count: r.deleted_count }, std::ptr::null());
            Ok(())
        });
    });
}

/// Delete all documents matching `filter`.
///
/// # Safety
///
/// Same safety requirements as `mongo_delete_one`.
#[no_mangle]
pub unsafe extern "C" fn mongo_delete_many(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    opts: *const DeleteOptions,
    callback: DeleteCallback,
    userdata: *mut c_void,
) {
    let (coll, filter_doc, options) = with_err_callback!(callback, userdata, || {
        use crate::error::Error;
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        if filter.is_null() {
            return Err(Error::invalid_argument("filter cannot be null"));
        }
        let db = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        let coll = (*client).client
            .database(db)
            .collection::<crate::bson::Document>(coll_name_str);
        let filter_doc: crate::bson::Document = (&*filter).as_raw_doc()?.try_into()?;
        let options = parse_delete_options(opts, ctx)?;
        Ok((coll, filter_doc, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll.delete_many(filter_doc).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            let r = result?;
            callback(userdata, &DeleteResult { deleted_count: r.deleted_count }, std::ptr::null());
            Ok(())
        });
    });
}
