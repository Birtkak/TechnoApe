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

/// The page's worst case on the frame thread (review 2026-11-18): ten nodes, two 8-octave noises, the first
/// one's slider dragged, then Play; every node's 64 px picture as the page asks for them, and the files
/// column's 16 px picture of a changed map. Run: `cargo test -p engine --profile play --lib
/// node_picture_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn node_picture_cost() {
    let text = "size = 256\nframes = 32\n\
        [[node]]\nid = \"a\"\nkind = \"smooth noise\"\noctaves = 8\nspeed = 1\n\
        [[node]]\nid = \"b\"\nkind = \"smooth noise\"\nseed = 2\noctaves = 8\nspeed = 2\n\
        [[node]]\nid = \"s\"\nkind = \"slope\"\nin = \"a\"\n\
        [[node]]\nid = \"wp\"\nkind = \"warp\"\nin = \"b\"\nby = \"s\"\n\
        [[node]]\nid = \"bl\"\nkind = \"blur\"\nradius = 8\nin = \"wp\"\n\
        [[node]]\nid = \"l\"\nkind = \"levels\"\nin = \"bl\"\n\
        [[node]]\nid = \"c\"\nkind = \"cells\"\nspeed = 1\n\
        [[node]]\nid = \"x\"\nkind = \"blend\"\na = \"l\"\nb = \"c\"\n\
        [[node]]\nid = \"mv\"\nkind = \"move\"\nspeed_x = 1\nin = \"x\"\n\
        [[node]]\nid = \"out\"\nkind = \"out\"\nin = \"mv\"\n";
    let m = map(text).unwrap();
    let mut g = Graph::new(m.clone());
    let all = |g: &mut Graph, f: usize| (0..10).for_each(|i| {
        g.pic(i, f, 64);
    });
    all(&mut g, 0);
    let ms = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1000.0;
    let (mut drag, mut play, mut files) = (0f64, 0f64, 0f64);
    for k in 0..20 {
        let mut next = g.map.clone();
        next.set_num(0, "roughness", 0.3 + k as f32 * 0.02);
        let t = std::time::Instant::now();
        g.update(&next);
        all(&mut g, 0);
        drag = drag.max(ms(t));
        let t = std::time::Instant::now();
        let mut f = Graph::new(next);
        let o = f.out();
        f.pic(o, 0, 16);
        files = files.max(ms(t));
    }
    for f in 1..32 {
        let t = std::time::Instant::now();
        all(&mut g, f);
        play = play.max(ms(t));
    }
    // the page's way since the review: a pass stops starting node pictures after 4 ms (ui/maps.rs NODE_BUDGET),
    // the rest keep theirs till a later pass; the frame scrubbed one step a pass
    let mut g = Graph::new(m);
    all(&mut g, 0);
    let (mut have, mut pass, mut passes) = ([0usize; 10], 0f64, 0);
    for f in 1..32 {
        while have.iter().any(|h| *h != f) {
            let t = std::time::Instant::now();
            for (i, h) in have.iter_mut().enumerate() {
                if *h != f && ms(t) <= 4.0 {
                    g.pic(i, f, 64);
                    *h = f;
                }
            }
            pass = pass.max(ms(t));
            passes += 1;
        }
    }
    println!("{{\"drag_ms_max\":{drag:.2},\"scrub_ms_max\":{play:.2},\"files_thumb_ms_max\":{files:.2},\"budgeted_pass_ms_max\":{pass:.2},\"passes_a_frame\":{:.2}}}", passes as f64 / 31.0);
}

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
        // (repeat 1: a repeated or mirrored move is not the same half a side on, and its wrap was reviewed apart)
        &format!("kind = \"move\"\nshift_x = 0.37\nshift_y = -0.21\nin = \"n\"{NOISE}"),
        &format!("kind = \"warp\"\namount = 0.2\nin = \"n\"\nby = \"s\"{NOISE}\n\n[[node]]\nid = \"s\"\nkind = \"slope\"\nin = \"n\""),
    ] {
        // a map that tiles, worked out starting half a side on, is the picture rolled by half: its edges are
        // then in the middle, so a seam of any size would show
        let mut g = one(node, 1);
        let p = g.pic(g.out(), 0, 64).clone();
        let mut h = one(node, 1);
        h.shift = 0.5;
        let q = h.pic(h.out(), 0, 64);
        let rolled: Pic = (0..64 * 64).map(|k| p[((k / 64 + 32) % 64) * 64 + (k % 64 + 32) % 64]).collect();
        assert!(diff(&p, &vec![p[0]; 64 * 64]) > 0.0, "{node}: flat");
        assert!(diff(q, &rolled) < 1e-3, "{node}: half a side on is not the roll by half ({})", diff(q, &rolled));
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
        (format!("[[node]]\nid = \"n\"\nkind = \"value noise\"\nat = [nan, inf]\n{}", out("n")), "node 'n': at is not [x, y]"),
        (format!("{noise}{}{}", out("n"), out("n").replace("\"out\"\nkind", "\"out2\"\nkind")), "nodes 'out' and 'out2' are both an out"),
    ] {
        let e = map(&text).unwrap_err();
        assert!(e.starts_with("t.toml: ") && e.contains(says), "{e}\nwanted: {says}");
    }
}

#[test]
fn curl_has_no_sinks() {
    let div = |kind: &str| {
        // at the slider's top: the push held to length 1 must not make sinks
        let mut g = one(&format!("kind = \"{kind}\"\nstrength = 10\nin = \"n\"{NOISE}"), 1);
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
    assert!(len > 0.1 && len < 1.0 + 1e-4, "a field that pushes, no push over 1: {len}");
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
fn edits_keep_the_map_valid() {
    let mut m = starter("clouds");
    let before = m.clone();
    let id = |m: &Map, s: &str| m.find(s).unwrap();
    let inv = m.add("invert", [0.0, 300.0]);
    assert_eq!(m.nodes[inv].id, "invert");
    m.wire(id(&m, "levels"), inv, 0).unwrap();
    m.wire(inv, id(&m, "out"), 0).unwrap();
    let wired = m.clone();
    let e = m.wire(inv, id(&m, "levels"), 0).unwrap_err();
    assert!(e.contains("circle"), "{e}");
    assert_eq!(m, wired, "a refused wire leaves the map as it was");
    let len = m.add("length", [0.0, 400.0]);
    assert!(m.wire(inv, len, 0).unwrap_err().contains("takes a direction"));
    // a direction through a blur into length; the curl taken away, the blur gives a number and length is unplugged
    let curl = m.add("curl", [0.0, 500.0]);
    let blur = m.add("blur", [0.0, 600.0]);
    m.wire(id(&m, "noise"), curl, 0).unwrap();
    m.wire(curl, blur, 0).unwrap();
    m.wire(blur, len, 0).unwrap();
    m.remove(&[curl]);
    assert!(m.find("curl").is_none());
    let (blur, len) = (id(&m, "blur"), id(&m, "length"));
    assert!(matches!(m.nodes[blur].pins[0], Pin::Value(_)) && matches!(m.nodes[len].pins[0], Pin::Value(_)), "{}", m.write());
    Graph::new(m.clone());
    assert_eq!(map(&m.write()).unwrap(), m, "indexes put right: the file reads back the same");
    let n = m.nodes.len();
    m.remove(&[id(&m, "out")]);
    assert_eq!(m.nodes.len(), n, "the out stays");
    let copies = m.copy(&[id(&m, "levels"), id(&m, "invert")]);
    let (l2, i2) = (copies[0], copies[1]);
    assert_eq!((m.nodes[l2].id.as_str(), m.nodes[i2].id.as_str()), ("levels 2", "invert 2"));
    assert_eq!(m.nodes[l2].pins[0], Pin::Wire(id(&m, "noise")), "a copy keeps what fed it");
    assert_eq!(m.nodes[i2].pins[0], Pin::Wire(l2), "and the wires among the copies");
    m.set_num(l2, "low", 7.0);
    assert_eq!(m.nodes[l2].nums[0], 1.0, "held to its range");
    let c = m.add("colours", [0.0, 0.0]);
    let ramp = pal().ramps()[1].clone();
    m.set_word(c, "ramp", &ramp.0, &pal()).unwrap();
    assert!(m.set_word(c, "ramp", "no such", &pal()).is_err());
    assert_eq!(m.rgb[c].len(), ramp.1.len());
    assert_eq!(map(&m.write()).unwrap(), m);
    assert_ne!(m, before);
}

#[test]
fn an_update_reworks_only_what_changed() {
    let m = starter("clouds");
    let mut g = Graph::new(m.clone());
    let o = g.out();
    g.pic(o, 0, 32);
    assert_eq!(g.worked, 3);
    let mut moved = m.clone();
    moved.nodes[0].at = [50.0, 50.0];
    g.update(&moved);
    g.pic(o, 0, 32);
    assert_eq!(g.worked, 3, "a node moved is no change to the pictures");
    let mut low = moved.clone();
    low.set_num(low.find("levels").unwrap(), "low", 0.2);
    g.update(&low);
    g.pic(o, 0, 32);
    assert_eq!(g.worked, 5, "levels and out again");
    let mut more = low.clone();
    more.add("invert", [0.0, 0.0]);
    g.update(&more);
    g.pic(o, 0, 32);
    assert_eq!(g.worked, 8, "a new node: worked out anew");
}
