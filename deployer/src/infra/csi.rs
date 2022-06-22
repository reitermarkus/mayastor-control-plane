use super::*;
use tokio::{
    net::UnixStream,
    time::{sleep, Duration},
};
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;

use rpc::csi::{identity_client::IdentityClient, GetPluginInfoRequest};

const CSI_SOCKET: &str = "/var/tmp/csi.sock";

#[async_trait]
impl ComponentAction for Csi {
    fn configure(&self, options: &StartOptions, cfg: Builder) -> Result<Builder, Error> {
        Ok(if !options.csi {
            cfg
        } else {
            if options.build {
                std::process::Command::new("cargo")
                    .args(&["build", "-p", "csi-controller", "--bin", "csi-controller"])
                    .status()?;
            }

            let mut binary = Binary::from_dbg("csi-controller")
                .with_args(vec!["--rest-endpoint", "http://rest:8081"])
                // Make sure that CSI socket is always under shared directory
                // regardless of what its default value is.
                .with_args(vec!["--csi-socket", CSI_SOCKET]);

            if cfg.container_exists("jaeger") {
                let jaeger_config = format!("jaeger.{}:6831", cfg.get_name());
                binary = binary.with_args(vec!["--jaeger", &jaeger_config])
            };

            if let Some(size) = &options.otel_max_batch_size {
                binary = binary.with_env("OTEL_BSP_MAX_EXPORT_BATCH_SIZE", size);
            }

            cfg.add_container_spec(
                ContainerSpec::from_binary("csi-controller", binary)
                    .with_bypass_default_mounts(true)
                    .with_bind("/var/tmp", "/var/tmp"),
            )
        })
    }
    async fn start(&self, options: &StartOptions, cfg: &ComposeTest) -> Result<(), Error> {
        if options.csi {
            cfg.start("csi-controller").await?;
        }
        Ok(())
    }

    async fn wait_on(&self, options: &StartOptions, _cfg: &ComposeTest) -> Result<(), Error> {
        if !options.csi {
            return Ok(());
        }

        // Step 1: Wait till CSI controller's gRPC server is registered and is ready
        // to serve API requests.
        let endpoint =
            Endpoint::try_from("http://[::]:50051")?.connect_timeout(Duration::from_millis(100));
        let channel = loop {
            match endpoint
                .connect_with_connector(service_fn(|_: Uri| UnixStream::connect(CSI_SOCKET)))
                .await
            {
                Ok(channel) => break channel,
                Err(_) => sleep(Duration::from_secs(1)).await,
            }
        };

        let mut client = IdentityClient::new(channel);

        // Step 2: Make sure we can perform a successful RPC call.
        loop {
            match client
                .get_plugin_info(GetPluginInfoRequest::default())
                .await
            {
                Ok(_) => break,
                Err(_) => sleep(Duration::from_millis(100)).await,
            }
        }

        Ok(())
    }
}
