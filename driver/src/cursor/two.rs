#![allow(missing_docs)]

#[cfg(feature = "sync")]
pub mod sync;

#[allow(unused)]
pub(crate) mod session;

use std::{collections::VecDeque, task::Poll};

use derive_where::derive_where;
use futures_core::Stream as AsyncStream;
use futures_util::{stream::StreamExt, FutureExt};
use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    bson::{RawDocument, RawDocumentBuf},
    error::{Error, Result},
    BoxFuture,
};

use super::{common::AdvanceResult, raw_batch::RawBatch};

#[derive_where(Debug)]
pub struct Cursor<T> {
    stream: Stream<'static, super::raw_batch::RawBatchCursor, T>,
}

impl<T> Cursor<T> {
    pub async fn advance(&mut self) -> Result<bool> {
        self.stream.state_mut().advance().await
    }

    pub fn current(&self) -> &RawDocument {
        self.stream.state().current()
    }

    pub fn has_next(&self) -> bool {
        let state = self.stream.state();
        !state.batch.is_empty() || state.raw.has_next()
    }

    pub fn deserialize_current<'a>(&'a self) -> Result<T>
    where
        T: Deserialize<'a>,
    {
        self.stream.state().deserialize_current()
    }

    pub fn with_type<'a, D>(self) -> Cursor<D>
    where
        D: Deserialize<'a>,
    {
        Cursor {
            stream: self.stream.with_type(),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_kill_watcher(&mut self, tx: tokio::sync::oneshot::Sender<()>) {
        self.stream.state_mut().raw.set_kill_watcher(tx);
    }

    #[cfg(test)]
    pub(crate) fn client(&self) -> &crate::Client {
        self.stream.state().raw.client()
    }

    #[cfg(test)]
    pub(crate) async fn try_advance(&mut self) -> Result<()> {
        self.stream.state_mut().try_advance().await.map(|_| ())
    }
}

impl<T> crate::cursor::NewCursor for Cursor<T> {
    fn generic_new(
        client: crate::Client,
        spec: crate::cursor::CursorSpecification,
        implicit_session: Option<crate::ClientSession>,
        pinned: Option<crate::cmap::conn::PinnedConnectionHandle>,
    ) -> Result<Self> {
        let raw = crate::cursor::raw_batch::RawBatchCursor::generic_new(
            client,
            spec,
            implicit_session,
            pinned,
        )?;
        Ok(Self {
            stream: Stream::new(raw),
        })
    }
}

#[derive_where(Debug)]
struct Stream<'a, Raw, T> {
    state: StreamState<'a, Raw>,
    _phantom: std::marker::PhantomData<fn() -> T>,
}

impl<'a, Raw, T> Stream<'a, Raw, T> {
    fn new(raw: Raw) -> Self {
        Self::from_cursor(CursorState::new(raw))
    }

    fn from_cursor(cs: CursorState<Raw>) -> Self {
        Self {
            state: StreamState::Idle(cs),
            _phantom: std::marker::PhantomData,
        }
    }

    fn state(&self) -> &CursorState<Raw> {
        match &self.state {
            StreamState::Idle(state) => state,
            _ => panic!("state access while streaming"),
        }
    }

    fn state_mut(&mut self) -> &mut CursorState<Raw> {
        match &mut self.state {
            StreamState::Idle(state) => state,
            _ => panic!("state access while streaming"),
        }
    }

    fn take_state(&mut self) -> CursorState<Raw> {
        match std::mem::replace(&mut self.state, StreamState::Polling) {
            StreamState::Idle(state) => state,
            _ => panic!("state access while streaming"),
        }
    }

    fn with_type<D>(self) -> Stream<'a, Raw, D> {
        Stream {
            state: self.state,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[derive_where(Debug)]
enum StreamState<'a, Raw> {
    Idle(CursorState<Raw>),
    Polling,
    Advance(#[derive_where(skip)] BoxFuture<'a, AdvanceDone<Raw>>),
}

struct AdvanceDone<Raw> {
    state: CursorState<Raw>,
    result: Result<bool>,
}

#[derive_where(Debug)]
struct CursorState<Raw> {
    #[derive_where(skip)]
    raw: Raw,
    batch: VecDeque<RawDocumentBuf>,
}

impl<Raw> CursorState<Raw> {
    fn new(raw: Raw) -> Self {
        Self {
            raw,
            batch: VecDeque::new(),
        }
    }

    fn current(&self) -> &RawDocument {
        self.batch.front().unwrap()
    }

    fn deserialize_current<'a, V>(&'a self) -> Result<V>
    where
        V: Deserialize<'a>,
    {
        crate::bson_compat::deserialize_from_slice(self.current().as_bytes()).map_err(Error::from)
    }

    fn map<G>(self, f: impl FnOnce(Raw) -> G) -> CursorState<G> {
        CursorState {
            raw: f(self.raw),
            batch: self.batch,
        }
    }
}

impl<Raw: AsyncStream<Item = Result<RawBatch>> + Unpin> CursorState<Raw> {
    /// Attempt to advance the cursor forward to the next item. If there are no items cached
    /// locally, perform getMores until the cursor is exhausted or the buffer has been refilled.
    /// Return whether or not the cursor has been advanced.
    async fn advance(&mut self) -> Result<bool> {
        loop {
            match self.try_advance().await? {
                AdvanceResult::Advanced => return Ok(true),
                AdvanceResult::Exhausted => return Ok(false),
                AdvanceResult::Waiting => continue,
            }
        }
    }

    /// Attempt to advance the cursor forward to the next item. If there are no items cached
    /// locally, perform a single getMore to attempt to retrieve more.
    async fn try_advance(&mut self) -> Result<AdvanceResult> {
        // Next stored batch item
        self.batch.pop_front();
        if !self.batch.is_empty() {
            return Ok(AdvanceResult::Advanced);
        }

        // Batch is empty, need a new one
        let Some(raw_batch) = self.raw.next().await else {
            return Ok(AdvanceResult::Exhausted);
        };
        let raw_batch = raw_batch?;
        for item in raw_batch.doc_slices()? {
            self.batch.push_back(
                item?
                    .as_document()
                    .ok_or_else(|| Error::invalid_response("invalid cursor batch item"))?
                    .to_owned(),
            );
        }
        return Ok(if self.batch.is_empty() {
            AdvanceResult::Waiting
        } else {
            AdvanceResult::Advanced
        });
    }
}

impl<'a, Raw: 'a + AsyncStream<Item = Result<RawBatch>> + Send + Unpin, T: DeserializeOwned>
    AsyncStream for Stream<'a, Raw, T>
{
    type Item = Result<T>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            match std::mem::replace(&mut self.state, StreamState::Polling) {
                StreamState::Idle(mut state) => {
                    self.state = StreamState::Advance(
                        async move {
                            let result = state.advance().await;
                            AdvanceDone { state, result }
                        }
                        .boxed(),
                    );
                    continue;
                }
                StreamState::Advance(mut fut) => {
                    return match fut.poll_unpin(cx) {
                        Poll::Pending => Poll::Pending,
                        Poll::Ready(ar) => {
                            let out = match ar.result {
                                Err(e) => Some(Err(e)),
                                Ok(false) => None,
                                Ok(true) => Some(ar.state.deserialize_current()),
                            };
                            self.state = StreamState::Idle(ar.state);
                            return Poll::Ready(out);
                        }
                    }
                }
                StreamState::Polling => {
                    return Poll::Ready(Some(Err(Error::internal(
                        "attempt to poll cursor already in polling state",
                    ))))
                }
            }
        }
    }
}

impl<T> AsyncStream for Cursor<T>
where
    T: DeserializeOwned,
{
    type Item = Result<T>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}
