//! Live definitions (ADR 0134), in a process of its own: it swaps the tables every other test reads.
use engine::blocks::BLOCKS;
use engine::items::ITEMS;
use engine::live::{self, BLOCKS_TOML, ITEMS_TOML};
use engine::world::World;

fn lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn a_changed_row_is_in_force_at_once_and_a_bad_file_changes_nothing() {
    let (stone, lamp) = (BLOCKS.id("stone"), BLOCKS.id("lamp"));
    assert_eq!(BLOCKS.get(stone).unwrap().hardness, 7.5);
    // a world with a lamp placed before the reload
    let mut w = World::new(256, 32, 1);
    let cell = w.air_over_ground(0, w.bodies[0].spawn);
    w.set_block(cell, lamp);
    assert_eq!(w.lights.get(&cell), Some(&[1.0, 0.72, 0.42]));
    assert_eq!((live::rev(), live::reloaded()), (0, None), "nothing reloaded yet");
    assert_eq!(live::apply(BLOCKS_TOML, ITEMS_TOML), Ok(false), "the files as compiled are no change");

    // read when used: hardness; derived by the world: a placed lamp's light
    let blocks = lf(BLOCKS_TOML).replace("name = \"stone\"\nhardness = 7.5", "name = \"stone\"\nhardness = 2.25").replace("light = [1.0, 0.72, 0.42]", "light = [0.1, 0.2, 0.9]");
    assert_ne!(blocks, lf(BLOCKS_TOML), "the test's swaps found their rows");
    assert_eq!(live::apply(&blocks, ITEMS_TOML), Ok(true));
    assert_eq!(BLOCKS.get(stone).unwrap().hardness, 2.25, "the next read is the new row");
    assert_eq!(ITEMS.name(stone), "stone", "the items are made again over the new blocks");
    assert_eq!(live::rev(), 1);
    // the mesher's own tables are made again: stone seen through is more blocks in its table (the rows made
    // from stone follow it)
    let see = engine::mesh::table_counts()[0];
    assert_eq!(live::apply(&blocks.replace("pattern = \"stone\"", "pattern = \"stone\"\nsee = \"cutout\""), ITEMS_TOML), Ok(true));
    assert!(engine::mesh::table_counts()[0] > see, "the mesher reads the new rows");
    assert_eq!(live::apply(&blocks, ITEMS_TOML), Ok(true));
    assert_eq!((engine::mesh::table_counts()[0], live::rev()), (see, 3));
    assert_eq!(live::reloaded().map(|t| t.0), Some(blocks.clone()), "kept for a client that joins later");
    assert_eq!(w.lights.get(&cell), Some(&[1.0, 0.72, 0.42]), "the world's own tables wait for its refresh");
    let rev = w.lights_rev;
    w.refresh_defs();
    assert_eq!(w.lights.get(&cell), Some(&[0.1, 0.2, 0.9]));
    assert!(w.lights_rev > rev, "the lights' users are told");
    assert_eq!(live::apply(&blocks, ITEMS_TOML), Ok(false), "the same again is no change");
    // a block keeps its name: the code asks for blocks by name, and a world holds them by id
    let e = live::apply(&blocks.replace("name = \"copper\"", "name = \"red metal\""), ITEMS_TOML).unwrap_err();
    assert!(e.contains("'copper') is gone or renamed"), "{e}");

    // refused by name, nothing changed: a file that does not parse, a block gone, an item gone
    let e = live::apply(&blocks.replace("hardness = 2.25", "hardness = soft"), ITEMS_TOML).unwrap_err();
    assert!(e.starts_with("data/blocks.toml:"), "{e}");
    let cut = blocks.find("[[block]]\nid = 4\n").unwrap();
    let gone = format!("{}{}", &blocks[..cut], &blocks[cut + blocks[cut + 1..].find("[[block]]").unwrap() + 1..]);
    let e = live::apply(&gone, ITEMS_TOML).unwrap_err();
    assert!(e.contains("'copper') is gone or renamed"), "{e}");
    let e = live::apply(&blocks, &lf(ITEMS_TOML).replace("name = \"basket\"", "name = \"hamper\"")).unwrap_err();
    assert!(e.starts_with("data/items.toml:") && e.contains("is gone"), "{e}");
    assert_eq!((BLOCKS.get(stone).unwrap().hardness, live::rev(), BLOCKS.get(4).map(|d| d.name.as_str()), ITEMS.named("basket").is_some()), (2.25, 3, Some("copper"), true));

    // a new row is taken
    let more = format!("{blocks}\n[[block]]\nid = 250\nname = \"test brick\"\nhardness = 1.0\ncolour = [0.6, 0.2, 0.2]\n");
    assert_eq!(live::apply(&more, ITEMS_TOML), Ok(true));
    assert_eq!((BLOCKS.id("test brick"), ITEMS.named("test brick"), live::rev()), (250, Some(250), 4));
}
