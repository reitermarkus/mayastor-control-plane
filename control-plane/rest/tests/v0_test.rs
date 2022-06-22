use common_lib::types::v0::{
    message_bus::WatchResourceId,
    openapi::{apis, models},
};

use rest_client::RestClient;

use common_lib::types::v0::{
    message_bus::{NexusId, NodeId, ReplicaId, VolumeId},
    openapi::{
        client::direct::ApiClient,
        clients::tower::{Error, ResponseError},
    },
};
use deployer_cluster::{Cluster, ClusterBuilder};
use std::{
    convert::{TryFrom, TryInto},
    str::FromStr,
    time::Duration,
};
use tracing::info;

// Returns the path to the JWK file.
fn jwk_file() -> String {
    let jwk_file = std::env::current_dir()
        .unwrap()
        .join("authentication")
        .join("jwk");
    jwk_file.to_str().unwrap().into()
}

// Setup the infrastructure ready for the tests.
async fn test_setup(auth: &bool) -> Cluster {
    let rest_jwk = match auth {
        true => Some(jwk_file()),
        false => None,
    };

    let cluster = ClusterBuilder::builder()
        .with_rest_auth(true, rest_jwk)
        .with_options(|o| o.with_jaeger(true))
        .with_agents(vec!["core", "jsongrpc"])
        .with_io_engines(2)
        .with_cache_period("1s")
        .with_reconcile_period(Duration::from_secs(1), Duration::from_secs(1))
        .build()
        .await
        .unwrap();

    cluster
}

// Return a bearer token to be sent with REST requests.
fn bearer_token() -> String {
    let token_file = std::env::current_dir()
        .expect("Failed to get current directory")
        .join("authentication")
        .join("token");
    std::fs::read_to_string(token_file).expect("Failed to get bearer token")
}

#[tokio::test]
async fn client() {
    // Run the client test both with and without authentication.
    for auth in &[false, true] {
        let cluster = test_setup(auth).await;
        client_test(&cluster, auth).await;
    }
}

async fn client_test(cluster: &Cluster, auth: &bool) {
    let test = cluster.composer();
    let client = RestClient::new(
        "https://localhost:8080",
        true,
        match auth {
            true => Some(bearer_token()),
            false => None,
        },
    )
    .unwrap()
    .v00();

    let nodes = client.nodes_api().get_nodes().await.unwrap();
    info!("Nodes: {:#?}", nodes);
    assert_eq!(nodes.len(), 2);
    let io_engine1 = cluster.node(0);
    let io_engine2 = cluster.node(1);

    let listed_node = client.nodes_api().get_node(io_engine1.as_str()).await;
    let mut node = models::Node {
        id: io_engine1.to_string(),
        spec: Some(models::NodeSpec {
            id: io_engine1.to_string(),
            grpc_endpoint: format!(
                "{}:10124",
                cluster.composer().container_ip(cluster.node(0).as_str())
            ),
        }),
        state: Some(models::NodeState {
            id: io_engine1.to_string(),
            grpc_endpoint: format!(
                "{}:10124",
                cluster.composer().container_ip(cluster.node(0).as_str())
            ),
            status: models::NodeStatus::Online,
        }),
    };
    assert_eq!(listed_node.unwrap(), node);

    let _ = client.pools_api().get_pools().await.unwrap();
    let pool = client
        .pools_api()
        .put_node_pool(
            io_engine1.as_str(),
            "pooloop",
            models::CreatePoolBody::new(vec![
                "malloc:///malloc0?blk_size=512&size_mb=100&uuid=b940f4f2-d45d-4404-8167-3b0366f9e2b0",
            ]),
        )
        .await
        .unwrap();

    info!("Pools: {:#?}", pool);
    assert_eq!(
        pool,
        models::Pool::new_all(
            "pooloop",
            models::PoolSpec::new(vec!["malloc:///malloc0?blk_size=512&size_mb=100&uuid=b940f4f2-d45d-4404-8167-3b0366f9e2b0"], "pooloop", &io_engine1, models::SpecStatus::Created),
            models::PoolState::new(100663296u64, vec!["malloc:///malloc0?blk_size=512&size_mb=100&uuid=b940f4f2-d45d-4404-8167-3b0366f9e2b0"], "pooloop", &io_engine1, models::PoolStatus::Online, 0u64)
        )
    );

    assert_eq!(
        Some(&pool),
        client.pools_api().get_pools().await.unwrap().first()
    );

    let pool = client
        .pools_api()
        .put_node_pool(
            io_engine2.as_str(),
            "pooloop2",
            models::CreatePoolBody::new(vec![
                "malloc:///malloc0?blk_size=512&size_mb=100&uuid=b940f4f2-d45d-4404-8167-3b0366f9e2b1",
            ]),
        )
        .await
        .unwrap();

    info!("Pools: {:#?}", pool);

    let _ = client.replicas_api().get_replicas().await.unwrap();
    let replica = client
        .replicas_api()
        .put_node_pool_replica(
            &pool.spec.as_ref().unwrap().node,
            &pool.id,
            &ReplicaId::try_from("e6e7d39d-e343-42f7-936a-1ab05f1839db").unwrap(),
            /* actual size will be a multiple of 4MB so just
             * create it like so */
            models::CreateReplicaBody::new_all(
                models::ReplicaShareProtocol::Nvmf,
                12582912u64,
                false,
            ),
        )
        .await
        .unwrap();
    info!("Replica: {:#?}", replica);

    let uri = replica.uri.clone();
    assert_eq!(
        replica,
        models::Replica {
            node: pool.spec.clone().unwrap().node,
            uuid: FromStr::from_str("e6e7d39d-e343-42f7-936a-1ab05f1839db").unwrap(),
            pool: pool.id.clone(),
            thin: false,
            size: 12582912,
            share: models::Protocol::Nvmf,
            uri,
            state: models::ReplicaState::Online
        }
    );
    assert_eq!(
        Some(&replica),
        client.replicas_api().get_replicas().await.unwrap().first()
    );
    client
        .replicas_api()
        .del_node_pool_replica(&replica.node, &replica.pool, &replica.uuid)
        .await
        .unwrap();

    let replicas = client.replicas_api().get_replicas().await.unwrap();
    assert!(replicas.is_empty());

    let nexuses = client.nexuses_api().get_nexuses().await.unwrap();
    assert_eq!(nexuses.len(), 0);
    let nexus = client
        .nexuses_api()
        .put_node_nexus(
            io_engine1.as_str(),
            &NexusId::try_from("e6e7d39d-e343-42f7-936a-1ab05f1839db").unwrap(),
            models::CreateNexusBody::new(
                vec!["malloc:///malloc1?blk_size=512&size_mb=100&uuid=b940f4f2-d45d-4404-8167-3b0366f9e2b0"],
                12582912u64
            ),
        )
        .await
        .unwrap();
    info!("Nexus: {:#?}", nexus);

    assert_eq!(
        nexus,
        models::Nexus {
            node: io_engine1.to_string(),
            uuid: NexusId::try_from("e6e7d39d-e343-42f7-936a-1ab05f1839db").unwrap().into(),
            size: 12582912,
            state: models::NexusState::Online,
            children: vec![models::Child {
                uri: "malloc:///malloc1?blk_size=512&size_mb=100&uuid=b940f4f2-d45d-4404-8167-3b0366f9e2b0".into(),
                state: models::ChildState::Online,
                rebuild_progress: None
            }],
            device_uri: "".to_string(),
            rebuilds: 0,
            protocol: models::Protocol::None
        }
    );

    let child = client
        .children_api()
        .put_node_nexus_child(
            &nexus.node,
            &nexus.uuid,
            "malloc:///malloc2?blk_size=512&size_mb=100&uuid=b940f4f2-d45d-4404-8167-3b0366f9e2b1",
        )
        .await
        .unwrap();

    let children = client
        .children_api()
        .get_nexus_children(&nexus.uuid)
        .await
        .unwrap();
    let child_updated = children.iter().find(|c| c.uri == child.uri);

    // It's possible that the rebuild progress will change between putting a child and getting the
    // list of children so don't bother comparing the states
    assert!(child.rebuild_progress.is_some());
    assert!(child_updated.is_some());
    assert!(
        child_updated.unwrap().rebuild_progress.is_some()
            || child_updated.unwrap().state == models::ChildState::Online
    );

    client
        .nexuses_api()
        .del_node_nexus(&nexus.node, &nexus.uuid)
        .await
        .unwrap();
    let nexuses = client.nexuses_api().get_nexuses().await.unwrap();
    assert!(nexuses.is_empty());
    let volume_uuid: VolumeId = "058a95e5-cee6-4e81-b682-fe864ca99b9c".try_into().unwrap();

    let volume = client
        .volumes_api()
        .put_volume(
            &volume_uuid,
            models::CreateVolumeBody::new(models::VolumePolicy::default(), 1, 12582912u64),
        )
        .await
        .unwrap();

    tracing::info!("Volume: {:#?}", volume);
    assert_eq!(
        volume,
        client.volumes_api().get_volume(&volume_uuid).await.unwrap()
    );

    let volume = client
        .volumes_api()
        .put_volume_target(
            &volume.state.uuid,
            io_engine1.as_str(),
            models::VolumeShareProtocol::Nvmf,
        )
        .await
        .unwrap();
    let volume_state = volume.state;
    let nexus = volume_state.target.unwrap();
    tracing::info!("Published on '{}'", nexus.node);

    let volume = client
        .volumes_api()
        .put_volume_replica_count(&volume_state.uuid, 2)
        .await
        .expect("We have 2 nodes with a pool each");
    tracing::info!("Volume: {:#?}", volume);
    let volume_state = volume.state;
    let nexus = volume_state.target.unwrap();
    assert_eq!(nexus.children.len(), 2);

    let volume = client
        .volumes_api()
        .put_volume_replica_count(&volume_state.uuid, 1)
        .await
        .expect("Should be able to reduce back to 1");
    tracing::info!("Volume: {:#?}", volume);
    let volume_state = volume.state;
    let nexus = volume_state.target.unwrap();
    assert_eq!(nexus.children.len(), 1);

    let volume = client
        .volumes_api()
        .del_volume_target(&volume_state.uuid, None)
        .await
        .unwrap();
    tracing::info!("Volume: {:#?}", volume);
    let volume_state = volume.state;
    assert!(volume_state.target.is_none());

    let volume_uuid = volume_state.uuid;

    let _watch_volume = WatchResourceId::Volume(volume_uuid.into());
    let callback = url::Url::parse("http://lala/test").unwrap();

    let watchers = client
        .watches_api()
        .get_watch_volume(&volume_uuid)
        .await
        .unwrap();
    assert!(watchers.is_empty());

    client
        .watches_api()
        .put_watch_volume(&volume_uuid, &callback.to_string())
        .await
        .expect_err("volume does not exist in the store");

    client
        .watches_api()
        .del_watch_volume(&volume_uuid, &callback.to_string())
        .await
        .expect_err("Does not exist");

    let watchers = client
        .watches_api()
        .get_watch_volume(&volume_uuid)
        .await
        .unwrap();
    assert!(watchers.is_empty());

    client.volumes_api().del_volume(&volume_uuid).await.unwrap();

    let volumes = client.volumes_api().get_volumes(0, None).await.unwrap();
    assert!(volumes.entries.is_empty());

    client
        .pools_api()
        .del_node_pool(&pool.spec.as_ref().unwrap().node, &pool.id)
        .await
        .unwrap();
    let pools = client
        .pools_api()
        .get_node_pools(&pool.spec.as_ref().unwrap().node)
        .await
        .unwrap();
    assert!(pools.is_empty());

    client
        .json_grpc_api()
        .put_node_jsongrpc(
            io_engine1.as_str(),
            "rpc_get_methods",
            serde_json::json!({}),
        )
        .await
        .expect("Failed to call JSON gRPC method");

    client
        .block_devices_api()
        .get_node_block_devices(io_engine1.as_str(), Some(false))
        .await
        .expect("Failed to get block devices");

    test.stop("io-engine-1").await.unwrap();
    wait_until_node_not_online(&client, &io_engine1, Duration::from_secs(1)).await;
    node.state.as_mut().unwrap().status = models::NodeStatus::Unknown;
    assert_eq!(
        client
            .nodes_api()
            .get_node(io_engine1.as_str())
            .await
            .unwrap(),
        node
    );
}

async fn wait_until_node_not_online(client: &ApiClient, node: &NodeId, timeout: Duration) {
    let start = std::time::Instant::now();
    loop {
        let node = client.nodes_api().get_node(node.as_str()).await.unwrap();

        match node.state {
            Some(state) if state.status == models::NodeStatus::Online => {
                if std::time::Instant::now() > (start + timeout) {
                    let message = format!(
                        "Timeout waiting for the node not to have state: '{:#?}'. Actual: '{:#?}'",
                        models::NodeStatus::Online,
                        state.status
                    );
                    tracing::error!(%message);
                    panic!("{}", message);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            _ => break,
        };
    }
}

#[tokio::test]
async fn client_invalid_token() {
    let _cluster = test_setup(&true).await;

    // Use an invalid token to make requests.
    let mut token = bearer_token();
    token.push_str("invalid");

    let client = RestClient::new("https://localhost:8080", true, Some(token))
        .unwrap()
        .v00();

    let error = client
        .nodes_api()
        .get_nodes()
        .await
        .expect_err("Request should fail with invalid token");

    let unauthorized = match error {
        Error::Response(ResponseError::Expected(r)) => r.status() == apis::StatusCode::UNAUTHORIZED,
        _ => false,
    };
    assert!(unauthorized);
}
