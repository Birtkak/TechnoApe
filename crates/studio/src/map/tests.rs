use super::*;
use engine::map::Data::*;

fn pal() -> Palette {
    Palette::parse(&std::fs::read_to_string("../../data/palette.toml").unwrap()).unwrap()
}
fn map(text: &str) -> Result<Map, String> {
    parse("t.toml", text, &pal())
}
/// A map of node `g` (its kind, numbers and pins; more nodes may follow) wired into the out.
fn one(node: &str, frames: usize) -> Graph {
    let text = format!("size = 64\nframes = {frames}\n\n[[node]]\nid = \"out\"\nkind = \"out\"\nin = \"g\"\n\n[[node]]\nid = \"g\"\n{node}\n");
    Graph::new(map(&text).unwrap_or_else(|e| panic!("{e}")))
}
fn starter(name: &str) -> Map {
    let file = format!("../../data/maps/{name}.toml");
    parse(&file, &std::fs::read_to_string(&file).unwrap(), &pal()).unwrap()
}

#[test]
fn the_starter_maps_pass_the_check() {
    let (ok, bad) = check_dir(Path::new("../../data/maps"), &pal());
    assert!(bad.is_empty(), "{bad:?}");
    assert!(ok >= 3);
    assert_eq!(Graph::new(starter("clouds")).kinds[starter("clouds").find("out").unwrap()], Data::Num);
    assert_eq!(starter("clouds").frames, 1);
    for name in ["wind gusts", "smoke swirl"] {
        let m = starter(name);
        let o = m.find("out").unwrap();
        assert!(m.frames > 1, "{name} moves");
        assert_eq!(Graph::new(m).kinds[o], Data::Dir, "{name} is a direction");
    }
    assert!(starter("smoke swirl").nodes.iter().any(|n| n.spec().kind == "curl"));
}

#[test]
fn export_writes_a_strip_in_each_kinds_encoding() {
    // a number as grey, a colour as it is, a direction in red and green with the middle as no push
    let (num, col, dir) = (vec![[0.5, 0.5, 0.5]], vec![[1.0, 0.25, 0.0]], vec![[0.0, 0.0, 0.0], [1.0, -1.0, 0.0]]);
    assert_eq!(rgba(&num, Num), [128, 128, 128, 255]);
    assert_eq!(rgba(&col, Col), [255, 64, 0, 255]);
    assert_eq!(rgba(&dir, Dir), [128, 128, 0, 255, 255, 0, 0, 255]);
    // an animated map: its frames side by side at its size
    let mut g = one("kind = \"smooth noise\"\nscale = 2\nspeed = 1", 3);
    let (w, h, px) = strip(&mut g, &[0, 1, 2]);
    assert_eq!((w, h, px.len()), (3 * 64, 64, 3 * 64 * 64 * 4));
    let f1 = rgba(g.pic(g.out(), 1, 64), Num);
    assert_eq!(&px[64 * 4..2 * 64 * 4], &f1[..64 * 4], "frame 1 is the second square of the first row");
    let out = std::env::temp_dir().join(format!("technoape-export-{}.png", std::process::id()));
    let out = out.to_string_lossy().into_owned();
    assert_eq!(export(&g.map, &out), Ok((192, 64)));
    let _ = std::fs::remove_file(&out);
    let e = export(&g.map, "Cargo.toml/x.png").unwrap_err();
    assert!(e.contains("Cargo.toml"), "a path that cannot be written is said: {e}");
}
