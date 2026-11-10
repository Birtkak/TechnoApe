//! The traced terrain (ADR 0022): a BLAS for every chunk mesh the streamer selects, drawn or hidden, under one
//! TLAS. Every instance has the identity transform and the mask `1 << body`: rays are turned into their body's
//! frame in the shader, so the TLAS is rebuilt only when its instances change, not when the camera moves
//! (ADR 0025, B). A new mesh's quads are expanded into triangles on the GPU
//! (`expand` in shader.wgsl, the drawn geometry slightly grown) into a scratch buffer its BLAS is built from;
//! the BLAS keeps no reference to it, so the next frame reuses it.
use std::collections::HashMap;

/// Quads expanded and built per frame at most: a stream of new chunks is spread over frames, as meshing is.
const BUILD_QUADS: u32 = 1 << 17;
const BUILDS_MAX: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Build {
    base: [i32; 3],
    info: u32,
    off: u32,
    n: u32,
    dst: u32,
    _p: u32,
}

/// One chunk to trace this frame: its mesh's offset and land quads, its chunk-table entry, its body's transform.
pub struct Inst {
    pub off: u32,
    pub land: u32,
    pub base: [i32; 3],
    pub info: u32,
    pub body: u8,
    /// Its index in the chunk table: a reflection ray finds the quad it hit from there.
    pub ci: u32,
}

pub struct Rt {
    pub tlas: wgpu::Tlas,
    expand: wgpu::ComputePipeline,
    bind: wgpu::BindGroup,
    builds: wgpu::Buffer,
    verts: wgpu::Buffer,
    /// By mesh offset in the quad buffer: its land quads and its BLAS, dropped when the streamer releases the
    /// offset (ADR 0025, G).
    blas: HashMap<u32, (u32, wgpu::Blas)>,
    n: usize,
    /// The instances the TLAS was last built from: (mesh offset, chunk-table index, body).
    built: Vec<(u32, u32, u8)>,
    /// TLAS builds, for the benchmark line.
    pub tlas_builds: u64,
    /// Chunks that waited for their BLAS in the last frame.
    pub pending: usize,
}

impl Rt {
    pub fn new(device: &wgpu::Device, shader: &wgpu::ShaderModule, globals: &wgpu::Buffer, quads: &wgpu::Buffer, max: u32) -> Rt {
        use wgpu::BufferUsages as U;
        let tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("terrain"),
            max_instances: max,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });
        let buf = |label, size: u64, usage| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        let builds = buf("builds", (BUILDS_MAX * std::mem::size_of::<Build>()) as u64, U::STORAGE | U::COPY_DST);
        let verts = buf("rt-verts", BUILD_QUADS as u64 * 6 * 12, U::STORAGE | U::BLAS_INPUT);
        let expand = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("expand"),
            layout: None,
            module: shader,
            entry_point: Some("expand"),
            compilation_options: Default::default(),
            cache: None,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &expand.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: quads.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: builds.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: verts.as_entire_binding() },
            ],
        });
        Rt { tlas, expand, bind, builds, verts, blas: HashMap::new(), n: 0, built: Vec::new(), tlas_builds: 0, pending: 0 }
    }

    /// New quads were written at `off`: a BLAS from there is stale.
    pub fn forget(&mut self, off: u32) {
        self.blas.remove(&off);
    }

    /// Build the missing BLAS within the budget, and the TLAS from every chunk that has one when that set or a
    /// BLAS in it changed.
    pub fn record(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, insts: &[Inst]) {
        let mut todo: Vec<Build> = Vec::new();
        let mut sizes = Vec::new();
        let mut quads = 0;
        self.pending = 0;
        for c in insts {
            if self.blas.get(&c.off).is_some_and(|b| b.0 == c.land) {
                continue;
            }
            if todo.len() >= BUILDS_MAX || quads + c.land > BUILD_QUADS {
                self.pending += 1;
                continue;
            }
            let size = wgpu::BlasTriangleGeometrySizeDescriptor {
                vertex_format: wgpu::VertexFormat::Float32x3,
                vertex_count: c.land * 6,
                index_format: None,
                index_count: None,
                flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
            };
            let blas = device.create_blas(
                &wgpu::CreateBlasDescriptor {
                    label: None,
                    flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                    update_mode: wgpu::AccelerationStructureUpdateMode::Build,
                },
                wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors: vec![size.clone()] },
            );
            self.blas.insert(c.off, (c.land, blas));
            todo.push(Build { base: c.base, info: c.info, off: c.off, n: c.land, dst: quads * 6, _p: 0 });
            sizes.push(size);
            quads += c.land;
        }
        if !todo.is_empty() {
            queue.write_buffer(&self.builds, 0, bytemuck::cast_slice(&todo));
            let most = todo.iter().map(|b| b.n).max().unwrap();
            let mut cp = enc.begin_compute_pass(&Default::default());
            cp.set_pipeline(&self.expand);
            cp.set_bind_group(0, &self.bind, &[]);
            cp.dispatch_workgroups(most.div_ceil(64), todo.len() as u32, 1);
        }
        const IDENTITY: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let mut now = Vec::with_capacity(insts.len());
        for c in insts {
            if !self.blas.get(&c.off).is_some_and(|b| b.0 == c.land) {
                continue;
            }
            now.push((c.off, c.ci, c.body));
        }
        // a new BLAS at an offset the last build had is a change too; the TLAS is built at least once before use
        let rebuild = !todo.is_empty() || now != self.built || self.tlas_builds == 0;
        if rebuild {
            for (n, &(off, ci, body)) in now.iter().enumerate() {
                self.tlas[n] = Some(wgpu::TlasInstance::new(&self.blas[&off].1, IDENTITY, ci, 1 << body));
            }
            for i in now.len()..self.n {
                self.tlas[i] = None;
            }
            self.n = now.len();
            self.built = now;
            self.tlas_builds += 1;
        }
        let entries: Vec<wgpu::BlasBuildEntry> = todo
            .iter()
            .zip(&sizes)
            .map(|(b, size)| wgpu::BlasBuildEntry {
                blas: &self.blas[&b.off].1,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry {
                    size,
                    vertex_buffer: &self.verts,
                    first_vertex: b.dst,
                    vertex_stride: 12,
                    index_buffer: None,
                    first_index: None,
                    transform_buffer: None,
                    transform_buffer_offset: None,
                }]),
            })
            .collect();
        if rebuild {
            enc.build_acceleration_structures(entries.iter(), std::iter::once(&self.tlas));
        }
    }
}
