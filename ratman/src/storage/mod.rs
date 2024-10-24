//! This module handles several storage engines.  The backing database is fjall, a
//!
//! - Block storage: keep track of full blocks
//!
//! - Frame storage: keep track of in-flight frames that don't fully assemble a
//! block (yet)
//!
//! - Peer metadata: persistent routing tables
//!
//! -

use crate::{
    journal::page::CachePage,
    storage::{
        addr_key::AddressData, block::IncompleteBlockData, link::NeighbourData, route::RouteData,
        subs::SubscriptionData,
    },
};
use fjall::{Keyspace, PartitionCreateOptions};
use libratman::{
    types::{Ident32, LetterheadV1, Namespace},
    Result,
};
use std::marker::PhantomData;

pub mod addr_key;
pub mod block;
pub mod link;
pub mod route;
pub mod subs;

/// Metadata database handle
///
/// This database keeps track of various bits of metadata which aren't directly
/// related to the flow of message streams through an Irdest network.
///
/// - Registered addresses and their encrypted private key information
///
/// - Routing table: keep track of known peers via their links and various
/// metrics like MTU, uptime, and average ping.
///
/// - Link metadata: certain message streams have associations between them, or
/// can be tagged with additional information for importance to prevent them
/// from being cleaned from the journal in case of storage quota limits.
// todo: currently this type handles some encryption operations directly even
// though there's no real reason for this.  Maybe move this code to the crypto
// module?
pub struct MetadataDb {
    pub db: Keyspace,
    pub addrs: CachePage<AddressData>,
    pub routes: CachePage<RouteData>,
    pub neighbours: CachePage<NeighbourData>,
    pub incomplete: CachePage<IncompleteBlockData>,
    pub available_streams: CachePage<LetterheadV1>,
    pub subscriptions: CachePage<SubscriptionData>,
}

impl MetadataDb {
    pub fn router_id(&self) -> Ident32 {
        let part = self
            .db
            .open_partition("meta_meta", PartitionCreateOptions::default())
            .expect("failed to open meta_meta partition; can't generate router id key! :(");

        if let Some(router_key_id) = part.get("router.key_id").unwrap() {
            Ident32::from_bytes(&*router_key_id)
        } else {
            let router_key_id = Ident32::random();
            part.insert("router.key_id", router_key_id.as_bytes())
                .expect("failed to insert router id");
            router_key_id
        }
    }

    pub fn new(db: Keyspace) -> Result<Self> {
        let addrs = CachePage(
            db.open_partition("meta_addrs", PartitionCreateOptions::default())?,
            PhantomData,
        );
        let routes = CachePage(
            db.open_partition("meta_routes", PartitionCreateOptions::default())?,
            PhantomData,
        );
        let neighbours = CachePage(
            db.open_partition("meta_neighbours", PartitionCreateOptions::default())?,
            PhantomData,
        );
        let incomplete = CachePage(
            db.open_partition("meta_incomplete", PartitionCreateOptions::default())?,
            PhantomData,
        );
        let available_streams = CachePage(
            db.open_partition("meta_available_streams", PartitionCreateOptions::default())?,
            PhantomData,
        );
        let subscriptions = CachePage(
            db.open_partition("meta_subscriptions", PartitionCreateOptions::default())?,
            PhantomData,
        );

        Ok(Self {
            db,
            addrs,
            routes,
            neighbours,
            incomplete,
            available_streams,
            subscriptions,
        })
    }
}
