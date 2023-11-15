/// Represent a set of direct peers
///
/// A neighbour is another Ratman router that is directly connected to
/// the current one.  A direct neighbour may know the current device
/// IP address, or the router identification keys of our peers.
///
/// In this context a `Neighbour` represents a set of direct peers
/// that we can pass messages to.  In some cases we may want to
/// address them all, just a subset, or none of them, for example if a
/// packet is being filtered from a particular subnet.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Neighbour {
    /// Send message to all reachable neighbours
    Flood,
    /// Send to all neighbours, except one
    FloodExcept(u16),
    /// Exclude this envelope from all neighbours
    ///
    /// Note: your endpoint may ignore this type, since it is filtered
    /// in the ratmand switch.  However, potentially this could change
    /// in the future, so implementing a drop feature anyway may be
    /// warranted.
    Drop,
    /// Encodes a specific neighbour ID
    Single(u16),
}
