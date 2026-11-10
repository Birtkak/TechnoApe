//! The bounce light (ADR 0022): the G-buffer the terrain pass writes, one bounce ray per pixel (`gi_trace` in
//! shader.wgsl), its history over frames and its smoothing (post.wgsl), and the composite onto the screen.
use glam::{DVec3, Mat4};

/// The G-buffer's formats, in the terrain pass's target order: direct light, albedo (body in a), normal and
/// distance (f32: a bounce ray starts from the point it rebuilds).
pub const TARGETS: [wgpu::TextureFormat; 3] = [wgpu::TextureFormat::Rgba16Float, wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Rgba32Float];
/// A history holds this many frames at most while the camera moves: older light fades within about a
/// third of a second at 60 fps. A still shot averages without a limit, so it settles on the same picture.
const HISTORY: u32 = 24;
const HISTORY_STILL: u32 = 4096;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Post {
    prev_vp: [[f32; 4]; 4],
    inv_vp: [[f32; 4]; 4],
    delta: [f32; 4],
    cfg: [u32; 4],
}

pub struct Gi {
    direct: wgpu::TextureView,
    albedo: wgpu::TextureView,
    gbuf: [wgpu::TextureView; 2],
    trace: wgpu::ComputePipeline,
    temporal: wgpu::ComputePipeline,
    atrous: wgpu::ComputePipeline,
    composite: wgpu::RenderPipeline,
    trace0: wgpu::BindGroup,
    /// Per frame parity: the trace's group 1, the temporal pass, the four smoothing passes.
    trace1: [wgpu::BindGroup; 2],
    passes: [[wgpu::BindGroup; 5]; 2],
    comp: wgpu::BindGroup,
    ubufs: Vec<wgpu::Buffer>,
    prev: Option<(Mat4, DVec3, usize)>,
    frame: u64,
    size: (u32, u32),
}

impl Gi {
    /// `shared` is the terrain's resources for the trace's group 0: globals, quads, chunks, looks, TLAS, lamps.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        shared: [wgpu::BindingResource; 6],
        screen: wgpu::TextureFormat,
        w: u32,
        h: u32,
    ) -> Gi {
        use wgpu::TextureUsages as T;
        let tex_at = |label, format, usage, (w, h): (u32, u32)| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let tex = |label, format, usage| tex_at(label, format, usage, (w, h));
        let rt = T::RENDER_ATTACHMENT | T::TEXTURE_BINDING;
        let st = T::STORAGE_BINDING | T::TEXTURE_BINDING;
        let f16 = wgpu::TextureFormat::Rgba16Float;
        let direct = tex("direct", TARGETS[0], rt);
        let albedo = tex("albedo", TARGETS[1], rt);
        let gbuf = [tex("gbuf0", TARGETS[2], rt), tex("gbuf1", TARGETS[2], rt)];
        // a bounce ray per 2x2 pixels
        let noisy = tex_at("noisy", f16, st, (w.div_ceil(2), h.div_ceil(2)));
        let hist = [tex("hist0", f16, st), tex("hist1", f16, st)];
        let (a, b) = (tex("smooth-a", f16, st), tex("smooth-b", f16, st));

        let trace = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gi_trace"),
            layout: None,
            module: shader,
            entry_point: Some("gi_trace"),
            compilation_options: Default::default(),
            cache: None,
        });
        let [r0, r1, r2, r3, r5, r8] = shared;
        let trace0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &trace.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: r0 },
                wgpu::BindGroupEntry { binding: 1, resource: r1 },
                wgpu::BindGroupEntry { binding: 2, resource: r2 },
                wgpu::BindGroupEntry { binding: 3, resource: r3 },
                wgpu::BindGroupEntry { binding: 5, resource: r5 },
                wgpu::BindGroupEntry { binding: 8, resource: r8 },
            ],
        });
        fn view(v: &wgpu::TextureView) -> wgpu::BindingResource<'_> {
            wgpu::BindingResource::TextureView(v)
        }
        let trace1 = [0, 1].map(|p| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &trace.get_bind_group_layout(1),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: view(&gbuf[p]) },
                    wgpu::BindGroupEntry { binding: 1, resource: view(&albedo) },
                    wgpu::BindGroupEntry { binding: 2, resource: view(&noisy) },
                ],
            })
        });

        let post = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let texture = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let cs = wgpu::ShaderStages::COMPUTE;
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: cs,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                texture(1, cs),
                texture(2, cs),
                texture(3, cs),
                texture(4, cs),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: cs,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: f16,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&bgl)], immediate_size: 0 });
        let compute = |entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module: &post,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let (temporal, atrous) = (compute("temporal"), compute("atrous"));
        let ubufs: Vec<wgpu::Buffer> = (0..5)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("post"),
                    size: std::mem::size_of::<Post>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let group = |u: &wgpu::Buffer, src: &wgpu::TextureView, gb: &wgpu::TextureView, gbp: &wgpu::TextureView, hp: &wgpu::TextureView, dst: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: u.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: view(src) },
                    wgpu::BindGroupEntry { binding: 2, resource: view(gb) },
                    wgpu::BindGroupEntry { binding: 3, resource: view(gbp) },
                    wgpu::BindGroupEntry { binding: 4, resource: view(hp) },
                    wgpu::BindGroupEntry { binding: 5, resource: view(dst) },
                ],
            })
        };
        // parity p writes gbuf[p] and hist[p] and reads last frame's gbuf[1 - p] and hist[1 - p]; the smoothing
        // passes go hist[p] -> a -> b -> a -> b, binding last frame's as the unused inputs
        let passes = [0usize, 1].map(|p| {
            let q = 1 - p;
            [
                group(&ubufs[0], &noisy, &gbuf[p], &gbuf[q], &hist[q], &hist[p]),
                group(&ubufs[1], &hist[p], &gbuf[p], &gbuf[q], &hist[q], &a),
                group(&ubufs[2], &a, &gbuf[p], &gbuf[q], &hist[q], &b),
                group(&ubufs[3], &b, &gbuf[p], &gbuf[q], &hist[q], &a),
                group(&ubufs[4], &a, &gbuf[p], &gbuf[q], &hist[q], &b),
            ]
        });

        let fs = wgpu::ShaderStages::FRAGMENT;
        let cbgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite"),
            entries: &[texture(6, fs), texture(7, fs), texture(8, fs)],
        });
        let comp = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &cbgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 6, resource: view(&direct) },
                wgpu::BindGroupEntry { binding: 7, resource: view(&albedo) },
                wgpu::BindGroupEntry { binding: 8, resource: view(&b) },
            ],
        });
        let clayout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&cbgl)], immediate_size: 0 });
        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite"),
            layout: Some(&clayout),
            vertex: wgpu::VertexState { module: &post, entry_point: Some("vs_composite"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &post,
                entry_point: Some("fs_composite"),
                targets: &[Some(screen.into())],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Gi { direct, albedo, gbuf, trace, temporal, atrous, composite, trace0, trace1, passes, comp, ubufs, prev: None, frame: 0, size: (w, h) }
    }

    /// The terrain pass's targets this frame, in `TARGETS` order.
    pub fn targets(&self) -> [&wgpu::TextureView; 3] {
        [&self.direct, &self.albedo, &self.gbuf[(self.frame % 2) as usize]]
    }

    /// The frame number for the bounce rays' seed.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// After the terrain pass: trace, gather over frames, smooth. `cam` is the camera in frame `cf`'s
    /// coordinates; a change of frame or a jump drops the history. `still` averages without a limit.
    pub fn record(&mut self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, vp: Mat4, cam: DVec3, cf: usize, still: bool) {
        let p = (self.frame % 2) as usize;
        let (prev_vp, delta, reset) = match self.prev {
            Some((pvp, pcam, pcf)) if pcf == cf && (cam - pcam).length() < 20.0 => (pvp, (cam - pcam).as_vec3(), 0.0),
            _ => (vp, glam::Vec3::ZERO, 1.0),
        };
        let cap = if still { HISTORY_STILL } else { HISTORY };
        for (i, u) in self.ubufs.iter().enumerate() {
            let step = if i == 0 { 0 } else { 1 << (i - 1) };
            let post = Post {
                prev_vp: prev_vp.to_cols_array_2d(),
                inv_vp: vp.inverse().to_cols_array_2d(),
                delta: [delta.x, delta.y, delta.z, reset],
                cfg: [cap, step, self.frame as u32, 0],
            };
            queue.write_buffer(u, 0, bytemuck::bytes_of(&post));
        }
        let groups = (self.size.0.div_ceil(8), self.size.1.div_ceil(8));
        let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("gi"), timestamp_writes: None });
        cp.set_pipeline(&self.trace);
        cp.set_bind_group(0, &self.trace0, &[]);
        cp.set_bind_group(1, &self.trace1[p], &[]);
        cp.dispatch_workgroups(self.size.0.div_ceil(16), self.size.1.div_ceil(16), 1);
        cp.set_pipeline(&self.temporal);
        cp.set_bind_group(0, &self.passes[p][0], &[]);
        cp.dispatch_workgroups(groups.0, groups.1, 1);
        cp.set_pipeline(&self.atrous);
        for g in &self.passes[p][1..] {
            cp.set_bind_group(0, g, &[]);
            cp.dispatch_workgroups(groups.0, groups.1, 1);
        }
        drop(cp);
        self.prev = Some((vp, cam, cf));
        self.frame += 1;
    }

    /// The bounce light onto the screen, under the water and the overlays.
    pub fn composite<'a>(&'a self, rp: &mut wgpu::RenderPass<'a>) {
        rp.set_pipeline(&self.composite);
        rp.set_bind_group(0, &self.comp, &[]);
        rp.draw(0..3, 0..1);
    }
}
