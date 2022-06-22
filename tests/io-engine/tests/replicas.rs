#![feature(allow_fail)]

use common_lib::types::v0::message_bus as v0;
use grpc::operations::replica::traits::ReplicaOperations;

use deployer_cluster::{result_either, test_result_grpc, ClusterBuilder};

// FIXME: CAS-721
#[tokio::test]
#[allow_fail]
async fn create_replica() {
    let cluster = ClusterBuilder::builder()
        .with_pools(1)
        // don't log whilst we have the allow_fail
        .compose_build(|c| c.with_logs(false))
        .await
        .unwrap();

    let rep_client = cluster.grpc_client().replica();

    let replica = v0::CreateReplica {
        node: cluster.node(0),
        uuid: Default::default(),
        pool: cluster.pool(0, 0),
        size: 5 * 1024 * 1024,
        thin: true,
        share: v0::Protocol::None,
        ..Default::default()
    };
    let created_replica = rep_client.create(&replica, None).await.unwrap();
    assert_eq!(created_replica.node, replica.node);
    assert_eq!(created_replica.uuid, replica.uuid);
    assert_eq!(created_replica.pool, replica.pool);

    // todo: why is this not the same?
    // assert_eq!(created_replica.size, replica.size);
    // fixme: replicas are always created without thin provisioning
    assert_eq!(created_replica.thin, replica.thin);
    assert_eq!(created_replica.share, replica.share);
}

#[tokio::test]
async fn create_replica_protocols() {
    let cluster = ClusterBuilder::builder()
        .with_pools(1)
        .build()
        .await
        .unwrap();

    let rep_client = cluster.grpc_client().replica();

    let protocols = vec![
        Err(v0::Protocol::Nbd),
        Err(v0::Protocol::Iscsi),
        Ok(v0::Protocol::Nvmf),
        Ok(v0::Protocol::None),
    ];

    for test in protocols {
        let protocol = result_either!(&test);
        test_result_grpc(
            &test,
            rep_client.create(
                &v0::CreateReplica {
                    node: cluster.node(0),
                    uuid: v0::ReplicaId::new(),
                    pool: cluster.pool(0, 0),
                    size: 5 * 1024 * 1024,
                    thin: true,
                    share: *protocol,
                    ..Default::default()
                },
                None,
            ),
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn create_replica_sizes() {
    let size = 100 * 1024 * 1024;
    let disk = format!("malloc:///disk?size_mb={}", size / (1024 * 1024));
    let cluster = ClusterBuilder::builder()
        .with_pool(0, &disk)
        .build()
        .await
        .unwrap();
    let rep_client = cluster.grpc_client().replica();
    let pool = cluster
        .rest_v00()
        .pools_api()
        .get_pool(cluster.pool(0, 0).as_str())
        .await
        .unwrap();
    let capacity = pool.state.unwrap().capacity;
    assert!(size > capacity && capacity > 0);
    let sizes = vec![Ok(capacity / 2), Ok(capacity), Err(capacity + 512)];
    for test in sizes {
        let size = result_either!(test);
        test_result_grpc(&test, async {
            let result = rep_client
                .create(
                    &v0::CreateReplica {
                        node: cluster.node(0),
                        uuid: v0::ReplicaId::new(),
                        pool: cluster.pool(0, 0),
                        size,
                        thin: false,
                        ..Default::default()
                    },
                    None,
                )
                .await;
            if let Ok(replica) = &result {
                rep_client
                    .destroy(&v0::DestroyReplica::from(replica.clone()), None)
                    .await
                    .unwrap();
            }
            result
        })
        .await
        .unwrap();
    }
}

// FIXME: CAS-731
#[tokio::test]
#[allow_fail]
async fn create_replica_idempotent_different_sizes() {
    let cluster = ClusterBuilder::builder()
        .with_pools(1)
        // don't log whilst we have the allow_fail
        .compose_build(|c| c.with_logs(false))
        .await
        .unwrap();
    let rep_client = cluster.grpc_client().replica();
    let uuid = v0::ReplicaId::new();
    let size = 5 * 1024 * 1024;
    let replica = rep_client
        .create(
            &v0::CreateReplica {
                node: cluster.node(0),
                uuid: uuid.clone(),
                pool: cluster.pool(0, 0),
                size,
                thin: false,
                share: v0::Protocol::None,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(&replica.uuid, &uuid);

    rep_client
        .create(
            &v0::CreateReplica {
                node: cluster.node(0),
                uuid: uuid.clone(),
                pool: cluster.pool(0, 0),
                size,
                thin: replica.thin,
                share: v0::Protocol::None,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();

    let sizes = vec![Ok(size), Err(size / 2), Err(size * 2)];
    for test in sizes {
        let size = result_either!(test);
        test_result_grpc(
            &test,
            rep_client.create(
                &v0::CreateReplica {
                    node: cluster.node(0),
                    uuid: replica.uuid.clone(),
                    pool: cluster.pool(0, 0),
                    size,
                    thin: replica.thin,
                    share: v0::Protocol::None,
                    ..Default::default()
                },
                None,
            ),
        )
        .await
        .unwrap();
    }
}

// FIXME: CAS-731
#[tokio::test]
#[allow_fail]
async fn create_replica_idempotent_different_protocols() {
    let cluster = ClusterBuilder::builder()
        .with_pools(1)
        // don't log whilst we have the allow_fail
        .compose_build(|c| c.with_logs(false))
        .await
        .unwrap();
    let rep_client = cluster.grpc_client().replica();
    let uuid = v0::ReplicaId::new();
    let size = 5 * 1024 * 1024;
    let replica = rep_client
        .create(
            &v0::CreateReplica {
                node: cluster.node(0),
                uuid: uuid.clone(),
                pool: cluster.pool(0, 0),
                size,
                thin: false,
                share: v0::Protocol::None,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(&replica.uuid, &uuid);

    let protocols = vec![
        Ok(v0::Protocol::None),
        Err(v0::Protocol::Iscsi),
        Err(v0::Protocol::Nvmf),
    ];
    for test in protocols {
        let protocol = result_either!(&test);
        test_result_grpc(
            &test,
            rep_client.create(
                &v0::CreateReplica {
                    node: cluster.node(0),
                    uuid: replica.uuid.clone(),
                    pool: replica.pool.clone(),
                    size: replica.size,
                    thin: replica.thin,
                    share: *protocol,
                    ..Default::default()
                },
                None,
            ),
        )
        .await
        .unwrap();
    }
}
