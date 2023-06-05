// SPDX-FileCopyrightText: 2019-2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use async_std::sync::{Arc, RwLock};
use std::collections::BTreeSet;
use types::{Frame, Id};

/// Remote frame journal
pub(crate) struct Journal {
    /// Keeps track of known frames to do reflood
    known: RwLock<BTreeSet<Id>>,
}

impl Journal {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            known: Default::default(),
        })
    }

    /// Dispatches a long-running task to run the journal logic
    pub(crate) fn run(self: Arc<Self>) {
        // task::spawn(async move { loop {} });
    }

    /// Add a new frame to the known set
    pub(crate) async fn queue(&self, _: Frame) {}

    /// Save a FrameID in the known journal page
    #[allow(unused)]
    pub(crate) async fn save(&self, fid: &Id) {
        self.known.write().await.insert(fid.clone());
    }

    /// Checks if a frame ID has not been seen before
    pub(crate) async fn unknown(&self, fid: &Id) -> bool {
        !self.known.read().await.contains(fid)
    }
}
