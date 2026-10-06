// Boxes (opaque, translucent ghost, selection mask), labels and the selection outline.

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz: direction towards the light (unit length).
    light_dir: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct BoxIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) box_min: vec3<f32>,
    @location(3) box_max: vec3<f32>,
    @location(4) color: vec4<f32>,
};

struct BoxOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    // Position within the unit cube (0..1 on each axis), for edge lines.
    @location(2) local: vec3<f32>,
};

@vertex
fn vs_box(v: BoxIn) -> BoxOut {
    let world = v.box_min + v.position * (v.box_max - v.box_min);
    var out: BoxOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.normal = v.normal;
    out.color = v.color;
    out.local = v.position;
    return out;
}

// Width of the edge lines in pixels. Edges darken light faces and lighten dark ones.
const EDGE_PX: f32 = 1.0;
const EDGE_DARKEN: f32 = 0.6;
const EDGE_LIGHTEN: f32 = 0.3;
const DARK_LUMINANCE: f32 = 0.3;

@fragment
fn fs_box(in: BoxOut) -> @location(0) vec4<f32> {
    let light = 0.5 + 0.5 * max(dot(normalize(in.normal), globals.light_dir.xyz), 0.0);
    let lit = in.color.rgb * light;
    // Distance in pixels to the face's nearest edge, ignoring the axis along the normal.
    let per_px = max(fwidth(in.local), vec3<f32>(1e-6));
    let in_plane = abs(in.normal) < vec3<f32>(0.5);
    let to_edge = select(vec3<f32>(1e9), min(in.local, 1.0 - in.local) / per_px, in_plane);
    let edge_px = min(to_edge.x, min(to_edge.y, to_edge.z));
    // Faces only a few pixels across would be all edge, so their lines fade out.
    let across = select(vec3<f32>(1e9), 1.0 / per_px, in_plane);
    let fade = smoothstep(4.0, 12.0, min(across.x, min(across.y, across.z)));
    let edge = (1.0 - smoothstep(EDGE_PX - 0.5, EDGE_PX + 0.5, edge_px)) * fade;
    let dark = dot(lit, vec3<f32>(0.2126, 0.7152, 0.0722)) < DARK_LUMINANCE;
    let edge_color = select(lit * EDGE_DARKEN, mix(lit, vec3<f32>(1.0), EDGE_LIGHTEN), dark);
    return vec4<f32>(mix(lit, edge_color, edge), in.color.a);
}

@fragment
fn fs_mask(in: BoxOut) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

@group(1) @binding(0) var label_texture: texture_2d<f32>;
@group(1) @binding(1) var label_sampler: sampler;

struct LabelIn {
    // min.x, min.y, max.x, max.y of the label rectangle (world units).
    @location(0) rect: vec4<f32>,
    @location(1) z: f32,
};

struct LabelOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Four vertices as a triangle strip; uv (0, 0) is the top-left of the texture.
@vertex
fn vs_label(@builtin(vertex_index) index: u32, l: LabelIn) -> LabelOut {
    let uv = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let x = mix(l.rect.x, l.rect.z, uv.x);
    let y = mix(l.rect.w, l.rect.y, uv.y);
    var out: LabelOut;
    out.clip = globals.view_proj * vec4<f32>(x, y, l.z, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_label(in: LabelOut) -> @location(0) vec4<f32> {
    return textureSample(label_texture, label_sampler, in.uv);
}

@group(0) @binding(0) var selection_mask: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

const GLOW_RADIUS: i32 = 3;

// Glow just outside the selection mask, fading with distance. The mask holds each pixel's
// coverage (resolved MSAA), so a neighbour with coverage c puts the shape's edge roughly
// `d - c` pixels away; that keeps the glow smooth along slanted edges.
@fragment
fn fs_outline(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let centre = vec2<i32>(position.xy);
    let last = vec2<i32>(textureDimensions(selection_mask)) - vec2<i32>(1, 1);
    let inside = textureLoad(selection_mask, centre, 0).r;
    if (inside >= 1.0) {
        discard;
    }
    var strength = 0.0;
    for (var dy = -GLOW_RADIUS; dy <= GLOW_RADIUS; dy++) {
        for (var dx = -GLOW_RADIUS; dx <= GLOW_RADIUS; dx++) {
            let q = clamp(centre + vec2<i32>(dx, dy), vec2<i32>(0, 0), last);
            let coverage = textureLoad(selection_mask, q, 0).r;
            if (coverage > 0.0) {
                let d = length(vec2<f32>(f32(dx), f32(dy)));
                strength = max(strength, 1.0 - (d - coverage) / f32(GLOW_RADIUS));
            }
        }
    }
    // Only the uncovered part of a pixel on the edge glows.
    let alpha = clamp(strength, 0.0, 1.0) * (1.0 - inside);
    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(1.0, 0.65, 0.0, alpha);
}
