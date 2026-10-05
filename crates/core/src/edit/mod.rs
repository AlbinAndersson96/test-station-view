//! Edit operations. Each `plan_*` function computes a complete candidate document
//! without touching its input; a `Rejection` means nothing changes.

mod device;
mod document;
mod port;
mod rack;

pub use device::*;
pub use document::*;
pub use port::*;
pub use rack::*;

use crate::ids::{DeviceId, PortId, RackId};
use crate::model::Document;
use crate::placement::PlacementError;
use crate::port_grid::PortPlacementError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectId {
    Rack(RackId),
    Device(DeviceId),
    Port(PortId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub document: Document,
    /// The object created or moved by this plan, if any.
    pub subject: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    #[error("the object no longer exists")]
    NotFound,
    #[error("the name '{0}' is already used here")]
    NameTaken(String),
    #[error("the height must be at least 1U")]
    ZeroHeight,
    #[error("the device is taller than the rack")]
    TooTall,
    #[error("there is not enough free space above")]
    NoRoomAbove,
    #[error("there is not enough free space below")]
    NoRoomBelow,
    #[error("equipment would end up outside the rack")]
    EquipmentOutside,
    #[error("ports would end up outside the device face")]
    PortsOutside,
    #[error("the position is outside the device face")]
    OutOfGrid,
    #[error("there are not enough free cells to make room")]
    NoRoomForPorts,
    #[error("a port can only be placed on its own device")]
    WrongDevice,
    #[error("the height must be at most {max}U")]
    HeightAboveLimit { max: u32 },
}

impl From<PlacementError> for Rejection {
    fn from(e: PlacementError) -> Self {
        match e {
            PlacementError::TooTall => Rejection::TooTall,
            PlacementError::NoRoomAbove => Rejection::NoRoomAbove,
            PlacementError::NoRoomBelow => Rejection::NoRoomBelow,
        }
    }
}

impl From<PortPlacementError> for Rejection {
    fn from(e: PortPlacementError) -> Self {
        match e {
            PortPlacementError::OutOfGrid => Rejection::OutOfGrid,
            PortPlacementError::NoRoom => Rejection::NoRoomForPorts,
        }
    }
}
