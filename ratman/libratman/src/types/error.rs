// SPDX-FileCopyrightText: 2019-2023 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use super::{Ident32, InMemoryEnvelope};
use crate::types::Address;
use async_eris::BlockReference;
use hex::FromHexError;
use serde::{Deserialize, Serialize};
use std::net::AddrParseError;
use tokio::{io, time::error::Elapsed};

/// An Irdest-wide Result capable of expressing many different states
pub type Result<T> = std::result::Result<T, RatmanError>;

/// A central error facade for Ratman and tools
///
/// Every error must have a namespace.  When mapping an existing error
/// type into RatmanError care should be taken to not overload a
/// particular domain.  For example: having a single I/O error makes
/// sense, instead of having.  However, consider giving it a secondary
/// Error, to express the context that a particular "base error"
/// originated from.
///
/// The special `Nonfatal` type should be used for errors that don't
/// impact the runtime of the Router.  These could be taken as
/// "suboptimal event status messages".
#[repr(C)]
#[derive(Debug, thiserror::Error)]
pub enum RatmanError {
    #[error("a non-fatal error {0}")]
    Nonfatal(#[from] self::NonfatalError),
    #[error("an i/o error: {0}")]
    Io(io::Error),
    #[error("an i/o error: {0}")]
    TokioIo(#[from] tokio::io::Error),
    #[error("a threading error: {0}")]
    TokioJoin(#[from] tokio::task::JoinError),
    #[error("a frame parsing error: {0}")]
    Encoding(#[from] self::EncodingError),
    #[error("microframe failed to decode: {0}")]
    Microframe(#[from] self::MicroframeError),
    #[error("a json encoding error: {0}")]
    Json(#[from] serde_json::error::Error),
    #[cfg(feature = "client")]
    #[error("a client API error: {0}")]
    ClientApi(#[from] self::ClientError),
    #[cfg(feature = "netmod")]
    #[error("a netmod error: {0}")]
    Netmod(#[from] self::NetmodError),
    #[error("a block error: {0}")]
    Block(#[from] self::BlockError),
    #[error("a scheduling error: {0}")]
    Schedule(#[from] self::ScheduleError),
    #[error("a storage error: {0}")]
    Storage(#[from] fjall::Error),
    #[error("web engine error: {0}")]
    WebDashboardError(#[from] axum::Error),
    #[error("failed to acquire state directory lock")]
    StateDirectoryAlreadyLocked,
    #[error("{0}")]
    User(#[from] UserError),
    // #[cfg(all(feature = "daemon", target_family = "unix"))]
    // #[error("a unix system error: {0}")]
    // UnixSystem(#[from] nix::errno::Errno),
}

impl From<RatmanError> for io::Error {
    fn from(e: RatmanError) -> Self {
        match e {
            RatmanError::Io(e) => e,
            e => panic!("unexpected IPC error: {}", e),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NonfatalError {
    #[error("ratman is running ephemaral mode: no data will be persisted to disk!")]
    IsEphemeral,
    #[error("the current MTU of a netmod channel is too small to fit the desired frame")]
    MtuTooSmallForFrame,
    #[error("the frame couldn't be parsed as the type it was expected to be")]
    MismatchedEncodingTypes,
    #[error("the stream or buffer didn't have any data at this time")]
    NoData,
    #[error("a stream is already in progress")]
    OngoingStream,
    #[error("no stream is in progress")]
    NoStream,
    #[error("requested address {0} is unknown")]
    UnknownAddress(Address),
    #[error("requested metrics were not available")]
    NoMetrics,
    #[error("requested address could not be routed to at this moment")]
    NoAvailableRoute,
}

#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("structure had invalid version number {0}")]
    InvalidVersion(u8),
    #[error("incoming stream could not be parsed because {0}")]
    Parsing(String),
    #[error("encryption encoding/decoding failed because {0}")]
    Encryption(String),
    #[error("internal encoding/decoding failed because {0}")]
    Internal(String),
    #[error(
        "provided frame is too large to fit into the {} size envelope: {0}",
        core::u16::MAX
    )]
    FrameTooLarge(usize),
    #[error("provided buffer did not contain any data")]
    NoData,
    #[error("provided data could not be hex-decoded")]
    InvalidHexEncoding(String),
}

impl From<FromHexError> for EncodingError {
    fn from(hex: FromHexError) -> Self {
        Self::InvalidHexEncoding(hex.to_string())
    }
}

// fixme? shouldn't this be implemented for RatmanError
impl<T: std::fmt::Debug> From<nom::Err<T>> for EncodingError {
    fn from(nom: nom::Err<T>) -> Self {
        Self::Parsing(format!("{}", nom))
    }
}

impl<T: std::fmt::Debug> From<nom::Err<T>> for RatmanError {
    fn from(nom: nom::Err<T>) -> Self {
        Self::Encoding(EncodingError::from(nom))
    }
}

impl From<bincode::Error> for RatmanError {
    fn from(bc: bincode::Error) -> Self {
        Self::Encoding(EncodingError::Internal(bc.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BlockError {
    #[error("provided data block had an invalid length: {0}")]
    InvalidLength(usize),
    #[error("provided data block integrity could not be verified (expected reference {expected}, got {got})")]
    InvalidReference {
        expected: BlockReference,
        got: BlockReference,
    },
    #[error("ERIS block decoding failed because {0}")]
    Eris(#[from] async_eris::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ScheduleError {
    #[error("a timeout limit was reached after {0}")]
    Timeout(#[from] Elapsed),
    #[error("contention around resource {0} is leading to slowdown")]
    Contention(String),
}

/// Client API errors beetw Ratman and an application
///
/// The client API consists of an authentication handshake, message
/// sending and receiving, and simple state management (for
/// subscriptions, tokens, etc).
///
/// Importantly, more base-type errors (such as I/O and encoding) are
/// handled by [RatmanError](crate::RatmanError) instead!
#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum ClientError {
    #[error("ratman-client ({0}) and router ({1}) have incompatible versions")]
    IncompatibleVersion(String, String),
    #[error("failed to provide correct authentication in handshake")]
    InvalidAuth,
    #[error("connection was unexpectedly dropped")]
    ConnectionLost,
    #[error("operation not supported")]
    NotSupported,
    #[error("requested an unknown address")]
    NoAddress,
    #[error("address already exists in routing table")]
    DuplicateAddress,
    #[error("internal server error: {0}")]
    Internal(String),
    #[error("requested subscrition ({0}) does not exist")]
    NoSuchSubscription(Ident32),
    #[error("bad user input data: {0}")]
    User(#[from] UserError),
}

/// Any error that can occur when interacting with a netmod driver
#[derive(Debug, thiserror::Error)]
pub enum NetmodError {
    #[error("the requested operation is not supported by the netmod")]
    NotSupported,
    #[error("frame is too large to send through this channel")]
    FrameTooLarge,
    /// Connection was dropped during transmission
    ///
    /// Returns the payload to be queued in the journal.  Do not use this type
    /// for receiving action
    #[error("peering connection was lost mid-transfer")]
    ConnectionLost(InMemoryEnvelope),
    #[error("unable to receive new data since the local socket has closed")]
    RecvSocketClosed,
    #[error("the provided peer '{}' was invalid!", 0)]
    InvalidPeer(String),
    /// An error type for a netmod that tries to bind any resource
    #[error("failed to setup netmod bind: {}", 0)]
    InvalidBind(String),
}

impl From<AddrParseError> for NetmodError {
    fn from(err: AddrParseError) -> Self {
        Self::InvalidBind(err.to_string())
    }
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum MicroframeError {
    #[error("invalid mode: (ns: {0}, op: {1})")]
    InvalidMode(u8, u8),
    #[error("failed to read a full microframe: timeout")]
    ReadTimeout,
    #[error("failed to read a valid cstring from input")]
    InvalidString,
    #[error("failed to parse type because of missing fields: {:?}", 0)]
    MissingFields(&'static [&'static str]),
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum UserError {
    InvalidInput(String, Option<String>),
    MissingInput(String),
    RecvLimitReached,
}

// We manually implement Display for the user error to include some more
// specific output, considering these messages are shown to end-users.
use core::fmt;
impl fmt::Display for UserError {
    fn fmt(&self, w: &mut fmt::Formatter) -> fmt::Result {
        write!(
            w,
            "{}",
            match self {
                Self::InvalidInput(got, Some(expected)) =>
                    format!("got invalid input '{got}' (expected '{expected}')"),
                Self::InvalidInput(got, None) => format!("got invalid input '{got}'"),
                Self::MissingInput(expected) => format!("required input '{expected}' was missing"),
                Self::RecvLimitReached =>
                    format!("this stream generator has reached its end and should be dropped!"),
            }
        )
    }
}
