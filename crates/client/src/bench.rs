//! The benchmark runs: their camera poses and lengths, the samples and the JSON line, filmstrips and shots (ADR 0025 L: moved out of main.rs).
use super::*;

/// Camera pose for the run at time t (seconds after load).
pub(crate) struct Pose {
    pub(crate) pos: DVec3,
    pub(crate) fwd: DVec3,
    pub(crate) up: DVec3,
}

pub(crate) fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub(crate) fn pose(run: &str, t: f64, w: &World, speed: f64, alt: f64, warm: f64, q: f64, a: &Args) -> Pose {
    let planet = &w.bodies[0];
    let moon = &w.bodies[w.moon];
    // the spawn: land nearest the +y pole (ADR 0021)
    let pole = planet.spawn * (planet.height(planet.spawn) + 1.7);
    match run {
        "fly" => {
            let r0 = planet.radius + alt;
            let th = speed * t / r0;
            // at= starts the great circle there (the see-through spike's forest fly-through, ADR 0068)
            let (d0, t0) = match a.0.contains_key("at").then(|| crate::stand_at(a).ok()).flatten() {
                Some(d) => (d, d.cross(DVec3::Y).try_normalize().unwrap_or(DVec3::X)),
                None => (DVec3::Z, DVec3::X),
            };
            let dir = d0 * th.cos() + t0 * th.sin();
            let pos = dir * (planet.top(dir) + alt);
            Pose { pos, fwd: t0 * th.cos() - d0 * th.sin(), up: dir }
        }
        "orbit" => {
            // the waypoints where the bodies are now, in the inertial frame: the flight between them is there,
            // and each point is drawn in the frame it lies in (ADR 0022)
            let (pp, pm) = (w.pose(0, w.time), w.pose(1, w.time));
            let a = pp.to_inertial(planet.centre, pole);
            // high over both bodies: 3.3 planet radii, as 3.5 km was over the 1 km planet
            let high = planet.radius * 3.3;
            let b = pp.to_inertial(planet.centre, DVec3::new(0.0, high, 0.0));
            // c as high over the moon on the same side, and d landing below it: the run starts when the moon
            // stands 90 degrees from the spawn's zenith (`orbit_start`), so no leg passes through a body
            let up = pp.rot * DVec3::Y;
            let c = pm.centre + up * high;
            let mu = pm.rot.inverse() * up;
            let d = pm.centre + up * (moon.height(mu) + 1.7);
            let legs = [(a, b, 0), (b, c, 0), (c, d, 1), (d, d, 2), (d, c, 3), (c, b, 3), (b, a, 3)];
            let durs = [10.0, 10.0, 10.0, 5.0, 10.0, 10.0, 10.0].map(|d| d * q);
            let mut tt = (t - warm).max(0.0);
            let mut i = 0;
            while i < legs.len() - 1 && tt > durs[i] {
                tt -= durs[i];
                i += 1;
            }
            if t < warm {
                return Pose { pos: pole, fwd: DVec3::Z, up: DVec3::Y };
            }
            let (p0, p1, look) = legs[i];
            let x = p0.lerp(p1, smooth(tt / durs[i]));
            // up from the nearest body, and the look, in the inertial frame
            let near = if (x - pm.centre).length() - moon.radius < x.length() - planet.radius { pm } else { pp };
            let up = (x - near.centre).normalize();
            let fwd = match look {
                0 | 1 => (pm.centre - x).normalize(),
                2 => pm.rot * DVec3::Z,
                _ => (pp.centre - x).normalize(),
            };
            let fwd = if i == legs.len() - 1 && tt >= durs[i] { pp.rot * DVec3::Z } else { fwd };
            let (pos, f) = w.locate(x);
            let back = w.pose(f, w.time).rot.inverse();
            Pose { pos, fwd: back * fwd, up: back * up }
        }
        // approach / leave (step 6b's films): body= (Mars by default) from 3,000 km to 2 m over its day side and
        // back, the distance shrinking by the same share each second, looking at its centre
        "approach" | "leave" => {
            let b = a.get("body", 2usize).min(w.bodies.len() - 1);
            let p = w.pose(b, w.time);
            let sun = (w.sky.sun(w.time) - p.centre).normalize();
            let side = (sun + sun.any_orthonormal_vector() * 0.6).normalize();
            let s = smooth((t - warm).max(0.0) / (30.0 * q));
            let s = if run == "leave" { 1.0 - s } else { s };
            // (from its centre: 2 m over the ground there at the end)
            let near = w.bodies[b].top(p.rot.inverse() * side) + 2.0;
            let x = p.centre + side * near * (3.0e6 / near).powf(1.0 - s);
            let (pos, f) = w.locate(x);
            let back = w.pose(f, w.time).rot.inverse();
            Pose { pos, fwd: back * (p.centre - x).normalize(), up: back * side }
        }
        "edit" => Pose { pos: pole, fwd: DVec3::new(0.0, -1.0, 1.0).normalize(), up: DVec3::Y },
        // stand: at=x,y,z picks the spot (a direction from the centre of body= 0 Terra or 1 Luna), yaw and pitch
        // turn the head; look=terra or look=luna looks at that body's centre instead (ADR 0070)
        _ => {
            // (a bad at= ends the run with its reason before this: `stand_at`)
            let up = crate::stand_at(a).unwrap_or(DVec3::Y);
            let i = a.get("body", 0usize).min(w.bodies.len() - 1);
            let b = &w.bodies[i];
            let r = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
            let ahead = (r - up * r.dot(up)).normalize();
            let (yaw, pitch) = (a.get("yaw", 0f64).to_radians(), a.get("pitch", 0f64).to_radians());
            // depth=m (or floor=k): stand that far under the ground (place= digs the room, ADR 0152)
            let pos = b.centre + up * (b.top(up) - crate::stand_depth(a, b, up) + a.get("eye", 1.7));
            let fwd = match a.s("look", "").as_str() {
                // the other body's centre, now, in this body's frame
                l if engine::bodies::find(l).is_some_and(|j| j < w.bodies.len()) => {
                    let (me, it) = (w.pose(i, w.time), w.pose(engine::bodies::find(l).unwrap(), w.time));
                    (b.centre + me.rot.inverse() * (it.centre - me.centre) - pos).normalize()
                }
                _ => (ahead * yaw.cos() + up.cross(ahead) * yaw.sin()) * pitch.cos() + up * pitch.sin(),
            };
            // an eye past the body's sphere of influence (eye= far out over a small moon) is in another frame: put it
            // in the frame it lies in, else its grid coordinates are read as that frame's
            let (_, f) = w.locate(w.pose(i, w.time).to_inertial(b.centre, pos));
            if f != i {
                let (rot, off) = w.place(i, f);
                return Pose { pos: rot * (pos - b.centre) + off, fwd: rot * fwd, up: rot * up };
            }
            Pose { pos, fwd, up }
        }
    }
}

/// The yaw and pitch (degrees) of run=stand that look along `fwd` from a spot whose up is `up`.
pub(crate) fn stand_angles(up: DVec3, fwd: DVec3) -> (f64, f64) {
    let r = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
    let ahead = (r - up * r.dot(up)).normalize();
    let pitch = fwd.dot(up).clamp(-1.0, 1.0).asin();
    (fwd.dot(up.cross(ahead)).atan2(fwd.dot(ahead)).to_degrees(), pitch.to_degrees())
}

/// The world time at which the orbit run starts: the first moment the moon stands 90 degrees from the spawn's
/// zenith, as the route was laid out before the bodies moved (ADR 0022).
pub(crate) fn orbit_start(w: &World) -> f64 {
    let side = |t: f64| (w.pose(0, t).rot * DVec3::Y).dot(w.pose(1, t).centre - w.pose(0, t).centre);
    let mut t = 0.0;
    while t < engine::sky::DAY && side(t).signum() == side(t + 0.5).signum() {
        t += 0.5;
    }
    t
}

pub(crate) static LOW_WORKERS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether a run keeps every frame's numbers for its JSON line: an open-ended play session (no `seconds=`)
/// keeps none, or they grow every frame for as long as it is played; a play run with `seconds=` (the gate's
/// feel report) and the benchmark runs do.
pub(crate) fn keeps_frames(run: &str, a: &Args) -> bool {
    run != "play" || a.0.contains_key("seconds")
}

pub(crate) fn run_len(run: &str, warm: f64, q: f64) -> f64 {
    warm + q * match run {
        "fly" => 60.0,
        "orbit" => 65.0,
        _ => 30.0,
    }
}

#[derive(Default)]
pub(crate) struct Samples {
    pub(crate) frame_ms: Vec<f32>,
    pub(crate) cpu_ms: Vec<f32>,
    pub(crate) gpu_ms: Vec<f32>,
    pub(crate) draws: u64,
    pub(crate) quads: u64,
    pub(crate) frames: u64,
    pub(crate) missing_max: u32,
    pub(crate) holes_max: u32,
    /// The longest a visible chunk was drawn coarser than its level while the camera was slow, s (ADR 0073).
    pub(crate) coarse_slow_s: f64,
    pub(crate) applied: u64,
    pub(crate) applied_nonempty: u64,
    pub(crate) ram_max: f64,
    pub(crate) vram_max: f64,
    pub(crate) edits: u64,
    pub(crate) sel_ms: Vec<f32>,
    pub(crate) acq_ms: Vec<f32>,
    pub(crate) submit_ms: Vec<f32>,
    /// Per stage of `CPU_STAGES` and `GPU_STAGES`, every measured frame.
    pub(crate) cpu_st: Vec<Vec<f32>>,
    pub(crate) gpu_st: Vec<Vec<f32>>,
    /// A frame over budget: its frame number, the CPU part, and the GPU stages once they are read back.
    pub(crate) spikes: Vec<(u64, String, Option<String>)>,
    pub(crate) capture_stalls: u32,
    /// Particles drawn, summed over the measured frames (ADR 0031).
    pub(crate) fx_live: u64,
}

pub(crate) fn pct(v: &mut Vec<f32>, p: f64) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

impl App {
    /// The frame copied to the capture buffer, at full size.
    pub(crate) fn read_frame(&self) -> image::RgbaImage {
        let gpu = self.gpu.as_ref().unwrap();
        let buf = gpu.capture.as_ref().unwrap();
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().unwrap();
        let bgra = matches!(gpu.config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut img = image::RgbaImage::new(w(), h());
        let row = row_bytes() as usize;
        for (x, y, px) in img.enumerate_pixels_mut() {
            let i = y as usize * row + x as usize * 4;
            let d = &data[i..i + 4];
            *px = image::Rgba(if bgra { [d[2], d[1], d[0], 255] } else { [d[0], d[1], d[2], 255] });
        }
        drop(data);
        buf.unmap();
        img
    }

    pub(crate) fn capture_frame(&mut self, t: f64) {
        let every: f64 = self.args.get("capture_ms", 0.0) / 1000.0;
        self.next_cap = t + every;
        let full = self.read_frame();
        let tw: u32 = self.args.get("capture_w", 384);
        let th = tw * h() / w();
        let img = image::RgbaImage::from_fn(tw, th, |x, y| *full.get_pixel(x * w() / tw, y * h() / th));
        self.sheet.push(img);
    }

    /// shot= (ADR 0018): counts cracks, the clear colour where a ray from the camera must meet a body (it
    /// passes inside the lowest terrain any column can have), then stores the frame at a third of its size
    /// and counts the pixels that differ from baseline= by more than 8 of 255 in a channel.
    pub(crate) fn take_shot(&mut self) {
        let img = self.read_frame();
        let (pos, fwd, up) = self.cam;
        let right = fwd.cross(up).normalize();
        let cu = right.cross(fwd);
        let th = (self.fov().to_radians() / 2.0).tan();
        let aspect = w() as f64 / h() as f64;
        let mut cracks = 0u64;
        let mut at: Vec<String> = Vec::new();
        for (x, y, px) in img.enumerate_pixels() {
            if px.0[..3] != [255, 0, 255] {
                continue;
            }
            let nx = (2.0 * (x as f64 + 0.5) / w() as f64 - 1.0) * th * aspect;
            let ny = (1.0 - 2.0 * (y as f64 + 0.5) / h() as f64) * th;
            let d = (fwd + right * nx + cu * ny).normalize();
            let hits = |c: DVec3, r: f64| {
                let oc = pos - c;
                let (b, cc) = (oc.dot(d), oc.dot(oc) - r * r);
                b * b - cc >= 0.0 && -b + (b * b - cc).sqrt() > 0.0
            };
            // (under the lowest ground: the old AMP of 140, not ADR 0105's, which bounds the peaks upward)
            if self.world.bodies.iter().any(|b| hits(b.centre, b.radius - 141.0)) {
                cracks += 1;
                if at.len() < 8 {
                    at.push(format!("[{x},{y}]"));
                    // where the ray meets the terrain the generator makes, to find the hole in the world
                    let mut t = 0.0;
                    while t < 5000.0 {
                        let q = pos + d * t;
                        let b = &self.world.bodies[self.world.nearest(q)];
                        let r = (q - b.centre).length();
                        if r < b.height((q - b.centre) / r) {
                            eprintln!("[technoape] crack [{x},{y}] meets the ground {t:.2} m away at {:?}: {:?}", q.to_array(), self.world.block_at(q));
                            break;
                        }
                        t += 0.02;
                    }
                }
            }
        }
        let small = image::imageops::resize(&img, w() / 3, h() / 3, image::imageops::FilterType::Triangle);
        let base = self.args.0.get("baseline").and_then(|b| image::open(b).ok()).map(|i| i.to_rgba8());
        let diff = base.filter(|b| b.dimensions() == small.dimensions()).map(|b| {
            let n = small.pixels().zip(b.pixels()).filter(|(p, q)| p.0.iter().zip(q.0.iter()).any(|(u, v)| u.abs_diff(*v) > 8)).count();
            n as f64 / (small.width() * small.height()) as f64 * 100.0
        });
        let path = self.shot.clone().unwrap();
        if let Some(d) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Err(e) = small.save(&path) {
            eprintln!("[technoape] shot {path}: {e}");
        }
        self.shot_json = format!("\"shot\":{{\"file\":{path:?},\"cracks\":{cracks},\"crack_px\":[{}],\"diff_pct\":{}}},", at.join(","), diff.map_or("null".into(), |d| format!("{d:.3}")));
    }

    /// run=play's feel report (fundamentals section 1, ADR 0018) as a JSON field, or nothing in other runs.
    pub(crate) fn feel(&self) -> String {
        if self.run != "play" {
            return String::new();
        }
        let mut mags: Vec<f32> = self.jerks.iter().map(|j| j.1 as f32).collect();
        let mut top = self.jerks.clone();
        top.sort_by(|a, b| b.1.total_cmp(&a.1));
        let top: Vec<String> = top.iter().take(5).map(|(t, j)| format!("{{\"t\":{t:.2},\"jerk\":{j:.0}}}")).collect();
        let frames = self.shown.iter().map(|c| c.0).max().unwrap_or(0);
        let ms = self.shown.iter().map(|c| c.1).fold(0.0, f64::max);
        format!(
            "\"feel\":{{\"corrections\":{},\"corrections_at\":[{}],\"drift_max_m\":{:.3},\"jerk_p50\":{:.0},\"jerk_p99\":{:.0},\"jerk_max\":{:.0},\"jerk_top\":[{}],\"clicks\":{},\"clicks_not_shown\":{},\"click_to_change_frames_max\":{},\"click_to_change_ms_max\":{:.2}}},",
            self.snaps,
            self.snap_t.iter().map(|(t, d)| format!("{{\"t\":{t:.2},\"m\":{d:.2}}}")).collect::<Vec<_>>().join(","),
            self.drift_max,
            pct(&mut mags, 0.5),
            pct(&mut mags, 0.99),
            pct(&mut mags, 1.0),
            top.join(","),
            self.shown.len(),
            self.clicks.len(),
            frames,
            ms
        )
    }

    pub(crate) fn finish(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        self.globe_save();
        let gpu = self.gpu.as_ref().unwrap();
        let s = &mut self.samples;
        let secs: f64 = s.frame_ms.iter().map(|v| *v as f64).sum::<f64>() / 1000.0;
        let o = &self.streamer.opts;
        let a = &self.args;
        let job_us = self.streamer.job_us_total / self.streamer.jobs_done.max(1) as f64;
        let later = (self.streamer.jobs_done as u64).saturating_sub(self.jobs_at_load.1);
        let job_us_after = (self.streamer.job_us_total - self.jobs_at_load.0) / later.max(1) as f64;
        let (p99, max) = (pct(&mut s.frame_ms, 0.99), pct(&mut s.frame_ms, 1.0));
        // ADR 0004; an unknown RAM or VRAM reading fails
        let budget_ok = p99 <= 8.3 && max <= 16.7 && s.ram_max > 0.0 && s.ram_max <= 8192.0 && s.vram_max > 0.0 && s.vram_max <= 6144.0;
        let line = format!(
            concat!(
                "{{\"run\":\"{}\",\"label\":\"{}\",\"quick\":{},\"planet_n\":{},\"planet_radius_m\":{:.0},\"field\":{:.4},\"budget_ok\":{},",
                "\"fps_avg\":{:.1},\"frame_p50_ms\":{:.3},\"frame_p99_ms\":{:.3},\"frame_max_ms\":{:.3},",
                "\"cpu_ms_p50\":{:.3},\"cpu_ms_p99\":{:.3},\"gpu_ms_p50\":{:.3},\"gpu_ms_p99\":{:.3},",
                "\"ram_mb\":{:.0},\"vram_mb\":{:.0},\"chunks_meshed_per_s\":{:.1},\"chunks_nonempty_per_s\":{:.1},",
                "\"missing_chunks_max\":{},\"holes_max\":{},\"draws_avg\":{:.0},\"quads_avg\":{:.0},\"quad_buffer_mb\":{:.1},",
                "\"select_ms_p50\":{:.3},\"submit_ms_p50\":{:.3},\"pre_acquire_ms_p50\":{:.3},\"edits_per_s\":{:.0},\"job_us_avg\":{:.0},",
                "\"felt\":{},\"look\":{},",
                "\"edits_sent\":{},\"edits_applied\":{},\"players\":{},\"feet\":[{:.2},{:.2},{:.2}],\"snaps\":{},\"drift_max_m\":{:.2},\"frames\":{},\"capture_stalls\":{},\"speed\":{},\"k\":{},\"greedy\":{},\"horizon\":{},\"mdi\":{},\"prefetch_s\":{},",
                "\"coarse_slow_s\":{:.2},\"threads\":{},\"low_workers\":{},\"alloc_fail\":{},\"backend\":\"{}\",\"adapter\":\"{}\",\"spikes\":[{}]}}"
            ),
            self.run,
            a.s("label", ""),
            a.get("quick", 0),
            self.world.bodies[0].n,
            self.world.bodies[0].radius,
            self.world.field(0) * crate::frame::field_scale(a),
            budget_ok,
            s.frames as f64 / secs.max(1e-9),
            pct(&mut s.frame_ms, 0.5),
            p99,
            max,
            pct(&mut s.cpu_ms, 0.5),
            pct(&mut s.cpu_ms, 0.99),
            pct(&mut s.gpu_ms, 0.5),
            pct(&mut s.gpu_ms, 0.99),
            s.ram_max,
            s.vram_max,
            s.applied as f64 / secs.max(1e-9),
            s.applied_nonempty as f64 / secs.max(1e-9),
            s.missing_max,
            s.holes_max,
            s.draws as f64 / s.frames.max(1) as f64,
            s.quads as f64 / s.frames.max(1) as f64,
            self.streamer.alloc.used as f64 * 8.0 / 1e6,
            pct(&mut s.sel_ms, 0.5),
            pct(&mut s.submit_ms, 0.5),
            pct(&mut s.acq_ms, 0.5),
            s.edits as f64 / secs.max(1e-9),
            job_us,
            // the felt temperature at the eye and the look-at panel's lines when the run ended (`App::sense`)
            self.sensed.as_ref().map_or("null".into(), |s| format!("{{\"temp\":{:.2},\"air\":{:.2},\"radiant\":{:.2}}}", s.1.temp, s.1.air, s.1.radiant)),
            self.sensed.as_ref().and_then(|s| s.2.as_ref()).map_or("null".into(), |l| format!("{{\"name\":{:?},\"note\":{:?}}}", l.name, l.note)),
            self.edits_done,
            self.edits_applied,
            self.players,
            self.me.pos.x,
            self.me.pos.y,
            self.me.pos.z,
            self.snaps,
            self.drift_max,
            s.frames,
            s.capture_stalls,
            a.get("speed", 60.0),
            o.k,
            o.greedy as u8,
            o.horizon as u8,
            a.get("mdi", 1),
            o.prefetch_s,
            s.coarse_slow_s,
            rayon::current_num_threads(),
            LOW_WORKERS.load(std::sync::atomic::Ordering::Relaxed),
            self.streamer.alloc_fail,
            gpu.backend,
            gpu.adapter_name,
            s.spikes.iter().map(|(_, c, g)| format!("{{{c},\"gpu\":{{{}}}}}", g.as_deref().unwrap_or(""))).collect::<Vec<_>>().join(",")
        );
        // ADR 0025: every stage's p50 and p99, the frame thread's and the GPU's
        let mut stages = String::new();
        for (key, names, v) in [("cpu", &CPU_STAGES[..], &mut s.cpu_st), ("gpu", &GPU_STAGES[..], &mut s.gpu_st)] {
            v.resize(names.len(), Vec::new());
            for p in [0.5, 0.99] {
                let f: Vec<String> = names.iter().zip(v.iter_mut()).map(|(n, x)| format!("\"{n}\":{:.3}", pct(x, p))).collect();
                stages += &format!("\"{key}_p{}\":{{{}}},", (p * 100.0) as u32, f.join(","));
            }
        }
        // the shot's and the feel report's fields go in before the spikes
        // what wgpu itself holds at the end, finer than the driver's 256 MB blocks in vram_mb (ADR 0025, G)
        if let Some(r) = gpu.device.generate_allocator_report() {
            stages += &format!("\"gpu_alloc_mb\":{:.0},\"gpu_reserved_mb\":{:.0},", r.total_allocated_bytes as f64 / 1e6, r.total_reserved_bytes as f64 / 1e6);
        }
        stages += &format!("\"fx_live_avg\":{:.0},", s.fx_live as f64 / s.frames.max(1) as f64);
        stages += &format!("\"job_us_after_load\":{job_us_after:.0},\"jobs_after_load\":{later},\"walks\":{},\"tlas_builds\":{},\"rt_dead_peak\":{},", self.streamer.walks, self.rt.as_ref().map_or(0, |r| r.tlas_builds), self.rt.as_ref().map_or(0, |r| r.dead_peak()));
        // ADR 0025, S: the server's ticks, the workers' jobs and what each holds of the cores over the measured time
        // play drains the ticks for F3, so its line carries no tick numbers
        if let Some(v) = self.tick_log.as_ref().filter(|_| self.run != "play") {
            let mut v = v.lock().unwrap().clone();
            let busy = v.iter().map(|x| *x as f64).sum::<f64>() / 1000.0;
            stages += &format!(
                "\"ticks\":{},\"tick_ms_p50\":{:.3},\"tick_ms_p99\":{:.3},\"tick_ms_max\":{:.3},\"tick_cores\":{:.3},",
                v.len(),
                pct(&mut v, 0.5),
                pct(&mut v, 0.99),
                pct(&mut v, 1.0),
                busy / secs.max(1e-9)
            );
        }
        if self.run == "play" {
            // ADR 0040: health and hunger at the end, the lowest health, and whether the player is dead
            stages += &format!(
                "\"vitals\":{{\"creative\":{},\"health\":{},\"hunger\":{},\"thirst\":{},\"breath\":{},\"health_min\":{},\"dead\":{}}},",
                self.creative,
                self.vitals.0,
                self.vitals.1,
                self.vitals.2,
                self.breath,
                self.health_min,
                self.dead.as_ref().map_or("null".into(), |c| format!("{c:?}"))
            );
        }
        let jobs = std::mem::take(&mut self.streamer.job_times);
        let (mut wait, work): (Vec<f32>, Vec<f32>) = jobs.iter().map(|&(w, u)| (w / 1000.0, u)).unzip();
        let frame_busy = s.cpu_ms.iter().map(|x| *x as f64).sum::<f64>() / 1000.0;
        stages += &format!(
            "\"jobs_measured\":{},\"job_wait_ms_p50\":{:.3},\"job_wait_ms_p99\":{:.3},\"job_wait_ms_max\":{:.3},\"worker_cores\":{:.3},\"frame_cores\":{:.3},",
            jobs.len(),
            pct(&mut wait, 0.5),
            pct(&mut wait, 0.99),
            pct(&mut wait, 1.0),
            work.iter().map(|x| *x as f64).sum::<f64>() / 1e6 / secs.max(1e-9),
            frame_busy / secs.max(1e-9)
        );
        stages += &self.audio.json();
        let line = line.replacen("\"spikes\":", &format!("{}{}{}\"spikes\":", self.shot_json, self.feel(), stages), 1);
        println!("{}", line);
        if let Some(path) = a.0.get("out") {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
            writeln!(f, "{}", line).unwrap();
        }
        if !self.sheet.is_empty() {
            let cols = 6.min(self.sheet.len() as u32);
            let rows = (self.sheet.len() as u32 + cols - 1) / cols;
            let (tw, th) = (self.sheet[0].width(), self.sheet[0].height());
            let mut big = image::RgbaImage::new(tw * cols, th * rows);
            for (i, im) in self.sheet.iter().enumerate() {
                image::imageops::replace(&mut big, im, ((i as u32 % cols) * tw) as i64, ((i as u32 / cols) * th) as i64);
            }
            let path = a.s("sheet", &format!("{}_strip.png", self.run));
            big.save(&path).unwrap();
            eprintln!("[technoape] filmstrip {} ({} frames)", path, self.sheet.len());
        }
    }
}

/// Frames a shot waits after loading before it is taken (`shot=`, ADR 0018).
pub(crate) const SHOT_FRAMES: u64 = 400;
/// Whether a shot `frames` after loading has its light settled.
pub(crate) fn shot_settled(frames: u64) -> bool {
    frames >= SHOT_FRAMES
}

/// The sky's clock off the server, `t` seconds into the run. A view and a shot hold it at `time=`: a shot is
/// taken when loading and its frames are done, and a sun moved by those seconds made the `lod` view differ
/// from run to run (ADR 0073).
pub(crate) fn sky_time(run: &str, shot: bool, time0: f64, t: f64) -> f64 {
    if run == "view" || shot { time0 } else { time0 + t }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_shows_the_sky_at_its_time_however_long_loading_took() {
        assert_eq!(sky_time("stand", true, 225.0, 1.5), sky_time("stand", true, 225.0, 6.0), "a shot on a busy PC is the quiet PC's");
        assert_eq!(sky_time("stand", true, 225.0, 6.0), 225.0);
        assert_eq!(sky_time("stand", false, 225.0, 6.0), 231.0, "a benchmark's sky runs");
    }

    /// A stand's eye far over a small moon, past its sphere of influence (eye=40000 over the giant's icy moon), is
    /// put in the frame it lies in: before, its grid coordinates were read as the inertial frame's (10^10 m off,
    /// the giant and the moon gone from the sky).
    #[test]
    fn a_stand_eye_past_its_bodys_sphere_is_in_the_frame_it_lies_in() {
        let w = World::with_grid(1536, engine::bodies::moon_columns(1536), 1, None, None, 0.0, 7);
        let i = engine::bodies::find("callisto").unwrap();
        let args = |eye: &str| Args([("body", "4"), ("at", "1,0.15,0"), ("eye", eye)].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        let b = &w.bodies[i];
        for eye in ["2", "40000", "150000"] {
            let p = pose("stand", 0.0, &w, 0.0, 0.0, 0.0, 1.0, &args(eye));
            let up = crate::stand_at(&args(eye)).unwrap();
            let want = w.pose(i, w.time).to_inertial(b.centre, b.centre + up * (b.top(up) + eye.parse::<f64>().unwrap()));
            assert!((w.to_inertial(p.pos) - want).length() < 0.01,"eye {eye}: {:?} against {want:?}", w.to_inertial(p.pos));
        }
    }

    #[test]
    fn an_open_ended_play_session_keeps_no_frame_numbers() {
        let args = |kv: &[(&str, &str)]| Args(kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        assert!(!keeps_frames("play", &args(&[])), "a session played until it is closed");
        assert!(keeps_frames("play", &args(&[("seconds", "20")])), "the gate's feel report");
        assert!(keeps_frames("stand", &args(&[])), "a benchmark run");
    }
}
