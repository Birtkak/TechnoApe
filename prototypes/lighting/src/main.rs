//! Lighting spike: what hardware ray queries cost on the planet's real terrain (see README.md).
//! `lighting r=8 mode=0|1|2 out=shot.png`: r = chunk radius around the spawn (level 0, 16 m chunks);
//! mode 0 = camera ray + soft sun shadow, 1 = + a reflection ray on every pixel (all metal), 2 = + 4 shadow
//! samples. Prints build and trace times.
use engine::mesh::mesh;
use engine::world::{generate, Gen, Key, World, CS};
use glam::{DVec3, IVec3};
use std::collections::HashMap;
use std::time::Instant;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Cam {
    f: [f32; 4],
    r: [f32; 4],
    u: [f32; 4],
    sun: [f32; 4],
    size: [u32; 4],
}

fn main() {
    let args: HashMap<String, String> = std::env::args().skip(1).filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    let get = |k: &str, d: i64| args.get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let (rad, mode) = (get("r", 8) as i32, get("mode", 0) as u32);
    let (w, h) = (1920u32, 1080u32);

    // the terrain: level 0 chunks around the spawn, as world-space triangles relative to the eye
    let world = World::new(12288, 512, 1337);
    let b = &world.bodies[0];
    let up = b.spawn;
    let eye = b.centre + up * (b.height(up) + 1.7);
    let blk = world.block_at(eye);
    let (cx, cy, cz) = (blk.pos.x.div_euclid(CS), blk.pos.y.div_euclid(CS), blk.pos.z.div_euclid(CS));
    let t = Instant::now();
    let mut chunks: Vec<Vec<[f32; 3]>> = Vec::new();
    let per = b.n / CS;
    for dz in -2..=1 {
        for dy in -rad..=rad {
            for dx in -rad..=rad {
                let pos = IVec3::new(cx + dx, cy + dy, cz + dz);
                if pos.x < 0 || pos.y < 0 || pos.x >= per || pos.y >= per {
                    continue;
                }
                let k = Key { body: 0, face: blk.face, level: 0, pos };
                let Gen::Cells(c) = generate(b, k, &[]) else { continue };
                let m = mesh(&c, true);
                let base = k.base();
                let at = |l: IVec3| {
                    let g = base + l;
                    let p = b.centre + b.dir(k.face as usize, g.x as f64, g.y as f64) * b.layer_r(g.z as f64) - eye;
                    [p.x as f32, p.y as f32, p.z as f32]
                };
                let mut v = Vec::with_capacity(m.quads.len() * 6);
                for q in &m.quads {
                    let lo = q[0];
                    let p = IVec3::new((lo & 63) as i32, (lo >> 6 & 63) as i32, (lo >> 12 & 63) as i32);
                    let (qw, qh, a) = (((lo >> 18) & 31) as i32 + 1, ((lo >> 23) & 31) as i32 + 1, ((lo >> 28) & 7) / 2);
                    let axis = |i: u32| IVec3::new((i == 0) as i32, (i == 1) as i32, (i == 2) as i32);
                    let (u, vv) = (axis((a + 1) % 3) * qw, axis((a + 2) % 3) * qh);
                    let c = [at(p), at(p + u), at(p + u + vv), at(p + vv)];
                    v.extend([c[0], c[1], c[2], c[0], c[2], c[3]]);
                }
                if !v.is_empty() {
                    chunks.push(v);
                }
            }
        }
    }
    let tris: usize = chunks.iter().map(|c| c.len() / 3).sum();
    println!("terrain: {} chunks, {} triangles, cpu {:.0} ms", chunks.len(), tris, t.elapsed().as_secs_f64() * 1e3);

    let mut idesc = wgpu::InstanceDescriptor::new_without_display_handle();
    idesc.backends = wgpu::Backends::VULKAN;
    let inst = wgpu::Instance::new(idesc);
    let adapter = pollster::block_on(inst.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() })).unwrap();
    println!("gpu: {}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: None,
        required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY,
        required_limits: wgpu::Limits::default().using_minimum_supported_acceleration_structure_values(),
        // SAFETY: a throwaway spike; ray queries are behind wgpu's experimental gate
        experimental_features: unsafe { wgpu::ExperimentalFeatures::enabled() },
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .unwrap();
    let wait = || {
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
    };

    // one BLAS per chunk, all vertices also in one storage buffer for the normals
    let all: Vec<[f32; 4]> = chunks.iter().flatten().map(|p| [p[0], p[1], p[2], 0.0]).collect();
    let verts = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&all), usage: wgpu::BufferUsages::STORAGE });
    let mut first = 0u32;
    let mut blas = Vec::new();
    for c in &chunks {
        let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(c), usage: wgpu::BufferUsages::BLAS_INPUT });
        let size = wgpu::BlasTriangleGeometrySizeDescriptor { vertex_format: wgpu::VertexFormat::Float32x3, vertex_count: c.len() as u32, index_format: None, index_count: None, flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE };
        let bl = device.create_blas(
            &wgpu::CreateBlasDescriptor { label: None, flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE, update_mode: wgpu::AccelerationStructureUpdateMode::Build },
            wgpu::BlasGeometrySizeDescriptors::Triangles { descriptors: vec![size.clone()] },
        );
        blas.push((bl, buf, size, first));
        first += c.len() as u32;
    }
    let mut tlas = device.create_tlas(&wgpu::CreateTlasDescriptor { label: None, max_instances: blas.len() as u32, flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE, update_mode: wgpu::AccelerationStructureUpdateMode::Build });
    for (i, (bl, _, _, first)) in blas.iter().enumerate() {
        tlas[i] = Some(wgpu::TlasInstance::new(bl, [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.], *first, 0xff));
    }
    let build = |which: &[usize]| {
        let entries: Vec<wgpu::BlasBuildEntry> = which
            .iter()
            .map(|&i| {
                let (bl, buf, size, _) = &blas[i];
                wgpu::BlasBuildEntry {
                    blas: bl,
                    geometry: wgpu::BlasGeometries::TriangleGeometries(vec![wgpu::BlasTriangleGeometry { size, vertex_buffer: buf, first_vertex: 0, vertex_stride: 12, index_buffer: None, first_index: None, transform_buffer: None, transform_buffer_offset: None }]),
                }
            })
            .collect();
        let mut enc = device.create_command_encoder(&Default::default());
        enc.build_acceleration_structures(entries.iter(), std::iter::once(&tlas));
        queue.submit([enc.finish()]);
    };
    let all_ids: Vec<usize> = (0..blas.len()).collect();
    let t = Instant::now();
    build(&all_ids);
    wait();
    println!("build all {} BLAS + TLAS: {:.1} ms", blas.len(), t.elapsed().as_secs_f64() * 1e3);
    // a streaming frame: 8 chunks rebuilt (an edit or newly meshed chunks) and the TLAS
    let t = Instant::now();
    for _ in 0..20 {
        build(&all_ids[..8.min(all_ids.len())]);
    }
    wait();
    println!("rebuild 8 BLAS + TLAS: {:.2} ms", t.elapsed().as_secs_f64() * 1e3 / 20.0);

    // the camera looks along the ground, 15 degrees down; the sun 30 degrees up, 0.53 degrees across
    let fwd0 = (DVec3::X - up * up.dot(DVec3::X)).normalize();
    let right = fwd0.cross(up).normalize();
    let fwd = (fwd0 * 15f64.to_radians().cos() - up * 15f64.to_radians().sin()).normalize();
    let camup = right.cross(fwd).normalize();
    let tanh = 35f64.to_radians().tan();
    let sun = (fwd0 * 30f64.to_radians().cos() + up * 30f64.to_radians().sin() + right * 0.3).normalize();
    let v4 = |v: DVec3, s: f64| [(v.x * s) as f32, (v.y * s) as f32, (v.z * s) as f32, 0.0];
    let cam = Cam { f: v4(fwd, 1.0), r: v4(right, tanh * w as f64 / h as f64), u: v4(camup, tanh), sun: [sun.x as f32, sun.y as f32, sun.z as f32, (0.265f64).to_radians().cos() as f32], size: [w, h, 0, mode] };
    let ubuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::bytes_of(&cam), usage: wgpu::BufferUsages::UNIFORM });
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(include_str!("trace.wgsl").into()) });
    let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: None, layout: None, module: &shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipe.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: tlas.as_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 2, resource: ubuf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: verts.as_entire_binding() },
        ],
    });
    let dispatch = |n: u32| {
        let mut enc = device.create_command_encoder(&Default::default());
        for _ in 0..n {
            let mut cp = enc.begin_compute_pass(&Default::default());
            cp.set_pipeline(&pipe);
            cp.set_bind_group(0, &bind, &[]);
            cp.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        queue.submit([enc.finish()]);
        wait();
    };
    dispatch(5);
    let t = Instant::now();
    dispatch(50);
    println!("trace 1920x1080 mode {mode}: {:.2} ms per frame", t.elapsed().as_secs_f64() * 1e3 / 50.0);

    if let Some(out) = args.get("out") {
        let bpr = (w * 4).div_ceil(256) * 256;
        let rb = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (bpr * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &rb, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: None } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([enc.finish()]);
        rb.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        wait();
        let data = rb.slice(..).get_mapped_range().unwrap();
        let mut img = image::RgbaImage::new(w, h);
        for y in 0..h {
            let row = &data[(y * bpr) as usize..(y * bpr + w * 4) as usize];
            for x in 0..w {
                let i = (x * 4) as usize;
                img.put_pixel(x, y, image::Rgba([row[i], row[i + 1], row[i + 2], 255]));
            }
        }
        img.save(out).unwrap();
    }
}
