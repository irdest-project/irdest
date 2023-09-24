// SPDX-FileCopyrightText: 2019-2022 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use crate::{
    api::{client::OnlineClient, io::Io},
    context::RatmanContext,
    util::transform,
};
use async_std::sync::Arc;
use libratman::types::{
    self,
    api::{
        self, all_peers, api_peers, api_setup, online_ack, ping, ApiMessageEnum, Peers, Peers_Type,
        Receive, Send, Setup_Type,
    },
    encode_message, Address, ClientError, Id, Message, Recipient, Result,
};

async fn handle_send(ctx: &Arc<RatmanContext>, _self: Address, send: Send) -> Result<()> {
    debug!("Queuing message to send");
    let mirror = send.mirror;
    for msg in transform::send_to_message(send) {
        let Message {
            ref id,
            ref sender,
            ref recipient,
            ref payload,
            ref time,
            ref signature,
        } = msg;

        match msg.recipient {
            Recipient::Flood(_) => {
                let recv = api::receive_default(Message::received(
                    *id,
                    *sender,
                    recipient.clone(),
                    payload.clone(),
                    format!("{:?}", time),
                    signature.clone(),
                ));

                for (_client_id, OnlineClient { ref io, ref base }) in
                    ctx.clients.online.lock().await.iter_mut()
                {
                    // Skip if recipient is self and mirror = false
                    if base.primary_address() == _self && !mirror && continue {}

                    // Otherwise try to forward the message to the given I/O socket
                    if let Err(e) = forward_recv(&io, recv.clone()).await {
                        error!("Failed to forward received message: {}", e);
                    }
                }
            }
            _ => {}
        }
        ctx.core.send(msg).await?;
    }
    Ok(())
}

async fn handle_peers(io: &Io, ctx: &Arc<RatmanContext>, peers: Peers) -> Result<()> {
    if peers.field_type != Peers_Type::REQ {
        return Ok(()); // Ignore all other messages
    }

    let all = ctx
        .core
        .all_known_addresses()
        .await
        .into_iter()
        .map(|(addr, _)| addr)
        .collect();
    let response = encode_message(api_peers(all_peers(all))).unwrap();
    io.send_to(response).await;
    Ok(())
}

async fn send_ping_response(io: &Io, _: String) -> Result<()> {
    let pong = encode_message(api_setup(ping("Howdy there!".to_owned()))).unwrap();
    io.send_to(pong).await;
    Ok(())
}

async fn send_online_ack(io: &Io, id: Address, token: Id) -> Result<()> {
    let ack = encode_message(api_setup(online_ack(id, token)))?;
    io.send_to(ack).await;
    Ok(())
}

/// Handle the initial handshake with the daemon
///
/// It either authenticates an existing client, or registers a new
/// one.  In either case, the return value will be `Ok(Some(_))`,
/// containing the newly created address and associated client token
/// (FIXME: currently `client_id` and `address` are interchangable in
/// certain parts of the API, but not others.  This needs to become
/// more consistent).
///
/// If the client wishes to remain anynomous (for example simply for
/// querying the status interfaces, but never receiving a message),
/// the return value will be `Ok(None)`.
///
/// If any error occurs during authentication, `Err(_)` is returned.
pub(crate) async fn handle_auth(
    io: &Io,
    ctx: &Arc<RatmanContext>,
) -> Result<Option<(Address, Vec<u8>)>> {
    debug!("Handle authentication request for new connection");

    // Wait for a message to come in.  Either it is
    //
    // 1. An `Online` message with attached identity
    //   - Authenticate token
    //   - Save stream for address
    // 2. An `Online` without attached identity
    //   - Assign an address
    //   - Return address and auth token
    // 3. Any other payload is invalid
    let one_of = io
        .read_message()
        .await
        .map(|msg| msg.inner)?
        .ok_or(ClientError::InvalidAuth)?;

    match one_of {
        // Anonymous clients don't get authenticated or stored
        ApiMessageEnum::setup(setup) if setup.field_type == Setup_Type::ANONYMOUS => Ok(None),

        // A client requests to go online
        ApiMessageEnum::setup(setup) if setup.field_type == Setup_Type::ONLINE => {
            let address = Address::try_from_bytes(&setup.id).ok();
            let token = Id::try_from_bytes(&setup.token).ok();

            info!("Received: ({:?}, {:?})", address, token);

            match (address, token) {
                // Both address and token were sent -> existing client
                (Some(address), Some(token)) => {
                    let client_id = match ctx.clients.get_client_for_address(&address).await {
                        Some(id) => id,
                        None => {
                            warn!("Failed to look up client_id for provided address!");
                            return Err(ClientError::InvalidAuth.into());
                        }
                    };

                    trace!("Address belongs to client_id: {}", client_id);

                    if ctx.clients.check_token(&client_id, &token).await {
                        // Set client online in both connection
                        // manager and router core
                        if let Err(e) = ctx.clients.set_online(client_id, token, io.clone()).await {
                            warn!("failed to set client as online: {:?}", e);
                            return Err(e.into());
                        }

                        if ctx.load_existing_address(address, &[0]).await.is_ok() {
                            if let Err(e) = send_online_ack(&io, address, token).await {
                                warn!("failed to send online_ack: {:?}", e);
                                return Err(e.into());
                            }
                        }

                        // Reply to the client
                        send_online_ack(&io, address, token).await?;

                        // FIXME: what is the second argument here
                        // supposed to be doing anyway ?
                        Ok(Some((address, vec![])))
                    } else {
                        Err(ClientError::InvalidAuth.into())
                    }
                }

                // Neither an address nor token were sent -> new client
                (None, None) => {
                    let address = ctx.create_new_address().await?;
                    let token = ctx.clients.register(address, io.clone()).await;

                    // Reply to the client
                    send_online_ack(io, address, token).await?;

                    // We try to write the new users out to disk, and
                    // return a non-fatal error if it fails
                    match ctx.clients.sync_users().await {
                        Ok(_) => Ok(Some((address, vec![]))),
                        Err(e) if ctx.ephemeral() => Err(e),
                        Err(_) => {
                            warn!(
                                "failed to sync address store: registered clients won't be persistent!"
                            );
                            Ok(Some((address, vec![])))
                        }
                    }
                }

                // address XOR token were sent -> invalid
                (addr, token) => {
                    warn!("Received (addr,token): ({:?}, {:?})", addr, token);
                    Err(ClientError::InvalidAuth.into())
                }
            }
        }

        ApiMessageEnum::setup(setup) if setup.field_type == Setup_Type::PING => {
            let p_target = String::from_utf8(setup.id).unwrap_or_else(|_| "0".to_string());
            debug!("Incoming ping: {}", p_target);
            send_ping_response(&io, p_target).await?;
            Ok(None)
        }

        // If the client wants to remain anonymous we don't return an ID/token pair
        ApiMessageEnum::setup(setup) if setup.field_type == Setup_Type::ANONYMOUS => {
            debug!("Authorisation for anonymous client");
            Ok(None)
        }

        // Any other payload here is invalid and we return an error
        _ => Err(ClientError::InvalidAuth.into()),
    }
}

/// Parse messages from a stream until it terminates
pub(crate) async fn parse_stream(ctx: Arc<RatmanContext>, _self: Address, io: Io) {
    loop {
        // Match on the msg type and call the appropriate handler
        match io.read_message().await.map(|msg| {
            trace!("Received message from stream {}", _self);
            msg.inner
        }) {
            // If the payload is present
            Ok(Some(one_of)) => match one_of {
                ApiMessageEnum::send(send) => handle_send(&ctx, _self, send).await,
                ApiMessageEnum::peers(peers) => handle_peers(&io, &ctx, peers).await,
                ApiMessageEnum::setup(_) => continue, // Otherwise handled during "auth"
                ApiMessageEnum::recv(_) => continue,  // Ignore "Receive" messages
            },

            // If the payload is missing
            Ok(None) => {
                warn!("Received invalid message: empty payload");
                continue;
            }

            // Other fatal errors
            Err(e) => {
                trace!("Error: {:?}", e);
                info!("API stream was dropped by client");
                break;
            }
        }
        .unwrap_or_else(|e| error!("Failed to execute command: {:?}", e));
    }
}

pub(crate) async fn forward_recv(io: &Io, r: Receive) -> Result<()> {
    let api = api::api_recv(r);
    trace!("Encoding received message...");
    let msg = types::encode_message(api)?;
    trace!("Forwarding payload through stream");
    io.send_to(msg).await;
    Ok(())
}
