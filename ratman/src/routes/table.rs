// SPDX-FileCopyrightText: 2019-2024 Katharina Fey <kookie@spacekookie.de>
// SPDX-FileCopyrightText: 2022 embr <hi@liclac.eu>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use crate::{
    links::LinksMap,
    storage::{
        route::{RouteData, RouteEntry, RouteState},
        MetadataDb,
    },
    util::IoPair,
    web::NeighbourEntry,
};
use chrono::Utc;
use libratman::{
    api::types::PeerEntry,
    frame::carrier::AnnounceFrameV1,
    tokio::{
        sync::{mpsc::channel, RwLock},
        task::{spawn_blocking, spawn_local},
        time::sleep,
    },
    types::{Address, Ident32, Neighbour},
    NonfatalError, RatmanError, Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use super::scoring::{DefaultScorer, RouteScorer, ScorerConfiguration, StoreForwardScorer};

/// Main Ratman routing table
///
/// It keeps track of available addresses and their types (i.e. remote
/// or local, and an address key or a namespace key).  New addresses
/// can be polled via the `new` announce channel.
pub(crate) struct RouteTable {
    meta_db: Arc<MetadataDb>,
    activity_tasks: Arc<RwLock<BTreeSet<Address>>>,
    pub(crate) solvers: Vec<Box<dyn RouteScorer + Send + Sync + 'static>>,
    pub(crate) solver_state: RwLock<ScorerConfiguration>,
    #[allow(unused)]
    new: IoPair<Address>,
    #[allow(unused)]
    #[cfg(feature = "dashboard")]
    metrics: metrics::RouteTableMetrics,
}

impl RouteTable {
    pub(crate) fn new(meta_db: Arc<MetadataDb>) -> Arc<Self> {
        let this = Arc::new(Self {
            meta_db,
            activity_tasks: Default::default(),
            solvers: vec![Box::new(DefaultScorer), Box::new(StoreForwardScorer)],
            solver_state: RwLock::new(ScorerConfiguration::default()),
            new: channel(1),
            #[cfg(feature = "dashboard")]
            metrics: metrics::RouteTableMetrics::default(),
        });

        // When we have just come online we need to start a route activity
        // checker for every peer.  Peers that were stored as "Active" in the
        // database because they were the last time this instance ran, but have
        // since disappeared will appear online after startup, until the task
        // notices that they're not and marks them as such
        this.meta_db
            .routes
            .iter()
            .into_iter()
            .filter_map(|(_, entry)| {
                if entry.link_data.is_empty() {
                    None
                } else {
                    Some(entry.peer)
                }
            })
            .for_each(|peer| {
                this.clone().start_activity_task(peer);
            });

        this
    }
}

/////////////////////////////////// SNIP ///////////////////////////////////

/// A netmod endpoint ID and an endpoint target ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct EpNeighbourPair(pub(crate) usize, pub(crate) Ident32);

/// An ephemeral routing table
///
/// It only captures the current state of best routes and has no
/// persistence relationships.  It can update entries for topology
/// changes, but these are not carried between sessions.

impl RouteTable {
    /// Register metrics with a Prometheus registry.
    #[cfg(feature = "dashboard")]
    pub(crate) fn register_metrics(&self, registry: &mut prometheus_client::registry::Registry) {
        self.metrics.register(registry);
    }

    /// Update or add an IDs entry in the routing table
    ///
    /// If the Id was not previously known to the router, it is queued
    /// to the `new` set which can be polled by calling `discovered().await`.
    pub(crate) async fn update(
        self: &Arc<Self>,
        ep_neighbour: EpNeighbourPair,
        peer_addr: Address,
        announce_f: AnnounceFrameV1,
    ) -> Result<()> {
        let peer_ping = announce_f.origin.elapsed();
        let newly_active;
        let new_route;

        match self.meta_db.routes.get(&peer_addr.to_string()).await? {
            Some(RouteData {
                peer,
                mut link_id,
                mut link_data,
                route_id,
            }) => {
                // If there are no currently active links we mark this address
                // as "newly_active", which spawns a activity_check task below
                newly_active = link_data
                    .iter()
                    .find(|(_, entry)| entry.state == RouteState::Active)
                    .is_none();

                // Update the peer ping for this neighbour
                match link_data.get_mut(&ep_neighbour) {
                    Some(ref mut entry) => {
                        entry.ping = peer_ping;
                        entry.last_seen = Utc::now();
                        entry.state = RouteState::Active;
                        entry.data = announce_f.route;
                    }
                    None => {
                        link_data.insert(
                            ep_neighbour,
                            RouteEntry {
                                data: announce_f.route,
                                state: RouteState::Active,
                                ping: peer_ping,
                                first_seen: Utc::now(),
                                last_seen: Utc::now(),
                            },
                        );
                        link_id.push(ep_neighbour);
                    }
                }

                // Sort the available neighbours by the new ping times
                link_id.sort_by(|a, b| link_data[&a].ping.cmp(&link_data[&b].ping));

                new_route = RouteData {
                    peer,
                    link_id,
                    link_data,
                    route_id,
                };

                trace!(
                    "Update existing route to {} via neighbour {ep_neighbour:?}",
                    peer.pretty_string()
                );
            }
            None => {
                info!("Discovered new address: {}", peer_addr.pretty_string());
                newly_active = true;
                new_route = RouteData {
                    peer: peer_addr,
                    link_id: vec![ep_neighbour],
                    link_data: {
                        let mut map = BTreeMap::new();
                        map.insert(
                            ep_neighbour,
                            RouteEntry {
                                data: announce_f.route,
                                state: RouteState::Active,
                                ping: peer_ping,
                                first_seen: Utc::now(),
                                last_seen: Utc::now(),
                            },
                        );

                        map
                    },
                    route_id: Ident32::random(),
                };
            }
        }

        // Then update the caches and on-disk table
        self.meta_db
            .routes
            .insert(peer_addr.to_string(), &new_route)
            .await?;

        // If a route is declared active we want to keep checking whether it is
        // still active.  Inactive addresses don't need this since any new
        // announcement will set their state to active, which will then spawn
        // this task anyway :)
        if newly_active {
            if self.activity_tasks.read().await.get(&peer_addr).is_none() {
                Arc::clone(self).start_activity_task(peer_addr);
            }
        }

        // #[cfg(feature = "dashboard")]
        // self.metrics
        //     .routes_count
        //     .get_or_create(&metrics::RouteLabels { kind: route })
        //     .inc();

        Ok(())
    }

    fn start_activity_task(self: Arc<Self>, peer_addr: Address) {
        spawn_local(async move {
            self.activity_tasks.write().await.insert(peer_addr);
            let announce_timeout = 10;
            let sleep_time = 4;
            loop {
                // Check every 30 seconds whether the last announcement
                // is older than 1 minute.  If so, we declare the route
                // DOWN and end this task
                let check = Utc::now();
                sleep(Duration::from_secs(sleep_time)).await;
                match self.meta_db.routes.get(&peer_addr.to_string()).await {
                    Ok(Some(mut entry)) => {
                        // Iterate over all endpoints and mark those that
                        // haven't received an announcement for a while as
                        // inactive.
                        let mut all_down = true;
                        for (_, ref mut data) in entry.link_data.iter_mut() {
                            if (check - data.last_seen).num_seconds() > announce_timeout {
                                data.state = RouteState::Idle;
                            } else {
                                all_down = false;
                            }
                        }

                        if all_down {
                            // Log that the address is now inaccessible
                            info!("No announcement in >{announce_timeout} seconds: marking address {peer_addr} as IDLE");
                        }

                        if let Err(e) = self
                            .meta_db
                            .routes
                            .insert(peer_addr.to_string(), &entry)
                            .await
                        {
                            error!("failed to update activity status for peer: {e}, abort acitivy check task");
                        }
                    }
                    _ => {
                        warn!(
                            "Route entry {} has disappeared, aborting activity check task",
                            peer_addr
                        );
                        break;
                    }
                }
            }

            // Remove this task from the active set
            self.activity_tasks.write().await.remove(&peer_addr);
        });
    }

    pub(crate) async fn list_remote(self: &Arc<Self>) -> Result<Vec<PeerEntry>> {
        let this = Arc::clone(&self);
        spawn_blocking(move || {
            Ok(this
                .meta_db
                .routes
                // todo: this function loads all routes into memory, which may
                // fail on very large nodes.  But that's a future problem
                .iter()
                .into_iter()
                // Any entry that has link_data is remote
                .filter(|(_, entry)| !entry.link_data.is_empty())
                // Then construct a PeerEntry type from the available data
                .map(|(_, entry)| entry.make_peer_entry())
                .collect())
        })
        .await?
    }

    pub(crate) async fn register_local_route(&self, local: Address) -> Result<()> {
        let local_addr = RouteData::local(local);
        self.meta_db
            .routes
            .insert(local.to_string(), &local_addr)
            .await?;
        Ok(())
    }

    pub(crate) async fn scrub_local(&self, local: Address) -> Result<()> {
        self.meta_db.routes.remove(local.to_string()).await?;
        Ok(())
    }

    pub(crate) async fn is_local(&self, maybe_local: Address) -> Result<bool> {
        Ok(self
            .meta_db
            .routes
            .get(&maybe_local.to_string())
            .await?
            .ok_or(RatmanError::Nonfatal(NonfatalError::UnknownAddress(
                maybe_local,
            )))
            // Local addresses don't have route data associated
            .is_ok_and(|rd| rd.link_data.is_empty()))
    }

    /// Get the endpoint and target ID for a peer's address
    pub(crate) async fn resolve(
        &self,
        links: &Arc<LinksMap>,
        peer_addr: Address,
    ) -> Result<EpNeighbourPair> {
        let route_data = self
            .meta_db
            .routes
            .get(&peer_addr.to_string())
            .await
            .ok()
            .flatten()
            .ok_or(RatmanError::Nonfatal(NonfatalError::NoAvailableRoute))?;

        let mut scorer_state = self.solver_state.write().await;

        // We know what neighbours we are considering, and we have access to the
        // links map here.  So we fill in the available bandwidth metrics into
        // the scorer state here.
        for EpNeighbourPair(ref link_id, ref neighbour_id) in &route_data.link_id {
            match links
                .get(*link_id)
                .await
                .1
                .metrics_for_neighbour(Neighbour::Single(*neighbour_id))
                .await
            {
                Ok(metrics) => {
                    scorer_state
                        .available_bw
                        .insert(EpNeighbourPair(*link_id, *neighbour_id), metrics);
                }
                Err(e) => {
                    warn!("couldn't collect bandwidth metrics for {link_id}:{neighbour_id}: {e}");
                }
            }
        }

        // Now we iterate all the available solvers in the order they were added
        // to the route table and pass the available route data and scorer state
        // into it.  The first solver that produces a hit will resolve the
        // address.
        for idx in 0..self.solvers.len() {
            match self
                .solvers
                .get(idx)
                .unwrap()
                .compute(0, &scorer_state, &route_data)
                .await
            {
                Ok(ep) => return Ok(ep),
                Err(_) => {
                    debug!("failed to resolve route via solver id={idx}");
                }
            }
        }

        // If no solverwas able to produce a route we return an error
        Err(RatmanError::Nonfatal(NonfatalError::NoAvailableRoute))
    }

    /// Check if an ID is reachable via currently known routes
    ///
    /// - `Some(State)` indicates a remote address with a particular connection
    /// state
    ///
    /// - `None` indicates a local address, since the state is always
    /// "available".
    pub(crate) async fn reachable(&self, peer_addr: Address) -> Option<RouteState> {
        self.meta_db
            .routes
            .get(&peer_addr.to_string())
            .await
            .ok()
            .flatten()
            .and_then(|route_data| {
                route_data
                    .link_data
                    .iter()
                    .find(|(_, entry)| entry.state == RouteState::Active)
                    .map(|(_, entry)| entry.state)
            })
    }

    pub(crate) async fn neighbours(&self) -> BTreeMap<EpNeighbourPair, NeighbourEntry> {
        let neighbours_bw = &self.solver_state.read().await.available_bw.clone();
        let neighbours_buffer = &self.solver_state.read().await.available_buffer.clone();

        neighbours_bw
            .into_iter()
            .zip(neighbours_buffer.into_iter())
            .fold(BTreeMap::new(), |mut map, ((id, bw), (_, buffer))| {
                map.insert(
                    *id,
                    NeighbourEntry {
                        neighbour_id: format!("Driver#{} Device#{}", id.0, id.1),
                        neighbour_ping: Duration::from_millis(100),
                        bandwidth: bw.clone(),
                        buffer: *buffer,
                    },
                );
                map
            })
    }

    pub(crate) async fn all_entries(&self) -> BTreeMap<Address, RouteData> {
        self.meta_db
            .routes
            .iter()
            .into_iter()
            .map(|(addr, inner)| (Address::from_string(&addr), inner))
            .collect()
    }
}

#[cfg(feature = "dashboard")]
mod metrics {
    //! Metric helpers.

    use prometheus_client::{
        encoding::text::Encode,
        metrics::{family::Family, gauge::Gauge},
        registry::Registry,
    };

    #[derive(Clone, Hash, PartialEq, Eq, Encode)]
    pub(super) struct RouteLabels {
        pub kind: String,
    }

    #[derive(Default)]
    pub(super) struct RouteTableMetrics {
        pub routes_count: Family<RouteLabels, Gauge>,
    }

    impl RouteTableMetrics {
        pub fn register(&self, registry: &mut Registry) {
            registry.register(
                "ratman_routes_current",
                "Number of routes currently in the table",
                Box::new(self.routes_count.clone()),
            );
        }
    }
}
