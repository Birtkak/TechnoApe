//! The bounce light (ADR 0022): the G-buffer the terrain pass writes, one bounce ray per pixel (`gi_trace` in
//! shader.wgsl), its history over frames and its smoothing (post.wgsl), and the composite onto the screen.
use glam::{DVec3, Mat4};

/// The G-buffer's formats, in the terrain pass's target order: direct light, albedo (body in a), normal and
/// distance (the normal as two 16-bit octahedral numbers, the distance's f32 bits: a bounce ray starts from
/// the point it rebuilds). 16 bytes a pixel, 28 before ADR 0025's J2.
pub const TARGETS: [wgpu::TextureFormat; 3] = [wgpu::TextureFormat::Rg11b10Ufloat, wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Rg32Uint];
/// A history holds this many frames at most while the camera moves: older light fades within about a
/// third of a second at 60 fps. A still shot averages without a limit, so it settles on the same picture.
const HISTORY: u32 = 24;
const HISTORY_STILL: u32 = 4096;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Post {
    prev_vp: [[f32; 4]; 4],
    /// The view ray through the screen's top-left corner, and its change across x and y (columns 0 to 2).
    rays: [[f32; 4]; 4],
    delta: [f32; 4],
    cfg: [u32; 4],
    /// The fill light when the bounce is off (rgb) and the screen's brightness (the settings').
    tune: [f32; 4],
    /// The air between the camera and what it sees: its colour at the horizon (rgb) and how fast the ground
    /// fades into it, per metre (w); and how much stronger the bounce is drawn (x), the camera's body (w).
    air: [f32; 4],
    amb: [f32; 4],
    /// The clouds' (ADR 0076, post.wgsl `Post`): the planet's centre and frame, the sun, its light, the sky's
    /// light, `Globals::cloud`.
    clouds: [[f32; 4]; 8],
}

pub struct Gi {
    direct: wgpu::TextureView,
    albedo: wgpu::TextureView,
    gbuf: [wgpu::TextureView; 2],
    trace: wgpu::ComputePipeline,
    temporal: wgpu::ComputePipeline,
    atrous: wgpu::ComputePipeline,
    composite: wgpu::RenderPipeline,
    clouds: wgpu::RenderPipeline,
    trace0: wgpu::BindGroup,
    /// Per frame parity: the trace's group 1, the temporal pass, the three smoothing passes, the composite.
    trace1: [wgpu::BindGroup; 2],
    passes: [[wgpu::BindGroup; 4]; 2],
    comp: [wgpu::BindGroup; 2],
    ubufs: Vec<wgpu::Buffer>,
    prev: Option<(Mat4, DVec3, usize)>,
    frame: u64,
    size: (u32, u32),
}

impl Gi {
    /// `shared` is the terrain's resources for the trace's group 0: globals, quads, chunks, looks, TLAS, lamps, lamp cells.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        shared: [wgpu::BindingResource; 12],
        sky: (&wgpu::TextureView, &wgpu::Sampler),
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
        // the history and its smoothing at the same half resolution (ADR 0025, C)
        let half = |label| tex_at(label, f16, st, (w.div_ceil(2), h.div_ceil(2)));
        let hist = [half("hist0"), half("hist1")];
        let (a, b) = (half("smooth-a"), half("smooth-b"));

        let trace = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gi_trace"),
            layout: None,
            module: shader,
            entry_point: Some("gi_trace"),
            compilation_options: Default::default(),
            cache: None,
        });
        let [r0, r1, r2, r3, r5, r8, r9, r11, r14, r15, r16, r17] = shared;
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
                wgpu::BindGroupEntry { binding: 9, resource: r9 },
                // the painted faces: a ray through a cutout block reads its texel (ADR 0069)
                wgpu::BindGroupEntry { binding: 11, resource: r11 },
                // the clouds' map: their shadow where a bounce ray lands (ADR 0076)
                wgpu::BindGroupEntry { binding: 14, resource: r14 },
                wgpu::BindGroupEntry { binding: 15, resource: r15 },
                // the still faces and their paintings: a plant shades a lamp's ray (ADR 0091)
                wgpu::BindGroupEntry { binding: 16, resource: r16 },
                wgpu::BindGroupEntry { binding: 17, resource: r17 },
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
            source: wgpu::ShaderSource::Wgsl(concat!(include_str!("post.wgsl"), include_str!("clouds.wgsl")).into()),
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
        let utexture = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Uint, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
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
                utexture(2, cs),
                utexture(3, cs),
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
        let ubufs: Vec<wgpu::Buffer> = (0..4)
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
        // passes go hist[p] -> a -> b -> a, binding last frame's as the unused inputs
        let passes = [0usize, 1].map(|p| {
            let q = 1 - p;
            [
                group(&ubufs[0], &noisy, &gbuf[p], &gbuf[q], &hist[q], &hist[p]),
                group(&ubufs[1], &hist[p], &gbuf[p], &gbuf[q], &hist[q], &a),
                group(&ubufs[2], &a, &gbuf[p], &gbuf[q], &hist[q], &b),
                group(&ubufs[3], &b, &gbuf[p], &gbuf[q], &hist[q], &a),
            ]
        });

        let fs = wgpu::ShaderStages::FRAGMENT;
        let cbgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: fs,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                utexture(2, fs),
                texture(6, fs),
                texture(7, fs),
                texture(8, fs),
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: fs,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 10, visibility: fs, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        // the full-resolution G-buffer of the frame's parity guides the upsampling
        let comp = [0usize, 1].map(|p| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &cbgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ubufs[0].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: view(&gbuf[p]) },
                    wgpu::BindGroupEntry { binding: 6, resource: view(&direct) },
                    wgpu::BindGroupEntry { binding: 7, resource: view(&albedo) },
                    wgpu::BindGroupEntry { binding: 8, resource: view(&a) },
                    wgpu::BindGroupEntry { binding: 9, resource: view(sky.0) },
                    wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::Sampler(sky.1) },
                ],
            })
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
        // the clouds over it all, premultiplied (post.wgsl fs_clouds)
        let clouds = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("clouds"),
            layout: Some(&clayout),
            vertex: wgpu::VertexState { module: &post, entry_point: Some("vs_composite"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &post,
                entry_point: Some("fs_clouds"),
                targets: &[Some(wgpu::ColorTargetState { format: screen, blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
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
        Gi { direct, albedo, gbuf, trace, temporal, atrous, composite, clouds, trace0, trace1, passes, comp, ubufs, prev: None, frame: 0, size: (w, h) }
    }

    /// The terrain pass's targets this frame, in `TARGETS` order.
    pub fn targets(&self) -> [&wgpu::TextureView; 3] {
        [&self.direct, &self.albedo, &self.gbuf[(self.frame % 2) as usize]]
    }

    /// The frame number for the bounce rays' seed.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Drops the history and the rays' seed: the next frame starts both anew. A shot does so once the world has
    /// loaded, so what it averages is the loaded world's light alone, however long loading took.
    pub fn drop_history(&mut self) {
        self.prev = None;
        // and the seed with it: the frames it averages get the same rays however many frames loading took
        self.frame = 0;
    }

    /// After the terrain pass: trace, gather over frames, smooth. `cam` is the camera in frame `cf`'s
    /// coordinates; a change of frame or a jump drops the history. `still` averages without a limit.
    /// `stamp(enc, k)` writes the timer after the trace (0) and after `temporal` (1) (ADR 0025).
    pub fn record(&mut self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, vp: Mat4, cam: DVec3, cf: usize, still: bool, tune: (bool, [f32; 4], [f32; 4], [f32; 4]), clouds: [[f32; 4]; 8], stamp: &mut dyn FnMut(&mut wgpu::CommandEncoder, usize)) {
        let p = (self.frame % 2) as usize;
        let (prev_vp, delta, reset) = match self.prev {
            Some((pvp, pcam, pcf)) if pcf == cf && (cam - pcam).length() < 20.0 => (pvp, (cam - pcam).as_vec3(), 0.0),
            _ => (vp, glam::Vec3::ZERO, 1.0),
        };
        let cap = if still { HISTORY_STILL } else { HISTORY };
        // the view ray is affine across the screen: three vectors give every pixel's (post.wgsl point_at)
        let inv = vp.inverse();
        let ray = |x: f32, y: f32| {
            let w = inv * glam::Vec4::new(x * 2.0 - 1.0, 1.0 - y * 2.0, 1.0, 1.0);
            w.truncate() / w.w
        };
        let r0 = ray(0.0, 0.0);
        let col = |v: glam::Vec3| [v.x, v.y, v.z, 0.0];
        let rays = [col(r0), col(ray(1.0, 0.0) - r0), col(ray(0.0, 1.0) - r0), [0.0; 4]];
        for (i, u) in self.ubufs.iter().enumerate() {
            let step = if i == 0 { 0 } else { 1 << (i - 1) };
            let post = Post {
                prev_vp: prev_vp.to_cols_array_2d(),
                rays,
                delta: [delta.x, delta.y, delta.z, reset],
                cfg: [cap, step, self.frame as u32, !tune.0 as u32],
                tune: tune.1,
                air: tune.2,
                amb: tune.3,
                clouds,
            };
            queue.write_buffer(u, 0, bytemuck::bytes_of(&post));
        }
        if !tune.0 {
            // the bounce light is off (the settings): no rays, no smoothing; the composite fills in flat
            stamp(enc, 0);
            stamp(enc, 1);
            self.prev = None;
            self.frame += 1;
            return;
        }
        let groups = (self.size.0.div_ceil(16), self.size.1.div_ceil(16));
        let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("gi"), timestamp_writes: None });
        cp.set_pipeline(&self.trace);
        cp.set_bind_group(0, &self.trace0, &[]);
        cp.set_bind_group(1, &self.trace1[p], &[]);
        cp.dispatch_workgroups(self.size.0.div_ceil(16), self.size.1.div_ceil(16), 1);
        drop(cp);
        stamp(enc, 0);
        let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("temporal"), timestamp_writes: None });
        cp.set_pipeline(&self.temporal);
        cp.set_bind_group(0, &self.passes[p][0], &[]);
        cp.dispatch_workgroups(groups.0, groups.1, 1);
        drop(cp);
        stamp(enc, 1);
        let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("atrous"), timestamp_writes: None });
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
        // record() has counted this frame already
        rp.set_bind_group(0, &self.comp[((self.frame + 1) % 2) as usize], &[]);
        rp.draw(0..3, 0..1);
    }

    /// The clouds over the screen, after the water (ADR 0076); none when the frame has none (`Post::clouds`).
    pub fn clouds<'a>(&'a self, rp: &mut wgpu::RenderPass<'a>) {
        rp.set_pipeline(&self.clouds);
        rp.set_bind_group(0, &self.comp[((self.frame + 1) % 2) as usize], &[]);
        rp.draw(0..3, 0..1);
    }
}
