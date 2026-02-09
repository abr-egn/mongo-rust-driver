use serde::{de::DeserializeOwned, Deserialize};
#[cfg(test)]
use tokio::sync::oneshot;

use futures_util::StreamExt;

use crate::{
    bson::{Document, RawDocument},
    cursor::raw_batch::SessionRawBatchCursor,
    error::Result,
    options::ServerAddress,
    raw_batch_cursor::SessionRawBatchCursorStream,
    ClientSession,
};

pub struct SessionCursor<T> {
    // `None` while a `SessionCursorStream` is live; because that stream holds a `&mut` to this
    // struct, any access of this will always see `Some`.
    state: Option<super::CursorState<()>>,
    raw: crate::cursor::raw_batch::SessionRawBatchCursor,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> crate::cursor::NewCursor for SessionCursor<T> {
    fn generic_new(
        client: crate::Client,
        spec: crate::cursor::CursorSpecification,
        implicit_session: Option<ClientSession>,
        pinned: Option<crate::cmap::conn::PinnedConnectionHandle>,
    ) -> Result<Self> {
        let raw = crate::cursor::raw_batch::SessionRawBatchCursor::generic_new(
            client,
            spec,
            implicit_session,
            pinned,
        )?;
        Ok(Self {
            state: Some(super::CursorState::new(())),
            raw,
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<T> SessionCursor<T> {
    pub fn stream<'session>(
        &mut self,
        session: &'session mut ClientSession,
    ) -> SessionCursorStream<'_, 'session, T> {
        let raw_stream = self.raw.stream(session);
        let stream = super::Stream::from_cursor(self.state.take().unwrap().map(|_| raw_stream));
        SessionCursorStream {
            parent: &mut self.state,
            stream,
        }
    }

    pub async fn next(&mut self, session: &mut ClientSession) -> Option<Result<T>>
    where
        T: DeserializeOwned,
    {
        self.stream(session).next().await
    }

    pub async fn advance(&mut self, session: &mut ClientSession) -> Result<bool> {
        self.stream(session).stream.state_mut().advance().await
    }

    #[cfg(test)]
    pub(crate) async fn try_advance(&mut self, session: &mut ClientSession) -> Result<()> {
        self.stream(session)
            .stream
            .state_mut()
            .try_advance()
            .await
            .map(|_| ())
    }

    fn state(&self) -> &super::CursorState<()> {
        self.state.as_ref().unwrap()
    }

    fn state_mut(&mut self) -> &mut super::CursorState<()> {
        self.state.as_mut().unwrap()
    }

    pub fn current(&self) -> &RawDocument {
        self.state().current()
    }

    pub fn deserialize_current<'a>(&'a self) -> Result<T>
    where
        T: Deserialize<'a>,
    {
        self.state().deserialize_current()
    }

    /// Update the type streamed values will be parsed as.
    pub fn with_type<'a, D>(mut self) -> SessionCursor<D>
    where
        D: Deserialize<'a>,
    {
        SessionCursor {
            state: self.state,
            raw: self.raw,
            _phantom: std::marker::PhantomData,
        }
    }

    pub(crate) fn address(&self) -> &ServerAddress {
        self.raw.address()
    }

    pub(crate) fn set_drop_address(&mut self, address: ServerAddress) {
        self.raw.set_drop_address(address);
    }

    #[cfg(test)]
    pub(crate) fn set_kill_watcher(&mut self, tx: oneshot::Sender<()>) {
        self.raw.set_kill_watcher(tx);
    }
}

pub struct SessionCursorStream<'cursor, 'session, T = Document> {
    parent: &'cursor mut Option<super::CursorState<()>>,
    stream: super::Stream<'cursor, SessionRawBatchCursorStream<'cursor, 'session>, T>,
}

impl<T> Drop for SessionCursorStream<'_, '_, T> {
    fn drop(&mut self) {
        *self.parent = Some(self.stream.take_state().map(|_| ()));
    }
}

impl<'cursor, 'session, T> futures_core::Stream for SessionCursorStream<'cursor, 'session, T>
where
    T: DeserializeOwned,
    'session: 'cursor,
{
    type Item = Result<T>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}
