//! FFI session management for the MongoDB Rust driver.
//!
//! Sessions are opaque pointers (`*mut Session`) backed by real `ClientSession` objects.
//! Transaction state is managed entirely within the session.

use std::{ffi::c_void, os::raw::c_char};

use super::{
    client::MongoClient,
    error::{Error, InvalidArgumentError},
    types::ClientSession,
    utils::{c_char_to_string, i64_to_duration_ms, parse_read_preference_mode},
};

/// Session options for FFI.
#[repr(C)]
pub struct SessionOptions {
    /// Causal consistency. -1 = not set (use default), 0 = false, 1 = true
    pub causal_consistency: i8,
    /// Snapshot reads. -1 = not set, 0 = false, 1 = true
    pub snapshot: i8,
    /// Default transaction options (applied when starting transactions)
    pub default_transaction_options: *const TransactionOptions,
}

/// Transaction options for FFI.
#[repr(C)]
pub struct TransactionOptions {
    /// Read concern level (null-terminated string, nullable)
    pub read_concern_level: *const c_char,
    /// Write concern w value. -1 = not set, 0+ = w value
    pub write_concern_w: i32,
    /// Write concern w tag (for w:"majority", null-terminated, nullable)
    pub write_concern_w_tag: *const c_char,
    /// Write concern journal. -1 = not set, 0 = false, 1 = true
    pub write_concern_j: i8,
    /// Write concern wtimeout in milliseconds. -1 = not set
    pub write_concern_w_timeout_ms: i64,
    /// Read preference mode. 0=primary, 1=primaryPreferred, 2=secondary, etc. 255 = not set
    pub read_preference_mode: u8,
    /// Max commit time in milliseconds. -1 = not set
    pub max_commit_time_ms: i64,
}

/// Callback type for transaction operations (start, commit, abort).
pub type TransactionCallback = extern "C" fn(userdata: *mut c_void, error: *const Error);

/// Parse FFI transaction options into Rust TransactionOptions.
unsafe fn parse_transaction_options(
    options: *const TransactionOptions,
) -> Result<Option<crate::options::TransactionOptions>, Error> {
    if options.is_null() {
        return Ok(None);
    }

    let opts = &*options;
    let mut tx_options = crate::options::TransactionOptions::default();

    // Parse read concern
    if !opts.read_concern_level.is_null() {
        let level = c_char_to_string(opts.read_concern_level)
            .map_err(|e| Error::from(InvalidArgumentError::new(&e.to_string())))?;
        if let Some(level_str) = level {
            tx_options.read_concern = Some(crate::options::ReadConcern::custom(level_str));
        }
    }

    // Parse write concern
    let w = if opts.write_concern_w >= 0 {
        Some(crate::options::Acknowledgment::from(
            opts.write_concern_w as u32,
        ))
    } else if !opts.write_concern_w_tag.is_null() {
        let tag = c_char_to_string(opts.write_concern_w_tag)
            .map_err(|e| Error::from(InvalidArgumentError::new(&e.to_string())))?;
        tag.map(crate::options::Acknowledgment::Custom)
    } else {
        None
    };

    let journal = if opts.write_concern_j >= 0 {
        Some(opts.write_concern_j != 0)
    } else {
        None
    };

    let w_timeout = i64_to_duration_ms(opts.write_concern_w_timeout_ms);

    if w.is_some() || journal.is_some() || w_timeout.is_some() {
        tx_options.write_concern = Some(crate::options::WriteConcern {
            w,
            w_timeout,
            journal,
        });
    }

    // Parse read preference
    let read_pref = parse_read_preference_mode(opts.read_preference_mode)
        .map_err(|e| Error::from(InvalidArgumentError::new(&e.to_string())))?;
    if let Some(rp) = read_pref {
        tx_options.selection_criteria =
            Some(crate::selection_criteria::SelectionCriteria::ReadPreference(rp));
    }

    // Parse max commit time
    tx_options.max_commit_time = i64_to_duration_ms(opts.max_commit_time_ms);

    Ok(Some(tx_options))
}

/// Parse FFI session options into Rust SessionOptions.
unsafe fn parse_session_options(
    options: *const SessionOptions,
) -> Result<Option<crate::options::SessionOptions>, Error> {
    if options.is_null() {
        return Ok(None);
    }

    let opts = &*options;
    let mut session_options = crate::options::SessionOptions::default();

    // Parse causal consistency
    if opts.causal_consistency >= 0 {
        session_options.causal_consistency = Some(opts.causal_consistency != 0);
    }

    // Parse snapshot
    if opts.snapshot >= 0 {
        session_options.snapshot = Some(opts.snapshot != 0);
    }

    // Parse default transaction options
    session_options.default_transaction_options =
        parse_transaction_options(opts.default_transaction_options)?;

    Ok(Some(session_options))
}

/// Start a new session.
///
/// Returns a session handle on success, or null on error.
///
/// # Safety
///
/// - `client` must be a valid pointer to a MongoClient.
/// - `options` can be null (use defaults) or a valid pointer to SessionOptionsFFI.
/// - `error_out` can be null (errors ignored) or a valid pointer to an error pointer.
#[no_mangle]
pub unsafe extern "C" fn mongo_session_start(
    client: *mut MongoClient,
    options: *const SessionOptions,
    error_out: *mut *mut Error,
) -> *mut ClientSession {
    if client.is_null() {
        if !error_out.is_null() {
            *error_out =
                Box::into_raw(Box::new(InvalidArgumentError::new("client is null").into()));
        }
        return std::ptr::null_mut();
    }

    let client_ref = &*client;

    // Parse session options
    let session_options = match parse_session_options(options) {
        Ok(opts) => opts,
        Err(e) => {
            if !error_out.is_null() {
                *error_out = Box::into_raw(Box::new(e));
            }
            return std::ptr::null_mut();
        }
    };

    // Create the session synchronously using the runtime
    let rust_client = client_ref.client.clone();
    let session_result = client_ref.runtime.block_on(async {
        rust_client
            .start_session()
            .with_options(session_options)
            .await
    });

    match session_result {
        Ok(session) => Box::into_raw(Box::new(session)),
        Err(e) => {
            if !error_out.is_null() {
                *error_out = Box::into_raw(Box::new(Error::from(&e)));
            }
            std::ptr::null_mut()
        }
    }
}

/// End a session.
///
/// The session handle becomes invalid after this call.
///
/// # Safety
///
/// - `session` must be a valid pointer to a Session, or null (no-op).
#[no_mangle]
pub unsafe extern "C" fn mongo_session_end(session: *mut ClientSession) {
    if !session.is_null() {
        let _ = Box::from_raw(session);
    }
}

/// Start a transaction on the session.
///
/// # Safety
///
/// - `client` must be a valid pointer to a MongoClient.
/// - `session` must be a valid pointer to a Session.
/// - `options` can be null (use defaults) or a valid pointer to TransactionOptionsFFI.
/// - `callback` will be invoked when the operation completes.
/// - `userdata` is passed to the callback.
#[no_mangle]
pub unsafe extern "C" fn mongo_session_start_transaction(
    client: *mut MongoClient,
    session: *mut ClientSession,
    options: *const TransactionOptions,
    callback: TransactionCallback,
    userdata: *mut c_void,
) {
    if client.is_null() {
        let error = InvalidArgumentError::new("client is null").into();
        callback(userdata, &error);
        return;
    }

    if session.is_null() {
        let error = InvalidArgumentError::new("session is null").into();
        callback(userdata, &error);
        return;
    }

    // Parse transaction options
    let tx_options = match parse_transaction_options(options) {
        Ok(opts) => opts,
        Err(e) => {
            callback(userdata, &e);
            return;
        }
    };

    let client_ref = &*client;
    let session_ref = &mut *session;
    let userdata_ptr = userdata as usize;

    client_ref.runtime.spawn(async move {
        let result = session_ref
            .start_transaction()
            .with_options(tx_options)
            .await;
        let userdata = userdata_ptr as *mut c_void;

        match result {
            Ok(()) => callback(userdata, std::ptr::null()),
            Err(e) => {
                let error = Error::from(&e);
                callback(userdata, &error);
            }
        }
    });
}

/// Commit the current transaction.
///
/// # Safety
///
/// - `client` must be a valid pointer to a MongoClient.
/// - `session` must be a valid pointer to a Session with an active transaction.
/// - `callback` will be invoked when the operation completes.
/// - `userdata` is passed to the callback.
#[no_mangle]
pub unsafe extern "C" fn mongo_session_commit_transaction(
    client: *mut MongoClient,
    session: *mut ClientSession,
    callback: TransactionCallback,
    userdata: *mut c_void,
) {
    if client.is_null() {
        let error = InvalidArgumentError::new("client is null").into();
        callback(userdata, &error);
        return;
    }

    if session.is_null() {
        let error = InvalidArgumentError::new("session is null").into();
        callback(userdata, &error);
        return;
    }

    let client_ref = &*client;
    let session_ref = &mut *session;
    let userdata_ptr = userdata as usize;

    client_ref.runtime.spawn(async move {
        let result = session_ref.commit_transaction().await;
        let userdata = userdata_ptr as *mut c_void;

        match result {
            Ok(()) => callback(userdata, std::ptr::null()),
            Err(e) => {
                let error = Error::from(&e);
                callback(userdata, &error);
            }
        }
    });
}

/// Abort the current transaction.
///
/// # Safety
///
/// - `client` must be a valid pointer to a MongoClient.
/// - `session` must be a valid pointer to a Session with an active transaction.
/// - `callback` will be invoked when the operation completes.
/// - `userdata` is passed to the callback.
#[no_mangle]
pub unsafe extern "C" fn mongo_session_abort_transaction(
    client: *mut MongoClient,
    session: *mut ClientSession,
    callback: TransactionCallback,
    userdata: *mut c_void,
) {
    if client.is_null() {
        let error = InvalidArgumentError::new("client is null").into();
        callback(userdata, &error);
        return;
    }

    if session.is_null() {
        let error = InvalidArgumentError::new("session is null").into();
        callback(userdata, &error);
        return;
    }

    let client_ref = &*client;
    let session_ref = &mut *session;
    let userdata_ptr = userdata as usize;

    client_ref.runtime.spawn(async move {
        let result = session_ref.abort_transaction().await;
        let userdata = userdata_ptr as *mut c_void;

        match result {
            Ok(()) => callback(userdata, std::ptr::null()),
            Err(e) => {
                let error = Error::from(&e);
                callback(userdata, &error);
            }
        }
    });
}
