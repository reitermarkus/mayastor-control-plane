use crate::{
    store::etcd_keep_alive::{EtcdSingletonLock, LeaseLockInfo},
    types::v0::store::{
        definitions::{
            Connect, Delete, DeserialiseValue, Get, GetPrefix, KeyString, ObjectKey, Put,
            SerialiseValue, StorableObject, Store, StoreError, StoreError::MissingEntry, StoreKey,
            StoreValue, ValueString, Watch, WatchEvent,
        },
        registry::ControlPlaneService,
    },
};
use async_trait::async_trait;
use etcd_client::{
    Client, Compare, CompareOp, EventType, GetOptions, KeyValue, Txn, TxnOp, WatchStream, Watcher,
};
use serde_json::Value;
use snafu::ResultExt;
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// etcd client
#[derive(Clone)]
pub struct Etcd {
    client: Client,
    lease_lock_info: Option<LeaseLockInfo>,
}

impl std::fmt::Debug for Etcd {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl Etcd {
    /// Create a new instance of the etcd client
    pub async fn new(endpoint: &str) -> Result<Etcd, StoreError> {
        let _ = crate::platform::init_cluster_info()
            .await
            .map_err(|error| StoreError::NotReady {
                reason: format!("Platform not ready: {}", error),
            })?;
        Ok(Self::from(
            &Client::connect([endpoint], None)
                .await
                .context(Connect {})?,
            None,
        ))
    }
    /// Create `Etcd` from an existing instance of the etcd `Client`
    pub(crate) fn from(client: &Client, lease_lock_info: Option<LeaseLockInfo>) -> Etcd {
        Etcd {
            client: client.clone(),
            lease_lock_info,
        }
    }
    /// Create a new instance of the etcd client with a lease associated with `service_name`.
    /// See `EtcdLeaseLockKeeper` for more information.
    pub async fn new_leased<E: AsRef<str>, S: AsRef<[E]>>(
        endpoints: S,
        service_name: ControlPlaneService,
        lease_time: std::time::Duration,
    ) -> Result<Etcd, StoreError> {
        let _ = crate::platform::init_cluster_info()
            .await
            .map_err(|error| StoreError::NotReady {
                reason: format!("Platform not ready: {}", error),
            })?;

        let client = Client::connect(endpoints, None).await.context(Connect {})?;

        let lease_info = EtcdSingletonLock::start(client.clone(), service_name, lease_time).await?;
        Ok(Self::from(&client, Some(lease_info)))
    }

    /// Get the lease lock pair, (lease_id, lock_key)
    /// Returns `StoreError::NotReady` if the lease is not active
    fn lease_lock(&self) -> Result<Option<(i64, String)>, StoreError> {
        match &self.lease_lock_info {
            None => Ok(None),
            Some(lease_info) => lease_info.lease_lock().map(Some),
        }
    }

    /// Revokes the lease and releases the associated lock
    pub async fn revoke(&self) {
        if let Some(info) = &self.lease_lock_info {
            info.revoke().await;
        }
    }
}

#[async_trait]
impl Store for Etcd {
    /// 'Put' a key-value pair into etcd.
    async fn put_kv<K: StoreKey, V: StoreValue>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), StoreError> {
        let vec_value = serde_json::to_vec(value).context(SerialiseValue)?;
        if let Some((lease_id, lock_key)) = self.lease_lock()? {
            let cmp = Compare::lease(lock_key.clone(), CompareOp::Equal, lease_id);
            let put = TxnOp::put(key.to_string(), vec_value, None);
            let resp = self
                .client
                .txn(Txn::new().when([cmp]).and_then([put]))
                .await
                .context(Put {
                    key: key.to_string(),
                    value: serde_json::to_string(value).context(SerialiseValue)?,
                })?;
            if !resp.succeeded() {
                return Err(StoreError::FailedLock {
                    reason: format!(
                        "Etcd Txn Compare key '{}' to lease id '{:x}' failed",
                        lock_key, lease_id
                    ),
                });
            }
        } else {
            self.client
                .put(key.to_string(), vec_value, None)
                .await
                .context(Put {
                    key: key.to_string(),
                    value: serde_json::to_string(value).context(SerialiseValue)?,
                })?;
        };

        Ok(())
    }

    /// 'Get' the value for the given key from etcd.
    async fn get_kv<K: StoreKey>(&mut self, key: &K) -> Result<Value, StoreError> {
        let resp = self.client.get(key.to_string(), None).await.context(Get {
            key: key.to_string(),
        })?;
        match resp.kvs().first() {
            Some(kv) => Ok(
                serde_json::from_slice(kv.value()).context(DeserialiseValue {
                    value: kv.value_str().context(ValueString {})?,
                })?,
            ),
            None => Err(MissingEntry {
                key: key.to_string(),
            }),
        }
    }

    /// 'Delete' the entry with the given key from etcd.
    async fn delete_kv<K: StoreKey>(&mut self, key: &K) -> Result<(), StoreError> {
        if let Some((lease_id, lock_key)) = self.lease_lock()? {
            let cmp = Compare::lease(lock_key.clone(), CompareOp::Equal, lease_id);
            let del = TxnOp::delete(key.to_string(), None);
            let resp = self
                .client
                .txn(Txn::new().when([cmp]).and_then([del]))
                .await
                .context(Delete {
                    key: key.to_string(),
                })?;
            if !resp.succeeded() {
                return Err(StoreError::FailedLock {
                    reason: format!(
                        "Etcd Txn Compare key '{}' to lease id '{:x}' failed",
                        lock_key, lease_id
                    ),
                });
            }
        } else {
            self.client
                .delete(key.to_string(), None)
                .await
                .context(Delete {
                    key: key.to_string(),
                })?;
        };

        Ok(())
    }

    /// 'Watch' the etcd entry with the given key.
    /// A receiver channel is returned which is signalled when the entry with
    /// the given key is changed.
    async fn watch_kv<K: StoreKey>(
        &mut self,
        key: &K,
    ) -> Result<Receiver<Result<WatchEvent, StoreError>>, StoreError> {
        let (sender, receiver) = channel(100);
        let (watcher, stream) = self
            .client
            .watch(key.to_string(), None)
            .await
            .context(Watch {
                key: key.to_string(),
            })?;
        watch(watcher, stream, sender);
        Ok(receiver)
    }

    async fn put_obj<O: StorableObject>(&mut self, object: &O) -> Result<(), StoreError> {
        let key = object.key().key();
        let vec_value = serde_json::to_vec(object).context(SerialiseValue)?;

        if let Some((lease_id, lock_key)) = self.lease_lock()? {
            let cmp = Compare::lease(lock_key.clone(), CompareOp::Equal, lease_id);
            let put = TxnOp::put(key.to_string(), vec_value, None);
            let resp = self
                .client
                .txn(Txn::new().when([cmp]).and_then([put]))
                .await
                .context(Put {
                    key: object.key().key(),
                    value: serde_json::to_string(object).context(SerialiseValue)?,
                })?;
            if !resp.succeeded() {
                return Err(StoreError::FailedLock {
                    reason: format!(
                        "Etcd Txn Compare key '{}' to lease id '{:x}' failed",
                        lock_key, lease_id
                    ),
                });
            }
        } else {
            self.client.put(key, vec_value, None).await.context(Put {
                key: object.key().key(),
                value: serde_json::to_string(object).context(SerialiseValue)?,
            })?;
        };

        Ok(())
    }

    async fn get_obj<O: StorableObject>(&mut self, key: &O::Key) -> Result<O, StoreError> {
        let resp = self
            .client
            .get(key.key(), None)
            .await
            .context(Get { key: key.key() })?;
        match resp.kvs().first() {
            Some(kv) => Ok(
                serde_json::from_slice(kv.value()).context(DeserialiseValue {
                    value: kv.value_str().context(ValueString {})?,
                })?,
            ),
            None => Err(MissingEntry { key: key.key() }),
        }
    }

    /// Retrieve objects with the given key prefix
    async fn get_values_prefix(
        &mut self,
        key_prefix: &str,
    ) -> Result<Vec<(String, Value)>, StoreError> {
        let resp = self
            .client
            .get(key_prefix, Some(GetOptions::new().with_prefix()))
            .await
            .context(GetPrefix { prefix: key_prefix })?;
        let result = resp
            .kvs()
            .iter()
            .map(|kv| {
                (
                    kv.key_str().unwrap().to_string(),
                    // unwrap_or_default is used since when using to dump data, the lease entry
                    // does not have a value, which can cause panic
                    serde_json::from_slice(kv.value()).unwrap_or_default(),
                )
            })
            .collect();
        Ok(result)
    }

    async fn watch_obj<K: ObjectKey>(
        &mut self,
        key: &K,
    ) -> Result<Receiver<Result<WatchEvent, StoreError>>, StoreError> {
        let (sender, receiver) = channel(100);
        let (watcher, stream) = self
            .client
            .watch(key.key(), None)
            .await
            .context(Watch { key: key.key() })?;
        watch(watcher, stream, sender);
        Ok(receiver)
    }

    async fn online(&mut self) -> bool {
        self.client.status().await.is_ok()
    }
}

/// Watch for events in the key-value store.
/// When an event occurs, a WatchEvent is sent over the channel.
/// When a 'delete' event is received, the watcher stops watching.
fn watch(
    _watcher: Watcher,
    mut stream: WatchStream,
    sender: Sender<Result<WatchEvent, StoreError>>,
) {
    // For now we spawn a thread for each value that is watched.
    // If we find that we are watching lots of events, this can be optimised.
    // TODO: Optimise the spawning of threads if required.
    tokio::spawn(async move {
        loop {
            let response = match stream.message().await {
                Ok(msg) => {
                    match msg {
                        Some(resp) => resp,
                        // stream cancelled
                        None => {
                            return;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to get message with error {}", e);
                    return;
                }
            };

            for event in response.events() {
                match event.event_type() {
                    EventType::Put => {
                        if let Some(kv) = event.kv() {
                            let result = match deserialise_kv(kv) {
                                Ok((key, value)) => Ok(WatchEvent::Put(key, value)),
                                Err(e) => Err(e),
                            };
                            if sender.send(result).await.is_err() {
                                // Send only fails if the receiver is closed, so
                                // just stop watching.
                                return;
                            }
                        }
                    }
                    EventType::Delete => {
                        // Send only fails if the receiver is closed. We are
                        // returning here anyway, so the error doesn't need to
                        // be handled.
                        let _ = sender.send(Ok(WatchEvent::Delete)).await;
                        return;
                    }
                }
            }
        }
    });
}

/// Deserialise a key-value pair into serde_json::Value representations.
fn deserialise_kv(kv: &KeyValue) -> Result<(String, Value), StoreError> {
    let key_str = kv.key_str().context(KeyString {})?.to_string();
    let value_str = kv.value_str().context(ValueString {})?;
    let value = serde_json::from_str(value_str).context(DeserialiseValue {
        value: value_str.to_string(),
    })?;
    Ok((key_str, value))
}

/// Returns the key prefix that is used for the keys.
/// The platform info and namespace where the product is running must be specified.
pub fn build_key_prefix(platform: impl crate::platform::PlatformInfo, namespace: String) -> String {
    crate::types::v0::store::definitions::build_key_prefix(&platform, namespace)
}
