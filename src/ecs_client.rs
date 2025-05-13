use std::sync::Arc;

use bevy_ecs::{prelude::*, system::SystemId};
use bson::{oid::ObjectId, Document, RawDocumentBuf};

use crate::{
    cmap::{
        conn::{wire::Message, PendingConnection},
        establish::{ConnectionEstablisher, EstablisherOptions},
        Connection,
    },
    error::Result,
    event::cmap::CmapEventEmitter,
    options::ClientOptions,
};

/// An experimental ECS-based MongoDB client.
pub struct Client {
    world: Arc<tokio::sync::Mutex<World>>,
    systems: Systems,
}

struct Systems {
    check_out_connection: SystemId<(), Option<Entity>>,
    new_pending_connection: SystemId<(), PendingConnection>,
}

impl Client {
    /// Creates a new client with the given options.
    pub async fn new(opts: &ClientOptions) -> Result<Self> {
        // resources: ClientOptions, ConnectionString
        // components: Connection, CheckedIn, CheckedOut, Pinned
        // systems: ConnectionEstablisher, Handshaker

        let mut world = World::new();

        world.insert_resource(opts.clone());
        world.insert_resource(ConnectionEstablisher::new(
            EstablisherOptions::from_client_options(opts),
        )?);
        world.insert_resource(NextConnectionId(0));

        world.add_observer(|_: Trigger<OnAdd, CheckedIn>| {
            eprintln!("Connection checked in");
        });
        world.add_observer(|_: Trigger<OnAdd, CheckedOut>| {
            eprintln!("Connection checked out");
        });

        let check_out_connection = world.register_system(check_out_connection);
        let new_pending_connection = world.register_system(new_pending_connection);

        Ok(Self {
            world: Arc::new(tokio::sync::Mutex::new(world)),
            systems: Systems {
                check_out_connection,
                new_pending_connection,
            },
        })
    }

    async fn get_connection(&self) -> Result<CheckedOutId> {
        let mut world = self.world.lock().await;
        let entity =
            if let Some(entity) = world.run_system(self.systems.check_out_connection).unwrap() {
                entity
            } else {
                let pending = world
                    .run_system(self.systems.new_pending_connection)
                    .unwrap();
                let conn_establisher = world
                    .get_resource::<ConnectionEstablisher>()
                    .expect("ConnectionEstablisher not found");
                let conn = conn_establisher
                    .establish_connection_ecs(pending, None)
                    .await?;
                world.spawn((conn, CheckedOut)).id()
            };

        Ok(CheckedOutId {
            entity,
            world: Arc::clone(&self.world),
        })
    }

    /// Runs a command against the server.
    pub async fn run_command(&self, command: RawDocumentBuf) -> Result<Document> {
        let conn_id = self.get_connection().await?;
        let mut world = self.world.lock().await;
        let mut conn_ent = world.entity_mut(conn_id.entity);
        let mut conn = conn_ent.get_mut::<Connection>().unwrap();

        let message = Message {
            document_payload: command,
            document_sequences: vec![],
            response_to: 0,
            flags: crate::cmap::conn::wire::MessageFlags::empty(),
            checksum: None,
            request_id: None,
            #[cfg(any(
                feature = "zstd-compression",
                feature = "zlib-compression",
                feature = "snappy-compression"
            ))]
            should_compress: false,
        };
        Ok(conn.send_message(message).await?.body::<Document>()?)
    }
}

#[derive(Debug, Component)]
struct CheckedIn;

#[derive(Debug, Component)]
struct CheckedOut;

#[derive(Debug)]
struct CheckedOutId {
    entity: Entity,
    world: Arc<tokio::sync::Mutex<World>>,
}

impl Drop for CheckedOutId {
    fn drop(&mut self) {
        eprintln!("Dropping CheckedOutId");
        let entity = self.entity;
        let world = Arc::clone(&self.world);
        tokio::spawn(async move {
            let mut world = world.lock().await;
            world
                .entity_mut(entity)
                .remove::<CheckedOut>()
                .insert(CheckedIn);
        });
    }
}

fn check_out_connection(
    mut commands: Commands,
    query: Query<Entity, (With<Connection>, With<CheckedIn>)>,
) -> Option<Entity> {
    if let Some(entity) = query.iter().next() {
        commands
            .entity(entity)
            .remove::<CheckedIn>()
            .insert(CheckedOut);
        Some(entity)
    } else {
        None
    }
}

#[derive(Resource)]
struct NextConnectionId(u32);

fn new_pending_connection(
    mut next_id: ResMut<NextConnectionId>,
    opts: Res<ClientOptions>,
) -> PendingConnection {
    let id = next_id.0;
    next_id.0 += 1;
    let address = opts
        .hosts
        .get(0)
        .expect("No hosts found in ClientOptions")
        .clone();
    PendingConnection {
        id,
        address,
        generation: crate::cmap::PoolGeneration::Normal(0),
        event_emitter: CmapEventEmitter::new(None, ObjectId::new()),
        time_created: std::time::Instant::now(),
        cancellation_receiver: None,
    }
}

#[tokio::test]
async fn test_client() {
    use bson::rawdoc;

    let opts = ClientOptions::parse("mongodb://localhost:27017")
        .await
        .unwrap();

    let client = Client::new(&opts).await.unwrap();
    dbg!(client
        .run_command(rawdoc! {
            "hello": 1,
            "$db": "admin",
        })
        .await
        .unwrap());
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}
