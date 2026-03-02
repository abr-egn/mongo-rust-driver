# FFI Operations Layer Design

## Design Principles

1. **Options as FFI structs, not BSON** - Type-safe contracts, no serialization overhead
2. **Documents/filters/pipelines as BSON** - User data stays as raw BSON
3. **Results as FFI structs** - Except embedded documents which stay as BSON
4. **Async with callbacks** - Operations spawn onto Tokio, invoke callback on completion
5. **Opaque session handles** - Sessions are fully managed in Rust, including transaction state

## Benefits

1. **Spec logic in Rust only** - Retry, batching, version checks handled internally
2. **Type-safe contracts** - FFI structs make the API explicit
3. **No BSON overhead for options** - Direct struct passing
4. **Language drivers become thin wrappers** - Map API to FFI, parse results
5. **No driver changes required** - FFI uses existing `execute_operation` with real `ClientSession`

## MongoClient

The `MongoClient` is an opaque pointer to the Rust client with its Tokio runtime and handle pools.

### Client Options

```rust
#[repr(C)]
pub struct ConnectionSettingsFFI {
    /// Hosts as comma-separated "host:port" pairs (null-terminated)
    pub hosts: *const c_char,
    /// Application name (null-terminated, nullable)
    pub app_name: *const c_char,
    /// Compressors as comma-separated list (null-terminated, nullable)
    pub compressors: *const c_char,
    /// Direct connection flag
    pub direct_connection: bool,
    /// Load balanced flag
    pub load_balanced: bool,
    /// Max pool size (-1 = not set)
    pub max_pool_size: i32,
    /// Min pool size (-1 = not set)
    pub min_pool_size: i32,
    /// Max idle time in milliseconds (-1 = not set)
    pub max_idle_time_ms: i64,
    /// Connect timeout in milliseconds (-1 = not set)
    pub connect_timeout_ms: i64,
    /// Socket timeout in milliseconds (-1 = not set)
    pub socket_timeout_ms: i64,
    /// Server selection timeout in milliseconds (-1 = not set)
    pub server_selection_timeout_ms: i64,
    /// Local threshold in milliseconds (-1 = not set)
    pub local_threshold_ms: i64,
    /// Heartbeat frequency in milliseconds (-1 = not set)
    pub heartbeat_frequency_ms: i64,
    /// Replica set name (null-terminated, nullable)
    pub replica_set: *const c_char,
    /// Read preference mode (0=primary, 1=primaryPreferred, etc.)
    pub read_preference_mode: u8,
    /// SRV service name (null-terminated, nullable)
    pub srv_service_name: *const c_char,
    /// SRV max hosts (-1 = not set)
    pub srv_max_hosts: i32,
}

#[repr(C)]
pub struct AuthSettingsFFI {
    /// Auth mechanism (null-terminated, nullable: "SCRAM-SHA-1", "SCRAM-SHA-256", etc.)
    pub mechanism: *const c_char,
    /// Username (null-terminated, nullable)
    pub username: *const c_char,
    /// Password (null-terminated, nullable)
    pub password: *const c_char,
    /// Auth source database (null-terminated, nullable)
    pub source: *const c_char,
}

#[repr(C)]
pub struct TlsSettingsFFI {
    /// Enable TLS
    pub enabled: bool,
    /// Allow invalid certificates
    pub allow_invalid_certificates: bool,
    /// Allow invalid hostnames
    pub allow_invalid_hostnames: bool,
    /// CA file path (null-terminated, nullable)
    pub ca_file: *const c_char,
    /// Certificate file path (null-terminated, nullable)
    pub cert_file: *const c_char,
    /// Certificate key file path (null-terminated, nullable)
    pub cert_key_file: *const c_char,
}
```

### Client Lifecycle

```rust
/// Create a new MongoClient. Returns pointer on success, null on error.
/// The client owns a Tokio runtime and all handle pools.
pub extern "C" fn mongo_client_new(
    connection_settings: *const ConnectionSettingsFFI,
    auth_settings: *const AuthSettingsFFI,       // nullable
    tls_settings: *const TlsSettingsFFI,         // nullable
) -> *mut MongoClient

/// Destroy a MongoClient. All sessions, cursors, and handles become invalid.
/// Waits for in-flight operations to complete (up to timeout).
pub extern "C" fn mongo_client_destroy(
    client: *mut MongoClient,
)
```

### Memory Model

#### Input Data (caller → Rust)

**Input pointers must remain valid until the callback completes.**

- Options structs, BSON bytes, strings passed to FFI functions are **borrowed** by Rust
- Rust does **not** copy input data - it borrows directly for the lifetime of the operation
- Caller must keep input data alive until the callback is invoked
- Caller can free input data after callback returns

Example:
```java
// Java allocates filter bytes - must keep alive until callback
byte[] filter = buildFilter();
Pointer filterPtr = copyToNative(filter);

// Call FFI - Rust borrows filterPtr (no copy)
mongo_find(client, db, coll, filterPtr, ..., (result, error) -> {
    // Process result...

    // Now safe to free input data
    freeNative(filterPtr);
});
```

This avoids copying potentially large documents (filters, pipelines, update docs) while
maintaining a simple ownership model.

#### Output Data (Rust → caller via callback)

**Callback data is valid only during the callback invocation.**

Result structs, error structs, and their nested pointers (strings, BSON bytes, arrays) are owned by Rust
and freed immediately after the callback returns. Language drivers must extract/copy any data they need
to retain before returning from the callback.

This means:
- **No explicit free functions needed** - Rust manages all memory
- **Callbacks must not store raw pointers** - Copy data into language-native types
- **Thread safety** - Callbacks may be invoked from any Tokio worker thread

Example:
```java
// In callback - extract data before returning
void onInsertResult(Pointer resultPtr) {
    InsertOneResult result = new InsertOneResult(resultPtr);
    // Copy the inserted_id bytes into a Java BsonValue
    this.insertedId = parseBson(result.inserted_id);
    // After this method returns, resultPtr is invalid
}
```

## Core FFI Types

### BSON Types

Two types for passing BSON across FFI:

```rust
/// Raw BSON document - the common case (filters, updates, results, batches)
#[repr(C)]
pub struct Bson {
    pub data: *const u8,
    pub len: usize,
}

/// Any BSON value with explicit type byte (for inserted_id, comment, hint, etc.)
#[repr(C)]
pub struct BsonValue {
    pub data: *const u8,
    pub len: usize,
    pub bson_type: u8,  // BSON type: 0x01=double, 0x02=string, 0x07=objectid, etc.
}
```

**Why two types?**

BSON only supports documents at the top level - there's no concept of a standalone typed value.
The spec defines elements as `type + key + value`, so the type byte is bound to a key name.
This means a standalone `ObjectId` or `String` has no self-describing representation.

Options considered:
1. **Single type, wrap everything in documents** (`{ "": value }`) - works but feels hacky
2. **Single type with type byte always** - wasteful for the 90%+ document case
3. **Two types** - `Bson` for documents (common), `BsonValue` for arbitrary values (rare)

We chose option 3. Most FFI calls pass documents (filters, updates, results), so `Bson` keeps
those simple. The few fields that can be any type (`inserted_id`, `comment`, `hint`) use
`BsonValue` with an explicit type byte so the receiver knows how to parse.

**When to use which:**
- `Bson` - documents, filters, updates, pipelines, batches (arrays wrapped as `{ "": [...] }`)
- `BsonValue` - `inserted_id`, `upserted_id`, `comment`, `hint`, or any field that can be an arbitrary BSON type

For input: caller owns the data, must keep alive until callback completes.
For output: Rust owns the data, valid only during callback invocation.

### Opaque Pointer Types

All managed objects use opaque pointer types (not `u64`) for type safety and clarity:

```rust
/// Opaque session - points to ClientSession in Rust
#[repr(C)]
pub struct Session { _private: [u8; 0] }

/// Opaque read preference
#[repr(C)]
pub struct ReadPreference { _private: [u8; 0] }

/// Opaque write concern
#[repr(C)]
pub struct WriteConcern { _private: [u8; 0] }

/// Opaque read concern
#[repr(C)]
pub struct ReadConcern { _private: [u8; 0] }

/// Opaque cursor
#[repr(C)]
pub struct Cursor { _private: [u8; 0] }

/// Opaque change stream cursor
#[repr(C)]
pub struct ChangeStream { _private: [u8; 0] }
```

Null pointers indicate "not set" (use default). Pointers are obtained via FFI functions
and must be destroyed when no longer needed.

## Sessions

Sessions are **opaque pointers** (`*mut Session`) backed by real `ClientSession` objects stored in Rust.
Transaction state is managed entirely within the session - language drivers don't track it.

### Session Options

```rust
#[repr(C)]
pub struct SessionOptionsFFI {
    /// Causal consistency. -1 = not set (use default), 0 = false, 1 = true
    pub causal_consistency: i8,

    /// Snapshot reads. -1 = not set, 0 = false, 1 = true
    pub snapshot: i8,

    /// Default transaction options (applied when starting transactions)
    pub default_transaction_options: *const TransactionOptionsFFI,  // nullable
}

#[repr(C)]
pub struct TransactionOptionsFFI {
    /// Read concern level (null-terminated string, nullable)
    pub read_concern_level: *const c_char,

    /// Write concern w value. -1 = not set, 0 = unacknowledged, 1+ = w value
    pub write_concern_w: i32,
    /// Write concern w tag (for w:"majority", null-terminated, nullable)
    pub write_concern_w_tag: *const c_char,
    /// Write concern journal. -1 = not set, 0 = false, 1 = true
    pub write_concern_j: i8,
    /// Write concern wtimeout in milliseconds. -1 = not set
    pub write_concern_w_timeout_ms: i64,

    /// Read preference mode. 0=primary, 1=primaryPreferred, 2=secondary, etc.
    pub read_preference_mode: u8,

    /// Max commit time in milliseconds. -1 = not set
    pub max_commit_time_ms: i64,
}
```

### Session Lifecycle

```rust
/// Start a new session. Returns session handle (non-zero), or 0 on error.
pub extern "C" fn mongo_session_start(
    client: *mut MongoClient,
    options: *const SessionOptionsFFI,  // nullable
) -> *mut Session

/// End a session. The session handle becomes invalid after this call.
pub extern "C" fn mongo_session_end(
    client: *mut MongoClient,
    session_handle: *mut Session,
)
```

### Transaction Control

Transaction methods are on the session, not standalone operations. This keeps transaction state opaque.

```rust
pub type TransactionCallback = extern "C" fn(
    userdata: *mut c_void,
    error: *const ErrorFFI,  // null on success
);

/// Start a transaction on the session.
pub extern "C" fn mongo_session_start_transaction(
    client: *mut MongoClient,
    session_handle: *mut Session,
    options: *const TransactionOptionsFFI,  // nullable
    callback: TransactionCallback,
    userdata: *mut c_void,
)

/// Commit the current transaction.
pub extern "C" fn mongo_session_commit_transaction(
    client: *mut MongoClient,
    session_handle: *mut Session,
    callback: TransactionCallback,
    userdata: *mut c_void,
)

/// Abort the current transaction.
pub extern "C" fn mongo_session_abort_transaction(
    client: *mut MongoClient,
    session_handle: *mut Session,
    callback: TransactionCallback,
    userdata: *mut c_void,
)
```

### Why Opaque Sessions?

| Aspect | External State (Old) | Opaque Sessions (New) |
|--------|---------------------|----------------------|
| **Transaction state** | Language driver tracks | Rust `ClientSession` manages |
| **State sync** | Must pass state every operation | Not needed |
| **Commit retry** | Driver must handle | `commit_transaction()` handles internally |
| **Error labels** | Driver interprets | Rust applies automatically |
| **Driver changes** | New executor method needed | **None** - uses existing APIs |

## Read Preference

Read preferences are **opaque pointers** (`*const ReadPreference`). Create once, reuse across operations.

```rust
#[repr(C)]
pub struct ReadPreferenceOptionsFFI {
    /// Mode: 0=primary, 1=primaryPreferred, 2=secondary, 3=secondaryPreferred, 4=nearest
    pub mode: u8,

    /// Tag sets as BSON array wrapped in doc, nullable. Example: [{"dc": "east"}, {"dc": "west"}]
    pub tags: *const Bson,

    /// Max staleness in seconds. -1 = not set
    pub max_staleness_seconds: i64,

    /// Hedge options as BSON document, nullable. Example: {"enabled": true}
    pub hedge: *const Bson,
}

/// Create a read preference. Returns handle (non-zero), or 0 on error.
pub extern "C" fn mongo_read_preference_create(
    client: *mut MongoClient,
    options: *const ReadPreferenceOptionsFFI,
) -> *mut ReadPreference

/// Destroy a read preference handle.
pub extern "C" fn mongo_read_preference_destroy(
    client: *mut MongoClient,
    handle: *mut ReadPreference,
)
```

## Write Concern

Write concerns are **opaque pointers** (`*const WriteConcern`). Create once, reuse across operations.

```rust
#[repr(C)]
pub struct WriteConcernOptionsFFI {
    /// W value. -1 = not set, 0 = unacknowledged, 1+ = w value
    /// Use w_tag for string values like "majority"
    pub w: i32,

    /// W tag (for w:"majority" etc), null-terminated, nullable
    /// If set, w field is ignored
    pub w_tag: *const c_char,

    /// Journal. -1 = not set, 0 = false, 1 = true
    pub journal: i8,

    /// Write timeout in milliseconds. -1 = not set
    pub w_timeout_ms: i64,
}

/// Create a write concern. Returns handle (non-zero), or 0 on error.
pub extern "C" fn mongo_write_concern_create(
    client: *mut MongoClient,
    options: *const WriteConcernOptionsFFI,
) -> *mut WriteConcern

/// Destroy a write concern handle.
pub extern "C" fn mongo_write_concern_destroy(
    client: *mut MongoClient,
    handle: *mut WriteConcern,
)
```

## Read Concern

Read concerns are **opaque pointers** (`*const ReadConcern`). Create once, reuse across operations.

```rust
#[repr(C)]
pub struct ReadConcernOptionsFFI {
    /// Level: null-terminated string (e.g., "local", "majority", "snapshot", "linearizable")
    pub level: *const c_char,
}

/// Create a read concern. Returns handle (non-zero), or 0 on error.
pub extern "C" fn mongo_read_concern_create(
    client: *mut MongoClient,
    options: *const ReadConcernOptionsFFI,
) -> *mut ReadConcern

/// Destroy a read concern handle.
pub extern "C" fn mongo_read_concern_destroy(
    client: *mut MongoClient,
    handle: *mut ReadConcern,
)
```

## Operation Context

Passed to every operation. Contains handles for session, read preference, write concern, and read concern.

```rust
#[repr(C)]
pub struct OperationContext {
    /// Session handle (null = no session)
    pub session_handle: *const Session,

    /// Read preference handle (null = use default/inherit from session)
    pub read_preference_handle: *const ReadPreference,

    /// Write concern handle (null = use default/inherit from session)
    pub write_concern_handle: *const WriteConcern,

    /// Read concern handle (null = use default/inherit from session)
    pub read_concern_handle: *const ReadConcern,

    /// Timeout in milliseconds (CSOT). -1 = not set (use client default)
    pub timeout_ms: i64,
}
```

## Errors

Errors use a tagged union. Language drivers switch on `error_type` to produce idiomatic exceptions.

The error types are designed to map directly from Rust's `Error` and `ErrorKind` types, providing
all information the server returns without loss.

### Server Errors

```rust
#[repr(C)]
pub struct ServerError {
    pub code: i32,
    pub code_name: *const c_char,       // null-terminated
    pub message: *const c_char,         // null-terminated
    pub labels: *const *const c_char,   // error labels (RetryableWriteError, etc.)
    pub labels_len: usize,
    pub server_response: Bson,     // full raw server response for debugging/future fields
}

#[repr(C)]
pub struct WriteError {
    pub index: u32,
    pub code: i32,
    pub code_name: *const c_char,       // nullable - server doesn't always return
    pub message: *const c_char,
    pub details: Bson,             // raw BSON errorInfo, empty if none
}

#[repr(C)]
pub struct WriteConcernError {
    pub code: i32,
    pub code_name: *const c_char,       // null-terminated
    pub message: *const c_char,
    pub details: Bson,             // raw BSON errInfo, empty if none
    pub labels: *const *const c_char,
    pub labels_len: usize,
}

/// Error from insert_many with partial success info
#[repr(C)]
pub struct InsertManyError {
    pub write_errors: *const WriteError,
    pub write_errors_len: usize,
    pub write_concern_error: *const WriteConcernError,  // nullable
    pub inserted_ids: Bson,        // BSON document: { "0": ObjectId, "1": ObjectId, ... }
}

/// Error from client.bulk_write or collection bulk operations
#[repr(C)]
pub struct BulkWriteError {
    pub write_errors: *const WriteError,
    pub write_errors_len: usize,
    pub write_concern_error: *const WriteConcernError,  // nullable
    pub partial_result: *const BulkWriteResult,         // nullable
}
```

### Client Errors

```rust
#[repr(C)]
pub struct IoError {
    pub message: *const c_char,
}

#[repr(C)]
pub struct ServerSelectionError {
    pub message: *const c_char,
    pub timeout_ms: i64,
}

#[repr(C)]
pub struct TimeoutError {
    pub message: *const c_char,
    pub timeout_ms: i64,
}

#[repr(C)]
pub struct AuthError {
    pub message: *const c_char,
}

#[repr(C)]
pub struct InvalidArgumentError {
    pub message: *const c_char,
}

#[repr(C)]
pub struct TransactionError {
    pub message: *const c_char,
    pub labels: *const *const c_char,   // TransientTransactionError, UnknownTransactionCommitResult
    pub labels_len: usize,
}

#[repr(C)]
pub struct IncompatibleServerError {
    pub message: *const c_char,
}

#[repr(C)]
pub struct InvalidResponseError {
    pub message: *const c_char,
}

#[repr(C)]
pub struct ChangeStreamError {
    pub message: *const c_char,
    pub resumable: bool,                // whether the change stream can be resumed
}

#[repr(C)]
pub struct ShutdownError {
    // Client has been shut down
}
```

### ErrorFFI (Tagged Union)

```rust
/// Error type discriminator
#[repr(u8)]
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

#[repr(C)]
pub struct ErrorFFI {
    pub error_type: u8,
    pub error: ErrorUnion,
}

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
```

### Mapping from Rust ErrorKind

| Rust ErrorKind | FFI ErrorType | Notes |
|----------------|---------------|-------|
| `Command(CommandError)` | `Server` | Server command errors |
| `Write(WriteFailure)` | `Server` | Single write errors |
| `InsertMany(InsertManyError)` | `InsertMany` | Partial insert success |
| `BulkWrite(BulkWriteError)` | `BulkWrite` | Client bulk write errors |
| `Io(_)` | `Io` | Network/IO errors |
| `ServerSelection { .. }` | `ServerSelection` | No suitable server |
| `Authentication { .. }` | `Auth` | Auth failures |
| `InvalidArgument { .. }` | `InvalidArgument` | Bad input |
| `Transaction { .. }` | `Transaction` | Transaction errors |
| `IncompatibleServer { .. }` | `IncompatibleServer` | Server version mismatch |
| `InvalidResponse { .. }` | `InvalidResponse` | Malformed server reply |
| `MissingResumeToken` | `ChangeStream` | Change stream errors |
| `Shutdown` | `Shutdown` | Client shut down |
| `ConnectionPoolCleared { .. }` | `Io` | Map to IO error |
| `SessionsNotSupported` | `IncompatibleServer` | Map to incompatible |
| `BsonSerialization/_` | `InvalidArgument` | Map to invalid arg |

## Results

### Write Results

```rust
#[repr(C)]
pub struct InsertOneResult {
    pub inserted_id: BsonValue,       // Any BSON type (ObjectId, String, Int, etc.)
}

#[repr(C)]
pub struct UpdateResult {
    pub matched_count: u64,
    pub modified_count: u64,
    pub upserted_id: *const BsonValue, // nullable (null if no upsert)
}

#[repr(C)]
pub struct DeleteResult {
    pub deleted_count: u64,
}

#[repr(C)]
pub struct BulkWriteResult {
    pub inserted_count: u64,
    pub matched_count: u64,
    pub modified_count: u64,
    pub deleted_count: u64,
    pub upserted_count: u64,
    pub inserted_ids: Bson,           // BSON document: { "0": id, "1": id, ... }
    pub upserted_ids: Bson,           // BSON document: { "0": id, "1": id, ... }
}
```

### Cursor Results

All cursor-returning operations (find, aggregate, listCollections, listDatabases, listIndexes, etc.)
use a common `CursorResult`. The initial response includes the first batch of documents.

```rust
/// Common result for all cursor-returning operations
#[repr(C)]
pub struct CursorResult {
    pub cursor: *mut Cursor,          // null if exhausted with single batch
    pub exhausted: bool,              // true if no more batches (cursor already closed)
    pub first_batch: Bson,       // raw BSON array of documents from initial response
    pub server_address: *const c_char,
    pub server_port: u16,
}

/// Change streams have additional resume token
#[repr(C)]
pub struct ChangeStreamCursorResult {
    pub cursor: *mut ChangeStream,
    pub first_batch: Bson,       // raw BSON array of change events (usually empty initially)
    pub resume_token: Bson,      // initial resume token
    pub server_address: *const c_char,
    pub server_port: u16,
}
```

## Callbacks

```rust
pub type InsertOneCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const InsertOneResult,  // null on error
    error: *const ErrorFFI,          // null on success
);

pub type UpdateCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const UpdateResult,
    error: *const ErrorFFI,
);

pub type FindCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const FindResult,
    error: *const ErrorFFI,
);

pub type BulkWriteCallback = extern "C" fn(
    userdata: *mut c_void,
    result: *const BulkWriteResult,
    error: *const ErrorFFI,
);
```

## Cursors

Cursors are opaque pointers. Operations that return cursors (find, aggregate, watch) include
the cursor pointer and first batch in their result struct.

```rust
/// Get more results from a cursor (async).
pub extern "C" fn mongo_cursor_get_more(
    client: *mut MongoClient,
    cursor: *mut Cursor,
    userdata: *mut c_void,
    callback: GetMoreResultCallback,
)

/// Close a cursor (async). Also called automatically when cursor is exhausted.
pub extern "C" fn mongo_cursor_close(
    client: *mut MongoClient,
    cursor: *mut Cursor,
    userdata: *mut c_void,
    callback: SingleResultCallback,
)

pub type GetMoreResultCallback = extern "C" fn(
    userdata: *mut c_void,
    success: bool,
    exhausted: bool,      // true if no more batches
    data: *const Bson,
);

/// Change stream operations - exposes resume token
pub extern "C" fn mongo_change_stream_get_more(
    client: *mut MongoClient,
    change_stream: *mut ChangeStream,
    userdata: *mut c_void,
    callback: ChangeStreamGetMoreCallback,
)

pub extern "C" fn mongo_change_stream_close(
    client: *mut MongoClient,
    change_stream: *mut ChangeStream,
    userdata: *mut c_void,
    callback: SingleResultCallback,
)

pub type ChangeStreamGetMoreCallback = extern "C" fn(
    userdata: *mut c_void,
    success: bool,
    exhausted: bool,
    data: *const Bson,         // batch of change events
    resume_token: *const Bson, // current resume token (for resuming after disconnect)
);
```

## Logging

Logging bridges Rust's `tracing` to language callbacks (e.g., Java SLF4J).
Initialize once per process with per-component log levels.

```rust
/// Log level constants
pub const LOG_LEVEL_DEBUG: i32 = 0;
pub const LOG_LEVEL_INFO: i32 = 1;
pub const LOG_LEVEL_WARN: i32 = 2;

/// A single log field (name/value pair)
#[repr(C)]
pub struct LogField {
    pub name: *const c_char,
    pub value: *const c_char,
}

/// Log event structure passed to callback
#[repr(C)]
pub struct LogEvent {
    /// Log level: 0=DEBUG, 1=INFO
    pub level: i32,
    /// Tracing target: "mongodb::command", "mongodb::connection", etc.
    pub target: *const c_char,
    /// Message identifier: "Command started", "Connection pool created", etc.
    pub message: *const c_char,
    /// Number of fields
    pub field_count: i32,
    /// Array of fields (name/value pairs)
    pub fields: *const LogField,
}

/// Callback type for log events
pub type LogCallback = extern "C" fn(userdata: *mut c_void, event: *const LogEvent);

/// Initialize logging globally with per-component log levels.
/// Call once at first client creation. Levels can be updated at runtime.
///
/// Log levels: 0 = DEBUG, 1 = INFO, 2 = WARN (effectively disabled)
pub extern "C" fn mongo_init_logging(
    command_level: i32,
    connection_level: i32,
    server_selection_level: i32,
    topology_level: i32,
    userdata: *mut c_void,
    callback: LogCallback,
)

/// Update log levels at runtime (e.g., when SLF4J config changes)
pub extern "C" fn mongo_update_log_levels(
    command_level: i32,
    connection_level: i32,
    server_selection_level: i32,
    topology_level: i32,
)
```

## Tokio Runtime

A single Tokio runtime is shared across all `MongoClient` instances in a process.
The runtime is created automatically on first client creation and destroyed when
the last client is destroyed (reference-counted internally).

This avoids:
- The overhead of creating hundreds of runtimes when applications create many clients
- Explicit runtime lifecycle management by host languages

```rust
// No explicit FFI functions needed - runtime is managed internally:
// - First mongo_client_new() → creates runtime
// - Last mongo_client_destroy() → shuts down runtime (waits for in-flight ops)
```

## Operations

All operations take a `MongoClient*` as the first parameter. This provides access to:
- The Tokio runtime for async execution
- The session pool for resolving session handles
- The cursor manager for cursor handles

### Collection Namespace

Operations use database name and collection name strings rather than handles, for simplicity:

```rust
/// Insert a single document (async)
pub extern "C" fn ffi_insert_one(
    client: *const MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,           // null-terminated
    coll_name: *const c_char,         // null-terminated
    document: *const Bson,
    // Operation-specific options
    bypass_document_validation: i8,   // -1 = None, 0 = false, 1 = true
    comment: *const BsonValue,        // nullable, any BSON type
    // Async callback
    callback: InsertOneCallback,
    userdata: *mut c_void,
)

/// Update documents matching a filter (async)
pub extern "C" fn ffi_update_one(
    client: *const MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,
    update: *const Bson,
    // Options
    upsert: i8,
    array_filters: *const Bson,       // nullable, BSON array wrapped as doc
    hint: *const BsonValue,           // nullable, string or document
    collation: *const Bson,           // nullable
    // Async callback
    callback: UpdateCallback,
    userdata: *mut c_void,
)

/// Find documents (async) - returns cursor with first batch
pub extern "C" fn ffi_find(
    client: *const MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    filter: *const Bson,              // nullable
    // Options
    projection: *const Bson,          // nullable
    sort: *const Bson,                // nullable
    limit: i64,                       // -1 = not set
    skip: i64,                        // -1 = not set
    batch_size: i32,                  // -1 = not set
    // Async callback
    callback: FindCallback,
    userdata: *mut c_void,
)

/// Watch for changes (async) - returns change stream cursor
pub extern "C" fn ffi_watch(
    client: *const MongoClient,
    ctx: *const OperationContext,
    target_type: u8,                  // 0=client, 1=database, 2=collection
    db_name: *const c_char,           // nullable for client-level watch
    coll_name: *const c_char,         // nullable for database/client-level watch
    pipeline: *const Bson,            // BSON array wrapped as doc
    // Options
    full_document: u8,                // 0=default, 1=updateLookup, etc.
    start_after: *const Bson,         // nullable, resume token
    // Async callback
    callback: ChangeStreamCallback,
    userdata: *mut c_void,
)

/// Bulk write (async)
pub extern "C" fn ffi_bulk_write(
    client: *const MongoClient,
    ctx: *const OperationContext,
    db_name: *const c_char,
    coll_name: *const c_char,
    models: *const WriteModel,
    models_len: usize,
    // Options
    ordered: i8,                      // -1=not set, 0=false, 1=true
    bypass_document_validation: i8,
    // Async callback
    callback: BulkWriteCallback,
    userdata: *mut c_void,
)
```

### Bulk Write Models

```rust
#[repr(C)]
pub struct InsertOneModel {
    pub document: *const Bson,
}

#[repr(C)]
pub struct UpdateOneModel {
    pub filter: *const Bson,
    pub update: *const Bson,
    pub upsert: i8,                   // -1=not set, 0=false, 1=true
    pub array_filters: *const Bson,   // nullable
    pub collation: *const Bson,       // nullable
    pub hint: *const BsonValue,       // nullable, string or document
}

#[repr(C)]
pub struct WriteModel {
    pub model_type: u8,               // 0=insertOne, 1=updateOne, 2=updateMany, etc.
    pub model: WriteModelUnion,       // tagged union
}
```

### Operations to Implement

| Category | Operations |
|----------|------------|
| **Session** | `mongo_session_start`, `mongo_session_end`, `mongo_session_start_transaction`, `mongo_session_commit_transaction`, `mongo_session_abort_transaction` |
| **Read Preference** | `mongo_read_preference_create`, `mongo_read_preference_destroy` |
| **Write Concern** | `mongo_write_concern_create`, `mongo_write_concern_destroy` |
| **Read Concern** | `mongo_read_concern_create`, `mongo_read_concern_destroy` |
| **CRUD** | insert_one, insert_many, update_one, update_many, replace_one, delete_one, delete_many |
| **Find** | find, find_one, find_one_and_update, find_one_and_replace, find_one_and_delete |
| **Aggregate** | aggregate, count_documents, estimated_document_count, distinct |
| **Bulk** | bulk_write (collection), client_bulk_write |
| **Index** | create_indexes, drop_index, list_indexes |
| **Collection** | create_collection, drop_collection, rename_collection, list_collections |
| **Database** | drop_database, list_databases, run_command |
| **Change Stream** | watch (collection, database, client) |
| **Cursor** | cursor_next, cursor_close |

**Notes:**
- Transaction operations (`start_transaction`, `commit_transaction`, `abort_transaction`) are **session methods**, not standalone operations.
- Read preference, write concern, and read concern handles can be created once and reused across multiple operations.