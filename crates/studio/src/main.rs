//! `technoape-studio`: paints the game's block textures from its one palette (docs/design/studio.md, ADR 0046).
//! Without arguments it opens its window in the repository the exe sits in. Headless, one JSON line each:
//! `check=1` (palette and every texture), `drafts=1`, `render=<png> sheet=out/x.png` (tiled 3 x 3),
//! `shot=out/studio.png [open=<block>] [page=palette]` (one frame of the window, off-screen).
mod art;
mod ui;

use eframe::egui;
use std::collections::HashMap;
use std::path::Path;

fn js(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
fn list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| js(s)).collect::<Vec<_>>().join(","))
}

fn write_rgba(path: &str, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(d) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let f = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut wr = e.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(rgba).map_err(|e| e.to_string())
}

fn check() -> Result<String, String> {
    let reg = engine::blocks::Registry::parse(&std::fs::read_to_string("data/blocks.toml").map_err(|e| e.to_string())?).map_err(|e| format!("data/blocks.toml: {e}"))?;
    let pal = std::fs::read_to_string(ui::PAL).map_err(|e| e.to_string()).and_then(|t| engine::palette::Palette::parse(&t)).map_err(|e| format!("{}: {e}", ui::PAL))?;
    let (ok, bad) = art::check_dir(Path::new(ui::TEX), &pal, &reg);
    let line = format!("{{\"check\":{},\"palette\":{},\"textures\":{ok},\"refused\":{}}}", js(if bad.is_empty() { "ok" } else { "failed" }), pal.colours.len(), list(&bad));
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
fn shot(studio: &mut ui::Studio, path: &str, w: u32, h: u32) -> Result<String, String> {
    let ctx = egui::Context::default();
    ctx.set_theme(egui::Theme::Dark);
    let mut textures = vec![];
    let mut out = None;
    // a few passes, so sizes measured in one frame are laid out in the next
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32))), ..Default::default() };
        let mut o = ctx.run_ui(input, |ui| studio.show(ui));
        textures.extend(std::mem::take(&mut o.textures_delta.set));
        o.textures_delta.clear();
        out = Some(o);
    }
    let out = out.unwrap();
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
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (w * h * 4) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    enc.copy_texture_to_buffer(
        tex.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) } },
        size,
    );
    cmds.push(enc.finish());
    queue.submit(cmds);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let px = buf.slice(..).get_mapped_range().map_err(|e| e.to_string())?.to_vec();
    write_rgba(path, w, h, &px)?;
    Ok(format!("{{\"shot\":{},\"width\":{w},\"height\":{h},\"adapter\":{}}}", js(path), js(&adapter.get_info().name)))
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
    let load = || {
        let mut s = ui::Studio::load(get("open"))?;
        if get("page") == Some("palette") {
            s.page = ui::Page::Palette;
        }
        Ok::<_, String>(s)
    };
    let result = if get("check") == Some("1") {
        check()
    } else if get("drafts") == Some("1") {
        Ok(format!("{{\"drafts\":{}}}", list(&art::drafts(&std::fs::read_to_string(ui::DRAFTS).unwrap_or_default()))))
    } else if let Some(file) = get("render") {
        render(file, get("sheet").unwrap_or("out/studio-sheet.png"))
    } else if let Some(path) = get("shot") {
        load().and_then(|mut s| shot(&mut s, path, 1280, 800))
    } else {
        let studio = match load() {
            Ok(s) => s,
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
