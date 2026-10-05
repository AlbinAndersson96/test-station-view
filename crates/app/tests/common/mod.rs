#![allow(dead_code)]

use glam::{Vec2, Vec3};
use tsv_app::session::Session;
use tsv_core::ids::{DeviceId, PortId, RackId};
use tsv_core::limits::Limits;
use tsv_core::model::{Device, DeviceKind, Document, Port, PortKind, Rack, Rgb};
use tsv_core::name::{DocumentName, Name};

pub const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

pub fn limits() -> Limits {
    Limits::default()
}

pub fn name(s: &str) -> Name {
    Name::parse(s, &limits()).unwrap()
}

pub fn port(n: &str, row: u32, col: u32) -> Port {
    Port {
        id: PortId::new(),
        name: name(n),
        row,
        col,
        kind: PortKind::default(),
    }
}

pub fn device(n: &str, bottom_u: u32, height_u: u32) -> Device {
    Device {
        id: DeviceId::new(),
        name: name(n),
        bottom_u,
        height_u,
        color: Rgb::NEUTRAL_GREY,
        kind: DeviceKind::default(),
        ports: Vec::new(),
    }
}

pub fn with_ports(mut d: Device, ports: Vec<Port>) -> Device {
    d.ports = ports;
    d
}

pub fn rack(n: &str, height_u: u32, devices: Vec<Device>) -> Rack {
    Rack {
        id: RackId::new(),
        name: name(n),
        height_u,
        devices,
    }
}

pub fn doc(racks: Vec<Rack>) -> Document {
    Document {
        name: DocumentName::parse("Test").unwrap(),
        racks,
    }
}

/// A session on `d` whose camera has finished any tween.
pub fn session(d: Document) -> Session {
    let mut s = Session::new(d, limits(), VIEWPORT);
    s.advance(0.0);
    s
}

/// Lets every camera tween and motion finish.
pub fn settle(s: &mut Session, now: &mut f64) {
    for _ in 0..200 {
        *now += 0.016;
        if !s.advance(*now) {
            return;
        }
    }
}

/// Canvas position of a world point.
pub fn px(s: &Session, p: Vec3) -> Vec2 {
    s.world_to_pixel(p)
}
