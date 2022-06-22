pub mod mbus_api;
/// Platform specific information, such as the cluster uid which is used as part of the pstor(etcd)
/// key prefix.
pub mod platform;
pub mod store;
pub mod types;

/// Helper to convert from Vec<F> into Vec<T>
pub trait IntoVec<T>: Sized {
    /// Performs the conversion.
    fn into_vec(self) -> Vec<T>;
}

impl<F: Into<T>, T> IntoVec<T> for Vec<F> {
    fn into_vec(self) -> Vec<T> {
        self.into_iter().map(Into::into).collect()
    }
}

/// Helper to convert from Option<F> into Option<T>
pub trait IntoOption<T>: Sized {
    /// Performs the conversion.
    fn into_opt(self) -> Option<T>;
}

impl<F: Into<T>, T> IntoOption<T> for Option<F> {
    fn into_opt(self) -> Option<T> {
        self.map(Into::into)
    }
}

/// Pre-init the Platform information.
pub use platform::init_cluster_info_or_panic;

/// Prefix for all keys stored in the persistent store (ETCD)
pub const ETCD_KEY_PREFIX: &str = "/openebs.io/mayastor";
