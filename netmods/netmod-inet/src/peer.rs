// SPDX-FileCopyrightText: 2019-2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use std::net::SocketAddr;
use std::sync::Arc;

use crate::session::{SessionData, SessionError};
use crate::{proto, routes::Target};
use libratman::tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use libratman::tokio::{
    sync::Mutex,
    task::{spawn, yield_now},
};
use libratman::types::Ident32;
use libratman::{
    tokio::sync::mpsc::{Receiver, Sender},
    types::InMemoryEnvelope,
    EncodingError, RatmanError,
};
use useful_netmod_bits::metrics::MetricsTable;

pub(crate) type FrameReceiver = Receiver<(Ident32, InMemoryEnvelope)>;
pub(crate) type FrameSender = Sender<(Ident32, InMemoryEnvelope)>;

/// Represent another node running netmod-inet
///
/// A peer is represented by a pair of socket addresses, and two
/// sockets.  A peer runs an incoming packet socket via `peer.run()`
/// and can send messages via `peer.send(...)`
///
/// There are two peering modes: `standard` and `cross`.  They specify
/// the way that connections are established, and how connection drops
/// are handled.
///
/// ## Types of guys
///
/// 1. Peer is set to connect to a remote via standard connection
///
///    In this mode the peer creates a single outgoing connection, and
///    uses the same stream for sending and receiving messages.  When
///    the peer disconnects, it is responsible for re-connecting.  The
///    "server" will drop the peer and not re-connect (because it
///    doesn't know how).
///
///
/// 2. Peer is set to connect to a remote via cross connection
///
///    In this mode the peer creates a single outgoing connection, and
///    is upgraded with an incoming connection for receiving, which is
///    established by the remote.  In this model there is no "server"
///    and thus in the case of a connection drop, either side can
///    re-establish the connection without causing a race-condition.
///
/// The two inverse scenarios exist on the "server" side.
pub struct Peer {
    pub(crate) session: SessionData,
    tx: Mutex<Option<OwnedWriteHalf>>,
    rx: Mutex<Option<OwnedReadHalf>>,
    receiver: FrameSender,
    restart: Option<Sender<SessionData>>,
}

impl Peer {
    /// Connect to a peer via "standard" connection
    pub(crate) fn standard(
        session: SessionData,
        receiver: FrameSender,
        restart: Option<Sender<SessionData>>,
        tx: OwnedWriteHalf,
        rx: OwnedReadHalf,
    ) -> Arc<Self> {
        Arc::new(Self {
            session,
            tx: Mutex::new(Some(tx)),
            rx: Mutex::new(Some(rx)),
            receiver,
            restart,
        })
    }

    /// Return this Peer's ID
    #[inline]
    pub(crate) fn id(&self) -> Target {
        self.session.id
    }

    /// Send a frame to this peer
    ///
    /// If the sending fails for any reason, the underlying
    /// `SessionData` is returned so that a new session may be
    /// started.
    pub(crate) async fn send(
        self: &Arc<Self>,
        env: &InMemoryEnvelope,
    ) -> Result<usize, SessionError> {
        trace!(
            "Sending data for '{}'",
            match env.header.get_seq_id() {
                Some(seq_id) => format!("{}", seq_id.hash),
                None => format!("<???>"),
            }
        );
        let mut txg = self.tx.lock().await;

        // The TcpStream SHOULD never just disappear
        let tx = txg.as_mut().unwrap();
        match proto::write(&mut *tx, env).await {
            Ok(bytes_written) => Ok(bytes_written),
            Err(e) => {
                warn!("Failed to send data for peer {}: {e:?}", self.session.id);

                // If we are the outgoing side we signal to be restarted
                if let Some(ref tx) = self.restart {
                    if let Err(e) = tx.send(self.session).await {
                        error!("failed to forward peer restart signal: {e:?}");
                    }
                    debug!("Notify restart hook");
                    Ok(0)
                }
                // Else we just inform the sending context that this
                // has failed.  On the server side we then remove this
                // peer from the routing table and insert a temp
                // buffer instead.
                else {
                    Err(SessionError::Dropped(self.session.addr))
                }
            }
        }
    }

    /// Repeatedly attempt to read from the reading socket
    pub(crate) async fn run(self: Arc<Self>, metrics: Arc<MetricsTable<SocketAddr>>) {
        let mut no_data_ctr = 0;

        loop {
            trace!("Peer::run loop for {:?}", self.session);
            let mut rxg = self.rx.lock().await;
            let rx = match rxg.as_mut() {
                Some(rx) => rx,
                None => {
                    warn!("Peer {:?} became invalid", self.session);
                    break;
                }
            };

            let envelope = match proto::read(rx).await {
                Ok(f) => {
                    no_data_ctr = 0;
                    f
                }
                Err(RatmanError::Encoding(EncodingError::NoData)) => {
                    no_data_ctr += 1;
                    drop(rxg);

                    if no_data_ctr > 128 {
                        break;
                    } else {
                        yield_now().await;
                        continue;
                    }
                }
                Err(RatmanError::Io(io)) => {
                    error!(
                        "Peers {} encountered I/O error during receiving: {}",
                        self.id(),
                        io
                    );

                    // If we were the outgoing peer we signal to re-connect
                    if let Some(ref tx) = self.restart {
                        tx.send(self.session).await.unwrap();
                    }

                    break;
                }
                _ => unreachable!(),
            };

            {
                // Spawn the metrics update on a local task to not block this loop
                let metrics2 = Arc::clone(&metrics);
                let peer_addr = self.session.addr;
                let bytes_read = envelope.buffer.len();
                spawn(async move { metrics2.append_read(peer_addr, bytes_read).await });
            }

            // If we received a correct frame we forward it to the receiver
            self.receiver
                .send((self.session.peer_router_key_id, envelope))
                .await
                .unwrap();
        }

        trace!("Exit receive loop for peer {}", self.id());
    }
}
