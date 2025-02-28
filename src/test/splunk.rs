use std::{collections::HashMap, time::Duration};

use bson::RawDocumentBuf;

use crate::{
    hello::{HelloCommandResponse, HelloReply},
    options::ServerAddress,
    sdam::{ServerDescription, TopologyDescription, TransactionSupportStatus},
    ServerType,
    TopologyType,
};

#[test]
fn topology_update() {
    let server_addr = ServerAddress::Tcp {
        host: "::1".to_string(),
        port: Some(8191),
    };
    let server_desc = ServerDescription {
        address: server_addr.clone(),
        server_type: ServerType::Unknown,
        last_update_time: None,
        average_round_trip_time: None,
        reply: Ok(None),
    };
    let servers: HashMap<_, _> = [(server_addr.clone(), server_desc)].into_iter().collect();
    let mut desc = TopologyDescription {
        single_seed: true,
        topology_type: TopologyType::ReplicaSetNoPrimary,
        set_name: Some("C6B08298-1B8F-497F-8095-666C9B11BD1F".to_string()),
        max_set_version: None,
        max_election_id: None,
        compatibility_error: None,
        logical_session_timeout: None,
        transaction_support_status: TransactionSupportStatus::Supported,
        cluster_time: None,
        local_threshold: None,
        heartbeat_freq: None,
        servers,
        srv_max_hosts: None,
    };
    let hello_reply = HelloReply {
        server_address: server_addr.clone(),
        command_response: HelloCommandResponse {
            is_writable_primary: todo!(),
            is_master: todo!(),
            hello_ok: todo!(),
            hosts: todo!(),
            passives: todo!(),
            arbiters: todo!(),
            msg: todo!(),
            me: todo!(),
            compressors: todo!(),
            set_version: todo!(),
            set_name: todo!(),
            hidden: todo!(),
            secondary: todo!(),
            arbiter_only: todo!(),
            is_replica_set: todo!(),
            logical_session_timeout_minutes: todo!(),
            last_write: todo!(),
            min_wire_version: todo!(),
            max_wire_version: todo!(),
            tags: todo!(),
            election_id: todo!(),
            primary: todo!(),
            sasl_supported_mechs: todo!(),
            speculative_authenticate: todo!(),
            max_bson_object_size: todo!(),
            max_write_batch_size: todo!(),
            service_id: todo!(),
            topology_version: todo!(),
            max_message_size_bytes: todo!(),
            connection_id: todo!(),
        },
        raw_command_response: RawDocumentBuf::new(),
        cluster_time: None,
    };
    let avg_rtt: Duration = todo!();
    let server_description =
        ServerDescription::new_from_hello_reply(server_addr, hello_reply, avg_rtt);
    desc.update(server_description).unwrap();
}
