// SPDX-FileCopyrightText: 2023 Katharina Fey <kookie@spacekookie.de>
//
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-AppStore

use crate::{
    frame::{parse, FrameGenerator, FrameParser},
    types::identifiers::{id::Ident32, ID_LEN},
    Result,
};
use nom::IResult;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

#[cfg(feature = "metrics")]
use prometheus_client::encoding::text::Encode;

/// An ed22519 pubkey which represents a single address on the Irdest network
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address(pub Ident32);

/// A namespace address
pub type Namespace = Address;

impl Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Address {
    /// Peel an address down to a simple Id
    pub fn peel(self) -> Ident32 {
        self.0
    }

    /// This function is only exposed for testing purposes
    #[doc(hidden)]
    pub fn random() -> Self {
        Self(Ident32::random())
    }

    /// Parse a string representation of an address
    pub fn from_string(s: &String) -> Self {
        Self(Ident32::from_string(s))
    }

    pub fn pretty_string(&self) -> String {
        self.0.pretty_string()
    }

    /// create an address from a byte slice
    ///
    /// If the slice is not long enough (or too long), an appropriate
    /// error will be returned, instead of panicking.
    pub fn try_from_bytes(buf: &[u8]) -> Result<Self> {
        Ident32::try_from_bytes(buf).map(|id| Self(id))
    }

    /// Expand a slice of bytes into an address
    ///
    /// This function will panic if not enough bytes were provided
    pub fn from_bytes(buf: &[u8]) -> Self {
        Self(Ident32::from_bytes(buf))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Copy the contents out as a slice
    #[inline]
    pub fn slice(&self) -> [u8; ID_LEN] {
        self.0.slice()
    }
}

/// Implement RAW `From` binary array
impl From<[u8; ID_LEN]> for Address {
    fn from(i: [u8; ID_LEN]) -> Self {
        Self(i.into())
    }
}

/// Implement RAW `From` binary (reference) array
impl From<&[u8; ID_LEN]> for Address {
    fn from(i: &[u8; ID_LEN]) -> Self {
        Self(i.clone().into())
    }
}

#[cfg(feature = "metrics")]
impl Encode for Address {
    fn encode(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        self.0.encode(w)
    }
}

impl FrameGenerator for Address {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        buf.extend_from_slice(self.as_bytes());
        Ok(())
    }
}

impl FrameGenerator for Option<Address> {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        // If the Address is None we simply push a zero-byte
        match self {
            Some(id) => buf.extend_from_slice(id.as_bytes()),
            None => buf.push(0),
        }

        Ok(())
    }
}

impl FrameParser for Address {
    type Output = Self;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        parse::take_address(input)
    }
}
