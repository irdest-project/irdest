use crate::{
    frame::{
        micro::parse::{maybe, vec_of},
        parse::{self, maybe_cstring},
        FrameGenerator, FrameParser,
    },
    types::Address,
    MicroframeError, RatmanError, Result,
};
use nom::IResult;
use std::ffi::CString;

/// Create a new address with an optional description identifier
pub struct AddrCreate {
    pub name: Option<CString>,
}

impl FrameGenerator for AddrCreate {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        match self.name {
            Some(n) => buf.extend_from_slice(n.as_bytes()),
            None => buf.push(0),
        };
        Ok(())
    }
}

impl FrameParser for AddrCreate {
    type Output = Self;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, maybe_name) = maybe_cstring(input).unwrap();

        let name: Option<CString> = match maybe_name {
            Ok(Some(name)) => Some(name),
            _ => None,
        };

        Ok((input, Self { name }))
    }
}

/// Destroy an existing adress, optionally deleting all associated data
pub struct AddrDestroy {
    pub addr: Address,
    pub force: bool,
}

impl FrameGenerator for AddrDestroy {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.addr.generate(buf)?;
        match self.force {
            true => buf.push(1),
            false => buf.push(0),
        }
        Ok(())
    }
}

impl FrameParser for AddrDestroy {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, addr) = maybe(parse::take_address, input)?;
        let (input, _) = maybe(parse::take(1 as u8), input)?;

        let res = match addr {
            Some(addr) => Ok(Self { addr, force: false }),
            None => Err(MicroframeError::MissingFields(&["addr"])),
        }
        .map_err(|e| RatmanError::Microframe(e));

        Ok((input, res))
    }
}

/// Mark an address as up
pub struct AddrUp {
    pub addr: Address,
}

impl FrameGenerator for AddrUp {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.addr.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for AddrUp {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, addr) = maybe(parse::take_address, input)?;

        let res = match addr {
            Some(addr) => Ok(Self { addr }),
            None => Err(MicroframeError::MissingFields(&["addr"])),
        }
        .map_err(|e| RatmanError::Microframe(e));

        Ok((input, res))
    }
}

/// Mark an address as down
pub struct AddrDown {
    pub addr: Address,
}

impl FrameGenerator for AddrDown {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.addr.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for AddrDown {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, addr) = maybe(parse::take_address, input)?;

        let res = match addr {
            Some(addr) => Ok(Self { addr }),
            None => Err(MicroframeError::MissingFields(&["addr"])),
        }
        .map_err(|e| RatmanError::Microframe(e));

        Ok((input, res))
    }
}

/// List all locally available addresses
pub struct AddrList {
    pub list: Vec<Address>,
}

impl FrameGenerator for AddrList {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.list.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for AddrList {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, list) = vec_of(parse::take_address, input)?;
        Ok((input, Ok(Self { list })))
    }
}
