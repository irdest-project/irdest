use crate::{
    frame::{
        micro::parse::*,
        parse::{take_address, take_byte, take_datetime},
        FrameGenerator, FrameParser,
    },
    types::{Address, TrustFilter},
    Result,
};
use chrono::{DateTime, Utc};
use core::fmt;
use nom::IResult;
use serde::{Deserialize, Serialize};
use std::{ffi::CString, fmt::Display};

/// Query existing peers
#[repr(C)]
pub struct PeerQuery {
    /// A regex filter that will have to pass on the peer note contents
    pub note_filter: Option<CString>,
    /// A filter for the existence of a tag for said peer
    pub tag_filter: Vec<CString>,
    /// A filter for a trust threashold for said peer
    pub trust_filter: Option<TrustFilter>,
}

impl FrameParser for PeerQuery {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, note_filter) = maybe(cstring, input).map(|(i, x)| (i, x.flatten()))?;
        let (input, tag_filter) = vec_of(cstring, input)?;

        Ok((
            input,
            Ok(Self {
                note_filter,
                tag_filter: tag_filter.into_iter().filter_map(|x| x).collect(),
                trust_filter: None,
            }),
        ))
    }
}

/// A peer entry
#[derive(Debug, Serialize, Deserialize)]
pub struct PeerEntry {
    pub addr: Address,
    pub first_connection: DateTime<Utc>,
    pub last_connection: DateTime<Utc>,
    pub active: bool,
}

impl Display for PeerEntry {
    fn fmt(&self, w: &mut fmt::Formatter) -> fmt::Result {
        write!(
            w,
            "{}\t{}\t{}\t{}",
            self.addr,
            self.first_connection,
            self.last_connection,
            if self.active { "ACTIVE" } else { "IDLE" }
        )
    }
}

impl FrameGenerator for PeerEntry {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.addr.generate(buf)?;
        self.first_connection.generate(buf)?;
        self.last_connection.generate(buf)?;
        if self.active {
            buf.push(1);
        } else {
            buf.push(0);
        }
        Ok(())
    }
}

impl FrameParser for PeerEntry {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, addr) = take_address(input)?;
        let (input, first) = take_datetime(input)?;
        let (input, last) = take_datetime(input)?;
        let (input, active) = take_byte(input)?;

        match (first, last) {
            (Ok(f), Ok(l)) => Ok((
                input,
                Ok(Self {
                    first_connection: f,
                    last_connection: l,
                    active: if active == 1 { true } else { false },
                    addr,
                }),
            )),
            (Err(e), _) | (_, Err(e)) => Ok((input, Err(e))),
        }
    }
}

/// A list of available peers
#[repr(C)]
#[derive(Debug)]
pub struct PeerList {
    pub list: Vec<PeerEntry>,
}

impl FrameGenerator for PeerList {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.list.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for PeerList {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, peer_list) = vec_of(PeerEntry::parse, input)?;
        Ok((
            input,
            peer_list
                .into_iter()
                .collect::<Result<Vec<_>>>()
                .map(|list| Self { list }),
        ))
    }
}
