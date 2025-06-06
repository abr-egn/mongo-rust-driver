pub mod action;
pub mod auth;
#[cfg(feature = "in-use-encryption")]
pub(crate) mod csfle;
mod executor;
pub mod options;
pub mod session;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex as SyncMutex,
    },
    time::{Duration, Instant},
};

#[cfg(feature = "in-use-encryption")]
pub use self::csfle::client_builder::*;
use derive_where::derive_where;
use futures_core::Future;
use futures_util::FutureExt;

#[cfg(feature = "tracing-unstable")]
use crate::trace::{
    command::CommandTracingEventEmitter,
    server_selection::ServerSelectionTracingEventEmitter,
    trace_or_log_enabled,
    TracingOrLogLevel,
    COMMAND_TRACING_EVENT_TARGET,
};
use crate::{
    bson::doc,
    concern::{ReadConcern, WriteConcern},
    db::Database,
    error::{Error, ErrorKind, Result},
    event::command::CommandEvent,
    id_set::IdSet,
    operation::OverrideCriteriaFn,
    options::{ClientOptions, DatabaseOptions, ReadPreference, SelectionCriteria, ServerAddress},
    sdam::{
        server_selection::{self, attempt_to_select_server},
        SelectedServer,
        Topology,
    },
    tracking_arc::TrackingArc,
    BoxFuture,
    ClientSession,
};

pub(crate) use executor::{HELLO_COMMAND_NAMES, REDACTED_COMMANDS};
pub(crate) use session::{ClusterTime, SESSIONS_UNSUPPORTED_COMMANDS};

use session::{ServerSession, ServerSessionPool};

const DEFAULT_SERVER_SELECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// This is the main entry point for the API. A `Client` is used to connect to a MongoDB cluster.
/// By default, it will monitor the topology of the cluster, keeping track of any changes, such
/// as servers being added or removed.
///
/// `Client` uses [`std::sync::Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html) internally,
/// so it can safely be shared across threads or async tasks. For example:
///
/// ```rust
/// # use mongodb::{bson::Document, Client, error::Result};
/// #
/// # async fn start_workers() -> Result<()> {
/// let client = Client::with_uri_str("mongodb://example.com").await?;
///
/// for i in 0..5 {
///     let client_ref = client.clone();
///
///     tokio::task::spawn(async move {
///         let collection = client_ref.database("items").collection::<Document>(&format!("coll{}", i));
///
///         // Do something with the collection
///     });
/// }
/// #
/// # Ok(())
/// # }
/// ```
/// ## Notes on performance
/// Spawning many asynchronous tasks that use the driver concurrently like this is often the best
/// way to achieve maximum performance, as the driver is designed to work well in such situations.
///
/// Additionally, using a custom Rust type that implements `Serialize` and `Deserialize` as the
/// generic parameter of [`Collection`](../struct.Collection.html) instead of
/// [`Document`](crate::bson::Document) can reduce the amount of time the driver and your
/// application spends serializing and deserializing BSON, which can also lead to increased
/// performance.
///
/// ## TCP Keepalive
/// TCP keepalive is enabled by default with ``tcp_keepalive_time`` set to 120 seconds. The
/// driver does not set ``tcp_keepalive_intvl``. See the
/// [MongoDB Diagnostics FAQ keepalive section](https://www.mongodb.com/docs/manual/faq/diagnostics/#does-tcp-keepalive-time-affect-mongodb-deployments)
/// for instructions on setting these values at the system level.
///
/// ## Clean shutdown
/// Because Rust has no async equivalent of `Drop`, values that require server-side cleanup when
/// dropped spawn a new async task to perform that cleanup.  This can cause two potential issues:
///
/// * Drop tasks pending or in progress when the async runtime shuts down may not complete, causing
///   server-side resources to not be freed.
/// * Drop tasks may run at an arbitrary time even after no `Client` values exist, making it hard to
///   reason about associated resources (e.g. event handlers).
///
/// To address these issues, we highly recommend you use [`Client::shutdown`] in the termination
/// path of your application.  This will ensure that outstanding resources have been cleaned up and
/// terminate internal worker tasks before returning.  Please note that `shutdown` will wait for
/// _all_ outstanding resource handles to be dropped, so they must either have been dropped before
/// calling `shutdown` or in a concurrent task; see the documentation of `shutdown` for more
/// details.
#[derive(Debug, Clone)]
pub struct Client {
    inner: TrackingArc<ClientInner>,
}

#[allow(dead_code, unreachable_code, clippy::diverging_sub_expression)]
const _: fn() = || {
    fn assert_send<T: Send>(_t: T) {}
    fn assert_sync<T: Sync>(_t: T) {}

    let _c: super::Client = todo!();
    assert_send(_c);
    assert_sync(_c);
};

#[derive(Debug)]
struct ClientInner {
    topology: Topology,
    options: ClientOptions,
    session_pool: ServerSessionPool,
    shutdown: Shutdown,
    dropped: AtomicBool,
    end_sessions_token: std::sync::Mutex<AsyncDropToken>,
    #[cfg(feature = "in-use-encryption")]
    csfle: tokio::sync::RwLock<Option<csfle::ClientState>>,
    #[cfg(test)]
    disable_command_events: AtomicBool,
}

#[derive(Debug)]
struct Shutdown {
    pending_drops: SyncMutex<IdSet<crate::runtime::AsyncJoinHandle<()>>>,
    executed: AtomicBool,
}

impl Client {
    /// Creates a new `Client` connected to the cluster specified by `uri`. `uri` must be a valid
    /// MongoDB connection string.
    ///
    /// See the documentation on
    /// [`ClientOptions::parse`](options/struct.ClientOptions.html#method.parse) for more details.
    pub async fn with_uri_str(uri: impl AsRef<str>) -> Result<Self> {
        let options = ClientOptions::parse(uri.as_ref()).await?;

        Client::with_options(options)
    }

    /// Creates a new `Client` connected to the cluster specified by `options`.
    pub fn with_options(options: ClientOptions) -> Result<Self> {
        options.validate()?;

        // Spawn a cleanup task, similar to register_async_drop
        let (cleanup_tx, cleanup_rx) = tokio::sync::oneshot::channel::<BoxFuture<'static, ()>>();
        crate::runtime::spawn(async move {
            // If the cleanup channel is closed, that task was dropped.
            if let Ok(cleanup) = cleanup_rx.await {
                cleanup.await;
            }
        });
        let end_sessions_token = std::sync::Mutex::new(AsyncDropToken {
            tx: Some(cleanup_tx),
        });

        let inner = TrackingArc::new(ClientInner {
            topology: Topology::new(options.clone())?,
            session_pool: ServerSessionPool::new(),
            options,
            shutdown: Shutdown {
                pending_drops: SyncMutex::new(IdSet::new()),
                executed: AtomicBool::new(false),
            },
            dropped: AtomicBool::new(false),
            end_sessions_token,
            #[cfg(feature = "in-use-encryption")]
            csfle: Default::default(),
            #[cfg(test)]
            disable_command_events: AtomicBool::new(false),
        });
        Ok(Self { inner })
    }

    /// Return an `EncryptedClientBuilder` for constructing a `Client` with auto-encryption enabled.
    ///
    /// ```no_run
    /// # use mongocrypt::ctx::KmsProvider;
    /// # use mongodb::{Client, bson::{self, doc}, error::Result};
    /// # async fn func() -> Result<()> {
    /// # let client_options = todo!();
    /// # let key_vault_namespace = todo!();
    /// # let key_vault_client: Client = todo!();
    /// # let local_key: bson::Binary = todo!();
    /// let encrypted_client = Client::encrypted_builder(
    ///     client_options,
    ///     key_vault_namespace,
    ///     [(KmsProvider::local(), doc! { "key": local_key }, None)],
    /// )?
    /// .key_vault_client(key_vault_client)
    /// .build()
    /// .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "in-use-encryption")]
    pub fn encrypted_builder(
        client_options: ClientOptions,
        key_vault_namespace: crate::Namespace,
        kms_providers: impl IntoIterator<
            Item = (
                mongocrypt::ctx::KmsProvider,
                crate::bson::Document,
                Option<options::TlsOptions>,
            ),
        >,
    ) -> Result<EncryptedClientBuilder> {
        Ok(EncryptedClientBuilder::new(
            client_options,
            csfle::options::AutoEncryptionOptions::new(
                key_vault_namespace,
                csfle::options::KmsProviders::new(kms_providers)?,
            ),
        ))
    }

    /// Whether commands sent via this client should be auto-encrypted.
    pub(crate) async fn should_auto_encrypt(&self) -> bool {
        #[cfg(feature = "in-use-encryption")]
        {
            let csfle = self.inner.csfle.read().await;
            match *csfle {
                Some(ref csfle) => csfle
                    .opts()
                    .bypass_auto_encryption
                    .map(|b| !b)
                    .unwrap_or(true),
                None => false,
            }
        }
        #[cfg(not(feature = "in-use-encryption"))]
        {
            false
        }
    }

    #[cfg(all(test, feature = "in-use-encryption"))]
    pub(crate) async fn mongocryptd_spawned(&self) -> bool {
        self.inner
            .csfle
            .read()
            .await
            .as_ref()
            .is_some_and(|cs| cs.exec().mongocryptd_spawned())
    }

    #[cfg(all(test, feature = "in-use-encryption"))]
    pub(crate) async fn has_mongocryptd_client(&self) -> bool {
        self.inner
            .csfle
            .read()
            .await
            .as_ref()
            .is_some_and(|cs| cs.exec().has_mongocryptd_client())
    }

    fn test_command_event_channel(&self) -> Option<&options::TestEventSender> {
        #[cfg(test)]
        {
            self.inner
                .options
                .test_options
                .as_ref()
                .and_then(|t| t.async_event_listener.as_ref())
        }
        #[cfg(not(test))]
        {
            None
        }
    }

    pub(crate) async fn emit_command_event(&self, generate_event: impl FnOnce() -> CommandEvent) {
        #[cfg(test)]
        if self
            .inner
            .disable_command_events
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        #[cfg(feature = "tracing-unstable")]
        let tracing_emitter = if trace_or_log_enabled!(
            target: COMMAND_TRACING_EVENT_TARGET,
            TracingOrLogLevel::Debug
        ) {
            Some(CommandTracingEventEmitter::new(
                self.inner.options.tracing_max_document_length_bytes,
                self.inner.topology.id,
            ))
        } else {
            None
        };
        let test_channel = self.test_command_event_channel();
        let should_send = test_channel.is_some() || self.options().command_event_handler.is_some();
        #[cfg(feature = "tracing-unstable")]
        let should_send = should_send || tracing_emitter.is_some();
        if !should_send {
            return;
        }

        let event = generate_event();
        if let Some(tx) = test_channel {
            let (msg, ack) = crate::runtime::AcknowledgedMessage::package(event.clone());
            let _ = tx.send(msg).await;
            ack.wait_for_acknowledgment().await;
        }
        #[cfg(feature = "tracing-unstable")]
        if let Some(ref tracing_emitter) = tracing_emitter {
            tracing_emitter.handle(event.clone());
        }
        if let Some(handler) = &self.options().command_event_handler {
            handler.handle(event);
        }
    }

    /// Gets the default selection criteria the `Client` uses for operations..
    pub fn selection_criteria(&self) -> Option<&SelectionCriteria> {
        self.inner.options.selection_criteria.as_ref()
    }

    /// Gets the default read concern the `Client` uses for operations.
    pub fn read_concern(&self) -> Option<&ReadConcern> {
        self.inner.options.read_concern.as_ref()
    }

    /// Gets the default write concern the `Client` uses for operations.
    pub fn write_concern(&self) -> Option<&WriteConcern> {
        self.inner.options.write_concern.as_ref()
    }

    /// Gets a handle to a database specified by `name` in the cluster the `Client` is connected to.
    /// The `Database` options (e.g. read preference and write concern) will default to those of the
    /// `Client`.
    ///
    /// This method does not send or receive anything across the wire to the database, so it can be
    /// used repeatedly without incurring any costs from I/O.
    pub fn database(&self, name: &str) -> Database {
        Database::new(self.clone(), name, None)
    }

    /// Gets a handle to a database specified by `name` in the cluster the `Client` is connected to.
    /// Operations done with this `Database` will use the options specified by `options` by default
    /// and will otherwise default to those of the `Client`.
    ///
    /// This method does not send or receive anything across the wire to the database, so it can be
    /// used repeatedly without incurring any costs from I/O.
    pub fn database_with_options(&self, name: &str, options: DatabaseOptions) -> Database {
        Database::new(self.clone(), name, Some(options))
    }

    /// Gets a handle to the default database specified in the `ClientOptions` or MongoDB connection
    /// string used to construct this `Client`.
    ///
    /// If no default database was specified, `None` will be returned.
    pub fn default_database(&self) -> Option<Database> {
        self.inner
            .options
            .default_database
            .as_ref()
            .map(|db_name| self.database(db_name))
    }

    pub(crate) fn register_async_drop(&self) -> AsyncDropToken {
        let (cleanup_tx, cleanup_rx) = tokio::sync::oneshot::channel::<BoxFuture<'static, ()>>();
        let (id_tx, id_rx) = tokio::sync::oneshot::channel::<crate::id_set::Id>();
        let weak = self.weak();
        let handle = crate::runtime::spawn(async move {
            // Unwrap safety: the id is sent immediately after task creation, with no
            // await points in between.
            let id = id_rx.await.unwrap();
            // If the cleanup channel is closed, that task was dropped.
            if let Ok(cleanup) = cleanup_rx.await {
                cleanup.await;
            }
            if let Some(client) = weak.upgrade() {
                client
                    .inner
                    .shutdown
                    .pending_drops
                    .lock()
                    .unwrap()
                    .remove(&id);
            }
        });
        let id = self
            .inner
            .shutdown
            .pending_drops
            .lock()
            .unwrap()
            .insert(handle);
        let _ = id_tx.send(id);
        AsyncDropToken {
            tx: Some(cleanup_tx),
        }
    }

    /// Check in a server session to the server session pool. The session will be discarded if it is
    /// expired or dirty.
    pub(crate) async fn check_in_server_session(&self, session: ServerSession) {
        let timeout = self.inner.topology.logical_session_timeout();
        self.inner.session_pool.check_in(session, timeout).await;
    }

    #[cfg(test)]
    pub(crate) async fn clear_session_pool(&self) {
        self.inner.session_pool.clear().await;
    }

    #[cfg(test)]
    pub(crate) async fn is_session_checked_in(&self, id: &crate::bson::Document) -> bool {
        self.inner.session_pool.contains(id).await
    }

    #[cfg(test)]
    pub(crate) fn disable_command_events(&self, disable: bool) {
        self.inner
            .disable_command_events
            .store(disable, std::sync::atomic::Ordering::SeqCst);
    }

    /// Get the address of the server selected according to the given criteria.
    /// This method is only used in tests.
    #[cfg(test)]
    pub(crate) async fn test_select_server(
        &self,
        criteria: Option<&SelectionCriteria>,
    ) -> Result<ServerAddress> {
        let (server, _) = self
            .select_server(criteria, "Test select server", None, |_, _| None)
            .await?;
        Ok(server.address.clone())
    }

    /// Select a server using the provided criteria. If none is provided, a primary read preference
    /// will be used instead.
    async fn select_server(
        &self,
        criteria: Option<&SelectionCriteria>,
        #[allow(unused_variables)] // we only use the operation_name for tracing.
        operation_name: &str,
        deprioritized: Option<&ServerAddress>,
        override_criteria: OverrideCriteriaFn,
    ) -> Result<(SelectedServer, SelectionCriteria)> {
        let criteria =
            criteria.unwrap_or(&SelectionCriteria::ReadPreference(ReadPreference::Primary));

        let start_time = Instant::now();
        let timeout = self
            .inner
            .options
            .server_selection_timeout
            .unwrap_or(DEFAULT_SERVER_SELECTION_TIMEOUT);

        #[cfg(feature = "tracing-unstable")]
        let event_emitter = ServerSelectionTracingEventEmitter::new(
            self.inner.topology.id,
            criteria,
            operation_name,
            start_time,
            timeout,
        );
        #[cfg(feature = "tracing-unstable")]
        event_emitter.emit_started_event(self.inner.topology.watch().observe_latest().description);
        // We only want to emit this message once per operation at most.
        #[cfg(feature = "tracing-unstable")]
        let mut emitted_waiting_message = false;

        let mut watcher = self.inner.topology.watch();
        loop {
            let state = watcher.observe_latest();
            let override_slot;
            let effective_criteria =
                if let Some(oc) = override_criteria(criteria, &state.description) {
                    override_slot = oc;
                    &override_slot
                } else {
                    criteria
                };
            let result = server_selection::attempt_to_select_server(
                effective_criteria,
                &state.description,
                &state.servers(),
                deprioritized,
            );
            match result {
                Err(error) => {
                    #[cfg(feature = "tracing-unstable")]
                    event_emitter.emit_failed_event(&state.description, &error);

                    return Err(error);
                }
                Ok(result) => {
                    if let Some(server) = result {
                        #[cfg(feature = "tracing-unstable")]
                        event_emitter.emit_succeeded_event(&state.description, &server);

                        return Ok((server, effective_criteria.clone()));
                    } else {
                        #[cfg(feature = "tracing-unstable")]
                        if !emitted_waiting_message {
                            event_emitter.emit_waiting_event(&state.description);
                            emitted_waiting_message = true;
                        }

                        watcher.request_immediate_check();

                        let change_occurred = start_time.elapsed() < timeout
                            && watcher
                                .wait_for_update(timeout - start_time.elapsed())
                                .await;
                        if !change_occurred {
                            let error: Error = ErrorKind::ServerSelection {
                                message: state
                                    .description
                                    .server_selection_timeout_error_message(criteria),
                            }
                            .into();

                            #[cfg(feature = "tracing-unstable")]
                            event_emitter.emit_failed_event(&state.description, &error);

                            return Err(error);
                        }
                    }
                }
            }
        }
    }

    #[cfg(all(test, feature = "dns-resolver"))]
    pub(crate) fn get_hosts(&self) -> Vec<String> {
        let watcher = self.inner.topology.watch();
        let state = watcher.peek_latest();

        state
            .servers()
            .keys()
            .map(|stream_address| format!("{}", stream_address))
            .collect()
    }

    #[cfg(test)]
    pub(crate) async fn sync_workers(&self) {
        self.inner.topology.sync_workers().await;
    }

    #[cfg(test)]
    pub(crate) fn topology_description(&self) -> crate::sdam::TopologyDescription {
        self.inner
            .topology
            .watch()
            .peek_latest()
            .description
            .clone()
    }

    #[cfg(test)]
    pub(crate) fn topology(&self) -> &Topology {
        &self.inner.topology
    }

    #[cfg(feature = "in-use-encryption")]
    pub(crate) async fn primary_description(&self) -> Option<crate::sdam::ServerDescription> {
        let start_time = Instant::now();
        let timeout = self
            .inner
            .options
            .server_selection_timeout
            .unwrap_or(DEFAULT_SERVER_SELECTION_TIMEOUT);
        let mut watcher = self.inner.topology.watch();
        loop {
            let topology = watcher.observe_latest();
            if let Some(desc) = topology.description.primary() {
                return Some(desc.clone());
            }
            if !watcher
                .wait_for_update(timeout - start_time.elapsed())
                .await
            {
                return None;
            }
        }
    }

    pub(crate) fn weak(&self) -> WeakClient {
        WeakClient {
            inner: TrackingArc::downgrade(&self.inner),
        }
    }

    #[cfg(feature = "in-use-encryption")]
    pub(crate) async fn auto_encryption_opts(
        &self,
    ) -> Option<tokio::sync::RwLockReadGuard<'_, csfle::options::AutoEncryptionOptions>> {
        tokio::sync::RwLockReadGuard::try_map(self.inner.csfle.read().await, |csfle| {
            csfle.as_ref().map(|cs| cs.opts())
        })
        .ok()
    }

    pub(crate) fn options(&self) -> &ClientOptions {
        &self.inner.options
    }

    /// Ends all sessions contained in this client's session pool on the server.
    pub(crate) async fn end_all_sessions(&self) {
        // The maximum number of session IDs that should be sent in a single endSessions command.
        const MAX_END_SESSIONS_BATCH_SIZE: usize = 10_000;

        let mut watcher = self.inner.topology.watch();
        let selection_criteria =
            SelectionCriteria::from(ReadPreference::PrimaryPreferred { options: None });

        let session_ids = self.inner.session_pool.get_session_ids().await;
        for chunk in session_ids.chunks(MAX_END_SESSIONS_BATCH_SIZE) {
            let state = watcher.observe_latest();
            let Ok(Some(_)) = attempt_to_select_server(
                &selection_criteria,
                &state.description,
                &state.servers(),
                None,
            ) else {
                // If a suitable server is not available, do not proceed with the operation to avoid
                // spinning for server_selection_timeout.
                return;
            };

            let end_sessions = doc! {
                "endSessions": chunk,
            };
            let _ = self
                .database("admin")
                .run_command(end_sessions)
                .selection_criteria(selection_criteria.clone())
                .await;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WeakClient {
    inner: crate::tracking_arc::Weak<ClientInner>,
}

impl WeakClient {
    pub(crate) fn upgrade(&self) -> Option<Client> {
        self.inner.upgrade().map(|inner| Client { inner })
    }
}

#[derive_where(Debug)]
pub(crate) struct AsyncDropToken {
    #[derive_where(skip)]
    tx: Option<tokio::sync::oneshot::Sender<BoxFuture<'static, ()>>>,
}

impl AsyncDropToken {
    pub(crate) fn spawn(&mut self, fut: impl Future<Output = ()> + Send + 'static) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(fut.boxed());
        } else {
            #[cfg(debug_assertions)]
            panic!("exhausted AsyncDropToken");
        }
    }

    pub(crate) fn take(&mut self) -> Self {
        Self { tx: self.tx.take() }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if !self.inner.shutdown.executed.load(Ordering::SeqCst)
            && !self.inner.dropped.load(Ordering::SeqCst)
            && TrackingArc::strong_count(&self.inner) == 1
        {
            // We need an owned copy of the client to move into the spawned future. However, if this
            // call to drop completes before the spawned future completes, the number of strong
            // references to the inner client will again be 1 when the cloned client drops, and thus
            // end_all_sessions will be called continuously until the runtime shuts down. Storing a
            // flag indicating whether end_all_sessions has already been called breaks
            // this cycle.
            self.inner.dropped.store(true, Ordering::SeqCst);
            let client = self.clone();
            self.inner
                .end_sessions_token
                .lock()
                .unwrap()
                .spawn(async move {
                    client.end_all_sessions().await;
                });
        }
    }
}
