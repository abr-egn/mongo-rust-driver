use std::ffi::c_void;

use futures_util::stream::StreamExt;
use tokio::sync::Mutex;

use crate::{
    raw_batch_cursor::{RawBatchCursor, SessionRawBatchCursor},
    ClientSession,
};

use super::{client::MongoClient, error::Error, types::Bson};

/// A handle used to request batches of results from the server.
pub struct Cursor(Mutex<CursorKind>);

enum CursorKind {
    Base(RawBatchCursor),
    Session(SessionRawBatchCursor),
}

/// Common result for all cursor-returning operations
#[repr(C)]
pub struct CursorResult {
    pub cursor: *mut Cursor, // null if exhausted with single batch
    pub exhausted: bool,     // true if no more batches (cursor already closed)
    pub first_batch: Bson,   // raw BSON array of documents from initial response
}

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
        match cursor.0.try_lock() {
            Err(_) => {
                return Err(Error::invalid_argument(
                    "cannot iterate on a cursor already in use",
                ))
            }
            Ok(guard) => match *guard {
                CursorKind::Base(_) => {
                    if !session.is_null() {
                        return Err(Error::invalid_argument(
                            "cursors created without a session must not be iterated with one",
                        ));
                    }
                }
                CursorKind::Session(_) => {
                    if session.is_null() {
                        return Err(Error::invalid_argument(
                            "cursors created with a session must be iterated with that session",
                        ));
                    }
                }
            },
        }

        Ok(())
    };
    if let Err(e) = init() {
        callback(userdata, false, std::ptr::null(), &Error::from(&e));
        return;
    }

    let client = &*client;
    let cursor = &*cursor;
    let userdata_ptr = userdata as usize;
    let session_ptr = session as usize;
    client.runtime.spawn(async move {
        let userdata = userdata_ptr as *mut c_void;
        let session = session_ptr as *mut ClientSession;

        let Ok(mut guard) = cursor.0.try_lock() else {
            callback(
                userdata,
                false,
                std::ptr::null(),
                &Error::from(&crate::error::Error::invalid_argument(
                    "cannot iterate on a cursor already in use",
                )),
            );
            return;
        };
        let (batch, exhausted) = match &mut *guard {
            CursorKind::Base(cursor) => (cursor.next().await, cursor.is_exhausted()),
            CursorKind::Session(cursor) => (
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
