//! Client API protocol type definitions

mod addr;
mod contact;
mod link;
mod namespace;
mod peer;
mod recv;
mod send;

pub use addr::*;
use byteorder::{BigEndian, ByteOrder};
pub use contact::*;
pub use link::*;
pub use namespace::*;
pub use peer::*;
pub use recv::*;
pub use send::*;
use serde::{Deserialize, Serialize};

use crate::{
    frame::{
        generate::generate_cstring,
        micro::parse::vec_of,
        parse::{self, take_cstring, take_id, take_u32, take_u64},
        FrameGenerator, FrameParser,
    },
    types::{Address, Ident32},
    ClientError, EncodingError, Result,
};
use core::fmt;
use nom::{bytes::complete::take, IResult};
use std::{ffi::CString, fmt::Display};

/// Sent from the router to the client when a client connects
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handshake {
    /// Indicate to the client which version of the protocol is used
    ///
    /// A client that connects with an older version MUST print an
    /// error to the user, indicating that the tools version they are
    /// using is not compatible with the Router version.
    pub client_version: [u8; 2],
}

impl Handshake {
    pub fn new() -> Self {
        Self {
            client_version: super::VERSION,
        }
    }
}

impl FrameGenerator for Handshake {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        buf.push(1);
        buf.extend_from_slice(self.client_version.as_slice());
        Ok(())
    }
}

impl FrameParser for Handshake {
    type Output = Self;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, version) = parse::take_byte(input).unwrap();
        assert_eq!(version, 1);
        let (input, proto_version) = parse::take(2 as usize)(input)?;

        Ok((
            input,
            Self {
                client_version: proto_version.try_into().expect("wat??"),
            },
        ))
    }
}

/// Router-client ping and response type
// todo: this is a big kitchen sink of a protocol type and I hate it.  These
// should not exist and instead be their own types.  The header should contain
// in its mode field whether it contains an error, and then the client can
// handle it that way.
#[derive(Debug)]
pub enum ServerPing {
    /// A generic "everything is good" response
    Ok,
    /// Indicate that subscriptions have data available
    ///
    /// This is only the case when the subscription is currently idle, meaning
    /// the client is not actively listening to events on the given subscription
    /// socket.  Active subscriptions are not included in this set!
    Update {
        available_subscriptions: Vec<Ident32>,
    },
    /// Communicate some kind of API error to the calling client
    Error(ClientError),
    ///
    IncompatibleVersion {
        router: CString,
        client: CString,
    },
    /// Connection timed out
    Timeout,
    /// Subscription response type
    Subscription {
        sub_id: Ident32,
        sub_bind: CString,
    },
    /// A list of addresses, either local or remote
    AddrList(Vec<Address>),
    /// A list of peer entries
    PeerList(PeerList),
    /// Indicate that a client should connect to a separate socket to input a data stream
    SendSocket {
        socket_bind: CString,
    },
    Status {
        num_peers: u64,
        num_local: u64,
        num_auth: u64,
        num_collector_workers: u64,
    },
    Anycast(Vec<(Address, u64)>),
}

#[derive(Serialize, Deserialize)]
pub struct RouterStatus {
    pub num_peers: u64,
    pub num_local: u64,
    pub num_auth: u64,
    pub num_collector_workers: u64,
}

impl Display for RouterStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "known peers: {}, local addrs: {}, active auths: {}, collector workers: {}",
            self.num_peers, self.num_local, self.num_auth, self.num_collector_workers
        )
    }
}

impl FrameGenerator for ServerPing {
    #[tracing::instrument]
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        match self {
            Self::Ok => buf.push(1),
            Self::Update {
                available_subscriptions,
            } => {
                buf.push(2);
                available_subscriptions.generate(buf)?;
            }
            Self::Error(error) => {
                buf.push(3);
                let mut err_buf = bincode::serialize(&error)?;

                let mut len_buf = vec![0; 4];
                BigEndian::write_u32(len_buf.as_mut_slice(), err_buf.len() as u32);

                trace!("Ping::Error(len_buf) = {:?}", len_buf);
                trace!("Ping::Error(err_buf) = {:?}", err_buf);

                buf.append(&mut len_buf);
                buf.append(&mut err_buf);
            }
            Self::Timeout => buf.push(4),
            Self::IncompatibleVersion { router, client } => {
                buf.push(5);
                generate_cstring(router, buf)?;
                generate_cstring(client, buf)?;
            }
            Self::Subscription { sub_id, sub_bind } => {
                buf.push(6);
                Some(sub_id).generate(buf)?;
                generate_cstring(sub_bind, buf)?;
            }
            Self::AddrList(list) => {
                buf.push(7);
                list.generate(buf)?;
            }
            Self::PeerList(list) => {
                buf.push(8);
                list.generate(buf)?;
            }
            Self::SendSocket { socket_bind } => {
                buf.push(9);
                generate_cstring(socket_bind, buf)?;
            }
            Self::Status {
                num_peers,
                num_local,
                num_auth,
                num_collector_workers,
            } => {
                buf.push(10);
                num_peers.generate(buf)?;
                num_local.generate(buf)?;
                num_auth.generate(buf)?;
                num_collector_workers.generate(buf)?;
            }
            Self::Anycast(list) => {
                buf.push(11);
                list.generate(buf)?;
            }
        }

        Ok(())
    }
}

impl FrameParser for ServerPing {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (mut input, tt) = parse::take_byte(input)?;

        let output = match tt {
            1 => Ok(Self::Ok),
            2 => {
                let (input_, available_subscriptions) = vec_of(take_id, input)?;
                input = input_; // wish we didn't need this weirdness
                Ok(Self::Update {
                    available_subscriptions,
                })
            }
            3 => {
                let (input_, err_len) = take_u32(input)?;
                let (input_, err_buf) = take(err_len as usize)(input_)?;

                // println!("Ping::Error(len_buf) = {:?}", err_len);
                // println!("Ping::Error(err_buf) = {:?}", err_buf);

                let err = bincode::deserialize(&err_buf).unwrap();
                input = input_;
                Ok(Self::Error(err))
            }
            4 => Ok(Self::Timeout),
            5 => {
                let (input_, router) = take_cstring(input)?;
                let (input_, client) = take_cstring(input_)?;
                input = input_;
                Ok(Self::IncompatibleVersion {
                    router: router.expect("failed to decode version string"),
                    client: client.expect("failed to decode version string"),
                })
            }
            6 => {
                let (input_, sub_id) = take_id(input)?;
                let (input_, sub_bind) = take_cstring(input_)?;
                input = input_;
                sub_bind.map(|sub_bind| Self::Subscription { sub_id, sub_bind })
            }
            7 => {
                let (input_, list) = Vec::<Address>::parse(input)?;
                input = input_;
                Ok(Self::AddrList(list))
            }
            8 => {
                let (input_, list) = PeerList::parse(input)?;
                input = input_;
                list.map(|list| Self::PeerList(list))
            }
            9 => {
                let (input_, send_bind) = take_cstring(input)?;
                input = input_;
                send_bind.map(|socket_bind| Self::SendSocket { socket_bind })
            }
            10 => {
                let (input_, num_peers) = take_u64(input)?;
                let (input_, num_local) = take_u64(input_)?;
                let (input_, num_auth) = take_u64(input_)?;
                let (input_, num_collector_workers) = take_u64(input_)?;
                input = input_;
                Ok(Self::Status {
                    num_peers,
                    num_local,
                    num_auth,
                    num_collector_workers,
                })
            }
            11 => {
                let (input_, list) = Vec::<(Address, u64)>::parse(input)?;
                input = input_;
                Ok(Self::Anycast(list))
            }
            _ => Err(EncodingError::Parsing(format!("Invalid ServerPing type={}", tt)).into()),
        };

        Ok((input, output))
    }
}
