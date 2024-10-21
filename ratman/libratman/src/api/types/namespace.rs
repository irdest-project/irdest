use crate::{
    frame::{
        parse::{take_address, take_id, take_u128},
        FrameGenerator, FrameParser,
    },
    types::{Address, Ident32, Namespace},
    Result,
};
use nom::IResult;

/// Register a new namespace key and subscribe to it on the server side (for a
/// given application/ address pair)
pub struct NamespaceRegister {
    pub pubkey: Address,
    pub privkey: Ident32,
}

impl FrameGenerator for NamespaceRegister {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.pubkey.generate(buf)?;
        self.privkey.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for NamespaceRegister {
    type Output = NamespaceRegister;

    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, pubkey) = take_address(input)?;
        let (input, privkey) = take_id(input)?;

        Ok((input, NamespaceRegister { pubkey, privkey }))
    }
}

/// Destroy an existing namespace
pub struct NamespaceDestroy {
    pub pubkey: Address,
    pub privkey: Ident32,
}

impl FrameGenerator for NamespaceDestroy {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.pubkey.generate(buf)?;
        self.privkey.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for NamespaceDestroy {
    type Output = NamespaceDestroy;

    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, pubkey) = take_address(input)?;
        let (input, privkey) = take_id(input)?;

        Ok((input, NamespaceDestroy { pubkey, privkey }))
    }
}

/// Mark an existing namespace as up
pub struct NamespaceUp {
    pub client_addr: Address,
    pub namespace_addr: Namespace,
}

impl FrameGenerator for NamespaceUp {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.client_addr.generate(buf)?;
        self.namespace_addr.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for NamespaceUp {
    type Output = NamespaceUp;

    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, client_addr) = take_address(input)?;
        let (input, namespace_addr) = take_address(input)?;

        Ok((
            input,
            NamespaceUp {
                client_addr,
                namespace_addr,
            },
        ))
    }
}

/// Mark an existing namespace as down
pub struct NamespaceDown {
    pub client_addr: Address,
    pub namespace_addr: Namespace,
}

impl FrameGenerator for NamespaceDown {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.client_addr.generate(buf)?;
        self.namespace_addr.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for NamespaceDown {
    type Output = NamespaceDown;

    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, client_addr) = take_address(input)?;
        let (input, namespace_addr) = take_address(input)?;

        Ok((
            input,
            NamespaceDown {
                client_addr,
                namespace_addr,
            },
        ))
    }
}

/// Send an anycast probe into a namespace
pub struct AnycastProbe {
    pub self_addr: Address,
    pub namespace_addr: Address,
    pub timeout_ms: u128,
}

impl FrameGenerator for AnycastProbe {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.self_addr.generate(buf)?;
        self.namespace_addr.generate(buf)?;
        self.timeout_ms.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for AnycastProbe {
    type Output = AnycastProbe;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, self_addr) = take_address(input)?;
        let (input, namespace_addr) = take_address(input)?;
        let (input, timeout_ms) = take_u128(input)?;

        Ok((
            input,
            AnycastProbe {
                self_addr,
                namespace_addr,
                timeout_ms,
            },
        ))
    }
}
