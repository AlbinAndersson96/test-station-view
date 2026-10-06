//! Label text rasterisation (spec §4.2 "Text").

/// Pixels per millimetre of label texture resolution.
pub const LABEL_PX_PER_MM: f32 = 4.0;
pub const MAX_LABEL_PX: u32 = 2048;

/// Draws `text` centred in a `width` × `height` RGBA8 image (straight alpha, row-major,
/// top row first), scaled to fit, in `color`. Everything else is transparent.
pub trait TextRasterizer {
    fn rasterize(&mut self, text: &str, width: u32, height: u32, color: [u8; 4]) -> Vec<u8>;
}

/// Texture size for a label rectangle of `width_mm` × `height_mm`.
pub fn label_texture_size(width_mm: f32, height_mm: f32) -> (u32, u32) {
    let px = |mm: f32| ((mm * LABEL_PX_PER_MM).round() as u32).clamp(1, MAX_LABEL_PX);
    (px(width_mm), px(height_mm))
}

/// Converts straight-alpha RGBA8 to premultiplied alpha in place. Filtering and blending
/// premultiplied texels keeps transparent (black) texels from darkening the edges of text.
pub fn premultiply(pixels: &mut [u8]) {
    for texel in pixels.chunks_exact_mut(4) {
        let alpha = u32::from(texel[3]);
        for channel in &mut texel[..3] {
            *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
}

/// The full mip chain of an RGBA8 image, level 0 first, each level a 2×2 box filter of the
/// previous one (odd edges are dropped), down to 1 × 1. Lets labels stay legible when they are
/// drawn much smaller than their texture.
pub fn mip_chain(pixels: &[u8], width: u32, height: u32) -> Vec<(u32, u32, Vec<u8>)> {
    let mut levels = vec![(width, height, pixels.to_vec())];
    while let Some((w, h, src)) = levels.last().filter(|(w, h, _)| *w > 1 || *h > 1) {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut dst = Vec::with_capacity((nw * nh * 4) as usize);
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = (2 * x + dx).min(w - 1);
                        let sy = (2 * y + dy).min(h - 1);
                        sum += u32::from(src[((sy * w + sx) * 4 + c) as usize]);
                    }
                    dst.push((sum / 4) as u8);
                }
            }
        }
        levels.push((nw, nh, dst));
    }
    levels
}

/// Rasteriser using the browser's 2D canvas text engine.
#[cfg(target_arch = "wasm32")]
pub struct CanvasTextRasterizer {
    canvas: web_sys::OffscreenCanvas,
    context: web_sys::OffscreenCanvasRenderingContext2d,
}

#[cfg(target_arch = "wasm32")]
impl CanvasTextRasterizer {
    pub fn new() -> Result<CanvasTextRasterizer, wasm_bindgen::JsValue> {
        use wasm_bindgen::JsCast;
        let canvas = web_sys::OffscreenCanvas::new(1, 1)?;
        let context = canvas
            .get_context("2d")?
            .ok_or_else(|| wasm_bindgen::JsValue::from_str("no 2d context"))?
            .dyn_into::<web_sys::OffscreenCanvasRenderingContext2d>()?;
        Ok(CanvasTextRasterizer { canvas, context })
    }
}

#[cfg(target_arch = "wasm32")]
impl TextRasterizer for CanvasTextRasterizer {
    fn rasterize(&mut self, text: &str, width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        let ctx = &self.context;
        ctx.clear_rect(0.0, 0.0, width as f64, height as f64);
        let mut font_px = height as f64 * 0.8;
        ctx.set_font(&format!("600 {font_px}px system-ui, sans-serif"));
        if let Ok(metrics) = ctx.measure_text(text) {
            let max_width = width as f64 * 0.95;
            if metrics.width() > max_width {
                font_px *= max_width / metrics.width();
                ctx.set_font(&format!("600 {font_px}px system-ui, sans-serif"));
            }
        }
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.set_fill_style_str(&format!(
            "rgba({}, {}, {}, {})",
            color[0],
            color[1],
            color[2],
            color[3] as f64 / 255.0
        ));
        let _ = ctx.fill_text(text, width as f64 / 2.0, height as f64 / 2.0);
        ctx.get_image_data(0.0, 0.0, width as f64, height as f64)
            .map(|data| data.data().0)
            .unwrap_or_else(|_| vec![0; (width * height * 4) as usize])
    }
}
