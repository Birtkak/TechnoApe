//! `technoape-studio`: paints the game's block textures from its one palette (docs/design/studio.md, ADR 0046).
//! Without arguments it opens its window in the repository the exe sits in. Headless, one JSON line each:
//! `check=1` (palette, every texture, model and map), `drafts=1`, `map=<name> out=out/x.png [frame=k]` (a map
//! of `data/maps/`, an animated one as a strip), `render=<png> sheet=out/x.png` (tiled 3 x 3),
//! `shot=out/studio.png [open=<block>] [page=palette] [cmd=id,id..] [query=text] [scale=1.5] [size=1280x800]
//! [pointer=x,y] [season=autumn]` (one frame of the window, off-screen: `cmd` runs commands as the command box does, e.g.
//! `frame:keys`, `tool.shade`, `open:Models:furnace`, `part:door`, `time:0.4`; `season` sets the view's year and the Textures page's season tab; `query` opens the command box with
//! that typed; `pointer` hovers there). Sounds: `page=Sounds cmd=open:Sounds:furnace.clank,start:metallic,mutate`
//! (`add:<step>`, `toggle:<card>`, `take:<wall item>`); a shot never opens the sound card. Effects:
//! `page=Effects cmd=open:Effects:flames,hour:22,vary,stop:1`. The game's view is drawn by `technoape run=view` (ADR 0049): the first there of
//! `renderer=`, `target/release/technoape.exe`, the exe beside the Studio, `build/TechnoApe.exe`; `view=0` none.
//! `input="move 400 300; down; drag @canvas:0.1,0.1 @canvas:0.9,0.1; key Ctrl+S"` drives the shot with real
//! input first (`drive.rs`); `shot=-` draws nothing; `rects=1` adds the pages' named rects; `root=dir` works in
//! that folder's `data/` (a scratch copy) instead of the working folder's; `recover=1` offers (and answers) the
//! unsaved work a crash left in `studio-recovery/`, which a shot otherwise leaves alone.
mod art;
mod drive;
mod effect;
mod font;
mod import;
mod map;
mod ramp;
mod recover;
mod rows;
mod sound;
mod pattern;
mod shell;
mod tools;
mod wall;
mod ui;
mod voxel;

use eframe::egui;
use std::collections::HashMap;
use std::path::Path;

pub fn js(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
fn list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| js(s)).collect::<Vec<_>>().join(","))
}

fn write_rgba(path: &str, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let mut buf = vec![];
    let mut e = png::Encoder::new(&mut buf, w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().and_then(|mut wr| wr.write_image_data(rgba)).map_err(|e| e.to_string())?;
    // through a temporary file and a rename: a reader never sees half a picture
    import::put_file(path, Some(&buf))
}

fn check() -> Result<String, String> {
    let (pal, ok, mut bad) = art::check_data(Path::new("data"))?;
    bad.extend(import::check_credits(Path::new("data/sounds")));
    let line = format!("{{\"check\":{},\"palette\":{pal},\"textures\":{ok},\"refused\":{}}}", js(if bad.is_empty() { "ok" } else { "failed" }), list(&bad));
    if bad.is_empty() {
        Ok(line)
    } else {
        Err(line)
    }
}

fn render(file: &str, sheet: &str) -> Result<String, String> {
    let img = art::decode(&std::fs::read(file).map_err(|e| format!("{file}: {e}"))?).map_err(|e| format!("{file}: {e}"))?;
    let (side, px) = art::tiled(&img, 3, 8);
    write_rgba(sheet, side as u32, side as u32, &px)?;
    Ok(format!("{{\"render\":{},\"sheet\":{},\"variants\":{},\"size\":{side}}}", js(file), js(sheet), img.variants()))
}

/// One frame of the window drawn off-screen by egui's own wgpu renderer, read back into a PNG.
fn shot(studio: &mut ui::Studio, path: &str, w: u32, h: u32, pointer: Option<egui::Pos2>, input: &str, rects: bool) -> Result<String, String> {
    let ctx = egui::Context::default();
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_zoom_factor(studio.settings.scale);
    let mut textures = vec![];
    let mut out = None;
    // a few passes, so sizes measured in one frame are laid out in the next; half a second apart, so a
    // tooltip under `pointer` shows
    let points = egui::vec2(w as f32, h as f32) / studio.settings.scale;
    let mut time = 0.0;
    let mut frame = |studio: &mut ui::Studio, f: drive::Frame, dt: f64| {
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, points)), time: Some(time), events: f.events, ..Default::default() };
        if f.close {
            input.viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
        }
        time += dt;
        let mut o = ctx.run_ui(input, |ui| studio.show(ui));
        textures.extend(std::mem::take(&mut o.textures_delta.set));
        o.textures_delta.clear();
        out = Some(o);
    };
    frame(studio, drive::Frame::default(), 0.5);
    // the game's renderer started by the first pass: wait for its settled frame
    studio.settle(&ctx);
    // sizes measured in one frame are laid out in the next: the script starts on a settled layout
    if !input.is_empty() {
        for _ in 0..2 {
            frame(studio, drive::Frame::default(), 0.5);
        }
    }
    // the script: frames 50 ms apart, so a press and a release are a click, not a long press
    let mut driver = drive::Driver::default();
    let (mut frames, mut slowest) = (0, 0.0f64);
    for step in input.split(';') {
        for f in driver.step(step, &drive::rects(&ctx)).map_err(|e| format!("input '{}': {e}", step.trim()))? {
            // the window's own work for a scripted frame (the UI pass, no GPU), the slowest kept
            let t = std::time::Instant::now();
            frame(studio, f, 0.05);
            slowest = slowest.max(t.elapsed().as_secs_f64() * 1e3);            frames += 1;
        }
        // and one more: what a step changed is drawn, and marked, before the next step aims at it
        frame(studio, drive::Frame::default(), 0.05);
        frames += 1;
    }
    // what the input changed in the game's view (the camera, a voxel) drawn before the picture is taken
    if !input.is_empty() {
        studio.settle(&ctx);
    }
    for pass in 0..3 {
        let events = pointer.filter(|_| pass == 0).map(|p| vec![egui::Event::PointerMoved(p)]).unwrap_or_default();
        frame(studio, drive::Frame { events, close: false }, 0.5);
    }
    drop(frame);
    let out = out.unwrap();
    let extra = format!(",\"frames\":{frames},\"slowest_ms\":{slowest:.1},\"state\":{}{}", studio.drive_report(), if rects { format!(",\"rects\":{}", drive::rects_json(&drive::rects(&ctx))) } else { String::new() });
    if path == "-" {
        return Ok(format!("{{\"shot\":\"-\"{extra}}}"));
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| format!("no device: {e}"))?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let mut r = egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());
    for (id, delta) in &textures {
        for d in delta {
            r.update_texture(&device, &queue, *id, d);
        }
    }
    let jobs = ctx.tessellate(out.shapes, out.pixels_per_point);
    let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: out.pixels_per_point };
    let mut enc = device.create_command_encoder(&Default::default());
    let mut cmds = r.update_buffers(&device, &queue, &mut enc, &jobs, &sd);
    {
        let mut rp = enc
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("studio shot"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            })
            .forget_lifetime();
        r.render(&mut rp, &jobs, &sd);
    }
    // wgpu wants a copied row's length a multiple of 256 bytes: padded here, cut off below
    let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    enc.copy_texture_to_buffer(
        tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        size,
    );
    cmds.push(enc.finish());
    queue.submit(cmds);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let px: Vec<u8> = buf.slice(..).get_mapped_range().map_err(|e| e.to_string())?.chunks(row as usize).flat_map(|r| r[..(w * 4) as usize].to_vec()).collect();
    write_rgba(path, w, h, &px)?;
    Ok(format!("{{\"shot\":{},\"width\":{w},\"height\":{h},\"adapter\":{},\"view\":{}{extra}}}", js(path), js(&adapter.get_info().name), studio.view_report()))
}

impl eframe::App for ui::Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.show(ui);
    }
}

fn main() {
    let args: HashMap<String, String> = std::env::args().skip(1).filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    if args.is_empty() {
        // a double click: the repository the exe sits in, like the game
        let exe = std::env::current_exe().unwrap_or_default();
        if let Some(r) = exe.ancestors().skip(1).find(|d| d.join("Cargo.toml").exists() && d.join("crates").is_dir()) {
            let _ = std::env::set_current_dir(r);
        }
    }
    let get = |k: &str| args.get(k).map(String::as_str);
    if let Some(r) = get("renderer").or(get("view").filter(|v| *v == "0")) {
        let _ = ui::viewport::RENDERER.set(r.to_string());
    }
    // the shot's file stays where it was named; the data is the root's
    let shot_path = get("shot").map(|p| if p == "-" { p.to_string() } else { std::path::absolute(p).map_or(p.to_string(), |a| a.to_string_lossy().into_owned()) });
    if let Some(r) = get("root") {
        if let Err(e) = std::env::set_current_dir(r) {
            println!("{{\"error\":{}}}", js(&format!("root={r}: {e}")));
            std::process::exit(1);
        }
    }
    let load = || {
        let mut s = ui::Studio::load(get("open"))?;
        if let Some(p) = get("page") {
            s.run_command(&format!("page:{p}"));
        }
        if let Some(q) = get("season") {
            s.run_command(&format!("season:{q}"));
        }
        Ok::<_, String>(s)
    };
    let result = if get("check") == Some("1") {
        check()
    } else if get("drafts") == Some("1") {
        Ok(format!("{{\"drafts\":{}}}", list(&art::drafts(&std::fs::read_to_string(ui::DRAFTS).unwrap_or_default()))))
    } else if let Some(name) = get("map") {
        map::draw(name, get("out").unwrap_or("out/map.png"), get("frame").and_then(|k| k.parse().ok()))
    } else if let Some(file) = get("render") {
        render(file, get("sheet").unwrap_or("out/studio-sheet.png"))
    } else if let Some(path) = shot_path.as_deref() {
        let (w, h) = get("size").and_then(|s| s.split_once('x')).and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))).unwrap_or((1280, 800));
        let pointer = get("pointer").and_then(|s| s.split_once(',')).and_then(|(x, y)| Some(egui::pos2(x.parse().ok()?, y.parse().ok()?)));
        load().and_then(|mut s| {
            // a shot never writes the developer's studio.txt
            s.keep_settings();
            if get("recover") == Some("1") {
                s.offer_kept();
            }
            if let Some(k) = get("scale").and_then(|k| k.parse::<f32>().ok()) {
                s.settings.scale = k.clamp(shell::Settings::SCALE.0, shell::Settings::SCALE.1);
            }
            for c in get("cmd").into_iter().flat_map(|c| c.split(',')).filter(|c| !c.is_empty()) {
                s.run_command(c);
            }
            if let Some(q) = get("query") {
                s.type_query(q);
            }
            shot(&mut s, path, w, h, pointer, get("input").unwrap_or(""), get("rects") == Some("1"))
        })
    } else {
        let studio = match load() {
            Ok(mut s) => {
                // (open= names a file: that one, not the last one)
                if get("open").is_none() {
                    s.resume();
                }
                s
            }
            Err(e) => {
                eprintln!("technoape-studio: {e}");
                std::process::exit(1);
            }
        };
        let opts = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_title("TechnoApe Studio").with_inner_size([1280.0, 800.0]), ..Default::default() };
        let r = eframe::run_native(
            "TechnoApe Studio",
            opts,
            Box::new(|cc| {
                cc.egui_ctx.set_theme(egui::Theme::Dark);
                Ok(Box::new(studio))
            }),
        );
        r.map(|_| String::new()).map_err(|e| e.to_string())
    };
    match result {
        Ok(line) if line.is_empty() => {}
        Ok(line) => println!("{line}"),
        Err(e) if e.starts_with('{') => {
            println!("{e}");
            std::process::exit(1);
        }
        Err(e) => {
            println!("{{\"error\":{}}}", js(&e));
            std::process::exit(1);
        }
    }
}
