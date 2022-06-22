use crate::{
    context::{Client, Context, TracedChannel},
    operations::registry::traits::{GetSpecsInfo, RegistryOperations},
    registry::{get_specs_reply, registry_grpc_client::RegistryGrpcClient},
};
use common_lib::{
    mbus_api::{ReplyError, ResourceKind, TimeoutOptions},
    types::v0::message_bus::{MessageIdVs, Specs},
};
use std::{convert::TryFrom, ops::Deref};
use tonic::transport::Uri;

/// RPC Registry Client
#[derive(Clone)]
pub struct RegistryClient {
    inner: Client<RegistryGrpcClient<TracedChannel>>,
}
impl Deref for RegistryClient {
    type Target = Client<RegistryGrpcClient<TracedChannel>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl RegistryClient {
    /// creates a new base tonic endpoint with the timeout options and the address
    pub async fn new<O: Into<Option<TimeoutOptions>>>(addr: Uri, opts: O) -> Self {
        let client = Client::new(addr, opts, RegistryGrpcClient::new).await;
        Self { inner: client }
    }
}
/// Implement registry operations supported by the Registry RPC client.
/// This converts the client side data into a RPC request.
#[tonic::async_trait]
impl RegistryOperations for RegistryClient {
    async fn get_specs(
        &self,
        request: &dyn GetSpecsInfo,
        ctx: Option<Context>,
    ) -> Result<Specs, ReplyError> {
        let req = self.request(request, ctx, MessageIdVs::GetSpecs);
        let response = self.client().get_specs(req).await?.into_inner();
        match response.reply {
            Some(get_specs_reply) => match get_specs_reply {
                get_specs_reply::Reply::Specs(specs) => Ok(Specs::try_from(specs)?),
                get_specs_reply::Reply::Error(err) => Err(err.into()),
            },
            None => Err(ReplyError::invalid_response(ResourceKind::Spec)),
        }
    }
}
