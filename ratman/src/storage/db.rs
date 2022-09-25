use crate::daemon::state::data_path;

use sled::{open, Db as Handle, Error::Io, Lazy};
use std::io::ErrorKind::PermissionDenied;

/// A database newtype, abstracting internal Db operations
pub(crate) struct Db(Handle);
// TODO: it is meant to only be instantiated once at the start of the daemon.
// a Db handle is completely safe to clone and access from multiple threads, but
// must only be opened once.

impl Db {
    pub(crate) fn new() -> Self {
        // Invariant:
        // there should only be one ratman router per device, so there's no need
        // for a unique db per-process. $datadir/db suffices as a path.
        // This invariant is broken in some tests, so we resort to this hack for
        // now, until the tests stop spawning many routers in the same filesystem.
        static HANDLE: Lazy<Handle, fn() -> Handle> =
            Lazy::new(|| match open(data_path().join("db")) {
                // TODO: DO NOT HARDCODE TMPDIR
                Err(Io(e)) if e.kind() == PermissionDenied => open("/tmp/irdest/db").unwrap(),
                Err(e) => panic!("open failure: {}", e),
                Ok(db) => db,
            });
        Db(HANDLE.clone())
    }
}

#[test]
fn multiple_database_inits() {
    Db::new();
    Db::new();
}
