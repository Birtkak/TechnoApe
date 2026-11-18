use super::*;

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
fn diff(a: &Pic, b: &Pic) -> f32 {
    a.iter().zip(b).flat_map(|(a, b)| (0..3).map(move |c| (a[c] - b[c]).abs())).fold(0.0, f32::max)
}
const NOISE: &str = "\n\n[[node]]\nid = \"n\"\nkind = \"smooth noise\"\nscale = 3\noctaves = 3";

#[test]
fn every_generator_tiles() {
    for node in [
        "kind = \"value noise\"\nscale = 4\noctaves = 3",
        "kind = \"smooth noise\"\nscale = 3\noctaves = 4",
        "kind = \"cells\"\nscale = 5\nmode = \"distance\"",
        "kind = \"cells\"\nscale = 5\nmode = \"edges\"",
        "kind = \"cells\"\nscale = 5\nmode = \"cell\"",
        "kind = \"speckle\"\nscale = 16",
        "kind = \"gradient\"\nshape = \"across\"",
        "kind = \"gradient\"\nshape = \"down\"\nrepeat = 3",
        "kind = \"gradient\"\nshape = \"round\"\nrepeat = 2",
        "kind = \"shape\"\nshape = \"disc\"\nrepeat = 3\nsoft = 0.4",
        "kind = \"shape\"\nshape = \"square\"\nrepeat = 3\nsoft = 0.4",
        "kind = \"shape\"\nshape = \"bricks\"\nrepeat = 3\nsoft = 0.4",
        "kind = \"shape\"\nshape = \"checker\"\nrepeat = 3",
        "kind = \"shape\"\nshape = \"stripes\"\nrepeat = 3\nsoft = 0.4",
        &format!("kind = \"blur\"\nradius = 6\nin = \"n\"{NOISE}"),
        &format!("kind = \"move\"\nshift_x = 0.37\nshift_y = -0.21\nrepeat = 3\nmirror = 1\nin = \"n\"{NOISE}"),
        &format!("kind = \"warp\"\namount = 0.2\nin = \"n\"\nby = \"s\"{NOISE}\n\n[[node]]\nid = \"s\"\nkind = \"slope\"\nin = \"n\""),
    ] {
        let mut g = one(node, 1);
        let p = g.pic(g.out(), 0, 64);
        let at = |x: usize, y: usize| p[y * 64 + x];
        let step = |a: [f32; 3], b: [f32; 3]| (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max);
        let (mut inside, mut across) = (0f32, 0f32);
        for i in 0..64 {
            for j in 0..63 {
                inside = inside.max(step(at(j, i), at(j + 1, i))).max(step(at(i, j), at(i, j + 1)));
            }
            across = across.max(step(at(63, i), at(0, i))).max(step(at(i, 63), at(i, 0)));
        }
        assert!(inside > 0.0, "{node}: flat");
        assert!(across <= inside + 1e-6, "{node}: a seam of {across} where no step inside is over {inside}");
    }
}

#[test]
fn an_animated_map_loops() {
    for node in [
        "kind = \"value noise\"\nscale = 4\nspeed = 1",
        "kind = \"smooth noise\"\nscale = 3\noctaves = 2\nspeed = 2",
        "kind = \"cells\"\nscale = 4\nspeed = -1",
        "kind = \"speckle\"\nscale = 16\nspeed = 16",
        &format!("kind = \"move\"\nspeed_x = 1\nspeed_y = -2\nin = \"n\"{NOISE}"),
    ] {
        let mut g = one(node, 8);
        let o = g.out();
        let first = g.pic(o, 0, 32).clone();
        assert!(diff(&first, g.pic(o, 8, 32)) < 1e-4, "{node}: the frame after the last is not the first");
        for k in 0..8 {
            let a = g.pic(o, k, 32).clone();
            assert!(diff(&a, g.pic(o, k + 1, 32)) > 1e-3, "{node}: frames {k} and {} are the same", k + 1);
        }
    }
}

#[test]
fn a_seed_gives_the_same_pixels() {
    let clouds = starter("clouds");
    let pixels = |m: &Map| {
        let mut g = Graph::new(m.clone());
        g.pic(g.out(), 0, 64).clone()
    };
    assert_eq!(pixels(&clouds), pixels(&clouds));
    let mut other = clouds.clone();
    let n = other.nodes.iter().position(|n| n.spec().nums.iter().any(|s| s.name == "seed")).unwrap();
    let mut g = Graph::new(other.clone());
    g.set(n, "seed", 77.0).unwrap();
    other = g.map.clone();
    assert!(diff(&pixels(&clouds), &pixels(&other)) > 0.1, "another seed, other pixels");
}

#[test]
fn a_file_round_trips() {
    // every kind once, wired, with a pin left at its number
    let ramp = pal().ramps()[0].0.clone();
    let mut t = String::from("size = 32\nframes = 4\nseconds = 2.5\n");
    for s in SPECS {
        t += &format!("\n[[node]]\nid = \"{0}\"\nkind = \"{0}\"\nat = [1.5, -20]\n", s.kind);
        for (p, k, _) in s.pins {
            t += &match (*p, *k) {
                ("mask", _) => "mask = 0.25\n".into(),
                (_, Data::Dir) => format!("{p} = \"curl\"\n"),
                _ => format!("{p} = \"value noise\"\n"),
            };
        }
    }
    let t = t.replace("kind = \"colours\"\n", &format!("kind = \"colours\"\nramp = \"{ramp}\"\n")).replace("kind = \"blend\"\n", "kind = \"blend\"\nmode = \"screen\"\n");
    let all = map(&t).unwrap_or_else(|e| panic!("{e}"));
    let mut maps = vec![all.clone(), starter("clouds"), starter("wind gusts"), starter("smoke swirl")];
    let mut chosen = all.clone();
    chosen.nodes.iter_mut().find(|n| n.id == "colours").unwrap().words[0].clear();
    chosen.nodes.iter_mut().find(|n| n.id == "colours").unwrap().colours = pal().colours[..3].iter().map(|c| c.name.clone()).collect();
    maps.push(map(&chosen.write()).unwrap_or_else(|e| panic!("{e}")));
    for m in maps {
        let text = m.write();
        let back = map(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(back, m, "{text}");
        assert_eq!(back.write(), text);
    }
    // and every kind works out at a small size and an odd frame
    let mut g = Graph::new(all);
    for i in 0..g.map.nodes.len() {
        assert_eq!(g.pic(i, 3, 16).len(), 256);
    }
}

#[test]
fn each_refusal_names_its_node() {
    let noise = "[[node]]\nid = \"n\"\nkind = \"value noise\"\n";
    let out = |src: &str| format!("[[node]]\nid = \"out\"\nkind = \"out\"\nin = \"{src}\"\n");
    for (text, says) in [
        (format!("[[node]]\nid = \"odd\"\nkind = \"sparkle\"\n{}", out("odd")), "node 'odd': unknown kind 'sparkle'"),
        (format!("[[node]]\nid = \"a\"\nkind = \"invert\"\nin = \"b\"\n[[node]]\nid = \"b\"\nkind = \"invert\"\nin = \"a\"\n{}", out("a")), "node 'a': its wires make a circle"),
        (format!("{noise}[[node]]\nid = \"len\"\nkind = \"length\"\nin = \"n\"\n{}", out("len")), "node 'len': pin 'in' takes a direction, 'n' gives a number"),
        (format!("{noise}[[node]]\nid = \"w\"\nkind = \"warp\"\nin = \"n\"\nby = \"n\"\n{}", out("w")), "node 'w': pin 'by' takes a direction, 'n' gives a number"),
        (format!("[[node]]\nid = \"big\"\nkind = \"value noise\"\nscale = 100\n{}", out("big")), "node 'big': scale 100 is out of 1..64"),
        (format!("[[node]]\nid = \"half\"\nkind = \"value noise\"\nscale = 2.5\n{}", out("half")), "node 'half': scale 2.5 is not a whole number"),
        (format!("[[node]]\nid = \"n\"\nkind = \"value noise\"\nscael = 3\n{}", out("n")), "node 'n': unknown key 'scael'"),
        (format!("[[node]]\nid = \"c\"\nkind = \"colours\"\nramp = \"no such\"\n{}", out("c")), "node 'c': ramp 'no such' is not in the palette"),
        (out("ghost"), "node 'out': pin 'in' names 'ghost', which is no node"),
        (noise.to_string(), "no node is the out"),
        (format!("{noise}{}{}", out("n"), out("n").replace("\"out\"\nkind", "\"out2\"\nkind")), "nodes 'out' and 'out2' are both an out"),
    ] {
        let e = map(&text).unwrap_err();
        assert!(e.starts_with("t.toml: ") && e.contains(says), "{e}\nwanted: {says}");
    }
}

#[test]
fn curl_has_no_sinks() {
    let div = |kind: &str| {
        let mut g = one(&format!("kind = \"{kind}\"\nstrength = 1\nin = \"n\"{NOISE}"), 1);
        let p = g.pic(g.out(), 0, 64);
        let at = |x: usize, y: usize| p[(y % 64) * 64 + x % 64];
        let (mut d, mut len) = (0f32, 0f32);
        for y in 0..64 {
            for x in 0..64 {
                d = d.max((at(x + 1, y)[0] - at(x + 63, y)[0] + at(x, y + 1)[1] - at(x, y + 63)[1]).abs());
                len = len.max(at(x, y)[0].hypot(at(x, y)[1]));
            }
        }
        (d, len)
    };
    let (curl, len) = div("curl");
    assert!(len > 0.1 && len < 1.0, "a field that pushes, unclamped: {len}");
    assert!(curl < 1e-5, "curl's divergence {curl}");
    assert!(div("slope").0 > 0.05, "slope's field has sinks: the check can see them");
}

#[test]
fn only_what_is_downstream_is_reworked() {
    let text = ["a", "b"].map(|id| format!("[[node]]\nid = \"{id}\"\nkind = \"value noise\"\n")).concat()
        + "[[node]]\nid = \"inv\"\nkind = \"invert\"\nin = \"a\"\n[[node]]\nid = \"mix\"\nkind = \"blend\"\na = \"inv\"\nb = \"b\"\n[[node]]\nid = \"out\"\nkind = \"out\"\nin = \"mix\"\n";
    let mut g = Graph::new(map(&format!("size = 32\n{text}")).unwrap());
    let o = g.out();
    let first = g.pic(o, 0, 32).clone();
    assert_eq!(g.worked, 5);
    g.pic(o, 0, 32);
    assert_eq!(g.worked, 5, "kept");
    let b = g.map.find("b").unwrap();
    g.set(b, "scale", 7.0).unwrap();
    assert!(diff(&first, g.pic(o, 0, 32)) > 0.01);
    assert_eq!(g.worked, 8, "b, mix and out again; a and inv kept");
    assert!(g.set(b, "nothing", 1.0).is_err());
    assert_eq!(g.pic(o, 0, 16).len(), 256, "a smaller preview");
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
