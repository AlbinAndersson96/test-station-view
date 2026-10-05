//! Pointer and keyboard handling for the 3D view (spec §4.3 "Interaction controller").
//!
//! Positions are canvas-relative CSS pixels. `over_trash` says whether the pointer is over the
//! trash zone (the UI knows where that element is).

use glam::{Vec2, Vec3};
use tsv_core::edit::{
    DeviceSource, ObjectId, PortSource, plan_device_drop, plan_port_drop, plan_remove_device,
    plan_remove_port,
};
use tsv_core::ids::{DeviceId, PortId};
use tsv_core::model::Rgb;
use tsv_core::name::Name;
use tsv_core::port_grid::Cell;
use tsv_render::layout::{device_face, object_bounds, port_marker_box, units_box};
use tsv_render::pick::{device_drop_target, grab_offset_u, pick, pick_hit, port_drop_target};
use tsv_render::scene::{Ghost, contrast_text};

use crate::session::Session;

/// Pointer movement (CSS px) that turns a press into a drag.
pub const DRAG_THRESHOLD_PX: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

/// What is being dragged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragSource {
    NewDevice { name: Name, height_u: u32 },
    Device(DeviceId),
    NewPort { device: DeviceId, name: Name },
    Port { device: DeviceId, port: PortId },
}

impl DragSource {
    fn is_port(&self) -> bool {
        matches!(self, DragSource::NewPort { .. } | DragSource::Port { .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Mode {
    Idle,
    /// Left button down, not yet moved far enough to be a drag.
    Pressed {
        start: Vec2,
        hit: Option<(ObjectId, Vec3)>,
    },
    Orbiting {
        last: Vec2,
    },
    Panning {
        last: Vec2,
    },
    Dragging {
        source: DragSource,
        grab_offset_u: u32,
    },
}

impl Mode {
    pub(crate) fn is_idle(&self) -> bool {
        matches!(self, Mode::Idle)
    }
}

impl Session {
    /// `true` while a device or port is being dragged (the UI shows the trash zone).
    pub fn is_dragging(&self) -> bool {
        matches!(self.mode, Mode::Dragging { .. })
    }

    pub fn pointer_down(&mut self, pos: Vec2, button: Button) {
        if !self.mode.is_idle() {
            return;
        }
        self.mode = match button {
            Button::Left => {
                let hit = pick_hit(self.document(), &self.limits, &self.ray_at(pos));
                Mode::Pressed { start: pos, hit }
            }
            Button::Middle => Mode::Panning { last: pos },
            Button::Right => Mode::Idle,
        };
    }

    pub fn pointer_move(&mut self, pos: Vec2, over_trash: bool, now_s: f64) {
        match self.mode.clone() {
            Mode::Idle => {
                let hovered = match pick(self.document(), &self.limits, &self.ray_at(pos)) {
                    Some(ObjectId::Port(p)) => Some(p),
                    _ => None,
                };
                self.hovered_port = hovered;
            }
            Mode::Pressed { start, hit } => {
                if pos.distance(start) < DRAG_THRESHOLD_PX {
                    return;
                }
                match hit {
                    Some((ObjectId::Device(d), at)) => {
                        let grab = self
                            .document()
                            .device(d)
                            .map_or(0, |(_, device)| grab_offset_u(device, at.y));
                        self.mode = Mode::Dragging {
                            source: DragSource::Device(d),
                            grab_offset_u: grab,
                        };
                    }
                    Some((ObjectId::Port(p), _)) => {
                        let Some((_, device, _)) = self.document().port(p) else {
                            return;
                        };
                        let device = device.id;
                        self.start_drag(DragSource::Port { device, port: p }, now_s);
                    }
                    _ => {
                        self.mode = Mode::Orbiting { last: start };
                    }
                }
                self.pointer_move(pos, over_trash, now_s);
            }
            Mode::Orbiting { last } => {
                let d = pos - last;
                self.view.camera_mut().orbit(d.x, d.y);
                self.mode = Mode::Orbiting { last: pos };
            }
            Mode::Panning { last } => {
                let d = pos - last;
                let height = self.viewport.y;
                self.view.camera_mut().pan(d.x, d.y, height);
                self.mode = Mode::Panning { last: pos };
            }
            Mode::Dragging {
                source,
                grab_offset_u,
            } => {
                self.update_drag(&source, grab_offset_u, pos, over_trash);
            }
        }
    }

    pub fn pointer_up(&mut self, pos: Vec2, over_trash: bool, now_s: f64) {
        match std::mem::replace(&mut self.mode, Mode::Idle) {
            Mode::Pressed { hit, .. } => self.select(hit.map(|(id, _)| id)),
            Mode::Dragging {
                source,
                grab_offset_u,
            } => {
                self.update_drag(&source, grab_offset_u, pos, over_trash);
                if over_trash {
                    self.drop_in_trash(&source);
                } else if let Some(plan) = self.preview.take() {
                    self.commit(plan);
                }
                self.end_drag(&source, now_s);
            }
            _ => {}
        }
    }

    /// Esc: cancels a drag without changing anything.
    pub fn escape(&mut self, now_s: f64) {
        if let Mode::Dragging { source, .. } = std::mem::replace(&mut self.mode, Mode::Idle) {
            self.end_drag(&source, now_s);
        }
    }

    /// The browser cancelled the pointer or took away its capture: end any gesture, change
    /// nothing.
    pub fn pointer_cancel(&mut self, now_s: f64) {
        if let Mode::Dragging { source, .. } = std::mem::replace(&mut self.mode, Mode::Idle) {
            self.end_drag(&source, now_s);
        }
    }

    /// Mouse wheel over the view (positive = zoom out).
    pub fn wheel(&mut self, delta: f32) {
        self.view.camera_mut().zoom(delta);
    }

    /// A drag that starts on a sidebar handle ("new device" / "new port").
    pub fn start_drag(&mut self, source: DragSource, now_s: f64) {
        if source.is_port() {
            let device = match &source {
                DragSource::NewPort { device, .. } | DragSource::Port { device, .. } => *device,
                _ => unreachable!(),
            };
            let doc = self.document();
            let face = doc.racks.iter().enumerate().find_map(|(ri, r)| {
                r.devices
                    .iter()
                    .find(|d| d.id == device)
                    .map(|d| device_face(ri, d))
            });
            if let Some(face) = face {
                let aspect = self.aspect();
                self.view.enter_port_mode(&face, aspect, now_s);
            }
        }
        self.preview = None;
        self.ghost = None;
        self.mode = Mode::Dragging {
            source,
            grab_offset_u: 0,
        };
    }

    /// The object a right-click at `pos` refers to (for the context menu).
    pub fn context_target(&self, pos: Vec2) -> Option<ObjectId> {
        if !self.mode.is_idle() {
            return None;
        }
        pick(self.document(), &self.limits, &self.ray_at(pos))
    }

    pub fn hovered_port(&self) -> Option<PortId> {
        self.hovered_port
    }

    pub fn ghost(&self) -> Option<Ghost> {
        self.ghost
    }

    fn end_drag(&mut self, source: &DragSource, now_s: f64) {
        self.preview = None;
        self.ghost = None;
        if source.is_port() {
            self.view.exit_port_mode(now_s);
        }
    }

    fn drop_in_trash(&mut self, source: &DragSource) {
        let doc = self.document();
        let result = match source {
            DragSource::Device(d) => plan_remove_device(doc, *d),
            DragSource::Port { port, .. } => plan_remove_port(doc, *port),
            DragSource::NewDevice { .. } | DragSource::NewPort { .. } => return,
        };
        let _ = self.apply(result);
    }

    /// Recomputes the preview document and the ghost for the pointer at `pos`.
    fn update_drag(
        &mut self,
        source: &DragSource,
        grab_offset_u: u32,
        pos: Vec2,
        over_trash: bool,
    ) {
        self.preview = None;
        self.ghost = None;
        if over_trash {
            return;
        }
        let ray = self.ray_at(pos);
        let doc = self.document();
        let limits = &self.limits;
        match source {
            DragSource::NewDevice { .. } | DragSource::Device(_) => {
                let (core_source, height_u, color) = match source {
                    DragSource::NewDevice { name, height_u } => (
                        DeviceSource::New {
                            name: name.clone(),
                            height_u: *height_u,
                        },
                        *height_u,
                        Rgb::NEUTRAL_GREY,
                    ),
                    DragSource::Device(d) => {
                        let Some((_, device)) = doc.device(*d) else {
                            return;
                        };
                        (DeviceSource::Existing(*d), device.height_u, device.color)
                    }
                    _ => unreachable!(),
                };
                let Some(target) = device_drop_target(doc, &ray, grab_offset_u) else {
                    return;
                };
                match plan_device_drop(doc, limits, &core_source, target.rack, target.bottom_u) {
                    Ok(plan) => {
                        let aabb = plan
                            .subject
                            .and_then(|s| object_bounds(&plan.document, limits, s));
                        self.ghost = aabb.map(|aabb| Ghost {
                            aabb,
                            color,
                            valid: true,
                        });
                        self.preview = Some(plan);
                    }
                    Err(_) => {
                        let Some(ri) = doc.racks.iter().position(|r| r.id == target.rack) else {
                            return;
                        };
                        let rack_height = doc.racks[ri].height_u;
                        let bottom = target
                            .bottom_u
                            .min(rack_height.saturating_sub(height_u) + 1)
                            .max(1);
                        let aabb = units_box(ri, bottom, height_u);
                        self.ghost = Some(Ghost {
                            aabb,
                            color,
                            valid: false,
                        });
                    }
                }
            }
            DragSource::NewPort { device, .. } | DragSource::Port { device, .. } => {
                let core_source = match source {
                    DragSource::NewPort { name, .. } => PortSource::New { name: name.clone() },
                    DragSource::Port { port, .. } => PortSource::Existing(*port),
                    _ => unreachable!(),
                };
                let Some((ri, d)) = doc.racks.iter().enumerate().find_map(|(ri, r)| {
                    r.devices.iter().find(|d| d.id == *device).map(|d| (ri, d))
                }) else {
                    return;
                };
                let ink = contrast_text(d.color);
                let color = Rgb {
                    r: ink[0],
                    g: ink[1],
                    b: ink[2],
                };
                let Some(target) = port_drop_target(doc, limits, *device, &ray) else {
                    return;
                };
                match plan_port_drop(doc, limits, *device, &core_source, target.cell, target.dir) {
                    Ok(plan) => {
                        let aabb = plan
                            .subject
                            .and_then(|s| object_bounds(&plan.document, limits, s));
                        self.ghost = aabb.map(|aabb| Ghost {
                            aabb,
                            color,
                            valid: true,
                        });
                        self.preview = Some(plan);
                    }
                    Err(_) => {
                        let cell = Cell {
                            row: target.cell.row,
                            col: target.cell.col,
                        };
                        let aabb = port_marker_box(ri, d, cell, limits);
                        self.ghost = Some(Ghost {
                            aabb,
                            color,
                            valid: false,
                        });
                    }
                }
            }
        }
    }
}
