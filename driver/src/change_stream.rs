//! Contains the functionality for change streams.
pub mod event;
pub(crate) mod options;
pub mod session;

#[cfg(test)]
use std::collections::VecDeque;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{
    bson::{Document, RawDocument, RawDocumentBuf, Timestamp},
    bson_compat::deserialize_from_slice,
    error::Error,
    operation::OperationTarget,
};
use derive_where::derive_where;
use futures_core::{future::BoxFuture, Stream};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
#[cfg(test)]
use tokio::sync::oneshot;

#[cfg(feature = "bson-3")]
use crate::bson_compat::RawBsonRefExt as _;
use crate::{
    change_stream::event::{ChangeStreamEvent, ResumeToken},
    error::{ErrorKind, Result},
    ClientSession,
    Cursor,
};

/// A `ChangeStream` streams the ongoing changes of its associated collection, database or
/// deployment. `ChangeStream` instances should be created with method `watch` against the relevant
/// target.
///
/// `ChangeStream`s are "resumable", meaning that they can be restarted at a given place in the
/// stream of events. This is done automatically when the `ChangeStream` encounters certain
/// ["resumable"](https://specifications.readthedocs.io/en/latest/change-streams/change-streams/#resumable-error)
/// errors, such as transient network failures. It can also be done manually by passing
/// a [`ResumeToken`] retrieved from a past event into either the
/// [`resume_after`](crate::action::Watch::resume_after) or
/// [`start_after`](crate::action::Watch::start_after) (4.2+) options used to create the
/// `ChangeStream`. Issuing a raw change stream aggregation is discouraged unless users wish to
/// explicitly opt out of resumability.
///
/// A `ChangeStream` can be iterated like any other [`Stream`]:
///
/// ```
/// # use futures::stream::StreamExt;
/// # use mongodb::{Client, error::Result, bson::doc,
/// # change_stream::event::ChangeStreamEvent};
/// # use tokio::task;
/// #
/// # async fn func() -> Result<()> {
/// # let client = Client::with_uri_str("mongodb://example.com").await?;
/// # let coll = client.database("foo").collection("bar");
/// let mut change_stream = coll.watch().await?;
/// let coll_ref = coll.clone();
/// task::spawn(async move {
///     coll_ref.insert_one(doc! { "x": 1 }).await;
/// });
/// while let Some(event) = change_stream.next().await.transpose()? {
///     println!("operation performed: {:?}, document: {:?}", event.operation_type, event.full_document);
///     // operation performed: Insert, document: Some(Document({"x": Int32(1)}))
/// }
/// #
/// # Ok(())
/// # }
/// ```
///
/// If a [`ChangeStream`] is still open when it goes out of scope, it will automatically be closed
/// via an asynchronous [killCursors](https://www.mongodb.com/docs/manual/reference/command/killCursors/) command executed
/// from its [`Drop`](https://doc.rust-lang.org/std/ops/trait.Drop.html) implementation.
///
/// See the documentation [here](https://www.mongodb.com/docs/manual/changeStreams) for more
/// details. Also see the documentation on [usage recommendations](https://www.mongodb.com/docs/manual/administration/change-streams-production-recommendations/).
#[derive_where(Debug)]
pub struct ChangeStream<T>
where
    T: DeserializeOwned,
{
    /// The cursor to iterate over event instances.
    cursor: Cursor<RawDocumentBuf>,

    /// Arguments to `watch` that created this change stream.
    args: WatchArgs,

    /// Dynamic information associated with this change stream.
    data: ChangeStreamData,

    /// A pending future for a resume.
    #[derive_where(skip)]
    pending_resume: Option<BoxFuture<'static, Result<ChangeStream<T>>>>,
}

impl<T> ChangeStream<T>
where
    T: DeserializeOwned,
{
    pub(crate) fn new(
        cursor: Cursor<RawDocumentBuf>,
        args: WatchArgs,
        data: ChangeStreamData,
    ) -> Self {
        let pending_resume: Option<BoxFuture<'static, Result<ChangeStream<T>>>> = None;
        Self {
            cursor,
            args,
            data,
            pending_resume,
        }
    }

    /// Returns the cached resume token that can be used to resume after the most recently returned
    /// change.
    ///
    /// See the documentation
    /// [here](https://www.mongodb.com/docs/manual/changeStreams/#change-stream-resume-token) for more
    /// information on change stream resume tokens.
    pub fn resume_token(&self) -> Option<ResumeToken> {
        self.data.resume_token.clone()
    }

    /// Update the type streamed values will be parsed as.
    pub fn with_type<D: DeserializeOwned>(self) -> ChangeStream<D> {
        ChangeStream {
            cursor: self.cursor.with_type(),
            args: self.args,
            data: self.data,
            pending_resume: None,
        }
    }

    /// Returns whether the change stream will continue to receive events.
    pub fn is_alive(&self) -> bool {
        !self.cursor.raw().is_exhausted()
    }

    /// Retrieves the next result from the change stream, if any.
    ///
    /// Where calling `Stream::next` will internally loop until a change document is received,
    /// this will make at most one request and return `None` if the returned document batch is
    /// empty.  This method should be used when storing the resume token in order to ensure the
    /// most up to date token is received, e.g.
    ///
    /// ```
    /// # use mongodb::{Client, Collection, bson::Document, error::Result};
    /// # async fn func() -> Result<()> {
    /// # let client = Client::with_uri_str("mongodb://example.com").await?;
    /// # let coll: Collection<Document> = client.database("foo").collection("bar");
    /// let mut change_stream = coll.watch().await?;
    /// let mut resume_token = None;
    /// while change_stream.is_alive() {
    ///     if let Some(event) = change_stream.next_if_any().await? {
    ///         // process event
    ///     }
    ///     resume_token = change_stream.resume_token();
    /// }
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub async fn next_if_any(&mut self) -> Result<Option<T>> {
        if self.cursor.try_advance().await? {
            Ok(Some(
                deserialize_from_slice(self.cursor.current().as_bytes()).map_err(Error::from)?,
            ))
        } else {
            Ok(None)
        }
    }

    #[cfg(test)]
    pub(crate) fn set_kill_watcher(&mut self, tx: oneshot::Sender<()>) {
        self.cursor.raw_mut().set_kill_watcher(tx);
    }

    #[cfg(test)]
    pub(crate) fn current_batch(&self) -> &VecDeque<RawDocumentBuf> {
        self.cursor.batch()
    }

    #[cfg(test)]
    pub(crate) fn client(&self) -> &crate::Client {
        self.cursor.raw().client()
    }
}

/// Arguments passed to a `watch` method, captured to allow resume.
#[derive(Debug, Clone)]
pub(crate) struct WatchArgs {
    /// The pipeline of stages to append to an initial `$changeStream` stage.
    pub(crate) pipeline: Vec<Document>,

    /// The original target of the change stream.
    pub(crate) target: OperationTarget,

    /// The options provided to the initial `$changeStream` stage.
    pub(crate) options: Option<options::ChangeStreamOptions>,
}

/// Dynamic change stream data needed for resume.
#[derive(Debug, Default)]
pub(crate) struct ChangeStreamData {
    /// The `operationTime` returned by the initial `aggregate` command.
    pub(crate) initial_operation_time: Option<Timestamp>,

    /// The cached resume token.
    pub(crate) resume_token: Option<ResumeToken>,

    /// Whether or not the change stream has attempted a resume, used to attempt a resume only
    /// once.
    pub(crate) resume_attempted: bool,

    /// Whether or not the change stream has returned a document, used to update resume token
    /// during an automatic resume.
    pub(crate) document_returned: bool,

    /// The implicit session used to create the original cursor.
    pub(crate) implicit_session: Option<ClientSession>,
}

impl ChangeStreamData {
    fn take(&mut self) -> Self {
        Self {
            initial_operation_time: self.initial_operation_time,
            resume_token: self.resume_token.clone(),
            resume_attempted: self.resume_attempted,
            document_returned: self.document_returned,
            implicit_session: self.implicit_session.take(),
        }
    }
}

fn get_resume_token(
    batch_value: Option<&RawDocument>,
    is_last: bool,
    batch_token: Option<&ResumeToken>,
) -> Result<Option<ResumeToken>> {
    Ok(match batch_value {
        Some(doc) => {
            let doc_token = doc
                .get("_id")?
                .ok_or_else(|| Error::from(ErrorKind::MissingResumeToken))?;
            if is_last && batch_token.is_some() {
                batch_token.cloned()
            } else {
                Some(ResumeToken(doc_token.to_raw_bson()))
            }
        }
        None => None,
    })
}

impl<T> Stream for ChangeStream<T>
where
    T: DeserializeOwned,
{
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(mut pending) = self.pending_resume.take() {
                match Pin::new(&mut pending).poll(cx) {
                    Poll::Pending => {
                        self.pending_resume = Some(pending);
                        return Poll::Pending;
                    }
                    Poll::Ready(Ok(new_stream)) => {
                        // Ensure that the old cursor is killed on the server selected for the new
                        // one.
                        self.cursor
                            .raw_mut()
                            .set_drop_address(new_stream.cursor.raw().address().clone());
                        self.cursor = new_stream.cursor;
                        self.args = new_stream.args;
                        // After a successful resume, another resume must be allowed.
                        self.data.resume_attempted = false;
                        continue;
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                }
            }
            return match self.cursor.poll_next_unpin(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(value) => match value.transpose() {
                    Ok(item) => {
                        self.data.resume_token = get_resume_token(
                            item.as_deref(),
                            self.cursor.batch().is_empty(),
                            self.cursor.raw().post_batch_resume_token(),
                        )?;
                        Poll::Ready(item.map(|raw| {
                            deserialize_from_slice(raw.as_bytes())
                                .map_err(crate::error::Error::from)
                        }))
                    }
                    Err(e) if e.is_resumable() && !self.data.resume_attempted => {
                        self.data.resume_attempted = true;
                        let client = self.cursor.raw().client().clone();
                        let args = self.args.clone();
                        let mut data = self.data.take();
                        data.implicit_session = self.cursor.raw_mut().take_implicit_session();
                        self.pending_resume = Some(Box::pin(async move {
                            let new_stream: Result<ChangeStream<ChangeStreamEvent<()>>> = client
                                .execute_watch(args.pipeline, args.options, args.target, Some(data))
                                .await;
                            new_stream.map(|cs| cs.with_type::<T>())
                        }));
                        // Iterate the loop so the new future gets polled and can register
                        // wakers.
                        continue;
                    }
                    Err(e) => Poll::Ready(Some(Err(e))),
                },
            };
        }
    }
}
