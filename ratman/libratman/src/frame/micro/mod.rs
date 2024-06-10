//! A minimalist framing mechanism
//!
//! Each frame is split into two parts: the header and the body.  The
//! header consists of a 16-bit mode field, an optional 32-byte
//! authentication key (or 1 zero byte) and a 32-bit unsigned integer
//! payload size.
//!
//! When reading a Microframe from a socket first read a complete
//! header, then read the rest of the frame payload.

mod error;
pub mod parse;

use crate::{
    frame::{parse as fparse, FrameGenerator, FrameParser},
    types::ClientAuth,
    Result,
};
use nom::IResult;

#[rustfmt::skip]
pub mod client_modes {

    //// List of mode namespaces that are available

    /// Namespace for basic handshake and client-router communication
    pub const INTRINSIC: u8 = 0x0;
    /// Local addresses
    pub const ADDR: u8      = 0x1;
    /// Per-address contact book
    pub const CONTACT: u8   = 0x2;
    /// Active router-to-router links
    pub const LINK: u8      = 0x3;
    /// Peers on the network
    pub const PEER: u8      = 0x4;
    /// Receive mode
    pub const RECV: u8      = 0x5;
    /// Send mode
    pub const SEND: u8      = 0x6;
    /// General status queries
    pub const STATUS: u8    = 0x7;
    /// Stream and namespace subscriptions
    pub const SUB: u8       = 0x8;

    //// Creating new data or destroying it permanently
    pub const CREATE: u8    = 0x1;
    pub const DESTROY: u8   = 0x2;

    //// Changing the uptime state of a component
    pub const UP: u8        = 0x3;
    pub const DOWN: u8      = 0x4;

    //// Add and delete are reversible, and re-appliable
    pub const ADD: u8       = 0x5;
    pub const DELETE: u8    = 0x6;
    pub const MODIFY: u8    = 0x7;

    //// A bunch of modes I forgot why we needed
    pub const LIST: u8      = 0x10;
    pub const QUERY: u8     = 0x11;
    pub const ONE: u8       = 0x12;
    pub const MANY: u8      = 0x13;
    pub const FLOOD: u8     = 0x14;
    pub const FETCH: u8     = 0x15;
    pub const SYSTEM: u8    = 0x16;
    pub const OP_ADDR: u8   = 0x17;
    pub const OP_LINK: u8   = 0x18;


    /// Assemble a full mode byte from a command namespace and a
    /// compatible operator.  Not all mode encodings are valid and may
    /// be rejected by the remote.
    pub const fn make(ns: u8, op: u8) -> u16 {
        ((ns as u16) << 8) as u16 | op as u16
    }

    // todo: add a better test here

    #[test]
    fn test_addr_create() {
        let mode = make(ADDR, CREATE);
        println!("{:#018b}", mode);
        assert_eq!(mode, 257);
    }
}

/// Metadata header for a Microframe
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct MicroframeHeader {
    pub modes: u16,
    pub auth: Option<ClientAuth>,
    pub payload_size: u32,
}

impl MicroframeHeader {
    pub fn intrinsic_noauth() -> Self {
        Self {
            modes: client_modes::make(client_modes::INTRINSIC, client_modes::INTRINSIC),
            auth: None,
            payload_size: 0,
        }
    }

    pub fn intrinsic_auth(auth: ClientAuth) -> Self {
        Self {
            modes: client_modes::make(client_modes::INTRINSIC, client_modes::INTRINSIC),
            auth: Some(auth),
            payload_size: 0,
        }
    }
}

impl FrameGenerator for MicroframeHeader {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.modes.generate(buf)?;
        self.auth.generate(buf)?;
        self.payload_size.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for MicroframeHeader {
    type Output = Result<Self>;

    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, modes) = fparse::take_u16(input)?;
        let (input, auth) = ClientAuth::parse(input)?;
        let (input, payload_size) = fparse::take_u32(input)?;

        Ok((
            input,
            Ok(MicroframeHeader {
                modes,
                auth,
                payload_size,
            }),
        ))
    }
}

/// Creates a Microframe from
///
/// - message modes
/// - an optional client auth token
/// - an optional inner message payload
pub fn encode_micro_frame<T: FrameGenerator>(
    modes: u16,
    auth: Option<ClientAuth>,
    payload: Option<T>,
) -> Result<Vec<u8>> {
    let mut payload_buf = vec![];
    match payload {
        Some(p) => p.generate(&mut payload_buf)?,
        None => {}
    };

    let header = MicroframeHeader {
        modes,
        auth,
        payload_size: payload_buf.len() as u32,
    };

    let mut complete = vec![];
    header.generate(&mut complete)?;
    complete.append(&mut payload_buf);

    Ok(complete)
}
