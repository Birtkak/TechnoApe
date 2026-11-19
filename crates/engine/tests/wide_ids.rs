//! Block ids past 255 (12 bits, `blocks::IDS`), in a process of its own: it adds rows to the tables every
//! other test reads (`live::apply`, ADR 0134).
use engine::blocks::{BLOCKS, IDS};
use engine::live::{self, BLOCKS_TOML, ITEMS_TOML};
use engine::mesh::{mesh_cut, quad_id, Cut};
use engine::world::{generate, pidx, Block, Gen, Key, World, CS, P};

/// Rows up there load from data and are what the world holds and generates: a joining block at 300 (its
/// sides on bits 24-27), a cutout at 301 (its twigs and its opening on air on 20-25), a rock near the top whose
/// rubble is 4000; placed, a chunk generates with them and meshes them as themselves.
#[test]
fn blocks_past_255_load_generate_and_mesh_as_themselves() {
    let rows = "\n[[block]]\nid = 300\nname = \"high lamp\"\nhardness = 0.5\nloose = true\njoins = true\nlight = [0.2, 0.4, 1.0]\ncolour = [0.3, 0.5, 0.9]\n\
                [[block]]\nid = 301\nname = \"high leaves\"\nhardness = 0.3\nsee = \"cutout\"\ncolour = [0.2, 0.6, 0.1]\n\
                [[block]]\nid = 4094\nname = \"high rock\"\nhardness = 4\nrubble = 4000\nbedding = 2\njoints = 4\nside = \"high lamp\"\ncolour = [0.7, 0.3, 0.3]\n\
                [block.winter]\ncolour = [0.9, 0.9, 0.9]\n";
    assert_eq!(live::apply(&format!("{BLOCKS_TOML}{rows}"), ITEMS_TOML), Ok(true));
    let (lamp, leaves, rock) = (BLOCKS.id("high lamp"), BLOCKS.id("high leaves"), BLOCKS.id("high rock"));
    assert_eq!((lamp, leaves, rock, BLOCKS.id("high rock rubble")), (300, 301, 4094, 4000));
    assert!(BLOCKS.joins(lamp) && !BLOCKS.opaque(leaves) && BLOCKS.get(rock).unwrap().side == lamp);
    assert!((rock as usize) < IDS);

    let mut w = World::new(256, 32, 1);
    let cell = w.air_over_ground(0, w.bodies[0].spawn);
    // a row of two lamps, a leaf beside them, the rock under the first
    let at = |dx: i32, dz: i32| Block { pos: cell.pos + glam::IVec3::new(dx, 0, dz), ..cell };
    for (b, id) in [(at(0, 1), lamp), (at(1, 1), lamp), (at(2, 1), leaves), (at(0, 0), rock)] {
        w.set_block(b, id);
        assert_eq!(w.block(b), id, "the world holds {id}");
    }
    let k = Key { body: cell.body, face: cell.face, level: 0, pos: at(0, 1).pos.div_euclid(glam::IVec3::splat(CS)) };
    let Gen::Cells(c) = generate(&w.bodies[0], k, &w.edits_for(k)) else { panic!("an edited chunk has cells") };
    let q = |b: Block| { let p = b.pos - (k.pos * CS - 1); pidx(p.x as usize, p.y as usize, p.z as usize) };
    assert_eq!([c[q(at(0, 1))], c[q(at(2, 1))], c[q(at(0, 0))]], [lamp, leaves, rock], "generated with its edits");

    // meshed alone in the air: the lamps join each other along x, the leaf carries its twig and air bits
    let mut cells = Box::new([0u16; P * P * P]);
    cells[pidx(5, 5, 5)] = lamp;
    cells[pidx(6, 5, 5)] = lamp;
    cells[pidx(7, 5, 5)] = leaves;
    cells[pidx(5, 5, 4)] = rock;
    let m = mesh_cut(&cells, 1, Cut::Full, false);
    let ids: std::collections::BTreeSet<u16> = m.quads.iter().map(|q| quad_id(q[1])).collect();
    assert_eq!(ids, [lamp, leaves, rock].into(), "every quad is one of the three");
    // the first lamp's +y face (u = z, v = x; the rock under it at z 3): its neighbour at +x is v+
    let face = |x: u32, dir: u32| *m.quads.iter().find(|q| q[0] & 63 == x && q[0] >> 6 & 63 == 4 + (dir == 2) as u32 && q[0] >> 12 & 63 == 4 && q[0] >> 28 & 7 == dir).unwrap();
    let lamp_top = face(4, 2);
    assert_eq!((quad_id(lamp_top[1]), lamp_top[1] >> 24 & 15), (lamp, 8), "the join to +x survives");
    let leaf_top = face(6, 2);
    assert_eq!((quad_id(leaf_top[1]), leaf_top[1] >> 25 & 1), (leaves, 1), "a cutout face open on air");
    assert_eq!(m.quads.iter().filter(|q| quad_id(q[1]) == leaves).count() as u32, m.cut, "the cutout quads are the leaf's");
}
