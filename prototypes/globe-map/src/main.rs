//! The globe map's spike (README.md): the client's own globe.rs and nav.rs, run on a real bake. It times the
//! pictures and the seen-mask, and draws the globe the way overlay.wgsl will (the same steps on the CPU) to
//! PNGs to look at. Usage: globe-map <bakes dir> <seed> <out dir>
#[path = "../../../crates/client/src/globe.rs"]
#[allow(dead_code)]
mod globe;
#[path = "../../../crates/client/src/nav.rs"]
#[allow(dead_code)]
mod nav;

use glam::DVec3;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (dir, seed, out) = (&a[1], &a[2], std::path::PathBuf::from(&a[3]));
    std::fs::create_dir_all(&out).unwrap();
    let load = |n: i32, v: u32| {
        let p = format!("{dir}/seed-{seed}-planet-{n}-v{v}.map");
        engine::bake::Map::from_bytes(&std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))).unwrap()
    };
    let maps = [load(6144, engine::bake::VERSION), load(1664, engine::bake::luna::VERSION)];
    let zeros = [DVec3::from_array(maps[0].spawn), -DVec3::X];
    let mut imgs = Vec::new();
    for b in 0..2 {
        let size = globe::SIZE[b];
        let mut best = f64::MAX;
        let mut img = Vec::new();
        for _ in 0..3 {
            let t = Instant::now();
            img = globe::image(&maps[b], zeros[b], size);
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("{{\"body\":{b},\"cells\":{},\"size\":[{},{}],\"image_ms\":{best:.1},\"bytes\":{}}}", maps[b].h.len(), size.0, size.1, img.len() * 4);
        png(&out.join(format!("flat-{b}.png")), size, &img);
        imgs.push(img);
    }
    println!("{{\"buffer_bytes\":{}}}", globe::LEN * 4);
    // the seen-mask: a walk (a 2 m eye), a mountain (300 m), orbit (2000 km)
    let r = maps[0].n as f64 * 2.0 / std::f64::consts::PI;
    for over in [2.0, 300.0, 2.0e6] {
        let mut s = globe::Seen::new(globe::SIZE[0]);
        let t = Instant::now();
        let mut k = 0;
        // a walk along the equator, a mark every 8 m
        for i in 0..100 {
            k += s.mark(globe::dir(0.0, i as f64 * 8.0 / r * 57.3, zeros[0]), zeros[0], globe::horizon(r, over)) as usize;
        }
        let us = t.elapsed().as_secs_f64() * 1e6 / 100.0;
        let seen = s.bits.iter().map(|b| b.count_ones()).sum::<u32>();
        println!("{{\"over_m\":{over},\"mark_us\":{us:.0},\"new\":{k},\"texels_seen\":{seen},\"file_bytes\":{}}}", s.to_bytes().len());
    }
    // the look: the globe as the shader draws it, at noon and dusk over the spawn, zoomed out and in, the
    // unseen bare in one (a walk's band seen)
    let mut walked = globe::Seen::new(globe::SIZE[0]);
    for i in 0..400 {
        walked.mark(globe::dir(0.0, i as f64 * 0.5, zeros[0]), zeros[0], globe::horizon(r, 300.0));
    }
    let shots: [(&str, usize, f64, DVec3, bool); 5] = [
        ("noon", 0, 1.0, zeros[0], true),
        ("dusk", 0, 1.0, DVec3::from_array(maps[0].spawn).cross(DVec3::Z), true),
        ("close", 0, 8.0, zeros[0], true),
        ("walked", 0, 1.0, zeros[0], false),
        ("luna", 1, 1.0, DVec3::new(-1.0, 0.4, 0.0), true),
    ];
    for (name, b, zoom, sun, all) in shots {
        let (w, h) = (480usize, 320usize);
        let v = globe::View { cx: w as f64 / 2.0, cy: h as f64 / 2.0, r: globe::fit(h as f64) * zoom, lat: 20.0, lon: 30.0 };
        let px = 3usize;
        let mut pic = vec![0xff201010u32; w * h];
        for y in 0..h {
            for x in 0..w {
                // the pixel look: every 3 x 3 block takes its middle's colour
                let (qx, qy) = ((x / px * px) as f64 + px as f64 / 2.0, (y / px * px) as f64 + px as f64 / 2.0);
                let Some(d) = v.pick(qx, qy, zeros[b]) else { continue };
                pic[y * w + x] = shade(&imgs[b], globe::SIZE[b], d, zeros[b], sun, all.then_some(()).map_or(Some(&walked), |_| None));
            }
        }
        png(&out.join(format!("{name}.png")), (w, h), &pic);
    }
}

/// One globe pixel: the picture's texel, night's dark beyond the terminator (soft over 6 degrees), the
/// unseen bare, a line every 30 degrees. overlay.wgsl `globe_fs` does the same.
fn shade(img: &[u32], size: (usize, usize), d: DVec3, zero: DVec3, sun: DVec3, seen: Option<&globe::Seen>) -> u32 {
    let (lat, lon) = nav::geo(d, zero);
    let k = globe::texel(lat, lon, size);
    let v = img[k];
    let mut c = [v & 255, v >> 8 & 255, v >> 16 & 255].map(|c| c as f64 / 255.0);
    if seen.is_some_and(|s| !s.get(k)) {
        c = [0.32, 0.30, 0.27];
    }
    let day = (d.dot(sun.normalize()) / 0.1 + 0.5).clamp(0.0, 1.0);
    c = [c[0] * (0.25 + 0.75 * day), c[1] * (0.28 + 0.72 * day), c[2] * (0.4 + 0.6 * day)];
    let near = |a: f64| (a / 30.0 - (a / 30.0).round()).abs() * 30.0 < 0.35;
    if near(lat) || near(lon) {
        c = c.map(|v| v * 0.7 + 0.3 * 0.85);
    }
    let [r, g, b] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u32);
    r | g << 8 | b << 16 | 255 << 24
}

fn png(path: &std::path::Path, (w, h): (usize, usize), px: &[u32]) {
    let f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut e = png::Encoder::new(f, w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    let bytes: Vec<u8> = px.iter().flat_map(|p| p.to_le_bytes()).collect();
    e.write_header().unwrap().write_image_data(&bytes).unwrap();
}
