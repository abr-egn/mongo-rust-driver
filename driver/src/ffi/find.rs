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
        utils::{c_char_to_str, with_callback, with_err_callback},
    },
    options::{FindOptions, SelectionCriteria},
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
    allow_disk_use: bool,
    // TODO: more options
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

        let mut options = FindOptions::default();
        options.allow_disk_use = Some(allow_disk_use);
        options.read_concern = ctx.read_concern();
        options.selection_criteria = ctx.read_preference().map(SelectionCriteria::ReadPreference);

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
