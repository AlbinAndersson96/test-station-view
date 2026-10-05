use std::cmp::Reverse;

use crate::ids::{DeviceId, PortId, RackId};
use crate::limits::Limits;
use crate::name::{DocumentName, Name};

pub const DEFAULT_RACK_HEIGHT_U: u32 = 42;
pub const DEFAULT_DOCUMENT_NAME: &str = "Station1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const NEUTRAL_GREY: Rgb = Rgb { r: 0xa0, g: 0xa0, b: 0xa0 };

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Parses exactly `#rrggbb` (hex digits, either case).
    pub fn from_hex(s: &str) -> Option<Rgb> {
        let hex = s.strip_prefix('#')?;
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Rgb { r: channel(0)?, g: channel(2)?, b: channel(4)? })
    }
}

/// Reserved for a future equipment catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceKind {
    #[default]
    AdHoc,
}

/// Reserved for future connector types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortKind {
    #[default]
    Unspecified,
}

/// A port in cell (`row`, `col`) of its device's front face. Row 0 is the bottom row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    pub id: PortId,
    pub name: Name,
    pub row: u32,
    pub col: u32,
    pub kind: PortKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: DeviceId,
    pub name: Name,
    pub bottom_u: u32,
    pub height_u: u32,
    pub color: Rgb,
    pub kind: DeviceKind,
    pub ports: Vec<Port>,
}

impl Device {
    pub fn top_u(&self) -> u32 {
        self.bottom_u + self.height_u - 1
    }

    pub fn port_rows(&self, limits: &Limits) -> u32 {
        self.height_u * limits.port_rows_per_u
    }

    /// Top row first, then left to right.
    pub fn ports_in_reading_order(&self) -> Vec<&Port> {
        let mut ports: Vec<&Port> = self.ports.iter().collect();
        ports.sort_by_key(|p| (Reverse(p.row), p.col));
        ports
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rack {
    pub id: RackId,
    pub name: Name,
    pub height_u: u32,
    pub devices: Vec<Device>,
}

impl Rack {
    /// Physical order: highest U first.
    pub fn devices_top_down(&self) -> Vec<&Device> {
        let mut devices: Vec<&Device> = self.devices.iter().collect();
        devices.sort_by_key(|d| Reverse(d.bottom_u));
        devices
    }
}

/// `racks` order is the left-to-right order of the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub name: DocumentName,
    pub racks: Vec<Rack>,
}

impl Document {
    pub fn new_default(limits: &Limits) -> Document {
        Document {
            name: DocumentName::parse(DEFAULT_DOCUMENT_NAME).expect("default name is valid"),
            racks: vec![Rack {
                id: RackId::new(),
                name: Name::parse("Rack1", limits).expect("default rack name is valid"),
                height_u: DEFAULT_RACK_HEIGHT_U,
                devices: Vec::new(),
            }],
        }
    }

    pub fn rack(&self, id: RackId) -> Option<&Rack> {
        self.racks.iter().find(|r| r.id == id)
    }

    pub fn device(&self, id: DeviceId) -> Option<(&Rack, &Device)> {
        let (ri, di) = self.device_location(id)?;
        let rack = &self.racks[ri];
        Some((rack, &rack.devices[di]))
    }

    pub fn port(&self, id: PortId) -> Option<(&Rack, &Device, &Port)> {
        let (ri, di, pi) = self.port_location(id)?;
        let rack = &self.racks[ri];
        let device = &rack.devices[di];
        Some((rack, device, &device.ports[pi]))
    }

    pub(crate) fn rack_index(&self, id: RackId) -> Option<usize> {
        self.racks.iter().position(|r| r.id == id)
    }

    pub(crate) fn device_location(&self, id: DeviceId) -> Option<(usize, usize)> {
        self.racks.iter().enumerate().find_map(|(ri, rack)| {
            rack.devices.iter().position(|d| d.id == id).map(|di| (ri, di))
        })
    }

    pub(crate) fn port_location(&self, id: PortId) -> Option<(usize, usize, usize)> {
        self.racks.iter().enumerate().find_map(|(ri, rack)| {
            rack.devices.iter().enumerate().find_map(|(di, device)| {
                device.ports.iter().position(|p| p.id == id).map(|pi| (ri, di, pi))
            })
        })
    }
}
