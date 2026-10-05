//! wgpu renderer (spec §4.2): instanced boxes, labels, translucent ghost, selection outline.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::camera::OrbitCamera;
use crate::scene::{BACKGROUND, BoxInstance, Label, Scene};
use crate::text::{TextRasterizer, label_texture_size, mip_chain};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The browser or system offers no usable WebGPU adapter.
    NoAdapter(String),
    NoDevice(String),
    Surface(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::NoAdapter(e) => write!(f, "no WebGPU adapter: {e}"),
            RenderError::NoDevice(e) => write!(f, "could not create a GPU device: {e}"),
            RenderError::Surface(e) => write!(f, "could not use the canvas: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Outcome of `Renderer::render`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus {
    Drawn,
    /// The surface was briefly unavailable (resized, hidden, reconfigured); nothing was drawn.
    /// Request another frame.
    Skipped,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CubeVertex {
    position: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BoxRaw {
    min: [f32; 3],
    max: [f32; 3],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LabelRaw {
    rect: [f32; 4],
    z: f32,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct LabelKey {
    text: String,
    color: [u8; 4],
    width: u32,
    height: u32,
}

enum Target {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Surface {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Offscreen {
        texture: wgpu::Texture,
    },
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: Target,
    width: u32,
    height: u32,
    depth: wgpu::TextureView,
    mask: wgpu::TextureView,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    cube: wgpu::Buffer,
    box_pipeline: wgpu::RenderPipeline,
    ghost_pipeline: wgpu::RenderPipeline,
    mask_pipeline: wgpu::RenderPipeline,
    label_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,
    label_layout: wgpu::BindGroupLayout,
    outline_layout: wgpu::BindGroupLayout,
    outline_bind: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    labels: HashMap<LabelKey, wgpu::BindGroup>,
    lost: Arc<AtomicBool>,
}

fn instance() -> wgpu::Instance {
    let backends = if cfg!(target_arch = "wasm32") {
        wgpu::Backends::BROWSER_WEBGPU
    } else {
        wgpu::Backends::PRIMARY
    };
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    wgpu::Instance::new(desc)
}

async fn request_device(
    adapter: &wgpu::Adapter,
) -> Result<(wgpu::Device, wgpu::Queue), RenderError> {
    adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|e| RenderError::NoDevice(e.to_string()))
}

/// Whether this browser exposes a working WebGPU implementation.
#[cfg(target_arch = "wasm32")]
pub async fn webgpu_available() -> bool {
    wgpu::util::is_browser_webgpu_supported().await
}

impl Renderer {
    /// Renders into an offscreen RGBA8 texture (tests; `read_pixels` reads it back).
    pub async fn new_offscreen(width: u32, height: u32) -> Result<Renderer, RenderError> {
        let instance = instance();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let (device, queue) = request_device(&adapter).await?;
        let texture = create_offscreen_texture(&device, width, height);
        Ok(Self::from_parts(
            device,
            queue,
            Target::Offscreen { texture },
            OFFSCREEN_FORMAT,
            width,
            height,
        ))
    }

    /// Renders into a page canvas through WebGPU.
    #[cfg(target_arch = "wasm32")]
    pub async fn for_canvas(canvas: web_sys::HtmlCanvasElement) -> Result<Renderer, RenderError> {
        let (width, height) = (canvas.width().max(1), canvas.height().max(1));
        let instance = instance();
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let (device, queue) = request_device(&adapter).await?;
        let max = device.limits().max_texture_dimension_2d;
        let (width, height) = (width.min(max), height.min(max));
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| RenderError::Surface("canvas not supported by adapter".into()))?;
        let caps = surface.get_capabilities(&adapter);
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        surface.configure(&device, &config);
        let format = config.format;
        Ok(Self::from_parts(
            device,
            queue,
            Target::Surface { surface, config },
            format,
            width,
            height,
        ))
    }

    fn from_parts(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target: Target,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Renderer {
        let lost = Arc::new(AtomicBool::new(false));
        let flag = lost.clone();
        device.set_device_lost_callback(move |_, _| flag.store(true, Ordering::SeqCst));

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsv shaders"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let label_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("label"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let outline_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("outline"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("label"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let box_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("box"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let label_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("label"),
                bind_group_layouts: &[Some(&globals_layout), Some(&label_layout)],
                immediate_size: 0,
            });
        let outline_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("outline"),
                bind_group_layouts: &[Some(&outline_layout)],
                immediate_size: 0,
            });

        let box_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<CubeVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<BoxRaw>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![2 => Float32x3, 3 => Float32x3, 4 => Float32x4],
            }),
        ];
        let label_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LabelRaw>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32],
        })];

        let depth_state = |write: bool| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(write),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        let culled = wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        };
        let pipeline = |label: &str,
                        layout: &wgpu::PipelineLayout,
                        vs: &str,
                        fs: &str,
                        buffers: &[Option<wgpu::VertexBufferLayout>],
                        primitive: wgpu::PrimitiveState,
                        depth: Option<wgpu::DepthStencilState>,
                        target: wgpu::ColorTargetState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive,
                depth_stencil: depth,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(target)],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let opaque_target = wgpu::ColorTargetState::from(format);
        let blended_target = wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        };
        let box_pipeline = pipeline(
            "box",
            &box_layout,
            "vs_box",
            "fs_box",
            &box_buffers,
            culled,
            Some(depth_state(true)),
            opaque_target,
        );
        let ghost_pipeline = pipeline(
            "ghost",
            &box_layout,
            "vs_box",
            "fs_box",
            &box_buffers,
            culled,
            Some(depth_state(false)),
            blended_target.clone(),
        );
        let mask_pipeline = pipeline(
            "mask",
            &box_layout,
            "vs_box",
            "fs_mask",
            &box_buffers,
            culled,
            None,
            wgpu::ColorTargetState::from(MASK_FORMAT),
        );
        let strip = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        };
        let label_pipeline = pipeline(
            "label",
            &label_pipeline_layout,
            "vs_label",
            "fs_label",
            &label_buffers,
            strip,
            Some(depth_state(false)),
            blended_target.clone(),
        );
        let outline_pipeline = pipeline(
            "outline",
            &outline_pipeline_layout,
            "vs_fullscreen",
            "fs_outline",
            &[],
            wgpu::PrimitiveState::default(),
            None,
            blended_target,
        );

        let cube_vertices = unit_cube();
        let cube = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cube"),
            size: std::mem::size_of_val(cube_vertices.as_slice()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&cube, 0, bytemuck::cast_slice(&cube_vertices));

        let (depth, mask) = create_size_dependent(&device, width, height);
        let outline_bind = create_outline_bind(&device, &outline_layout, &mask);
        Renderer {
            device,
            queue,
            target,
            width,
            height,
            depth,
            mask,
            globals,
            globals_bind,
            cube,
            box_pipeline,
            ghost_pipeline,
            mask_pipeline,
            label_pipeline,
            outline_pipeline,
            label_layout,
            outline_layout,
            outline_bind,
            sampler,
            labels: HashMap::new(),
            lost,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// `true` once the GPU device has been lost; the app should offer a reload.
    pub fn is_device_lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }

    /// Largest width or height the drawing surface can have on this device.
    pub fn max_dimension(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// Resizes the drawing surface (canvas pixel size), clamped to `1..=max_dimension()`.
    /// The app must set the canvas's pixel size to `size()` afterwards so they stay equal.
    pub fn resize(&mut self, width: u32, height: u32) {
        let max = self.max_dimension();
        let (width, height) = (width.clamp(1, max), height.clamp(1, max));
        if (width, height) == (self.width, self.height) {
            return;
        }
        self.width = width;
        self.height = height;
        match &mut self.target {
            Target::Surface { surface, config } => {
                config.width = width;
                config.height = height;
                surface.configure(&self.device, config);
            }
            Target::Offscreen { texture } => {
                *texture = create_offscreen_texture(&self.device, width, height);
            }
        }
        let (depth, mask) = create_size_dependent(&self.device, width, height);
        self.depth = depth;
        self.mask = mask;
        self.outline_bind = create_outline_bind(&self.device, &self.outline_layout, &self.mask);
    }

    /// Draws one frame, or reports `Skipped` when the surface is temporarily unavailable.
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &OrbitCamera,
        text: &mut dyn TextRasterizer,
    ) -> Result<FrameStatus, RenderError> {
        let surface_texture = match &self.target {
            Target::Surface { surface, config } => match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t)
                | wgpu::CurrentSurfaceTexture::Suboptimal(t) => Some(t),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    surface.configure(&self.device, config);
                    return Ok(FrameStatus::Skipped);
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(RenderError::Surface(
                        "validation error acquiring frame".into(),
                    ));
                }
                _ => return Ok(FrameStatus::Skipped),
            },
            Target::Offscreen { .. } => None,
        };
        let view = match (&surface_texture, &self.target) {
            (Some(t), _) => t
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
            (None, Target::Offscreen { texture }) => {
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            }
            (None, Target::Surface { .. }) => unreachable!("surface frames always have a texture"),
        };

        let aspect = self.width as f32 / self.height as f32;
        let light = Vec3::new(0.4, 0.8, 0.6).normalize();
        let globals = Globals {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            light_dir: [light.x, light.y, light.z, 0.0],
        };
        self.queue
            .write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        let label_binds = self.prepare_labels(&scene.labels, text);
        let opaque = self.instance_buffer("opaque", &scene.opaque);
        let translucent = self.instance_buffer("translucent", &scene.translucent);
        let selected = self.instance_buffer("selected", &scene.selected);
        let label_raw: Vec<LabelRaw> = scene
            .labels
            .iter()
            .map(|l| LabelRaw {
                rect: [l.rect.min.x, l.rect.min.y, l.rect.max.x, l.rect.max.y],
                z: l.rect.z,
            })
            .collect();
        let label_buffer = self.vertex_buffer("labels", bytemuck::cast_slice(&label_raw));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let bg = BACKGROUND;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg.r as f64 / 255.0,
                            g: bg.g as f64 / 255.0,
                            b: bg.b as f64 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind, &[]);
            if let Some(buffer) = &opaque {
                pass.set_pipeline(&self.box_pipeline);
                pass.set_vertex_buffer(0, self.cube.slice(..));
                pass.set_vertex_buffer(1, buffer.slice(..));
                pass.draw(0..36, 0..scene.opaque.len() as u32);
            }
            if let Some(buffer) = &label_buffer {
                pass.set_pipeline(&self.label_pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                for (i, bind) in label_binds.iter().enumerate() {
                    pass.set_bind_group(1, bind, &[]);
                    pass.draw(0..4, i as u32..i as u32 + 1);
                }
            }
            if let Some(buffer) = &translucent {
                pass.set_pipeline(&self.ghost_pipeline);
                pass.set_vertex_buffer(0, self.cube.slice(..));
                pass.set_vertex_buffer(1, buffer.slice(..));
                pass.draw(0..36, 0..scene.translucent.len() as u32);
            }
        }
        if let Some(buffer) = &selected {
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("selection mask"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.mask,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.mask_pipeline);
                pass.set_bind_group(0, &self.globals_bind, &[]);
                pass.set_vertex_buffer(0, self.cube.slice(..));
                pass.set_vertex_buffer(1, buffer.slice(..));
                pass.draw(0..36, 0..scene.selected.len() as u32);
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("outline"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.outline_pipeline);
            pass.set_bind_group(0, &self.outline_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        if let Some(t) = surface_texture {
            self.queue.present(t);
        }
        Ok(FrameStatus::Drawn)
    }

    /// Reads the offscreen target back as tightly packed RGBA8 rows (top row first).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_pixels(&self) -> Option<Vec<u8>> {
        let Target::Offscreen { texture } = &self.target else {
            return None;
        };
        let unpadded = self.width * 4;
        let padded = unpadded.div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let data = buffer.slice(..).get_mapped_range().ok()?;
        let mut pixels = Vec::with_capacity((unpadded * self.height) as usize);
        for row in data.chunks(padded as usize) {
            pixels.extend_from_slice(&row[..unpadded as usize]);
        }
        Some(pixels)
    }

    fn vertex_buffer(&self, label: &str, bytes: &[u8]) -> Option<wgpu::Buffer> {
        if bytes.is_empty() {
            return None;
        }
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&buffer, 0, bytes);
        Some(buffer)
    }

    fn instance_buffer(&self, label: &str, boxes: &[BoxInstance]) -> Option<wgpu::Buffer> {
        let raw: Vec<BoxRaw> = boxes
            .iter()
            .map(|b| BoxRaw {
                min: b.aabb.min.to_array(),
                max: b.aabb.max.to_array(),
                color: b.color,
            })
            .collect();
        self.vertex_buffer(label, bytemuck::cast_slice(&raw))
    }

    /// Bind groups for every label, in order; rasterises labels not seen last frame and drops
    /// cached textures that are no longer used.
    fn prepare_labels(
        &mut self,
        labels: &[Label],
        text: &mut dyn TextRasterizer,
    ) -> Vec<wgpu::BindGroup> {
        let mut used = HashMap::with_capacity(labels.len());
        let mut binds = Vec::with_capacity(labels.len());
        for label in labels {
            let size = label.rect.size();
            let (width, height) = label_texture_size(size.x, size.y);
            let key = LabelKey {
                text: label.text.clone(),
                color: label.color,
                width,
                height,
            };
            let bind = match self.labels.remove(&key).or_else(|| used.get(&key).cloned()) {
                Some(bind) => bind,
                None => {
                    let pixels = text.rasterize(&label.text, width, height, label.color);
                    self.create_label_bind(&pixels, width, height)
                }
            };
            binds.push(bind.clone());
            used.insert(key, bind);
        }
        self.labels = used;
        binds
    }

    fn create_label_bind(&self, pixels: &[u8], width: u32, height: u32) -> wgpu::BindGroup {
        let levels = mip_chain(pixels, width, height);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("label"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, (w, h, data)) in levels.iter().enumerate() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(*h),
                },
                wgpu::Extent3d {
                    width: *w,
                    height: *h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("label"),
            layout: &self.label_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

fn create_offscreen_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn create_size_dependent(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::TextureView, wgpu::TextureView) {
    let make = |label: &str, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    (
        make(
            "depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        ),
        make(
            "selection mask",
            MASK_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        ),
    )
}

fn create_outline_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    mask: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("outline"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(mask),
        }],
    })
}

/// 36 vertices of the unit cube [0, 1]³, counter-clockwise when seen from outside.
fn unit_cube() -> Vec<CubeVertex> {
    // Each face: outward normal and its four corners counter-clockwise seen from outside.
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [0.0, 0.0, 1.0],
            [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
        ),
        (
            [1.0, 0.0, 0.0],
            [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
        ),
    ];
    faces
        .iter()
        .flat_map(|(normal, c)| {
            [c[0], c[1], c[2], c[0], c[2], c[3]].map(|position| CubeVertex {
                position,
                normal: *normal,
            })
        })
        .collect()
}
