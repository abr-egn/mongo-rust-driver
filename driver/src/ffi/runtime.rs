//! Global Tokio runtime management for FFI.
//!
//! This module provides a reference-counted global Tokio runtime that is shared
//! across all MongoClient instances created through the FFI. The runtime is
//! created on first client creation and destroyed when the last client is destroyed.

use std::sync::{Arc, Mutex, OnceLock, Weak};

use tokio::runtime::Runtime;

/// Global weak reference to the FFI runtime.
/// Using Weak allows the runtime to be dropped when all MongoClient instances are destroyed.
static RUNTIME: OnceLock<Mutex<Weak<Runtime>>> = OnceLock::new();

/// Get a reference to the global runtime, creating it if necessary.
///
/// Returns an Arc<Runtime> that keeps the runtime alive. When all Arc references
/// are dropped, the runtime will be shut down automatically.
///
/// # Panics
///
/// Panics if the runtime cannot be created (e.g., system resources exhausted).
pub(super) fn acquire_runtime() -> Arc<Runtime> {
    let weak_runtime = RUNTIME.get_or_init(|| Mutex::new(Weak::new()));
    let mut guard = weak_runtime.lock().expect("FFI runtime mutex poisoned");

    if let Some(runtime) = guard.upgrade() {
        return runtime;
    }

    let runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime for FFI"));
    *guard = Arc::downgrade(&runtime);
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_lifecycle() {
        let runtime1 = acquire_runtime();
        let runtime2 = acquire_runtime();
        assert!(Arc::ptr_eq(&runtime1, &runtime2));

        drop(runtime1);
        // Runtime should still exist because runtime2 holds a reference
        let runtime3 = acquire_runtime();
        assert!(Arc::ptr_eq(&runtime2, &runtime3));

        drop(runtime2);
        drop(runtime3);

        // After all Arc references are dropped, acquiring again should create a new runtime
        let runtime4 = acquire_runtime();
        drop(runtime4);
    }
}
