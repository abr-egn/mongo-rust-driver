#![allow(missing_docs)]

use std::{collections::VecDeque, task::Poll};

use futures_core::Stream;
use futures_util::{stream::StreamExt, FutureExt};
use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    bson::{RawDocument, RawDocumentBuf},
    error::{Error, Result},
    BoxFuture,
};

pub struct Cursor<T> {
    state: StreamState,
    _phantom: std::marker::PhantomData<fn() -> T>,
}

impl<T> Cursor<T> {
    pub async fn advance(&mut self) -> Result<bool> {
        self.state.get_mut().advance().await
    }

    pub fn current(&self) -> &RawDocument {
        self.state.get().current()
    }

    pub fn has_next(&self) -> bool {
        self.state.get().has_next()
    }

    pub fn deserialize_current<'a>(&'a self) -> Result<T>
    where
        T: Deserialize<'a>,
    {
        crate::bson_compat::deserialize_from_slice(self.current().as_bytes()).map_err(Error::from)
    }

    pub fn with_type<'a, D>(self) -> Cursor<D>
    where
        D: Deserialize<'a>,
    {
        Cursor {
            state: self.state,
            _phantom: std::marker::PhantomData,
        }
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
            state: StreamState::Idle(CursorState {
                raw,
                batch: VecDeque::new(),
            }),
            _phantom: std::marker::PhantomData,
        })
    }
}

enum StreamState {
    Idle(CursorState),
    Polling,
    Advance(BoxFuture<'static, AdvanceResult>),
}

struct AdvanceResult {
    state: CursorState,
    result: Result<bool>,
}

impl StreamState {
    fn get(&self) -> &CursorState {
        match self {
            Self::Idle(s) => s,
            _ => panic!("state access while streaming"),
        }
    }

    fn get_mut(&mut self) -> &mut CursorState {
        match self {
            Self::Idle(s) => s,
            _ => panic!("state access while streaming"),
        }
    }
}

struct CursorState {
    raw: crate::raw_batch_cursor::RawBatchCursor,
    batch: VecDeque<RawDocumentBuf>,
}

impl CursorState {
    async fn advance(&mut self) -> Result<bool> {
        // Next stored batch item
        self.batch.pop_front();
        if !self.batch.is_empty() {
            return Ok(true);
        }

        // Batch is empty, need a new one
        if self.raw.is_exhausted() {
            return Ok(false);
        }
        let Some(raw_batch) = self.raw.next().await else {
            return Ok(false);
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
        return Ok(!self.batch.is_empty());
    }

    fn current(&self) -> &RawDocument {
        self.batch.front().unwrap()
    }

    fn has_next(&self) -> bool {
        !self.batch.is_empty() || !self.raw.is_exhausted()
    }
}

impl<T> Stream for Cursor<T>
where
    T: DeserializeOwned,
{
    type Item = Result<T>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if !self.has_next() {
            return Poll::Ready(None);
        }

        match std::mem::replace(&mut self.state, StreamState::Polling) {
            StreamState::Idle(mut state) => {
                self.state = StreamState::Advance(
                    async move {
                        let result = state.advance().await;
                        AdvanceResult { state, result }
                    }
                    .boxed(),
                );
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            StreamState::Advance(mut fut) => match fut.poll_unpin(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(ar) => {
                    self.state = StreamState::Idle(ar.state);
                    Poll::Ready(match ar.result {
                        Err(e) => Some(Err(e)),
                        Ok(false) => None,
                        Ok(true) => {
                            todo!()
                        }
                    })
                }
            },
            StreamState::Polling => Poll::Ready(Some(Err(Error::internal(
                "attempt to poll cursor already in polling state",
            )))),
        }
    }
}
