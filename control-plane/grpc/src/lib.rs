pub mod client;
pub mod context;
pub mod misc;
/// All server, client implementations and the traits
pub mod operations;
pub mod tracing;

/// Common module for all the misc operations
pub(crate) mod common {
    tonic::include_proto!("v1.common");
}

/// Pool GRPC module for the autogenerated pool code
pub(crate) mod pool {
    tonic::include_proto!("v1.pool");
}

/// Replica GRPC module for the autogenerated replica code
pub(crate) mod replica {
    tonic::include_proto!("v1.replica");
}

/// Nexus GRPC module for the autogenerated pool code
/// The allow rule is to allow clippy to let the enum member names
/// to have same suffix and prefix
#[allow(clippy::enum_variant_names)]
pub(crate) mod nexus {
    tonic::include_proto!("v1.nexus");
}

/// Volume GRPC module for the autogenerated replica code
#[allow(clippy::large_enum_variant)]
pub(crate) mod volume {
    tonic::include_proto!("v1.volume");
}

/// Node GRPC module for the autogenerated node code
pub(crate) mod node {
    tonic::include_proto!("v1.node");
}

/// Blockdevice GRPC module for the autogenerated blockdevice code
pub(crate) mod blockdevice {
    tonic::include_proto!("v1.blockdevice");
}

/// Registry GRPC module for the autogenerated registry code
pub(crate) mod registry {
    tonic::include_proto!("v1.registry");
}

/// JsonGrpc GRPC module for the autogenerated jsongrpc code
pub(crate) mod jsongrpc {
    tonic::include_proto!("v1.jsongrpc");
}
