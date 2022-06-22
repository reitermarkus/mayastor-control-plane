## About

This crate contains CSI protocol implementation, the part that implements
`identity` and `node` grpc CSI services. It is an asynchronous server
implementation making use of tokio.rs.

The io engine node csi plugin implements two gRPC services:

* identity CSI service
* node CSI service

## Building and running the gRPC server

After `cargo build` the binary will be in `target/debug` directory.
To get a debug output just from the server and not other libraries (i.e.
tokio, tower-grpc, etc.), run it as follows:

```bash
RUST_LOG=csi_node=trace ./target/debug/csi-node
```

# CSI

CSI methods can be tested by official csc tool written in golang. Assuming that golang
is installed and GOPATH set, csc tool can be installed as:

```bash
go get github.com/rexray/gocsi && make -C $GOPATH/src/github.com/rexray/gocsi csi-sp
```

Now assuming that csi-node server is running and for example we want to
invoke a probe method from csi identity service, we can type:

```bash
$GOPATH/src/github.com/rexray/gocsi/csc/csc -i -e unix:///var/tmp/csi.sock identity probe
```
