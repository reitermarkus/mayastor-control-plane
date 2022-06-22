pub mod core;
/// Services to launch the grpc server
pub mod lib;
pub mod nexus;
pub mod node;
pub mod pool;
pub mod registry;
pub mod volume;
pub mod watcher;

use common_lib::types::v0::message_bus::ChannelVs;
use http::Uri;

use crate::core::registry::NumRebuilds;
use common_lib::mbus_api::BusClient;
use opentelemetry::{global, KeyValue};
use structopt::StructOpt;
use utils::{version_info_str, DEFAULT_GRPC_SERVER_ADDR};

#[derive(Debug, StructOpt)]
#[structopt(name = utils::package_description!(), version = version_info_str!())]
pub(crate) struct CliArgs {
    /// The Nats Server URL to connect to
    /// (supports the nats schema)
    #[structopt(long, short)]
    pub(crate) nats: Option<String>,

    /// The period at which the registry updates its cache of all
    /// resources from all nodes
    #[structopt(long, short, default_value = utils::CACHE_POLL_PERIOD)]
    pub(crate) cache_period: humantime::Duration,

    /// The period at which the reconcile loop checks for new work
    #[structopt(long, default_value = "30s")]
    pub(crate) reconcile_idle_period: humantime::Duration,

    /// The period at which the reconcile loop attempts to do work
    #[structopt(long, default_value = "10s")]
    pub(crate) reconcile_period: humantime::Duration,

    /// Deadline for the io-engine instance keep alive registration
    #[structopt(long, short, default_value = "10s")]
    pub(crate) deadline: humantime::Duration,

    /// The Persistent Store URLs to connect to
    /// (supports the http/https schema)
    #[structopt(long, short, default_value = "http://localhost:2379")]
    pub(crate) store: String,

    /// The timeout for store operations
    #[structopt(long, default_value = utils::STORE_OP_TIMEOUT)]
    pub(crate) store_timeout: humantime::Duration,

    /// The lease lock ttl for the persistent store after which we'll lose the exclusive access
    #[structopt(long, default_value = utils::STORE_LEASE_LOCK_TTL)]
    pub(crate) store_lease_ttl: humantime::Duration,

    /// The timeout for every node connection (gRPC)
    #[structopt(long, default_value = utils::DEFAULT_CONN_TIMEOUT)]
    pub(crate) connect_timeout: humantime::Duration,

    /// The default timeout for node request timeouts (gRPC)
    #[structopt(long, short, default_value = utils::DEFAULT_REQ_TIMEOUT)]
    pub(crate) request_timeout: humantime::Duration,

    /// Add process service tags to the traces
    #[structopt(short, long, env = "TRACING_TAGS", value_delimiter=",", parse(try_from_str = utils::tracing_telemetry::parse_key_value))]
    tracing_tags: Vec<KeyValue>,

    /// Don't use minimum timeouts for specific requests
    #[structopt(long)]
    no_min_timeouts: bool,
    /// Trace rest requests to the Jaeger endpoint agent
    #[structopt(long, short)]
    jaeger: Option<String>,
    /// The GRPC Server URLs to connect to
    /// (supports the http/https schema)
    #[structopt(long, short, default_value = DEFAULT_GRPC_SERVER_ADDR)]
    pub(crate) grpc_server_addr: Uri,
    /// The maximum number of system-wide rebuilds permitted at any given time.
    /// If `None` do not limit the number of rebuilds.
    #[structopt(long)]
    max_rebuilds: Option<NumRebuilds>,
}
impl CliArgs {
    fn args() -> Self {
        CliArgs::from_args()
    }
}

#[tokio::main]
async fn main() {
    let cli_args = CliArgs::args();
    utils::print_package_info!();
    println!("Using options: {:?}", &cli_args);
    utils::tracing_telemetry::init_tracing(
        "core-agent",
        cli_args.tracing_tags.clone(),
        cli_args.jaeger.clone(),
    );
    server(cli_args).await;
}

async fn server(cli_args: CliArgs) {
    common_lib::init_cluster_info_or_panic().await;
    let registry = core::registry::Registry::new(
        cli_args.cache_period.into(),
        cli_args.store.clone(),
        cli_args.store_timeout.into(),
        cli_args.store_lease_ttl.into(),
        cli_args.reconcile_period.into(),
        cli_args.reconcile_idle_period.into(),
        cli_args.max_rebuilds,
    )
    .await;

    let base_service = common::Service::builder(cli_args.nats.clone(), ChannelVs::Core)
        .with_shared_state(global::tracer_with_version(
            "core-agent",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_default_liveness()
        .connect_message_bus(cli_args.no_min_timeouts, BusClient::CoreAgent)
        .await
        .with_shared_state(registry.clone())
        .with_shared_state(cli_args.grpc_server_addr.clone())
        .configure_async(node::configure)
        .await
        .configure(pool::configure)
        .configure(nexus::configure)
        .configure(volume::configure)
        .configure(watcher::configure)
        .configure(registry::configure);

    let service = lib::Service::new(base_service);
    registry.start().await;
    service.run().await;
    registry.stop().await;
    opentelemetry::global::shutdown_tracer_provider();
}

/// Constructs a service handler for `RequestType` which gets redirected to a
/// Service Handler named `ServiceFnName`
#[macro_export]
macro_rules! impl_request_handler {
    ($RequestType:ident, $ServiceFnName:ident) => {
        /// Needed so we can implement the ServiceSubscriber trait for
        /// the message types external to the crate
        #[derive(Clone, Default)]
        struct ServiceHandler<T> {
            data: PhantomData<T>,
        }
        #[async_trait]
        impl common::ServiceSubscriber for ServiceHandler<$RequestType> {
            async fn handler(&self, args: common::Arguments<'_>) -> Result<(), SvcError> {
                #[tracing::instrument(skip(args), fields(result, error, request.service = true))]
                async fn $ServiceFnName(
                    args: common::Arguments<'_>,
                ) -> Result<<$RequestType as Message>::Reply, SvcError> {
                    let request: ReceivedMessage<$RequestType> = args.request.try_into()?;
                    let service: &service::Service = args.context.get_state()?;
                    match service.$ServiceFnName(&request.inner()).await {
                        Ok(reply) => {
                            if let Ok(result_str) = serde_json::to_string(&reply) {
                                if result_str.len() < 2048 {
                                    tracing::Span::current().record("result", &result_str.as_str());
                                }
                            }
                            tracing::Span::current().record("error", &false);
                            Ok(reply)
                        }
                        Err(error) => {
                            tracing::Span::current()
                                .record("result", &format!("{:?}", error).as_str());
                            tracing::Span::current().record("error", &true);
                            Err(error)
                        }
                    }
                }
                use opentelemetry::trace::FutureExt;
                match $ServiceFnName(args.clone())
                    .with_context(args.request.context())
                    .await
                {
                    Ok(reply) => Ok(args.request.respond(reply).await?),
                    Err(error) => Err(error),
                }
            }
            fn filter(&self) -> Vec<MessageId> {
                vec![$RequestType::default().id()]
            }
        }
    };
}

/// Constructs a service handler for `PublishType` which gets redirected to a
/// Service Handler named `ServiceFnName`
#[macro_export]
macro_rules! impl_publish_handler {
    ($PublishType:ident, $ServiceFnName:ident) => {
        /// Needed so we can implement the ServiceSubscriber trait for
        /// the message types external to the crate
        #[derive(Clone, Default)]
        struct ServiceHandler<T> {
            data: PhantomData<T>,
        }
        #[async_trait]
        impl common::ServiceSubscriber for ServiceHandler<$PublishType> {
            async fn handler(&self, args: common::Arguments<'_>) -> Result<(), SvcError> {
                let request: ReceivedMessage<$PublishType> = args.request.try_into()?;

                let service: &service::Service = args.context.get_state()?;
                service.$ServiceFnName(&request.inner()).await;
                Ok(())
            }
            fn filter(&self) -> Vec<MessageId> {
                vec![$PublishType::default().id()]
            }
        }
    };
}

/// Constructs and calls out to a service handler for `RequestType` which gets
/// redirected to a Service Handler where its name is either:
/// `RequestType` as a snake lowercase (default) or
/// `ServiceFn` parameter (if provided)
#[macro_export]
macro_rules! handler {
    ($RequestType:ident) => {{
        paste::paste! {
            impl_request_handler!(
                $RequestType,
                [<$RequestType:snake:lower>]
            );
        }
        ServiceHandler::<$RequestType>::default()
    }};
    ($RequestType:ident, $ServiceFn:ident) => {{
        paste::paste! {
            impl_request_handler!(
                $RequestType,
                $ServiceFn
            );
        }
        ServiceHandler::<$RequestType>::default()
    }};
}

/// Constructs and calls out to a service handler for `RequestType` which gets
/// redirected to a Service Handler where its name is either:
/// `RequestType` as a snake lowercase (default) or
/// `ServiceFn` parameter (if provided)
#[macro_export]
macro_rules! handler_publish {
    ($RequestType:ident) => {{
        paste::paste! {
            impl_publish_handler!(
                $RequestType,
                [<$RequestType:snake:lower>]
            );
        }
        ServiceHandler::<$RequestType>::default()
    }};
    ($RequestType:ident, $ServiceFn:ident) => {{
        paste::paste! {
            impl_publish_handler!(
                $RequestType,
                $ServiceFn
            );
        }
        ServiceHandler::<$RequestType>::default()
    }};
}
