// SPDX-FileCopyrightText: 2019-2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use async_std::sync::{Arc, RwLock};
use libratman::netmod::Endpoint;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A dynamicly allocated, generic driver in memory
pub(crate) type GenericEndpoint = dyn Endpoint + 'static + Send + Sync;

type EpVec = Vec<EpWrap>;

/// Wrap around endpoints that can be removed
///
/// This way, when remove an interface, the ID's of other interfaces
/// don't have have to be updated or mapped, because their place in the list doesn't change.
enum EpWrap {
    Used(Arc<GenericEndpoint>),
    Void,
}

/// A map of available endpoint drivers
///
/// Currently the removing of drivers isn't supported, but it's
/// possible to have the same endpoint in the map multiple times, with
/// unique IDs.
#[derive(Default)]
pub(crate) struct DriverMap {
    curr: AtomicUsize,
    map: RwLock<EpVec>,
}

impl DriverMap {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Insert a new endpoint to the set of known endpoints
    pub(crate) async fn add(&self, ep: Arc<GenericEndpoint>) -> usize {
        let mut map = self.map.write().await;
        let curr = self.curr.fetch_add(1, Ordering::Relaxed);
        map.push(EpWrap::Used(ep));
        curr
    }

    /// Remove an endpoint from the list
    pub(crate) async fn remove(&self, id: usize) {
        let mut map = self.map.write().await;
        std::mem::swap(&mut map[id], &mut EpWrap::Void);
    }

    /// Get access to an endpoint via an Arc wrapper
    pub(crate) async fn get(&self, id: usize) -> Arc<GenericEndpoint> {
        let map = self.map.read().await;
        Arc::clone(match map[id] {
            EpWrap::Used(ref ep) => ep,
            EpWrap::Void => panic!("Trying to use a removed endpoint!"),
        })
    }

    /// Get access to all endpoints wrapped in Arc
    pub(crate) async fn get_all(&self) -> Vec<Arc<GenericEndpoint>> {
        let map = self.map.read().await;
        map.iter()
            .filter_map(|ep| match ep {
                EpWrap::Used(ref ep) => Some(Arc::clone(ep)),
                _ => None,
            })
            .collect()
    }

    /// Get all endpoints, except for the one provided via the ID
    pub(crate) async fn get_with_ids(&self) -> Vec<(Arc<GenericEndpoint>, usize)> {
        let map = self.map.read().await;
        map.iter()
            .enumerate()
            .filter_map(|(i, ep)| match ep {
                EpWrap::Used(ref ep) => Some((Arc::clone(ep), i)),
                _ => None,
            })
            .collect()
    }
}
