// SPDX-FileCopyrightText: 2019-2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

//! `netmod-mem` is an in-memory `netmod` endpoint
//!
//! This aims to make testing any structure that binds against
//! `netmod` easier and reproducible.

#![doc(html_favicon_url = "https://qaul.org/favicon.ico")]
#![doc(html_logo_url = "https://qaul.org/img/qaul_icon-128.png")]

use async_std::{
    sync::{Arc, RwLock},
    task,
};
use async_trait::async_trait;
use ratman_netmod::{Endpoint, Error as NetError, Frame, Result as NetResult, Target};

/// An input/output pair of `mpsc::channel`s.
///
/// This is the actual mechanism by which data is moved around between `MemMod`s in
/// different places.
pub(crate) mod io;
// Simulated transmission media.
// pub mod media;

/// Represents a single netmod endpoint that can connect to exactly one other, either
/// as a 1-to-1 link between libqaul instances or as a link into a transmission
/// medium of some kind.
pub struct MemMod {
    /// Internal memory access to send/receive
    io: Arc<RwLock<Option<io::Io>>>,
}

impl MemMod {
    /// Create a new, unpaired `MemMod`.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            io: Default::default(),
        })
    }

    /// Create two already-paired `MemMod`s, ready for use.
    pub fn make_pair() -> (Arc<Self>, Arc<Self>) {
        let (a, b) = (MemMod::new(), MemMod::new());
        a.link(&b);
        (a, b)
    }

    /// Return `true` if the MemMod is linked to another one or
    /// `false` otherwise.
    pub fn linked(&self) -> bool {
        task::block_on(async { self.io.read().await.is_some() })
    }

    /// Establish a 1-to-1 link between two `MemMod`s.
    ///
    /// # Panics
    ///
    /// Panics if this MemMod, or the other one, is already linked.
    pub fn link(&self, pair: &MemMod) {
        if self.linked() || pair.linked() {
            panic!("Attempted to link an already linked MemMod.");
        }
        let (my_io, their_io) = io::Io::make_pair();

        self.set_io_async(my_io);
        pair.set_io_async(their_io);
    }

    /// Remove the connection between MemMods.
    pub fn split(&self) {
        // The previous value in here will now be dropped,
        // so future messages will fail.
        self.set_io_async(None);
    }

    fn set_io_async<I: Into<Option<io::Io>>>(&self, val: I) {
        task::block_on(async { *self.io.write().await = val.into() });
    }
}

#[async_trait]
impl Endpoint for MemMod {
    /// Provides maximum frame-size information to `RATMAN`
    fn size_hint(&self) -> usize {
        ::std::u32::MAX as usize
    }

    /// Send a message to a specific endpoint (client)
    ///
    /// # Errors
    ///
    /// Returns `OperationNotSupported` if attempting to send through
    /// a connection that is not yet connected.
    async fn send(&self, frame: Frame, _: Target, exclude: Option<u16>) -> NetResult<()> {
        let io = self.io.read().await;
        match *io {
            None => Err(NetError::NotSupported),
            Some(ref io) if exclude.is_none() => Ok(io.out.send(frame).await.unwrap()),
            _ => Ok(()), // when exclude is some we just drop the frame
        }
    }

    async fn next(&self) -> NetResult<(Frame, Target)> {
        let io = self.io.read().await;
        match *io {
            None => Err(NetError::NotSupported),
            Some(ref io) => match io.inc.recv().await {
                Ok(f) => Ok((f, Target::default())),
                Err(_) => Err(NetError::ConnectionLost),
            },
        }
    }
}
