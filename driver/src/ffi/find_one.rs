//! FFI find-and-modify operations.

#[cfg(test)]
mod tests;

use std::ffi::{c_char, c_void};

use crate::{
    ffi::{
        client::MongoClient,
        error::Error,
        types::{Bson, BsonArray, BsonValue, ContextExt, OwnedBson, OperationContext},
        utils::{c_char_to_str, c_char_to_string, i64_to_duration_ms, i8_to_option_bool, with_err_callback},
    },
};

/// Callback for find-and-modify results.
/// `result` is null when no document matched (success with no match) or on error.
/// Check `error` to distinguish the two cases.
pub type FindOneCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const OwnedBson,
    error: *const Error,
);

/// Options for find_one_and_delete.
#[repr(C)]
pub struct FindOneAndDeleteOptions {
    /// Max query execution time in milliseconds. -1 = not set.
    pub max_time_ms: i64,
    /// Projection as a serialized BSON document. Nullable.
    pub projection: *const Bson,
    /// Sort order as a serialized BSON document. Nullable.
    pub sort: *const Bson,
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

/// Options for find_one_and_replace.
#[repr(C)]
pub struct FindOneAndReplaceOptions {
    /// Tri-state: -1 = not set, 0 = false, 1 = true.
    pub bypass_document_validation: i8,
    /// Max query execution time in milliseconds. -1 = not set.
    pub max_time_ms: i64,
    /// Projection as a serialized BSON document. Nullable.
    pub projection: *const Bson,
    /// Which document to return: -1 = not set, 0 = Before, 1 = After.
    pub return_document: i8,
    /// Sort order as a serialized BSON document. Nullable.
    pub sort: *const Bson,
    /// Tri-state: -1 = not set, 0 = false, 1 = true.
    pub upsert: i8,
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

/// Options for find_one_and_update.
#[repr(C)]
pub struct FindOneAndUpdateOptions {
    /// Array filter documents for array update operators. Empty = not set.
    pub array_filters: BsonArray,
    /// Tri-state: -1 = not set, 0 = false, 1 = true.
    pub bypass_document_validation: i8,
    /// Max query execution time in milliseconds. -1 = not set.
    pub max_time_ms: i64,
    /// Projection as a serialized BSON document. Nullable.
    pub projection: *const Bson,
    /// Which document to return: -1 = not set, 0 = Before, 1 = After.
    pub return_document: i8,
    /// Sort order as a serialized BSON document. Nullable.
    pub sort: *const Bson,
    /// Tri-state: -1 = not set, 0 = false, 1 = true.
    pub upsert: i8,
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

unsafe fn parse_return_document(value: i8) -> Option<crate::coll::options::ReturnDocument> {
    match value {
        0 => Some(crate::coll::options::ReturnDocument::Before),
        1 => Some(crate::coll::options::ReturnDocument::After),
        _ => None,
    }
}

unsafe fn parse_hint(
    hint_name: *const c_char,
    hint_keys: *const Bson,
) -> crate::error::Result<Option<crate::options::Hint>> {
    if let Some(name) = c_char_to_string(hint_name)? {
        return Ok(Some(crate::options::Hint::Name(name)));
    }
    if let Some(keys) = Bson::to_doc(hint_keys)? {
        return Ok(Some(crate::options::Hint::Keys(keys)));
    }
    Ok(None)
}

unsafe fn parse_update_mods(
    update_doc: *const Bson,
    update_pipeline: BsonArray,
) -> crate::error::Result<crate::coll::options::UpdateModifications> {
    use crate::error::Error;
    match (!update_doc.is_null(), !update_pipeline.is_empty()) {
        (false, false) => Err(Error::invalid_argument(
            "one of update_doc or update_pipeline must be provided",
        )),
        (true, true) => Err(Error::invalid_argument(
            "only one of update_doc or update_pipeline may be provided",
        )),
        (true, false) => {
            let doc: crate::bson::Document = (&*update_doc).as_raw_doc()?.try_into()?;
            Ok(crate::coll::options::UpdateModifications::Document(doc))
        }
        (false, true) => {
            let pipeline = update_pipeline
                .to_raw_docs()
                .iter()
                .map(|raw| -> crate::error::Result<_> { Ok((*raw).try_into()?) })
                .collect::<crate::error::Result<Vec<crate::bson::Document>>>()?;
            Ok(crate::coll::options::UpdateModifications::Pipeline(pipeline))
        }
    }
}

unsafe fn parse_foad_options(
    opts: *const FindOneAndDeleteOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<crate::coll::options::FindOneAndDeleteOptions> {
    let mut options = crate::coll::options::FindOneAndDeleteOptions::default();
    options.write_concern = ctx.write_concern();

    if opts.is_null() {
        return Ok(options);
    }
    let opts = &*opts;

    options.max_time = i64_to_duration_ms(opts.max_time_ms);
    options.projection = Bson::to_doc(opts.projection)?;
    options.sort = Bson::to_doc(opts.sort)?;
    options.hint = parse_hint(opts.hint_name, opts.hint_keys)?;
    options.let_vars = Bson::to_doc(opts.let_vars)?;
    if let Some(doc) = Bson::to_doc(opts.collation)? {
        options.collation = Some(crate::bson_compat::deserialize_from_document(doc)?);
    }
    if !opts.comment.is_null() {
        options.comment = (&*opts.comment).to_bson()?;
    }
    Ok(options)
}

unsafe fn parse_foar_options(
    opts: *const FindOneAndReplaceOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<crate::coll::options::FindOneAndReplaceOptions> {
    let mut options = crate::coll::options::FindOneAndReplaceOptions::default();
    options.write_concern = ctx.write_concern();

    if opts.is_null() {
        return Ok(options);
    }
    let opts = &*opts;

    options.bypass_document_validation = i8_to_option_bool(opts.bypass_document_validation);
    options.max_time = i64_to_duration_ms(opts.max_time_ms);
    options.projection = Bson::to_doc(opts.projection)?;
    options.return_document = parse_return_document(opts.return_document);
    options.sort = Bson::to_doc(opts.sort)?;
    options.upsert = i8_to_option_bool(opts.upsert);
    options.hint = parse_hint(opts.hint_name, opts.hint_keys)?;
    options.let_vars = Bson::to_doc(opts.let_vars)?;
    if let Some(doc) = Bson::to_doc(opts.collation)? {
        options.collation = Some(crate::bson_compat::deserialize_from_document(doc)?);
    }
    if !opts.comment.is_null() {
        options.comment = (&*opts.comment).to_bson()?;
    }
    Ok(options)
}

unsafe fn parse_foau_options(
    opts: *const FindOneAndUpdateOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<crate::coll::options::FindOneAndUpdateOptions> {
    let mut options = crate::coll::options::FindOneAndUpdateOptions::default();
    options.write_concern = ctx.write_concern();

    if opts.is_null() {
        return Ok(options);
    }
    let opts = &*opts;

    options.bypass_document_validation = i8_to_option_bool(opts.bypass_document_validation);
    options.max_time = i64_to_duration_ms(opts.max_time_ms);
    options.projection = Bson::to_doc(opts.projection)?;
    options.return_document = parse_return_document(opts.return_document);
    options.sort = Bson::to_doc(opts.sort)?;
    options.upsert = i8_to_option_bool(opts.upsert);
    options.hint = parse_hint(opts.hint_name, opts.hint_keys)?;
    options.let_vars = Bson::to_doc(opts.let_vars)?;
    if let Some(doc) = Bson::to_doc(opts.collation)? {
        options.collation = Some(crate::bson_compat::deserialize_from_document(doc)?);
    }
    if !opts.comment.is_null() {
        options.comment = (&*opts.comment).to_bson()?;
    }
    if !opts.array_filters.is_empty() {
        let filters = opts
            .array_filters
            .to_raw_docs()
            .iter()
            .map(|raw| -> crate::error::Result<_> { Ok((*raw).try_into()?) })
            .collect::<crate::error::Result<Vec<crate::bson::Document>>>()?;
        options.array_filters = Some(filters);
    }
    Ok(options)
}

/// Atomically find a document matching `filter` and delete it.
///
/// Callback receives null result when no document matched.
///
/// # Safety
///
/// - `client` must be a valid pointer from `mongo_client_new`
/// - `db_name`, `coll_name` must be valid null-terminated C strings
/// - `filter` must be a valid BSON document pointer
/// - `opts` and `ctx` may be null
#[no_mangle]
pub unsafe extern "C" fn mongo_find_one_and_delete(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    opts: *const FindOneAndDeleteOptions,
    callback: FindOneCallback,
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
        let options = parse_foad_options(opts, ctx)?;
        Ok((coll, filter_doc, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll.find_one_and_delete(filter_doc).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            match result? {
                None => callback(userdata, std::ptr::null(), std::ptr::null()),
                Some(doc) => callback(userdata, &OwnedBson::from_doc(&doc), std::ptr::null()),
            }
            Ok(())
        });
    });
}

/// Atomically find a document matching `filter` and update it.
///
/// Exactly one of `update_doc` (non-null) or `update_pipeline` (non-empty) must be provided.
/// Callback receives null result when no document matched.
///
/// # Safety
///
/// Same safety requirements as `mongo_find_one_and_delete`.
#[no_mangle]
pub unsafe extern "C" fn mongo_find_one_and_update(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    update_doc: *const Bson,
    update_pipeline: BsonArray,
    opts: *const FindOneAndUpdateOptions,
    callback: FindOneCallback,
    userdata: *mut c_void,
) {
    let (coll, filter_doc, update, options) = with_err_callback!(callback, userdata, || {
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
        let update = parse_update_mods(update_doc, update_pipeline)?;
        let options = parse_foau_options(opts, ctx)?;
        Ok((coll, filter_doc, update, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll.find_one_and_update(filter_doc, update).with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            match result? {
                None => callback(userdata, std::ptr::null(), std::ptr::null()),
                Some(doc) => callback(userdata, &OwnedBson::from_doc(&doc), std::ptr::null()),
            }
            Ok(())
        });
    });
}

/// Atomically find a document matching `filter` and replace it with `replacement`.
///
/// Callback receives null result when no document matched.
///
/// # Safety
///
/// Same safety requirements as `mongo_find_one_and_delete`.
#[no_mangle]
pub unsafe extern "C" fn mongo_find_one_and_replace(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    replacement: *const Bson,
    opts: *const FindOneAndReplaceOptions,
    callback: FindOneCallback,
    userdata: *mut c_void,
) {
    let (coll, filter_doc, replacement_doc, options) = with_err_callback!(callback, userdata, || {
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
        let coll = (*client).client
            .database(db)
            .collection::<crate::bson::Document>(coll_name_str);
        let filter_doc: crate::bson::Document = (&*filter).as_raw_doc()?.try_into()?;
        let repl_bytes = {
            let b = &*replacement;
            std::slice::from_raw_parts(b.data, b.len).to_vec()
        };
        let replacement_doc: crate::bson::Document =
            crate::bson::RawDocumentBuf::from_bytes(repl_bytes)?.try_into()?;
        let options = parse_foar_options(opts, ctx)?;
        Ok((coll, filter_doc, replacement_doc, options))
    });

    let session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let mut action = coll
            .find_one_and_replace(filter_doc, replacement_doc)
            .with_options(options);
        if let Some(session) = session_ref {
            action = action.session(session);
        }
        let result = action.await;

        let userdata = userdata_ptr as *mut c_void;
        with_err_callback!(callback, userdata, || {
            match result? {
                None => callback(userdata, std::ptr::null(), std::ptr::null()),
                Some(doc) => callback(userdata, &OwnedBson::from_doc(&doc), std::ptr::null()),
            }
            Ok(())
        });
    });
}
