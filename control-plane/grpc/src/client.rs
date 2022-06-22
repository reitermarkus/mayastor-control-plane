use crate::{
    context::Context,
    operations::{
        nexus::{client::NexusClient, traits::NexusOperations},
        node::{client::NodeClient, traits::NodeOperations},
        pool::{client::PoolClient, traits::PoolOperations},
        registry::{client::RegistryClient, traits::RegistryOperations},
        replica::{client::ReplicaClient, traits::ReplicaOperations},
        volume::{client::VolumeClient, traits::VolumeOperations},
    },
};
use common_lib::mbus_api::TimeoutOptions;
use std::time::Duration;
use tonic::transport::Uri;

/// CoreClient encapsulates all the individual clients needed for gRPC transport
pub struct CoreClient {
    pool: PoolClient,
    replica: ReplicaClient,
    volume: VolumeClient,
    node: NodeClient,
    registry: RegistryClient,
    nexus: NexusClient,
}

impl CoreClient {
    /// generates a new CoreClient to get the individual clients
    pub async fn new<O: Into<Option<TimeoutOptions>>>(addr: Uri, opts: O) -> Self {
        let timeout_opts = opts.into();
        let pool_client = PoolClient::new(addr.clone(), timeout_opts.clone()).await;
        let replica_client = ReplicaClient::new(addr.clone(), timeout_opts.clone()).await;
        let volume_client = VolumeClient::new(addr.clone(), timeout_opts.clone()).await;
        let node_client = NodeClient::new(addr.clone(), timeout_opts.clone()).await;
        let registry_client = RegistryClient::new(addr.clone(), timeout_opts.clone()).await;
        let nexus_client = NexusClient::new(addr, timeout_opts).await;
        Self {
            pool: pool_client,
            replica: replica_client,
            volume: volume_client,
            node: node_client,
            registry: registry_client,
            nexus: nexus_client,
        }
    }
    /// retrieve the corresponding pool client
    pub fn pool(&self) -> impl PoolOperations {
        self.pool.clone()
    }
    /// retrieve the corresponding replica client
    pub fn replica(&self) -> impl ReplicaOperations {
        self.replica.clone()
    }
    /// retrieve the corresponding volume client
    pub fn volume(&self) -> impl VolumeOperations {
        self.volume.clone()
    }
    /// retrieve the corresponding node client
    pub fn node(&self) -> impl NodeOperations {
        self.node.clone()
    }
    /// retrieve the corresponding registry client
    pub fn registry(&self) -> impl RegistryOperations {
        self.registry.clone()
    }
    /// retrieve the corresponding nexus client
    pub fn nexus(&self) -> impl NexusOperations {
        self.nexus.clone()
    }
    /// Try to wait until the Core Agent is ready, up to a timeout, by using the Probe method.
    pub async fn wait_ready(&self, timeout_opts: Option<TimeoutOptions>) -> Result<(), ()> {
        let timeout_opts = match timeout_opts {
            Some(opts) => opts,
            None => TimeoutOptions::new()
                .with_timeout(Duration::from_millis(250))
                .with_max_retries(10),
        };
        for attempt in 1 ..= timeout_opts.max_retries().unwrap_or_default() {
            match self
                .volume
                .probe(Some(Context::new(Some(timeout_opts.clone()))))
                .await
            {
                Ok(true) => return Ok(()),
                _ => {
                    let delay = std::time::Duration::from_millis(100);
                    tracing::trace!(%attempt, delay=?delay, "Not available, retrying after...");
                    tokio::time::sleep(delay).await;
                }
            }
        }
        match self
            .volume
            .probe(Some(Context::new(Some(timeout_opts.clone()))))
            .await
        {
            Ok(true) => Ok(()),
            _ => {
                tracing::error!("Timed out");
                Err(())
            }
        }
    }
}
