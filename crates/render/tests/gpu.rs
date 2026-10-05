//! Pixel tests on a real (possibly software) GPU adapter. Skipped when none is available.

use glam::{Vec2, Vec3};
use tsv_render::camera::OrbitCamera;
use tsv_render::gpu::{FrameStatus, Renderer};
use tsv_render::layout::{Aabb, FaceRect};
use tsv_render::scene::{BACKGROUND, BoxInstance, Label, Scene};
use tsv_render::text::TextRasterizer;

const SIZE: u32 = 64;

/// Fills the whole label texture with the label colour.
struct SolidText;

impl TextRasterizer for SolidText {
    fn rasterize(&mut self, _text: &str, width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        color.repeat((width * height) as usize)
    }
}

fn renderer() -> Option<Renderer> {
    match pollster::block_on(Renderer::new_offscreen(SIZE, SIZE)) {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("skipping GPU test: {e}");
            None
        }
    }
}

/// Looks straight at the origin from 1 m; a ±100 mm box spans pixels 24..40.
fn camera() -> OrbitCamera {
    OrbitCamera {
        target: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        distance: 1000.0,
    }
}

fn red_box() -> BoxInstance {
    BoxInstance {
        aabb: Aabb::new(
            Vec3::new(-100.0, -100.0, -100.0),
            Vec3::new(100.0, 100.0, 0.0),
        ),
        color: [1.0, 0.0, 0.0, 1.0],
    }
}

fn draw(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    r.render(scene, &camera(), &mut SolidText).unwrap();
    r.read_pixels().unwrap()
}

fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

fn background() -> [u8; 4] {
    [BACKGROUND.r, BACKGROUND.g, BACKGROUND.b, 255]
}

#[test]
fn draws_a_shaded_box_on_the_background() {
    let Some(mut r) = renderer() else { return };
    let scene = Scene {
        opaque: vec![red_box()],
        ..Default::default()
    };
    let p = draw(&mut r, &scene);
    let [red, green, blue, _] = pixel(&p, 32, 32);
    assert!(
        (195..=212).contains(&red) && green < 8 && blue < 8,
        "front face {red},{green},{blue}"
    );
    assert_eq!(pixel(&p, 2, 2), background());
}

#[test]
fn selection_draws_a_glow_just_outside_the_box() {
    let Some(mut r) = renderer() else { return };
    let plain = Scene {
        opaque: vec![red_box()],
        ..Default::default()
    };
    assert_eq!(pixel(&draw(&mut r, &plain), 22, 32), background());
    let selected = Scene {
        selected: vec![red_box()],
        ..plain
    };
    let p = draw(&mut r, &selected);
    let [red, green, blue, _] = pixel(&p, 22, 32);
    assert!(
        red > 240 && (150..220).contains(&green) && blue < 120,
        "glow {red},{green},{blue}"
    );
    assert_eq!(
        pixel(&p, 10, 32),
        background(),
        "glow fades within a few pixels"
    );
}

#[test]
fn labels_show_their_texture() {
    let Some(mut r) = renderer() else { return };
    let label = Label {
        text: "DMM".into(),
        rect: FaceRect {
            min: Vec2::splat(-20.0),
            max: Vec2::splat(20.0),
            z: 1.0,
        },
        color: [0, 0, 255, 255],
    };
    let scene = Scene {
        opaque: vec![red_box()],
        labels: vec![label],
        ..Default::default()
    };
    assert_eq!(pixel(&draw(&mut r, &scene), 32, 32), [0, 0, 255, 255]);
}

#[test]
fn ghost_is_blended_over_the_scene() {
    let Some(mut r) = renderer() else { return };
    let ghost = BoxInstance {
        aabb: Aabb::new(
            Vec3::new(-300.0, -20.0, 50.0),
            Vec3::new(-150.0, 20.0, 60.0),
        ),
        color: [0.9, 0.15, 0.15, 0.45],
    };
    let scene = Scene {
        translucent: vec![ghost],
        ..Default::default()
    };
    let [red, green, blue, _] = pixel(&draw(&mut r, &scene), 15, 32);
    assert!(
        red > green + 40 && green > 100 && blue > 100,
        "reddish tint {red},{green},{blue}"
    );
}

#[test]
fn resize_changes_the_output_size() {
    let Some(mut r) = renderer() else { return };
    r.resize(32, 16);
    assert_eq!(r.size(), (32, 16));
    r.render(&Scene::default(), &camera(), &mut SolidText)
        .unwrap();
    assert_eq!(r.read_pixels().unwrap().len(), 32 * 16 * 4);
}

#[test]
fn zero_size_is_clamped_to_one_pixel() {
    let Some(mut r) = renderer() else { return };
    r.resize(0, 0);
    assert_eq!(r.size(), (1, 1));
    r.render(&Scene::default(), &camera(), &mut SolidText)
        .unwrap();
    assert_eq!(r.read_pixels().unwrap().len(), 4);
}

#[test]
fn offscreen_frames_report_that_they_were_drawn() {
    let Some(mut r) = renderer() else { return };
    assert_eq!(
        r.render(&Scene::default(), &camera(), &mut SolidText),
        Ok(FrameStatus::Drawn)
    );
}

#[test]
fn resize_is_clamped_to_the_device_texture_limit() {
    let Some(mut r) = renderer() else { return };
    r.resize(100_000, 10);
    assert_eq!(r.size(), (r.max_dimension(), 10));
    assert_eq!(
        r.render(&Scene::default(), &camera(), &mut SolidText),
        Ok(FrameStatus::Drawn)
    );
}
