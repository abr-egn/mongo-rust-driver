#![allow(missing_docs)]

mod common;
pub(crate) mod session;
#[cfg(feature = "sync")]
pub mod sync;

use std::task::Poll;

use derive_where::derive_where;
use futures_core::Stream as AsyncStream;
use futures_util::stream::StreamExt;
use serde::{de::DeserializeOwned, Deserialize};

use crate::{bson::RawDocument, error::Result};

#[derive_where(Debug)]
pub struct Cursor<T> {
    stream: common::Stream<'static, super::raw_batch::RawBatchCursor, T>,
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
        !state.is_empty() || state.raw.has_next()
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
            stream: common::Stream::new(raw),
        })
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
