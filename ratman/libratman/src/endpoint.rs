//! Endpoint abstraction module

use crate::{
    types::{CurrentStatus, Ident32, InMemoryEnvelope, Neighbour},
    Result,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The main trait describing a Ratman networking interface
///
/// All functions work without mutability because an endpoint is
/// expected to implement some access multiplexing or rely on atomic
/// operations to ensure thread safety.  This is because it's not
/// reasonable for an endpoint driver to rely purely on Rust's
/// ownership and mutability model, because it will inevitably have to
/// interact with system components, other buffers that push into a
/// queue, or similar.
///
/// This interface doesn't care about the implementation details of
/// these endpoints, and so, to make matters simpler for the router,
/// and to make it obvious that internal mutability needs to be used,
/// this interface is immutable by default.
#[async_trait]
pub trait EndpointExt {
    /// Return a string identifier for this netmod instance
    ///
    /// The identifier logic MUST handle multi-instancing, if it supports this.
    fn identifier(&self) -> String {
        "<unknown>".into()
    }

    /// Return a status digest for the current instance
    fn status(&self) -> CurrentStatus {
        CurrentStatus::Unknown
    }

    /// Query collected connection metrics to a given neighbour.  Currently this
    /// only includes the last measured receive bandwidth.
    async fn metrics_for_neighbour(&self, _neighbour: Neighbour) -> Result<NeighbourMetrics> {
        Err(crate::RatmanError::Netmod(crate::NetmodError::NotSupported))
    }

    /// Start a peering session with a remote address
    ///
    /// The formatting of this address is specific to the netmod implementation,
    /// meaning that different netmods can rely on fundamentally different
    /// address schemas to establish their connections.  For example, the `inet`
    /// netmod simply uses IPv6 socket addresses, while the `lora` netmod relies
    /// on cryptographic IDs of nearby gateways.
    ///
    /// The identifier returned must be a unique peer identifier, similar to the
    /// `Neighbour` abstraction that is used by `send` and `next`.  Currently
    /// this API doesn't consider stopping a peering intent (i.e. even if a
    /// connection drops, the netmod should always attempt to re-establish the
    /// connection).  The returned peer identifier can be used in the future to
    /// disconnect two routers from each other without having to restart all
    /// other connections.
    async fn start_peering(&self, _addr: &str) -> Result<u16> {
        Err(crate::RatmanError::Netmod(crate::NetmodError::NotSupported))
    }

    /// Send a frame envelope to a target over this link
    ///
    /// Sending characteristics are entirely up to the implementation.
    /// As mentioned in the `size_hint()` documentation, this function
    /// **must not** panic on a frame envelope for size reasons,
    /// instead it should return `Error::FrameTooLarge`.
    ///
    /// The target ID is a way to instruct a netmod where to send a
    /// frame in a one-to-many mapping.  When implementing a
    /// one-to-one endpoint this ID can be ignored (set to 0).
    ///
    /// Optionally an exclusion target can be provided.  This is used
    /// to prevent sending flood messages back to their original
    /// recipients.  *When implementing a one-to-one endpoint*, the
    /// frame MUST be dropped when exclude contains any value!
    async fn send(
        &self,
        envelope: InMemoryEnvelope,
        target: Neighbour,
        exclude: Option<Ident32>,
    ) -> Result<()>;

    /// Poll for the next available Frame from this interface
    ///
    /// The errors returned by this netmod can currently not be
    /// returned to the sending application, but they can be logged
    /// for statistics purposes.
    ///
    /// **Note**: the implementation of this future MUST be
    /// cancellation safe!  Please consult the `tokio::select`
    /// documentation on what exactly that means for your Netmod
    /// implementation!  *Serious data loss* can occur if special care
    /// is not taken!
    async fn next(&self) -> Result<(InMemoryEnvelope, Neighbour)>;
}

#[async_trait]
impl<T: EndpointExt + Send + Sync> EndpointExt for Arc<T> {
    async fn send(
        &self,
        envelope: InMemoryEnvelope,
        target: Neighbour,
        exclude: Option<Ident32>,
    ) -> Result<()> {
        T::send(self, envelope, target, exclude).await
    }

    async fn next(&self) -> Result<(InMemoryEnvelope, Neighbour)> {
        T::next(self).await
    }
}

/// Return measured metrics for a given neighbouing connection
///
/// This information is used by the router to update the RouteData in
/// encountered announcement frames.
#[derive(
    Default, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct NeighbourMetrics {
    /// TX speeds measured in bytes per second
    pub write_bandwidth: u64,
    /// RX speeds measured in bytes per second
    pub read_bandwidth: u64,
}
