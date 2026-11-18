//! Maps (ADR 0153): the file and its working out are the engine's (`engine::map`, which the game reads too);
//! what only the Studio does with one is here: `check=1` over the folder, a picture's PNG bytes, export, `map=`.
pub use engine::map::*;
use engine::palette::Palette;
use std::path::Path;

/// `check=1` over `data/maps/*.toml`: the maps that passed and every refusal.
pub fn check_dir(dir: &Path, pal: &Palette) -> (usize, Vec<String>) {
    let mut files: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".toml")).collect();
    files.sort();
    let mut bad = vec![];
    for n in &files {
        let shown = format!("data/maps/{n}");
        if let Err(e) = std::fs::read_to_string(dir.join(n)).map_err(|e| format!("{shown}: {e}")).and_then(|t| parse(&shown, &t, pal)) {
            bad.push(e);
        }
    }
    (files.len() - bad.len(), bad)
}

/// A picture as PNG bytes: a number grey, a colour as it is, a direction in red and green (the middle is no push).
pub fn rgba(p: &Pic, kind: Data) -> Vec<u8> {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    p.iter().flat_map(|c| match kind {
        Data::Dir => [b(c[0] * 0.5 + 0.5), b(c[1] * 0.5 + 0.5), 0, 255],
        _ => [b(c[0]), b(c[1]), b(c[2]), 255],
    }).collect()
}

/// The out's frames side by side at the map's size, as written to a PNG: its width, height and RGBA bytes.
pub fn strip(g: &mut Graph, frames: &[usize]) -> (usize, usize, Vec<u8>) {
    let (s, o) = (g.map.size, g.out());
    let kind = g.kinds[o];
    let pics: Vec<Vec<u8>> = frames.iter().map(|&k| rgba(g.pic(o, k, s), kind)).collect();
    let row = s * 4;
    let px = (0..s).flat_map(|y| pics.iter().flat_map(move |p| p[y * row..(y + 1) * row].iter().copied())).collect();
    (s * frames.len(), s, px)
}

/// Export: the map's out at its size (an animated one as a strip of its frames) written to `out` as a PNG,
/// free colours; its width and height.
pub fn export(m: &Map, out: &str) -> Result<(usize, usize), String> {
    let mut g = Graph::new(m.clone());
    let (w, h, px) = strip(&mut g, &(0..m.frames).collect::<Vec<_>>());
    crate::write_rgba(out, w as u32, h as u32, &px)?;
    Ok((w, h))
}

/// `map=<name> out=x.png [frame=k]`: the map's out at its size, an animated one without `frame` as a strip of
/// its frames side by side; one JSON line.
pub fn draw(name: &str, out: &str, frame: Option<usize>) -> Result<String, String> {
    let read = |f: &str| std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"));
    let pal = Palette::parse(&read("data/palette.toml")?).map_err(|e| format!("data/palette.toml: {e}"))?;
    let file = format!("data/maps/{name}.toml");
    let mut g = Graph::new(parse(&file, &read(&file)?, &pal)?);
    let (s, o) = (g.map.size, g.out());
    let kind = g.kinds[o];
    let frames: Vec<usize> = frame.map_or((0..g.map.frames).collect(), |k| vec![k]);
    let start = std::time::Instant::now();
    let (w, h, px) = strip(&mut g, &frames);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    crate::write_rgba(out, w as u32, h as u32, &px)?;
    let hash = px.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100_0000_01b3));
    let kind = ["number", "colour", "direction", "any"][kind as usize];
    Ok(format!("{{\"map\":{},\"out\":{},\"size\":{s},\"frames\":{},\"drawn\":{},\"kind\":\"{kind}\",\"nodes\":{},\"ms\":{ms:.1},\"hash\":\"{hash:016x}\"}}", crate::js(name), crate::js(out), g.map.frames, frames.len(), g.map.nodes.len()))
}

#[cfg(test)]
mod tests;
