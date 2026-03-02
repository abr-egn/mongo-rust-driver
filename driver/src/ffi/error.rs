#![allow(missing_docs)]

use std::{
    ffi::{c_char, CString},
    os::raw::c_void,
};

use crate::error::{
    BulkWriteError as RustBulkWriteError,
    CommandError,
    Error,
    ErrorKind,
    InsertManyError as RustInsertManyError,
    WriteFailure,
};

/// Error type discriminator
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType {
    Server = 0,
    InsertMany = 1,
    BulkWrite = 2,
    Io = 3,
    ServerSelection = 4,
    Timeout = 5,
    Auth = 6,
    InvalidArgument = 7,
    Transaction = 8,
    IncompatibleServer = 9,
    InvalidResponse = 10,
    ChangeStream = 11,
    Shutdown = 12,
}

/// Server error (command or write errors)
#[repr(C)]
pub struct ServerErrorFFI {
    pub code: i32,
    pub code_name: *const c_char,
    pub message: *const c_char,
    pub labels: *const *const c_char,
    pub labels_len: usize,
    // TODO: Need FFI type for BSON
    // pub server_response: Bson,
}

/// Individual write error
#[repr(C)]
pub struct WriteErrorFFI {
    pub index: u32,
    pub code: i32,
    pub code_name: *const c_char,
    pub message: *const c_char,
    // TODO: Need FFI type for BSON
    // pub details: Bson,
}

/// Write concern error
#[repr(C)]
pub struct WriteConcernErrorFFI {
    pub code: i32,
    pub code_name: *const c_char,
    pub message: *const c_char,
    // TODO: Need FFI type for BSON
    // pub details: Bson,
    pub labels: *const *const c_char,
    pub labels_len: usize,
}

/// Error from insert_many with partial success info
#[repr(C)]
pub struct InsertManyErrorFFI {
    pub write_errors: *const WriteErrorFFI,
    pub write_errors_len: usize,
    pub write_concern_error: *const WriteConcernErrorFFI,
    // TODO: Need FFI type for BSON
    // pub inserted_ids: Bson,
}

/// Error from client.bulk_write or collection bulk operations
#[repr(C)]
pub struct BulkWriteErrorFFI {
    pub write_errors: *const WriteErrorFFI,
    pub write_errors_len: usize,
    pub write_concern_error: *const WriteConcernErrorFFI,
    pub partial_result: *const c_void,
}

/// IO error
#[repr(C)]
pub struct IoErrorFFI {
    pub message: *const c_char,
}

/// Server selection error
#[repr(C)]
pub struct ServerSelectionErrorFFI {
    pub message: *const c_char,
    pub timeout_ms: i64,
}

/// Timeout error
#[repr(C)]
pub struct TimeoutErrorFFI {
    pub message: *const c_char,
    pub timeout_ms: i64,
}

/// Authentication error
#[repr(C)]
pub struct AuthErrorFFI {
    pub message: *const c_char,
}

/// Invalid argument error
#[repr(C)]
pub struct InvalidArgumentErrorFFI {
    pub message: *const c_char,
}

/// Transaction error
#[repr(C)]
pub struct TransactionErrorFFI {
    pub message: *const c_char,
    pub labels: *const *const c_char,
    pub labels_len: usize,
}

/// Incompatible server error
#[repr(C)]
pub struct IncompatibleServerErrorFFI {
    pub message: *const c_char,
}

/// Invalid response error
#[repr(C)]
pub struct InvalidResponseErrorFFI {
    pub message: *const c_char,
}

/// Change stream error
#[repr(C)]
pub struct ChangeStreamErrorFFI {
    pub message: *const c_char,
    pub resumable: bool,
}

/// Shutdown error
#[repr(C)]
pub struct ShutdownErrorFFI {
    // Empty struct - just indicates shutdown
}

/// Error union - contains pointer to specific error type
#[repr(C)]
pub union ErrorUnion {
    pub server: *const ServerErrorFFI,
    pub insert_many: *const InsertManyErrorFFI,
    pub bulk_write: *const BulkWriteErrorFFI,
    pub io: *const IoErrorFFI,
    pub server_selection: *const ServerSelectionErrorFFI,
    pub timeout: *const TimeoutErrorFFI,
    pub auth: *const AuthErrorFFI,
    pub invalid_argument: *const InvalidArgumentErrorFFI,
    pub transaction: *const TransactionErrorFFI,
    pub incompatible_server: *const IncompatibleServerErrorFFI,
    pub invalid_response: *const InvalidResponseErrorFFI,
    pub change_stream: *const ChangeStreamErrorFFI,
    pub shutdown: *const ShutdownErrorFFI,
}

/// Tagged union for FFI errors
#[repr(C)]
pub struct ErrorFFI {
    pub error_type: u8,
    pub error: ErrorUnion,
}

impl ErrorFFI {
    /// Convert a Rust Error to an FFI error.
    ///
    /// The returned ErrorFFI and all its nested data are owned by the caller
    /// and must be freed using `error_ffi_free()`.
    pub fn from_error(error: &Error) -> Box<Self> {
        match error.kind.as_ref() {
            ErrorKind::Command(cmd_err) => Self::from_command_error(cmd_err, error),
            ErrorKind::Write(write_failure) => Self::from_write_failure(write_failure, error),
            ErrorKind::InsertMany(insert_many_err) => Self::from_insert_many_error(insert_many_err),
            ErrorKind::BulkWrite(bulk_write_err) => Self::from_bulk_write_error(bulk_write_err),
            ErrorKind::Io(io_err) => Self::from_io_error(io_err),
            ErrorKind::ServerSelection { message } => Self::from_server_selection_error(message),
            ErrorKind::Authentication { message } => Self::from_auth_error(message),
            ErrorKind::InvalidArgument { message } => Self::from_invalid_argument_error(message),
            ErrorKind::Transaction { message } => Self::from_transaction_error(message, error),
            ErrorKind::IncompatibleServer { message } => {
                Self::from_incompatible_server_error(message)
            }
            ErrorKind::InvalidResponse { message } => Self::from_invalid_response_error(message),
            ErrorKind::MissingResumeToken => Self::from_change_stream_error(
                "Cannot provide resume functionality when the resume token is missing",
                true,
            ),
            ErrorKind::Shutdown => Self::from_shutdown_error(),
            ErrorKind::ConnectionPoolCleared { message } => Self::from_io_error_message(message),
            ErrorKind::SessionsNotSupported => Self::from_incompatible_server_error(
                "Attempted to start a session on a deployment that does not support sessions",
            ),
            ErrorKind::BsonDeserialization(err) => {
                Self::from_invalid_argument_error(&err.to_string())
            }
            ErrorKind::BsonSerialization(err) => {
                Self::from_invalid_argument_error(&err.to_string())
            }
            #[cfg(feature = "bson-3")]
            ErrorKind::Bson(err) => Self::from_invalid_argument_error(&err.to_string()),
            _ => Self::from_invalid_argument_error(&format!("Unhandled error: {}", error)),
        }
    }

    fn from_command_error(cmd_err: &CommandError, error: &Error) -> Box<Self> {
        let code_name = CString::new(cmd_err.code_name.as_str()).unwrap();
        let message = CString::new(cmd_err.message.as_str()).unwrap();
        let labels = error_labels_to_c_array(error.labels());

        let server_error = Box::new(ServerErrorFFI {
            code: cmd_err.code,
            code_name: code_name.into_raw(),
            message: message.into_raw(),
            labels: labels.0,
            labels_len: labels.1,
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::Server as u8,
            error: ErrorUnion {
                server: Box::into_raw(server_error),
            },
        })
    }

    fn from_write_failure(write_failure: &WriteFailure, error: &Error) -> Box<Self> {
        match write_failure {
            WriteFailure::WriteConcernError(wc_err) => {
                let labels = error_labels_to_c_array(error.labels());

                let server_error = Box::new(ServerErrorFFI {
                    code: wc_err.code,
                    code_name: CString::new(wc_err.code_name.as_str()).unwrap().into_raw(),
                    message: CString::new(wc_err.message.as_str()).unwrap().into_raw(),
                    labels: labels.0,
                    labels_len: labels.1,
                });

                Box::new(ErrorFFI {
                    error_type: ErrorType::Server as u8,
                    error: ErrorUnion {
                        server: Box::into_raw(server_error),
                    },
                })
            }
            WriteFailure::WriteError(write_err) => {
                let code_name = write_err
                    .code_name
                    .as_ref()
                    .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                    .unwrap_or(std::ptr::null_mut());
                let message = CString::new(write_err.message.as_str()).unwrap();
                let labels = error_labels_to_c_array(error.labels());

                let server_error = Box::new(ServerErrorFFI {
                    code: write_err.code,
                    code_name,
                    message: message.into_raw(),
                    labels: labels.0,
                    labels_len: labels.1,
                });

                Box::new(ErrorFFI {
                    error_type: ErrorType::Server as u8,
                    error: ErrorUnion {
                        server: Box::into_raw(server_error),
                    },
                })
            }
        }
    }

    fn from_insert_many_error(insert_many_err: &RustInsertManyError) -> Box<Self> {
        let write_errors_ffi = insert_many_err
            .write_errors
            .as_ref()
            .map(|errors| {
                let ffi_errors: Vec<WriteErrorFFI> = errors
                    .iter()
                    .map(|err| WriteErrorFFI {
                        index: err.index as u32,
                        code: err.code,
                        code_name: err
                            .code_name
                            .as_ref()
                            .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                            .unwrap_or(std::ptr::null_mut()),
                        message: CString::new(err.message.as_str()).unwrap().into_raw(),
                    })
                    .collect();
                let boxed = ffi_errors.into_boxed_slice();
                let len = boxed.len();
                let ptr = Box::into_raw(boxed) as *const WriteErrorFFI;
                (ptr, len)
            })
            .unwrap_or((std::ptr::null(), 0));

        let write_concern_error_ffi = insert_many_err
            .write_concern_error
            .as_ref()
            .map(|wc_err| {
                let labels = string_vec_to_c_array(&wc_err.labels);
                Box::into_raw(Box::new(WriteConcernErrorFFI {
                    code: wc_err.code,
                    code_name: CString::new(wc_err.code_name.as_str()).unwrap().into_raw(),
                    message: CString::new(wc_err.message.as_str()).unwrap().into_raw(),
                    labels: labels.0,
                    labels_len: labels.1,
                }))
            })
            .unwrap_or(std::ptr::null_mut());

        let insert_many_error_ffi = Box::new(InsertManyErrorFFI {
            write_errors: write_errors_ffi.0,
            write_errors_len: write_errors_ffi.1,
            write_concern_error: write_concern_error_ffi,
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::InsertMany as u8,
            error: ErrorUnion {
                insert_many: Box::into_raw(insert_many_error_ffi),
            },
        })
    }

    fn from_bulk_write_error(bulk_write_err: &RustBulkWriteError) -> Box<Self> {
        let write_errors_ffi = if bulk_write_err.write_errors.is_empty() {
            (std::ptr::null(), 0)
        } else {
            let ffi_errors: Vec<WriteErrorFFI> = bulk_write_err
                .write_errors
                .iter()
                .map(|(idx, err)| WriteErrorFFI {
                    index: *idx as u32,
                    code: err.code,
                    code_name: err
                        .code_name
                        .as_ref()
                        .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                        .unwrap_or(std::ptr::null_mut()),
                    message: CString::new(err.message.as_str()).unwrap().into_raw(),
                })
                .collect();
            let boxed = ffi_errors.into_boxed_slice();
            let len = boxed.len();
            let ptr = Box::into_raw(boxed) as *const WriteErrorFFI;
            (ptr, len)
        };

        let write_concern_error_ffi = bulk_write_err
            .write_concern_errors
            .first()
            .map(|wc_err| {
                let labels = string_vec_to_c_array(&wc_err.labels);
                Box::into_raw(Box::new(WriteConcernErrorFFI {
                    code: wc_err.code,
                    code_name: CString::new(wc_err.code_name.as_str()).unwrap().into_raw(),
                    message: CString::new(wc_err.message.as_str()).unwrap().into_raw(),
                    labels: labels.0,
                    labels_len: labels.1,
                }))
            })
            .unwrap_or(std::ptr::null_mut());

        let bulk_write_error_ffi = Box::new(BulkWriteErrorFFI {
            write_errors: write_errors_ffi.0,
            write_errors_len: write_errors_ffi.1,
            write_concern_error: write_concern_error_ffi,
            partial_result: std::ptr::null(),
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::BulkWrite as u8,
            error: ErrorUnion {
                bulk_write: Box::into_raw(bulk_write_error_ffi),
            },
        })
    }

    fn from_io_error(io_err: &std::io::Error) -> Box<Self> {
        Self::from_io_error_message(&io_err.to_string())
    }

    fn from_io_error_message(message: &str) -> Box<Self> {
        let message_cstr = CString::new(message).unwrap();
        let io_error = Box::new(IoErrorFFI {
            message: message_cstr.into_raw(),
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::Io as u8,
            error: ErrorUnion {
                io: Box::into_raw(io_error),
            },
        })
    }

    fn from_server_selection_error(message: &str) -> Box<Self> {
        let message_cstr = CString::new(message).unwrap();
        let error = Box::new(ServerSelectionErrorFFI {
            message: message_cstr.into_raw(),
            timeout_ms: -1,
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::ServerSelection as u8,
            error: ErrorUnion {
                server_selection: Box::into_raw(error),
            },
        })
    }

    fn from_auth_error(message: &str) -> Box<Self> {
        let message_cstr = CString::new(message).unwrap();
        let error = Box::new(AuthErrorFFI {
            message: message_cstr.into_raw(),
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::Auth as u8,
            error: ErrorUnion {
                auth: Box::into_raw(error),
            },
        })
    }

    fn from_invalid_argument_error(message: &str) -> Box<Self> {
        let message_cstr = CString::new(message).unwrap();
        let error = Box::new(InvalidArgumentErrorFFI {
            message: message_cstr.into_raw(),
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::InvalidArgument as u8,
            error: ErrorUnion {
                invalid_argument: Box::into_raw(error),
            },
        })
    }

    fn from_transaction_error(message: &str, error: &Error) -> Box<Self> {
        let message_cstr = CString::new(message).unwrap();
        let labels = error_labels_to_c_array(error.labels());
        let trans_error = Box::new(TransactionErrorFFI {
            message: message_cstr.into_raw(),
            labels: labels.0,
            labels_len: labels.1,
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::Transaction as u8,
            error: ErrorUnion {
                transaction: Box::into_raw(trans_error),
            },
        })
    }

    fn from_incompatible_server_error(message: &str) -> Box<Self> {
        let message_cstr = CString::new(message).unwrap();
        let error = Box::new(IncompatibleServerErrorFFI {
            message: message_cstr.into_raw(),
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::IncompatibleServer as u8,
            error: ErrorUnion {
                incompatible_server: Box::into_raw(error),
            },
        })
    }

    fn from_invalid_response_error(message: &str) -> Box<Self> {
        let message_cstr = CString::new(message).unwrap();
        let error = Box::new(InvalidResponseErrorFFI {
            message: message_cstr.into_raw(),
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::InvalidResponse as u8,
            error: ErrorUnion {
                invalid_response: Box::into_raw(error),
            },
        })
    }

    fn from_change_stream_error(message: &str, resumable: bool) -> Box<Self> {
        let message_cstr = CString::new(message).unwrap();
        let error = Box::new(ChangeStreamErrorFFI {
            message: message_cstr.into_raw(),
            resumable,
        });

        Box::new(ErrorFFI {
            error_type: ErrorType::ChangeStream as u8,
            error: ErrorUnion {
                change_stream: Box::into_raw(error),
            },
        })
    }

    fn from_shutdown_error() -> Box<Self> {
        let error = Box::new(ShutdownErrorFFI {});

        Box::new(ErrorFFI {
            error_type: ErrorType::Shutdown as u8,
            error: ErrorUnion {
                shutdown: Box::into_raw(error),
            },
        })
    }
}

/// Convert error labels to a C array of C strings.
/// Returns (pointer to array, length).
/// The returned array and all strings must be freed by the caller.
fn error_labels_to_c_array(
    labels: &std::collections::HashSet<String>,
) -> (*const *const c_char, usize) {
    if labels.is_empty() {
        return (std::ptr::null(), 0);
    }

    let ptrs: Vec<*mut c_char> = labels
        .iter()
        .map(|s| CString::new(s.as_str()).unwrap().into_raw())
        .collect();

    let len = ptrs.len();
    let boxed_ptrs = ptrs.into_boxed_slice();
    let ptr = Box::into_raw(boxed_ptrs) as *const *const c_char;

    (ptr, len)
}

/// Convert a Vec<String> to a C array of C strings.
/// Returns (pointer to array, length).
/// The returned array and all strings must be freed by the caller.
fn string_vec_to_c_array(labels: &[String]) -> (*const *const c_char, usize) {
    if labels.is_empty() {
        return (std::ptr::null(), 0);
    }

    let ptrs: Vec<*mut c_char> = labels
        .iter()
        .map(|s| CString::new(s.as_str()).unwrap().into_raw())
        .collect();

    let len = ptrs.len();
    let boxed_ptrs = ptrs.into_boxed_slice();
    let ptr = Box::into_raw(boxed_ptrs) as *const *const c_char;

    (ptr, len)
}

/// Free a C string array and all its strings.
///
/// # Safety
///
/// `array` must be a valid pointer to an array of C strings with `len` elements.
unsafe fn free_c_string_array(array: *const *const c_char, len: usize) {
    if array.is_null() {
        return;
    }

    let strings = Vec::from_raw_parts(array as *mut *mut c_char, len, len);
    for ptr in strings {
        if !ptr.is_null() {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Free an ErrorFFI and all its nested data.
///
/// # Safety
///
/// `error_ptr` must be a valid pointer returned from `ErrorFFI::from_error()`.
/// After calling this function, the pointer becomes invalid.
pub unsafe extern "C" fn error_ffi_free(error_ptr: *mut ErrorFFI) {
    if error_ptr.is_null() {
        return;
    }

    let error = Box::from_raw(error_ptr);

    match error.error_type {
        0 => {
            // Server error
            if !error.error.server.is_null() {
                let server_error = Box::from_raw(error.error.server as *mut ServerErrorFFI);
                if !server_error.code_name.is_null() {
                    let _ = CString::from_raw(server_error.code_name as *mut c_char);
                }
                if !server_error.message.is_null() {
                    let _ = CString::from_raw(server_error.message as *mut c_char);
                }
                free_c_string_array(server_error.labels, server_error.labels_len);
            }
        }
        1 => {
            // InsertMany error
            if !error.error.insert_many.is_null() {
                let insert_many_error =
                    Box::from_raw(error.error.insert_many as *mut InsertManyErrorFFI);

                if !insert_many_error.write_errors.is_null() {
                    let write_errors = Vec::from_raw_parts(
                        insert_many_error.write_errors as *mut WriteErrorFFI,
                        insert_many_error.write_errors_len,
                        insert_many_error.write_errors_len,
                    );
                    for we in write_errors {
                        if !we.code_name.is_null() {
                            let _ = CString::from_raw(we.code_name as *mut c_char);
                        }
                        if !we.message.is_null() {
                            let _ = CString::from_raw(we.message as *mut c_char);
                        }
                    }
                }

                if !insert_many_error.write_concern_error.is_null() {
                    let wc_error = Box::from_raw(
                        insert_many_error.write_concern_error as *mut WriteConcernErrorFFI,
                    );
                    if !wc_error.code_name.is_null() {
                        let _ = CString::from_raw(wc_error.code_name as *mut c_char);
                    }
                    if !wc_error.message.is_null() {
                        let _ = CString::from_raw(wc_error.message as *mut c_char);
                    }
                    free_c_string_array(wc_error.labels, wc_error.labels_len);
                }
            }
        }
        2 => {
            // BulkWrite error
            if !error.error.bulk_write.is_null() {
                let bulk_write_error =
                    Box::from_raw(error.error.bulk_write as *mut BulkWriteErrorFFI);

                if !bulk_write_error.write_errors.is_null() {
                    let write_errors = Vec::from_raw_parts(
                        bulk_write_error.write_errors as *mut WriteErrorFFI,
                        bulk_write_error.write_errors_len,
                        bulk_write_error.write_errors_len,
                    );
                    for we in write_errors {
                        if !we.code_name.is_null() {
                            let _ = CString::from_raw(we.code_name as *mut c_char);
                        }
                        if !we.message.is_null() {
                            let _ = CString::from_raw(we.message as *mut c_char);
                        }
                    }
                }

                if !bulk_write_error.write_concern_error.is_null() {
                    let wc_error = Box::from_raw(
                        bulk_write_error.write_concern_error as *mut WriteConcernErrorFFI,
                    );
                    if !wc_error.code_name.is_null() {
                        let _ = CString::from_raw(wc_error.code_name as *mut c_char);
                    }
                    if !wc_error.message.is_null() {
                        let _ = CString::from_raw(wc_error.message as *mut c_char);
                    }
                    free_c_string_array(wc_error.labels, wc_error.labels_len);
                }
            }
        }
        3 => {
            // IO error
            if !error.error.io.is_null() {
                let io_error = Box::from_raw(error.error.io as *mut IoErrorFFI);
                if !io_error.message.is_null() {
                    let _ = CString::from_raw(io_error.message as *mut c_char);
                }
            }
        }
        4 => {
            // ServerSelection error
            if !error.error.server_selection.is_null() {
                let ss_error =
                    Box::from_raw(error.error.server_selection as *mut ServerSelectionErrorFFI);
                if !ss_error.message.is_null() {
                    let _ = CString::from_raw(ss_error.message as *mut c_char);
                }
            }
        }
        5 => {
            // Timeout error
            if !error.error.timeout.is_null() {
                let timeout_error = Box::from_raw(error.error.timeout as *mut TimeoutErrorFFI);
                if !timeout_error.message.is_null() {
                    let _ = CString::from_raw(timeout_error.message as *mut c_char);
                }
            }
        }
        6 => {
            // Auth error
            if !error.error.auth.is_null() {
                let auth_error = Box::from_raw(error.error.auth as *mut AuthErrorFFI);
                if !auth_error.message.is_null() {
                    let _ = CString::from_raw(auth_error.message as *mut c_char);
                }
            }
        }
        7 => {
            // InvalidArgument error
            if !error.error.invalid_argument.is_null() {
                let invalid_arg_error =
                    Box::from_raw(error.error.invalid_argument as *mut InvalidArgumentErrorFFI);
                if !invalid_arg_error.message.is_null() {
                    let _ = CString::from_raw(invalid_arg_error.message as *mut c_char);
                }
            }
        }
        8 => {
            // Transaction error
            if !error.error.transaction.is_null() {
                let transaction_error =
                    Box::from_raw(error.error.transaction as *mut TransactionErrorFFI);
                if !transaction_error.message.is_null() {
                    let _ = CString::from_raw(transaction_error.message as *mut c_char);
                }
                free_c_string_array(transaction_error.labels, transaction_error.labels_len);
            }
        }
        9 => {
            // IncompatibleServer error
            if !error.error.incompatible_server.is_null() {
                let incompatible_error = Box::from_raw(
                    error.error.incompatible_server as *mut IncompatibleServerErrorFFI,
                );
                if !incompatible_error.message.is_null() {
                    let _ = CString::from_raw(incompatible_error.message as *mut c_char);
                }
            }
        }
        10 => {
            // InvalidResponse error
            if !error.error.invalid_response.is_null() {
                let invalid_response_error =
                    Box::from_raw(error.error.invalid_response as *mut InvalidResponseErrorFFI);
                if !invalid_response_error.message.is_null() {
                    let _ = CString::from_raw(invalid_response_error.message as *mut c_char);
                }
            }
        }
        11 => {
            // ChangeStream error
            if !error.error.change_stream.is_null() {
                let change_stream_error =
                    Box::from_raw(error.error.change_stream as *mut ChangeStreamErrorFFI);
                if !change_stream_error.message.is_null() {
                    let _ = CString::from_raw(change_stream_error.message as *mut c_char);
                }
            }
        }
        12 => {
            // Shutdown error
            if !error.error.shutdown.is_null() {
                let _ = Box::from_raw(error.error.shutdown as *mut ShutdownErrorFFI);
            }
        }
        _ => {}
    }
}
