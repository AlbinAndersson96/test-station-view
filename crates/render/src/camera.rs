//! Orbit camera, framing and camera tweens (spec §4.2 "Camera").

use std::f32::consts::FRAC_PI_2;

use glam::camera::rh::proj::directx;
use glam::camera::rh::view::look_at_mat4;
use glam::{Mat4, Vec2, Vec3};

use crate::layout::{Aabb, FaceRect};
use crate::pick::Ray;

pub const FOV_Y: f32 = 45.0 * std::f32::consts::PI / 180.0;
pub const NEAR_MM: f32 = 50.0;
pub const FAR_MM: f32 = 200_000.0;
pub const MIN_DISTANCE_MM: f32 = 150.0;
pub const MAX_DISTANCE_MM: f32 = 60_000.0;
pub const MAX_PITCH: f32 = 85.0 * std::f32::consts::PI / 180.0;
/// Pitch of the default front view: slightly from above.
pub const FRONT_VIEW_PITCH: f32 = 10.0 * std::f32::consts::PI / 180.0;
/// Radians per pixel of pointer movement while orbiting.
pub const ORBIT_SPEED: f32 = 0.005;
/// Margin around framed content (1.0 = tight).
pub const FRAME_MARGIN: f32 = 1.15;
pub const TWEEN_SECONDS: f64 = 0.4;

/// Camera orbiting `target`. `yaw = 0, pitch = 0` looks straight at the rack fronts (towards -Z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl OrbitCamera {
    pub fn eye(&self) -> Vec3 {
        let dir = Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        );
        self.target + dir * self.distance
    }

    pub fn view(&self) -> Mat4 {
        look_at_mat4(self.eye(), self.target, Vec3::Y)
    }

    pub fn projection(aspect: f32) -> Mat4 {
        directx::perspective(FOV_Y, aspect.max(0.01), NEAR_MM, FAR_MM)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        Self::projection(aspect) * self.view()
    }

    /// Rotates around the target; `dx`/`dy` are pointer deltas in pixels.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * ORBIT_SPEED;
        self.pitch = (self.pitch + dy * ORBIT_SPEED).clamp(-MAX_PITCH, MAX_PITCH);
    }

    /// Moves the target so the scene follows the pointer; deltas in pixels.
    pub fn pan(&mut self, dx: f32, dy: f32, viewport_height_px: f32) {
        let mm_per_px = 2.0 * self.distance * (FOV_Y * 0.5).tan() / viewport_height_px.max(1.0);
        let forward = (self.target - self.eye()).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        self.target += (-dx * right + dy * up) * mm_per_px;
    }

    /// Zooms by a wheel delta (positive = away).
    pub fn zoom(&mut self, wheel_delta: f32) {
        self.distance =
            (self.distance * 1.001f32.powf(wheel_delta)).clamp(MIN_DISTANCE_MM, MAX_DISTANCE_MM);
    }

    /// A camera at `yaw`/`pitch` that fits `bounds` in the viewport.
    pub fn framing(bounds: &Aabb, aspect: f32, yaw: f32, pitch: f32) -> OrbitCamera {
        let radius = bounds.size().length() * 0.5;
        let half_fov_x = ((FOV_Y * 0.5).tan() * aspect.max(0.01)).atan();
        let half_fov = (FOV_Y * 0.5).min(half_fov_x);
        let distance =
            (radius * FRAME_MARGIN / half_fov.sin()).clamp(MIN_DISTANCE_MM, MAX_DISTANCE_MM);
        OrbitCamera {
            target: bounds.center(),
            yaw,
            pitch,
            distance,
        }
    }

    /// The "Reset view" camera: all of `bounds` seen from the front, slightly from above.
    pub fn front_view(bounds: &Aabb, aspect: f32) -> OrbitCamera {
        Self::framing(bounds, aspect, 0.0, FRONT_VIEW_PITCH)
    }

    /// Straight-on view filling the viewport with `face` (port placement mode).
    pub fn face_view(face: &FaceRect, aspect: f32) -> OrbitCamera {
        let size = face.size() * FRAME_MARGIN;
        let tan_half = (FOV_Y * 0.5).tan();
        let for_height = size.y * 0.5 / tan_half;
        let for_width = size.x * 0.5 / (tan_half * aspect.max(0.01));
        OrbitCamera {
            target: face.center(),
            yaw: 0.0,
            pitch: 0.0,
            distance: for_height
                .max(for_width)
                .clamp(MIN_DISTANCE_MM, MAX_DISTANCE_MM),
        }
    }

    /// World-space ray through a point given in normalized device coordinates (-1..1, +y up).
    pub fn ray(&self, ndc: Vec2, aspect: f32) -> Ray {
        let inverse = self.view_proj(aspect).inverse();
        let near = inverse.project_point3(ndc.extend(0.0));
        let far = inverse.project_point3(ndc.extend(1.0));
        Ray {
            origin: near,
            dir: (far - near).normalize(),
        }
    }
}

/// Converts a pointer position in pixels (origin top-left) to normalized device coordinates.
pub fn pixel_to_ndc(px: Vec2, viewport: Vec2) -> Vec2 {
    Vec2::new(px.x / viewport.x * 2.0 - 1.0, 1.0 - px.y / viewport.y * 2.0)
}

/// An eased camera move over `TWEEN_SECONDS`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    pub from: OrbitCamera,
    pub to: OrbitCamera,
    pub start_s: f64,
}

impl Tween {
    /// The camera at time `now_s` and whether the tween has finished.
    pub fn sample(&self, now_s: f64) -> (OrbitCamera, bool) {
        let t = ((now_s - self.start_s) / TWEEN_SECONDS).clamp(0.0, 1.0) as f32;
        if t >= 1.0 {
            return (self.to, true);
        }
        let e = t * t * (3.0 - 2.0 * t);
        let lerp = |a: f32, b: f32| a + (b - a) * e;
        let camera = OrbitCamera {
            target: self.from.target.lerp(self.to.target, e),
            yaw: self.from.yaw + shortest_angle(self.from.yaw, self.to.yaw) * e,
            pitch: lerp(self.from.pitch, self.to.pitch),
            distance: lerp(self.from.distance, self.to.distance),
        };
        (camera, false)
    }
}

/// Signed difference `to - from` wrapped into (-π, π].
fn shortest_angle(from: f32, to: f32) -> f32 {
    let tau = 4.0 * FRAC_PI_2;
    let mut d = (to - from) % tau;
    if d > tau / 2.0 {
        d -= tau;
    } else if d <= -tau / 2.0 {
        d += tau;
    }
    d
}
