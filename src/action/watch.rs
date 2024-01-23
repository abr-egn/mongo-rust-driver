use std::time::Duration;

use bson::{Bson, Document, Timestamp};

use super::{action_impl, option_setters};
use crate::{
    change_stream::{
        event::{ChangeStreamEvent, ResumeToken},
        options::{ChangeStreamOptions, FullDocumentBeforeChangeType, FullDocumentType},
        session::SessionChangeStream,
        ChangeStream,
    },
    collation::Collation,
    error::{ErrorKind, Result},
    operation::AggregateTarget,
    options::ReadConcern,
    selection_criteria::SelectionCriteria,
    Client,
    ClientSession,
    Collection,
    Database,
};

impl Client {
    /// Starts a new [`ChangeStream`] that receives events for all changes in the cluster. The
    /// stream does not observe changes from system collections or the "config", "local" or
    /// "admin" databases. Note that this method (`watch` on a cluster) is only supported in
    /// MongoDB 4.0 or greater.
    ///
    /// See the documentation [here](https://www.mongodb.com/docs/manual/changeStreams/) on change
    /// streams.
    ///
    /// Change streams require either a "majority" read concern or no read
    /// concern. Anything else will cause a server error.
    ///
    /// Note that using a `$project` stage to remove any of the `_id` `operationType` or `ns` fields
    /// will cause an error. The driver requires these fields to support resumability. For
    /// more information on resumability, see the documentation for
    /// [`ChangeStream`](change_stream/struct.ChangeStream.html)
    ///
    /// If the pipeline alters the structure of the returned events, the parsed type will need to be
    /// changed via [`ChangeStream::with_type`].
    ///
    /// `await` will return `Result<`[`ChangeStream`]`<`[`ChangeStreamEvent`]`<Document>>>` or
    /// `Result<`[`SessionChangeStream`]`<`[`ChangeStreamEvent`]`<Document>>>` if a
    /// [`ClientSession`] has been provided.
    pub fn watch(&self) -> Watch {
        Watch::new_cluster(self)
    }
}

impl Database {
    /// Starts a new [`ChangeStream`](change_stream/struct.ChangeStream.html) that receives events
    /// for all changes in this database. The stream does not observe changes from system
    /// collections and cannot be started on "config", "local" or "admin" databases.
    ///
    /// See the documentation [here](https://www.mongodb.com/docs/manual/changeStreams/) on change
    /// streams.
    ///
    /// Change streams require either a "majority" read concern or no read
    /// concern. Anything else will cause a server error.
    ///
    /// Note that using a `$project` stage to remove any of the `_id`, `operationType` or `ns`
    /// fields will cause an error. The driver requires these fields to support resumability. For
    /// more information on resumability, see the documentation for
    /// [`ChangeStream`](change_stream/struct.ChangeStream.html).
    ///
    /// If the pipeline alters the structure of the returned events, the parsed type will need to be
    /// changed via [`ChangeStream::with_type`].
    ///
    /// `await` will return `Result<`[`ChangeStream`]`<`[`ChangeStreamEvent`]`<Document>>>` or
    /// `Result<`[`SessionChangeStream`]`<`[`ChangeStreamEvent`]`<Document>>>` if a
    /// [`ClientSession`] has been provided.
    pub fn watch(&self) -> Watch {
        Watch::new(
            self.client(),
            AggregateTarget::Database(self.name().to_string()),
        )
    }
}

impl<T> Collection<T> {
    /// Starts a new [`ChangeStream`](change_stream/struct.ChangeStream.html) that receives events
    /// for all changes in this collection. A
    /// [`ChangeStream`](change_stream/struct.ChangeStream.html) cannot be started on system
    /// collections.
    ///
    /// See the documentation [here](https://www.mongodb.com/docs/manual/changeStreams/) on change
    /// streams.
    ///
    /// Change streams require either a "majority" read concern or no read concern. Anything else
    /// will cause a server error.
    ///
    /// `await` will return `Result<`[`ChangeStream`]`<`[`ChangeStreamEvent`]`<Document>>>` or
    /// `Result<`[`SessionChangeStream`]`<`[`ChangeStreamEvent`]`<Document>>>` if a
    /// [`ClientSession`] has been provided.
    pub fn watch(&self) -> Watch {
        Watch::new(self.client(), self.namespace().into())
    }
}

#[cfg(any(feature = "sync", feature = "tokio-sync"))]
impl crate::sync::Client {
    /// Starts a new [`ChangeStream`] that receives events for all changes in the cluster. The
    /// stream does not observe changes from system collections or the "config", "local" or
    /// "admin" databases. Note that this method (`watch` on a cluster) is only supported in
    /// MongoDB 4.0 or greater.
    ///
    /// See the documentation [here](https://www.mongodb.com/docs/manual/changeStreams/) on change
    /// streams.
    ///
    /// Change streams require either a "majority" read concern or no read
    /// concern. Anything else will cause a server error.
    pub fn watch(&self) -> Watch {
        self.async_client.watch()
    }
}

#[cfg(any(feature = "sync", feature = "tokio-sync"))]
impl crate::sync::Database {
    /// Starts a new [`ChangeStream`](change_stream/struct.ChangeStream.html) that receives events
    /// for all changes in this database. The stream does not observe changes from system
    /// collections and cannot be started on "config", "local" or "admin" databases.
    ///
    /// See the documentation [here](https://www.mongodb.com/docs/manual/changeStreams/) on change
    /// streams.
    ///
    /// Change streams require either a "majority" read concern or no read
    /// concern. Anything else will cause a server error.
    pub fn watch(&self) -> Watch {
        self.async_database.watch()
    }
}

#[cfg(any(feature = "sync", feature = "tokio-sync"))]
impl<T> crate::sync::Collection<T> {
    /// Starts a new [`ChangeStream`](change_stream/struct.ChangeStream.html) that receives events
    /// for all changes in this collection. A
    /// [`ChangeStream`](change_stream/struct.ChangeStream.html) cannot be started on system
    /// collections.
    ///
    /// See the documentation [here](https://www.mongodb.com/docs/manual/changeStreams/) on change
    /// streams.
    ///
    /// Change streams require either a "majority" read concern or no read concern. Anything else
    /// will cause a server error.
    pub fn watch(&self) -> Watch {
        self.async_collection.watch()
    }
}

pub struct ImplicitSession;
pub struct ExplicitSession<'a>(&'a mut ClientSession);

/// Starts a new [`ChangeStream`] that receives events for all changes in a given scope.  Create by
/// calling [`Client::watch`], [`Database::watch`], or [`Collection::watch`] and execute with
/// `await` (or [run](Watch::run) if using the sync client).
#[must_use]
pub struct Watch<'a, S = ImplicitSession> {
    client: &'a Client,
    target: AggregateTarget,
    pipeline: Vec<Document>,
    options: Option<ChangeStreamOptions>,
    session: S,
    cluster: bool,
}

impl<'a> Watch<'a, ImplicitSession> {
    fn new(client: &'a Client, target: AggregateTarget) -> Self {
        Self {
            client,
            target,
            pipeline: vec![],
            options: None,
            session: ImplicitSession,
            cluster: false,
        }
    }

    fn new_cluster(client: &'a Client) -> Self {
        Self {
            client,
            target: AggregateTarget::Database("admin".to_string()),
            pipeline: vec![],
            options: None,
            session: ImplicitSession,
            cluster: true,
        }
    }
}

impl<'a, S> Watch<'a, S> {
    /// Apply an aggregation pipeline to the change stream.
    ///
    /// Note that using a `$project` stage to remove any of the `_id`, `operationType` or `ns`
    /// fields will cause an error. The driver requires these fields to support resumability. For
    /// more information on resumability, see the documentation for
    /// [`ChangeStream`](change_stream/struct.ChangeStream.html)
    ///
    /// If the pipeline alters the structure of the returned events, the parsed type will need to be
    /// changed via [`ChangeStream::with_type`].
    pub fn pipeline(mut self, value: impl IntoIterator<Item = Document>) -> Self {
        self.pipeline = value.into_iter().collect();
        self
    }

    /// Specifies the logical starting point for the new change stream. Note that if a watched
    /// collection is dropped and recreated or newly renamed, `start_after` should be set instead.
    /// `resume_after` and `start_after` cannot be set simultaneously.
    ///
    /// For more information on resuming a change stream see the documentation [here](https://www.mongodb.com/docs/manual/changeStreams/#change-stream-resume-after)
    pub fn resume_after(mut self, value: impl Into<Option<ResumeToken>>) -> Self {
        // Implemented manually to accept `impl Into<Option<ResumeToken>>` so the output of
        // `ChangeStream::resume_token()` can be passed in directly.
        self.options().resume_after = value.into();
        self
    }

    option_setters!(options: ChangeStreamOptions;
        /// Configures how the
        /// [`ChangeStreamEvent::full_document`](crate::change_stream::event::ChangeStreamEvent::full_document)
        /// field will be populated. By default, the field will be empty for updates.
        full_document: FullDocumentType,

        /// Configures how the
        /// [`ChangeStreamEvent::full_document_before_change`](
        /// crate::change_stream::event::ChangeStreamEvent::full_document_before_change) field will be
        /// populated.  By default, the field will be empty for updates.
        full_document_before_change: FullDocumentBeforeChangeType,

        /// The change stream will only provide changes that occurred at or after the specified
        /// timestamp. Any command run against the server will return an operation time that can be
        /// used here.
        start_at_operation_time: Timestamp,

        /// Takes a resume token and starts a new change stream returning the first notification after
        /// the token. This will allow users to watch collections that have been dropped and
        /// recreated or newly renamed collections without missing any notifications.
        ///
        /// This feature is only available on MongoDB 4.2+.
        ///
        /// See the documentation [here](https://www.mongodb.com/docs/master/changeStreams/#change-stream-start-after) for more
        /// information.
        start_after: ResumeToken,

        /// The maximum amount of time for the server to wait on new documents to satisfy a change
        /// stream query.
        max_await_time: Duration,

        /// The number of documents to return per batch.
        batch_size: u32,

        /// Specifies a collation.
        collation: Collation,

        /// The read concern to use for the operation.
        ///
        /// If none is specified, the read concern defined on the object executing this operation will
        /// be used.
        read_concern: ReadConcern,

        /// The criteria used to select a server for this operation.
        ///
        /// If none is specified, the selection criteria defined on the object executing this operation
        /// will be used.
        selection_criteria: SelectionCriteria,

        /// Tags the query with an arbitrary [`Bson`] value to help trace the operation through the
        /// database profiler, currentOp and logs.
        ///
        /// The comment can be any [`Bson`] value on server versions 4.4+. On lower server versions,
        /// the comment must be a [`Bson::String`] value.
        comment: Bson,
    );
}

impl<'a> Watch<'a, ImplicitSession> {
    /// Use the provided ['ClientSession'].
    pub fn session<'s>(
        self,
        session: impl Into<&'s mut ClientSession>,
    ) -> Watch<'a, ExplicitSession<'s>> {
        Watch {
            client: self.client,
            target: self.target,
            pipeline: self.pipeline,
            options: self.options,
            session: ExplicitSession(session.into()),
            cluster: self.cluster,
        }
    }
}

action_impl! {
    impl Action<'a> for Watch<'a, ImplicitSession> {
        type Future = WatchImplicitSessionFuture;

        async fn execute(mut self) -> Result<ChangeStream<ChangeStreamEvent<Document>>> {
            resolve_options!(
                self.client,
                self.options,
                [read_concern, selection_criteria]
            );
            if self.cluster {
                self.options
                    .get_or_insert_with(Default::default)
                    .all_changes_for_cluster = Some(true);
            }
            self.client
                .execute_watch(self.pipeline, self.options, self.target, None)
                .await
        }

        fn sync_wrap(out) -> Result<crate::sync::ChangeStream<ChangeStreamEvent<Document>>> {
            out.map(crate::sync::ChangeStream::new)
        }
    }
}

action_impl! {
    impl Action<'a> for Watch<'a, ExplicitSession<'a>> {
        type Future = WatchExplicitSessionFuture;

        async fn execute(mut self) -> Result<SessionChangeStream<ChangeStreamEvent<Document>>> {
            resolve_read_concern_with_session!(
                self.client,
                self.options,
                Some(&mut *self.session.0)
            )?;
            resolve_selection_criteria_with_session!(
                self.client,
                self.options,
                Some(&mut *self.session.0)
            )?;
            if self.cluster {
                self.options
                    .get_or_insert_with(Default::default)
                    .all_changes_for_cluster = Some(true);
            }
            self.client
                .execute_watch_with_session(
                    self.pipeline,
                    self.options,
                    self.target,
                    None,
                    self.session.0,
                )
                .await
        }

        fn sync_wrap(out) -> Result<crate::sync::SessionChangeStream<ChangeStreamEvent<Document>>> {
            out.map(crate::sync::SessionChangeStream::new)
        }
    }
}
