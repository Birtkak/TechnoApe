//! run=view: the game's renderer for the Studio (ADR 0049). An off-screen stand run that never ends on its own: it
//! takes one command a line on stdin and answers `frame` with the drawn frame on stdout. Commands:
//! `cam ex ey ez tx ty tz` (the eye and the point looked at in the stand cell's frame made square,
//! `models::view_axes`: x, y up, z in metres, the floor's middle at 0.5 0 0.5, as a model's metres are), `time s` (the sky's clock), `model name [clip] t [x<scale>]`
//! (`model -` shows none; `x2` draws it twice as large, about its cell's floor centre), `models dir` and `textures dir` (read on the reading thread, drawn at once), `place di:dj:dl:block,...`
//! (`_` air, around the stand cell), `reset` (every placed cell back as it was), `size w h`, `frame id`,
//! `effects file` (an effects file read on the reading thread, its rows played from then on; ADR 0050) and
//! `zoom k` (the field of view k times narrower, the Studio's flat view), `effect name` (that row or group lasting on the stand cell's floor, from its start; `effect -` none). Effects
//! play when the run has `fx=1`.
//! `season north|equator|south` shows the seasons as at that latitude (`season -` by the place's own).
//! `bare 1` leaves the land, the sea and the sky out of the picture (a plain grey instead; the light stays the
//! place's and the time's), `bare 0` draws them again.
//! `state name` puts the shown model in that state (its clip, effects at points, sound, light; ADR 0050); `state -` none.
//! A frame is a line `frame id w h fov` then w * h * 4 bytes of RGBA. Stdin closing ends the run.
use super::*;
use std::io::{BufRead, Write};
use std::sync::mpsc::{Receiver, Sender};

enum Cmd {
    Line(String),
    Quit,
}

pub(crate) struct ViewRun {
    rx: Receiver<Cmd>,
    /// The eye and the point looked at, in the stand cell's frame.
    eye: DVec3,
    target: DVec3,
    /// The model's clip and the time into it.
    pub(crate) t: f64,
    want: Option<u64>,
    /// The air cell at the stand point's feet: the model's cell, the origin of `place`.
    feet: Block,
    /// Cells `place` changed, with what they were.
    orig: HashMap<IVec3, u16>,
    last: Instant,
    /// Effects files read on the reading thread, and the effect asked for (and whether it plays).
    sets: Receiver<Result<fx::Set, String>>,
    effect: Option<String>,
    effect_on: bool,
    /// `bare 1`: no land and no sky are drawn, only the model and the effects on a plain ground colour.
    pub(crate) bare: bool,
    /// `season north|equator|south`: the seasons as at that latitude everywhere (the Studio's year slider shows
    /// them whole); `season -` (the default) by the place's own.
    pub(crate) lat: Option<u8>,
}

impl ViewRun {
    fn start(models: &mut models::Models, textures: Sender<engine::textures::Set>, feet: Block, effects: std::path::PathBuf) -> ViewRun {
        let (tx, rx) = std::sync::mpsc::channel();
        let feed = models.feed();
        let (sets_tx, sets) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("view stdin".into())
            .spawn(move || {
                // the effects file the models' states are checked against: `effects=`, then each `effects` line's
                let mut effects = effects;
                let mut kept = fx::Kept::new();
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else { break };
                    let line = line.trim().to_string();
                    // the files are read here, never on the frame thread
                    if let Some(d) = line.strip_prefix("models ") {
                        let _ = feed.send(models::load(std::path::Path::new(d), &effects));
                    } else if let Some(f) = line.strip_prefix("effects ") {
                        effects = f.into();
                        let _ = sets_tx.send(read_set(f, &mut kept));
                    } else if let Some(d) = line.strip_prefix("textures ") {
                        let _ = textures.send(engine::textures::load(std::path::Path::new(d), &BLOCKS));
                    } else if tx.send(Cmd::Line(line)).is_err() {
                        return;
                    }
                }
                let _ = tx.send(Cmd::Quit);
            })
            .expect("a thread for stdin");
        ViewRun { rx, eye: DVec3::new(2.0, 1.5, 2.5), target: DVec3::splat(0.5), t: 0.0, want: None, feet, orig: HashMap::new(), last: Instant::now(), sets, effect: None, effect_on: false, bare: false, lat: None }
    }

    /// The camera: the eye looking at the target, up the cell's up.
    pub(crate) fn pose(&self, w: &World) -> Pose {
        cam_pose(w, self.feet, self.eye, self.target)
    }

    pub(crate) fn wants(&self) -> bool {
        self.want.is_some()
    }
}

impl App {
    /// The year's phase the first body's looks take (`BodyGpu::_p[1]`, shader.wgsl `season`) when it is `year`:
    /// by each place's latitude; the Studio's `season` latitude makes the seasons whole everywhere (8 and on,
    /// the phase at full strength), the north's or half a year on the south's, or none on the equator.
    pub(crate) fn season_year(&self, year: f64) -> f64 {
        match self.view.as_ref().and_then(|v| v.lat) {
            None => year,
            Some(1) => -1.0,
            Some(k) => 8.0 + (year + k as f64).rem_euclid(4.0),
        }
    }
}

impl App {
    /// The Studio's commands since the last frame; at most 60 frames a second, so an idle Studio leaves the GPU
    /// alone.
    pub(crate) fn view_take(&mut self) {
        if self.view.is_none() {
            let Some(gpu) = self.gpu.as_mut() else { return };
            let (ttx, trx) = std::sync::mpsc::channel();
            gpu.tex_rx = trx;
            let up = stand_at(&self.args).unwrap_or(DVec3::Y);
            let feet = self.world.air_over_ground(0, up);
            self.view = Some(ViewRun::start(&mut self.models, ttx, feet, self.args.s("effects", "data/effects.toml").into()));
        }
        let v = self.view.as_mut().unwrap();
        let wait = Duration::from_millis(16).saturating_sub(v.last.elapsed());
        std::thread::sleep(wait);
        v.last = Instant::now();
        let mut places: Vec<(Block, Option<u16>)> = Vec::new();
        // a new effects file: what plays keeps playing with its new numbers; one asked for before it came starts now
        for s in v.sets.try_iter() {
            match s.and_then(|s| self.fx.swap(s)) {
                Ok(()) if !v.effect_on && v.effect.is_some() => v.effect_on = self.fx.show(v.effect.as_deref(), &self.world, v.feet).is_some(),
                Ok(()) => {}
                Err(e) => eprintln!("[technoape] view: effects: {e}"),
            }
        }
        for c in v.rx.try_iter() {
            let line = match c {
                Cmd::Quit => {
                    self.done = true;
                    return;
                }
                Cmd::Line(l) => l,
            };
            let f: Vec<&str> = line.split_whitespace().collect();
            let num = |i: usize| f.get(i).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            match f.first().copied() {
                Some("cam") => (v.eye, v.target) = (DVec3::new(num(1), num(2), num(3)), DVec3::new(num(4), num(5), num(6))),
                Some("time") => self.time0 = num(1),
                // `zoom k`: the field of view k times narrower (the Studio's flat view: its eye k times as far)
                Some("zoom") => self.zoom = num(1).clamp(1.0, 64.0),
                Some("bare") => v.bare = num(1) != 0.0,
                Some("season") => v.lat = ["north", "equator", "south"].iter().position(|n| f.get(1) == Some(n)).map(|k| k as u8),
                Some("frame") => v.want = Some(num(1) as u64),
                Some("model") => {
                    // a last word `@x,z,yaw`: it stands that far from its cell (metres), turned (radians); the walk in a circle
                    let place: Option<Vec<f32>> = f.last().and_then(|w| w.strip_prefix('@')).and_then(|s| s.split(',').map(|n| n.parse().ok().filter(|v: &f32| v.is_finite())).collect()).filter(|p: &Vec<f32>| p.len() == 3);
                    let f = &f[..f.len() - place.is_some() as usize];
                    // a last word `x<scale>`: drawn at that scale (a creature's, ADR 0053)
                    let scale = f.last().and_then(|w| w.strip_prefix('x')).and_then(|s| s.parse::<f32>().ok()).filter(|s| s.is_finite() && *s > 0.0);
                    let f = &f[..f.len() - scale.is_some() as usize];
                    match f.get(1) {
                        Some(&"-") | None => self.models.show(None, v.feet),
                        Some(n) => {
                            let clip = (f.len() > 3).then(|| f[2].to_string());
                            v.t = f[f.len() - 1].parse().unwrap_or(0.0);
                            self.models.show(Some((n.to_string(), clip)), v.feet);
                            if let Some(s) = scale {
                                self.models.show_scale(s);
                            }
                            if let Some(p) = place {
                                self.models.show_place([p[0], p[1]], p[2]);
                            }
                        }
                    }
                }
                Some("place") => {
                    for spec in f.get(1).unwrap_or(&"").split(',').filter(|s| !s.is_empty()) {
                        match place_spec(spec) {
                            Some((d, id)) => places.push((Block { pos: v.feet.pos + d, ..v.feet }, Some(id))),
                            _ => eprintln!("[technoape] view: place {spec}: no such cell or block"),
                        }
                    }
                }
                Some("effect") => {
                    v.effect = f.get(1).filter(|n| **n != "-").map(|n| n.to_string());
                    v.effect_on = self.fx.show(v.effect.as_deref(), &self.world, v.feet).is_some();
                }
                // `state name`: the shown model in that state (its clip, effects at points, sound, light); `state -` none
                Some("state") => self.models.show_state(f.get(1).filter(|n| **n != "-").copied()),
                // `fluff id size push droop long dense light`: a cutout block's fluff drawn so for now (the Studio's
                // sliders as they are dragged, ADR 0135); the saved row, reloaded, puts its own back
                Some("fluff") => {
                    let id = f.get(1).and_then(|s| s.parse::<u16>().ok()).filter(|id| engine::blocks::BLOCKS.get(*id).is_some_and(|d| d.fluff.is_some()));
                    // (all six numbers, finite, in the Studio sliders' ranges: light goes to the shadow ray as it is)
                    let n: Vec<f32> = f.iter().skip(2).filter_map(|s| s.parse::<f32>().ok()).filter(|v| v.is_finite()).collect();
                    let fl = (f.len() == 8 && n.len() == 6).then(|| engine::blocks::Fluff { size: n[0].clamp(0.0, 3.0), push: n[1].clamp(0.0, 1.2), droop: n[2].clamp(0.0, 1.2), long: n[3].clamp(1.0, 4.0), dense: n[4].clamp(0.0, 1.0), light: n[5].clamp(0.0, 1.0) });
                    match (id, fl, &self.gpu) {
                        (Some(id), Some(fl), Some(g)) => g.set_fluff(id, fl),
                        (None, ..) => eprintln!("[technoape] view: {line:?}: no cutout block of that id"),
                        (_, None, _) => eprintln!("[technoape] view: {line:?}: wants six numbers: size push droop long dense light"),
                        _ => eprintln!("[technoape] view: {line:?}: no GPU to draw it on"),
                    }
                }
                Some("reset") => places.extend(v.orig.keys().map(|p| (Block { pos: *p, ..v.feet }, None))),
                Some("size") => {
                    if let Some(g) = &self.gpu {
                        let [w, h] = view_size(num(1), num(2), g.device.limits().max_texture_dimension_2d);
                        let _ = g.window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
                    }
                }
                _ => eprintln!("[technoape] view: unknown command {line:?}"),
            }
        }
        for (b, id) in places {
            let was = self.world.block(b);
            let id = match id {
                Some(id) => {
                    v.orig.entry(b.pos).or_insert(was);
                    id
                }
                None => v.orig.remove(&b.pos).unwrap_or(was),
            };
            if id != was {
                self.world.set_block(b, id);
                self.streamer.touch_block(&self.world, b);
            }
        }
    }

    /// The frame just presented to the Studio, if it asked for one.
    pub(crate) fn view_send(&mut self) {
        let img = self.read_frame();
        let fov = self.fov();
        let v = self.view.as_mut().unwrap();
        let id = v.want.take().unwrap_or(0);
        let out = std::io::stdout();
        let mut o = out.lock();
        let ok = writeln!(o, "frame {id} {} {} {fov:.4}", img.width(), img.height()).and_then(|_| o.write_all(img.as_raw())).and_then(|_| o.flush());
        if ok.is_err() {
            self.done = true;
        }
    }
}

/// `di:dj:dl:block` (`_` for a space, none for air): the offset from the stand cell, within `PLACE_REACH`, and
/// the block.
fn place_spec(spec: &str) -> Option<(IVec3, u16)> {
    let p: Vec<&str> = spec.split(':').collect();
    let d = (p.len() == 4).then(|| Some(IVec3::new(p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?))).flatten().filter(|d| d.to_array().iter().all(|v| (-PLACE_REACH..=PLACE_REACH).contains(v)))?;
    let name = p[3].replace('_', " ");
    let id = if name.trim().is_empty() { Some(AIR) } else { (0..engine::blocks::IDS as u16).filter_map(|i| BLOCKS.get(i)).find(|b| b.name == name).map(|b| b.id) }?;
    Some((d, id))
}

/// The view's camera: `eye` looking at `target`, both in the shown model's frame (square metres, as the Studio's
/// overlay counts them, `models::view_axes`), up its up.
fn cam_pose(w: &World, feet: Block, eye: DVec3, target: DVec3) -> Pose {
    let [o, x, y, z] = models::view_axes(w, feet);
    let at = |p: DVec3| o + x * p.x + y * p.y + z * p.z;
    let (e, t) = (at(eye), at(target));
    Pose { pos: e, fwd: (t - e).normalize_or(x), up: y.normalize() }
}

/// An `effects` line's file read and checked, its map rows' fields worked out as the game's watcher does
/// (`fx::load`; on the stdin thread, never the frame thread). `kept`: the fields worked out before, so a map
/// unchanged is not worked out again; a map changed reaches the view at the next `effects` line.
fn read_set(f: &str, kept: &mut fx::Kept) -> Result<fx::Set, String> {
    let pal = std::fs::read_to_string("data/palette.toml").ok().and_then(|t| engine::palette::Palette::parse(&t).ok()).unwrap_or_else(|| fx::PALETTE.clone());
    fx::load(std::path::Path::new(f), fx::maps_dir(), &pal, kept)
}

/// `size w h` as a window the surface can take: 64 pixels a side at least, `max` at most.
fn view_size(w: f64, h: f64, max: u32) -> [u32; 2] {
    [w, h].map(|v| v.max(64.0).min(max as f64) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Studio's preview plays a map row as the game does: an `effects` line's file is read with its rows'
    /// fields worked out (the game's `fx::load`), so the smoke's map pushes in the view.
    #[test]
    fn an_effects_line_brings_its_map_rows_fields() {
        let f = std::env::temp_dir().join(format!("technoape-view-effects-{}.toml", std::process::id()));
        std::fs::write(&f, include_str!("../../../data/effects.toml")).unwrap();
        let set = read_set(f.to_str().unwrap(), &mut Default::default()).unwrap();
        assert!(set.fields.iter().any(|(n, _)| n == "smoke swirl"), "the smoke's field: {:?}", set.fields.iter().map(|f| &f.0).collect::<Vec<_>>());
        // a map not there is named (the view says it on stderr and keeps the rows before), never a panic
        std::fs::write(&f, include_str!("../../../data/effects.toml").replace("\"smoke swirl\"", "\"no such map\"")).unwrap();
        let e = read_set(f.to_str().unwrap(), &mut Default::default()).unwrap_err();
        assert!(e.contains("no such map"), "{e}");
        let _ = std::fs::remove_file(f);
    }

    /// Where a point shows, across and up over ahead, for an eye at `e` looking along `f`, up `u`.
    fn seen(p: DVec3, e: DVec3, f: DVec3, u: DVec3) -> [f64; 2] {
        let r = f.cross(u).normalize();
        let (d, u) = (p - e, r.cross(f));
        [d.dot(r) / d.dot(f), d.dot(u) / d.dot(f)]
    }

    /// The Studio draws its overlay (the part's box, the edges, the click's ray) as if the model stood in square
    /// metres seen from the eye it sends: a voxel of the model drawn in the view (`models::view_axes`, as the
    /// shown model is) shows where the Studio projects it, far from the look-at point too (the snake's tail).
    #[test]
    fn the_view_shows_a_model_point_where_the_studio_projects_it() {
        let w = World::new(768, 256, 1);
        let feet = w.air_over_ground(0, DVec3::Y);
        let (eye, target) = (DVec3::new(2.2, 1.4, 1.9), DVec3::new(0.5, 0.3, 0.5));
        let cam = cam_pose(&w, feet, eye, target);
        let [o, x, y, z] = models::view_axes(&w, feet);
        for p in [DVec3::new(0.5, 0.0, -0.5), DVec3::new(-1.0, 0.25, 1.75), DVec3::new(1.5, 0.75, 0.0)] {
            let drawn = seen(o + x * p.x + y * p.y + z * p.z, cam.pos, cam.fwd, cam.up);
            let studio = seen(p, eye, (target - eye).normalize(), DVec3::Y);
            assert!((drawn[0] - studio[0]).abs() < 1e-6 && (drawn[1] - studio[1]).abs() < 1e-6, "{p}: drawn at {drawn:?}, the Studio's {studio:?}");
        }
    }

    #[test]
    fn a_size_is_one_the_surface_takes() {
        assert_eq!(view_size(800.0, 600.0, 8192), [800, 600]);
        assert_eq!(view_size(1e5, f64::NAN, 8192), [8192, 64]);
        assert_eq!(view_size(-3.0, 1e12, 8192), [64, 8192]);
    }

    #[test]
    fn a_place_is_a_cell_near_the_stand_cell_and_a_block() {
        assert_eq!(place_spec("1:-2:0:stone"), Some((IVec3::new(1, -2, 0), BLOCKS.id("stone"))));
        assert_eq!(place_spec("0:0:1:_"), Some((IVec3::Z, AIR)));
        for bad in ["1:2", "0:0:1:nosuch", "x:0:0:stone", "2000000000:0:0:stone", "0:-2147483648:0:stone"] {
            assert_eq!(place_spec(bad), None, "{bad}");
        }
    }
}
