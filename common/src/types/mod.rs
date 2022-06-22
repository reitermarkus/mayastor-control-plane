use crate::{
    mbus_api::{ReplyError, ReplyErrorKind},
    types::v0::{
        message_bus::ChannelVs,
        openapi::{
            actix::server::RestError,
            apis::StatusCode,
            models::{rest_json_error::Kind, RestJsonError},
        },
    },
};

use std::{fmt::Debug, str::FromStr};

pub mod v0;

/// Available Message Bus channels
#[derive(Clone, Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum Channel {
    /// Version 0 of the Channels
    v0(ChannelVs),
}

impl FromStr for Channel {
    type Err = strum::ParseError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        match source.split('/').next() {
            Some(version) => {
                let c: ChannelVs = source[version.len() + 1 ..].parse()?;
                Ok(Self::v0(c))
            }
            _ => Err(strum::ParseError::VariantNotFound),
        }
    }
}

impl ToString for Channel {
    fn to_string(&self) -> String {
        match self {
            Self::v0(channel) => format!("v0/{}", channel.to_string()),
        }
    }
}

impl Default for Channel {
    fn default() -> Self {
        Channel::v0(ChannelVs::Default)
    }
}

impl From<crate::mbus_api::Error> for RestError<openapi::models::RestJsonError> {
    fn from(src: crate::mbus_api::Error) -> Self {
        Self::from(ReplyError::from(src))
    }
}

impl From<ReplyError> for RestError<RestJsonError> {
    fn from(src: ReplyError) -> Self {
        let details = src.extra.clone();
        let message = src.source.clone();
        let (status, error) = match &src.kind {
            ReplyErrorKind::WithMessage => {
                let error = RestJsonError::new(details, message, Kind::Internal);
                (StatusCode::INTERNAL_SERVER_ERROR, error)
            }
            ReplyErrorKind::DeserializeReq => {
                let error = RestJsonError::new(details, message, Kind::Deserialize);
                (StatusCode::BAD_REQUEST, error)
            }
            ReplyErrorKind::Internal => {
                let error = RestJsonError::new(details, message, Kind::Internal);
                (StatusCode::INTERNAL_SERVER_ERROR, error)
            }
            ReplyErrorKind::Timeout => {
                let error = RestJsonError::new(details, message, Kind::Timeout);
                (StatusCode::REQUEST_TIMEOUT, error)
            }
            ReplyErrorKind::InvalidArgument => {
                let error = RestJsonError::new(details, message, Kind::InvalidArgument);
                (StatusCode::BAD_REQUEST, error)
            }
            ReplyErrorKind::DeadlineExceeded => {
                let error = RestJsonError::new(details, message, Kind::DeadlineExceeded);
                (StatusCode::GATEWAY_TIMEOUT, error)
            }
            ReplyErrorKind::NotFound => {
                let error = RestJsonError::new(details, message, Kind::NotFound);
                (StatusCode::NOT_FOUND, error)
            }
            ReplyErrorKind::AlreadyExists => {
                let error = RestJsonError::new(details, message, Kind::AlreadyExists);
                (StatusCode::UNPROCESSABLE_ENTITY, error)
            }
            ReplyErrorKind::PermissionDenied => {
                let error = RestJsonError::new(details, message, Kind::PermissionDenied);
                (StatusCode::UNAUTHORIZED, error)
            }
            ReplyErrorKind::ResourceExhausted => {
                let error = RestJsonError::new(details, message, Kind::ResourceExhausted);
                (StatusCode::INSUFFICIENT_STORAGE, error)
            }
            ReplyErrorKind::FailedPrecondition => {
                let error = RestJsonError::new(details, message, Kind::FailedPrecondition);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::Aborted => {
                let error = RestJsonError::new(details, message, Kind::Aborted);
                (StatusCode::SERVICE_UNAVAILABLE, error)
            }
            ReplyErrorKind::OutOfRange => {
                let error = RestJsonError::new(details, message, Kind::OutOfRange);
                (StatusCode::RANGE_NOT_SATISFIABLE, error)
            }
            ReplyErrorKind::Unimplemented => {
                let error = RestJsonError::new(details, message, Kind::Unimplemented);
                (StatusCode::NOT_IMPLEMENTED, error)
            }
            ReplyErrorKind::Unavailable => {
                let error = RestJsonError::new(details, message, Kind::Unavailable);
                (StatusCode::SERVICE_UNAVAILABLE, error)
            }
            ReplyErrorKind::Unauthenticated => {
                let error = RestJsonError::new(details, message, Kind::Unauthenticated);
                (StatusCode::UNAUTHORIZED, error)
            }
            ReplyErrorKind::Unauthorized => {
                let error = RestJsonError::new(details, message, Kind::Unauthorized);
                (StatusCode::UNAUTHORIZED, error)
            }
            ReplyErrorKind::Conflict => {
                let error = RestJsonError::new(details, message, Kind::Conflict);
                (StatusCode::CONFLICT, error)
            }
            ReplyErrorKind::FailedPersist => {
                let error = RestJsonError::new(details, message, Kind::FailedPersist);
                (StatusCode::INSUFFICIENT_STORAGE, error)
            }
            ReplyErrorKind::AlreadyShared => {
                let error = RestJsonError::new(details, message, Kind::AlreadyShared);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::NotShared => {
                let error = RestJsonError::new(details, message, Kind::NotShared);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::NotPublished => {
                let error = RestJsonError::new(details, message, Kind::NotPublished);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::AlreadyPublished => {
                let error = RestJsonError::new(details, message, Kind::AlreadyPublished);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::Deleting => {
                let error = RestJsonError::new(details, message, Kind::Deleting);
                (StatusCode::CONFLICT, error)
            }
            ReplyErrorKind::ReplicaCountAchieved => {
                let error = RestJsonError::new(details, message, Kind::FailedPrecondition);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::ReplicaChangeCount => {
                let error = RestJsonError::new(details, message, Kind::FailedPrecondition);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::ReplicaIncrease => {
                let error = RestJsonError::new(details, message, Kind::FailedPrecondition);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::VolumeNoReplicas => {
                let error = RestJsonError::new(details, message, Kind::FailedPrecondition);
                (StatusCode::PRECONDITION_FAILED, error)
            }
            ReplyErrorKind::InUse => {
                let error = RestJsonError::new(details, message, Kind::InUse);
                (StatusCode::CONFLICT, error)
            }
            ReplyErrorKind::ReplicaCreateNumber => {
                let error = RestJsonError::new(details, message, Kind::FailedPrecondition);
                (StatusCode::PRECONDITION_FAILED, error)
            }
        };

        RestError::new(status, error)
    }
}
