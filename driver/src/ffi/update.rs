//! FFI update and replace operations.
#[cfg(test)]
mod tests;

use std::ffi::{c_char, c_void};

use crate::{
    bson::Document,
    coll::options::{ReplaceOptions, UpdateModifications, UpdateOptions},
    ffi::{
        client::MongoClient,
        error::Error,
        types::{Bson, BsonArray, BsonValue, ContextExt, OperationContext, OwnedBsonValue},
        utils::{c_char_to_str, c_char_to_string, with_err_callback},
    },
};

/// Callback for update/replace operation results.
pub type UpdateCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const UpdateResult,
    error: *const Error,
);

/// Result of an update_one, update_many, or replace_one operation.
#[repr(C)]
pub struct UpdateResult {
    /// Number of documents that matched the filter.
    pub matched_count: u64,
    /// Number of documents that were modified.
    pub modified_count: u64,
    /// The `_id` of the upserted document, or null/zero-type if no upsert occurred.
    pub upserted_id: OwnedBsonValue,
}

/// Options for update operations.
///
/// All pointer fields are nullable (null = not set).
#[repr(C)]
pub struct UpdateOneOptions {
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
    /// Array filters as BSON array. Nullable.
    pub array_filters: *const Bson,
    /// If true, insert a document if no matching document is found. -1 = not set.
    pub upsert: i8,
    /// Opt out of document-level validation. -1 = not set.
    pub bypass_document_validation: i8,
    /// Sort order to select which document to update when multiple match. Nullable. MongoDB 8.0+.
    pub sort: *const Bson,
}

/// Options for replace_one operations.
#[repr(C)]
pub struct ReplaceOneOptions {
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
    /// If true, insert a document if no matching document is found. -1 = not set.
    pub upsert: i8,
    /// Opt out of document-level validation. -1 = not set.
    pub bypass_document_validation: i8,
    /// Sort order to select which document to replace when multiple match. Nullable. MongoDB 8.0+.
    pub sort: *const Bson,
}

unsafe fn parse_update_options(
    opts: *const UpdateOneOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<UpdateOptions> {
    use crate::ffi::utils::i8_to_option_bool;

    let mut options = UpdateOptions::default();
    options.write_concern = ctx.write_concern();

    if opts.is_null() {
        return Ok(options);
    }
    let opts = &*opts;

    options.upsert = i8_to_option_bool(opts.upsert);
    options.bypass_document_validation = i8_to_option_bool(opts.bypass_document_validation);

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

    options.sort = Bson::to_doc(opts.sort)?;

    if let Some(af_doc) = Bson::to_doc(opts.array_filters)? {
        // array_filters is passed as a BSON document wrapping an array: {"": [...]}
        // Extract the array from the first field
        let mut filters = vec![];
        for (_, val) in &af_doc {
            if let crate::bson::Bson::Array(arr) = val {
                for item in arr {
                    if let crate::bson::Bson::Document(d) = item {
                        filters.push(d.clone());
                    }
                }
            }
        }
        if !filters.is_empty() {
            options.array_filters = Some(filters);
        }
    }

    Ok(options)
}

unsafe fn parse_replace_options(
    opts: *const ReplaceOneOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<ReplaceOptions> {
    use crate::ffi::utils::i8_to_option_bool;

    let mut options = ReplaceOptions::default();
    options.write_concern = ctx.write_concern();

    if opts.is_null() {
        return Ok(options);
    }
    let opts = &*opts;

    options.upsert = i8_to_option_bool(opts.upsert);
    options.bypass_document_validation = i8_to_option_bool(opts.bypass_document_validation);

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

    options.sort = Bson::to_doc(opts.sort)?;

    Ok(options)
}

fn build_update_result(r: crate::results::UpdateResult) -> crate::error::Result<UpdateResult> {
    let upserted_id = match r.upserted_id {
        Some(id) => OwnedBsonValue::from_bson(&id)?,
        None => OwnedBsonValue::null(),
    };
    Ok(UpdateResult {
        matched_count: r.matched_count,
        modified_count: r.modified_count,
        upserted_id,
    })
}

/// Update up to one document matching `filter`.
///
/// Exactly one of `update` (BSON document) or `pipeline` (non-empty array) must be provided.
///
/// # Safety
///
/// - `client` must be a valid pointer from `mongo_client_new`
/// - `db_name`, `coll_name` must be valid null-terminated C strings
/// - `filter` must be a valid BSON document pointer
/// - Exactly one of `update` or `pipeline` must be non-null/non-empty
/// - `opts` may be null (use defaults)
/// - `ctx` may be null (no session/write concern)
#[no_mangle]
pub unsafe extern "C" fn mongo_update_one(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    update: *const Bson,
    pipeline: BsonArray,
    opts: *const UpdateOneOptions,
    callback: UpdateCallback,
    userdata: *mut c_void,
) {
    let (coll, filter_doc, modifications, options) = with_err_callback!(callback, userdata, || {
        use crate::error::Error;
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        if filter.is_null() {
            return Err(Error::invalid_argument("filter cannot be null"));
        }
        let update_set = !update.is_null();
        let pipeline_set = !pipeline.is_empty();
        if update_set && pipeline_set {
            return Err(Error::invalid_argument(
                "only one of update document or pipeline may be provided",
            ));
        }
        if !update_set && !pipeline_set {
            return Err(Error::invalid_argument(
                "one of update document or pipeline must be provided",
            ));
        }
        let db = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        let coll = (*client)
            .client
            .database(db)
            .collection::<Document>(coll_name_str);
        let filter_doc: Document = (&*filter).as_raw_doc()?.try_into()?;
        let modifications = if update_set {
            let update_doc: Document = (&*update).as_raw_doc()?.try_into()?;
            UpdateModifications::Document(update_doc)
        } else {
            let raw_docs = pipeline.to_raw_docs();
            let mut docs = vec![];
            for raw in raw_docs {
                docs.push(Document::try_from(raw)?);
            }
            UpdateModifications::Pipeline(docs)
        };
        let options = parse_update_options(opts, ctx)?;
        Ok((coll, filter_doc, modifications, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll.update_one(filter_doc, modifications).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            let out = build_update_result(result?)?;
            callback(userdata, &out, std::ptr::null());
            Ok(())
        });
    });
}

/// Update all documents matching `filter`.
///
/// Exactly one of `update` (BSON document) or `pipeline` (non-empty array) must be provided.
///
/// # Safety
///
/// Same safety requirements as `mongo_update_one`.
#[no_mangle]
pub unsafe extern "C" fn mongo_update_many(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    update: *const Bson,
    pipeline: BsonArray,
    opts: *const UpdateOneOptions,
    callback: UpdateCallback,
    userdata: *mut c_void,
) {
    let (coll, filter_doc, modifications, options) = with_err_callback!(callback, userdata, || {
        use crate::error::Error;
        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        if filter.is_null() {
            return Err(Error::invalid_argument("filter cannot be null"));
        }
        let update_set = !update.is_null();
        let pipeline_set = !pipeline.is_empty();
        if update_set && pipeline_set {
            return Err(Error::invalid_argument(
                "only one of update document or pipeline may be provided",
            ));
        }
        if !update_set && !pipeline_set {
            return Err(Error::invalid_argument(
                "one of update document or pipeline must be provided",
            ));
        }
        let db = c_char_to_str(db_name)?
            .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
        let coll_name_str = c_char_to_str(coll_name)?
            .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
        let coll = (*client)
            .client
            .database(db)
            .collection::<Document>(coll_name_str);
        let filter_doc: Document = (&*filter).as_raw_doc()?.try_into()?;
        let modifications = if update_set {
            let update_doc: Document = (&*update).as_raw_doc()?.try_into()?;
            UpdateModifications::Document(update_doc)
        } else {
            let raw_docs = pipeline.to_raw_docs();
            let mut docs = vec![];
            for raw in raw_docs {
                docs.push(Document::try_from(raw)?);
            }
            UpdateModifications::Pipeline(docs)
        };
        let options = parse_update_options(opts, ctx)?;
        Ok((coll, filter_doc, modifications, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll.update_many(filter_doc, modifications).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            let out = build_update_result(result?)?;
            callback(userdata, &out, std::ptr::null());
            Ok(())
        });
    });
}

/// Replace up to one document matching `filter` with `replacement`.
///
/// # Safety
///
/// - `client` must be a valid pointer from `mongo_client_new`
/// - `db_name`, `coll_name` must be valid null-terminated C strings
/// - `filter` must be a valid BSON document pointer
/// - `replacement` must be a valid BSON document pointer
/// - `opts` may be null (use defaults)
/// - `ctx` may be null (no session/write concern)
#[no_mangle]
pub unsafe extern "C" fn mongo_replace_one(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    replacement: *const Bson,
    opts: *const ReplaceOneOptions,
    callback: UpdateCallback,
    userdata: *mut c_void,
) {
    let (coll, filter_doc, replacement_doc, options) =
        with_err_callback!(callback, userdata, || {
            use crate::error::Error;
            if client.is_null() {
                return Err(Error::invalid_argument("client cannot be null"));
            }
            if filter.is_null() {
                return Err(Error::invalid_argument("filter cannot be null"));
            }
            if replacement.is_null() {
                return Err(Error::invalid_argument("replacement cannot be null"));
            }
            let db = c_char_to_str(db_name)?
                .ok_or_else(|| Error::invalid_argument("db_name cannot be null"))?;
            let coll_name_str = c_char_to_str(coll_name)?
                .ok_or_else(|| Error::invalid_argument("coll_name cannot be null"))?;
            let coll = (*client)
                .client
                .database(db)
                .collection::<Document>(coll_name_str);
            let filter_doc: Document = (&*filter).as_raw_doc()?.try_into()?;
            let replacement_doc: Document = (&*replacement).as_raw_doc()?.try_into()?;
            let options = parse_replace_options(opts, ctx)?;
            Ok((coll, filter_doc, replacement_doc, options))
        });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll
            .replace_one(filter_doc, replacement_doc)
            .with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            let out = build_update_result(result?)?;
            callback(userdata, &out, std::ptr::null());
            Ok(())
        });
    });
}
