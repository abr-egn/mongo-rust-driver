//! FFI distinct operation.

#[cfg(test)]
mod tests;

use std::ffi::{c_char, c_void};

use crate::ffi::{
    client::MongoClient,
    error::Error,
    types::{Bson, BsonValue, ContextExt, OwnedBsonValue, OperationContext},
    utils::{c_char_to_str, c_char_to_string, i64_to_duration_ms},
};

/// Result for `mongo_distinct`.
///
/// On success, `values` points to an array of `len` BSON values.
/// The memory is owned by the driver and valid only for the duration of the callback.
#[repr(C)]
pub struct DistinctResult {
    /// Pointer to an array of distinct values.
    pub values: *const OwnedBsonValue,
    /// Number of values in the array.
    pub len: usize,
}

/// Callback for distinct results.
/// On error: `result` is null and `error` is non-null.
/// On success: `result` is non-null and `error` is null.
pub type DistinctCallback =
    extern "C" fn(userdata: *mut c_void, result: *const DistinctResult, error: *const Error);

/// Options for `mongo_distinct`.
///
/// All pointer fields are nullable (null = not set).
#[repr(C)]
pub struct DistinctOptions {
    /// Collation as a serialized BSON document. Nullable.
    pub collation: *const Bson,
    /// Index hint by name. Nullable. Takes precedence over `hint_keys`.
    pub hint_name: *const c_char,
    /// Index hint by key pattern as BSON document. Nullable.
    pub hint_keys: *const Bson,
    /// Maximum time in milliseconds. -1 = not set.
    pub max_time_ms: i64,
    /// Comment BSON value. Nullable.
    pub comment: *const BsonValue,
}

unsafe fn parse_distinct_options(
    opts: *const DistinctOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<crate::coll::options::DistinctOptions> {
    let mut options = crate::coll::options::DistinctOptions::default();
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

    options.max_time = i64_to_duration_ms(opts.max_time_ms);

    if !opts.comment.is_null() {
        options.comment = (&*opts.comment).to_bson()?;
    }

    Ok(options)
}

/// Find the distinct values for a specified field across a collection.
///
/// If `filter` is null, all documents are considered (equivalent to `{}`).
///
/// # Safety
///
/// - `client` must be a valid pointer from `mongo_client_new`
/// - `db_name`, `coll_name`, `field_name` must be valid null-terminated C strings
/// - `filter` may be null (considers all documents) or a valid BSON document pointer
/// - `opts` may be null (use defaults)
/// - `ctx` may be null (no session/read concern)
#[no_mangle]
pub unsafe extern "C" fn mongo_distinct(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    field_name: *const c_char,
    filter: *const Bson,
    opts: *const DistinctOptions,
    callback: DistinctCallback,
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
        let field = c_char_to_str(field_name)?
            .ok_or_else(|| Error::invalid_argument("field_name cannot be null"))?;
        let coll = (*client)
            .client
            .database(db)
            .collection::<crate::bson::Document>(coll_name_str);
        let filter_doc: crate::bson::Document = if filter.is_null() {
            crate::bson::doc! {}
        } else {
            (&*filter).as_raw_doc()?.try_into()?
        };
        let options = parse_distinct_options(opts, ctx)?;
        Ok((coll, field.to_string(), filter_doc, options))
    })();

    let (coll, field, filter_doc, options) = match setup {
        Ok(v) => v,
        Err(e) => {
            callback(userdata, std::ptr::null(), &Error::from(&e));
            return;
        }
    };

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll.distinct(field, filter_doc).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        match result {
            Ok(values) => {
                let owned: Result<Vec<OwnedBsonValue>, _> = values
                    .iter()
                    .map(|v| OwnedBsonValue::from_bson(v))
                    .collect();
                match owned {
                    Ok(owned_values) => {
                        let out = DistinctResult {
                            values: owned_values.as_ptr(),
                            len: owned_values.len(),
                        };
                        callback(userdata, &out, std::ptr::null());
                    }
                    Err(e) => {
                        callback(userdata, std::ptr::null(), &Error::from(&e));
                    }
                }
            }
            Err(e) => {
                callback(userdata, std::ptr::null(), &Error::from(&e));
            }
        }
    });
}
