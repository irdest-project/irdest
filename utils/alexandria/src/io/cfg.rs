use crate::{dir::Dirs, error::Result};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read};

/// The current version of the library
pub const VERSION: u32 = 0;

/// A module with older version numbers to match against
pub mod legacy {
    /// Prototype/ rapid development phase
    ///
    /// It is not recommended to load _any_ database that was written
    /// in this version, due to no backwards compatible library
    /// structures.
    pub const ALPHA: u32 = 0;
}

/// Database configuration
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
}

impl Config {
    pub(crate) fn init() -> Self {
        Self { version: VERSION }
    }

    pub(crate) fn load(d: &Dirs) -> Result<Self> {
        let path = d.root().join("db.config");

        let mut buf = String::new();
        let mut f = File::open(path)?;
        f.read_to_string(&mut buf)?;

        Ok(toml::from_str(buf.as_str())?)
    }
}
