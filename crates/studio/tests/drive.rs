//! The Studio driven by real input (`shot=- input=...`, src/drive.rs) on a scratch copy of `data/` under
//! `out/drive/<test>/`: what a user does first on each page, checked by the saved files and the state line.
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

/// A scratch folder whose `block` has no painting yet (the repository's blocks are painted, ADR 0084).
fn scratch_unpainted(name: &str, block: &str) -> PathBuf {
    let dir = scratch(name);
    // (and its fine painting in hd/, which the game reads in the namesake's place)
    for f in std::fs::read_dir(dir.join("data/textures")).unwrap().flatten().chain(std::fs::read_dir(dir.join("data/textures/hd")).into_iter().flatten().flatten()) {
        let n = f.file_name().to_string_lossy().into_owned();
        if n.strip_suffix(".png").is_some_and(|s| s.split(['@']).next().unwrap().trim_end_matches("_top").trim_end_matches("_side").trim_end_matches("_bottom") == block) {
            std::fs::remove_file(f.path()).unwrap();
        }
    }
    dir
}

/// A scratch folder whose furnace is the voxel one these tests paint (the shipped one is painted boxes, ADR 0116).
fn scratch_voxel_furnace(name: &str) -> PathBuf {
    let dir = scratch(name);
    std::fs::write(dir.join("data/models/furnace.toml"), include_str!("../testdata/furnace_voxels.toml")).unwrap();
    dir
}

/// A fresh scratch folder with the repository's `data/` in it.
fn scratch(name: &str) -> PathBuf {
    let dir = root().join("out/drive").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    copy(&root().join("data"), &dir.join("data"));
    dir
}

/// Runs the Studio in `dir` with the args (no game view, no picture) and returns its JSON line.
fn studio(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_technoape-studio")).current_dir(dir).args(["shot=-", "view=0"]).args(args).output().unwrap();
    let line = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{line}{}", String::from_utf8_lossy(&out.stderr));
    line
}

fn png(path: &Path) -> (u32, u32, Vec<u8>) {
    let d = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut r = d.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    (info.width, info.height, buf)
}

#[test]
fn textures_paint_undo_save() {
    let dir = scratch("textures");
    let line = studio(
        &dir,
        &[
            "open=dirt",
            "input=click @btn:pattern; click @btn:pat.use; drag @face:0.08,0.09 @face:0.92,0.09; drag @face:0.08,0.28 @face:0.92,0.28 right; key G; click @face:0.5,0.72; \
             mods alt; click @face:0.2,0.6; mods none; key Ctrl+Z; key Ctrl+Z; key Ctrl+Y; key Ctrl+S",
        ],
    );
    assert!(line.contains("\"unsaved\":[]"), "{line}");
    let (w, _, px) = png(&dir.join("data/textures/dirt.png"));
    let at = |x: u32, y: u32| px[((y * w + x) * 4) as usize..][..4].to_vec();
    // the pencil's row and the right button's row are one colour each, and not the same one
    assert!((2..14).all(|x| at(x, 1) == at(2, 1)), "the pencil row is not painted: {line}");
    assert!((2..14).all(|x| at(x, 4) == at(2, 4)), "the right row is not painted (or the redo lost it): {line}");
    assert_ne!(at(2, 1), at(2, 4), "both rows have the first colour");
    // the fill was undone: the pattern's noise is back there
    assert!((0..16).any(|x| at(x, 11) != at(0, 11)), "the fill is still there after two undos: {line}");
    let check = Command::new(env!("CARGO_BIN_EXE_technoape-studio")).current_dir(&dir).arg("check=1").output().unwrap();
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stdout));
}

/// ADR 0090: a fine painting (32 high) is painted and saved at its own size, and a new face of a block that has
/// one is fine too.
#[test]
fn a_fine_painting_paints_at_its_size_and_a_new_face_takes_it() {
    let dir = scratch_unpainted("fine", "shrub");
    let grass = [0x4c, 0x94, 0x33, 255];
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, 64, 32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut w = e.write_header().unwrap();
    w.write_image_data(&grass.repeat(64 * 32)).unwrap();
    w.finish().unwrap();
    std::fs::write(dir.join("data/textures/shrub_top.png"), out).unwrap();
    // the top: erase near its far corner; All (no file yet): erase near its near corner
    studio(&dir, &["open=shrub", "input=click @facetab:1; key X; click @face:0.9,0.9; click @facetab:0; click @face:0.1,0.1; key Ctrl+S"]);
    let alpha = |f: &str, x: u32, y: u32| {
        let (w, h, px) = png(&dir.join("data/textures").join(f));
        (w, h, px[((y * w + x) * 4 + 3) as usize])
    };
    assert_eq!(alpha("shrub_top.png", 28, 28), (64, 32, 0), "erased at (28, 28) of the 32 x 32 face");
    assert_eq!(alpha("shrub_top.png", 14, 14), (64, 32, 255), "not at (14, 14), as a 16 px face would");
    assert_eq!(alpha("shrub.png", 3, 3), (32, 32, 0), "the new face is fine, erased at (3, 3)");
    let check = Command::new(env!("CARGO_BIN_EXE_technoape-studio")).current_dir(&dir).arg("check=1").output().unwrap();
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stdout));
}

/// The JSON array of a named rect in a `rects=1` line: [x0, y0, x1, y1].
fn rect(line: &str, name: &str) -> [f32; 4] {
    let at = line.find(&format!("\"{name}\":[")).unwrap_or_else(|| panic!("no rect {name}: {line}")) + name.len() + 4;
    let v: Vec<f32> = line[at..line[at..].find(']').unwrap() + at].split(',').map(|n| n.parse().unwrap()).collect();
    [v[0], v[1], v[2], v[3]]
}

#[test]
fn models_tree_never_covers_the_files_at_scale_2() {
    let dir = scratch("models-scale2");
    let line = studio(&dir, &["page=Models", "scale=2", "size=1280x800", "rects=1"]);
    let (files, tree) = (rect(&line, "files"), rect(&line, "tree"));
    assert!(files[3] <= tree[1] + 0.5, "the file list runs under the parts tree: files {files:?}, tree {tree:?}");
}

#[test]
fn models_add_then_undo_leaves_the_model_as_opened() {
    let dir = scratch("models-undo");
    // orbit, add a voxel, remove it, scrub the timeline, undo both edits: nothing is left to save
    let line = studio(&dir, &["cmd=open:Models:furnace", "input=drag @view:0.5,0.5 @view:0.7,0.5; click @view:0.5,0.55; click right @view:0.5,0.55; drag @timeline:0.2,0.5 @timeline:0.6,0.5; key Ctrl+Z; key Ctrl+Z"]);
    assert!(line.contains("\"undo\":[],\"redo\":[\"Add\",\"Remove\"]"), "{line}");
    assert!(line.contains("\"unsaved\":[]"), "undone back to the start, yet unsaved (a colour the add took stays): {line}");
}

#[test]
fn models_paint_box_part_event_save_and_reparse() {
    let dir = scratch_voxel_furnace("models-edit");
    let line = studio(
        &dir,
        &["cmd=open:Models:furnace", "input=key P; click @view:0.5,0.55; key B; drag @view:0.45,0.5 @view:0.55,0.6; click @part:door; drag @event:0 @event:0:-20,0.5; key Ctrl+S"],
    );
    assert!(line.contains("\"undo\":[\"Paint\",\"Box\",\"Move an event\"]"), "a quick drag of an event marker does not move it: {line}");
    assert!(line.contains("\"unsaved\":[]"), "{line}");
    // the game's reader takes the saved file, states and events intact; the leading comment kept
    let text = std::fs::read_to_string(dir.join("data/models/furnace.toml")).unwrap();
    assert!(text.starts_with("# A draft furnace"), "{text}");
    let pal = engine::palette::Palette::parse(&std::fs::read_to_string(dir.join("data/palette.toml")).unwrap()).unwrap();
    let (_, life) = engine::models::parse_life("furnace.toml", &text, &pal, &|_| true, &|_| true).unwrap();
    assert!(life.states.iter().any(|s| s.name == "working" && s.effects.len() == 2 && s.light.is_some()), "{text}");
    let ev = &life.events.iter().find(|(c, _)| c == "work").unwrap().1;
    assert_eq!(ev.len(), 2);
    assert!(ev.iter().any(|e| e.t < 1.95), "the moved event's time: {ev:?}");
}

#[test]
fn models_save_keeps_comments_above_a_part_and_a_clip_and_crlf() {
    let dir = scratch_voxel_furnace("models-comments");
    let file = dir.join("data/models/furnace.toml");
    let text = std::fs::read_to_string(&file).unwrap().replace("\r\n", "\n");
    let text = text.replacen("[[part]]\nname = \"door\"", "# the door swings in work\n[[part]]\nname = \"door\"", 1).replacen("[[clip]]\nname = \"work\"", "# the clip of the open door\n[[clip]]\nname = \"work\"", 1);
    std::fs::write(&file, text.replace('\n', "\r\n")).unwrap();
    let line = studio(&dir, &["cmd=open:Models:furnace", "input=key P; click @view:0.5,0.55; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"Paint\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let after = std::fs::read_to_string(&file).unwrap();
    assert!(!after.replace("\r\n", "").contains('\n'), "LF in a CRLF file");
    let after = after.replace("\r\n", "\n");
    // (the writer puts a section's keys in its own order: the comment stands right above the section of that name)
    for (note, head, name) in [("# the door swings in work\n", "[[part]]\n", "name = \"door\""), ("# the clip of the open door\n", "[[clip]]\n", "name = \"work\"")] {
        let at = after.find(&format!("{note}{head}")).unwrap_or_else(|| panic!("{note} lost or moved: {after}")) + note.len() + head.len();
        let section = &after[at..after[at..].find("\n[[").map_or(after.len(), |e| at + e)];
        assert!(section.contains(name), "{note} stands above another section: {after}");
    }
}

/// An undo that would take away a colour a texture now uses is refused, and the palette stays.
#[test]
fn palette_undo_never_removes_a_colour_in_use() {
    let dir = scratch("palette-in-use");
    studio(&dir, &["open=dirt", "input=click @btn:pattern; click @btn:pat.use; key Ctrl+S"]);
    // a colour added, painted into dirt.png and saved, then the add undone on the Palette page
    let line = studio(&dir, &["cmd=page:Palette,colour.add,open:Textures:dirt,colours=new 1;new 1", "input=click @face:0.5,0.5; key Ctrl+S; click @tab:Palette; key Ctrl+Z"]);
    assert!(line.contains("\"undo\":[\"Add\"],\"redo\":[]"), "the undo went through: {line}");
    let pal = std::fs::read_to_string(dir.join("data/palette.toml")).unwrap();
    assert!(pal.contains("name = \"new 1\""), "{pal}");
}

/// The file's line ends as they were (the checkout's: CRLF or LF), not mixed.
fn ends_kept(before: &str, after: &str) {
    let crlf = |t: &str| t.matches("\r\n").count() == t.matches('\n').count();
    let lf = |t: &str| !t.contains("\r\n");
    assert!((crlf(before) && crlf(after)) || (lf(before) && lf(after)), "line ends changed or mixed");
}

/// The `[[effect]]` (or `[[sound]]`) row named `name` of a file: from its header to the next `[[`.
fn row<'a>(text: &'a str, head: &str, name: &str) -> &'a str {
    let at = text.find(&format!("{head}\nname = \"{name}\"")).unwrap_or_else(|| panic!("no row {name}"));
    let end = text[at + 2..].find("\n[[").map_or(text.len(), |e| at + 2 + e + 1);
    &text[at..end]
}

#[test]
fn effects_stops_and_curve_points_drag_and_the_row_saves() {
    let dir = scratch("effects");
    let before = std::fs::read_to_string(dir.join("data/effects.toml")).unwrap();
    // a tall window: the gradient and the curves are low in the inspector
    let line = studio(
        &dir,
        &["cmd=open:Effects:flames", "size=1280x1800", "input=drag @stop:1 @stop:1:15,0.5; click @gradient:0.9,0.8; click right @stop:3; drag @point:size_curve:1 @point:size_curve:1:10,4; key Ctrl+S"],
    );
    assert!(line.contains("\"undo\":[\"Move a stop\",\"Add a stop\",\"Remove a stop\",\"size: move a point\"]"), "a quick drag of a stop or a curve point does not take it: {line}");
    assert!(line.contains("\"unsaved\":[]"), "{line}");
    let after = std::fs::read_to_string(dir.join("data/effects.toml")).unwrap();
    client::fx::parse_set(&after, &client::fx::PALETTE).unwrap();
    // the checkout's line ends kept
    ends_kept(&before, &after);
    let (before, after) = (before.replace("\r\n", "\n"), after.replace("\r\n", "\n"));
    // only the flames row changed
    let (old, new) = (row(&before, "[[effect]]", "flames"), row(&after, "[[effect]]", "flames"));
    assert_ne!(old, new);
    assert_eq!(before.replacen(old, "", 1), after.replacen(new, "", 1));
}

/// One long fast drag of a stop and of a curve point (the first move already past the pick distance): each
/// takes what it was pressed on, not what is under the pointer when egui reports the drag.
#[test]
fn effects_a_long_fast_drag_takes_the_stop_and_the_point_it_started_on() {
    let dir = scratch("effects-fast");
    let line = studio(&dir, &["cmd=open:Effects:flames", "size=1280x1800", "input=drag @stop:1 @stop:1:100,0.5; drag @point:size_curve:1 @point:size_curve:1:60,-20"]);
    assert!(line.contains("\"undo\":[\"Move a stop\",\"size: move a point\"]"), "a long fast drag missed the stop or the point it started on: {line}");
}

#[test]
fn sounds_slider_reorder_mutate_play_save_one_row() {
    let dir = scratch("sounds");
    let before = std::fs::read_to_string(dir.join("data/sounds.toml")).unwrap();
    let line = studio(
        &dir,
        &[
            "cmd=open:Sounds:furnace.clank",
            "size=1280x1400",
            "input=drag @handle:0:15.7,2.28 @handle:0:20,2.28; drag @handle:1 @handle:3:0.5,3; key M; wait 3; click @wall:2; key Space; key Ctrl+S",
        ],
    );
    assert!(line.contains("\"undo\":[\"pitch semitones\",\"Move a step\",\"Take a variation\"]"), "{line}");
    assert!(line.contains("\"unsaved\":[]"), "{line}");
    let after = std::fs::read_to_string(dir.join("data/sounds.toml")).unwrap();
    client::audio::parse(&after).unwrap();
    ends_kept(&before, &after);
    let (before, after) = (before.replace("\r\n", "\n"), after.replace("\r\n", "\n"));
    let (old, new) = (row(&before, "[[sound]]", "furnace.clank"), row(&after, "[[sound]]", "furnace.clank"));
    assert_ne!(old, new);
    assert_eq!(before.replacen(old, "", 1), after.replacen(new, "", 1), "a line outside the row changed");
}

#[test]
fn shell_search_find_and_the_close_question() {
    let dir = scratch("shell");
    // letters typed in Search stay text: no page key fires while typing, Ctrl+Z there is not the page's undo
    let line = studio(&dir, &["open=dirt", "input=click @face:0.5,0.5; click @search; key S; key G; text ir; key Ctrl+Z"]);
    assert!(line.contains("\"undo\":[\"Pencil\"]"), "{line}");
    let line = studio(&dir, &["open=dirt", "input=click @search; key S; key G; text ir"]);
    assert!(line.contains("\"search\":\"sgir\""), "{line}");
    // Find: typed and Enter opens the file on its page
    let line = studio(&dir, &["open=dirt", "input=click @find; text furn; key Enter"]);
    assert!(line.contains("\"page\":\"Models\",\"open\":\"furnace\""), "{line}");
    // the key sheet opens from its button and Esc closes it
    let line = studio(&dir, &["input=click @keys"]);
    assert!(line.contains("\"overlay\":\"keys\""), "{line}");
    let line = studio(&dir, &["input=click @keys; key Escape"]);
    assert!(line.contains("\"overlay\":\"none\""), "{line}");
    // closing with unsaved work asks first
    let line = studio(&dir, &["open=dirt", "input=click @face:0.5,0.5; close"]);
    assert!(line.contains("\"closing\":true"), "{line}");
    let line = studio(&dir, &["open=dirt", "input=close"]);
    assert!(line.contains("\"closing\":false"), "{line}");
}

#[test]
fn palette_slider_moves_a_colour_and_its_texture_follows() {
    let dir = scratch("palette");
    studio(&dir, &["open=dirt", "input=click @btn:pattern; click @btn:pat.use; key Ctrl+S"]);
    let tex = dir.join("data/textures/dirt.png");
    let (w, _, px) = png(&tex);
    let rgb = [px[0], px[1], px[2]];
    let pal = |d: &Path| engine::palette::Palette::parse(&std::fs::read_to_string(d.join("data/palette.toml")).unwrap()).unwrap();
    let p0 = pal(&dir);
    let name = p0.colours[p0.find(rgb).unwrap()].name.clone();
    // a window tall enough for every ramp: the colour the pattern snapped to can be any of them
    let line = studio(&dir, &["page=Palette", "rects=1", "size=1280x4000", &format!("input=click @colour:{}; drag @slider:value:0.5,0.5 @slider:value:0.2,0.5", name.replace(' ', "_"))]);
    let p1 = pal(&dir);
    let moved = p1.colours.iter().find(|c| c.name == name).unwrap().rgb;
    assert_ne!(moved, rgb, "the value slider did not move {name}: {line}");
    let (_, _, px) = png(&tex);
    assert_eq!([px[0], px[1], px[2]], moved, "dirt.png did not follow {name}");
    let _ = w;
}

#[test]
fn palette_undo_leaves_the_files_as_they_were() {
    let dir = scratch("palette-undo");
    studio(&dir, &["open=dirt", "input=click @btn:pattern; click @btn:pat.use; key Ctrl+S"]);
    let (pal_path, tex) = (dir.join("data/palette.toml"), dir.join("data/textures/dirt.png"));
    let (pal0, tex0) = (std::fs::read(&pal_path).unwrap(), std::fs::read(&tex).unwrap());
    let (_, _, px) = png(&tex);
    let p0 = engine::palette::Palette::parse(&String::from_utf8_lossy(&pal0)).unwrap();
    let name = p0.colours[p0.find([px[0], px[1], px[2]]).unwrap()].name.replace(' ', "_");
    let other = p0.colours.iter().find(|c| c.name.replace(' ', "_") != name).unwrap().name.replace(' ', "_");
    // a colour moved, a colour added, a colour dragged to another place: three undos and both files are as they were
    let line = studio(
        &dir,
        &["page=Palette", &format!("input=click @colour:{name}; drag @slider:value:0.5,0.5 @slider:value:0.2,0.5; key Ctrl+Z; key Ctrl+Y; key Ctrl+Z")],
    );
    assert!(line.contains("\"undo\":[],\"redo\":[\"Colour\"]"), "{line}");
    assert_eq!(std::fs::read(&pal_path).unwrap(), pal0, "the palette is not as it was after undo: {line}");
    assert_eq!(std::fs::read(&tex).unwrap(), tex0, "dirt.png did not go back with its colour: {line}");
    let line = studio(&dir, &["page=Palette", "cmd=colour.add", &format!("input=drag @colour:{other} @colour:{name}; key Ctrl+Z; key Ctrl+Z")]);
    assert!(line.contains("\"undo\":[],\"redo\":[\"Add\",\"Move a colour\"]"), "{line}");
    assert_eq!(std::fs::read(&pal_path).unwrap(), pal0, "add and move undone, yet the palette changed: {line}");
}

/// A slab of stone 8 x 2 x 8 voxels with one voxel of copper in its top at x 4, z 4 (only its top face shows),
/// and an empty layer over it.
const PROBE: &str = "colours = [\"stone\", \"copper\"]\n\n[[part]]\nname = \"slab\"\nbox = [8, 3, 8]\nvoxels = \"\"\"\n";

fn probe_model() -> String {
    let layer = |row: &dyn Fn(usize) -> &'static str| (0..8).map(|z| format!("{}\n", row(z))).collect::<String>();
    format!("{PROBE}{}\n{}\n{}\"\"\"\n", layer(&|_| "aaaaaaaa"), layer(&|z| if z == 4 { "aaaabaaa" } else { "aaaaaaaa" }), layer(&|_| "........"))
}

/// The game's view live (`technoape run=view`, a release build: a debug one bakes the planet for minutes). Run it
/// with `cargo build --release -p client` (hold `out/.lock`), then
/// `cargo test -p studio --test drive -- --ignored models_live_view`.
#[test]
#[ignore = "needs target/release/technoape.exe; see the doc comment"]
fn models_live_view_a_click_adds_on_the_face_seen_and_a_drag_turns_it() {
    let exe = root().join("target/release/technoape.exe");
    assert!(exe.is_file(), "no {}: cargo build --release -p client", exe.display());
    let dir = scratch("models-live");
    // the planet's bake (25 s to make again)
    std::fs::create_dir_all(dir.join("saves/bakes")).unwrap();
    for e in std::fs::read_dir(root().join("saves/bakes")).into_iter().flatten().flatten() {
        std::fs::copy(e.path(), dir.join("saves/bakes").join(e.file_name())).unwrap();
    }
    std::fs::write(dir.join("data/models/probe.toml"), probe_model()).unwrap();
    let renderer = format!("renderer={}", exe.display());
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_technoape-studio")).current_dir(&dir).args([renderer.as_str(), "size=1280x800", "cmd=open:Models:probe"]).args(args).output().unwrap();
        let line = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(out.status.success(), "{line}{}", String::from_utf8_lossy(&out.stderr));
        line
    };
    // the view as the game draws it: where the copper voxel shows
    let a = dir.join("a.png");
    let line0 = run(&[&format!("shot={}", a.display()), "rects=1"]);
    assert!(line0.contains("\"view\":{\"up\":true"), "the game's view never came up: {line0}");
    let v = rect(&line0, "view");
    let (w, _, px) = png(&a);
    let (mut n, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for y in v[1] as u32..v[3] as u32 {
        for x in v[0] as u32..v[2] as u32 {
            let p = &px[((y * w + x) * 4) as usize..][..3];
            let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
            if r - b > 90 && r - g > 50 {
                (n, sx, sy) = (n + 1.0, sx + x as f32, sy + y as f32);
            }
        }
    }
    assert!(n > 10.0, "no copper voxel in the view's picture ({n} pixels): {line0}");
    let (cx, cy) = (sx / n, sy / n);
    // a click there with the Add tool: the new voxel sits on it
    let line = run(&[&format!("input=key A; click {cx:.1} {cy:.1}; key Ctrl+S"), "shot=-"]);
    assert!(line.contains("\"undo\":[\"Add\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let pal = engine::palette::Palette::parse(&std::fs::read_to_string(dir.join("data/palette.toml")).unwrap()).unwrap();
    let m = engine::models::Model::parse("probe", &std::fs::read_to_string(dir.join("data/models/probe.toml")).unwrap(), &pal).unwrap();
    let p = &m.parts[0];
    let [sx, _, sz] = p.size.map(|v| v as usize);
    let added: Vec<[usize; 3]> = (0..p.cells.len()).filter(|&i| i / (sx * sz) == 2 && p.cells[i] != 0).map(|i| [i % sx, i / (sx * sz), i / sx % sz]).collect();
    assert_eq!(added.len(), 1, "one voxel added over the slab: {added:?}");
    let [x, _, z] = added[0];
    assert_eq!([x, z], [4, 4], "the click on the copper voxel's top (at {cx:.0}, {cy:.0}, {n} pixels) added beside another");
    // a right drag turns the camera, and the game draws it turned
    let b = dir.join("b.png");
    let line2 = run(&[&format!("shot={}", b.display()), "input=drag @view:0.5,0.5 @view:0.75,0.4 right"]);
    let orbit = |l: &str| l[l.find("\"orbit\":").unwrap()..].split(']').next().unwrap().to_string();
    assert_ne!(orbit(&line0), orbit(&line2), "the drag did not turn the orbit");
    let (_, _, px2) = png(&b);
    let mut differ = 0;
    for y in v[1] as u32..v[3] as u32 {
        for x in v[0] as u32..v[2] as u32 {
            let i = ((y * w + x) * 4) as usize;
            if px[i..i + 3].iter().zip(&px2[i..i + 3]).any(|(a, b)| a.abs_diff(*b) > 24) {
                differ += 1;
            }
        }
    }
    let all = ((v[2] - v[0]) * (v[3] - v[1])) as i32;
    assert!(differ > all / 10, "the game's frame barely changed after the turn ({differ} of {all} pixels)");
}

#[test]
fn models_pivot_drags_and_undoes() {
    let dir = scratch("models-pivot");
    let line = studio(&dir, &["cmd=open:Models:furnace", "input=click @part:door; key V; drag @pivot @pivot:-10,-5"]);
    assert!(line.contains("\"undo\":[\"Move the pivot\"]") && line.contains("\"unsaved\":[\"furnace.toml\"]"), "{line}");
    let line = studio(&dir, &["cmd=open:Models:furnace", "input=click @part:door; key V; drag @pivot @pivot:-10,-5; key Ctrl+Z"]);
    assert!(line.contains("\"undo\":[],\"redo\":[\"Move the pivot\"]") && line.contains("\"unsaved\":[]"), "{line}");
}

#[test]
fn sounds_switch_off_and_add_a_step_from_the_menu() {
    let dir = scratch("sounds-menu");
    let line = studio(&dir, &["cmd=open:Sounds:furnace.clank", "size=1280x1400", "input=click @on:1; click @add_step; click @step:echo; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"Switch a step off\",\"Add echo\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let text = std::fs::read_to_string(dir.join("data/sounds.toml")).unwrap();
    client::audio::parse(&text).unwrap();
}

#[test]
fn effects_start_shape_changes_and_saves() {
    let dir = scratch("effects-start");
    let line = studio(&dir, &["cmd=open:Effects:flames", "size=1280x1800", "input=click @choice:start:ring; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"start: ring\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let text = std::fs::read_to_string(dir.join("data/effects.toml")).unwrap().replace("\r\n", "\n");
    client::fx::parse_set(&text, &client::fx::PALETTE).unwrap();
    let r = row(&text, "[[effect]]", "flames");
    assert!(r.contains("start = \"ring\"") && r.contains("radius = ") && !r.contains("box = "), "{r}");
}

#[test]
fn effects_colour_dir_then_texture_and_model_save_as_the_game_reads_them() {
    let dir = scratch("effects-fields");
    let read = || {
        let text = std::fs::read_to_string(dir.join("data/effects.toml")).unwrap();
        let set = client::fx::parse_set(&text, &client::fx::PALETTE).unwrap_or_else(|e| panic!("the game refuses the saved file: {e}"));
        set.rows.into_iter().find(|r| r.name == "flames").unwrap()
    };
    let line = studio(
        &dir,
        &[
            "cmd=open:Effects:flames",
            "size=1280x1800",
            "input=click @btn:one_colour; drag @field:red:0.9,0.5 @field:red:0.3,0.5; click @choice:start:cone; drag @field:dir_x:0.5,0.5 @field:dir_x:0.8,0.5; \
             click @pick:then; click @pick:then:embers; key Ctrl+S",
        ],
    );
    assert!(line.contains("\"unsaved\":[]"), "{line}");
    let r = read();
    assert!(r.gradient.is_empty() && r.colour.is_some_and(|c| c[0] < 0.6), "one colour, its red moved: {:?} {line}", r.colour);
    assert!(r.dir[0] > 0.3 && r.dir[1] == 1.0, "the cone's dir: {:?} {line}", r.dir);
    assert_eq!(r.then.as_deref(), Some("embers"), "{line}");
    let line = studio(&dir, &["cmd=open:Effects:flames", "size=1280x1800", "input=click @choice:look:texture; click @pick:texture; click @pick:texture:dirt; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"look: texture\",\"texture: dirt\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let r = read();
    assert_eq!((r.look, r.texture.as_deref(), r.model.as_deref()), (client::fx::Look::Texture, Some("dirt"), None));
    let line = studio(&dir, &["cmd=open:Effects:flames", "size=1280x1800", "input=click @choice:look:model; click @pick:model; click @pick:model:furnace; key Ctrl+S"]);
    assert!(line.contains("\"unsaved\":[]"), "{line}");
    let r = read();
    assert_eq!((r.look, r.texture.as_deref(), r.model.as_deref(), r.glow), (client::fx::Look::Model, None, Some("furnace"), 0.0), "{line}");
}

/// A creature row of a creatures.toml as the game reads it.
fn creature(dir: &Path, name: &str) -> engine::creatures::Creature {
    let pal = engine::palette::Palette::parse(&std::fs::read_to_string(dir.join("data/palette.toml")).unwrap()).unwrap();
    let model = |n: &str| std::fs::read_to_string(dir.join(format!("data/models/{n}.toml"))).ok().and_then(|t| engine::models::Model::parse(n, &t, &pal).ok());
    let text = std::fs::read_to_string(dir.join("data/creatures.toml")).unwrap().replace("\r\n", "\n");
    engine::creatures::parse("creatures.toml", &text, &model, &|_| true).unwrap().into_iter().find(|c| c.name == name).unwrap()
}

#[test]
fn fauna_slider_and_a_gait_from_the_wall_save_the_row_as_shown() {
    let dir = scratch("fauna");
    let before = std::fs::read_to_string(dir.join("data/creatures.toml")).unwrap();
    let c0 = creature(&dir, "boar");
    let line = studio(&dir, &["cmd=open:Fauna:boar", "size=1280x1400", "input=drag @slider:scale @slider:scale:0,0.5; key V; wait 2; click @wall:3; wait 2; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"Row\",\"Take a variation\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let c = creature(&dir, "boar");
    assert_eq!(c.scale, 0.25, "the scale slider dragged to its left end: {line}");
    assert!(["swing", "bob", "sway", "wag"].iter().any(|k| c.get(k) != c0.get(k)), "the wall's gait was not saved: {line}");
    // the body numbers the wall does not vary stay as the slider left them, the rest of the file as it was
    assert_eq!((c.walk_speed, c.run_speed, c.model.as_str()), (c0.walk_speed, c0.run_speed, c0.model.as_str()));
    let after = std::fs::read_to_string(dir.join("data/creatures.toml")).unwrap();
    ends_kept(&before, &after);
    let (before, after) = (before.replace("\r\n", "\n"), after.replace("\r\n", "\n"));
    let (old, new) = (row(&before, "[[creature]]", "boar"), row(&after, "[[creature]]", "boar"));
    assert_eq!(before.replacen(old, "", 1), after.replacen(new, "", 1), "a line outside the row changed");
}

fn structure(dir: &Path, name: &str) -> (String, engine::structures::Structure, engine::blocks::Registry) {
    let reg = engine::blocks::Registry::parse(&std::fs::read_to_string(dir.join("data/blocks.toml")).unwrap()).unwrap();
    let text = std::fs::read_to_string(dir.join(format!("data/structures/{name}.toml"))).unwrap();
    let s = engine::structures::Structure::parse(name, &text, &reg).unwrap_or_else(|e| panic!("{e}\n{text}"));
    (text, s, reg)
}

#[test]
fn structures_box_drag_fills_the_cells_dragged_over_and_reloads_equal() {
    let dir = scratch("structures-box");
    // a new 5 x 3 x 5 box; the Add tool dragged along the last row, the same; a quick drag from cell (0, 0) to (4, 2) of the bottom layer (its first move already
    // in the next cell: the box starts where the button went down)
    let line = studio(&dir, &["page=Structures", "cmd=new,tool:Box,block:stone", "size=1280x1400", "input=drag @slice:0.15,0.1 @slice:0.95,0.5; key A; drag @slice:0.15,0.9 @slice:0.95,0.9; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"Box\",\"Add\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let (text, s, _) = structure(&dir, "new");
    let stone = s.key.iter().find(|(_, n)| *n == "stone").map(|(l, _)| l.as_bytes()[0]).unwrap_or_else(|| panic!("{text}"));
    for z in 0..5 {
        for x in 0..5 {
            let want = if z <= 2 || z == 4 { stone } else { engine::structures::KEEP };
            assert_eq!(s.cells[s.at(x, 0, z)], want, "cell x {x} z {z}: the box is not the cells dragged over\n{text}");
        }
    }
    assert!(s.cells[s.at(0, 1, 0)..].iter().all(|c| *c == engine::structures::KEEP), "only the layer shown: {text}");
    assert_eq!(s.write(), text.replace("\r\n", "\n"), "read and written again, the same file");
    let line = studio(&dir, &["cmd=open:Structures:new"]);
    assert!(line.contains("\"unsaved\":[]") && line.contains("\"open\":\"new\""), "reopened, it differs from its file: {line}");
}

/// Two strokes of Add are two undo steps: one undo takes back only the second.
#[test]
fn structures_two_strokes_undo_one_at_a_time() {
    let dir = scratch("structures-strokes");
    let line = studio(&dir, &["page=Structures", "cmd=new,tool:Add,block:stone", "size=1280x1400", "input=drag @slice:0.15,0.1 @slice:0.95,0.1; drag @slice:0.15,0.9 @slice:0.95,0.9; key Ctrl+Z"]);
    assert!(line.contains("\"undo\":[\"Add\"],\"redo\":[\"Add\"]"), "{line}");
}

#[test]
fn structures_freeze_writes_the_grown_blocks_as_layers() {
    let dir = scratch("structures-freeze");
    let (_, grown, reg) = structure(&dir, "oak");
    assert!(grown.grow.is_some());
    let line = studio(&dir, &["cmd=open:Structures:oak", "size=1280x1400", "input=click @btn:freeze; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"Freeze\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let (text, s, _) = structure(&dir, "oak");
    assert!(s.grow.is_none() && !text.contains("[grow]") && text.contains("layers"), "{text}");
    assert_eq!((s.size, s.anchor), (grown.size, grown.anchor));
    assert_eq!(s.blocks(&reg), grown.blocks(&reg), "the frozen layers are not what was grown");
    let line = studio(&dir, &["cmd=open:Structures:oak"]);
    assert!(line.contains("\"unsaved\":[]"), "{line}");
}

#[test]
fn structures_oak_recipe_another_wall_save() {
    let dir = scratch("structures-oak");
    // a variation taken from the wall is what was shown: no slider snaps it into another step
    let line = studio(&dir, &["cmd=open:Structures:oak", "size=1280x1400", "input=key V; wait 2; click @wall:3; wait 3"]);
    assert!(line.contains("\"undo\":[\"Take a variation\"]"), "the wall's pick was changed by the recipe sliders: {line}");
    let line = studio(
        &dir,
        &["cmd=open:Structures:oak", "size=1280x1400", "input=drag @recipe:branches:0.3,0.5 @recipe:branches:0.6,0.5; click @another; key V; wait 2; click @wall:3; key Ctrl+S"],
    );
    assert!(line.contains("\"undo\":[\"Recipe\",\"Another seed\",\"Take a variation\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let text = std::fs::read_to_string(dir.join("data/structures/oak.toml")).unwrap();
    let reg = engine::blocks::Registry::parse(&std::fs::read_to_string(dir.join("data/blocks.toml")).unwrap()).unwrap();
    engine::structures::Structure::parse("oak.toml", &text, &reg).unwrap();
}

/// docs/design/seasonal-looks.md: a season tab paints `<block>@<season>.png` from the base, "Add season" puts the
/// base colour in a `[block.<season>]` and "Remove" drops the files and the look again.
#[test]
fn a_season_is_added_painted_and_removed() {
    let dir = scratch("seasons");
    let blocks = || std::fs::read_to_string(dir.join("data/blocks.toml")).unwrap();
    let before = blocks();
    studio(&dir, &["open=dirt", "input=click @btn:pattern; click @btn:pat.use; key Ctrl+S"]);
    studio(&dir, &["open=dirt", "season=winter", "cmd=season.add", "input=key Ctrl+S"]);
    let winter = dir.join("data/textures/dirt@winter.png");
    assert_eq!(png(&winter).2, png(&dir.join("data/textures/dirt.png")).2, "the season starts as the base");
    let reg = engine::blocks::Registry::parse(&blocks()).unwrap();
    let d = reg.get(reg.id("dirt")).unwrap();
    let text = blocks();
    let row = &text[text.find("name = \"dirt\"").unwrap()..];
    let row = &row[..row.find("[[block]]").unwrap_or(row.len())];
    assert!(row.contains("[block.winter]"), "a [block.winter] after dirt's row: {row}");
    assert_eq!(d.tone(3).colour, d.colour, "with the base colour");
    let line = studio(&dir, &["open=dirt", "season=winter", "cmd=season.remove"]);
    assert!(!winter.exists(), "{line}");
    assert_eq!(blocks(), before, "data/blocks.toml as it was, byte for byte");
}

#[test]
fn the_pattern_panel_picks_bricks_three_colours_and_a_size() {
    // usability round step 2: pick bricks, set three colours and a size, Use, save; the file passes check=1
    let dir = scratch_unpainted("pattern-panel", "clay");
    let line = studio(
        &dir,
        &[
            "open=clay",
            "input=click @btn:pattern; click @pat:bricks; click @well:0; click @swatch:copper; click @well:1; click @swatch:stone_dark; \
             click @well:2; click @swatch:copper_light; click @slider:size:0.3,0.5; click @btn:pat.use; key Ctrl+S",
        ],
    );
    assert!(line.contains("\"undo\":[\"Start from pattern\"]") && line.contains("\"unsaved\":[]"), "{line}");
    let (w, h, px) = png(&dir.join("data/textures/clay.png"));
    assert_eq!((w, h), (16, 16));
    let red = |x: u32, y: u32| px[((y * w + x) * 4) as usize];
    // size 3: mortar rows 0, 3, 6 in the dark second colour, the bricks between them copper
    for y in [0, 3, 6] {
        assert!((0..16).all(|x| red(x, y) < 110), "row {y} is mortar: {line}");
    }
    assert!((0..16).filter(|&x| red(x, 2) > 150).count() > 10, "row 2 is brick");
    let out = Command::new(env!("CARGO_BIN_EXE_technoape-studio")).current_dir(&dir).arg("check=1").output().unwrap();
    assert!(out.status.success() && String::from_utf8_lossy(&out.stdout).contains("\"check\":\"ok\""), "{}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn generate_variants_picks_a_set_and_a_variant_is_removed() {
    // usability round step 3: a set is pending until Use this; the file is then 3 variants (48 px) wide
    let dir = scratch_unpainted("variants", "clay");
    let tex = dir.join("data/textures/clay.png");
    let line = studio(&dir, &["open=clay", "input=click @btn:pattern; click @btn:pat.use; click @btn:variants; click @wall:3; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"Start from pattern\"]"), "a picked set is only pending: {line}");
    assert_eq!(png(&tex).0, 16, "pending is not saved");
    let line = studio(&dir, &["open=clay", "input=click @btn:variants; click @wall:3; click @btn:wall.use; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"Generate variants\"]") && line.contains("\"unsaved\":[]"), "{line}");
    assert_eq!(png(&tex).0, 48);
    // the x on a variant that holds paint asks first; Remove takes it out, one undo step
    let line = studio(&dir, &["open=clay", "input=click @variant.remove:2; key Ctrl+S"]);
    assert_eq!(png(&tex).0, 48, "asked, not removed: {line}");
    let line = studio(&dir, &["open=clay", "input=click @variant.remove:2; click @btn:variant.remove; key Ctrl+S"]);
    assert!(line.contains("\"undo\":[\"Remove variant 2\"]"), "{line}");
    assert_eq!(png(&tex).0, 32);
}

#[test]
fn textures_kept_after_a_crash_are_offered_back() {
    // usability round: unsaved work kept in studio-recovery/ is offered at the next start; Restore makes it unsaved work
    let dir = scratch("recover");
    studio(&dir, &["open=clay", "input=click @btn:pattern; click @btn:pat.use; key Ctrl+S"]);
    let (tex, kept) = (dir.join("data/textures/clay.png"), dir.join("studio-recovery/textures"));
    std::fs::create_dir_all(&kept).unwrap();
    std::fs::rename(&tex, kept.join("clay.png")).unwrap();
    // a plain shot neither offers nor clears the developer's kept work
    let line = studio(&dir, &["open=clay", "input=wait", "rects=1"]);
    assert!(!line.contains("btn:recover") && kept.join("clay.png").exists(), "a shot leaves the kept set alone: {line}");
    let line = studio(&dir, &["open=clay", "recover=1", "input=wait", "rects=1"]);
    assert!(line.contains("\"unsaved\":[]") && line.contains("btn:recover.restore"), "offered, not restored before the answer: {line}");
    let line = studio(&dir, &["open=clay", "recover=1", "input=click @btn:recover.restore; key Ctrl+S"]);
    assert!(line.contains("Restored as unsaved work: clay.png") && line.contains("Saved clay.png"), "{line}");
    assert_eq!(std::fs::read(&tex).unwrap(), std::fs::read(kept.join("clay.png")).unwrap());
    // kept work the files already hold is not offered; Discard drops the rest
    let line = studio(&dir, &["open=clay", "recover=1", "input=wait", "rects=1"]);
    assert!(!line.contains("btn:recover.restore"), "the same as on disk: not offered: {line}");
    std::fs::write(kept.join("dirt.png"), std::fs::read(&tex).unwrap()).unwrap();
    studio(&dir, &["open=clay", "recover=1", "input=click @btn:recover.discard"]);
    assert!(!kept.exists(), "Discard clears the kept set");
}

#[test]
fn a_pending_set_goes_with_a_switch_of_face() {
    // review of ADR 0064: a set pending for one face must not stay open (and be used) on another
    let dir = scratch("pending-switch");
    let line = studio(&dir, &["open=clay", "rects=1", "input=click @btn:pattern; click @btn:pat.use; click @btn:variants; click @wall:2; click @facetab:2; wait"]);
    assert!(!line.contains("\"wall:0\"") && !line.contains("btn:wall.use"), "the wall closed with the switch: {line}");
    let line = studio(&dir, &["open=clay", "rects=1", "input=click @btn:pattern; click @facetab:1; wait"]);
    assert!(!line.contains("btn:pat.use"), "the pattern panel closed with the switch: {line}");
}

#[test]
fn a_block_made_cutout_takes_the_eraser_and_undo_takes_the_kind_back() {
    // ADR 0069: Kind writes `see` to the row; the Eraser (offered only then) leaves alpha 0 in the saved file
    let dir = scratch("cutout");
    let blocks = || std::fs::read_to_string(dir.join("data/blocks.toml")).unwrap();
    let clay = |t: &str| t.split("name = \"clay\"").nth(1).unwrap().split("[[block]]").next().unwrap().contains("see = \"cutout\"");
    studio(&dir, &["open=clay", "input=click @btn:pattern; click @btn:pat.use; click @kind:cutout; key X; click @face:0.5,0.5; key Ctrl+S"]);
    assert!(clay(&blocks()), "the row says cutout");
    let (w, _, px) = png(&dir.join("data/textures/clay.png"));
    let at = |x: u32, y: u32| px[((y * w + x) * 4 + 3) as usize];
    assert_eq!((at(8, 8), at(0, 0)), (0, 255), "the erased pixel is empty, the rest there");
    // to opaque: the hole filled with the mean in one step; undo brings the hole and the kind back
    let line = studio(&dir, &["open=clay", "input=click @kind:opaque"]);
    assert!(!clay(&blocks()) && line.contains("\"Kind: opaque\""), "{line}");
    studio(&dir, &["open=clay", "input=click @kind:cutout; click @kind:opaque; key Ctrl+Z"]);
    assert!(clay(&blocks()), "undo writes cutout back");
}

/// Second round step 8, "Done when": from the Textures page a colour is added with the "+" picker and given
/// shades, both are painted with and saved, `check=1` passes; an unused colour is removed there, a used one refused.
#[test]
fn the_palette_grows_from_the_textures_page() {
    let dir = scratch_unpainted("palette-grows", "dirt");
    let line = studio(
        &dir,
        &[
            "open=dirt",
            "size=1920x1080",
            // (the swatches scroll to their end, whatever the palette's size; a toast stands over the inspector's bottom corner for 4 s: `wait 20` lets it go before a click there)
            "input=click @btn:pattern; click @btn:pat.use; wait 20; move @palette.search; wheel -400; wait 20; click @btn:palette.new;              click @palette.hex; key Ctrl+A; text #8a3fa0; wait 20; click @palette.name; key Ctrl+A; text plum; wait 20;              move @palette.search; wheel -400; wait 20; click @btn:palette.add; wait 20; drag @face:0.08,0.09 @face:0.92,0.09;              move @palette.search; wheel -400; wait 20; click @btn:palette.shades; move @palette.search; wheel -400; wait 20; click @btn:palette.use; wait 20;              move @palette.search; wheel -400; wait 20; click @swatch:plum_darker; drag @face:0.08,0.28 @face:0.92,0.28; key Ctrl+S",
        ],
    );
    assert!(line.contains("\"unsaved\":[]"), "{line}");
    let pal = engine::palette::Palette::parse(&std::fs::read_to_string(dir.join("data/palette.toml")).unwrap()).unwrap();
    let run: Vec<&str> = pal.ramp("plum").iter().map(|&i| pal.colours[i].name.as_str()).collect();
    assert_eq!(run, ["plum darker", "plum dark", "plum", "plum light", "plum lighter"], "{line}");
    let rgb = |n: &str| pal.colours.iter().find(|c| c.name == n).unwrap().rgb;
    assert_eq!(rgb("plum"), [0x8a, 0x3f, 0xa0]);
    let (w, _, px) = png(&dir.join("data/textures/dirt.png"));
    let at = |x: u32, y: u32| px[((y * w + x) * 4) as usize..][..3].to_vec();
    assert_eq!(at(w / 2, w * 9 / 100), rgb("plum"), "painted with the new colour");
    assert_eq!(at(w / 2, w * 28 / 100), rgb("plum darker"), "painted with its shade");
    studio(&dir, &["check=1"]);
    // the unused lightest shade goes; plum, painted in dirt.png, stays with the count
    let line = studio(&dir, &["open=dirt", "size=1920x1080", "input=move @palette.search; wheel -400; wait 20; click @swatch:plum_lighter; click @btn:palette.remove; wait 20; click @swatch:plum; click @btn:palette.remove; wait"]);
    let pal = std::fs::read_to_string(dir.join("data/palette.toml")).unwrap();
    assert!(!pal.contains("\"plum lighter\"") && pal.contains("name = \"plum\""), "{line}\n{pal}");
}
