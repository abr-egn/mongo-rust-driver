#![allow(missing_docs)]

use std::{
    ffi::{c_char, CString},
    os::raw::c_void,
};

use crate::error::{
    BulkWriteError as RustBulkWriteError,
    CommandError,
    Error as RustError,
    ErrorKind,
    InsertManyError as RustInsertManyError,
    WriteFailure,
};

use super::types::OwnedBson;

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

/// Error union - contains pointer to specific error type
#[repr(C)]
pub union ErrorUnion {
    pub server: *const ServerError,
    pub insert_many: *const InsertManyError,
    pub bulk_write: *const BulkWriteError,
    pub io: *const IoError,
    pub server_selection: *const ServerSelectionError,
    pub timeout: *const TimeoutError,
    pub auth: *const AuthError,
    pub invalid_argument: *const InvalidArgumentError,
    pub transaction: *const TransactionError,
    pub incompatible_server: *const IncompatibleServerError,
    pub invalid_response: *const InvalidResponseError,
    pub change_stream: *const ChangeStreamError,
    pub shutdown: *const ShutdownError,
}

/// Tagged union for FFI errors
#[repr(C)]
pub struct Error {
    // `u8` rather than `ErrorType` to avoid undefined behavior from invalid values
    pub error_type: u8,
    pub error: ErrorUnion,
}

impl From<&RustError> for Error {
    fn from(error: &RustError) -> Self {
        match error.kind.as_ref() {
            ErrorKind::Command(e) => ServerError::from_command_error(e, error).into(),
            ErrorKind::Write(WriteFailure::WriteConcernError(e)) => {
                ServerError::from_write_concern_error(e, error).into()
            }
            ErrorKind::Write(WriteFailure::WriteError(e)) => {
                ServerError::from_write_error(e, error).into()
            }
            ErrorKind::InsertMany(e) => InsertManyError::from_rust(e).into(),
            ErrorKind::BulkWrite(e) => BulkWriteError::from_rust(e).into(),
            ErrorKind::Io(e) => IoError::from_io_error(e).into(),
            ErrorKind::ServerSelection { message } => ServerSelectionError::new(message).into(),
            ErrorKind::Authentication { message } => AuthError::new(message).into(),
            ErrorKind::InvalidArgument { message } => InvalidArgumentError::new(message).into(),
            ErrorKind::Transaction { message } => TransactionError::new(message, error).into(),
            ErrorKind::IncompatibleServer { message } => {
                IncompatibleServerError::new(message).into()
            }
            ErrorKind::InvalidResponse { message } => InvalidResponseError::new(message).into(),
            ErrorKind::MissingResumeToken => ChangeStreamError::new(
                "Cannot provide resume functionality when the resume token is missing",
                true,
            )
            .into(),
            ErrorKind::Shutdown => ShutdownError::new().into(),
            ErrorKind::ConnectionPoolCleared { message } => IoError::new(message).into(),
            ErrorKind::SessionsNotSupported => IncompatibleServerError::new(
                "Attempted to start a session on a deployment that does not support sessions",
            )
            .into(),
            ErrorKind::BsonDeserialization(e) => InvalidArgumentError::new(&e.to_string()).into(),
            ErrorKind::BsonSerialization(e) => InvalidArgumentError::new(&e.to_string()).into(),
            #[cfg(feature = "bson-3")]
            ErrorKind::Bson(e) => InvalidArgumentError::new(&e.to_string()).into(),
            _ => InvalidArgumentError::new(&format!("Unhandled error: {}", error)).into(),
        }
    }
}

/// Free an Error and all its nested data.
///
/// # Safety
///
/// `error_ptr` must be a valid pointer created in Rust.
/// After calling this function, the pointer becomes invalid.
pub unsafe extern "C" fn error_free(error_ptr: *mut Error) {
    if error_ptr.is_null() {
        return;
    }

    drop(Box::from_raw(error_ptr));
}

impl Drop for Error {
    fn drop(&mut self) {
        if self.error_type > ErrorType::Shutdown as u8 {
            return;
        }
        unsafe {
            let error_type: ErrorType = std::mem::transmute(self.error_type);
            match error_type {
                ErrorType::Server => {
                    if !self.error.server.is_null() {
                        let _ = Box::from_raw(self.error.server as *mut ServerError);
                    }
                }
                ErrorType::InsertMany => {
                    if !self.error.insert_many.is_null() {
                        let _ = Box::from_raw(self.error.insert_many as *mut InsertManyError);
                    }
                }
                ErrorType::BulkWrite => {
                    if !self.error.bulk_write.is_null() {
                        let _ = Box::from_raw(self.error.bulk_write as *mut BulkWriteError);
                    }
                }
                ErrorType::Io => {
                    if !self.error.io.is_null() {
                        let _ = Box::from_raw(self.error.io as *mut IoError);
                    }
                }
                ErrorType::ServerSelection => {
                    if !self.error.server_selection.is_null() {
                        let _ =
                            Box::from_raw(self.error.server_selection as *mut ServerSelectionError);
                    }
                }
                ErrorType::Timeout => {
                    if !self.error.timeout.is_null() {
                        let _ = Box::from_raw(self.error.timeout as *mut TimeoutError);
                    }
                }
                ErrorType::Auth => {
                    if !self.error.auth.is_null() {
                        let _ = Box::from_raw(self.error.auth as *mut AuthError);
                    }
                }
                ErrorType::InvalidArgument => {
                    if !self.error.invalid_argument.is_null() {
                        let _ =
                            Box::from_raw(self.error.invalid_argument as *mut InvalidArgumentError);
                    }
                }
                ErrorType::Transaction => {
                    if !self.error.transaction.is_null() {
                        let _ = Box::from_raw(self.error.transaction as *mut TransactionError);
                    }
                }
                ErrorType::IncompatibleServer => {
                    if !self.error.incompatible_server.is_null() {
                        let _ = Box::from_raw(
                            self.error.incompatible_server as *mut IncompatibleServerError,
                        );
                    }
                }
                ErrorType::InvalidResponse => {
                    if !self.error.invalid_response.is_null() {
                        let _ =
                            Box::from_raw(self.error.invalid_response as *mut InvalidResponseError);
                    }
                }
                ErrorType::ChangeStream => {
                    if !self.error.change_stream.is_null() {
                        let _ = Box::from_raw(self.error.change_stream as *mut ChangeStreamError);
                    }
                }
                ErrorType::Shutdown => {
                    if !self.error.shutdown.is_null() {
                        let _ = Box::from_raw(self.error.shutdown as *mut ShutdownError);
                    }
                }
            }
        }
    }
}

/// Server error (command or write errors)
#[repr(C)]
pub struct ServerError {
    pub code: i32,
    pub code_name: *const c_char,
    pub message: *const c_char,
    pub labels: *const *const c_char,
    pub labels_len: usize,
    pub server_response: OwnedBson,
}

impl ServerError {
    fn from_command_error(cmd_err: &CommandError, error: &RustError) -> Self {
        let code_name = CString::new(cmd_err.code_name.as_str()).unwrap();
        let message = CString::new(cmd_err.message.as_str()).unwrap();
        let labels = error_labels_to_c_array(error.labels());
        let server_response = error
            .server_response()
            .map(|d| OwnedBson::from_raw(&*d))
            .unwrap_or_else(OwnedBson::empty);

        Self {
            code: cmd_err.code,
            code_name: code_name.into_raw(),
            message: message.into_raw(),
            labels: labels.0,
            labels_len: labels.1,
            server_response,
        }
    }

    fn from_write_concern_error(
        wc_err: &crate::error::WriteConcernError,
        error: &RustError,
    ) -> Self {
        let labels = error_labels_to_c_array(error.labels());
        let server_response = error
            .server_response()
            .map(|d| OwnedBson::from_raw(&*d))
            .unwrap_or_else(OwnedBson::empty);

        Self {
            code: wc_err.code,
            code_name: CString::new(wc_err.code_name.as_str()).unwrap().into_raw(),
            message: CString::new(wc_err.message.as_str()).unwrap().into_raw(),
            labels: labels.0,
            labels_len: labels.1,
            server_response,
        }
    }

    fn from_write_error(write_err: &crate::error::WriteError, error: &RustError) -> Self {
        let code_name = write_err
            .code_name
            .as_ref()
            .map(|s| CString::new(s.as_str()).unwrap().into_raw())
            .unwrap_or(std::ptr::null_mut());
        let message = CString::new(write_err.message.as_str()).unwrap();
        let labels = error_labels_to_c_array(error.labels());
        let server_response = error
            .server_response()
            .map(|d| OwnedBson::from_raw(&*d))
            .unwrap_or_else(OwnedBson::empty);

        Self {
            code: write_err.code,
            code_name,
            message: message.into_raw(),
            labels: labels.0,
            labels_len: labels.1,
            server_response,
        }
    }
}

impl From<ServerError> for Error {
    fn from(error: ServerError) -> Self {
        Self {
            error_type: ErrorType::Server as u8,
            error: ErrorUnion {
                server: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for ServerError {
    fn drop(&mut self) {
        let Self {
            code: _,
            code_name,
            message,
            labels,
            labels_len,
            server_response: _,
        } = self;
        unsafe {
            if !code_name.is_null() {
                let _ = CString::from_raw(*code_name as *mut c_char);
            }
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
            free_c_string_array(*labels, *labels_len);
        }
    }
}

/// Individual write error
#[repr(C)]
pub struct WriteError {
    pub index: u32,
    pub code: i32,
    pub code_name: *const c_char,
    pub message: *const c_char,
    pub details: OwnedBson,
}

impl WriteError {
    fn from_rust(index: usize, err: &crate::error::WriteError) -> Self {
        let details = err
            .details
            .as_ref()
            .map(OwnedBson::from_doc)
            .unwrap_or_else(OwnedBson::empty);
        Self {
            index: index as u32,
            code: err.code,
            code_name: err
                .code_name
                .as_ref()
                .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                .unwrap_or(std::ptr::null_mut()),
            message: CString::new(err.message.as_str()).unwrap().into_raw(),
            details,
        }
    }

    fn from_indexed(err: &crate::error::IndexedWriteError) -> Self {
        let details = err
            .details
            .as_ref()
            .map(OwnedBson::from_doc)
            .unwrap_or_else(OwnedBson::empty);
        Self {
            index: err.index as u32,
            code: err.code,
            code_name: err
                .code_name
                .as_ref()
                .map(|s| CString::new(s.as_str()).unwrap().into_raw())
                .unwrap_or(std::ptr::null_mut()),
            message: CString::new(err.message.as_str()).unwrap().into_raw(),
            details,
        }
    }
}

impl Drop for WriteError {
    fn drop(&mut self) {
        let Self {
            index: _,
            code: _,
            code_name,
            message,
            details: _,
        } = self;
        unsafe {
            if !code_name.is_null() {
                let _ = CString::from_raw(*code_name as *mut c_char);
            }
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Write concern error
#[repr(C)]
pub struct WriteConcernError {
    pub code: i32,
    pub code_name: *const c_char,
    pub message: *const c_char,
    pub details: OwnedBson,
    pub labels: *const *const c_char,
    pub labels_len: usize,
}

impl WriteConcernError {
    fn from_rust(wc_err: &crate::error::WriteConcernError) -> Self {
        let labels = string_vec_to_c_array(&wc_err.labels);
        let details = wc_err
            .details
            .as_ref()
            .map(OwnedBson::from_doc)
            .unwrap_or_else(OwnedBson::empty);
        Self {
            code: wc_err.code,
            code_name: CString::new(wc_err.code_name.as_str()).unwrap().into_raw(),
            message: CString::new(wc_err.message.as_str()).unwrap().into_raw(),
            details,
            labels: labels.0,
            labels_len: labels.1,
        }
    }
}

impl Drop for WriteConcernError {
    fn drop(&mut self) {
        let Self {
            code: _,
            code_name,
            message,
            details: _,
            labels,
            labels_len,
        } = self;
        unsafe {
            if !code_name.is_null() {
                let _ = CString::from_raw(*code_name as *mut c_char);
            }
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
            free_c_string_array(*labels, *labels_len);
        }
    }
}

/// Error from insert_many with partial success info
#[repr(C)]
pub struct InsertManyError {
    pub write_errors: *const WriteError,
    pub write_errors_len: usize,
    pub write_concern_error: *const WriteConcernError,
    pub inserted_ids: OwnedBson,
}

impl InsertManyError {
    fn from_rust(err: &RustInsertManyError) -> Self {
        let write_errors_ffi = err
            .write_errors
            .as_ref()
            .map(|errors| {
                let ffi_errors: Vec<WriteError> =
                    errors.iter().map(|e| WriteError::from_indexed(e)).collect();
                let boxed = ffi_errors.into_boxed_slice();
                let len = boxed.len();
                let ptr = Box::into_raw(boxed) as *const WriteError;
                (ptr, len)
            })
            .unwrap_or((std::ptr::null(), 0));

        let write_concern_error_ffi = err
            .write_concern_error
            .as_ref()
            .map(|wc_err| Box::into_raw(Box::new(WriteConcernError::from_rust(wc_err))))
            .unwrap_or(std::ptr::null_mut());

        let inserted_ids_doc: crate::bson::Document = err
            .inserted_ids
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        let inserted_ids = OwnedBson::from_doc(&inserted_ids_doc);

        Self {
            write_errors: write_errors_ffi.0,
            write_errors_len: write_errors_ffi.1,
            write_concern_error: write_concern_error_ffi,
            inserted_ids,
        }
    }
}

impl From<InsertManyError> for Error {
    fn from(error: InsertManyError) -> Self {
        Self {
            error_type: ErrorType::InsertMany as u8,
            error: ErrorUnion {
                insert_many: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for InsertManyError {
    fn drop(&mut self) {
        let Self {
            write_errors,
            write_errors_len,
            write_concern_error,
            inserted_ids: _,
        } = self;
        unsafe {
            if !write_errors.is_null() {
                let errors = Vec::from_raw_parts(
                    *write_errors as *mut WriteError,
                    *write_errors_len,
                    *write_errors_len,
                );
                drop(errors);
            }
            if !write_concern_error.is_null() {
                let _ = Box::from_raw(*write_concern_error as *mut WriteConcernError);
            }
        }
    }
}

/// Error from client.bulk_write or collection bulk operations
#[repr(C)]
pub struct BulkWriteError {
    pub write_errors: *const WriteError,
    pub write_errors_len: usize,
    pub write_concern_error: *const WriteConcernError,
    pub partial_result: *const c_void,
}

impl BulkWriteError {
    fn from_rust(err: &RustBulkWriteError) -> Self {
        let write_errors_ffi = if err.write_errors.is_empty() {
            (std::ptr::null(), 0)
        } else {
            let ffi_errors: Vec<WriteError> = err
                .write_errors
                .iter()
                .map(|(idx, e)| WriteError::from_rust(*idx, e))
                .collect();
            let boxed = ffi_errors.into_boxed_slice();
            let len = boxed.len();
            let ptr = Box::into_raw(boxed) as *const WriteError;
            (ptr, len)
        };

        let write_concern_error_ffi = err
            .write_concern_errors
            .first()
            .map(|wc_err| Box::into_raw(Box::new(WriteConcernError::from_rust(wc_err))))
            .unwrap_or(std::ptr::null_mut());

        Self {
            write_errors: write_errors_ffi.0,
            write_errors_len: write_errors_ffi.1,
            write_concern_error: write_concern_error_ffi,
            partial_result: std::ptr::null(),
        }
    }
}

impl From<BulkWriteError> for Error {
    fn from(error: BulkWriteError) -> Self {
        Self {
            error_type: ErrorType::BulkWrite as u8,
            error: ErrorUnion {
                bulk_write: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for BulkWriteError {
    fn drop(&mut self) {
        let Self {
            write_errors,
            write_errors_len,
            write_concern_error,
            partial_result: _,
        } = self;
        unsafe {
            if !write_errors.is_null() {
                let errors = Vec::from_raw_parts(
                    *write_errors as *mut WriteError,
                    *write_errors_len,
                    *write_errors_len,
                );
                drop(errors);
            }
            if !write_concern_error.is_null() {
                let _ = Box::from_raw(*write_concern_error as *mut WriteConcernError);
            }
        }
    }
}

/// IO error
#[repr(C)]
pub struct IoError {
    pub message: *const c_char,
}

impl IoError {
    fn new(message: &str) -> Self {
        Self {
            message: CString::new(message).unwrap().into_raw(),
        }
    }

    fn from_io_error(io_err: &std::io::Error) -> Self {
        Self::new(&io_err.to_string())
    }
}

impl From<IoError> for Error {
    fn from(error: IoError) -> Self {
        Self {
            error_type: ErrorType::Io as u8,
            error: ErrorUnion {
                io: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for IoError {
    fn drop(&mut self) {
        let Self { message } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Server selection error
#[repr(C)]
pub struct ServerSelectionError {
    pub message: *const c_char,
    pub timeout_ms: i64,
}

impl ServerSelectionError {
    fn new(message: &str) -> Self {
        Self {
            message: CString::new(message).unwrap().into_raw(),
            timeout_ms: -1,
        }
    }
}

impl From<ServerSelectionError> for Error {
    fn from(error: ServerSelectionError) -> Self {
        Self {
            error_type: ErrorType::ServerSelection as u8,
            error: ErrorUnion {
                server_selection: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for ServerSelectionError {
    fn drop(&mut self) {
        let Self {
            message,
            timeout_ms: _,
        } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Timeout error
#[repr(C)]
pub struct TimeoutError {
    pub message: *const c_char,
    pub timeout_ms: i64,
}

impl Drop for TimeoutError {
    fn drop(&mut self) {
        let Self {
            message,
            timeout_ms: _,
        } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Authentication error
#[repr(C)]
pub struct AuthError {
    pub message: *const c_char,
}

impl AuthError {
    fn new(message: &str) -> Self {
        Self {
            message: CString::new(message).unwrap().into_raw(),
        }
    }
}

impl From<AuthError> for Error {
    fn from(error: AuthError) -> Self {
        Self {
            error_type: ErrorType::Auth as u8,
            error: ErrorUnion {
                auth: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for AuthError {
    fn drop(&mut self) {
        let Self { message } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Invalid argument error
#[repr(C)]
pub struct InvalidArgumentError {
    pub message: *const c_char,
}

impl InvalidArgumentError {
    fn new(message: &str) -> Self {
        Self {
            message: CString::new(message).unwrap().into_raw(),
        }
    }
}

impl From<InvalidArgumentError> for Error {
    fn from(error: InvalidArgumentError) -> Self {
        Self {
            error_type: ErrorType::InvalidArgument as u8,
            error: ErrorUnion {
                invalid_argument: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for InvalidArgumentError {
    fn drop(&mut self) {
        let Self { message } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Transaction error
#[repr(C)]
pub struct TransactionError {
    pub message: *const c_char,
    pub labels: *const *const c_char,
    pub labels_len: usize,
}

impl TransactionError {
    fn new(message: &str, error: &RustError) -> Self {
        let labels = error_labels_to_c_array(error.labels());
        Self {
            message: CString::new(message).unwrap().into_raw(),
            labels: labels.0,
            labels_len: labels.1,
        }
    }
}

impl From<TransactionError> for Error {
    fn from(error: TransactionError) -> Self {
        Self {
            error_type: ErrorType::Transaction as u8,
            error: ErrorUnion {
                transaction: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for TransactionError {
    fn drop(&mut self) {
        let Self {
            message,
            labels,
            labels_len,
        } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
            free_c_string_array(*labels, *labels_len);
        }
    }
}

/// Incompatible server error
#[repr(C)]
pub struct IncompatibleServerError {
    pub message: *const c_char,
}

impl IncompatibleServerError {
    fn new(message: &str) -> Self {
        Self {
            message: CString::new(message).unwrap().into_raw(),
        }
    }
}

impl From<IncompatibleServerError> for Error {
    fn from(error: IncompatibleServerError) -> Self {
        Self {
            error_type: ErrorType::IncompatibleServer as u8,
            error: ErrorUnion {
                incompatible_server: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for IncompatibleServerError {
    fn drop(&mut self) {
        let Self { message } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Invalid response error
#[repr(C)]
pub struct InvalidResponseError {
    pub message: *const c_char,
}

impl InvalidResponseError {
    fn new(message: &str) -> Self {
        Self {
            message: CString::new(message).unwrap().into_raw(),
        }
    }
}

impl From<InvalidResponseError> for Error {
    fn from(error: InvalidResponseError) -> Self {
        Self {
            error_type: ErrorType::InvalidResponse as u8,
            error: ErrorUnion {
                invalid_response: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for InvalidResponseError {
    fn drop(&mut self) {
        let Self { message } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Change stream error
#[repr(C)]
pub struct ChangeStreamError {
    pub message: *const c_char,
    pub resumable: bool,
}

impl ChangeStreamError {
    fn new(message: &str, resumable: bool) -> Self {
        Self {
            message: CString::new(message).unwrap().into_raw(),
            resumable,
        }
    }
}

impl From<ChangeStreamError> for Error {
    fn from(error: ChangeStreamError) -> Self {
        Self {
            error_type: ErrorType::ChangeStream as u8,
            error: ErrorUnion {
                change_stream: Box::into_raw(Box::new(error)),
            },
        }
    }
}

impl Drop for ChangeStreamError {
    fn drop(&mut self) {
        let Self {
            message,
            resumable: _,
        } = self;
        unsafe {
            if !message.is_null() {
                let _ = CString::from_raw(*message as *mut c_char);
            }
        }
    }
}

/// Shutdown error
#[repr(C)]
pub struct ShutdownError {
    // Empty struct - just indicates shutdown
}

impl ShutdownError {
    fn new() -> Self {
        Self {}
    }
}

impl From<ShutdownError> for Error {
    fn from(error: ShutdownError) -> Self {
        Self {
            error_type: ErrorType::Shutdown as u8,
            error: ErrorUnion {
                shutdown: Box::into_raw(Box::new(error)),
            },
        }
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
