//! FFI cursor definitions.

#[cfg(test)]
mod tests;

use std::ffi::c_void;

use futures_util::stream::StreamExt;

use crate::{
    raw_batch_cursor::{RawBatchCursor, SessionRawBatchCursor},
    ClientSession,
};

use super::{client::MongoClient, error::Error, types::Bson};

/// A handle used to request batches of results from the server.
pub enum Cursor {
    /// Non-session cursor
    Base(RawBatchCursor),
    /// Session cursor
    Session(SessionRawBatchCursor),
}

/// Common result for all cursor-returning operations
#[repr(C)]
pub struct CursorResult {
    /// null if exhausted with single batch
    pub cursor: *mut Cursor,
    /// true if no more batches (cursor already closed)
    pub exhausted: bool,
    /// raw BSON array of documents from initial response
    pub first_batch: Bson,
}

/// Asynchronous result callback for `mongo_cursor_get_more`.
pub type GetMoreResultCallback = extern "C" fn(
    userdata: *mut c_void,
    exhausted: bool, // true if no more batches
    data: *const Bson,
    error: *const Error, // null on success
);

/// Get more results from a cursor (async).
#[no_mangle]
pub unsafe extern "C" fn mongo_cursor_get_more(
    client: *mut MongoClient,
    cursor: *mut Cursor,
    session: *mut ClientSession,
    userdata: *mut c_void,
    callback: GetMoreResultCallback,
) {
    let init = || -> crate::error::Result<()> {
        use crate::error::Error;

        if client.is_null() {
            return Err(Error::invalid_argument("client cannot be null"));
        }
        if cursor.is_null() {
            return Err(Error::invalid_argument("cursor cannot be null"));
        }
        let cursor = &*cursor;
        match cursor {
            Cursor::Base(_) => {
                if !session.is_null() {
                    return Err(Error::invalid_argument(
                        "cursors created without a session must not be iterated with one",
                    ));
                }
            }
            Cursor::Session(_) => {
                if session.is_null() {
                    return Err(Error::invalid_argument(
                        "cursors created with a session must be iterated with that session",
                    ));
                }
            }
        }

        Ok(())
    };
    if let Err(e) = init() {
        callback(userdata, false, std::ptr::null(), &Error::from(&e));
        return;
    }

    let client = &*client;
    let cursor_ptr = cursor as usize;
    let userdata_ptr = userdata as usize;
    let session_ptr = session as usize;
    client.runtime.spawn(async move {
        let cursor = cursor_ptr as *mut Cursor;
        let session = session_ptr as *mut ClientSession;

        let (batch, exhausted) = match &mut *cursor {
            Cursor::Base(cursor) => (cursor.next().await, cursor.is_exhausted()),
            Cursor::Session(cursor) => (
                cursor.stream(&mut *session).next().await,
                cursor.is_exhausted(),
            ),
        };
        let process = || -> crate::error::Result<()> {
            use crate::error::Error;

            let batch = batch.ok_or_else(|| {
                Error::invalid_response("no batch returned for unexhausted cursor")
            })??;
            let batch_bytes = batch.as_raw_document().as_bytes();
            let data = Bson {
                data: batch_bytes.as_ptr(),
                len: batch_bytes.len(),
            };
            // Has to be re-reconsituted because we're past an await point from last time.
            let userdata = userdata_ptr as *mut c_void;
            callback(userdata, exhausted, &data, std::ptr::null());

            Ok(())
        };
        if let Err(e) = process() {
            callback(
                userdata_ptr as *mut c_void,
                false,
                std::ptr::null(),
                &Error::from(&e),
            );
            return;
        };
    });
}

/// Free a cursor and close it on the server in the background.
#[no_mangle]
pub unsafe extern "C" fn mongo_cursor_close(cursor: *mut Cursor) {
    if cursor.is_null() {
        return;
    }

    drop(Box::from_raw(cursor))
}
