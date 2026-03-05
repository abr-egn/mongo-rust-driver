//! Callback dispatch mechanism for FFI.
//!
//! This module provides a way to dispatch completed operation callbacks to a
//! caller-provided executor/event loop, rather than invoking them directly on
//! Tokio threads.
//!
//! This is useful for language runtimes (Java, C++, etc.) that need callbacks
//! to run on their own threads with proper thread context.

use std::ffi::c_void;

use super::client::MongoClient;

/// Opaque handle to a completed operation's callback and result.
///
/// Created by the driver when an operation completes. The caller should
/// pass this to their event loop and call `mongo_pending_callback_invoke`
/// when ready to process it.
#[repr(C)]
pub struct PendingCallback {
    // Type-erased invocation function.
    // The actual callback + result data follows in the concrete type.
    invoke_fn: unsafe fn(*mut PendingCallback),
}

/// Function pointer type for the dispatcher.
///
/// When set, the driver calls this instead of invoking callbacks directly.
/// The dispatcher should post the `PendingCallback` to its event loop and
/// call `mongo_pending_callback_invoke` when ready.
///
/// - `pending`: Handle to the completed operation (pass to invoke)
/// - `context`: User-provided context from `mongo_client_set_dispatcher`
pub type DispatchFn = extern "C" fn(pending: *mut PendingCallback, context: *mut c_void);

/// Set a dispatcher for callback invocation.
///
/// When set, operation callbacks will be dispatched through this function
/// instead of being invoked directly on Tokio threads.
///
/// Pass null to clear the dispatcher and revert to direct invocation.
///
/// # Safety
///
/// - `client` must be a valid pointer from `mongo_client_new`
/// - `dispatch_fn` can be null (to clear) or a valid function pointer
/// - `context` is passed to the dispatcher and can be any value
#[no_mangle]
pub unsafe extern "C" fn mongo_client_set_dispatcher(
    client: *mut MongoClient,
    dispatch_fn: DispatchFn,
    context: *mut c_void,
) {
    if client.is_null() {
        return;
    }
    let client = &mut *client;
    // Check if dispatch_fn is a null pointer (transmuted)
    let fn_ptr = dispatch_fn as *const ();
    if fn_ptr.is_null() {
        client.dispatcher = None;
    } else {
        client.dispatcher = Some(Dispatcher {
            dispatch_fn,
            context,
        });
    }
}

/// Invoke a pending callback.
///
/// This should be called from the target thread (e.g., Java executor thread)
/// after the dispatcher has posted the work.
///
/// After this call, the `pending` pointer is invalid.
///
/// # Safety
///
/// - `pending` must be a valid pointer received from a dispatcher callback
/// - `pending` must only be invoked once
#[no_mangle]
pub unsafe extern "C" fn mongo_pending_callback_invoke(pending: *mut PendingCallback) {
    if pending.is_null() {
        return;
    }
    // Call the type-erased invoke function
    ((*pending).invoke_fn)(pending);
}

/// Internal dispatcher state stored on MongoClient
#[derive(Clone, Copy)]
pub(super) struct Dispatcher {
    pub dispatch_fn: DispatchFn,
    pub context: *mut c_void,
}

// Safety: The context pointer is provided by the caller who is responsible
// for ensuring thread-safety of whatever it points to.
unsafe impl Send for Dispatcher {}
unsafe impl Sync for Dispatcher {}

/// Concrete pending callback for a specific result type.
///
/// This struct is heap-allocated when dispatch is used, and freed
/// when `mongo_pending_callback_invoke` is called.
#[repr(C)]
pub(super) struct PendingCallbackImpl<Out, Err> {
    // Must be first field - allows casting between PendingCallback and PendingCallbackImpl.
    // Not read directly, only via pointer cast in invoke().
    #[allow(dead_code)]
    base: PendingCallback,
    callback: extern "C" fn(*mut c_void, *const Out, *const Err),
    userdata: *mut c_void,
    result: Result<Out, Err>,
}

impl<Out, Err> PendingCallbackImpl<Out, Err> {
    pub fn new(
        callback: extern "C" fn(*mut c_void, *const Out, *const Err),
        userdata: *mut c_void,
        result: Result<Out, Err>,
    ) -> Box<Self> {
        Box::new(Self {
            base: PendingCallback {
                invoke_fn: Self::invoke,
            },
            callback,
            userdata,
            result,
        })
    }

    /// Type-erased invoke function
    unsafe fn invoke(pending: *mut PendingCallback) {
        // Cast back to concrete type
        let concrete = Box::from_raw(pending as *mut Self);
        match &concrete.result {
            Ok(out) => (concrete.callback)(concrete.userdata, out, std::ptr::null()),
            Err(e) => (concrete.callback)(concrete.userdata, std::ptr::null(), e),
        }
        // Box dropped here, frees memory
    }
}

