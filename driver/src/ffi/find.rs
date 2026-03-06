//! FFI find operations.

#[cfg(test)]
mod tests;

use std::ffi::{c_char, c_void};

use futures_util::stream::StreamExt;

use crate::{
    bson::Document,
    ffi::{
        client::MongoClient,
        cursor::{Cursor, CursorResult},
        error::Error,
        types::{Bson, BsonArray, ContextExt, OperationContext},
        utils::{
            c_char_to_str,
            c_char_to_string,
            i64_to_duration_ms,
            i8_to_option_bool,
            with_callback,
            with_err_callback,
        },
    },
    options::{CursorType, Hint, SelectionCriteria},
};

/// Callback for asynchronous `mongo_find` results.
pub type FindCallback =
    extern "C" fn(userdata: *mut c_void, result: *const CursorResult, error: *const Error);

/// Find documents matching a filter.
#[no_mangle]
pub unsafe extern "C" fn mongo_find(
    client: *mut MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    opts: *const FindOptions,
    callback: FindCallback,
    userdata: *mut c_void,
) {
    let (coll, filter, options) = with_err_callback!(callback, userdata, || {
        use crate::error::Error;

        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
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

        let filter_doc = if filter.is_null() {
            crate::bson::doc! {}
        } else {
            let filter = &*filter;
            filter.as_raw_doc()?.try_into()?
        };

        let options = parse_find_options(opts, ctx)?;

        Ok((coll, filter_doc, options))
    });

    let mut session_ref = ctx.session();
    let userdata_ptr = userdata as usize;
    let client_ref = &*client;
    client_ref.runtime.spawn(async move {
        let action = coll.find(filter).with_options(options);
        let cursor = match session_ref.as_deref_mut() {
            None => action.batch().await.map(Cursor::Base),
            Some(session) => action.session(session).batch().await.map(Cursor::Session),
        };
        let userdata = userdata_ptr as *mut c_void;
        let mut cursor = match cursor {
            Ok(c) => c,
            Err(e) => {
                callback(userdata, std::ptr::null(), &Error::from(&e));
                return;
            }
        };
        let first_batch = match &mut cursor {
            Cursor::Base(c) => c.next().await,
            Cursor::Session(c) => c.stream(session_ref.unwrap()).next().await,
        };

        let userdata = userdata_ptr as *mut c_void;
        with_callback(callback, userdata, || {
            let exhausted = match &cursor {
                Cursor::Base(c) => c.is_exhausted(),
                Cursor::Session(c) => c.is_exhausted(),
            };

            let _doc_ptrs;
            let first_batch = match first_batch {
                Some(raw_batch) => {
                    let out = BsonArray::from_batch(&raw_batch?)?;
                    _doc_ptrs = out.0;
                    out.1
                }
                None => BsonArray::null(),
            };

            let cursor = if exhausted {
                std::ptr::null_mut()
            } else {
                Box::into_raw(Box::new(cursor))
            };
            Ok(CursorResult {
                cursor,
                exhausted,
                first_batch,
            })
        });
    });
}

/// FFI-compatible find options.
///
/// Use -1 for "not set" on integer options, null for pointer options.
/// For tri-state booleans (i8): -1 = not set, 0 = false, 1 = true.
#[repr(C)]
pub struct FindOptions {
    /// Allow disk use for sorting large result sets. Tri-state: -1 = not set, 0 = false, 1 = true.
    pub allow_disk_use: i8,
    /// Allow partial results from mongos if some shards are down. Tri-state.
    pub allow_partial_results: i8,
    /// Number of documents per batch. -1 = not set.
    pub batch_size: i32,
    /// Comment to attach to the query. Nullable BSON value wrapped in doc with empty key.
    pub comment: *const Bson,
    /// Cursor type: -1 = not set, 0 = NonTailable, 1 = Tailable, 2 = TailableAwait.
    pub cursor_type: i8,
    /// Index name hint. Nullable, takes precedence over hint_keys if set.
    pub hint_name: *const c_char,
    /// Index keys hint as BSON document. Nullable.
    pub hint_keys: *const Bson,
    /// Maximum number of documents to return. 0 = not set.
    pub limit: i64,
    /// Exclusive upper bound for a specific index. Nullable BSON document.
    pub max: *const Bson,
    /// Max time for tailable cursor to wait for new documents. -1 = not set.
    pub max_await_time_ms: i64,
    /// Maximum query execution time in milliseconds. -1 = not set.
    pub max_time_ms: i64,
    /// Inclusive lower bound for a specific index. Nullable BSON document.
    pub min: *const Bson,
    /// Prevent cursor timeout after inactivity. Tri-state.
    pub no_cursor_timeout: i8,
    /// Projection document. Nullable BSON document.
    pub projection: *const Bson,
    /// Return only index keys, not full documents. Tri-state.
    pub return_key: i8,
    /// Include record identifier in results. Tri-state.
    pub show_record_id: i8,
    /// Number of documents to skip. -1 = not set.
    pub skip: i64,
    /// Sort specification. Nullable BSON document.
    pub sort: *const Bson,
    /// Collation options. Nullable BSON document (deserialized as Collation).
    pub collation: *const Bson,
    /// Variables for use in aggregation expressions. Nullable BSON document.
    pub let_vars: *const Bson,
}

/// Parse FFI FindOptions into driver FindOptions.
unsafe fn parse_find_options(
    opts: *const FindOptions,
    ctx: *const OperationContext,
) -> crate::error::Result<crate::options::FindOptions> {
    let mut options = crate::options::FindOptions::default();

    // Always set context-derived options
    options.read_concern = ctx.read_concern();
    options.selection_criteria = ctx.read_preference().map(SelectionCriteria::ReadPreference);

    if opts.is_null() {
        return Ok(options);
    }

    let opts = &*opts;

    options.allow_disk_use = i8_to_option_bool(opts.allow_disk_use);
    options.allow_partial_results = i8_to_option_bool(opts.allow_partial_results);
    options.batch_size = if opts.batch_size >= 0 {
        Some(opts.batch_size as u32)
    } else {
        None
    };

    // Parse comment (BSON value wrapped in doc with empty key)
    if !opts.comment.is_null() {
        let comment_bson = (*opts.comment).as_raw_doc()?;
        let comment_doc: Document = comment_bson.try_into()?;
        options.comment = comment_doc.get("").map(|v| v.clone());
    }

    // Parse cursor_type
    options.cursor_type = match opts.cursor_type {
        0 => Some(CursorType::NonTailable),
        1 => Some(CursorType::Tailable),
        2 => Some(CursorType::TailableAwait),
        _ => None,
    };

    // Parse hint (name takes precedence over keys)
    options.hint = if let Some(name) = c_char_to_string(opts.hint_name)? {
        Some(Hint::Name(name))
    } else if let Some(keys) = Bson::to_doc(opts.hint_keys)? {
        Some(Hint::Keys(keys))
    } else {
        None
    };

    options.limit = if opts.limit != 0 {
        Some(opts.limit)
    } else {
        None
    };
    options.max = Bson::to_doc(opts.max)?;
    options.max_await_time = i64_to_duration_ms(opts.max_await_time_ms);
    options.max_time = i64_to_duration_ms(opts.max_time_ms);
    options.min = Bson::to_doc(opts.min)?;
    options.no_cursor_timeout = i8_to_option_bool(opts.no_cursor_timeout);
    options.projection = Bson::to_doc(opts.projection)?;
    options.return_key = i8_to_option_bool(opts.return_key);
    options.show_record_id = i8_to_option_bool(opts.show_record_id);
    options.skip = if opts.skip >= 0 {
        Some(opts.skip as u64)
    } else {
        None
    };
    options.sort = Bson::to_doc(opts.sort)?;

    // Parse collation from BSON document
    if let Some(doc) = Bson::to_doc(opts.collation)? {
        options.collation = Some(crate::bson_compat::deserialize_from_document(doc)?);
    }

    options.let_vars = Bson::to_doc(opts.let_vars)?;

    Ok(options)
}
