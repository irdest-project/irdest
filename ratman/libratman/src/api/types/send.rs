use crate::{
    frame::{micro::parse::vec_of, FrameGenerator, FrameParser},
    types::LetterheadV1,
    Result,
};
use nom::IResult;

/// Send a data stream to a single recipient
pub struct SendOne {
    pub letterhead: LetterheadV1,
}

impl FrameGenerator for SendOne {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.letterhead.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for SendOne {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, letterhead) = LetterheadV1::parse(input)?;
        Ok((input, letterhead.map(|letterhead| Self { letterhead })))
    }
}

/// Send a  to many recipients
pub struct SendMany {
    pub letterheads: Vec<LetterheadV1>,
}

impl FrameGenerator for SendMany {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.letterheads.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for SendMany {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, letterheads) = vec_of(LetterheadV1::parse, input)?;
        Ok((
            input,
            letterheads
                .into_iter()
                .collect::<Result<Vec<LetterheadV1>>>()
                .map(|letterheads| Self { letterheads }),
        ))
    }
}
