//! Turns a document plus view state into draw lists (spec §4.2 "Input", "Look", "Ghost").

use std::collections::HashMap;

use glam::{Vec2, Vec3};
use tsv_core::edit::ObjectId;
use tsv_core::ids::PortId;
use tsv_core::limits::Limits;
use tsv_core::model::{Document, Rgb};
use tsv_core::port_grid::Cell;

use crate::layout::{
    Aabb, CABLE_RADIUS_MM, FLOOR_MARGIN_MM, FaceRect, cable_path, device_box, device_name_rect,
    marker_anchor, port_label_rect, port_marker_box, rack_name_rect, rack_parts, scene_bounds,
    u_label_rect,
};

pub const BACKGROUND: Rgb = Rgb {
    r: 236,
    g: 239,
    b: 243,
};
pub const FRAME_COLOR: Rgb = Rgb {
    r: 70,
    g: 74,
    b: 82,
};
pub const GHOST_INVALID: Rgb = Rgb {
    r: 230,
    g: 40,
    b: 40,
};
pub const GHOST_ALPHA: f32 = 0.45;
/// Time for moved objects to (visually) reach their new position.
pub const MOTION_SECONDS: f32 = 0.12;
/// Longest time step one `Motion::update` applies, so the first frame after an idle period
/// still eases instead of jumping straight to the target.
pub const MAX_MOTION_STEP_S: f32 = 1.0 / 30.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxInstance {
    pub aabb: Aabb,
    /// Straight (non-premultiplied) RGBA, 0..1.
    pub color: [f32; 4],
}

/// One straight piece of a cable: a tube from `a` to `b`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TubeInstance {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    /// Straight RGBA, 0..1.
    pub color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub text: String,
    pub rect: FaceRect,
    pub color: [u8; 4],
}

/// The translucent preview of what is being dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ghost {
    pub aabb: Aabb,
    pub color: Rgb,
    /// `false` draws the ghost red: the drop would be rejected.
    pub valid: bool,
}

/// The cable being drawn: from its start port to the pointer (or the port under it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CablePreview {
    pub from: Vec3,
    pub to: Vec3,
    /// `false` draws it red: releasing here would be rejected.
    pub valid: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneInput<'a> {
    /// The document to show: the committed one, or a previewed plan's document.
    pub document: &'a Document,
    pub limits: &'a Limits,
    pub selection: Option<ObjectId>,
    pub hovered_port: Option<PortId>,
    pub ghost: Option<Ghost>,
    pub cable_preview: Option<CablePreview>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub opaque: Vec<BoxInstance>,
    pub translucent: Vec<BoxInstance>,
    /// Boxes that get the selection outline.
    pub selected: Vec<BoxInstance>,
    /// Cable segments (opaque).
    pub tubes: Vec<TubeInstance>,
    /// Cable segments that get the selection outline.
    pub selected_tubes: Vec<TubeInstance>,
    pub labels: Vec<Label>,
    /// The floor grid under the racks; `None` draws no floor.
    pub floor: Option<Floor>,
}

/// A floor rectangle at y = 0, in world x (`.x`) and z (`.y`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Floor {
    pub min: Vec2,
    pub max: Vec2,
}

pub fn rgba(c: Rgb, alpha: f32) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        alpha,
    ]
}

/// Black or white, whichever reads better on `background`.
pub fn contrast_text(background: Rgb) -> [u8; 4] {
    let luminance =
        0.2126 * background.r as f32 + 0.7152 * background.g as f32 + 0.0722 * background.b as f32;
    if luminance > 140.0 {
        [0, 0, 0, 255]
    } else {
        [255, 255, 255, 255]
    }
}

/// Resting positions (box minimum corners) of everything that animates when it moves.
pub fn animation_targets(doc: &Document, limits: &Limits) -> Vec<(ObjectId, Vec3)> {
    let mut targets = Vec::new();
    for (ri, rack) in doc.racks.iter().enumerate() {
        for device in &rack.devices {
            targets.push((ObjectId::Device(device.id), device_box(ri, device).min));
            for port in &device.ports {
                let cell = Cell {
                    row: port.row,
                    col: port.col,
                };
                targets.push((
                    ObjectId::Port(port.id),
                    port_marker_box(ri, device, cell, limits).min,
                ));
            }
        }
    }
    targets
}

/// Displayed positions that ease towards their targets (spec: ~120 ms).
#[derive(Debug, Clone, Default)]
pub struct Motion {
    displayed: HashMap<ObjectId, Vec3>,
}

impl Motion {
    /// Advances displayed positions by `dt_s` seconds. New objects appear at their target.
    /// Returns `true` while anything is still moving.
    pub fn update(&mut self, targets: &[(ObjectId, Vec3)], dt_s: f32) -> bool {
        let rate = 3.0 / MOTION_SECONDS;
        let blend = 1.0 - (-rate * dt_s.clamp(0.0, MAX_MOTION_STEP_S)).exp();
        let mut moving = false;
        let mut next = HashMap::with_capacity(targets.len());
        for &(id, target) in targets {
            let shown = match self.displayed.get(&id) {
                Some(&p) => {
                    let p = p + (target - p) * blend;
                    if (target - p).length() < 0.05 {
                        target
                    } else {
                        p
                    }
                }
                None => target,
            };
            moving |= shown != target;
            next.insert(id, shown);
        }
        self.displayed = next;
        moving
    }

    /// Jumps every object to its target (e.g. after an import).
    pub fn snap(&mut self, targets: &[(ObjectId, Vec3)]) {
        self.displayed = targets.iter().copied().collect();
    }

    /// How far `id` is currently drawn from its resting position `target`.
    pub fn offset(&self, id: ObjectId, target: Vec3) -> Vec3 {
        self.displayed.get(&id).map_or(Vec3::ZERO, |p| *p - target)
    }
}

pub fn build_scene(input: &SceneInput, motion: &Motion) -> Scene {
    let mut scene = Scene::default();
    let doc = input.document;
    let bounds = scene_bounds(doc);
    scene.floor = Some(Floor {
        min: Vec2::new(bounds.min.x, bounds.min.z) - FLOOR_MARGIN_MM,
        max: Vec2::new(bounds.max.x, bounds.max.z) + FLOOR_MARGIN_MM,
    });
    let selected_cable = match input.selection {
        Some(ObjectId::Cable(c)) => doc.cable(c),
        _ => None,
    };
    // Where each port's cable end is drawn (its marker's displayed position).
    let mut anchors: HashMap<PortId, Vec3> = HashMap::new();
    for (ri, rack) in doc.racks.iter().enumerate() {
        let rack_selected = input.selection == Some(ObjectId::Rack(rack.id));
        for part in rack_parts(ri, rack) {
            let b = BoxInstance {
                aabb: part,
                color: rgba(FRAME_COLOR, 1.0),
            };
            scene.opaque.push(b);
            if rack_selected {
                scene.selected.push(b);
            }
        }
        let frame_text = contrast_text(FRAME_COLOR);
        scene.labels.push(Label {
            text: rack.name.to_string(),
            rect: rack_name_rect(ri, rack),
            color: frame_text,
        });
        for u in 1..=rack.height_u {
            scene.labels.push(Label {
                text: u.to_string(),
                rect: u_label_rect(ri, u),
                color: frame_text,
            });
        }

        for device in &rack.devices {
            let id = ObjectId::Device(device.id);
            let resting = device_box(ri, device);
            let offset = motion.offset(id, resting.min);
            let b = BoxInstance {
                aabb: resting.translated(offset),
                color: rgba(device.color, 1.0),
            };
            scene.opaque.push(b);
            if input.selection == Some(id) {
                scene.selected.push(b);
            }
            scene.labels.push(Label {
                text: device.name.to_string(),
                rect: device_name_rect(ri, device).translated(offset),
                color: contrast_text(device.color),
            });

            let device_selected = input.selection == Some(id);
            let ink = contrast_text(device.color);
            let marker_color = Rgb {
                r: ink[0],
                g: ink[1],
                b: ink[2],
            };
            for port in &device.ports {
                let pid = ObjectId::Port(port.id);
                let cell = Cell {
                    row: port.row,
                    col: port.col,
                };
                let resting = port_marker_box(ri, device, cell, input.limits);
                let offset = motion.offset(pid, resting.min);
                let b = BoxInstance {
                    aabb: resting.translated(offset),
                    color: rgba(marker_color, 1.0),
                };
                scene.opaque.push(b);
                anchors.insert(port.id, marker_anchor(&b.aabb));
                let selected = input.selection == Some(pid);
                if selected {
                    scene.selected.push(b);
                }
                let cable_end = selected_cable.is_some_and(|c| c.touches(port.id));
                if selected || device_selected || cable_end || input.hovered_port == Some(port.id) {
                    scene.labels.push(Label {
                        text: port.name.to_string(),
                        rect: port_label_rect(ri, device, cell, input.limits).translated(offset),
                        color: ink,
                    });
                }
            }
        }
    }
    for cable in &doc.cables {
        let (Some(&a), Some(&b)) = (anchors.get(&cable.a), anchors.get(&cable.b)) else {
            continue;
        };
        let start = scene.tubes.len();
        push_cable(&mut scene.tubes, a, b, rgba(cable.color, 1.0));
        if selected_cable.is_some_and(|c| c.id == cable.id) {
            let tubes = scene.tubes[start..].to_vec();
            scene.selected_tubes.extend(tubes);
        }
    }
    if let Some(preview) = input.cable_preview {
        let color = if preview.valid {
            Rgb::CABLE_BLUE
        } else {
            GHOST_INVALID
        };
        push_cable(&mut scene.tubes, preview.from, preview.to, rgba(color, 1.0));
    }
    if let Some(ghost) = input.ghost {
        let color = if ghost.valid {
            ghost.color
        } else {
            GHOST_INVALID
        };
        scene.translucent.push(BoxInstance {
            aabb: ghost.aabb,
            color: rgba(color, GHOST_ALPHA),
        });
    }
    scene
}

fn push_cable(tubes: &mut Vec<TubeInstance>, a: Vec3, b: Vec3, color: [f32; 4]) {
    let path = cable_path(a, b);
    tubes.extend(path.windows(2).map(|pair| TubeInstance {
        a: pair[0],
        b: pair[1],
        radius: CABLE_RADIUS_MM,
        color,
    }));
}
