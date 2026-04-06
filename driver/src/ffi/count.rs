//! FFI count operations.

#[cfg(test)]
mod tests;

use std::ffi::{c_char, c_void};

use crate::ffi::{
    client::MongoClient,
    error::Error,
    types::{Bson, BsonValue, ContextExt, OperationContext},
    utils::{c_char_to_str, c_char_to_string, i64_to_duration_ms},
};

/// Callback for count results.
/// On error: `count` is 0 and `error` is non-null.
/// On success: `count` is the document count and `error` is null.
pub type CountCallback = extern "C" fn(userdata: *mut c_void, count: u64, error: *const Error);

/// Options for `count_documents`.
///
/// All pointer fields are nullable (null = not set).
#[repr(C)]
pub struct CountOptions {
    /// Collation as a serialized BSON document. Nullable.
    pub collation: *const Bson,
    /// Index hint by name. Nullable. Takes precedence over `hint_keys`.
    pub hint_name: *const c_char,
    /// Index hint by key pattern as BSON document. Nullable.
    pub hint_keys: *const Bson,
    /// Maximum number of documents to count. -1 = not set.
    pub limit: i64,
    /// Number of documents to skip. -1 = not set.
    pub skip: i64,
    /// Maximum time in milliseconds. -1 = not set.
    pub max_time_ms: i64,
    /// Comment BSON value. Nullable.
    pub comment: *const BsonValue,
}

/// Options for `estimated_document_count`.
///
/// All pointer fields are nullable (null = not set).
#[repr(C)]
pub struct EstimatedDocumentCountOptions {
    /// Maximum time in milliseconds. -1 = not set.
    pub max_time_ms: i64,
    /// Comment BSON value. Nullable.
    pub comment: *const BsonValue,
}

unsafe fn parse_count_options(
    opts: *const CountOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<crate::coll::options::CountOptions> {
    let mut options = crate::coll::options::CountOptions::default();
    options.read_concern = ctx.read_concern();
    options.selection_criteria = ctx
        .read_preference()
        .map(crate::options::SelectionCriteria::ReadPreference);

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

    if opts.limit >= 0 {
        options.limit = Some(opts.limit as u64);
    }

    if opts.skip >= 0 {
        options.skip = Some(opts.skip as u64);
    }

    options.max_time = i64_to_duration_ms(opts.max_time_ms);

    if !opts.comment.is_null() {
        options.comment = (&*opts.comment).to_bson()?;
    }

    Ok(options)
}

unsafe fn parse_estimated_options(
    opts: *const EstimatedDocumentCountOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<crate::coll::options::EstimatedDocumentCountOptions> {
    let mut options = crate::coll::options::EstimatedDocumentCountOptions::default();
    options.read_concern = ctx.read_concern();
    options.selection_criteria = ctx
        .read_preference()
        .map(crate::options::SelectionCriteria::ReadPreference);

    if opts.is_null() {
        return Ok(options);
    }
    let opts = &*opts;

    options.max_time = i64_to_duration_ms(opts.max_time_ms);

    if !opts.comment.is_null() {
        options.comment = (&*opts.comment).to_bson()?;
    }

    Ok(options)
}

/// Count documents matching `filter` in the specified collection.
///
/// If `filter` is null, all documents are counted (equivalent to `{}`).
///
/// # Safety
///
/// - `client` must be a valid pointer from `mongo_client_new`
/// - `db_name`, `coll_name` must be valid null-terminated C strings
/// - `filter` may be null (counts all documents) or a valid BSON document pointer
/// - `opts` may be null (use defaults)
/// - `ctx` may be null (no session/read concern)
#[no_mangle]
pub unsafe extern "C" fn mongo_count_documents(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    opts: *const CountOptions,
    callback: CountCallback,
    userdata: *mut c_void,
) {
    let setup = (|| -> crate::error::Result<_> {
        use crate::error::Error;
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        let db = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        let coll = (*client)
            .client
            .database(db)
            .collection::<crate::bson::Document>(coll_name_str);
        let filter_doc: crate::bson::Document = if filter.is_null() {
            crate::bson::doc! {}
        } else {
            (&*filter).as_raw_doc()?.try_into()?
        };
        let options = parse_count_options(opts, ctx)?;
        Ok((coll, filter_doc, options))
    })();

    let (coll, filter_doc, options) = match setup {
        Ok(v) => v,
        Err(e) => {
            callback(userdata, 0, &Error::from(&e));
            return;
        }
    };

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll.count_documents(filter_doc).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(count) => callback(userdata, count, std::ptr::null()),
            Err(e) => callback(userdata, 0, &Error::from(&e)),
        }
    });
}

/// Return an estimated document count for the specified collection.
///
/// Uses collection metadata rather than scanning documents — does not support sessions.
///
/// # Safety
///
/// - `client` must be a valid pointer from `mongo_client_new`
/// - `db_name`, `coll_name` must be valid null-terminated C strings
/// - `opts` may be null (use defaults)
/// - `ctx` may be null (no read concern/read preference)
#[no_mangle]
pub unsafe extern "C" fn mongo_estimated_document_count(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    opts: *const EstimatedDocumentCountOptions,
    callback: CountCallback,
    userdata: *mut c_void,
) {
    let setup = (|| -> crate::error::Result<_> {
        use crate::error::Error;
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        let db = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        let coll = (*client)
            .client
            .database(db)
            .collection::<crate::bson::Document>(coll_name_str);
        let options = parse_estimated_options(opts, ctx)?;
        Ok((coll, options))
    })();

    let (coll, options) = match setup {
        Ok(v) => v,
        Err(e) => {
            callback(userdata, 0, &Error::from(&e));
            return;
        }
    };

    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let result = coll
            .estimated_document_count()
            .with_options(options)
            .await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(count) => callback(userdata, count, std::ptr::null()),
            Err(e) => callback(userdata, 0, &Error::from(&e)),
        }
    });
}
