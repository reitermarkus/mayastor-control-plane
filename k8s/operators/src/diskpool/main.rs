//! K8S pool operator watches for pool CRs and creates the pool on the given node.
//! There is a maximum retry limit that will put the pool into a steady error state.
//!
//! Successfully created pools are recreated by the control plane.

mod crd;

use chrono::Utc;
use clap::{App, Arg, ArgMatches};
use crd::{DiskPool, DiskPoolStatus, PoolState};
use futures::StreamExt;
use k8s_openapi::{
    api::core::v1::{Event as k8Event, ObjectReference},
    apimachinery::pkg::apis::meta::v1::MicroTime,
};
use kube::{
    api::{Api, ListParams, ObjectMeta, Patch, PatchParams, PostParams},
    Client, CustomResourceExt, ResourceExt,
};
use kube_runtime::{
    controller::{Context, Controller, ReconcilerAction},
    finalizer::{finalizer, Event},
};
use openapi::{
    clients::{self, tower::Url},
    models::{CreatePoolBody, Pool, RestJsonError},
};
use opentelemetry::global;

use serde_json::json;
use snafu::Snafu;
use std::{collections::HashMap, ops::Deref, sync::Arc, time::Duration};
use tracing::{debug, error, info, trace, warn};

const WHO_AM_I: &str = "DiskPool Operator";
const WHO_AM_I_SHORT: &str = "dsp-operator";

/// Errors generated during the reconciliation loop
#[derive(Debug, Snafu)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Error {
    #[snafu(display(
        "Failed to reconcile '{}' CRD within set limits, aborting operation",
        name
    ))]
    /// Error generated when the loop stops processing
    ReconcileError {
        name: String,
    },
    /// Generated when we have a duplicate resource version for a given resource
    Duplicate {
        timeout: u32,
    },
    /// Spec error
    SpecError {
        value: String,
        timeout: u32,
    },
    #[snafu(display("Kubernetes client error: {}", source))]
    /// k8s client error
    Kube {
        source: kube::Error,
    },
    #[snafu(display("HTTP request error: {}", source))]
    Request {
        source: clients::tower::RequestError,
    },
    #[snafu(display("HTTP response error: {}", source))]
    Response {
        source: clients::tower::ResponseError<RestJsonError>,
    },
    Noun {},
}

impl From<clients::tower::Error<RestJsonError>> for Error {
    fn from(source: clients::tower::Error<RestJsonError>) -> Self {
        match source {
            clients::tower::Error::Request(source) => Error::Request { source },
            clients::tower::Error::Response(source) => Self::Response { source },
        }
    }
}

/// Additional per resource context during the runtime; it is volatile
#[derive(Clone)]
pub(crate) struct ResourceContext {
    /// The latest CRD known to us
    inner: DiskPool,
    /// Counter that keeps track of how many times the reconcile loop has run
    /// within the current state
    num_retries: u32,
    /// Reference to the operator context
    ctx: Arc<OperatorContext>,
}

impl Deref for ResourceContext {
    type Target = DiskPool;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Data we want access to in error/reconcile calls
pub(crate) struct OperatorContext {
    /// Reference to our k8s client
    k8s: Client,
    /// Hashtable of name and the full last seen CRD
    inventory: tokio::sync::RwLock<HashMap<String, ResourceContext>>,
    /// HTTP client
    http: clients::tower::ApiClient,
    /// Interval
    interval: u64,
    /// Retries
    retries: u32,
    /// Disable device validation before attempting to create the pool
    disable_device_validation: bool,
}

impl OperatorContext {
    /// Upsert the potential new CRD into the operator context. If an existing
    /// resource with the same name is present, the old resource is
    /// returned.
    pub(crate) async fn upsert(&self, ctx: Arc<OperatorContext>, dsp: DiskPool) -> ResourceContext {
        let resource = ResourceContext {
            inner: dsp,
            num_retries: 0,
            ctx,
        };

        let mut i = self.inventory.write().await;
        debug!(count = ?i.keys().count(), "current number of CRDS");

        match i.get_mut(&resource.name()) {
            Some(p) => {
                if p.resource_version() == resource.resource_version() {
                    if matches!(
                        resource.status,
                        Some(DiskPoolStatus {
                            state: PoolState::Online,
                            ..
                        })
                    ) {
                        return p.clone();
                    }

                    debug!(status =? resource.status, "duplicate event or long running operation");

                    // The status should be the same here as well
                    assert_eq!(&p.status, &resource.status);
                    p.num_retries += 1;
                    return p.clone();
                }

                // Its a new resource version which means we will swap it out
                // to reset the counter.
                let p = i
                    .insert(resource.name(), resource.clone())
                    .expect("existing resource should be present");
                info!(name = ?p.name(), "new resource_version inserted");
                resource
            }

            None => {
                let p = i.insert(resource.name(), resource.clone());
                assert!(p.is_none());
                resource
            }
        }
    }
    /// Remove the resource from the operator
    pub(crate) async fn remove(&self, name: String) -> Option<ResourceContext> {
        let mut i = self.inventory.write().await;
        let removed = i.remove(&name);
        if let Some(removed) = removed {
            info!(name =? removed.name(), "removed from inventory");
            return Some(removed);
        }
        None
    }
}

impl ResourceContext {
    /// Called when putting our finalizer on top of the resource.
    #[tracing::instrument(fields(name = ?dsp.name()))]
    pub(crate) async fn put_finalizer(dsp: DiskPool) -> Result<ReconcilerAction, Error> {
        Ok(ReconcilerAction {
            requeue_after: None,
        })
    }

    /// Our notification that we should remove the pool and then the finalizer
    #[tracing::instrument(fields(name = ?resource.name()) skip(resource))]
    pub(crate) async fn delete_finalizer(
        resource: ResourceContext,
    ) -> Result<ReconcilerAction, Error> {
        let ctx = resource.ctx.clone();
        resource.delete_pool().await?;
        ctx.remove(resource.name()).await;
        Ok(ReconcilerAction {
            requeue_after: None,
        })
    }

    /// Clone the inner value of this resource
    fn inner(&self) -> DiskPool {
        self.inner.clone()
    }

    /// Construct an API handle for the resource
    fn api(&self) -> Api<DiskPool> {
        Api::namespaced(self.ctx.k8s.clone(), &self.namespace().unwrap())
    }

    fn pools_api(&self) -> &dyn openapi::apis::pools_api::tower::client::Pools {
        self.ctx.http.pools_api()
    }

    fn block_devices_api(
        &self,
    ) -> &dyn openapi::apis::block_devices_api::tower::client::BlockDevices {
        self.ctx.http.block_devices_api()
    }

    /// Patch the given dsp status to the state provided. When not online the
    /// size should be assumed to be zero.
    async fn patch_status(&self, status: DiskPoolStatus) -> Result<DiskPool, Error> {
        let status = json!({ "status": status });

        let ps = PatchParams::apply(WHO_AM_I);

        let o = self
            .api()
            .patch_status(&self.name(), &ps, &Patch::Merge(&status))
            .await
            .map_err(|source| Error::Kube { source })?;

        debug!(name = ?o.name(), old = ?self.status, new =?o.status, "status changed");

        Ok(o)
    }

    /// Create a pool when there is no status found. When no status is found for
    /// this resource it implies that it does not exist yet and so we create
    /// it. We set the state of the of the object to Creating, such that we
    /// can track the its progress
    async fn start(&self) -> Result<ReconcilerAction, Error> {
        let _ = self.patch_status(DiskPoolStatus::default()).await?;
        Ok(ReconcilerAction {
            requeue_after: None,
        })
    }

    /// Mark the resource as errored which is its final state. A pool in the
    /// error state will not be deleted.
    async fn mark_error(&self) -> Result<ReconcilerAction, Error> {
        let _ = self.patch_status(DiskPoolStatus::error()).await?;

        error!(name = ?self.name(),"status set to error");
        Ok(ReconcilerAction {
            requeue_after: None,
        })
    }

    /// patch the resource state to creating.
    async fn is_missing(&self) -> Result<ReconcilerAction, Error> {
        self.patch_status(DiskPoolStatus::default()).await?;
        Ok(ReconcilerAction {
            requeue_after: None,
        })
    }
    /// patch the resource state to unknown
    async fn mark_unknown(&self) -> Result<ReconcilerAction, Error> {
        self.patch_status(DiskPoolStatus::unknown()).await?;
        Ok(ReconcilerAction {
            requeue_after: Some(std::time::Duration::from_secs(self.ctx.interval)),
        })
    }

    /// Stop reconciliation immediately and notify k8s engine.
    async fn stop_reconciliation(self) -> Result<ReconcilerAction, Error> {
        self.k8s_notify(
            "Failing pool creation",
            "Creating",
            &format!("Retry attempts ({}) exceeded", self.num_retries),
            "Error",
        )
        .await; // if we fail to notify k8s of the error, we will do so when we
                // reestablish a connection
        self.mark_error().await?;
        // we updated the resource as an error stop reconciliation
        Err(Error::ReconcileError { name: self.name() })
    }

    /// Create or import the pool, on failure try again. When we reach max error
    /// count we fail the whole thing.
    #[tracing::instrument(fields(name = ?self.name(), status = ?self.status) skip(self))]
    pub(crate) async fn create_or_import(self) -> Result<ReconcilerAction, Error> {
        if self.num_retries >= self.ctx.retries {
            return self.stop_reconciliation().await;
        }
        if !self.ctx.disable_device_validation {
            match self
                .block_devices_api()
                .get_node_block_devices(&self.spec.node(), Some(true))
                .await
            {
                Ok(response) => {
                    if !response.into_body().into_iter().any(|b| {
                        b.devname == normalize_disk(&self.spec.disks()[0])
                            || b.devlinks
                                .iter()
                                .any(|d| *d == normalize_disk(&self.spec.disks()[0]))
                    }) {
                        self.k8s_notify(
                            "Create or import",
                            "Missing",
                            &format!(
                                "The block device(s): {} can not be found",
                                &self.spec.disks()[0]
                            ),
                            "Warn",
                        )
                        .await;

                        return Err(Error::SpecError {
                            value: self.spec.disks()[0].clone(),
                            timeout: u32::pow(2, self.num_retries),
                        });
                    }
                }
                // We would land here if some error occurred ex, precondition failed, i.e. node
                // down, in that case we check for pool existence before setting a status.
                Err(_) => match self.pools_api().get_pool(&self.name()).await {
                    Ok(response) => {
                        let pool = response.into_body();
                        // As pool exists, set the status based on the presence of pool state.
                        return self.set_status_or_unknown(pool).await;
                    }
                    Err(clients::tower::Error::Request(_)) => {
                        // Probably grpc server is not yet up
                        return self.mark_unknown().await;
                    }
                    Err(clients::tower::Error::Response(err)) => {
                        if err.status() == clients::tower::StatusCode::SERVICE_UNAVAILABLE
                            || err.status() == clients::tower::StatusCode::REQUEST_TIMEOUT
                        {
                            // Probably grpc server is not yet up
                            return self.mark_unknown().await;
                        } else {
                            // If we don't find the pool, i.e. its not present or not yet created
                            // so, set the status to Creating to retry creation.
                            return self.mark_error().await;
                        }
                    }
                },
            }
        }
        let mut labels: HashMap<String, String> = HashMap::new();
        labels.insert(
            String::from(utils::CREATED_BY_KEY),
            String::from(utils::DSP_OPERATOR),
        );

        let body = CreatePoolBody::new_all(self.spec.disks(), labels);
        match self
            .pools_api()
            .put_node_pool(&self.spec.node(), &self.name(), body)
            .await
        {
            Ok(_) => {}
            Err(clients::tower::Error::Response(response))
                if response.status() == clients::tower::StatusCode::UNPROCESSABLE_ENTITY =>
            {
                // UNPROCESSABLE_ENTITY indicates that the pool spec already exists in the
                // control plane. So we want to update the CRD to
                // 'Created' to reflect this.
            }
            Err(e) => {
                return Err(e.into());
            }
        };

        self.k8s_notify(
            "Create or Import",
            "Created",
            "Created or imported pool",
            "Normal",
        )
        .await;

        let _ = self.patch_status(DiskPoolStatus::created()).await?;

        // We are done creating the pool, we patched to created which triggers a
        // new loop. Any error in the loop will call our error handler where we
        // decide what to do
        Ok(ReconcilerAction {
            requeue_after: None,
        })
    }

    /// Delete the pool from the io-engine instance
    #[tracing::instrument(fields(name = ?self.name(), status = ?self.status) skip(self))]
    async fn delete_pool(&self) -> Result<ReconcilerAction, Error> {
        // Do not delete pools which are in the error state. We have no way of
        // knowing whats wrong with the physical pool. Simply discard the CRD.
        if matches!(
            self.status,
            Some(DiskPoolStatus {
                state: PoolState::Error,
                ..
            })
        ) {
            return Ok(ReconcilerAction {
                requeue_after: None,
            });
        }

        let res = self
            .pools_api()
            .del_node_pool(&self.spec.node(), &self.name())
            .await?;

        if res.status().is_success() {
            self.k8s_notify(
                "Destroyed pool",
                "Destroy",
                "The pool has been destroyed",
                "Normal",
            )
            .await;
        }

        Ok(ReconcilerAction {
            requeue_after: None,
        })
    }

    /// Online the pool which is no-op from the data plane point of view. However
    /// it does provide us feedback from the k8s side of things which is
    /// useful when trouble shooting.
    #[tracing::instrument(fields(name = ?self.name(), status = ?self.status) skip(self))]
    async fn online_pool(self) -> Result<ReconcilerAction, Error> {
        let pool = self
            .pools_api()
            .get_node_pool(&self.spec.node(), &self.name())
            .await?
            .into_body();

        if pool.state.is_some() {
            let _ = self.patch_status(DiskPoolStatus::from(pool)).await?;

            self.k8s_notify(
                "Online pool",
                "Online",
                "Pool online and ready to roll!",
                "Normal",
            )
            .await;

            Ok(ReconcilerAction {
                requeue_after: None,
            })
        } else {
            // the pool does not have a status yet reschedule the operation
            Ok(ReconcilerAction {
                requeue_after: Some(Duration::from_secs(3)),
            })
        }
    }

    /// Check the state of the pool.
    ///
    /// Get the pool information from the control plane and use this to set the state of the CRD
    /// accordingly. If the control plane returns a pool state, set the CRD to 'Online'. If the
    /// control plane does not return a pool state (occurs when a node is missing), set the CRD to
    /// 'Unknown' and let the reconciler retry later.
    #[tracing::instrument(fields(name = ?self.name(), status = ?self.status) skip(self))]
    async fn pool_check(&self) -> Result<ReconcilerAction, Error> {
        let pool = match self
            .pools_api()
            .get_node_pool(&self.spec.node(), &self.name())
            .await
        {
            Ok(response) => response,
            Err(clients::tower::Error::Response(response)) => {
                return if response.status() == clients::tower::StatusCode::NOT_FOUND {
                    if self.metadata.deletion_timestamp.is_some() {
                        tracing::debug!(name = ?self.name(), "deleted stopping checker");
                        Ok(ReconcilerAction {
                            requeue_after: None,
                        })
                    } else {
                        tracing::warn!(pool = ?self.name(), "deleted by external event NOT recreating");
                        self.k8s_notify(
                            "Offline",
                            "Check",
                            "The pool has been deleted through an external API request",
                            "Warning",
                        )
                            .await;

                        // We expected the control plane to have a spec for this pool. It didn't so
                        // set the CRD to the error state and don't try to recreate it.
                        self.mark_error().await
                    }
                } else if response.status() == clients::tower::StatusCode::SERVICE_UNAVAILABLE || response.status() == clients::tower::StatusCode::REQUEST_TIMEOUT {
                    // Probably grpc server is not yet up
                    self.mark_unknown().await
                } else {
                    self.k8s_notify(
                        "Missing",
                        "Check",
                        &format!("The pool information is not available: {}", response),
                        "Warning",
                    )
                        .await;
                    self.is_missing().await
                }
            }
            Err(clients::tower::Error::Request(_)) => {
                // Probably grpc server is not yet up
                return self.mark_unknown().await;
            }
        }.into_body();
        // As pool exists, set the status based on the presence of pool state.
        self.set_status_or_unknown(pool).await
    }

    /// If the pool, has a state we set that status to the CR and if it does not have a state
    /// we set the status as unknown so that we can try again later.
    async fn set_status_or_unknown(&self, pool: Pool) -> Result<ReconcilerAction, Error> {
        if pool.state.is_some() {
            if let Some(status) = &self.status {
                let new_status = DiskPoolStatus::from(pool);
                if status != &new_status {
                    // update the usage state such that users can see the values changes
                    // as replica's are added and/or removed.
                    let _ = self.patch_status(new_status).await;
                }
            }
        } else {
            // There is no pool state, so we can't determine the health of the pool. Reflect this in
            // the CRD as an 'Unknown' state.
            if let Some(status) = &self.status {
                if status.state != PoolState::Unknown {
                    debug!("Missing pool state. Setting dsp state to 'Unknown'.");
                    self.k8s_notify(
                        "Unknown",
                        "Check",
                        "Missing pool state. Setting state to 'Unknown'.",
                        "Warning",
                    )
                    .await;
                }
            }
            return self.mark_unknown().await;
        }

        // always reschedule though
        Ok(ReconcilerAction {
            requeue_after: Some(std::time::Duration::from_secs(self.ctx.interval)),
        })
    }

    /// Post an event, typically these events are used to indicate that
    /// something happened. They should not be used to "log" generic
    /// information. Events are GC-ed by k8s automatically.
    ///
    /// action:
    ///     What action was taken/failed regarding to the Regarding object.
    /// reason:
    ///     This should be a short, machine understandable string that gives the
    ///     reason for the transition into the object's current status.
    /// message:
    ///     A human-readable description of the status of this operation.
    /// type_:
    ///     Type of this event (Normal, Warning), new types could be added in
    ///     the  future

    async fn k8s_notify(&self, action: &str, reason: &str, message: &str, type_: &str) {
        let client = self.ctx.k8s.clone();
        let ns = self.namespace().expect("must be namespaced");
        let e: Api<k8Event> = Api::namespaced(client, &ns);
        let pp = PostParams::default();
        let time = Utc::now();

        let metadata = ObjectMeta {
            // the name must be unique for all events we post
            generate_name: Some(format!("{}.{:x}", self.name(), time.timestamp())),
            namespace: Some(ns),
            ..Default::default()
        };

        let _ = e
            .create(
                &pp,
                &k8Event {
                    //last_timestamp: Some(time2),
                    event_time: Some(MicroTime(time)),
                    involved_object: ObjectReference {
                        api_version: Some(self.api_version.clone()),
                        field_path: None,
                        kind: Some(self.kind.clone()),
                        name: Some(self.name()),
                        namespace: self.namespace(),
                        resource_version: self.resource_version(),
                        uid: self.uid(),
                    },
                    action: Some(action.into()),
                    reason: Some(reason.into()),
                    type_: Some(type_.into()),
                    metadata,
                    reporting_component: Some(WHO_AM_I_SHORT.into()),
                    reporting_instance: Some(
                        std::env::var("MY_POD_NAME")
                            .ok()
                            .unwrap_or_else(|| WHO_AM_I_SHORT.into()),
                    ),
                    message: Some(message.into()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| error!(?e));
    }

    /// Callback hooks for the finalizers
    async fn finalizer(&self) -> Result<ReconcilerAction, Error> {
        let _ = finalizer(
            &self.api(),
            "openebs.io/diskpool-protection",
            self.inner(),
            |event| async move {
                match event {
                    Event::Apply(dsp) => Self::put_finalizer(dsp).await,
                    Event::Cleanup(_dsp) => Self::delete_finalizer(self.clone()).await,
                }
            },
        )
        .await
        .map_err(|e| error!(?e));

        Ok(ReconcilerAction {
            requeue_after: None,
        })
    }
}

/// ensure the CRD is installed. This creates a chicken and egg problem. When the CRD is removed,
/// the operator will fail to list the CRD going into a error loop.
///
/// To prevent that, we will simply panic, and hope we can make progress after restart. Keep
/// running is not an option as the operator would be "running" and the only way to know something
/// is wrong would be to consult the logs.
async fn ensure_crd(k8s: Client) {
    let dsp: Api<k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition> = Api::all(k8s);
    let lp = ListParams::default().fields(&format!("metadata.name={}", "diskpools.openebs.io"));
    let crds = dsp.list(&lp).await.expect("failed to list CRDS");

    // the CRD has not been installed yet, to avoid overwriting (and create upgrade issues) only
    // install it when there is no crd with the given name
    if crds.iter().count() == 0 {
        let crd = DiskPool::crd();
        info!(
            "Creating CRD: {}",
            serde_json::to_string_pretty(&crd).unwrap()
        );

        let pp = PostParams::default();
        match dsp.create(&pp, &crd).await {
            Ok(o) => {
                info!(crd = ?o.name(), "created");
                // let the CRD settle this purely to avoid errors messages in the console
                // that are harmless but can cause some confusion maybe.
                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            Err(e) => {
                error!("failed to create CRD error {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
                std::process::exit(1);
            }
        }
    } else {
        info!("CRD present")
    }
}

/// Determine what we want to do when dealing with errors from the
/// reconciliation loop
fn error_policy(error: &Error, _ctx: Context<OperatorContext>) -> ReconcilerAction {
    let duration = Duration::from_secs(match error {
        Error::Duplicate { timeout } | Error::SpecError { timeout, .. } => (*timeout).into(),

        Error::ReconcileError { .. } => {
            return ReconcilerAction {
                requeue_after: None,
            };
        }
        _ => 5,
    });

    let when = Utc::now()
        .checked_add_signed(chrono::Duration::from_std(duration).unwrap())
        .unwrap();
    warn!(
        "{}, retry scheduled @{} ({} seconds from now)",
        error,
        when.to_rfc2822(),
        duration.as_secs()
    );
    ReconcilerAction {
        requeue_after: Some(duration),
    }
}

/// The main work horse
#[tracing::instrument(fields(name = %dsp.spec.node(), status = ?dsp.status) skip(dsp, ctx))]
async fn reconcile(
    dsp: DiskPool,
    ctx: Context<OperatorContext>,
) -> Result<ReconcilerAction, Error> {
    let ctx = ctx.into_inner();
    let dsp = ctx.upsert(ctx.clone(), dsp).await;

    let _ = dsp.finalizer().await;

    match dsp.status {
        Some(DiskPoolStatus {
            state: PoolState::Creating,
            ..
        }) => {
            return dsp.create_or_import().await;
        }

        Some(DiskPoolStatus {
            state: PoolState::Created,
            ..
        }) => {
            return dsp.online_pool().await;
        }

        Some(DiskPoolStatus {
            state: PoolState::Online,
            ..
        })
        | Some(DiskPoolStatus {
            state: PoolState::Unknown,
            ..
        }) => {
            return dsp.pool_check().await;
        }

        Some(DiskPoolStatus {
            state: PoolState::Error,
            ..
        }) => {
            error!(pool = ?dsp.name(), "entered error as final state");
            Err(Error::ReconcileError { name: dsp.name() })
        }

        // We use this state to indicate its a new CRD however, we could (and
        // perhaps should) use the finalizer callback.
        None => return dsp.start().await,
    }
}

async fn pool_controller(args: ArgMatches<'_>) -> anyhow::Result<()> {
    let k8s = Client::try_default().await?;
    let namespace = args.value_of("namespace").unwrap();
    ensure_crd(k8s.clone()).await;

    let dsp: Api<DiskPool> = Api::namespaced(k8s.clone(), namespace);
    let lp = ListParams::default();
    let url = Url::parse(args.value_of("endpoint").unwrap()).expect("endpoint is not a valid URL");

    let timeout: Duration = args
        .value_of("request-timeout")
        .unwrap()
        .parse::<humantime::Duration>()
        .expect("timeout value is invalid")
        .into();

    let cfg =
        clients::tower::Configuration::new(url, timeout, None, None, true).map_err(|error| {
            anyhow::anyhow!(
                "Failed to create openapi configuration, Error: '{:?}'",
                error
            )
        })?;

    let context = Context::new(OperatorContext {
        k8s,
        inventory: tokio::sync::RwLock::new(HashMap::new()),
        http: clients::tower::ApiClient::new(cfg),
        interval: args
            .value_of("interval")
            .unwrap()
            .parse::<humantime::Duration>()
            .expect("interval value is invalid")
            .as_secs(),
        retries: args
            .value_of("retries")
            .unwrap()
            .parse::<u32>()
            .expect("retries value is invalid"),
        disable_device_validation: args.is_present("disable_device_validation"),
    });

    info!(
        "Starting DiskPool Operator (dsp) in namespace {}",
        namespace
    );

    Controller::new(dsp, lp)
        .run(reconcile, error_policy, context)
        .for_each(|res| async move {
            match res {
                Ok(o) => {
                    trace!(?o);
                }
                Err(e) => {
                    trace!(?e);
                }
            }
        })
        .await;

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let matches = App::new(utils::package_description!())
        .author(clap::crate_authors!())
        .version(utils::version_info_str!())
        .settings(&[
            clap::AppSettings::ColoredHelp,
            clap::AppSettings::ColorAlways,
        ])
        .arg(
            Arg::with_name("interval")
                .short("i")
                .long("interval")
                .env("INTERVAL")
                .default_value(utils::CACHE_POLL_PERIOD)
                .help("specify timer based reconciliation loop"),
        )
        .arg(
            Arg::with_name("request-timeout")
                .short("t")
                .long("request-timeout")
                .env("REQUEST_TIMEOUT")
                .default_value(utils::DEFAULT_REQ_TIMEOUT)
                .help("the timeout for remote requests"),
        )
        .arg(
            Arg::with_name("retries")
                .short("r")
                .env("RETRIES")
                .default_value("10")
                .help("the number of retries before we set the resource into the error state"),
        )
        .arg(
            Arg::with_name("endpoint")
                .long("endpoint")
                .short("e")
                .env("ENDPOINT")
                .default_value("http://ksnode-1:30011")
                .help("an URL endpoint to the control plane's rest endpoint"),
        )
        .arg(
            Arg::with_name("namespace")
                .long("namespace")
                .short("-n")
                .env("NAMESPACE")
                .default_value("mayastor")
                .help("the default namespace we are supposed to operate in"),
        )
        .arg(
            Arg::with_name("jaeger")
                .short("-j")
                .long("jaeger")
                .env("JAEGER_ENDPOINT")
                .help("enable open telemetry and forward to jaeger"),
        )
        .arg(
            Arg::with_name("disable_device_validation")
                .long("disable-device-validation")
                .takes_value(false)
                .help("do not attempt to validate the block device prior to pool creation"),
        )
        .get_matches();

    utils::print_package_info!();

    let tags = utils::tracing_telemetry::default_tracing_tags(
        utils::raw_version_str(),
        env!("CARGO_PKG_VERSION"),
    );
    utils::tracing_telemetry::init_tracing(
        "dsp-operator",
        tags,
        matches.value_of("jaeger").map(|s| s.to_string()),
    );

    pool_controller(matches).await?;
    global::shutdown_tracer_provider();
    Ok(())
}

/// Normalize the disks if they have a schema, we dont want to change anything
/// or do any error checking -- the loop will converge to the error state eventually
fn normalize_disk(disk: &str) -> String {
    Url::parse(disk).map_or(disk.to_string(), |u| {
        u.to_file_path()
            .unwrap_or_else(|_| disk.into())
            .as_path()
            .display()
            .to_string()
    })
}

#[cfg(test)]
mod test {

    #[test]
    fn normalize_disk() {
        use super::*;
        let disks = vec![
            "aio:///dev/null",
            "uring:///dev/null",
            "uring://dev/null", // this URL is invalid
        ];

        assert_eq!(normalize_disk(disks[0]), "/dev/null");
        assert_eq!(normalize_disk(disks[1]), "/dev/null");
        assert_eq!(normalize_disk(disks[2]), "uring://dev/null");
    }
}
