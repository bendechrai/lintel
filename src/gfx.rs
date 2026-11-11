//! The 3D renderer: a `rune::Compositor` that draws the `Scene` into the `rune::effect(SLOT)`
//! element of the view.
//!
//! Each frame with a viewport on screen it renders the scene into its own multisampled target
//! (a sun shadow map first, then sky, opaque and translucent triangles), and then draws Rune's
//! UI, pasting the resolved image in at the slot's place in painter order, so anything the view
//! puts over the viewport element draws over the 3D image.

use std::sync::Arc;

use rune::compose::DeviceKey;
use rune::render::KIND_EFFECT;
use wgpu::util::DeviceExt;

use crate::camera::{Camera, Projection, mat_mul};
use crate::model::Pick;
use crate::scene::{Scene, Vertex, add, cross, dot, mul, normalize, sub};

/// The `El::effect` mark of the 3D viewport element.
pub const SLOT: u32 = 0x3d;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    /// Radians from +X toward +Y, where the sun is.
    pub azimuth: f32,
    /// Radians above the horizon.
    pub altitude: f32,
}

impl Default for Sun {
    fn default() -> Self {
        Sun { azimuth: -1.1, altitude: 0.85 }
    }
}

impl Sun {
    /// Unit vector from the ground toward the sun.
    pub fn direction(&self) -> [f32; 3] {
        let alt = self.altitude.clamp(0.05, 1.55);
        [self.azimuth.cos() * alt.cos(), self.azimuth.sin() * alt.cos(), alt.sin()]
    }
}

/// What the 3D viewport shows this frame.
#[derive(Clone)]
pub struct Viewport {
    pub scene: Arc<Scene>,
    /// Changes whenever `scene` does, so the renderer knows to upload it again.
    pub revision: u64,
    pub camera: Camera,
    pub sun: Sun,
    pub selected: Option<Pick>,
    pub hovered: Option<Pick>,
}

/// App state the renderer can draw from.
pub trait Host: 'static {
    /// The viewport to draw, when the view has a `SLOT` element on screen.
    fn viewport(&self) -> Option<Viewport>;
}

const SHADOW_SIZE: u32 = 2048;
const SAMPLES: u32 = 4;
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_vp: [[f32; 4]; 4],
    eye: [f32; 4],
    sun: [f32; 4],
    cut: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    fwd: [f32; 4],
    sel: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BlitUniform {
    rect: [f32; 4],
    flags: [f32; 4],
}

/// The offscreen image the scene is drawn into, sized to the slot.
struct Targets {
    size: (u32, u32),
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolved: wgpu::TextureView,
    blit_bind: wgpu::BindGroup,
}

struct Mesh {
    revision: u64,
    scene: usize,
    vertices: wgpu::Buffer,
    opaque: Option<(wgpu::Buffer, u32)>,
    translucent: Option<(wgpu::Buffer, u32)>,
}

/// Everything made on one device.
struct Dev {
    key: DeviceKey,
    globals: wgpu::Buffer,
    shadow_globals: wgpu::BindGroup,
    scene_bind: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    shadow_pipe: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    opaque_pipe: wgpu::RenderPipeline,
    translucent_pipe: wgpu::RenderPipeline,
    blit_module: wgpu::ShaderModule,
    blit_layout: wgpu::PipelineLayout,
    blit_bgl: wgpu::BindGroupLayout,
    blit_uniform: wgpu::Buffer,
    blit_sampler: wgpu::Sampler,
    blit_pipes: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    targets: Option<Targets>,
    mesh: Option<Mesh>,
}

#[derive(Default)]
pub struct Gfx {
    dev: Option<Dev>,
}

impl Gfx {
    pub fn new() -> Gfx {
        Gfx::default()
    }
}

fn texture(device: &wgpu::Device, label: &str, size: (u32, u32), samples: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

impl Dev {
    fn new(device: &wgpu::Device, key: DeviceKey) -> Dev {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("lintel scene"), source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
        let blit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("lintel blit"), source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()) });

        let uniform = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let fr = wgpu::ShaderStages::FRAGMENT;
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("lintel shadow"), entries: &[entry(0, vf, uniform)] });
        let scene_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lintel scene"),
            entries: &[
                entry(0, vf, uniform),
                entry(1, fr, wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }),
                entry(2, fr, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)),
            ],
        });
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lintel blit"),
            entries: &[
                entry(0, vf, uniform),
                entry(1, fr, wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }),
                entry(2, fr, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let layout = |label, bgl: &wgpu::BindGroupLayout| device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(label), bind_group_layouts: &[Some(bgl)], immediate_size: 0 });
        let shadow_layout = layout("lintel shadow", &shadow_bgl);
        let scene_layout = layout("lintel scene", &scene_bgl);
        let blit_layout = layout("lintel blit", &blit_bgl);

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lintel globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let blit_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lintel blit"),
            size: std::mem::size_of::<BlitUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_view = texture(device, "lintel shadow map", (SHADOW_SIZE, SHADOW_SIZE), 1, DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lintel shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("lintel blit"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let shadow_globals = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lintel shadow"),
            layout: &shadow_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let scene_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lintel scene"),
            layout: &scene_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
            ],
        });

        const ATTRS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x3, 3 => Float32, 4 => Float32x4, 5 => Float32x2, 6 => Float32, 7 => Float32];
        let buffers = [Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &ATTRS })];
        let depth = |write: bool, compare, bias| wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: Some(write),
            depth_compare: Some(compare),
            stencil: Default::default(),
            bias,
        };
        let multisample = wgpu::MultisampleState { count: SAMPLES, mask: !0, alpha_to_coverage_enabled: false };

        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lintel shadow"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &buffers },
            primitive: Default::default(),
            depth_stencil: Some(depth(true, wgpu::CompareFunction::Less, wgpu::DepthBiasState { constant: 2, slope_scale: 1.5, clamp: 0.0 })),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        let scene_pipe = |label, blend, write| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&scene_layout),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &buffers },
                primitive: Default::default(),
                depth_stencil: Some(depth(write, wgpu::CompareFunction::Less, Default::default())),
                multisample,
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: COLOR, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let opaque_pipe = scene_pipe("lintel opaque", None, true);
        let translucent_pipe = scene_pipe("lintel translucent", Some(wgpu::BlendState::ALPHA_BLENDING), false);
        let sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lintel sky"),
            layout: Some(&scene_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_sky"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: Some(depth(false, wgpu::CompareFunction::Always, Default::default())),
            multisample,
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Dev {
            key,
            globals,
            shadow_globals,
            scene_bind,
            shadow_view,
            shadow_pipe,
            sky_pipe,
            opaque_pipe,
            translucent_pipe,
            blit_module,
            blit_layout,
            blit_bgl,
            blit_uniform,
            blit_sampler,
            blit_pipes: Vec::new(),
            targets: None,
            mesh: None,
        }
    }

    fn blit_pipe(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.blit_pipes.iter().any(|(f, _)| *f == format) {
            return;
        }
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lintel blit"),
            layout: Some(&self.blit_layout),
            vertex: wgpu::VertexState { module: &self.blit_module, entry_point: Some("vs_blit"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.blit_module,
                entry_point: Some("fs_blit"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.blit_pipes.push((format, pipe));
    }

    fn targets(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let msaa = texture(device, "lintel colour", size, SAMPLES, COLOR, attach);
        let depth = texture(device, "lintel depth", size, SAMPLES, DEPTH, attach);
        let resolved = texture(device, "lintel resolved", size, 1, COLOR, attach | wgpu::TextureUsages::TEXTURE_BINDING);
        let blit_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lintel blit"),
            layout: &self.blit_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.blit_uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&resolved) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.blit_sampler) },
            ],
        });
        self.targets = Some(Targets { size, msaa, depth, resolved, blit_bind });
    }

    fn mesh(&mut self, device: &wgpu::Device, view: &Viewport) {
        let scene = Arc::as_ptr(&view.scene) as usize;
        if self.mesh.as_ref().is_some_and(|m| m.revision == view.revision && m.scene == scene) {
            return;
        }
        let s = &*view.scene;
        let indices = |label, list: &[u32]| {
            let whole = list.len() / 3 * 3;
            (whole > 0).then(|| (device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: bytemuck::cast_slice(&list[..whole]), usage: wgpu::BufferUsages::INDEX }), whole as u32))
        };
        let empty = [Vertex::zeroed()];
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lintel vertices"),
            contents: bytemuck::cast_slice(if s.vertices.is_empty() { &empty[..] } else { &s.vertices[..] }),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.mesh = Some(Mesh { revision: view.revision, scene, vertices, opaque: indices("lintel opaque", &s.opaque), translucent: indices("lintel translucent", &s.translucent) });
    }
}

use bytemuck::Zeroable;

/// The box the sun's shadow map covers: the scene's building bounds, or every vertex when the
/// scene doesn't say.
fn shadow_bounds(scene: &Scene) -> ([f32; 3], [f32; 3]) {
    let (lo, hi) = scene.bounds;
    if (0..3).all(|i| lo[i].is_finite() && hi[i].is_finite()) && (0..3).any(|i| hi[i] - lo[i] > 0.01) {
        return (lo, hi);
    }
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for v in &scene.vertices {
        for i in 0..3 {
            lo[i] = lo[i].min(v.pos[i]);
            hi[i] = hi[i].max(v.pos[i]);
        }
    }
    if scene.vertices.is_empty() {
        return ([-1.0; 3], [1.0; 3]);
    }
    // A wide site would spread the map thin: keep to 40 m around the middle.
    let mid = mul(add(lo, hi), 0.5);
    (std::array::from_fn(|i| lo[i].max(mid[i] - 40.0)), std::array::from_fn(|i| hi[i].min(mid[i] + 40.0)))
}

/// The sun's orthographic view-projection over a box, and the size of one shadow texel in metres.
fn light_matrix(sun: [f32; 3], bounds: ([f32; 3], [f32; 3])) -> ([[f32; 4]; 4], f32) {
    let (lo, hi) = bounds;
    let center = mul(add(lo, hi), 0.5);
    let d = sub(hi, lo);
    // Room around the building for the shadow it throws on the ground.
    let radius = dot(d, d).sqrt() * 0.5 + 5.0 + (hi[2] - lo[2]).max(0.0) * 0.8;
    let f = mul(sun, -1.0);
    let r = normalize(cross(f, if sun[2].abs() > 0.99 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] }));
    let u = cross(r, f);
    let eye = add(center, mul(sun, radius * 2.0));
    // Snap the map to whole texels so shadow edges don't crawl as the bounds change slightly.
    let texel = radius * 2.0 / SHADOW_SIZE as f32;
    let snap = |v: f32| (v / texel).round() * texel;
    let view = [[r[0], u[0], -f[0], 0.0], [r[1], u[1], -f[1], 0.0], [r[2], u[2], -f[2], 0.0], [-snap(dot(r, eye)), -snap(dot(u, eye)), dot(f, eye), 1.0]];
    let (near, far) = (0.0, radius * 4.0);
    let proj = [[1.0 / radius, 0.0, 0.0, 0.0], [0.0, 1.0 / radius, 0.0, 0.0], [0.0, 0.0, -1.0 / (far - near), 0.0], [0.0, 0.0, -near / (far - near), 1.0]];
    (mat_mul(proj, view), texel)
}

fn globals(view: &Viewport, aspect: f32) -> Globals {
    let cam = &view.camera;
    let b = cam.basis();
    let sun = view.sun.direction();
    let (light_vp, texel) = light_matrix(sun, shadow_bounds(&view.scene));
    let (ortho, t) = match cam.projection {
        Projection::Perspective { fov_y } => (0.0, (fov_y * 0.5).tan()),
        Projection::Orthographic { .. } => (1.0, 0.0),
    };
    let eye = cam.eye();
    let pick = |p: Option<Pick>| p.and_then(|p| view.scene.picks.iter().position(|q| *q == p)).map_or(0.0, |i| (i + 1) as f32);
    let v4 = |v: [f32; 3], w: f32| [v[0], v[1], v[2], w];
    Globals {
        view_proj: cam.view_proj(aspect),
        light_vp,
        eye: v4(eye, ortho),
        sun: v4(sun, texel),
        cut: cam.cut.unwrap_or([0.0; 4]),
        right: v4(mul(b.right, t * aspect), 0.0),
        up: v4(mul(b.up, t), 0.0),
        fwd: v4(b.forward, 0.0),
        sel: [pick(view.selected), pick(view.hovered), 0.0, 0.0],
    }
}

impl<S: Host> rune::Compositor<S> for Gfx {
    fn render(&mut self, f: &mut rune::Frame<'_, S>) {
        let Some(view) = f.rt.state.viewport() else {
            f.draw_ui();
            return;
        };
        // The slot's rectangle in window device pixels, from its record in the painter stream.
        let slot = f.rt.instances.iter().enumerate().find_map(|(n, i)| {
            let ours = f.rt.marks.get(n).is_none_or(|m| *m == SLOT);
            (i.params[1] == KIND_EFFECT && ours).then_some(i.rect)
        });
        let size = slot.map(|r| (r[2].round().max(0.0) as u32, r[3].round().max(0.0) as u32)).filter(|s| s.0 > 0 && s.1 > 0);
        let (Some(rect), Some(size)) = (slot, size) else {
            f.draw_ui();
            return;
        };
        let size = (size.0.min(8192), size.1.min(8192));

        let key = f.device_key();
        if self.dev.as_ref().is_none_or(|d| d.key != key) {
            self.dev = Some(Dev::new(f.device, key));
        }
        let Some(dev) = self.dev.as_mut() else { return };
        dev.targets(f.device, size);
        dev.mesh(f.device, &view);
        dev.blit_pipe(f.device, f.format);
        // A slot inside a fading or transformed element is drawn into an isolated layer.
        dev.blit_pipe(f.device, wgpu::TextureFormat::Rgba16Float);

        let g = globals(&view, size.0 as f32 / size.1 as f32);
        f.queue.write_buffer(&dev.globals, 0, bytemuck::bytes_of(&g));
        let (w, h) = (f.size.0.max(1) as f32, f.size.1.max(1) as f32);
        let ndc = [rect[0] / w * 2.0 - 1.0, 1.0 - rect[1] / h * 2.0, (rect[0] + rect[2]) / w * 2.0 - 1.0, 1.0 - (rect[1] + rect[3]) / h * 2.0];
        let blit = BlitUniform { rect: ndc, flags: [if f.srgb_blend() { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0] };
        f.queue.write_buffer(&dev.blit_uniform, 0, bytemuck::bytes_of(&blit));

        let (Some(targets), Some(mesh)) = (dev.targets.as_ref(), dev.mesh.as_ref()) else { return };
        {
            let mut pass = f.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lintel shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &dev.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some((indices, count)) = &mesh.opaque {
                pass.set_pipeline(&dev.shadow_pipe);
                pass.set_bind_group(0, &dev.shadow_globals, &[]);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*count, 0, 0..1);
            }
        }
        {
            let mut pass = f.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lintel scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.msaa,
                    depth_slice: None,
                    resolve_target: Some(&targets.resolved),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &dev.scene_bind, &[]);
            pass.set_pipeline(&dev.sky_pipe);
            pass.draw(0..3, 0..1);
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            for (pipe, list) in [(&dev.opaque_pipe, &mesh.opaque), (&dev.translucent_pipe, &mesh.translucent)] {
                if let Some((indices, count)) = list {
                    pass.set_pipeline(pipe);
                    pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*count, 0, 0..1);
                }
            }
        }

        let clear = f.rt.clear_color();
        f.ui.prepare(f.device, f.queue, &mut f.rt.text.atlas, &f.rt.instances, f.size);
        let pipes = &dev.blit_pipes;
        f.ui.draw_with(f.encoder, f.target, clear, &mut |pass, site| {
            if site.mark != SLOT {
                return;
            }
            let Some((_, pipe)) = pipes.iter().find(|(format, _)| *format == site.format) else { return };
            // The rectangle is in window NDC; inside a layer the pass's origin is elsewhere.
            let shifted = site.origin != [0.0; 2] || site.size != [w as u32, h as u32];
            if shifted {
                pass.set_viewport(-site.origin[0], -site.origin[1], w, h, 0.0, 1.0);
            }
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, &targets.blit_bind, &[]);
            pass.draw(0..4, 0..1);
            if shifted {
                pass.set_viewport(0.0, 0.0, site.size[0] as f32, site.size[1] as f32, 0.0, 1.0);
            }
        });
    }
}
