use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use crate::{
    cmap::{establish::ConnectionEstablisher, options::ConnectionPoolOptions, ConnectionPool},
    options::{ClientOptions, ServerAddress},
    sdam::TopologyUpdater,
};

/// Contains the state for a given server in the topology.
#[derive(Debug)]
pub(crate) struct Server {
    pub(crate) address: SdamServerAddress,

    /// The connection pool for the server.
    pub(crate) pool: ConnectionPool,

    /// Number of operations currently using this server.
    operation_count: AtomicU32,
}

impl Server {
    #[cfg(test)]
    pub(crate) fn new_mocked(address: SdamServerAddress, operation_count: u32) -> Self {
        Self {
            address: address.clone(),
            pool: ConnectionPool::new_mocked(address),
            operation_count: AtomicU32::new(operation_count),
        }
    }

    /// Create a new reference counted `Server`, including its connection pool.
    pub(crate) fn new(
        address: SdamServerAddress,
        options: ClientOptions,
        connection_establisher: ConnectionEstablisher,
        topology_updater: TopologyUpdater,
        topology_id: bson::oid::ObjectId,
    ) -> Arc<Server> {
        Arc::new(Self {
            pool: ConnectionPool::new(
                address.clone(),
                connection_establisher,
                topology_updater,
                topology_id,
                Some(ConnectionPoolOptions::from_client_options(&options)),
            ),
            address,
            operation_count: AtomicU32::new(0),
        })
    }

    pub(crate) fn increment_operation_count(&self) {
        self.operation_count.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn decrement_operation_count(&self) {
        self.operation_count.fetch_sub(1, Ordering::SeqCst);
    }

    pub(crate) fn operation_count(&self) -> u32 {
        self.operation_count.load(Ordering::SeqCst)
    }
}

/// This mirrors `ServerAddress`, but stores IP addresses in parsed form to avoid normalization
/// concerns.
#[derive(Debug, Clone, Eq)]
pub(crate) enum SdamServerAddress {
    Tcp {
        host: ServerHost,
        port: Option<u16>,
    },
    #[cfg(unix)]
    Unix {
        path: std::path::PathBuf,
    },
}

impl SdamServerAddress {
    // Translate to a `ServerAddress` for display.  Equivalent to `clone().into()` but helps type
    // inference along.
    pub(crate) fn display(&self) -> ServerAddress {
        self.clone().into()
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ServerHost {
    Ip(std::net::IpAddr),
    Name(String),
}

impl ServerHost {
    pub(crate) fn name(&self) -> String {
        match self {
            Self::Ip(addr) => addr.to_string(),
            Self::Name(s) => s.clone(),
        }
    }
}

impl From<ServerAddress> for SdamServerAddress {
    fn from(value: ServerAddress) -> Self {
        match value {
            ServerAddress::Tcp { host, port } => {
                use std::net::IpAddr;
                let host = if let Ok(v4) = host.parse() {
                    ServerHost::Ip(IpAddr::V4(v4))
                } else if let Ok(v6) = host.parse() {
                    ServerHost::Ip(IpAddr::V6(v6))
                } else {
                    ServerHost::Name(host)
                };
                Self::Tcp { host, port }
            }
            #[cfg(unix)]
            ServerAddress::Unix { path } => Self::Unix { path },
        }
    }
}

impl PartialEq for SdamServerAddress {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Tcp { host, port },
                Self::Tcp {
                    host: other_host,
                    port: other_port,
                },
            ) => host == other_host && port.unwrap_or(27017) == other_port.unwrap_or(27017),
            #[cfg(unix)]
            (Self::Unix { path }, Self::Unix { path: other_path }) => path == other_path,
            #[cfg(unix)]
            _ => false,
        }
    }
}

impl std::hash::Hash for SdamServerAddress {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        match self {
            Self::Tcp { host, port } => {
                host.hash(state);
                port.unwrap_or(27017).hash(state);
            }
            #[cfg(unix)]
            Self::Unix { path } => path.hash(state),
        }
    }
}

impl serde::Serialize for SdamServerAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.display().serialize(serializer)
    }
}

impl Default for SdamServerAddress {
    fn default() -> Self {
        ServerAddress::default().into()
    }
}
