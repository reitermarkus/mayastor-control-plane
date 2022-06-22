use crate::{
    common::VolumeFilter,
    context::{Client, Context, TracedChannel},
    operations::{
        volume::traits::{
            CreateVolumeInfo, DestroyVolumeInfo, PublishVolumeInfo, SetVolumeReplicaInfo,
            ShareVolumeInfo, UnpublishVolumeInfo, UnshareVolumeInfo, VolumeOperations,
        },
        Pagination,
    },
    volume::{
        create_volume_reply, get_volumes_reply, get_volumes_request, publish_volume_reply,
        set_volume_replica_reply, share_volume_reply, unpublish_volume_reply,
        volume_grpc_client::VolumeGrpcClient, GetVolumesRequest, ProbeRequest,
    },
};
use common_lib::{
    mbus_api::{v0::Volumes, ReplyError, ResourceKind, TimeoutOptions},
    types::v0::message_bus::{Filter, MessageIdVs, Volume},
};
use std::{convert::TryFrom, ops::Deref};
use tonic::transport::Uri;

/// RPC Volume Client
#[derive(Clone)]
pub struct VolumeClient {
    inner: Client<VolumeGrpcClient<TracedChannel>>,
}

impl VolumeClient {
    /// creates a new base tonic endpoint with the timeout options and the address
    pub async fn new<O: Into<Option<TimeoutOptions>>>(addr: Uri, opts: O) -> Self {
        let client = Client::new(addr, opts, VolumeGrpcClient::new).await;
        Self { inner: client }
    }
}

impl Deref for VolumeClient {
    type Target = Client<VolumeGrpcClient<TracedChannel>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Implement volume operations supported by the Volume RPC client.
/// This converts the client side data into a RPC request.
#[tonic::async_trait]
impl VolumeOperations for VolumeClient {
    #[tracing::instrument(name = "VolumeClient::create", level = "debug", skip(self), err)]
    async fn create(
        &self,
        request: &dyn CreateVolumeInfo,
        ctx: Option<Context>,
    ) -> Result<Volume, ReplyError> {
        let req = self.request(request, ctx, MessageIdVs::CreateVolume);
        let response = self.client().create_volume(req).await?.into_inner();
        match response.reply {
            Some(create_volume_reply) => match create_volume_reply {
                create_volume_reply::Reply::Volume(volume) => Ok(Volume::try_from(volume)?),
                create_volume_reply::Reply::Error(err) => Err(err.into()),
            },
            None => Err(ReplyError::invalid_response(ResourceKind::Volume)),
        }
    }

    #[tracing::instrument(name = "VolumeClient::get", level = "debug", skip(self), err)]
    async fn get(
        &self,
        filter: Filter,
        pagination: Option<Pagination>,
        ctx: Option<Context>,
    ) -> Result<Volumes, ReplyError> {
        let req: GetVolumesRequest = match filter {
            Filter::Volume(volume_id) => GetVolumesRequest {
                filter: Some(get_volumes_request::Filter::Volume(VolumeFilter {
                    volume_id: volume_id.to_string(),
                })),
                pagination: pagination.map(|p| p.into()),
            },
            _ => GetVolumesRequest {
                filter: None,
                pagination: pagination.map(|p| p.into()),
            },
        };
        let req = self.request(req, ctx, MessageIdVs::GetVolumes);
        let response = self.client().get_volumes(req).await?.into_inner();
        match response.reply {
            Some(get_volumes_reply) => match get_volumes_reply {
                get_volumes_reply::Reply::Volumes(volumes) => Ok(Volumes::try_from(volumes)?),
                get_volumes_reply::Reply::Error(err) => Err(err.into()),
            },
            None => Err(ReplyError::invalid_response(ResourceKind::Volume)),
        }
    }

    #[tracing::instrument(name = "VolumeClient::destroy", level = "debug", skip(self), err)]
    async fn destroy(
        &self,
        request: &dyn DestroyVolumeInfo,
        ctx: Option<Context>,
    ) -> Result<(), ReplyError> {
        let req = self.request(request, ctx, MessageIdVs::DestroyVolume);
        let response = self.client().destroy_volume(req).await?.into_inner();
        match response.error {
            None => Ok(()),
            Some(err) => Err(err.into()),
        }
    }

    #[tracing::instrument(name = "VolumeClient::share", level = "debug", skip(self), err)]
    async fn share(
        &self,
        request: &dyn ShareVolumeInfo,
        ctx: Option<Context>,
    ) -> Result<String, ReplyError> {
        let req = self.request(request, ctx, MessageIdVs::ShareVolume);
        let response = self.client().share_volume(req).await?.into_inner();
        match response.reply {
            Some(share_volume_reply) => match share_volume_reply {
                share_volume_reply::Reply::Response(message) => Ok(message),
                share_volume_reply::Reply::Error(err) => Err(err.into()),
            },
            None => Err(ReplyError::invalid_response(ResourceKind::Volume)),
        }
    }

    #[tracing::instrument(name = "VolumeClient::unshare", level = "debug", skip(self), err)]
    async fn unshare(
        &self,
        request: &dyn UnshareVolumeInfo,
        ctx: Option<Context>,
    ) -> Result<(), ReplyError> {
        let req = self.request(request, ctx, MessageIdVs::UnshareVolume);
        let response = self.client().unshare_volume(req).await?.into_inner();
        match response.error {
            None => Ok(()),
            Some(err) => Err(err.into()),
        }
    }

    #[tracing::instrument(name = "VolumeClient::publish", level = "debug", skip(self), err)]
    async fn publish(
        &self,
        request: &dyn PublishVolumeInfo,
        ctx: Option<Context>,
    ) -> Result<Volume, ReplyError> {
        let req = self.request(request, ctx, MessageIdVs::PublishVolume);
        let response = self.client().publish_volume(req).await?.into_inner();
        match response.reply {
            Some(publish_volume_reply) => match publish_volume_reply {
                publish_volume_reply::Reply::Volume(volume) => Ok(Volume::try_from(volume)?),
                publish_volume_reply::Reply::Error(err) => Err(err.into()),
            },
            None => Err(ReplyError::invalid_response(ResourceKind::Volume)),
        }
    }

    #[tracing::instrument(name = "VolumeClient::unpublish", level = "debug", skip(self), err)]
    async fn unpublish(
        &self,
        request: &dyn UnpublishVolumeInfo,
        ctx: Option<Context>,
    ) -> Result<Volume, ReplyError> {
        let req = self.request(request, ctx, MessageIdVs::UnpublishVolume);
        let response = self.client().unpublish_volume(req).await?.into_inner();
        match response.reply {
            Some(unpublish_volume_reply) => match unpublish_volume_reply {
                unpublish_volume_reply::Reply::Volume(volume) => Ok(Volume::try_from(volume)?),
                unpublish_volume_reply::Reply::Error(err) => Err(err.into()),
            },
            None => Err(ReplyError::invalid_response(ResourceKind::Volume)),
        }
    }

    #[tracing::instrument(name = "VolumeClient::set_replica", level = "debug", skip(self), err)]
    async fn set_replica(
        &self,
        request: &dyn SetVolumeReplicaInfo,
        ctx: Option<Context>,
    ) -> Result<Volume, ReplyError> {
        let req = self.request(request, ctx, MessageIdVs::SetVolumeReplica);
        let response = self.client().set_volume_replica(req).await?.into_inner();
        match response.reply {
            Some(set_volume_replica_reply) => match set_volume_replica_reply {
                set_volume_replica_reply::Reply::Volume(volume) => Ok(Volume::try_from(volume)?),
                set_volume_replica_reply::Reply::Error(err) => Err(err.into()),
            },
            None => Err(ReplyError::invalid_response(ResourceKind::Volume)),
        }
    }

    #[tracing::instrument(name = "VolumeClient::probe", level = "debug", skip(self))]
    async fn probe(&self, _ctx: Option<Context>) -> Result<bool, ReplyError> {
        match self.client().probe(ProbeRequest {}).await {
            Ok(resp) => Ok(resp.into_inner().ready),
            Err(e) => Err(e.into()),
        }
    }
}
