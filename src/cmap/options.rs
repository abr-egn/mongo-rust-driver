#[cfg(test)]
use std::cmp::Ordering;
use std::time::Duration;

use derive_where::derive_where;
#[cfg(test)]
use serde::de::{Deserializer, Error};
use serde::Deserialize;

use crate::{
    client::auth::Credential,
    event::{
        cmap::{CmapEvent, ConnectionPoolOptions as EventOptions},
        EventHandler,
    },
    options::ClientOptions,
    serde_util,
};

/// Contains the options for creating a connection pool.
#[derive(Clone, Default, Deserialize)]
#[derive_where(Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionPoolOptions {
    /// The credential to use for authenticating connections in this pool.
    #[serde(skip)]
    pub(crate) credential: Option<Credential>,

    /// Processes all events generated by the pool.
    #[derive_where(skip)]
    #[serde(skip)]
    pub(crate) cmap_event_handler: Option<EventHandler<CmapEvent>>,

    /// Interval between background thread maintenance runs (e.g. ensure minPoolSize).
    #[cfg(test)]
    #[serde(rename = "backgroundThreadIntervalMS")]
    pub(crate) background_thread_interval: Option<BackgroundThreadInterval>,

    /// Connections that have been ready for usage in the pool for longer than `max_idle_time` will
    /// not be used.
    ///
    /// The default is that connections will not be closed due to being idle.
    #[serde(rename = "maxIdleTimeMS")]
    #[serde(default)]
    #[serde(deserialize_with = "serde_util::deserialize_duration_option_from_u64_millis")]
    pub(crate) max_idle_time: Option<Duration>,

    /// The maximum number of connections that the pool can have at a given time. This includes
    /// connections which are currently checked out of the pool.
    ///
    /// The default is 10.
    pub(crate) max_pool_size: Option<u32>,

    /// The minimum number of connections that the pool can have at a given time. This includes
    /// connections which are currently checked out of the pool. If fewer than `min_pool_size`
    /// connections are in the pool, connections will be added to the pool in the background.
    ///
    /// The default is that no minimum is enforced
    pub(crate) min_pool_size: Option<u32>,

    /// Whether to start the pool as "ready" or not.
    /// For tests only.
    #[cfg(test)]
    pub(crate) ready: Option<bool>,

    /// Whether or not the client is connecting to a MongoDB cluster through a load balancer.
    pub(crate) load_balanced: Option<bool>,

    /// The maximum number of new connections that can be created concurrently.
    ///
    /// The default is 2.
    pub(crate) max_connecting: Option<u32>,
}

impl ConnectionPoolOptions {
    pub(crate) fn from_client_options(options: &ClientOptions) -> Self {
        Self {
            max_idle_time: options.max_idle_time,
            min_pool_size: options.min_pool_size,
            max_pool_size: options.max_pool_size,
            cmap_event_handler: options.cmap_event_handler.clone(),
            #[cfg(test)]
            background_thread_interval: None,
            #[cfg(test)]
            ready: None,
            load_balanced: options.load_balanced,
            credential: options.credential.clone(),
            max_connecting: options.max_connecting,
        }
    }

    pub(crate) fn to_event_options(&self) -> EventOptions {
        EventOptions {
            max_idle_time: self.max_idle_time,
            min_pool_size: self.min_pool_size,
            max_pool_size: self.max_pool_size,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum BackgroundThreadInterval {
    Never,
    Every(Duration),
}

#[cfg(test)]
impl<'de> Deserialize<'de> for BackgroundThreadInterval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = i64::deserialize(deserializer)?;
        Ok(match millis.cmp(&0) {
            Ordering::Less => BackgroundThreadInterval::Never,
            Ordering::Equal => return Err(D::Error::custom("zero is not allowed")),
            Ordering::Greater => {
                BackgroundThreadInterval::Every(Duration::from_millis(millis as u64))
            }
        })
    }
}
