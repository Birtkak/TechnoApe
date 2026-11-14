//! The traced terrain (ADR 0022): a BLAS for every chunk mesh the streamer selects, drawn or hidden, under one
//! TLAS. Every instance has the identity transform and the mask `1 << body`: rays are turned into their body's
//! frame in the shader, so the TLAS is rebuilt only when its instances change, not when the camera moves
//! (ADR 0025, B). A new mesh's quads are expanded into triangles on the GPU
//! (`expand` in shader.wgsl, the drawn geometry slightly grown) into a scratch buffer its BLAS is built from;
//! the BLAS keeps no reference to it, so the next frame reuses it. A BLAS holds two geometries (ADR 0069): the
//! opaque quads, flagged OPAQUE, then the cutout blocks' quads, not, so only those enter a ray's candidate loop
//! (shader.wgsl `trace`); a chunk with no cutout quads has the one. A chunk first built within SPLIT_M has its still models' and plants' quads (the last `still` of the land's) in a BLAS of their own, under
//! an instance with the mask `16 << body` and the chunk-table index plus `STILL_INST`: only shadow and lamp rays
//! ask for them (ADR 0104), so the bounce and the reflections no longer walk through every blade of grass they
//! never stop at. One built farther out keeps them as the second geometry's tail, and a split one stays split,
//! until its mesh goes: splitting or joining a built chunk as the camera moved cost the frame thread more builds.
use std::collections::HashMap;

/// Quads expanded and built per frame at most: a stream of new chunks is spread over frames, as meshing is.
const BUILD_QUADS: u32 = 1 << 17;
const BUILDS_MAX: usize = 256;
/// Added to a still instance's custom index (shader.wgsl `STILL_INST`): its quads start at the chunk's `sto`.
const STILL_INST: u32 = 1 << 23;
/// A chunk first built within this many metres of the camera has its stills split off: past every plant's reach
/// (96 m at most). In a stand every near chunk is; flying, the chunks come in from farther and few are.
const SPLIT_M: f32 = 128.0;
/// Split chunks at most. The TLAS keeps its one slot a chunk table entry: a TLAS with room for more cost the fly
/// run's frame thread 0.5 ms at p99 for 4096 more, 2 ms for twice as many (wgpu's build), so a split takes a
/// slot no chunk uses this frame. Past either a chunk keeps its stills in its land's BLAS, which is only slower.
const SPLIT_MAX: usize = 4096;
/// The TLAS's first size in instances (ADR 0111). wgpu walks every slot of a TLAS on each build, filled or not
/// (its wrapper and wgpu-core each make a list of `max_instances` entries): at the chunk table's 65536 a build cost
/// the frame thread about 2.7 ms whatever was in it, and up to 14 ms (orbit's one frame of 25 ms in the gate). It
/// starts this small and doubles when the instances outgrow it (`tlas_size`), a new TLAS the bind groups take
/// (`grew`).
const TLAS_FIRST: usize = 4096;

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

/// One chunk to trace this frame: its mesh's offset and land quads (the last `cut` of them cutout, the last
/// `still` of those the still models' and plants'), its chunk-table entry, its body's transform.
pub struct Inst {
    pub off: u32,
    pub land: u32,
    pub cut: u32,
    pub still: u32,
    /// Its nearest point's distance from the camera, m.
    pub dist: f32,
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
    /// By mesh offset in the quad buffer: its land quads, its BLAS and its stills' BLAS, dropped when the
    /// streamer releases the offset (ADR 0025, G).
    blas: HashMap<u32, (u32, Option<wgpu::Blas>, Option<wgpu::Blas>)>,
    n: usize,
    /// The instances the TLAS was last built from: (mesh offset, chunk-table index, body).
    built: Vec<(u32, u32, u8)>,
    /// TLAS builds, for the benchmark line.
    pub tlas_builds: u64,
    /// Chunks that waited for their BLAS in the last frame.
    pub pending: usize,
    /// Chunks whose stills are a BLAS of their own.
    split_n: usize,
    /// What each TLAS slot holds: (mesh offset, custom index, mask).
    slots: Vec<(u32, u32, u8)>,
    /// The TLAS's slots now, and at most (the chunk table's, which the splits' room is counted against).
    cap: usize,
    max: usize,
    /// The TLAS was made again this frame, larger: whatever binds it must bind the new one.
    pub grew: bool,
    /// Whether instances were ever left out for want of a slot (said once on stderr).
    overflowed: bool,
}

/// The TLAS's size for `need` instances when it has `cap` slots and may have `max`: as it is while they fit, else
/// twice what they need, a power of two
fn tlas_size(need: usize, cap: usize, max: usize) -> usize {
    if need <= cap { cap } else { (need * 2).next_power_of_two().min(max) }
}

fn new_tlas(device: &wgpu::Device, cap: usize) -> wgpu::Tlas {
    device.create_tlas(&wgpu::CreateTlasDescriptor {
        label: Some("terrain"),
        // a chunk's terrain and its stills
        max_instances: cap as u32,
        flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
    })
}

impl Rt {
    /// `sfaces`: the still faces, which place a still quad (ADR 0091). `max`: the TLAS's slots at most.
    pub fn new(device: &wgpu::Device, shader: &wgpu::ShaderModule, globals: &wgpu::Buffer, quads: &wgpu::Buffer, sfaces: &wgpu::Buffer, warp: &wgpu::TextureView, max: u32) -> Rt {
        use wgpu::BufferUsages as U;
        let cap = TLAS_FIRST.min(max as usize);
        let tlas = new_tlas(device, cap);
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
                wgpu::BindGroupEntry { binding: 16, resource: sfaces.as_entire_binding() },
                // the warp's table: `place` lays the grid (ADR 0131)
                wgpu::BindGroupEntry { binding: 18, resource: wgpu::BindingResource::TextureView(warp) },
            ],
        });
        Rt { tlas, expand, bind, builds, verts, blas: HashMap::new(), n: 0, built: Vec::new(), tlas_builds: 0, pending: 0, split_n: 0, slots: Vec::new(), cap, max: max as usize, grew: false, overflowed: false }
    }

    /// New quads were written at `off`: a BLAS from there is stale.
    pub fn forget(&mut self, off: u32) {
        if self.blas.remove(&off).is_some_and(|b| b.2.is_some()) {
            self.split_n -= 1;
        }
    }

    /// Build the missing BLAS within the budget, and the TLAS from every chunk that has one when that set or a
    /// BLAS in it changed.
    pub fn record(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, insts: &[Inst]) {
        let mut todo: Vec<Build> = Vec::new();
        let mut sizes = Vec::new();
        let mut quads = 0;
        // BLAS made this frame: a chunk with stills makes two, and the budget counts both
        let mut made = 0;
        self.pending = 0;
        // whether a chunk's stills are (to be) a BLAS of their own: once split, while it has stills; else when first
        // built near and there is room
        let most = SPLIT_MAX.min(self.max.saturating_sub(insts.len()));
        self.grew = false;
        let split = |b: Option<&(u32, Option<wgpu::Blas>, Option<wgpu::Blas>)>, c: &Inst, split_n: usize| {
            let was = b.is_some_and(|b| b.2.is_some());
            c.still > 0 && (was || b.is_none() && c.dist < SPLIT_M && split_n < most)
        };
        for c in insts {
            let b = self.blas.get(&c.off);
            let want = split(b, c, self.split_n);
            if b.is_some_and(|x| x.0 == c.land && x.2.is_some() == want) {
                continue;
            }
            let still_n = if want { c.still } else { 0 };
            if made >= BUILDS_MAX || quads + c.land > BUILD_QUADS {
                self.pending += 1;
                continue;
            }
            let geo = |n: u32, flags| wgpu::BlasTriangleGeometrySizeDescriptor {
                vertex_format: wgpu::VertexFormat::Float32x3,
                vertex_count: n * 6,
                index_format: None,
                index_count: None,
                flags,
            };
            // the opaque quads first (none in a chunk wholly of leaves), then the cutout ones; the stills apart
            let opq = c.land - c.cut;
            let sizes_of = |gs: &[(u32, wgpu::AccelerationStructureGeometryFlags)]| -> Vec<_> { gs.iter().filter(|g| g.0 > 0).map(|&(n, f)| (n, geo(n, f))).collect() };
            let new_blas = |size: &Vec<(u32, wgpu::BlasTriangleGeometrySizeDescriptor)>| {
                device.create_blas(
                    &wgpu::CreateBlasDescriptor {
                        label: None,
                        flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                        update_mode: wgpu::AccelerationStructureUpdateMode::Build,
                    },
                    wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors: size.iter().map(|g| g.1.clone()).collect() },
                )
            };
            let size = sizes_of(&[(opq, wgpu::AccelerationStructureGeometryFlags::OPAQUE), (c.cut - still_n, wgpu::AccelerationStructureGeometryFlags::empty())]);
            let still = sizes_of(&[(still_n, wgpu::AccelerationStructureGeometryFlags::empty())]);
            // (a chunk of stills alone, none of land: its terrain BLAS is empty and stays out of the TLAS)
            let blas = (!size.is_empty()).then(|| new_blas(&size));
            let sblas = (!still.is_empty()).then(|| new_blas(&still));
            made += blas.is_some() as usize + sblas.is_some() as usize;
            self.split_n += sblas.is_some() as usize;
            if self.blas.insert(c.off, (c.land, blas, sblas)).is_some_and(|b| b.2.is_some()) {
                self.split_n -= 1;
            }
            todo.push(Build { base: c.base, info: c.info, off: c.off, n: c.land, dst: quads * 6, _p: 0 });
            sizes.push((size, still));
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
            // (a chunk whose split changed waits on its old BLAS until the new one is built)
            if !self.blas.get(&c.off).is_some_and(|b| b.0 == c.land) {
                continue;
            }
            now.push((c.off, c.ci, c.body));
        }
        // a new BLAS at an offset the last build had is a change too; the TLAS is built at least once before use
        let mut rebuild = !todo.is_empty() || now != self.built || self.tlas_builds == 0;
        // the slots they take (a split chunk two): a TLAS too small for them is made again larger, every slot new
        let need = now.len() + now.iter().filter(|(off, ..)| self.blas[off].2.is_some()).count();
        let size = tlas_size(need, self.cap, self.max);
        if size != self.cap {
            eprintln!("[technoape] the traced terrain's TLAS grows to {size} slots ({need} wanted)");
            self.tlas = new_tlas(device, size);
            (self.cap, self.n, self.grew, rebuild) = (size, 0, true, true);
            self.slots.clear();
        }
        if rebuild {
            // a slot is written only when what it holds changed: the same mesh's same BLAS stays (a BLAS made this
            // frame is new even at an old offset)
            let fresh: std::collections::HashSet<u32> = todo.iter().map(|b| b.off).collect();
            let mut n = 0;
            for &(off, ci, body) in &now {
                let (_, land, still) = &self.blas[&off];
                for (b, id, mask) in [(land, ci, 1u8 << body), (still, ci | STILL_INST, 16u8 << body)] {
                    let Some(b) = b else { continue };
                    // (only when the chunks outgrow the slots the splits took: the old TLAS had none to spare)
                    if n >= self.cap {
                        if !self.overflowed {
                            eprintln!("[technoape] the traced terrain has more instances than its {} slots: some are left out", self.cap);
                            self.overflowed = true;
                        }
                        continue;
                    }
                    if self.slots.get(n) != Some(&(off, id, mask)) || fresh.contains(&off) {
                        self.tlas[n] = Some(wgpu::TlasInstance::new(b, IDENTITY, id, mask));
                        if n < self.slots.len() {
                            self.slots[n] = (off, id, mask);
                        } else {
                            self.slots.push((off, id, mask));
                        }
                    }
                    n += 1;
                }
            }
            for i in n..self.n {
                self.tlas[i] = None;
            }
            self.slots.truncate(n);
            self.n = n;
            self.built = now;
            self.tlas_builds += 1;
        }
        fn geometries<'a>(verts: &'a wgpu::Buffer, first: u32, size: &'a [(u32, wgpu::BlasTriangleGeometrySizeDescriptor)]) -> Vec<wgpu::BlasTriangleGeometry<'a>> {
            let mut first = first;
            size.iter()
                .map(|(n, size)| {
                    let g = wgpu::BlasTriangleGeometry {
                        size,
                        vertex_buffer: verts,
                        first_vertex: first,
                        vertex_stride: 12,
                        index_buffer: None,
                        first_index: None,
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    };
                    first += n * 6;
                    g
                })
                .collect()
        }
        let mut entries: Vec<wgpu::BlasBuildEntry> = Vec::with_capacity(todo.len() * 2);
        for (b, (size, still)) in todo.iter().zip(&sizes) {
            let (_, land, sb) = &self.blas[&b.off];
            if let Some(blas) = land {
                entries.push(wgpu::BlasBuildEntry { blas, geometry: wgpu::BlasGeometries::TriangleGeometries(geometries(&self.verts, b.dst, size)) });
            }
            if let (Some(blas), Some((n, _))) = (sb, still.first()) {
                // the stills are the last of the chunk's expanded land quads
                entries.push(wgpu::BlasBuildEntry { blas, geometry: wgpu::BlasGeometries::TriangleGeometries(geometries(&self.verts, b.dst + (b.n - n) * 6, still)) });
            }
        }
        if rebuild {
            enc.build_acceleration_structures(entries.iter(), std::iter::once(&self.tlas));
        }
    }
}

#[cfg(test)]
mod tests {
    /// The TLAS keeps its size while the instances fit, and outgrown takes twice their need, never past the most
    #[test]
    fn the_tlas_grows_to_twice_its_need_and_no_further_than_the_most() {
        use super::tlas_size;
        assert_eq!(tlas_size(100, 4096, 65536), 4096);
        assert_eq!(tlas_size(4096, 4096, 65536), 4096);
        assert_eq!(tlas_size(4097, 4096, 65536), 16384);
        assert_eq!(tlas_size(9000, 16384, 65536), 16384);
        assert_eq!(tlas_size(40000, 16384, 65536), 65536);
        assert_eq!(tlas_size(70000, 65536, 65536), 65536);
    }
}
