use crate::journal::page::SerdeFrameType;
use libratman::{frame::carrier::PeerDataV1, types::Ident32};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NeighbourData {
    link_id: Ident32,
    neighbour_data: SerdeFrameType<PeerDataV1>,
}
