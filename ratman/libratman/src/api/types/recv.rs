use crate::{
    frame::{
        parse::{take_address, take_id},
        FrameGenerator, FrameParser,
    },
    types::{Address, Ident32, Recipient},
    Result,
};
use nom::IResult;

/// Create a new subscription
pub struct SubsCreate {
    pub addr: Address,
    pub recipient: Recipient,
}

impl FrameGenerator for SubsCreate {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.addr.generate(buf)?;
        Some(self.recipient).generate(buf)?;
        Ok(())
    }
}

impl FrameParser for SubsCreate {
    type Output = Self;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, addr) = take_address(input)?;
        let (input, recipient) = Option::<Recipient>::parse(input)?;
        Ok((
            input,
            Self {
                addr,
                recipient: recipient.unwrap(),
            },
        ))
    }
}

/// Delete an existing subscription
pub struct SubsDelete {
    pub sub_id: Ident32,
    pub addr: Address,
}

impl FrameParser for SubsDelete {
    type Output = Self;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, sub_id) = take_id(input)?;
        let (input, addr) = take_address(input)?;
        Ok((input, Self { sub_id, addr }))
    }
}

impl FrameGenerator for SubsDelete {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        Some(self.sub_id).generate(buf)?;
        self.addr.generate(buf)?;
        Ok(())
    }
}

/// Restore a previous subscription
pub struct SubsRestore {
    pub sub_id: Ident32,
    pub addr: Address,
}

impl FrameGenerator for SubsRestore {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        Some(self.sub_id).generate(buf)?;
        self.addr.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for SubsRestore {
    type Output = Self;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, sub_id) = take_id(input)?;
        let (input, addr) = take_address(input)?;
        Ok((input, Self { sub_id, addr }))
    }
}

/// Request to send a single data stream
pub struct RecvOne {
    pub addr: Address,
    pub to: Recipient,
}

impl FrameGenerator for RecvOne {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.addr.generate(buf)?;
        Some(self.to).generate(buf)?;
        Ok(())
    }
}

impl FrameParser for RecvOne {
    type Output = Self;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, addr) = take_address(input)?;
        let (input, to) = Option::<Recipient>::parse(input)?;
        Ok((
            input,
            Self {
                addr,
                to: to.expect("invalid RecvOne payload"),
            },
        ))
    }
}

/// Receive many data streams
pub struct RecvMany {
    pub addr: Address,
    pub to: Recipient,
    pub limit: Option<u32>,
}

impl FrameGenerator for RecvMany {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.addr.generate(buf)?;
        Some(self.to).generate(buf)?;
        self.limit.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for RecvMany {
    type Output = Self;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, addr) = take_address(input)?;
        let (input, to) = Option::<Recipient>::parse(input)?;
        let (input, limit) = Option::<u32>::parse(input)?;
        Ok((
            input,
            Self {
                addr,
                to: to.expect("invalid RecvOne payload"),
                limit,
            },
        ))
    }
}
