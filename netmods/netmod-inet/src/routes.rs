// SPDX-FileCopyrightText: 2019-2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use crate::peer::Peer;
use libratman::tokio::{task::spawn_local, time::sleep};
use libratman::{endpoint::NeighbourMetrics, tokio::sync::RwLock, types::Ident32};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub(crate) type Target = u16;

#[derive(Default)]
pub(crate) struct Routes {
    latest: AtomicU16,
    pub(crate) inner: RwLock<BTreeMap<Ident32, Arc<Peer>>>,
    pub(crate) metrics: RwLock<MetricsTable>,
}

impl Routes {
    /// Create a new empty routing table
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Get the next valid target number for a peer
    pub(crate) fn next_target(self: &Arc<Self>) -> u16 {
        self.latest.fetch_add(1, Ordering::AcqRel)
    }

    /// Add a new peer routing routing map
    ///
    /// This is done by the server when adding a new peer, or by the
    /// session manager when creating a connection.
    ///
    /// When adding a peer there might already have been a previous
    /// peer in the slot, meaning that outgoing frames have
    /// accumulated in the out buffer.  These need to be scheduled to send after this call is done
    pub(crate) async fn add_peer(self: &Arc<Self>, peer_id: Ident32, peer: Arc<Peer>) {
        let mut inner = self.inner.write().await;
        inner.insert(peer_id, peer);
    }

    /// Remove a peer from the routing map
    ///
    /// This should only be done by the peer itself, when it closes
    /// its stream
    pub(crate) async fn remove_peer(self: &Arc<Self>, peer_id: Ident32) -> Arc<Peer> {
        let mut inner = self.inner.write().await;
        inner.remove(&peer_id).unwrap()
    }

    /// All peers are valid, but some are more valid than others
    ///
    /// Check if we can currently send data to this peer (i.e. will
    /// get_peer_by_id fail?).  There is a race condition in here
    /// somewhere. Woops
    pub(crate) async fn exists(self: &Arc<Self>, peer_id: Ident32) -> bool {
        let inner = self.inner.read().await;
        inner.get(&peer_id).is_some()
    }

    /// Return the peer associated with a particular target ID
    pub(crate) async fn get_peer_by_id(self: &Arc<Self>, peer_id: Ident32) -> Option<Arc<Peer>> {
        let inner = self.inner.read().await;
        inner.get(&peer_id).map(|peer| Arc::clone(&peer))
    }

    pub(crate) async fn get_all_valid(self: &Arc<Self>) -> Vec<(Arc<Peer>, Ident32)> {
        let inner = self.inner.read().await;
        inner
            .iter()
            .map(|(id, peer)| (Arc::clone(&peer), *id))
            .collect()
    }
}
