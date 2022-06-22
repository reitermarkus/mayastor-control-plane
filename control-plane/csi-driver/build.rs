extern crate tonic_build;

fn main() {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile(&["node/proto/csi.proto"], &["node/proto"])
        .expect("csi protobuf compilation failed");
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&["node/proto/ioenginenodeplugin.proto"], &["node/proto"])
        .expect("node grpc service protobuf compilation failed");
}
