use crate::{
    bson::{Document, RawDocument, Timestamp},
    bson_compat::deserialize_from_slice,
    error::Error,
    operation::OperationTarget,
};
use serde::de::DeserializeOwned;

#[cfg(feature = "bson-3")]
use crate::bson_compat::RawBsonRefExt as _;
use crate::{
    change_stream::event::ResumeToken,
    error::{ErrorKind, Result},
    ClientSession,
};

/// Arguments passed to a `watch` method, captured to allow resume.
#[derive(Debug, Clone)]
pub(crate) struct WatchArgs {
    /// The pipeline of stages to append to an initial `$changeStream` stage.
    pub(crate) pipeline: Vec<Document>,

    /// The original target of the change stream.
    pub(crate) target: OperationTarget,

    /// The options provided to the initial `$changeStream` stage.
    pub(crate) options: Option<super::options::ChangeStreamOptions>,
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
    pub(super) fn take(&mut self) -> Self {
        Self {
            initial_operation_time: self.initial_operation_time,
            resume_token: self.resume_token.clone(),
            resume_attempted: self.resume_attempted,
            document_returned: self.document_returned,
            implicit_session: self.implicit_session.take(),
        }
    }
}

#[derive(Debug)]
pub(super) struct CursorWrapper<Inner> {
    /// The cursor to iterate over event instances.
    pub(super) cursor: Inner,

    /// Arguments to `watch` that created this change stream.
    pub(super) args: WatchArgs,

    /// Dynamic information associated with this change stream.
    pub(super) data: ChangeStreamData,
}

impl<Inner> CursorWrapper<Inner> {
    pub(super) fn new(cursor: Inner, args: WatchArgs, data: ChangeStreamData) -> Self {
        Self { cursor, args, data }
    }

    pub(super) async fn next_if_any<T: DeserializeOwned>(
        &mut self,
        session: &mut Inner::Session,
    ) -> Result<Option<T>>
    where
        Inner: InnerCursor,
    {
        loop {
            match self.cursor.try_advance(session).await {
                Ok(has) => {
                    self.data.resume_token = self.cursor.get_resume_token()?;
                    return if has {
                        self.data.document_returned = true;
                        deserialize_from_slice(self.cursor.current().as_bytes())
                            .map(Some)
                            .map_err(Error::from)
                    } else {
                        Ok(None)
                    };
                }
                Err(e) if e.is_resumable() => {
                    let (new_cursor, new_args) = self
                        .cursor
                        .execute_watch(self.args.clone(), self.data.take(), session)
                        .await?;
                    // Ensure that the old cursor is killed on the server selected for the new one.
                    self.cursor.set_drop_address(&new_cursor);
                    self.cursor = new_cursor;
                    self.args = new_args;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

pub(super) fn get_resume_token(
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

pub(super) trait InnerCursor: Sized {
    type Session;

    async fn try_advance(&mut self, session: &mut Self::Session) -> Result<bool>;
    fn get_resume_token(&self) -> Result<Option<ResumeToken>>;
    fn current(&self) -> &RawDocument;
    async fn execute_watch(
        &mut self,
        args: WatchArgs,
        data: ChangeStreamData,
        session: &mut Self::Session,
    ) -> Result<(Self, WatchArgs)>;
    fn set_drop_address(&mut self, from: &Self);
}
