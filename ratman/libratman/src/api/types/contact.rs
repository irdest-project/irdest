use crate::{
    frame::{
        carrier::take_address,
        generate::{generate_cstring_tuple_vec, generate_option_cstring},
        parse::maybe_cstring,
        FrameGenerator, FrameParser,
    },
    types::{to_cstring, Address},
    Result,
};
use nom::IResult;
use std::ffi::CString;

/// Add a new contact entry
pub struct ContactAdd {
    /// The address to add as a contact
    pub addr: Address,
    /// A user defined (private!) note
    pub note: Option<CString>,
    /// Key=Value tags, strings are \0 terminated
    pub tags: Vec<(CString, CString)>,
    /// Trust level between 1 and 7
    pub trust: u8,
}

impl ContactAdd {
    pub fn new(
        addr: Address,
        note: Option<String>,
        tags: impl Iterator<Item = (String, String)>,
        trust: u8,
    ) -> Self {
        Self {
            addr,
            note: note.map(|n| to_cstring(&n)),
            tags: tags
                .map(|(k, v)| ((to_cstring(&k), to_cstring(&v))))
                .collect(),
            trust,
        }
    }
}

impl FrameGenerator for ContactAdd {
    fn generate(self, buf: &mut Vec<u8>) -> Result<()> {
        self.addr.generate(buf)?;
        generate_option_cstring(self.note, buf)?;
        generate_cstring_tuple_vec(self.tags, buf)?;
        self.trust.generate(buf)?;
        Ok(())
    }
}

impl FrameParser for ContactAdd {
    type Output = Result<Self>;
    fn parse(input: &[u8]) -> IResult<&[u8], Self::Output> {
        let (input, _addr) = take_address(input)?;

        // Read a CString note
        let (input, note) = maybe_cstring(input)?;
        if let Err(e) = note {
            return Ok((input, Err(e)));
        }

        todo!()
    }
}

/// Delete an existing contact entry
pub struct ContactDelete {
    /// An address to delete the contact book entry for
    pub addr: Address,
}

/// Modify a contact entry in-place
pub struct ContactModify {
    pub add_tags: Vec<(CString, CString)>,
    pub remove_tags_by_key: Vec<CString>,
    pub addr: Option<Address>,
    pub new_trust: Option<u8>,
}
