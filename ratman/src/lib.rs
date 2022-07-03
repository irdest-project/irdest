// SPDX-FileCopyrightText: 2019-2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

//! # Ratman
//!
//! **Note** most likely you are interested in the
//! [ratman-client](https://docs.rs/ratman-client) crate, which allows
//! you to connect to a Ratman daemon via an IPC socket.
//!
//! ## Technical overview
//!
//! A modular userspace frame router, implementing distance vector
//! routing, and delay tolerance.  Handles topology updates and user
//! discovery via flood heartbeats, and provides a non-namespaced view
//! into a network via ed25519 keyed user IDs.
//!
//! One of the core principles of Ratman is to make network roaming
//! easier, building a general abstraction over a network, leaving it
//! up to drivers to interface with implementation specifics.
//!
//! As such, the Ratman routing tables, and user IDs don't use IPs and
//! one device on the network could potentially be home to many user
//! IDs.  The decoupling of users and devices, making it impossible to
//! track a user back to a specific device, is by design.
//!
//!
//! ## Interface routing
//!
//! The interface that binds the Ratman router to underlying drivers
//! is called `netmod`, which handles sending and receiving frames.  A
//! frame is a piece of data, with a checksum, which may be part of a
//! larger mesage.
//!
//! Routing is then done by mapping a user ID to an interface (plus
//! some target data that's left to the driver to interpret).  This
//! way Ratman is able to route across network boundries, and on
//! unpriviledged platforms (such as phones).
//!
//!
//! ## Development status
//!
//! Despite the API looking relatively complete, the Ratman internals
//! are still very work-in-progres.  Topology changes _should_ be
//! handled gracefully, but there's no cycle detection or mitigation,
//! routing is done based on the last successful circuit, no
//! journaling, and there's no metrics API for netmod drivers.
//!
//! We would love to hear feedback from you, building applications on
//! top of Ratman, so that the project and routing protocol can get
//! better.  But please be aware that it is my no means "production
//! ready" code.
//!
//!
//! ## Usage
//!
//! To use Ratman, you need to create a Router.  This type exposes an
//! async API to interact with various network abstractions.  A
//! networking endpoint and basic datagram types are defined in the
//! [`ratman-netmod`] crate, and the routing identities are defined in
//! the [`ratman-identity`] crate.
//!
//! [`ratman-netmod`]: https://crates.io/crates/ratman-netmod
//! [`ratman-identity`]: https://crates.io/crates/ratman-identity
//!
//! ```rust
//! # use async_std::task;
//! # async fn testing() {
//! use ratman::{Router, Identity};
//! use netmod_mem::MemMod;
//! # use std::time::Duration;
//!
//! // Build a simple channel in memory
//! let mm1 = MemMod::new();
//! let mm2 = MemMod::new();
//! mm1.link(&mm2);
//!
//! // Initialise two routers, one for each device
//! let r1 = Router::new();
//! let r2 = Router::new();
//!
//! // Add channel endpoints to routers
//! r1.add_endpoint(mm1).await;
//! r2.add_endpoint(mm2).await;
//!
//! // Create some users and add them to the routers
//! let u1 = Identity::random();
//! r1.add_user(u1).await;
//!
//! let u2 = Identity::random();
//! r2.add_user(u2).await;
//!
//! // And mark them "online"
//! r1.online(u1).await;
//! r2.online(u2).await;
//!
//! // The routers will now start announcing their new users on the
//! // micro-network.  You can now poll for new user discoveries.
//! assert_eq!(r1.discover().await, u2);
//! # }
//! # task::block_on(testing());
//! ```
//!
//! Obviously this example is trivial, but hopefully it provides an
//! overview of how the API of the router works.  Larger networks are
//! fundamentally not any different from the example above: just more
//! users, more hops, and more delay between marking a user as
//! "online" and being able to sense their presence.
//!
//! ## License
//!
//! Ratman is part of the Irdest project, and licensed under the [GNU
//! Affero General Public License version 3 or
//! later](../licenses/agpl-3.0.md).
//!
//! See the Irdest repository README for additional permissions
//! granted by the authors for this code.

#[macro_use]
extern crate tracing;

pub mod clock;
mod core;
mod data;
mod error;
mod protocol;
mod slicer;

#[cfg(feature = "daemon")]
pub mod daemon;

#[cfg(feature = "ffi")]
pub mod ffi;

// Provide exports to the rest of the crate
pub(crate) use {data::Payload, protocol::Protocol, slicer::Slicer};
pub(crate) type IoPair<T> = (Sender<T>, Receiver<T>);

// Public API facade
pub use crate::{
    data::{Message, MsgId},
    error::{Error, Result},
};
pub use netmod;
pub use types::{Identity, Recipient, TimePair, ID_LEN};

use crate::core::Core;
use async_std::{
    channel::{Receiver, Sender},
    sync::Arc,
};
use clock::{ClockCtrl, Tasks};
use netmod::Endpoint;

/// Primary async ratman router handle
///
/// Make sure you initialise endpoints before calling [`run`], as the
/// set of endpoints gets locked and can't be changed during runtime.
///
/// [`run`]: struct.Router.html#method.run
#[derive(Clone)]
pub struct Router {
    inner: Arc<Core>,
    proto: Arc<Protocol>,
}

impl Router {
    /// Create a new and empty message router
    ///
    /// It's currently not possible to restore a router from stored
    /// state, which means that all routing tables are lost when the
    /// router is stopped.
    pub fn new() -> Self {
        let proto = Protocol::new();
        let inner = Arc::new(Core::init());
        Self { inner, proto }
    }

    /// Add a new endpoint to this router
    ///
    /// An endpoint is defined by the [`Endpoint`] trait from the
    /// `ratman-netmod` crate.  Once added, an endpoint can't be
    /// removed while in active operation: the router will have to be
    /// recreated without the endpoint you wish to remove.
    ///
    /// [`Endpoint`]: https://docs.rs/ratman-netmod/0.1.0/ratman_netmod/trait.Endpoint.html
    pub async fn add_endpoint(&self, ep: Arc<impl Endpoint + 'static + Send + Sync>) -> usize {
        self.inner.add_ep(ep).await
    }

    /// **Unstable fn:** get an endpoint from the driver set by ID
    #[doc(hidden)]
    pub async fn get_endpoint(&self, id: usize) -> Arc<dyn Endpoint + 'static + Send + Sync> {
        self.inner.get_ep(id).await
    }

    /// Remove an endpoint from the router by ID
    ///
    /// This function is primarily meant for testing purposes, and
    /// shouldn't be used in heavy operation.  The required ID is
    /// returned by `add_endpoint`.
    pub async fn del_endpoint(&self, id: usize) {
        self.inner.rm_ep(id).await;
    }

    /// Add an identity to the local set
    ///
    /// Ratman will listen for messages to local identities and offer
    /// them up for polling via the Router API.
    pub async fn add_user(&self, id: Identity) -> Result<()> {
        self.inner.add_local(id).await
    }

    /// Remove a local identity, discarding imcomplete messages
    ///
    /// Ratman will by default remove all cached frames from the
    /// collector.  Optionally these frames can be moved into the
    /// journal with low priority instead.
    pub async fn del_user(&self, id: Identity, _keep: bool) -> Result<()> {
        self.inner.rm_local(id).await
    }

    /// Set a user ID as online and broadcast announcements
    ///
    /// This function will return an error if the user is already
    /// marked as online, or if no such user is known to the router
    pub async fn online(&self, id: Identity) -> Result<()> {
        self.inner.known(id, true).await?;
        Arc::clone(&self.proto)
            .online(id, Arc::clone(&self.inner))
            .await
    }

    /// Set a user ID as offline and stop broadcasts
    pub async fn offline(&self, id: Identity) -> Result<()> {
        self.inner.known(id, true).await?;
        self.proto.offline(id).await
    }

    /// Check the local routing table for a user ID
    pub async fn known(&self, id: Identity) -> Result<()> {
        self.inner.known(id, false).await
    }

    /// Check for newly discovered users on the network
    pub async fn discover(&self) -> Identity {
        self.inner.discover().await
    }

    /// Register a manual clock controller object for internal tasks
    pub fn clock(&self, _cc: ClockCtrl<Tasks>) -> Result<()> {
        unimplemented!()
    }

    /// Send a flood message into the network
    pub async fn flood(
        &self,
        sender: Identity,
        scope: Identity,
        payload: Vec<u8>,
        sign: Vec<u8>,
    ) -> Result<MsgId> {
        debug!("Sending flood to namespace {}", scope);
        let id = MsgId::random();
        self.inner
            .send(Message {
                id,
                sender,
                recipient: Recipient::Flood(scope),
                payload,
                timesig: TimePair::sending(),
                sign,
            })
            .await
            .map(|_| id)
    }

    /// Dispatch a message into a network
    ///
    /// This operation completes asynchronously, and will yield a
    /// result with information about any error that occured while
    /// sending.
    ///
    /// If you result is an `Error::DispatchFaled`, that just means
    /// that at least one of the packets your Message was sliced into
    /// didn't send properly.  As long as you're not changing the data
    /// layout of your payload, or the `MsgId`, it's safe to simply
    /// retry: the receiving collector/ journals on the way will still
    /// be able to associate the frames, and drop the ones that were
    /// already dispatched, essentially only filling in the missing
    /// gaps.
    pub async fn send(&self, msg: Message) -> Result<()> {
        self.inner.send(msg).await
    }

    /// Get the next available message from the router
    ///
    /// **Note**: This function can't ever really fail, because it
    /// only reads from a set of completed Messages that have been
    /// parsed and handled.  When an error occurs on an incoming
    /// Message, the errors are logged in the diagnostics module, and
    /// can be read from there asynchronously.
    pub async fn next(&self) -> Message {
        self.inner.next().await
    }

    /// Return a list of all known addresses
    pub async fn known_addresses(&self) -> Vec<(Identity, bool)> {
        self.inner.all_addrs().await
    }
}

/// A very simple API level test to make sure that payloads remain the same
#[async_std::test]
async fn matching_payloads() {
    use crate::TimePair;
    use netmod_mem::MemMod;
    let (m1, m2) = MemMod::make_pair();

    let r1 = Router::new();
    let r2 = Router::new();

    r1.add_endpoint(m1).await;
    r2.add_endpoint(m2).await;

    let u1 = Identity::random();
    let u2 = Identity::random();

    r1.add_user(u1).await.unwrap();
    r2.add_user(u2).await.unwrap();

    r1.online(u1).await.unwrap();
    r2.online(u2).await.unwrap();

    let msg = Message {
        id: Identity::random(),
        sender: u1,
        recipient: Recipient::Standard(vec![u2]),
        payload: vec![1, 3, 1, 2],
        timesig: TimePair::sending(),
        sign: vec!['a' as u8, 'c' as u8, 'a' as u8, 'b' as u8],
    };

    // Wait for the announcement to sync
    let _ = r1.discover().await;

    // Then send a message
    r1.send(msg.clone()).await.unwrap();

    let msg2 = r2.next().await;

    assert_eq!(msg2.remove_recv_time(), msg);
}
