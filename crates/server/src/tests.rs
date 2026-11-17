use super::*;
use engine::net::local;
use std::sync::mpsc::channel;

fn server_with(n: usize) -> (Server, Vec<Link<ToServer, ToClient>>) {
    let (tx, rx) = channel();
    let mut s = Server::new(256, 32, 1, rx);
    let links = (0..n)
        .map(|_| {
            let (c, sv) = local();
            tx.send(sv).unwrap();
            c
        })
        .collect();
    s.tick();
    (s, links)
}

/// Each item and how many of it the slots hold, by id (an ape's grid cells not counted, as `items::count`).
fn held(slots: &[Slot]) -> Vec<(u16, u32)> {
    let mut ids: Vec<u16> = slots.iter().take(items::APE_SLOTS).flatten().map(|s| s.item).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter().map(|m| (m, items::count(slots, m))).collect()
}

/// What every `Inventory` among `got` held (`held`).
fn invs(got: &[ToClient]) -> Vec<Vec<(u16, u32)>> {
    got.iter().filter_map(|m| if let ToClient::Inventory { slots, .. } = m { Some(held(slots)) } else { None }).collect()
}

fn drain(l: &Link<ToServer, ToClient>) -> Vec<ToClient> {
    let mut v = Vec::new();
    while let Recv::Msg(m) = l.try_recv() {
        v.push(m);
    }
    v
}

/// The live air (W3): a server on a baked map runs it, sends each player the air where it stands every
/// `AIR_EVERY` ticks, and a save carries it on to the same weather as the server that kept running.
#[test]
fn a_player_is_sent_the_live_air_and_a_save_carries_it_on() {
    let (tx, rx) = channel();
    let mut s = Server::baked(256, 128, 5, rx);
    let (c, sv) = local();
    tx.send(sv).unwrap();
    for _ in 0..engine::net::AIR_EVERY * 3 {
        s.tick();
    }
    let airs: Vec<engine::weather::Here> = drain(&c).into_iter().filter_map(|m| if let ToClient::Air(h) = m { Some(h) } else { None }).collect();
    assert_eq!(airs.len(), 3, "one at ticks 0, 40 and 80");
    assert!((-40.0..50.0).contains(&airs[0].temp) && (0.0..=1.0).contains(&airs[0].cover), "{:?}", airs[0]);
    let save = Save::from_bytes(&s.snapshot().to_bytes()).unwrap();
    assert!(save.air.is_some());
    let mut back = Server::from_save(save, channel().1);
    for _ in 0..200 {
        s.tick();
        back.tick();
    }
    let view = |s: &Server| s.weather.as_ref().map(|w| (w.view().t, w.view().hash));
    assert_eq!(view(&back), view(&s));
    assert_eq!(view(&s).unwrap().0, 10.0, "the step to 10 s handed over at tick 200 and shown at 220");
    // the air at the player's height over the sea in metres, not over the sea's layer number (ADR 0072):
    // the one sent at tick 320 against where the player stands then
    s.tick();
    let sent = drain(&c).into_iter().filter_map(|m| if let ToClient::Air(h) = m { Some(h) } else { None }).last().unwrap();
    let b = &s.world.bodies[0];
    let pos = s.clients[0].phys.pos;
    let want = s.weather.as_ref().unwrap().here((pos - b.centre).normalize(), b.over_sea(pos).unwrap());
    assert!((sent.temp - want.temp).abs() < 0.05, "sent {} C, the air {} m over the sea is {} C", sent.temp, b.over_sea(pos).unwrap(), want.temp);
}

/// The live rain feeds the rivers (W5, ADR 0059): on a planet with rivers the air hands its runoff to the
/// reaches, and the air's ledger plus the water's stays whole every tick.
#[test]
fn the_live_rain_feeds_the_rivers_and_both_ledgers_hold() {
    let mut s = Server::baked(1536, 32, 7, channel().1);
    for _ in 0..400 {
        s.tick();
        assert_eq!((s.air_ledger(), s.water_ledger()), (Some(0), 0), "tick {}", s.tick);
    }
    assert!(s.world.water.rained > 0, "no rain reached a river");
}

/// A save mid-rain carries on (W5): saved while an air job is out and rain is in the reaches, the loaded
/// server hands over that job's runoff when the running one does, and its water and air stay the same.
#[test]
fn a_save_mid_rain_carries_on_to_the_same_water() {
    let mut s = Server::baked(1536, 32, 7, channel().1);
    while s.world.water.rain_held.is_empty() || s.tick % 200 != 10 {
        s.tick();
        assert!(s.tick < 3000, "no rain in the reaches");
    }
    let mut back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    let state = |s: &Server| {
        let w = &s.world.water;
        (postcard::to_allocvec(&(w.all(), w.sea_extra, &w.reach_extra, &w.rain_held, w.rained)).unwrap(), s.weather.as_ref().map(|w| (w.view().hash, w.ledger())))
    };
    for t in 0..400 {
        s.tick();
        back.tick();
        if t >= 12 && state(&back) != state(&s) {
            let (a, b) = (state(&back), state(&s));
            let (wa, wb) = (&back.world.water, &s.world.water);
            panic!("{t} ticks after the save (tick {} / {}): water {} air {:?} / {:?}; rained {} / {}, held {} / {}, extra {} / {}, sea {} / {}, stored {} / {}", back.tick, s.tick, a.0 == b.0, a.1, b.1, wa.rained, wb.rained, wa.rain_held.values().sum::<i64>(), wb.rain_held.values().sum::<i64>(), wa.reach_extra.values().sum::<i64>(), wb.reach_extra.values().sum::<i64>(), wa.sea_extra, wb.sea_extra, wa.stored(), wb.stored());
        }
    }
    assert_eq!((back.air_ledger(), back.water_ledger()), (Some(0), 0));
}

#[test]
fn edit_reaches_the_other_player_and_a_late_joiner() {
    let (mut s, links) = server_with(2);
    for l in &links {
        assert!(matches!(drain(l)[0], ToClient::Welcome { seed: 1, planet_n: 256, .. }));
    }
    // stand on the +z pole and toggle the block under the feet
    let (pole, under) = on_land(&s);
    let stone = BLOCKS.id("stone");
    let m = if s.world.block(under) == stone { AIR } else { stone };
    links[0].send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    links[0].send(ToServer::Creative(true));
    links[0].send(ToServer::Edit { seq: 1, shape: Shape::one(under), material: m });
    s.tick();
    assert!(drain(&links[0]).contains(&ToClient::Done { seq: 1, ok: true }));
    assert_eq!(s.world.block(under), m);
    assert!(drain(&links[1]).contains(&ToClient::Edits(vec![(under, m)])));
    // a third player joining later gets the edit in the initial sync
    let (tx, rx) = channel();
    let mut s2 = Server { accept: rx, ..s };
    let (c, sv) = local();
    tx.send(sv).unwrap();
    s2.tick();
    assert!(drain(&c).contains(&ToClient::Edits(vec![(under, m)])));
}

/// A spot on dry land away from the spawn (out of a new player's reach), 2 m over the middle of a column,
/// and the air cell on the ground under it (placing fills air only).
/// Animals (ADR 0083): a real one spawned by command reaches the player near it as a delta, goes when the
/// player leaves, is kept beside the save, and a recording from that save replays with it.
#[test]
fn animals_reach_the_player_near_them_and_keep_beside_the_save() {
    let (mut s, links) = server_with(1);
    let (pole, _) = on_land(&s);
    links[0].send(ToServer::Hello { name: "a".into() });
    links[0].send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    links[0].send(ToServer::Chat("/spawn deer live".into()));
    links[0].send(ToServer::Chat("/spawn unicorn live".into()));
    for _ in 0..10 {
        s.tick();
    }
    let got = drain(&links[0]);
    let said: Vec<&String> = got.iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
    assert!(said.iter().any(|t| t.starts_with("a deer")) && said.iter().any(|t| t.contains("no creature 'unicorn'")), "{said:?}");
    let kinds = got.iter().find_map(|m| if let ToClient::Kinds(k) = m { Some(k.clone()) } else { None }).expect("kinds on join");
    let seen: Vec<engine::net::Animal> = got.iter().filter_map(|m| if let ToClient::Animals { seen, .. } = m { Some(seen.clone()) } else { None }).flatten().collect();
    let deer = seen.iter().find(|a| kinds[a.kind as usize] == "deer" && (DVec3::from_array(a.pos) - pole).length() < 6.0).expect("the spawned deer is seen");
    // only deltas: a standing deer is not sent again every tick
    assert!(seen.iter().filter(|a| a.id == deer.id).count() < 10);
    // under /timewarp the herds' clock still counts ticks: their looks ahead and minds keep their pace
    let c0 = s.herds.clock;
    links[0].send(ToServer::Chat("/timewarp 100".into()));
    s.tick();
    s.tick();
    assert_eq!(s.herds.clock, c0 + 2);
    let far = s.world.bodies[0].centre - (pole - s.world.bodies[0].centre);
    links[0].send(ToServer::Move { pos: far.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    s.tick();
    assert!(drain(&links[0]).iter().any(|m| matches!(m, ToClient::Animals { gone, .. } if gone.contains(&deer.id))));
    // kept beside the save, and replayed from it
    let dir = std::env::temp_dir().join(format!("technoape-herds-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("w.sav");
    s.autosave = Some((path.clone(), u64::MAX));
    s.save_now();
    let (tx, rx) = channel();
    let mut back = Server::from_save(Save::read(&path).unwrap(), rx);
    back.read_herds(&path);
    assert_eq!(back.herds.to_bytes(), s.herds.to_bytes());
    assert!(back.herds.all().any(|a| a.id == deer.id));
    let rec = dir.join("w.rec");
    back.record(&rec).unwrap();
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Hello { name: "a".into() });
    for _ in 0..60 {
        back.tick();
        drain(&c);
    }
    let r = replay(&rec, Some(60)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!((r.diverged, r.checks), (None, 2));
    assert_eq!(r.server.herds.to_bytes(), back.herds.to_bytes());
}

fn on_land(s: &Server) -> (DVec3, Block) {
    let b = &s.world.bodies[0];
    let sea = b.layer_r(b.sea.unwrap());
    let d = (10..400).map(|t| (b.spawn + DVec3::X * 0.02 * t as f64).normalize()).find(|d| b.height(*d) > sea + 1.0).unwrap();
    let k = s.world.block_at(b.centre + d * b.radius);
    let d = b.dir(k.face as usize, k.pos.x as f64 + 0.5, k.pos.y as f64 + 0.5);
    let h = b.height(d);
    (b.centre + d * (h + 2.0), s.world.block_at(b.centre + d * (h + 0.6)))
}

/// Sand on a shelf in the air, the shelf dug out from under it: it falls to the ground, and the other
/// player is told each move as two edits (ADR 0079).
#[test]
fn sand_dug_from_under_falls_and_every_player_sees_it() {
    let (mut s, links) = server_with(2);
    let (pole, under) = on_land(&s);
    let shelf = Block { pos: under.pos + glam::IVec3::new(3, 0, 5), ..under };
    let over = Block { pos: shelf.pos + glam::IVec3::Z, ..shelf };
    let (stone, sand) = (BLOCKS.id("stone"), BLOCKS.id("sand"));
    s.world.set_block(shelf, stone);
    s.world.set_block(over, sand);
    links[0].send(ToServer::Creative(true));
    links[0].send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    drain(&links[1]);
    links[0].send(ToServer::Edit { seq: 1, shape: Shape::one(shelf), material: AIR });
    let mut got = Vec::new();
    for _ in 0..80 {
        s.tick();
        got.extend(drain(&links[1]));
    }
    assert_eq!(s.world.block(over), AIR, "it fell");
    // where it lies: the one edit that made sand (it may have slid down the slope it landed on)
    let lands: Vec<Block> = s.world.edits.iter().flat_map(|(k, c)| c.iter().map(move |(p, m)| (Block { body: k.body, face: k.face, pos: *p }, *m))).filter(|(b, m)| *m == sand && s.world.generated_block(*b) != sand).map(|(b, _)| b).collect();
    assert_eq!(lands.len(), 1, "one sand: {lands:?}");
    let rest = lands[0];
    assert!(rest.pos.z < shelf.pos.z && (rest.pos - shelf.pos).abs().max_element() < 8, "under the shelf: {rest:?}");
    assert!(BLOCKS.solid(s.world.block(Block { pos: rest.pos - glam::IVec3::Z, ..rest })), "on the ground");
    assert_eq!(s.falls.awake(), 0, "and asleep");
    let edits: Vec<(Block, u16)> = got.into_iter().filter_map(|m| if let ToClient::Edits(v) = m { Some(v) } else { None }).flatten().collect();
    assert!(edits.contains(&(over, AIR)) && edits.contains(&(rest, sand)), "{edits:?}");
}

#[test]
fn bad_edits_are_dropped() {
    let (mut s, links) = server_with(1);
    drain(&links[0]);
    let (pole, under) = on_land(&s);
    let before = s.world.block(under);
    let one = Shape::one(under);
    links[0].send(ToServer::Edit { seq: 1, shape: one, material: 0 }); // player still at the origin: out of reach
    links[0].send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    links[0].send(ToServer::Edit { seq: 2, shape: one, material: 200 }); // unknown material
    links[0].send(ToServer::Edit { seq: 3, shape: Shape { body: 9, ..one }, material: 0 }); // no such body
    let far = Shape { max: one.min + glam::IVec3::new(0, 0, 40), ..one };
    links[0].send(ToServer::Edit { seq: 4, shape: far, material: 0 }); // too long, and reaching the sky
    let wide = Shape { min: one.min - glam::IVec3::new(31, 31, 0), ..one };
    links[0].send(ToServer::Edit { seq: 5, shape: wide, material: 0 }); // in size, but 22 m away at one corner
    links[0].send(ToServer::Edit { seq: 6, shape: one, material: BLOCKS.id("stone") }); // survival, nothing held
    s.tick();
    assert_eq!(s.world.block(under), before);
    assert_eq!(s.world.edit_count, 0);
    let got = drain(&links[0]);
    assert!(!got.iter().any(|m| matches!(m, ToClient::Edits(_))));
    for seq in 1..=6 {
        assert!(got.contains(&ToClient::Done { seq, ok: false }), "edit {seq} refused: {:?}", got.iter().filter(|m| matches!(m, ToClient::Done { .. })).collect::<Vec<_>>());
    }
}

#[test]
fn survival_digs_into_the_inventory_and_pays_for_what_it_places() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let ground = s.world.bodies[0].centre + DVec3::Z * s.world.bodies[0].height(DVec3::Z);
    let pole = ground + DVec3::Z;
    let deep = s.world.block_at(ground - DVec3::Z * 6.0); // below the dirt: stone
    let stone = BLOCKS.id("stone");
    assert_eq!(s.world.block(deep), stone);
    let brush = Shape { max: deep.pos + 1, ..Shape::one(deep) };
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    // held for the stone's time: dug into rubble (ADR 0093), and the rubble dug: bare hands take it, never the
    // stone itself (ADR 0034); stone is given instead
    let rubble = BLOCKS.id("stone rubble");
    for (seq, left) in [(1, rubble), (4, AIR)] {
        l.send(ToServer::Dig(Some(brush)));
        s.tick();
        let t = edit::dig_time(&s.world, &brush);
        wait(&mut s, t);
        l.send(ToServer::Edit { seq, shape: brush, material: AIR });
        s.tick();
        assert!(brush.cells().all(|b| s.world.block(b) == left));
    }
    assert_eq!((items::count(&s.clients[0].slots, stone), items::count(&s.clients[0].slots, rubble)), (0, 8));
    assert!(s.item_ledger().is_empty() && s.piles.is_empty(), "into the slots, not a pile");
    s.clients[0].slots = items::ape();
    s.item_made.clear();
    l.send(ToServer::Chat("/give stone 8".into()));
    s.tick();
    let got = drain(l);
    assert!(invs(&got).contains(&vec![]), "sent on join");
    assert!(invs(&got).contains(&vec![(stone, 8)]));
    // eight dirt are not held, so none of them goes in; the eight stone are
    l.send(ToServer::Edit { seq: 2, shape: brush, material: BLOCKS.id("dirt") });
    l.send(ToServer::Edit { seq: 3, shape: brush, material: stone });
    s.tick();
    let got = drain(l);
    assert!(got.contains(&ToClient::Done { seq: 2, ok: false }) && got.contains(&ToClient::Done { seq: 3, ok: true }));
    assert!(invs(&got).contains(&vec![]));
    assert!(brush.cells().all(|b| s.world.block(b) == stone));
    assert_eq!(s.world.edit_count, 8, "the world is as generated again, but the cells stay recorded");
}

/// ADR 0117: a torch goes onto a floor or beside a wall, never into the open air or into water; a survival
/// player pays one for it, and its wall dug it drops as one torch in a pile, the items' ledger exact.
#[test]
fn a_torch_needs_a_floor_or_a_wall_and_drops_when_it_goes() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let dirt = over_dirt(&mut s, l);
    let (torch, stone) = (BLOCKS.id("torch"), BLOCKS.id("stone"));
    let at = |di: i32, dl: i32| Block { pos: dirt.pos + glam::IVec3::new(di, 0, dl), ..dirt };
    // a column of air two over, a stone floor at its foot and a stone wall beside it higher up
    for dl in 0..=6 {
        for di in 1..=3 {
            s.world.set_block(at(di, dl), AIR);
        }
    }
    s.world.set_block(at(2, 0), stone);
    s.world.set_block(at(3, 3), stone);
    l.send(ToServer::Chat("/give torch 3".into()));
    s.tick();
    drain(l);
    let (floor, wall, open) = (at(2, 1), at(2, 3), at(2, 5));
    for (seq, b) in [(1, floor), (2, wall), (3, open)] {
        l.send(ToServer::Edit { seq, shape: Shape::one(b), material: torch });
    }
    s.tick();
    let got = drain(l);
    for (seq, ok) in [(1, true), (2, true), (3, false)] {
        assert!(got.contains(&ToClient::Done { seq, ok }), "{seq}: {ok}");
    }
    assert_eq!((s.world.block(floor), s.world.block(wall), s.world.block(open)), (torch, torch, AIR));
    assert_eq!(items::count(&s.clients[0].slots, torch), 1, "two paid for, the third kept");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // the wall dug (creative, so it leaves no pile of its own): the torch on it drops as one torch
    l.send(ToServer::Creative(true));
    l.send(ToServer::Edit { seq: 4, shape: Shape::one(at(3, 3)), material: AIR });
    s.tick();
    let got = drain(l);
    assert_eq!(s.world.block(wall), AIR, "nothing holds it");
    assert_eq!(s.world.block(floor), torch, "the floor's stays");
    let pile = got.iter().rev().find_map(|m| if let ToClient::Piles(v) = m { v.iter().find(|p| p.1.iter().flatten().any(|st| st.item == torch)).map(|p| p.1.clone()) } else { None });
    assert_eq!(pile, Some(vec![Some(Stack::new(torch, 1)), None, None, None]), "one torch lies");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// ADR 0117 (review): a torch a falling block lands in, or that water covers, drops as one torch, the ledger
/// exact; a survival player's click takes one at once, as itself.
#[test]
fn a_torch_is_knocked_off_by_sand_and_water_and_taken_by_hand() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let dirt = over_dirt(&mut s, l);
    let (torch, stone, sand) = (BLOCKS.id("torch"), BLOCKS.id("stone"), BLOCKS.id("sand"));
    let at = |di: i32, dl: i32| Block { pos: dirt.pos + glam::IVec3::new(di, 0, dl), ..dirt };
    for dl in 0..=6 {
        for di in 1..=3 {
            s.world.set_block(at(di, dl), AIR);
        }
    }
    s.world.set_block(at(2, 0), stone);
    s.world.set_block(at(3, 0), stone);
    l.send(ToServer::Chat("/give torch 2".into()));
    s.tick();
    l.send(ToServer::Edit { seq: 1, shape: Shape::one(at(2, 1)), material: torch });
    l.send(ToServer::Edit { seq: 2, shape: Shape::one(at(3, 1)), material: torch });
    s.tick();
    assert_eq!((s.world.block(at(2, 1)), s.world.block(at(3, 1))), (torch, torch));
    // sand over the first falls into its cell: the torch is knocked off and lies as one
    s.world.set_block(at(2, 4), sand);
    s.falls.wake(&s.world, at(2, 4));
    for _ in 0..40 {
        s.tick();
    }
    assert_eq!(s.world.block(at(2, 1)), sand);
    let lying = |s: &Server| s.piles.values().flat_map(|p| p.slots.iter().flatten()).filter(|st| st.item == torch).map(|st| st.n as u32).sum::<u32>();
    assert_eq!(lying(&s), 1, "the knocked-off torch lies");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // a survival click on the second takes it at once (no time held), as one torch
    drain(l);
    l.send(ToServer::Edit { seq: 3, shape: Shape::one(at(3, 1)), material: AIR });
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 3, ok: true }));
    assert_eq!(s.world.block(at(3, 1)), AIR);
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // a torch under the sea (put there, as a flood would) pops off within a second
    let b = &s.world.bodies[0];
    let sea = b.layer_r(b.sea.unwrap());
    let d = (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) }).find(|d| b.height(*d) < sea - 4.0).expect("a sea");
    let wet = s.world.block_at(b.centre + d * (sea - 1.5));
    s.world.set_block(Block { pos: wet.pos - glam::IVec3::Z, ..wet }, stone);
    s.world.set_block(wet, torch);
    for _ in 0..TICK_HZ as u32 {
        s.tick();
    }
    assert_eq!(s.world.block(wet), AIR, "out of the water");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// ADR 0117 (review): taking a torch into the hand sends the roster at once, not at the next second.
#[test]
fn a_change_of_hand_is_sent_at_once() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Chat("/give torch 1".into()));
    s.tick();
    let k = s.clients[0].slots.iter().position(|x| x.is_some_and(|st| st.item == BLOCKS.id("torch"))).unwrap();
    // first an empty slot in hand
    let empty = (0..items::HOTBAR).find(|&j| s.clients[0].slots[j].is_none()).unwrap();
    l.send(ToServer::Hand(empty as u8));
    s.tick();
    while s.tick % TICK_HZ as u64 != 1 {
        s.tick();
    }
    drain(l);
    assert!(k < items::HOTBAR, "the torch is on the hotbar");
    l.send(ToServer::Hand(k as u8));
    s.tick();
    let hands: Vec<u16> = drain(l).iter().filter_map(|m| if let ToClient::Players(v) = m { Some(v[0].hand) } else { None }).collect();
    assert_eq!(hands, vec![BLOCKS.id("torch")], "sent in the tick the hand changed");
}

/// ADR 0117: a torch is never placed in water, held up by a floor or not.
#[test]
fn a_torch_is_never_placed_in_water() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let b = &s.world.bodies[0];
    let sea = b.layer_r(b.sea.unwrap());
    // (a spiral of directions over the whole planet, the first over deep sea)
    let d = (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) }).find(|d| b.height(*d) < sea - 4.0).expect("a sea");
    let wet = s.world.block_at(b.centre + d * (sea - 1.5));
    assert!(s.world.water_surface(s.world.block_centre(wet)).is_some(), "the cell is under water");
    s.world.set_block(Block { pos: wet.pos - glam::IVec3::Z, ..wet }, BLOCKS.id("stone"));
    l.send(ToServer::Creative(true));
    l.send(ToServer::Move { pos: (s.world.block_centre(wet) + d * 3.0).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    l.send(ToServer::Edit { seq: 1, shape: Shape::one(wet), material: BLOCKS.id("torch") });
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 1, ok: false }));
    assert_eq!(s.world.block(wet), AIR);
}

/// Ticks for `s` seconds.
fn wait(s: &mut Server, secs: f32) {
    for _ in 0..(secs as f64 * TICK_HZ).round() as u32 {
        s.tick();
    }
}

/// A survival player on dry land over a dirt cell (made dirt), and the cell.
fn over_dirt(s: &mut Server, l: &Link<ToServer, ToClient>) -> Block {
    let (pole, under) = on_land(s);
    let cell = Block { pos: under.pos - glam::IVec3::Z, ..under };
    s.world.set_block(cell, BLOCKS.id("dirt"));
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    drain(l);
    cell
}

/// The piles of `got`'s last `Piles` that names `b`: what it held then.
fn pile_in(got: &[ToClient], b: Block) -> Option<Vec<Slot>> {
    got.iter().rev().find_map(|m| if let ToClient::Piles(v) = m { v.iter().find(|p| p.0 == b).map(|p| p.1.clone()) } else { None })
}

/// A survival player (ADR 0047, 0074): dirt dug falls to the ground as a pile where it was, and walking over
/// it picks it up; a pile left lying falls when its floor is dug. The items' ledger holds throughout.
#[test]
fn a_dug_block_falls_as_a_pile_and_walking_over_it_picks_it_up() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let dirt = over_dirt(&mut s, l);
    // two dirt blocks under the first, so the pile has something to fall through later
    let below = Block { pos: dirt.pos - glam::IVec3::Z, ..dirt };
    s.world.set_block(below, BLOCKS.id("dirt"));
    let one = Shape::one(dirt);
    l.send(ToServer::Dig(Some(one)));
    s.tick();
    let need = edit::dig_time(&s.world, &one);
    wait(&mut s, need * 1.05);
    l.send(ToServer::Edit { seq: 1, shape: one, material: AIR });
    s.tick();
    let got = drain(l);
    let d = BLOCKS.id("dirt");
    assert!(got.contains(&ToClient::Done { seq: 1, ok: true }), "{got:?}");
    assert_eq!(pile_in(&got, dirt), Some(vec![Some(Stack::new(d, 1)), None, None, None]), "the pile lies where the block was");
    assert_eq!((items::count(&s.clients[0].slots, d), s.item_ledger()), (0, vec![]), "2 m up: not picked up yet");
    // the floor under it dug (creative, so no second pile): it falls a block
    l.send(ToServer::Creative(true));
    l.send(ToServer::Edit { seq: 2, shape: Shape::one(below), material: AIR });
    s.tick();
    let got = drain(l);
    assert_eq!((pile_in(&got, dirt), pile_in(&got, below)), (Some(vec![]), Some(vec![Some(Stack::new(d, 1)), None, None, None])));
    // the player comes down onto it: taken
    l.send(ToServer::Move { pos: s.world.block_centre(below).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    let got = drain(l);
    assert_eq!((invs(&got).last().cloned(), pile_in(&got, below)), (Some(vec![(d, 1)]), Some(vec![])), "{got:?}");
    assert!(s.piles.is_empty() && s.item_ledger().is_empty());
}

/// The move message (ADR 0074): within the slots, onto the ground ahead and back, refused for creative's grid in
/// survival, for a pile out of reach and for a pack taken off while its rows hold things.
#[test]
fn moves_follow_the_rule_and_a_refused_one_changes_nothing() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, _) = on_land(&s);
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Chat("/give snow 70".into()));
    s.tick();
    let stone = BLOCKS.id("snow");
    let mv = |seq, from, to, n| ToServer::Items { seq, from, to, n };
    l.send(mv(1, (Holder::Ape, 1), (Holder::Ape, 4), 3));
    l.send(mv(2, (Holder::Ape, 0), (Holder::Grid, 0), 1));
    l.send(mv(3, (Holder::Grid, stone), (Holder::Ape, 5), 1));
    // past every slot (40 was before the crafting grid's cells took 37..45)
    l.send(mv(4, (Holder::Ape, 0), (Holder::Ape, items::SLOTS as u16), 1));
    s.tick();
    let got = drain(l);
    let done: Vec<bool> = got.iter().filter_map(|m| if let ToClient::Done { ok, .. } = m { Some(*ok) } else { None }).collect();
    assert_eq!(done, [true, false, false, false]);
    assert_eq!((s.clients[0].slots[1], s.clients[0].slots[4]), (Some(Stack::new(stone, 3)), Some(Stack::new(stone, 3))));
    // thrown: two metres ahead, not taken back at once, taken once its wait is over and the player is there
    l.send(mv(5, (Holder::Ape, 0), (Holder::Ground, 0), 10));
    s.tick();
    assert_eq!(items::count(&s.clients[0].slots, stone), 60);
    let (k, p) = s.piles.iter().next().map(|(k, p)| (*k, p.clone())).expect("a pile");
    let at = s.world.block_centre(unkey(k));
    assert!((at.distance(pole) - 2.0).abs() < 2.0 && p.slots[0] == Some(Stack::new(stone, 10)), "{at} from {pole}");
    assert!(s.item_ledger().is_empty());
    // out of reach of a pile 4 m off: refused
    l.send(ToServer::Move { pos: (at + (at - s.world.bodies[0].centre).normalize() * 6.0).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(mv(6, (Holder::Pile(unkey(k)), 0), (Holder::Ape, 8), 10));
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 6, ok: false }));
    l.send(ToServer::Move { pos: at.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    assert_eq!(items::count(&s.clients[0].slots, stone), 60, "its own throw waits");
    wait(&mut s, 2.0);
    assert_eq!((items::count(&s.clients[0].slots, stone), s.piles.len()), (70, 0));
    // a basket worn opens nine slots; off again while they hold things: refused
    l.send(ToServer::Chat("/give basket".into()));
    s.tick();
    let b = s.clients[0].slots.iter().position(|x| x.is_some_and(|x| x.item == ITEMS.named("basket").unwrap())).unwrap() as u16;
    l.send(mv(7, (Holder::Ape, b), (Holder::Ape, items::WORN as u16), 1));
    l.send(ToServer::Chat("/give snow 1000".into()));
    l.send(mv(8, (Holder::Ape, items::WORN as u16), (Holder::Ape, b), 1));
    s.tick();
    let got = drain(l);
    assert!(got.contains(&ToClient::Done { seq: 7, ok: true }) && got.contains(&ToClient::Done { seq: 8, ok: false }));
    assert_eq!(items::count(&s.clients[0].slots, stone), 1070, "nine slots and nine more");
    assert_eq!(s.item_ledger(), vec![], "all given and held");
}

/// Creative's grid (ADR 0075): a click takes a stack of anything, a stack put back is gone; survival has none.
#[test]
fn creative_takes_from_the_grid_and_puts_back_into_nothing() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let lamp = BLOCKS.id("blue lamp");
    let pack = ITEMS.named("frame pack").unwrap();
    l.send(ToServer::Creative(true));
    let mv = |seq, from, to, n| ToServer::Items { seq, from, to, n };
    l.send(mv(1, (Holder::Grid, lamp), (Holder::Ape, 2), 64));
    l.send(mv(2, (Holder::Grid, pack), (Holder::Ape, 3), 2));
    l.send(mv(3, (Holder::Grid, pack), (Holder::Ape, 3), 1));
    l.send(mv(4, (Holder::Ape, 2), (Holder::Grid, 0), 14));
    l.send(mv(5, (Holder::Grid, 0x7fff), (Holder::Ape, 4), 1));
    s.tick();
    let done: Vec<bool> = drain(l).iter().filter_map(|m| if let ToClient::Done { ok, .. } = m { Some(*ok) } else { None }).collect();
    assert_eq!(done, [true, false, true, true, false], "a pack stacks by one; no item 0x7fff");
    assert_eq!(held(&s.clients[0].slots), vec![(lamp, 50), (pack, 1)]);
    assert!(s.item_ledger().is_empty());
    l.send(ToServer::Creative(false));
    l.send(mv(6, (Holder::Grid, lamp), (Holder::Ape, 5), 1));
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 6, ok: false }));
}

/// A player that leaves before it has a name has no save to keep its things in: they lie where it stood.
#[test]
fn an_unnamed_leaver_leaves_its_things_lying() {
    let (mut s, links) = server_with(1);
    let (pole, _) = on_land(&s);
    links[0].send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    links[0].send(ToServer::Chat("/give dirt 20".into()));
    s.tick();
    drop(links);
    s.tick();
    assert!(s.clients.is_empty() && s.piles.len() == 1 && s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// A save keeps the piles and the slots, and one from before slots puts what did not fit at its feet.
#[test]
fn piles_and_slots_are_saved_and_old_counts_overflow_to_the_feet() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, _) = on_land(&s);
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Chat("/give dirt 20".into()));
    l.send(ToServer::Items { seq: 1, from: (Holder::Ape, 0), to: (Holder::Ground, 0), n: 5 });
    s.tick();
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!((back.snapshot(), back.piles.len()), (s.snapshot(), 1));
    let mut old = s.snapshot();
    old.players[0].slots = items::from_counts(&[(BLOCKS.id("snow"), 64 * 10 + 1)]);
    let back = Server::from_save(old, channel().1);
    assert_eq!(held(&back.absent["ape"].slots), vec![(BLOCKS.id("snow"), 64 * 9)]);
    assert_eq!(back.piles.len(), 2, "the throw's and the overflow's");
    assert!(back.item_ledger().is_empty());
    // exactly 37 stacks, as many as the ape has slots, and a stack over its size (a smaller `stack` in
    // data/items.toml): re-packed all the same, nothing stuck in rows no pack opens, nothing lost
    let mut old = s.snapshot();
    old.players[0].slots = items::from_counts(&[(BLOCKS.id("snow"), 64 * 37)]);
    old.players[0].slots[0] = Some(Stack::new(BLOCKS.id("snow"), 100));
    let back = Server::from_save(old, channel().1);
    assert!(items::ape_fits(&back.absent["ape"].slots));
    assert_eq!(held(&back.absent["ape"].slots), vec![(BLOCKS.id("snow"), 64 * 9)]);
    assert!(back.item_ledger().is_empty());
    let n: u32 = back.piles.values().flat_map(|p| p.slots.iter().flatten()).filter(|s| s.item == BLOCKS.id("snow")).map(|s| s.n as u32).sum();
    assert_eq!(n, 64 * 36 + 100 - 64 * 9, "the rest at its feet");
}

#[test]
fn a_survival_dig_takes_its_time_and_stone_by_hand_gives_nothing() {
    let (mut s, links) = server_with(2);
    let l = &links[0];
    let dirt = over_dirt(&mut s, l);
    let one = Shape::one(dirt);
    let need = edit::dig_time(&s.world, &one);
    // at once: refused, as is a dig held for 40 % of its time
    l.send(ToServer::Edit { seq: 1, shape: one, material: AIR });
    l.send(ToServer::Dig(Some(one)));
    s.tick();
    wait(&mut s, need * 0.4);
    l.send(ToServer::Edit { seq: 2, shape: one, material: AIR });
    s.tick();
    let got = drain(l);
    assert!(got.contains(&ToClient::Done { seq: 1, ok: false }) && got.contains(&ToClient::Done { seq: 2, ok: false }), "{got:?}");
    assert_eq!(s.world.block(dirt), BLOCKS.id("dirt"));
    // the other player sees the cracks: the shape, how far it was and how fast it goes
    let id = s.clients[0].player.id;
    assert!(drain(&links[1]).iter().any(|m| matches!(m, ToClient::Digging { id: i, at: Some((sh, done, rate)) } if *i == id && *sh == one && *done == 0.0 && (*rate - 1.0 / need).abs() < 1e-6)));
    // held on for its time: dug, and the dirt falls to the ground as a pile (ADR 0074)
    wait(&mut s, need * 0.6);
    l.send(ToServer::Edit { seq: 3, shape: one, material: AIR });
    s.tick();
    let got = drain(l);
    assert!(got.contains(&ToClient::Done { seq: 3, ok: true }), "{got:?}");
    assert!(pile_in(&got, dirt).is_some_and(|p| p[0] == Some(Stack::new(BLOCKS.id("dirt"), 1))), "{got:?}");
    assert!(drain(&links[1]).contains(&ToClient::Digging { id, at: None }), "the cracks end with the dig");
    // stone by hand: slow, and it leaves rubble, not a pile (ADR 0093)
    let stone = Block { pos: dirt.pos - glam::IVec3::Z, ..dirt };
    s.world.set_block(stone, BLOCKS.id("stone"));
    let one = Shape::one(stone);
    l.send(ToServer::Dig(Some(one)));
    s.tick();
    let t = edit::dig_time(&s.world, &one);
    wait(&mut s, t);
    l.send(ToServer::Edit { seq: 4, shape: one, material: AIR });
    s.tick();
    let got = drain(l);
    assert!(got.contains(&ToClient::Done { seq: 4, ok: true }) && s.world.block(stone) == BLOCKS.id("stone rubble"), "{got:?}");
    assert_eq!(items::count(&s.clients[0].slots, BLOCKS.id("stone")), 0);
    assert!(pile_in(&got, stone).is_none());
    // creative does not wait
    l.send(ToServer::Creative(true));
    let below = Block { pos: stone.pos - glam::IVec3::Z, ..stone };
    l.send(ToServer::Edit { seq: 5, shape: Shape::one(below), material: AIR });
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 5, ok: true }));
}

/// Mining (ADR 0093, mining.md 4): granite dug in survival leaves granite rubble in its cell, which falls
/// into the open cell under it and blocks the way; digging the rubble puts it into the slots, no pile.
/// Creative digs it away as before.
/// Flint in gravel (ADR 0097): about one generated gravel cell in four holds a flint; a survival dig of one
/// gives the gravel and the flint, of another the gravel alone, and gravel put back into the flint's cell
/// gives none (no farming). The items' ledger stays even.
#[test]
fn generated_gravel_holds_flint_and_gravel_put_back_none() {
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    drain(&l);
    let (gravel, flint) = (BLOCKS.id("gravel"), ITEMS.named("flint").unwrap());
    // the generated gravel at the top of columns over the sea (alpine ground, river beds)
    let cells: Vec<Block> = {
        let b = &s.world.bodies[0];
        let sea = b.sea.unwrap();
        (0..6u8)
            .flat_map(|f| (40..b.n - 40).step_by(3).flat_map(move |i| (40..b.n - 40).step_by(3).map(move |j| (f, i, j))))
            .filter_map(|(f, i, j)| {
                let top = s.world.air_over_ground(0, b.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5));
                let c = Block { pos: top.pos - glam::IVec3::Z, ..top };
                (c.face == f && c.pos.x == i && c.pos.y == j && c.pos.z as f64 > sea && s.world.block(c) == gravel).then_some(c)
            })
            .take(400)
            .collect()
    };
    let with: Vec<Block> = cells.iter().copied().filter(|&c| edit::held(&s.world, c, gravel) == Some(flint)).collect();
    assert!(cells.len() >= 100, "{} gravel cells", cells.len());
    let share = with.len() as f64 / cells.len() as f64;
    assert!((0.15..0.35).contains(&share), "{share} of gravel holds flint");
    let without = *cells.iter().find(|c| !with.contains(c)).unwrap();
    let flints = |s: &Server| s.piles.values().flat_map(|p| p.slots.iter().flatten()).filter(|x| x.item == flint).map(|x| x.n as u32).sum::<u32>() + items::count(&s.clients[0].slots, flint);
    let mut seq = 0;
    let mut dig = |s: &mut Server, cell: Block| {
        let b = &s.world.bodies[0];
        let up = b.dir(cell.face as usize, cell.pos.x as f64 + 0.5, cell.pos.y as f64 + 0.5);
        let pos = b.centre + up * (b.layer_r(cell.pos.z as f64) + 2.5) + up.any_orthonormal_vector() * 1.5;
        l.send(ToServer::Move { pos: pos.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        s.tick();
        let one = Shape::one(cell);
        l.send(ToServer::Dig(Some(one)));
        s.tick();
        wait(s, edit::dig_time(&s.world, &one) * 1.05);
        seq += 1;
        l.send(ToServer::Edit { seq, shape: one, material: AIR });
        s.tick();
        let got = drain(&l);
        assert!(got.contains(&ToClient::Done { seq, ok: true }), "{got:?}");
    };
    dig(&mut s, with[0]);
    assert_eq!(flints(&s), 1, "the flint's cell gives one");
    dig(&mut s, without);
    assert_eq!(flints(&s), 1, "the other cell none");
    s.world.set_block(with[0], gravel);
    dig(&mut s, with[0]);
    assert_eq!(flints(&s), 1, "gravel put back holds none");
    let mut ledger = s.item_ledger();
    ledger.retain(|x| x.0 != gravel);
    assert!(ledger.is_empty(), "{ledger:?}");
}

#[test]
fn a_survival_dig_leaves_rock_as_rubble_and_digging_that_takes_it() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let cell = over_dirt(&mut s, l);
    let (granite, rubble) = (BLOCKS.id("granite"), BLOCKS.id("granite rubble"));
    let below = Block { pos: cell.pos - glam::IVec3::Z, ..cell };
    s.world.set_block(cell, granite);
    s.world.set_block(below, AIR);
    let dig = |s: &mut Server, b: Block, seq: u32| {
        let one = Shape::one(b);
        l.send(ToServer::Dig(Some(one)));
        s.tick();
        wait(s, edit::dig_time(&s.world, &one) * 1.05);
        l.send(ToServer::Edit { seq, shape: one, material: AIR });
        s.tick();
        drain(l)
    };
    let got = dig(&mut s, cell, 1);
    assert!(got.contains(&ToClient::Done { seq: 1, ok: true }), "{got:?}");
    assert!(got.iter().any(|m| matches!(m, ToClient::Edits(v) if v.contains(&(cell, rubble)))), "the digger is told the rubble");
    assert_eq!(items::count(&s.clients[0].slots, granite) + items::count(&s.clients[0].slots, rubble), 0);
    assert!(s.piles.is_empty());
    wait(&mut s, 1.0);
    assert_eq!((s.world.block(cell), s.world.block(below)), (AIR, rubble), "it fell into the open cell");
    assert!(edit::dig_time(&s.world, &Shape::one(below)) < 1.0, "rubble goes fast by hand");
    let got = dig(&mut s, below, 2);
    assert!(got.contains(&ToClient::Done { seq: 2, ok: true }) && s.world.block(below) == AIR, "{got:?}");
    assert_eq!(items::count(&s.clients[0].slots, rubble), 1);
    assert!(s.piles.is_empty() && s.item_ledger().is_empty());
    // a full pack: the rubble is not dug, it stays a block (nothing lost, no pile; the developer, via the lead)
    let snow = BLOCKS.id("snow");
    let full = s.clients[0].slots.clone();
    for k in 0..items::open(&full) {
        s.clients[0].slots[k] = Some(Stack::new(snow, 64));
    }
    *s.item_made.entry(snow).or_default() += 64 * items::open(&full) as i64;
    s.item_made.remove(&rubble);
    s.world.set_block(below, rubble);
    let got = dig(&mut s, below, 9);
    assert!(got.contains(&ToClient::Done { seq: 9, ok: false }) && s.world.block(below) == rubble, "{got:?}");
    assert!(s.piles.is_empty() && s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    s.world.set_block(below, AIR);
    // creative is unchanged: a dig takes the cell away
    l.send(ToServer::Creative(true));
    let deeper = Block { pos: below.pos - glam::IVec3::Z, ..below };
    s.world.set_block(deeper, granite);
    l.send(ToServer::Edit { seq: 3, shape: Shape::one(deeper), material: AIR });
    s.tick();
    assert_eq!(s.world.block(deeper), AIR);
}

/// Knapping by doing (ADR 0110): flint in hand struck on hard rock; strikes on soil count nothing, strikes
/// closer than `KNAP_WAIT_S` count once, and the fourth that counts makes one flint a hand axe.
#[test]
fn flint_struck_four_times_on_rock_becomes_a_hand_axe() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let cell = over_dirt(&mut s, l);
    let (flint, axe) = (ITEMS.named("flint").unwrap(), ITEMS.named("hand axe").unwrap());
    l.send(ToServer::Chat("/give flint 2".into()));
    l.send(ToServer::Hand(0));
    s.tick();
    let strike = |s: &mut Server, n: usize, secs: f32| {
        for _ in 0..n {
            l.send(ToServer::Strike(cell));
            wait(s, secs);
        }
    };
    strike(&mut s, 6, 0.5);
    assert_eq!(held(&s.clients[0].slots), vec![(flint, 2)], "soil knaps nothing");
    s.world.set_block(cell, BLOCKS.id("granite"));
    // eight strikes in 0.8 s count as two
    strike(&mut s, 8, 0.1);
    assert_eq!(held(&s.clients[0].slots), vec![(flint, 2)], "the wait between strikes holds");
    strike(&mut s, 2, 0.5);
    assert_eq!(held(&s.clients[0].slots), vec![(flint, 1), (axe, 1)]);
    assert_eq!(items::left(s.clients[0].slots.iter().flatten().copied().find(|x| x.item == axe)), Some(ITEMS.get(axe).unwrap().tool.as_ref().unwrap().uses));
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // creative knaps nothing
    l.send(ToServer::Creative(true));
    strike(&mut s, 5, 0.5);
    assert_eq!(held(&s.clients[0].slots), vec![(flint, 1), (axe, 1)]);
}

/// The tool in hand (ADR 0110): the hand axe digs a pick's block in `hardness / speed`, which bare hands
/// cannot; it takes a block hands only break (dolomite: no rubble, so it lies as a pile) but not one over its
/// grade (copper); every dig costs one use, and at the last it is gone with the ledger exact. Creative is as it
/// was: an instant dig that wears nothing.
#[test]
fn the_hand_axe_digs_faster_takes_more_and_wears_out() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let cell = over_dirt(&mut s, l);
    let axe = ITEMS.named("hand axe").unwrap();
    let t = ITEMS.get(axe).unwrap().tool.clone().unwrap();
    l.send(ToServer::Chat("/give hand_axe".into()));
    s.tick();
    let mut seq = 0;
    let mut dig = |s: &mut Server, b: Block, secs: f32| {
        seq += 1;
        let one = Shape::one(b);
        l.send(ToServer::Dig(Some(one)));
        s.tick();
        wait(s, secs);
        l.send(ToServer::Edit { seq, shape: one, material: AIR });
        s.tick();
        l.send(ToServer::Dig(None));
        drain(l).contains(&ToClient::Done { seq, ok: true })
    };
    let granite = BLOCKS.id("granite");
    s.world.set_block(cell, granite);
    let one = Shape::one(cell);
    let (hand, with) = (edit::dig_time(&s.world, &one), edit::dig_time_with(&s.world, &one, Some(&t)));
    assert!((with - hand / t.speed).abs() < 1e-4 && with < hand, "{with} s against {hand} s");
    // in hand slot 1 (empty): bare hands, too soon
    l.send(ToServer::Hand(1));
    assert!(!dig(&mut s, cell, with * 1.1), "bare hands in the axe's time");
    // the half-dug cell's kept seconds run out first
    wait(&mut s, edit::DIG_KEEP_S + 1.0);
    l.send(ToServer::Hand(0));
    assert!(dig(&mut s, cell, with * 1.1) && s.world.block(cell) == BLOCKS.id("granite rubble"), "the axe in its time, and rock leaves its rubble");
    let used = |s: &Server| s.clients[0].slots[0].map(|x| x.payload);
    assert_eq!(used(&s), Some(1));
    // a block hands only break: the axe takes it whole
    let dolomite = BLOCKS.id("dolomite");
    s.world.set_block(cell, dolomite);
    let n = edit::dig_time_with(&s.world, &one, Some(&t)) * 1.1;
    assert!(dig(&mut s, cell, n));
    let piled = |s: &Server, m: u16| s.piles.values().flat_map(|p| p.slots.iter().flatten()).filter(|x| x.item == m).count();
    assert_eq!(piled(&s, dolomite), 1, "dolomite lies dug");
    // over its grade: broken, nothing
    let copper = BLOCKS.id("copper");
    s.world.set_block(cell, copper);
    let n = edit::dig_time_with(&s.world, &one, Some(&t)) * 1.1;
    assert!(dig(&mut s, cell, n) && s.world.block(cell) == AIR);
    assert_eq!(piled(&s, copper), 0, "copper wants grade 2");
    assert_eq!(used(&s), Some(3));
    // the last use: gone, leaving nothing, and the ledger knows
    s.clients[0].slots[0].as_mut().unwrap().payload = t.uses as u64 - 1;
    s.world.set_block(cell, BLOCKS.id("dirt"));
    let n = edit::dig_time_with(&s.world, &one, Some(&t)) * 1.1;
    assert!(dig(&mut s, cell, n));
    assert_eq!(s.clients[0].slots[0], None, "worn away");
    assert!(s.item_ledger().iter().all(|x| x.0 != axe), "{:?}", s.item_ledger());
    // creative: instant, and its axe wears nothing
    l.send(ToServer::Chat("/give hand_axe".into()));
    l.send(ToServer::Creative(true));
    s.tick();
    s.world.set_block(cell, granite);
    seq += 1;
    l.send(ToServer::Edit { seq, shape: one, material: AIR });
    s.tick();
    assert_eq!((s.world.block(cell), used(&s)), (AIR, Some(0)));
}

/// A shape dug at once (the build hammer's) wears the tool a use for every cell it fits (the review of ADR 0110).
#[test]
fn a_shape_dug_with_a_tool_costs_a_use_a_cell() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let cell = over_dirt(&mut s, l);
    let next = Block { pos: cell.pos + glam::IVec3::X, ..cell };
    s.world.set_block(next, BLOCKS.id("dirt"));
    l.send(ToServer::Chat("/give hand_axe".into()));
    s.tick();
    let shape = Shape { min: cell.pos, max: next.pos, ..Shape::one(cell) };
    let t = ITEMS.get(ITEMS.named("hand axe").unwrap()).unwrap().tool.clone().unwrap();
    l.send(ToServer::Dig(Some(shape)));
    s.tick();
    let need = edit::dig_time_with(&s.world, &shape, Some(&t)) * 1.1;
    wait(&mut s, need);
    l.send(ToServer::Edit { seq: 1, shape, material: AIR });
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 1, ok: true }));
    assert_eq!(s.clients[0].slots[0].map(|x| x.payload), Some(2), "two cells, two uses");
}

/// A tool's wear is its stack's payload (ADR 0110), so a save keeps it as it keeps the stack.
#[test]
fn a_save_keeps_a_tools_wear() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Chat("/give hand_axe".into()));
    s.tick();
    s.clients[0].slots[0].as_mut().unwrap().payload = 7;
    drop(links);
    s.tick();
    let path = std::env::temp_dir().join(format!("technoape-wear-{}.sav", std::process::id()));
    s.snapshot().write(&path).unwrap();
    let back = Server::from_save(Save::read(&path).unwrap(), channel().1);
    std::fs::remove_file(&path).unwrap();
    let st = back.absent["ape"].slots[0].unwrap();
    assert_eq!((st.item, st.payload, items::left(Some(st))), (ITEMS.named("hand axe").unwrap(), 7, Some(ITEMS.get(st.item).unwrap().tool.as_ref().unwrap().uses - 7)));
}

/// Moves `n` from slot `a` to slot `b` of the ape, as the client sends it, and a tick.
fn shift(s: &mut Server, l: &Link<ToServer, ToClient>, a: usize, b: usize, n: u16) {
    l.send(ToServer::Items { seq: 1, from: (Holder::Ape, a as u16), to: (Holder::Ape, b as u16), n });
    s.tick();
}

/// The crafting grid (ADR 0140): the pattern moved into the grid's cells is taken as its output,
/// as many of n times as the cells hold and the slots have room for; another recipe's name, an empty grid or no
/// room changes nothing. The ledger is exact throughout; creative crafts the same way.
#[test]
fn the_grid_makes_its_pattern_from_its_cells_or_nothing_changes() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let id = |n: &str| ITEMS.named(n).unwrap();
    let (head, stick, pick, flint) = (id("copper pick head"), id("stick"), id("copper pick"), id("flint"));
    l.send(ToServer::Chat("/give copper_pick_head 2".into()));
    l.send(ToServer::Chat("/give stick 3".into()));
    s.tick();
    // the head above the sticks, in the 2 x 2's right column
    shift(&mut s, l, 0, items::CRAFT_AT + 1, 2);
    shift(&mut s, l, 1, items::CRAFT_AT + 4, 3);
    assert_eq!((s.clients[0].slots[items::CRAFT_AT + 1], s.clients[0].slots[items::CRAFT_AT + 4]), (Some(Stack::new(head, 2)), Some(Stack::new(stick, 3))));
    let craft = |s: &mut Server, recipe: &str, n: u16| {
        l.send(ToServer::Craft { recipe: recipe.into(), n });
        s.tick();
        (held(&s.clients[0].slots), engine::recipes::grid(&s.clients[0].slots))
    };
    assert_eq!(craft(&mut s, "copper axe", 1).0, vec![], "the grid holds no axe: nothing");
    // three asked: two made, then the head's cell is empty
    let (got, cells) = craft(&mut s, "copper pick", 3);
    assert_eq!((got, cells[1], cells[4]), (vec![(pick, 2)], None, Some(stick)));
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    assert_eq!(craft(&mut s, "copper pick", 1).0, vec![(pick, 2)], "no head: nothing");
    // no room: every usable slot full
    l.send(ToServer::Chat("/give copper_pick_head 1".into()));
    s.tick();
    let k = (0..items::HOTBAR).find(|&k| s.clients[0].slots[k].is_some_and(|t| t.item == head)).unwrap();
    shift(&mut s, l, k, items::CRAFT_AT + 1, 1);
    for k in items::usable(&s.clients[0].slots.clone()) {
        if s.clients[0].slots[k].is_none() {
            s.clients[0].slots[k] = Some(Stack::new(flint, 16));
            *s.item_made.entry(flint).or_default() += 16;
        }
    }
    let full = s.clients[0].slots.clone();
    craft(&mut s, "copper pick", 1);
    assert_eq!(s.clients[0].slots, full, "no room: nothing");
    // creative alike, once room is made
    s.clients[0].slots[8] = None;
    *s.item_made.entry(flint).or_default() -= 16;
    l.send(ToServer::Creative(true));
    s.tick();
    assert!(craft(&mut s, "copper pick", 1).0.contains(&(pick, 3)));
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// The workbench (ADR 0140): the grid's cells outside the 2 x 2 take nothing until the player uses
/// a workbench within `BENCH_REACH` (a use from too far, or of another block, opens nothing); there a pick is
/// made in the far column. Walking away puts the cells back into the pack and the far column refuses again.
#[test]
fn the_bench_cells_count_only_at_a_workbench() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let id = |n: &str| ITEMS.named(n).unwrap();
    let (pole, under) = on_land(&s);
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    let bench = Block { pos: under.pos + glam::IVec3::new(2, 0, 1), ..under };
    s.world.set_block(bench, BLOCKS.id("workbench"));
    l.send(ToServer::Chat("/give copper_pick_head 1".into()));
    l.send(ToServer::Chat("/give stick 1".into()));
    s.tick();
    let far = |s: &Server| (s.clients[0].slots[items::CRAFT_AT + 2], s.clients[0].slots[items::CRAFT_AT + 5]);
    shift(&mut s, l, 0, items::CRAFT_AT + 2, 1);
    assert_eq!(far(&s), (None, None), "no bench open: the far column takes nothing");
    // a use of another block, or of the bench from too far, opens nothing
    l.send(ToServer::Use(under));
    s.tick();
    shift(&mut s, l, 0, items::CRAFT_AT + 2, 1);
    assert_eq!(far(&s), (None, None));
    let away = pole + DVec3::X * 20.0;
    l.send(ToServer::Move { pos: away.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Use(bench));
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    shift(&mut s, l, 0, items::CRAFT_AT + 2, 1);
    assert_eq!(far(&s), (None, None), "used from 20 m: nothing opened");
    // at the bench: the head over the stick in the far column makes a pick
    l.send(ToServer::Use(bench));
    s.tick();
    shift(&mut s, l, 0, items::CRAFT_AT + 2, 1);
    shift(&mut s, l, 1, items::CRAFT_AT + 5, 1);
    assert_eq!(far(&s), (Some(Stack::new(id("copper pick head"), 1)), Some(Stack::new(id("stick"), 1))));
    l.send(ToServer::Craft { recipe: "copper pick".into(), n: 1 });
    s.tick();
    assert_eq!(held(&s.clients[0].slots), vec![(id("copper pick"), 1)]);
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // walking away: what lies in the cells goes back into the pack, and the far column refuses again
    l.send(ToServer::Chat("/give stick 2".into()));
    s.tick();
    let k = (0..items::HOTBAR).find(|&k| s.clients[0].slots[k].is_some_and(|t| t.item == id("stick"))).unwrap();
    shift(&mut s, l, k, items::CRAFT_AT + 8, 2);
    assert_eq!(s.clients[0].slots[items::CRAFT_AT + 8], Some(Stack::new(id("stick"), 2)));
    l.send(ToServer::Move { pos: away.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    assert_eq!((engine::recipes::grid(&s.clients[0].slots), items::count(&s.clients[0].slots, id("stick"))), ([None; items::CRAFT], 2));
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    shift(&mut s, l, k, items::CRAFT_AT + 2, 1);
    assert_eq!(far(&s), (None, None), "the bench closed when it was left");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// What lies in the grid's cells is never lost (ADR 0140): closing the screen puts it back into
/// the pack, what does not fit at the feet; so does a death and a leave; a save keeps it in the cells and the
/// join after puts it into the pack; a save from before the grid (37 slots) loads with empty cells.
#[test]
fn the_cells_go_back_on_close_death_and_leaving_and_a_save_keeps_them() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let id = |n: &str| ITEMS.named(n).unwrap();
    let (stick, flint, log) = (id("stick"), id("flint"), id("oak log"));
    let (pole, _) = on_land(&s);
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Chat("/give stick 5".into()));
    s.tick();
    let cell = |s: &Server, k: usize| s.clients[0].slots[items::CRAFT_AT + k];
    // close: back into the pack
    shift(&mut s, l, 0, items::CRAFT_AT, 5);
    assert_eq!((cell(&s, 0), items::count(&s.clients[0].slots, stick)), (Some(Stack::new(stick, 5)), 0));
    l.send(ToServer::Close);
    s.tick();
    assert_eq!((cell(&s, 0), items::count(&s.clients[0].slots, stick)), (None, 5));
    // close with a full pack: the sticks at the feet, nothing lost
    shift(&mut s, l, 0, items::CRAFT_AT + 3, 5);
    for k in items::usable(&s.clients[0].slots.clone()) {
        s.clients[0].slots[k] = Some(Stack::new(flint, 16));
        *s.item_made.entry(flint).or_default() += 16;
    }
    l.send(ToServer::Close);
    s.tick();
    let lying = |s: &Server, m: u16| s.piles.values().flat_map(|p| p.slots.iter().flatten()).filter(|x| x.item == m).map(|x| x.n as u32).sum::<u32>();
    assert_eq!((cell(&s, 3), lying(&s, stick)), (None, 5));
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // a death: the cells into the pack at once (the pack lies where it fell after the respawn, ADR 0040)
    for k in items::usable(&s.clients[0].slots.clone()) {
        s.clients[0].slots[k] = None;
        *s.item_made.entry(flint).or_default() -= 16;
    }
    l.send(ToServer::Chat("/give oak_log 4".into()));
    s.tick();
    shift(&mut s, l, 0, items::CRAFT_AT + 4, 4);
    l.send(ToServer::Chat("/hurt 40".into()));
    s.tick();
    assert!(s.clients[0].vitals.dead.is_some());
    assert_eq!((cell(&s, 4), items::count(&s.clients[0].slots, log)), (None, 4));
    l.send(ToServer::Respawn);
    s.tick();
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // a save with logs in a cell keeps them there; the join after puts them into the pack
    l.send(ToServer::Chat("/give oak_log 3".into()));
    s.tick();
    shift(&mut s, l, 0, items::CRAFT_AT + 1, 3);
    let save = Save::from_bytes(&s.snapshot().to_bytes()).unwrap();
    assert_eq!(save.players[0].slots[items::CRAFT_AT + 1], Some(Stack::new(log, 3)));
    let (tx, rx) = channel();
    let mut back = Server::from_save(save, rx);
    assert!(back.item_ledger().is_empty(), "{:?}", back.item_ledger());
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Hello { name: "ape".into() });
    back.tick();
    assert_eq!((back.clients[0].slots[items::CRAFT_AT + 1], items::count(&back.clients[0].slots, log)), (None, 3));
    assert!(back.item_ledger().is_empty(), "{:?}", back.item_ledger());
    // a leave: into the pack before the save remembers it
    shift(&mut s, l, 0, items::CRAFT_AT, 3);
    drop(links);
    s.tick();
    assert_eq!((s.absent["ape"].slots[items::CRAFT_AT], items::count(&s.absent["ape"].slots, log)), (None, 3));
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // a save from before the grid: 37 slots, the cells empty
    let mut old = s.snapshot();
    old.players[0].slots.truncate(items::APE_SLOTS);
    let back = Server::from_save(old, channel().1);
    assert_eq!((back.absent["ape"].slots.len(), items::count(&back.absent["ape"].slots, log)), (items::SLOTS, 3));
    assert!(back.item_ledger().is_empty(), "{:?}", back.item_ledger());
}

/// Each copper tool (draft ADR recipe-list-and-copper-tools) digs a block of its kind in `hardness / speed`
/// and wears a use; the pick takes copper (grade 2) whole, and a tool's last use leaves a stick in its slot,
/// the ledger exact. A save keeps the crafted tools and their wear.
#[test]
fn copper_tools_dig_their_kind_take_copper_and_leave_a_stick() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Hello { name: "ape".into() });
    let cell = over_dirt(&mut s, l);
    let one = Shape::one(cell);
    let mut seq = 0;
    let mut dig = |s: &mut Server, secs: f32| {
        seq += 1;
        l.send(ToServer::Dig(Some(one)));
        s.tick();
        wait(s, secs);
        l.send(ToServer::Edit { seq, shape: one, material: AIR });
        s.tick();
        l.send(ToServer::Dig(None));
        drain(l).contains(&ToClient::Done { seq, ok: true })
    };
    let kinds = [("copper pick", "granite"), ("copper axe", "log"), ("copper shovel", "dirt"), ("copper knife", "shrub")];
    for (tool, _) in kinds {
        l.send(ToServer::Chat(format!("/give {}", tool.replace(' ', "_"))));
    }
    s.tick();
    for (k, (tool, block)) in kinds.into_iter().enumerate() {
        let t = ITEMS.get(ITEMS.named(tool).unwrap()).unwrap().tool.clone().unwrap();
        l.send(ToServer::Hand(k as u8));
        s.world.set_block(cell, BLOCKS.id(block));
        let (hand, with) = (edit::dig_time(&s.world, &one), edit::dig_time_with(&s.world, &one, Some(&t)));
        assert!((with - hand / 4.0).abs() < 1e-4, "{tool} on {block}: {with} s against {hand} s");
        assert!(dig(&mut s, with * 1.1) && s.world.block(cell) != BLOCKS.id(block), "{tool} digs {block} in its time");
        assert_eq!(s.clients[0].slots[k].map(|x| (x.item, x.payload)), Some((ITEMS.named(tool).unwrap(), 1)), "{tool} wore a use");
        s.world.set_block(cell, AIR);
    }
    // the pick takes copper whole, and its last use leaves a stick
    let pick = ITEMS.get(ITEMS.named("copper pick").unwrap()).unwrap().tool.clone().unwrap();
    l.send(ToServer::Hand(0));
    let copper = BLOCKS.id("copper");
    s.world.set_block(cell, copper);
    let need = edit::dig_time_with(&s.world, &one, Some(&pick)) * 1.1;
    assert!(dig(&mut s, need));
    let piled = s.piles.values().flat_map(|p| p.slots.iter().flatten()).filter(|x| x.item == copper).count();
    assert_eq!(piled, 1, "copper lies dug");
    s.clients[0].slots[0].as_mut().unwrap().payload = pick.uses as u64 - 1;
    s.world.set_block(cell, BLOCKS.id("granite"));
    let need = edit::dig_time_with(&s.world, &one, Some(&pick)) * 1.1;
    assert!(dig(&mut s, need));
    let stick = ITEMS.named("stick").unwrap();
    assert_eq!(s.clients[0].slots[0], Some(Stack::new(stick, 1)), "worn away, the stick left");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // a save keeps the tools and their wear
    let kept: Vec<Slot> = s.clients[0].slots.clone();
    drop(links);
    s.tick();
    let path = std::env::temp_dir().join(format!("technoape-copper-{}.sav", std::process::id()));
    s.snapshot().write(&path).unwrap();
    let back = Server::from_save(Save::read(&path).unwrap(), channel().1);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(back.absent["ape"].slots, kept);
}

#[test]
fn a_half_dug_block_waits_five_seconds_for_its_digger() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let dirt = over_dirt(&mut s, l);
    let one = Shape::one(dirt);
    let need = edit::dig_time(&s.world, &one);
    let other = Shape::one(Block { pos: dirt.pos + glam::IVec3::X, ..dirt });
    // half, a look elsewhere for 2 s, back for the other half: dug
    l.send(ToServer::Dig(Some(one)));
    s.tick();
    wait(&mut s, need * 0.5);
    l.send(ToServer::Dig(Some(other)));
    s.tick();
    wait(&mut s, 2.0);
    l.send(ToServer::Dig(Some(one)));
    s.tick();
    wait(&mut s, need * 0.5);
    l.send(ToServer::Edit { seq: 1, shape: one, material: AIR });
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 1, ok: true }));
    // half, let go for longer than DIG_KEEP_S: it starts over
    let below = Block { pos: dirt.pos - glam::IVec3::Z, ..dirt };
    s.world.set_block(below, BLOCKS.id("dirt"));
    let next = Shape::one(below);
    l.send(ToServer::Dig(Some(next)));
    s.tick();
    wait(&mut s, need * 0.6);
    l.send(ToServer::Dig(None));
    s.tick();
    wait(&mut s, edit::DIG_KEEP_S + 1.0);
    l.send(ToServer::Dig(Some(next)));
    s.tick();
    wait(&mut s, need * 0.4);
    l.send(ToServer::Edit { seq: 2, shape: next, material: AIR });
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 2, ok: false }));
}

#[test]
fn a_dig_held_on_nothing_gathers_no_time_and_a_leaver_s_cracks_end() {
    let (mut s, mut links) = server_with(2);
    let dirt = over_dirt(&mut s, &links[0]);
    let air = Block { pos: dirt.pos + glam::IVec3::Z, ..dirt };
    assert_eq!(s.world.block(air), AIR);
    // held on the air, then a block lands there: it takes its whole time from then
    links[0].send(ToServer::Dig(Some(Shape::one(air))));
    s.tick();
    wait(&mut s, 2.0);
    s.world.set_block(air, BLOCKS.id("dirt"));
    links[0].send(ToServer::Edit { seq: 1, shape: Shape::one(air), material: AIR });
    s.tick();
    assert!(drain(&links[0]).contains(&ToClient::Done { seq: 1, ok: false }));
    // the other player sees a dig, and its end when the digger leaves
    links[0].send(ToServer::Dig(Some(Shape::one(dirt))));
    s.tick();
    let id = s.clients[0].player.id;
    assert!(drain(&links[1]).iter().any(|m| matches!(m, ToClient::Digging { id: d, at: Some(_) } if *d == id)));
    drop(links.remove(0));
    s.tick();
    assert!(drain(&links[0]).contains(&ToClient::Digging { id, at: None }));
}

/// A save from before ADR 0041 moved Luna's grid and the inertial frame in the shared coordinates (version 5
/// and older): a player on Luna is moved with it, one in space goes to the spawn, keeping what it held; in a
/// save of this version the same places are left alone.
#[test]
fn players_of_a_save_before_the_frames_moved_are_moved_with_them() {
    let (s, _links) = server_with(0);
    let mut save = s.snapshot();
    let player = |name: &str, at: DVec3| save::Saved {
        name: name.into(), pos: at.to_array(), fwd: [1.0, 0.0, 0.0], creative: false, slots: items::from_counts(&[(2, 5)]), water: 0, vitals: Default::default(),
    };
    let on_luna = engine::world::OLD_MOON_AT + DVec3::Y * 30.0;
    let in_space = DVec3::new(500.0, 0.0, -1.0e6);
    save.players = vec![player("luna", on_luna), player("space", in_space)];
    let at = |sv: &Server, name: &str| DVec3::from(sv.absent[name].pos);
    let now = Server::from_save(save.clone(), channel().1);
    assert_eq!((at(&now, "luna"), at(&now, "space")), (on_luna, in_space), "this version's save is left alone");
    save.version = 5;
    let old = Server::from_save(save, channel().1);
    assert_eq!(at(&old, "luna"), on_luna - engine::world::OLD_MOON_AT + engine::world::MOON_AT);
    assert_eq!((at(&old, "space"), held(&old.absent["space"].slots)), (old.spawn(), vec![(2, 5)]));
}

#[test]
fn a_saved_player_comes_back_with_what_it_held() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let ground = s.world.bodies[0].centre + DVec3::Z * s.world.bodies[0].height(DVec3::Z);
    let deep = s.world.block_at(ground - DVec3::Z * 6.0);
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Move { pos: (ground + DVec3::Z).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    // stone by hand gives nothing (ADR 0034): the stone it holds is given, and the dig still changes the world
    l.send(ToServer::Chat("/give stone 1".into()));
    l.send(ToServer::Creative(true));
    l.send(ToServer::Edit { seq: 1, shape: Shape::one(deep), material: AIR });
    l.send(ToServer::Creative(false));
    s.tick();
    drop(links); // the player leaves; the server keeps it by name
    s.tick();
    assert_eq!(s.players(), 0);
    let path = std::env::temp_dir().join(format!("technoape-test-{}.sav", std::process::id()));
    s.snapshot().write(&path).unwrap();
    let (tx, rx) = channel();
    let mut back = Server::from_save(Save::read(&path).unwrap(), rx);
    std::fs::remove_file(&path).unwrap();
    assert_eq!((back.world.hash(), back.snapshot()), (s.world.hash(), s.snapshot()));
    let rec = path.with_extension("rec");
    back.record(&rec).unwrap();
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Hello { name: "ape".into() });
    back.tick();
    let got = drain(&c);
    // a recording of a server that began from a save replays from that save
    for _ in 0..40 {
        back.tick();
    }
    let r = replay(&rec, None).unwrap();
    std::fs::remove_file(&rec).unwrap();
    assert_eq!((r.diverged, r.checks, r.server.world.hash()), (None, 2, back.world.hash()));
    assert!(invs(&got).contains(&vec![(BLOCKS.id("stone"), 1)]), "{got:?}");
    assert!(DVec3::from_array(back.clients[0].player.pos).distance(ground + DVec3::Z) < 0.01);
    assert!(Save::from_bytes(b"not a save").is_err());
}

/// A player digs an ore body out (ADR 0063, ore-budget.md "Checks"): its ledger goes from the count to zero,
/// and the recount from the edits (the soak's check) agrees.
#[test]
fn a_player_digs_an_ore_body_out_and_its_ledger_reaches_zero() {
    use engine::ore::{OreBody, MINERALS};
    let mut world = World::new(256, 32, 1);
    let b0 = &world.bodies[0];
    let top = world.air_over_ground(0, DVec3::Z);
    let ground = b0.centre + b0.dir(top.face as usize, top.pos.x as f64 + 0.5, top.pos.y as f64 + 0.5) * b0.col_height(top.face, top.pos.x, top.pos.y);
    let kind = MINERALS.kinds.iter().position(|k| k.name == "lead pocket").unwrap() as u16;
    let lump = OreBody { kind, face: top.face, at: DVec3::new(top.pos.x as f64 + 0.5, top.pos.y as f64 + 0.5, top.pos.z as f64 - 6.0), a: 2.5, b: 2.5, h: 2.0, angle: 0.0 };
    std::sync::Arc::get_mut(&mut world.bodies).unwrap()[0].set_ores(vec![lump]);
    let (tx, rx) = channel();
    let mut s = Server::with_world(world, 1, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    let cells = engine::ore::cells(&s.world.bodies[0], 0);
    assert!(s.ore_open == vec![cells.len() as i64] && cells.len() > 20, "{:?} for {} cells", s.ore_open, cells.len());
    l.send(ToServer::Hello { name: "miner".into() });
    l.send(ToServer::Move { pos: (ground + (ground - s.world.bodies[0].centre).normalize()).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Creative(true));
    // from the top down, so every cell is reached through the hole over it
    let mut order = cells.clone();
    order.sort_by_key(|c| -c.z);
    for (k, c) in order.iter().enumerate() {
        l.send(ToServer::Edit { seq: k as u32 + 1, shape: Shape::one(Block { body: 0, face: top.face, pos: *c }), material: AIR });
    }
    s.tick();
    assert_eq!((s.world.ore_left.clone(), s.ore_ledger()), (Some(vec![0]), vec![]), "dug out");
}

/// Prospecting (ADR 0138): the hand axe struck on rock over a lead lump 3.5 m down
/// answers that player "lead: strong" and costs a use; a strike sooner than a second after is not answered and
/// costs nothing; soil and creative answer nothing; a body the ledger has dug out leaves the rock ringing empty.
#[test]
fn a_prospecting_strike_hears_the_ore_below_and_costs_a_use() {
    use engine::ore::{OreBody, MINERALS};
    let mut world = World::new(256, 32, 1);
    let b0 = &world.bodies[0];
    let top = world.air_over_ground(0, DVec3::Z);
    let ground = b0.centre + b0.dir(top.face as usize, top.pos.x as f64 + 0.5, top.pos.y as f64 + 0.5) * b0.col_height(top.face, top.pos.x, top.pos.y);
    let kind = MINERALS.kinds.iter().position(|k| k.name == "lead pocket").unwrap() as u16;
    let lump = OreBody { kind, face: top.face, at: DVec3::new(top.pos.x as f64 + 0.5, top.pos.y as f64 + 0.5, top.pos.z as f64 - 6.0), a: 2.5, b: 2.5, h: 2.0, angle: 0.0 };
    std::sync::Arc::get_mut(&mut world.bodies).unwrap()[0].set_ores(vec![lump]);
    let (tx, rx) = channel();
    let mut s = Server::with_world(world, 1, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    l.send(ToServer::Hello { name: "prospector".into() });
    l.send(ToServer::Move { pos: (ground + (ground - s.world.bodies[0].centre).normalize()).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    let cell = Block { pos: top.pos - glam::IVec3::Z, ..top };
    s.world.set_block(cell, BLOCKS.id("granite"));
    l.send(ToServer::Chat("/give hand_axe 1".into()));
    l.send(ToServer::Hand(0));
    s.tick();
    drain(&l);
    let axe = ITEMS.named("hand axe").unwrap();
    let uses = ITEMS.get(axe).unwrap().tool.as_ref().unwrap().uses;
    let strike = |s: &mut Server| {
        l.send(ToServer::Strike(cell));
        s.tick();
        drain(&l).into_iter().filter_map(|m| if let ToClient::Prospect(v) = m { Some(v) } else { None }).collect::<Vec<_>>()
    };
    let lead = vec![("lead".to_string(), 2u8)];
    assert_eq!(strike(&mut s), vec![lead.clone()]);
    wait(&mut s, 0.5);
    assert!(strike(&mut s).is_empty(), "a second strike within the second is not answered");
    wait(&mut s, 0.6);
    assert_eq!(strike(&mut s), vec![lead]);
    let left = |s: &Server| items::left(s.clients[0].slots.iter().flatten().copied().find(|x| x.item == axe));
    assert_eq!(left(&s), Some(uses - 2), "a use a strike answered");
    s.world.set_block(cell, BLOCKS.id("dirt"));
    wait(&mut s, 1.1);
    assert!(strike(&mut s).is_empty(), "soil is not struck");
    s.world.set_block(cell, BLOCKS.id("granite"));
    s.world.ore_left = Some(vec![s.ore_open[0] / 2 - 1]);
    wait(&mut s, 1.1);
    assert_eq!(strike(&mut s), vec![vec![("lead".to_string(), 1u8)]], "over half mined: a band weaker");
    s.world.ore_left = Some(vec![0]);
    wait(&mut s, 1.1);
    assert_eq!(strike(&mut s), vec![vec![]], "dug out: the rock rings empty");
    l.send(ToServer::Creative(true));
    wait(&mut s, 1.1);
    assert!(strike(&mut s).is_empty(), "creative does not prospect");
    assert_eq!(left(&s), Some(uses - 4));
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// Ore samples (ADR 0138): a sample lying over a body is taken by hand like any
/// pick-up, once; the ore ledger does not move, and the items' ledger counts the sample as made.
#[test]
fn an_ore_sample_is_taken_once_and_the_ore_ledger_holds() {
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    drain(&l);
    let open = s.world.ore_left.clone();
    let (at, row) = {
        let b = &s.world.bodies[0];
        b.ores.iter().filter_map(|o| Some((o, engine::clutter::SAMPLES[o.kind as usize]?))).find_map(|(o, row)| {
            o.columns().find_map(|(i, j)| {
                let tile = b.tile(o.face, 0, i.div_euclid(engine::world::CS), j.div_euclid(engine::world::CS));
                let (z, _, _) = engine::clutter::ground(b, &tile, o.face, i, j)?;
                let at = Block { body: 0, face: o.face, pos: glam::IVec3::new(i, j, z) };
                engine::clutter::pickup_at(&s.world, at, 0.0).filter(|t| t.row == row).map(|_| (at, row))
            })
        }).expect("a sample over a body")
    };
    let item = engine::clutter::CLUTTER[row].item().unwrap().0;
    let b = &s.world.bodies[0];
    let up = b.dir(at.face as usize, at.pos.x as f64 + 0.5, at.pos.y as f64 + 0.5);
    let pos = b.centre + up * (b.layer_r(at.pos.z as f64) + 0.5) + up.any_orthonormal_vector() * 1.5;
    l.send(ToServer::Move { pos: pos.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    l.send(ToServer::Take { seq: 1, at });
    l.send(ToServer::Take { seq: 2, at });
    s.tick();
    let got = drain(&l);
    assert!(got.contains(&ToClient::Done { seq: 1, ok: true }) && got.contains(&ToClient::Done { seq: 2, ok: false }), "{got:?}");
    assert_eq!(items::count(&s.clients[0].slots, item), 1);
    assert_eq!(s.world.ore_left, open, "a sample holds no ore");
    assert!(s.ore_ledger().is_empty() && s.item_ledger().is_empty(), "{:?} {:?}", s.ore_ledger(), s.item_ledger());
}

/// A bucket scoops a block of sea and pours it back; creative water is counted as made. Through all of it,
/// a save and a load, not a unit appears or vanishes (the ledger stays 0).
#[test]
fn the_bucket_carries_water_and_none_is_lost() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let b = s.world.bodies.clone();
    let b = &b[0];
    let sea = b.radius - engine::world::SEA_DEPTH;
    // a spot over 2 m of sea
    let d = (0..2000)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / 2000.0;
            let t = k as f64 * 2.4;
            DVec3::new((1.0 - z * z).sqrt() * t.cos(), z, (1.0 - z * z).sqrt() * t.sin())
        })
        .find(|&d| b.height(d) < sea - 2.0)
        .expect("the planet has sea");
    let at = s.world.block_at(b.centre + d * (sea - 0.25));
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Move { pos: (b.centre + d * (sea + 1.0)).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Bucket { seq: 1, at, fill: true });
    s.tick();
    let got = drain(l);
    assert!(got.contains(&ToClient::Done { seq: 1, ok: true }), "{got:?}");
    assert!(got.iter().any(|m| matches!(m, ToClient::Inventory { water, .. } if *water == U as u32)));
    assert!(got.iter().any(|m| matches!(m, ToClient::Water(v) if !v.is_empty())), "the scoop is sent");
    assert_eq!(s.water_ledger(), 0);
    // a second scoop fills nothing: the bucket is full
    l.send(ToServer::Bucket { seq: 2, at, fill: true });
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 2, ok: false }));
    for _ in 0..100 {
        s.tick();
        assert_eq!(s.water_ledger(), 0);
    }
    l.send(ToServer::Bucket { seq: 3, at, fill: false });
    l.send(ToServer::Creative(true));
    l.send(ToServer::Bucket { seq: 4, at, fill: false });
    s.tick();
    let got = drain(l);
    assert!(got.contains(&ToClient::Done { seq: 3, ok: true }) && got.contains(&ToClient::Done { seq: 4, ok: true }));
    assert_eq!((s.clients[0].water, s.made), (0, U));
    // stored while it moves: a save and a load keep it
    assert!(s.world.water.stored() > 0);
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!((back.hash() == s.hash(), back.water_ledger()), (false, 0), "players differ, water does not");
    assert_eq!(back.world.water.all(), s.world.water.all());
    for _ in 0..200 {
        s.tick();
        assert_eq!(s.water_ledger(), 0);
    }
    // far out of reach: refused
    l.send(ToServer::Bucket { seq: 5, at: s.world.block_at(b.centre - d * sea), fill: true });
    s.tick();
    assert!(drain(l).contains(&ToClient::Done { seq: 5, ok: false }));
    // settled level with the sea again: forgotten, what it brought booked to the sea
    assert_eq!((s.world.water.stored(), s.world.water.sea_extra), (0, U));
}

#[test]
fn walking_input_moves_the_player_on_the_ground() {
    let (mut s, links) = server_with(1);
    drain(&links[0]);
    let start = DVec3::from_array(s.clients[0].player.pos);
    links[0].send(ToServer::Input(Input { mv: [1.0, 0.0], jump: true, fwd: [1.0, 0.0, 0.0], pitch: 0.0, ..Default::default() }));
    for _ in 0..60 {
        s.tick(); // 3 s at the default 20 Hz
    }
    let p = &s.clients[0].phys;
    let up = phys::up(&s.world, p.pos);
    // the spawn faces a steep slope that takes a jump per wall; walking speed is phys's test
    assert!((p.pos - start).reject_from(up).length() > 3.0, "moved {:?}", p.pos - start);
    assert!(!phys::inside(&s.world, p));
    let seen = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::You { me, .. } = m { Some(me) } else { None }).last().unwrap();
    assert_eq!(seen.pos, p.pos, "clients get the server's position");
}

#[test]
fn steps_are_taken_once_in_order_and_replay_to_the_same_place() {
    let (mut s, links) = server_with(1);
    drain(&links[0]);
    let l = &links[0];
    let start = s.clients[0].phys;
    // each step at its own time near the server's clock, as a client sends it: the moon's pull moves with
    // the clock (ADR 0024)
    let t0 = s.world.time + 0.5;
    let inp = |k: u32| Input { mv: [1.0, (k % 7) as f32 / 7.0 - 0.5], jump: k % 40 == 0, fwd: [1.0, 0.0, 0.2], pitch: 0.0, t: t0 + k as f64 * phys::DT, ..Default::default() };
    // 3 s of steps in batches of three, one batch a tick, with a duplicate and an old one thrown in
    let mut me = start;
    for t in 0..60u32 {
        l.send(ToServer::Steps { first: t * 3 + 1, inputs: (t * 3 + 1..t * 3 + 4).map(inp).collect() });
        if t == 30 {
            l.send(ToServer::Steps { first: 2, inputs: vec![inp(2)] });
        }
        for k in t * 3 + 1..t * 3 + 4 {
            phys::step_at(&mut s.world, &mut me, &inp(k), phys::DT);
        }
        s.tick();
    }
    let you = drain(l).into_iter().filter_map(|m| if let ToClient::You { step, me, .. } = m { Some((step, me)) } else { None }).last().unwrap();
    assert_eq!(you, (180, me), "the server took every step once and lands where the client predicted");
    // a client a second ahead is held to real time: sixty steps waiting at most, and in one tick the three
    // of this tick plus the three it had left over
    l.send(ToServer::Steps { first: 181, inputs: (181..400).map(inp).collect() });
    s.tick();
    assert_eq!((s.clients[0].acked, s.clients[0].queue.len()), (186, 54));
    assert!((start.pos - me.pos).length() > 3.0);
}

/// Sprint (ADR 0118): a survival player sprinting 100 ticks ends where the engine's step puts it and has lost
/// the sprint's hunger; at the floor the bit is refused and it walks; creative sprints for nothing.
#[test]
fn a_sprint_lands_where_predicted_costs_hunger_and_stops_at_the_floor() {
    let (mut s, links) = server_with(1);
    drain(&links[0]);
    let l = &links[0];
    let mut seq = 0u32;
    // 100 ticks of sprinting steps sent as a client sends them; `predict` the sprint bit the client steps with
    let mut go = |s: &mut Server, predict: bool| {
        let (mut me, mut sprinted) = (s.clients[0].phys, 0);
        for _ in 0..100 {
            let t0 = s.world.time + 0.5;
            // as a client steers: by turning, with a jump now and then
            let inp = |k: u32, sprint: bool| Input { mv: [1.0, 0.0], jump: k % 40 == 0, turn: if k % 50 == 0 { 0.4 } else { 0.0 }, t: t0 + (k % 3) as f64 * phys::DT, sprint, ..Default::default() };
            let inputs: Vec<Input> = (seq + 1..seq + 4).map(|k| inp(k, true)).collect();
            for k in seq + 1..seq + 4 {
                sprinted += phys::step_at(&mut s.world, &mut me, &inp(k, predict), phys::DT) as u32;
            }
            l.send(ToServer::steps(seq + 1, inputs));
            seq += 3;
            s.tick();
        }
        let you = drain(l).into_iter().filter_map(|m| if let ToClient::You { me, .. } = m { Some(me) } else { None }).last().unwrap();
        (you, me, sprinted)
    };
    let h0 = s.clients[0].vitals.hunger;
    let (you, me, sprinted) = go(&mut s, true);
    assert_eq!(you, me, "the server sprinted where the client predicted");
    // a sprint's drain for each step that sprinted (not a ledge's, which stopped it), a walk's for the rest
    let lost = h0 - s.clients[0].vitals.hunger;
    let want = vitals::DRAIN * vitals::WALK_DRAIN * (sprinted as f64 * 3.0 + (300 - sprinted) as f64);
    assert!(sprinted > 200 && (lost - want).abs() < 1e-9, "lost {lost}, the rule says {want} ({sprinted} of 300 sprinted)");
    s.clients[0].vitals.hunger = 6.0;
    let (you, me, _) = go(&mut s, false);
    assert_eq!(you, me, "at 6 of 20 the bit is refused and the player walks");
    l.send(ToServer::Creative(true));
    s.tick();
    let h = s.clients[0].vitals.hunger;
    let (you, me, _) = go(&mut s, true);
    assert_eq!((you, s.clients[0].vitals.hunger), (me, h), "creative sprints at the floor and for nothing");
}

/// Swimming (ADR 0118): a dive along the look, a turn under water and a stroke back up to the surface land
/// where the client predicted, step for step, and the stroke costs a sprint's hunger.
#[test]
fn a_dive_turn_and_surfacing_land_where_predicted() {
    // (a planet whose sea is deeper than the test planet's 4 m)
    let (tx, rx) = channel();
    let mut s = Server::new(1536, 32, 1, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    let l = &l;
    let b = &s.world.bodies[0];
    let sea = b.radius - engine::world::SEA_DEPTH;
    let d = (0..4000)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / 4000.0;
            let t = k as f64 * 2.4;
            DVec3::new((1.0 - z * z).sqrt() * t.cos(), z, (1.0 - z * z).sqrt() * t.sin())
        })
        .min_by(|a, c| b.height(*a).total_cmp(&b.height(*c)))
        .unwrap();
    assert!(b.height(d) < sea - 6.0, "the deepest sea is {} m", sea - b.height(d));
    let at = b.centre + d * (sea - 1.0);
    l.send(ToServer::Move { pos: at.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    drain(l);
    let mut me = s.clients[0].phys;
    let h0 = s.clients[0].vitals.hunger;
    let t0 = s.world.time + 0.5;
    // 2 s down at a pitch of -0.9, a turn, 6 s up at +0.7 with a little strafe: 160 ticks of 3 steps
    let inp = |k: u32| {
        let pitch = if k < 120 { -0.9 } else { 0.7 };
        Input { mv: [1.0, if k >= 120 { 0.2 } else { 0.0 }], sprint: true, pitch, turn: if k == 120 { 1.2 } else { 0.0 }, t: t0 + k as f64 * phys::DT, ..Default::default() }
    };
    let (mut deepest, mut strokes) = (0f64, 0);
    for t in 0..160u32 {
        let (first, inputs): (u32, Vec<Input>) = (t * 3 + 1, (t * 3 + 1..t * 3 + 4).map(inp).collect());
        for i in &inputs {
            strokes += phys::step_at(&mut s.world, &mut me, i, phys::DT) as u32;
        }
        deepest = deepest.max(sea - (me.pos - s.world.bodies[0].centre).length());
        l.send(ToServer::steps(first, inputs));
        s.tick();
    }
    let you = drain(l).into_iter().filter_map(|m| if let ToClient::You { step, me, .. } = m { Some((step, me)) } else { None }).last().unwrap();
    assert_eq!(you, (480, me), "the server swam where the client predicted");
    let feet = sea - (me.pos - s.world.bodies[0].centre).length();
    eprintln!("dived to {deepest:.2} m, came up to {feet:.2} m under");
    assert!(deepest > 4.0 && feet < 1.8, "dived to {deepest} m and came up to {feet} m under");
    let lost = h0 - s.clients[0].vitals.hunger;
    // every step that stroked (or sprinted) drains at the sprint's rate, the rest as a walk
    let want = vitals::DRAIN * vitals::WALK_DRAIN * (strokes as f64 * vitals::SPRINT_DRAIN + (480 - strokes) as f64);
    assert!(strokes > 300 && (lost - want).abs() < 1e-9, "lost {lost}, {want} for {strokes} strokes");
}

/// The review of ADR 0118: a client that predicts hunger across its steps with the server's drain refuses the
/// sprint at the very step the server does, so a run through the floor reconciles with no correction.
#[test]
fn a_sprint_through_the_hunger_floor_needs_no_correction() {
    let (mut s, links) = server_with(1);
    drain(&links[0]);
    let l = &links[0];
    s.clients[0].vitals.hunger = 6.0507; // (the floor falls inside a tick)
    let (mut me, mut h) = (s.clients[0].phys, 6.0507);
    let t0 = s.world.time + 0.5;
    let inp = |k: u32| Input { mv: [1.0, 0.0], jump: k % 40 == 0, turn: if k % 50 == 0 { 0.4 } else { 0.0 }, t: t0 + k as f64 * phys::DT, sprint: true, ..Default::default() };
    let mut refused = 0;
    for t in 0..40u32 {
        let inputs: Vec<Input> = (t * 3 + 1..t * 3 + 4).map(inp).collect();
        // the client's prediction: each step gated at the hunger it predicts, then drained as the server drains
        for i in &inputs {
            let g = vitals::gate(*i, h, false);
            refused += !g.sprint as u32;
            let sprinted = phys::step_at(&mut s.world, &mut me, &g, phys::DT);
            h = vitals::drained(h, vitals::rate(&g, sprinted));
        }
        l.send(ToServer::steps(t * 3 + 1, inputs));
        s.tick();
        let you = drain(l).into_iter().filter_map(|m| if let ToClient::You { step, me, hunger, .. } = m { Some((step, me, hunger)) } else { None }).last().unwrap();
        assert_eq!(you, (t * 3 + 3, me, h), "tick {t}: a correction");
    }
    assert!(refused > 30 && refused < 110, "the floor was crossed on the way: {refused} steps refused");
}

/// The review of ADR 0118: a sprint held in the sea drains as a sprint only on the steps that stroke along the
/// look, the rest (the body not yet deep enough in) as a walk.
#[test]
fn a_sprint_held_in_the_sea_drains_only_where_it_strokes() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let b = &s.world.bodies[0];
    let sea = b.radius - engine::world::SEA_DEPTH;
    let d = (0..2000)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / 2000.0;
            let t = k as f64 * 2.4;
            DVec3::new((1.0 - z * z).sqrt() * t.cos(), z, (1.0 - z * z).sqrt() * t.sin())
        })
        .find(|&d| b.height(d) < sea - 3.0)
        .expect("the planet has sea");
    l.send(ToServer::Move { pos: (b.centre + d * (sea - 1.0)).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    drain(l);
    let (h0, mut me, mut strokes) = (s.clients[0].vitals.hunger, s.clients[0].phys, 0);
    let inp = Input { mv: [1.0, 0.0], sprint: true, ..Default::default() };
    for t in 0..20u32 {
        for _ in 0..3 {
            strokes += phys::step_at(&mut s.world, &mut me, &inp, phys::DT) as u32;
        }
        l.send(ToServer::steps(t * 3 + 1, vec![inp; 3]));
        s.tick();
    }
    let lost = h0 - s.clients[0].vitals.hunger;
    let want = vitals::DRAIN * vitals::WALK_DRAIN * (strokes as f64 * vitals::SPRINT_DRAIN + (60 - strokes) as f64);
    assert!((lost - want).abs() < 1e-9, "lost {lost}, {want} for {strokes} strokes");
}

#[test]
fn a_client_whose_clock_is_off_lands_where_it_predicted_in_space() {
    // out of a body's frame, up and the way ahead follow where the bodies are: the server must step at
    // the client's time, or every flight there is corrected (the camera glitch of bugs/2026-11-10_19-03-47)
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Creative(true));
    s.tick();
    let w = &s.world;
    let (pc, mc) = (w.sky.planet(w.time).centre, w.sky.moon(w.time).centre);
    // (past Terra's frame: as far out as Luna, on the other side; ADR 0041 moved Luna to 60 radii)
    let (pos, f) = w.locate(pc + (pc - mc).normalize() * w.sky.moon_orbit);
    assert_eq!(f, engine::world::INERTIAL);
    l.send(ToServer::Move { pos: pos.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    drain(l);
    let mut me = s.clients[0].phys;
    let mut cw = World::new(256, 32, 1);
    for t in 0..40u32 {
        cw.time = s.world.time - 0.04; // the client's clock a little behind the server's
        let inp = Input { mv: [1.0, 0.3], fwd: [0.0, 1.0, 0.0], fly: 1000.0, jump: t % 2 == 0, t: cw.time, ..Default::default() };
        for _ in 0..3 {
            phys::step(&cw, &mut me, &inp, phys::DT);
        }
        l.send(ToServer::Steps { first: t * 3 + 1, inputs: vec![inp; 3] });
        s.tick();
    }
    let (step, server) = drain(l).into_iter().filter_map(|m| if let ToClient::You { step, me, .. } = m { Some((step, me)) } else { None }).last().unwrap();
    assert_eq!(step, 120);
    assert!((server.pos - me.pos).length() < 1e-9, "the server is {} m off", (server.pos - me.pos).length());
    assert!((me.pos - pos).length() > 100.0, "it flew");
}

#[test]
fn chat_reaches_everyone_and_commands_answer_their_sender() {
    let (mut s, links) = server_with(2);
    links[0].send(ToServer::Hello { name: "ape".into() });
    links[1].send(ToServer::Hello { name: "bot".into() });
    s.tick();
    let chat = |l: &Link<ToServer, ToClient>| -> Vec<String> {
        drain(l).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect()
    };
    assert!(chat(&links[1]).contains(&"ape joined".to_string()));
    chat(&links[0]);
    links[0].send(ToServer::Chat("hello there".into()));
    links[0].send(ToServer::Chat("/time set noon".into()));
    links[0].send(ToServer::Chat("/give Blue Lamp 5".into()));
    links[0].send(ToServer::Chat("/give stone".into()));
    links[0].send(ToServer::Chat("/playmode creative".into()));
    links[0].send(ToServer::Chat("/nope".into()));
    s.tick();
    let (a, b) = (chat(&links[0]), chat(&links[1]));
    assert_eq!(b, vec!["<ape> hello there".to_string()], "commands answer only their sender");
    assert_eq!(a.len(), 6, "{a:?}");
    assert!(a[1].starts_with("time set to day 1, 12:00"), "{}", a[1]);
    assert_eq!((a[2].as_str(), a[3].as_str()), ("gave 5 blue lamp: 5 held", "gave 16 stone: 16 held"));
    assert!(a[5].starts_with("unknown command"));
    s.tick();
    assert!((s.world.time - sky::DAY / 8.0).abs() < 0.2, "{}", s.world.time);
    let c = &s.clients.iter().find(|c| c.player.name == "ape").unwrap();
    assert!(c.creative && items::count(&c.slots, BLOCKS.id("blue lamp")) == 5 && items::count(&c.slots, BLOCKS.id("stone")) == 16);
    // time runs on from where it was set, and survives a save
    assert!(s.snapshot().tick as f64 / TICK_HZ > sky::DAY / 8.0);
    links[0].send(ToServer::Chat("/time set 10".into()));
    links[0].send(ToServer::Chat("/time".into()));
    s.tick();
    let a = chat(&links[0]);
    assert!(a.iter().any(|l| l.starts_with("it is day 1, 09:08")), "{a:?}");
    s.tick();
    assert!((s.world.time - 10.0).abs() < 0.2, "a time before the session's start: {}", s.world.time);
    // a season (ADR 0055): whole days on, so the hour stays; the summer solstice is 13.5 days in
    links[0].send(ToServer::Chat("/time set summer".into()));
    s.tick();
    let a = chat(&links[0]);
    assert!(a.iter().any(|l| l.starts_with("time set to day 14, 09:08")), "{a:?}");
}

#[test]
fn timewarp_runs_the_clock_faster() {
    let (mut s, links) = server_with(1);
    s.tick();
    links[0].send(ToServer::Chat("/timewarp 100".into()));
    s.tick();
    let t0 = s.world.time;
    for _ in 0..10 {
        s.tick();
    }
    assert!((s.world.time - t0 - 10.0 * 100.0 / TICK_HZ).abs() < 1e-6, "{}", s.world.time - t0);
    let you = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::You { warp, .. } = m { Some(warp) } else { None }).last();
    assert_eq!(you, Some(100.0));
    links[0].send(ToServer::Chat("/timewarp up".into()));
    links[0].send(ToServer::Chat("/timewarp 0".into()));
    s.tick();
    assert_eq!(s.warp, 1000, "0 is refused, up is ten times");
}

#[test]
fn commands_complete_to_known_words() {
    // every line the completion list builds (engine::net::complete) is one the commands know
    let mut lines = vec!["/".to_string()];
    let mut full = Vec::new();
    while let Some(l) = lines.pop() {
        let (at, opts) = engine::net::complete(&l);
        if opts.is_empty() {
            full.push(l.trim_end().to_string());
        }
        lines.extend(opts.iter().map(|o| format!("{}{o} ", &l[..at])));
    }
    assert!(full.len() > 15, "{full:?}");
    let (mut s, links) = server_with(1);
    s.tick();
    drain(&links[0]);
    for l in &full {
        links[0].send(ToServer::Chat(l.clone()));
    }
    for _ in 0..full.len().div_ceil(MSGS_PER_TICK) {
        s.tick();
    }
    let answers: Vec<String> = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
    assert_eq!(answers.len(), full.len(), "{answers:?}");
    for (l, a) in full.iter().zip(&answers) {
        assert!(!["unknown command", "no play mode", "no block", "not a"].iter().any(|bad| a.starts_with(bad)), "{l}: {a}");
    }
}

#[test]
fn goto_takes_a_player_to_the_bodies_and_the_sun() {
    let (mut s, links) = server_with(1);
    let me = |s: &Server| s.clients[0].phys.pos;
    for (to, body) in [("Luna", 1), ("home", 0), ("luna", 1), ("terra", 0)] {
        links[0].send(ToServer::Chat(format!("/goto {to}")));
        s.tick();
        let b = &s.world.bodies[body];
        let r = (me(&s) - b.centre).length();
        assert!((r - b.top((me(&s) - b.centre) / r)).abs() < 3.0, "{to}: {r}");
        assert!(!phys::inside(&s.world, &s.clients[0].phys));
    }
    links[0].send(ToServer::Chat("/goto sun".into()));
    s.tick();
    assert!(me(&s).distance(s.world.bodies[0].spawn_point()) < 3.0, "survival stays");
    links[0].send(ToServer::Chat("/playmode creative".into()));
    links[0].send(ToServer::Chat("/goto sun".into()));
    s.tick();
    let x = s.world.to_inertial(me(&s));
    let d = (x - s.world.sky.sun(s.world.time)).length() / s.world.sky.sun_radius;
    assert!((d - SUN_VIEW).abs() < 0.1, "{d} sun radii away");
    let got: Vec<ToClient> = drain(&links[0]);
    for line in ["gone to the sun", "gone to Luna", "gone to Terra"] {
        assert!(got.iter().any(|m| matches!(m, ToClient::Chat(t) if t == line)), "{line}");
    }
}

#[test]
fn goto_knows_the_bodies_by_name_only() {
    // bugs/2026-11-11_13-10-42: planet and moon are gone, Terra and Luna are the names
    let (mut s, links) = server_with(1);
    s.tick();
    drain(&links[0]);
    for to in ["planet", "moon"] {
        links[0].send(ToServer::Chat(format!("/goto {to}")));
    }
    links[0].send(ToServer::Chat("/help".into()));
    s.tick();
    let a: Vec<String> = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
    assert!(a[0].starts_with("usage: /goto") && a[1].starts_with("usage: /goto"), "{a:?}");
    assert!(!a.iter().any(|l| l.contains("planet") || l.contains("moon")), "{a:?}");
}

#[test]
fn place_puts_a_block_under_the_player_or_where_it_is_told() {
    // bugs/2026-11-11_13-03-12
    let (mut s, links) = server_with(2);
    s.tick();
    drain(&links[0]);
    drain(&links[1]);
    let feet = s.clients[0].phys.pos;
    let under = s.world.block_at(feet - phys::up(&s.world, feet) * 0.5);
    let there = Block { pos: under.pos + glam::IVec3::new(3, 0, 4), ..under };
    let (p, q) = (under.pos, there.pos);
    links[0].send(ToServer::Chat("/place blue lamp".into()));
    links[0].send(ToServer::Chat(format!("/place copper {} {} {} {}", there.face, q.x, q.y, q.z)));
    links[0].send(ToServer::Chat("/place nothing".into()));
    links[0].send(ToServer::Chat(format!("/place stone {} {} {} {}", under.face, p.x, p.y, p.z + 1)));
    // (the review) a face that is none, a negative one too, and a place off the grid are refused
    for bad in ["-1 5 5 0", "6 5 5 0", "0 5 5 2000000000", "0 -5 5 0"] {
        links[0].send(ToServer::Chat(format!("/place stone {bad}")));
    }
    s.tick();
    assert_eq!((s.world.block(under), s.world.block(there)), (BLOCKS.id("blue lamp"), BLOCKS.id("copper")));
    let a: Vec<String> = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
    assert_eq!(a.len(), 8, "{a:?}");
    assert!(a[4..].iter().all(|t| t.starts_with("no such block on")), "{a:?}");
    assert!(a[0].starts_with("placed blue lamp at face") && a[1].starts_with("placed copper"), "{a:?}");
    assert!(a[2].starts_with("no block 'nothing'"), "{a:?}");
    assert!(a[3].starts_with("a player stands there"), "a solid block never traps a player: {a:?}");
    let edits: usize = drain(&links[1]).into_iter().map(|m| if let ToClient::Edits(e) = m { e.len() } else { 0 }).sum();
    assert_eq!(edits, 2, "everyone sees the blocks");
}

/// Puts player 0 `metres` over the ground at `on_land`'s spot, walking (an empty input), and ticks until
/// it has stood still on the ground for a second; its health then.
fn drop_from(s: &mut Server, l: &Link<ToServer, ToClient>, metres: f64) -> u8 {
    let (pole, _) = on_land(s);
    let up = phys::up(&s.world, pole);
    l.send(ToServer::Move { pos: (pole + up * (metres - 2.0)).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Input(Input::default()));
    for _ in 0..140 {
        s.tick();
    }
    assert!(s.clients[0].phys.on_ground, "landed");
    s.clients[0].vitals.health
}

/// Drops `n` layers of sand on a survival player standing on the ground (the developer, 2026-11-13: sand
/// buries and hurts, ADR 0079) and waits `secs`; its health then and the sand's cells.
fn bury(n: i32, secs: f64) -> (u8, Option<String>, Vec<u16>) {
    bury_in(BLOCKS.id("sand"), n, secs)
}

/// `bury` with layers of block `sand`.
fn bury_in(sand: u16, n: i32, secs: f64) -> (u8, Option<String>, Vec<u16>) {
    let (mut s, links) = server_with(1);
    drop_from(&mut s, &links[0], 0.0);
    let pos = s.clients[0].phys.pos;
    let up = phys::up(&s.world, pos);
    let feet = s.world.block_at(pos + up * 0.5);
    // a slab 3 x 3 wide (a single column two high would slide off itself, to its angle of rest)
    for k in 0..n {
        for (di, dj) in (-1..=1).flat_map(|i| (-1..=1).map(move |j| (i, j))) {
            let b = Block { pos: feet.pos + glam::IVec3::new(di, dj, 4 + k), ..feet };
            s.world.set_block(b, sand);
            s.falls.poke(b);
        }
    }
    for _ in 0..(secs * 20.0) as usize {
        s.tick();
    }
    let cells = (0..3).map(|k| s.world.block(Block { pos: feet.pos + glam::IVec3::Z * k, ..feet })).collect();
    (s.clients[0].vitals.health, s.clients[0].vitals.dead.clone(), cells)
}

#[test]
fn sand_over_the_head_takes_breath_then_health() {
    let sand = BLOCKS.id("sand");
    let (health, _, cells) = bury(2, 25.0);
    assert_eq!(cells, [sand, sand, AIR], "the sand settles in the player's cells, feet and head");
    assert!(health < vitals::MAX, "buried to the eyes for 25 s hurts: health {health}");
    let (early, _, _) = bury(2, 5.0);
    assert_eq!(early, vitals::MAX, "a breath held: the first seconds cost nothing");
    let (_, dead, _) = bury(2, 60.0);
    assert!(dead.is_some_and(|d| d.contains("buried")), "staying under kills, by burial");
}

/// ADR 0094 (mining.md 2: a drop hurts who is under it): rock falling through a player's head hits it once, by
/// `ROCK_HIT`; one layer then lies at the feet, which costs no breath. Sand hits nothing (it only buries).
#[test]
fn falling_rock_hits_the_head_it_falls_through() {
    let rubble = BLOCKS.id("granite rubble");
    let (health, _, cells) = bury_in(rubble, 1, 5.0);
    assert_eq!(cells[..2], [rubble, AIR], "it fell through the head to the feet");
    assert_eq!(health, vitals::MAX - ROCK_HIT);
}

/// ADR 0094: a survival dig of granite on a joint loosens 2 to 5 more of it along the joint into rubble; a dig off
/// every crack takes the one block.
#[test]
fn a_dig_on_a_crack_drops_rubble_along_it() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, under) = on_land(&s);
    let granite = BLOCKS.id("granite");
    let rubble = BLOCKS.get(granite).unwrap().rubble.unwrap();
    // the ground under the player (whose feet are in `under`) made a granite floor three deep
    let cell = |di: i32, dj: i32, dl: i32| Block { pos: under.pos + glam::IVec3::new(di, dj, -dl), ..under };
    let near: Vec<Block> = (-6..=6).flat_map(|di| (-6..=6).flat_map(move |dj| (1..=3).map(move |dl| cell(di, dj, dl)))).collect();
    for &b in &near {
        s.world.set_block(b, granite);
    }
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    let mut seq = 0;
    let mut dig = |s: &mut Server, b: Block| {
        seq += 1;
        let one = Shape::one(b);
        l.send(ToServer::Dig(Some(one)));
        s.tick();
        wait(s, edit::dig_time(&s.world, &one) * 1.05);
        l.send(ToServer::Edit { seq, shape: one, material: AIR });
        s.tick();
        assert!(drain(l).contains(&ToClient::Done { seq, ok: true }), "dug {b:?}");
    };
    let reach = |b: &&Block| s.world.block_centre(**b).distance(pole) < 6.0 && b.pos.z == under.pos.z - 1;
    let on = *near.iter().filter(reach).find(|b| engine::crack::of(&s.world, **b) & engine::crack::HIT != 0).expect("a joint near");
    let off = *near.iter().filter(reach).find(|b| engine::crack::of(&s.world, **b) & engine::crack::HIT == 0).expect("solid rock near");
    let loose = engine::crack::drop(&s.world, on, granite);
    assert!((2..=5).contains(&loose.len()));
    dig(&mut s, on);
    let count = |s: &Server| near.iter().filter(|b| s.world.block(**b) == rubble).count();
    assert_eq!(count(&s), 1 + loose.len(), "the dug block and its neighbours along the joint");
    assert!(loose.iter().all(|b| s.world.block(*b) == rubble));
    if !loose.contains(&off) {
        dig(&mut s, off);
        assert_eq!(count(&s), 2 + loose.len(), "off the cracks: the one block");
    }
    assert!(s.item_ledger().is_empty());
}

/// ADR 0112 (mining.md 2, undercut and drop): a survival player who digs the last cell of a 3 x 3 room under shale
/// (span 1) brings the slab over its centre down as rubble; in creative the roof stays.
#[test]
fn digging_out_a_room_under_shale_drops_its_roof() {
    for creative in [false, true] {
        let (mut s, links) = server_with(1);
        let l = &links[0];
        let (pole, under) = on_land(&s);
        let shale = BLOCKS.id("shale");
        let rubble = BLOCKS.get(shale).unwrap().rubble.unwrap();
        // under the ground beside the player: shale 6 deep, a room at its 5th layer dug out but for one corner
        let cell = |di: i32, dj: i32, dl: i32| Block { pos: under.pos + glam::IVec3::new(di, dj, -dl), ..under };
        let near: Vec<Block> = (2..=8).flat_map(|di| (-3..=3).flat_map(move |dj| (1..=6).map(move |dl| cell(di, dj, dl)))).collect();
        for &b in &near {
            let room = b.pos.z == under.pos.z - 5 && (b.pos.x - under.pos.x - 5).abs() <= 1 && (b.pos.y - under.pos.y).abs() <= 1;
            s.world.set_block(b, if room { AIR } else { shale });
        }
        let last = cell(4, -1, 5);
        s.world.set_block(last, shale);
        if creative {
            l.send(ToServer::Chat("/playmode creative".into()));
        }
        l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        s.tick();
        let loose = engine::crack::drop(&s.world, last, shale);
        let slab = engine::crack::undercut(&s.world, &[last], &loose);
        assert!(!slab.is_empty() && slab.iter().all(|b| (b.pos.x, b.pos.y) == (cell(5, 0, 0).pos.x, cell(5, 0, 0).pos.y)), "{slab:?}");
        let one = Shape::one(last);
        l.send(ToServer::Dig(Some(one)));
        s.tick();
        let t = edit::dig_time(&s.world, &one) * 1.05;
        wait(&mut s, t);
        l.send(ToServer::Edit { seq: 1, shape: one, material: AIR });
        s.tick();
        assert!(drain(l).contains(&ToClient::Done { seq: 1, ok: true }));
        let count = near.iter().filter(|b| s.world.block(**b) == rubble).count();
        if creative {
            assert_eq!(count, 0, "creative takes the one block");
            assert!(slab.iter().all(|b| s.world.block(*b) == shale), "and the roof stays");
        } else {
            assert_eq!(count, 1 + loose.len() + slab.len(), "the dug block, its crack's and the slab, as rubble");
            assert!(s.item_ledger().is_empty());
        }
    }
}

#[test]
fn sand_at_the_feet_only_does_not_hurt() {
    let (health, _, cells) = bury(1, 25.0);
    assert_eq!(cells[..2], [BLOCKS.id("sand"), AIR], "one block settles at the feet");
    assert_eq!(health, vitals::MAX);
}

fn vitals_of(got: &[ToClient]) -> Vec<(u8, u8, Option<String>)> {
    got.iter().filter_map(|m| if let ToClient::Vitals { health, hunger, dead, .. } = m { Some((*health, *hunger, dead.clone())) } else { None }).collect()
}

fn chats(got: &[ToClient]) -> Vec<String> {
    got.iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t.clone()) } else { None }).collect()
}

#[test]
fn a_hard_landing_hurts_by_its_speed_and_a_soft_one_or_creative_does_not() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    assert_eq!(vitals_of(&drain(l)), vec![(20, 20, None)], "sent on join");
    assert_eq!(drop_from(&mut s, l, 2.5), 20, "a short drop is free");
    let h = drop_from(&mut s, l, 13.0);
    assert!((9..=11).contains(&(20 - h)), "13 m costs about 10: {h} left");
    let got = drain(l);
    assert!(vitals_of(&got).iter().any(|v| v.0 == h), "the client is told: {:?}", vitals_of(&got));
    l.send(ToServer::Creative(true));
    assert_eq!(drop_from(&mut s, l, 30.0), h, "creative takes no impact");
}

#[test]
fn a_cliff_on_luna_costs_what_its_landing_speed_says() {
    // whatever Luna's pull is: the damage follows the speed, so a weak pull lands softly
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Chat("/goto luna".into()));
    s.tick();
    let feet = s.clients[0].phys.pos;
    let (up, g) = phys::pull(&s.world, feet);
    l.send(ToServer::Move { pos: (feet + up * 18.0).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Input(Input::default()));
    for _ in 0..400 {
        s.tick();
    }
    let want = vitals::impact_damage((2.0 * g * 18.0).sqrt());
    assert!(s.clients[0].phys.on_ground);
    assert!((20 - s.clients[0].vitals.health).abs_diff(want) <= 1, "{} left, {want} expected under a pull of {g}", s.clients[0].vitals.health);
}

#[test]
fn landing_in_water_costs_nothing() {
    let (mut s, links) = server_with(1);
    let b = &s.world.bodies[0];
    let sea = b.radius - engine::world::SEA_DEPTH;
    let d = (0..2000)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / 2000.0;
            let t = k as f64 * 2.4;
            DVec3::new((1.0 - z * z).sqrt() * t.cos(), z, (1.0 - z * z).sqrt() * t.sin())
        })
        .find(|&d| b.height(d) < sea - 3.0)
        .expect("the planet has sea");
    let at = b.centre + d * (sea + 15.0);
    links[0].send(ToServer::Move { pos: at.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    links[0].send(ToServer::Input(Input::default()));
    for _ in 0..200 {
        s.tick();
    }
    assert_eq!(s.clients[0].vitals.health, 20);
}

#[test]
fn death_stops_the_player_tells_everyone_and_lasts_until_a_respawn() {
    let (mut s, links) = server_with(2);
    links[0].send(ToServer::Hello { name: "ape".into() });
    s.tick();
    drain(&links[1]);
    assert_eq!(drop_from(&mut s, &links[0], 45.0), 0);
    let cause = s.clients[0].vitals.dead.clone().expect("dead");
    assert!(cause.starts_with("ape hit the ground at ") && cause.ends_with(" m/s"), "{cause}");
    assert!(chats(&drain(&links[1])).contains(&cause), "everyone is told");
    assert!(vitals_of(&drain(&links[0])).contains(&(0, 20, Some(cause.clone()))));
    // dead, it moves nothing and edits nothing
    let at = s.clients[0].phys.pos;
    let (_, under) = on_land(&s);
    links[0].send(ToServer::Move { pos: (at + DVec3::X * 50.0).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    links[0].send(ToServer::Input(Input { mv: [1.0, 0.0], jump: true, ..Default::default() }));
    links[0].send(ToServer::Edit { seq: 1, shape: Shape::one(under), material: AIR });
    links[0].send(ToServer::Chat("/heal".into()));
    for _ in 0..20 {
        s.tick();
    }
    assert!(s.clients[0].phys.pos.distance(at) < 0.05, "moved {}", s.clients[0].phys.pos.distance(at));
    let got = drain(&links[0]);
    assert!(got.contains(&ToClient::Done { seq: 1, ok: false }));
    assert!(chats(&got).contains(&"you are dead: respawn first".to_string()));
    // a dead player who leaves is dead when it comes back, through a save too
    drop(links);
    s.tick();
    let mut back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    let (tx, rx) = channel();
    back.accept = rx;
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Hello { name: "ape".into() });
    back.tick();
    assert_eq!(back.clients[0].vitals.dead.as_ref(), Some(&cause));
    assert!(vitals_of(&drain(&c)).contains(&(0, 20, Some(cause.clone()))));
    // a respawn: at the spawn, full again, and what it carried left lying where it died (ADR 0047)
    let dirt = Some(Stack::new(BLOCKS.id("dirt"), 5));
    back.clients[0].slots[3] = dirt;
    c.send(ToServer::Respawn);
    back.tick();
    let me = &back.clients[0];
    assert!(held(&me.slots).is_empty() && back.piles.values().any(|p| p.slots[0] == dirt), "{:?}", held(&me.slots));
    assert!(me.phys.pos.distance(back.world.bodies[0].spawn_point()) < 1.0);
    assert_eq!(me.vitals, Vitals::default());
    assert_eq!(vitals_of(&drain(&c)), vec![(20, 20, None)]);
}

#[test]
fn hunger_follows_the_player_s_steps_not_the_warped_clock() {
    let (mut s, links) = server_with(2);
    links[0].send(ToServer::Chat("/timewarp 10000".into()));
    links[0].send(ToServer::Input(Input { mv: [1.0, 0.0], jump: true, fwd: [1.0, 0.0, 0.0], ..Default::default() }));
    links[1].send(ToServer::Creative(true));
    links[1].send(ToServer::Input(Input { mv: [1.0, 0.0], ..Default::default() }));
    for _ in 0..200 {
        s.tick(); // 10 s, more than 3 game years at the warp
    }
    let lost = 20.0 - s.clients[0].vitals.hunger;
    let want = 10.0 * vitals::WALK_DRAIN * 20.0 / sky::DAY;
    assert!((lost - want).abs() < 1e-6, "walking 10 s costs {want}, not {lost}");
    assert_eq!(s.clients[1].vitals, Vitals::default(), "creative drains nothing");
    // a block dug costs a twentieth of a point
    let ground = s.world.bodies[0].centre + DVec3::Z * s.world.bodies[0].height(DVec3::Z);
    let deep = s.world.block_at(ground - DVec3::Z * 6.0);
    links[0].send(ToServer::Move { pos: (ground + DVec3::Z).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    links[0].send(ToServer::Input(Input::default()));
    s.tick();
    let h = s.clients[0].vitals.hunger;
    // (a survival dig is held for its time first, ADR 0034; standing that long drains a little too)
    let brush = Shape { max: deep.pos + 1, ..Shape::one(deep) };
    links[0].send(ToServer::Dig(Some(brush)));
    s.tick();
    let t = edit::dig_time(&s.world, &brush);
    wait(&mut s, t);
    links[0].send(ToServer::Edit { seq: 1, shape: brush, material: AIR });
    s.tick();
    let (lost, stood) = (h - s.clients[0].vitals.hunger, (t as f64 + 1.0) * 20.0 / sky::DAY);
    assert!(lost > 8.0 * vitals::DIG - 1e-9 && lost < 8.0 * vitals::DIG + stood, "{h} to {}: {lost}", s.clients[0].vitals.hunger);
}

#[test]
fn structure_stands_a_structure_file_near_the_player_and_refuses_what_is_no_name() {
    let (mut s, links) = server_with(1);
    s.data = "../../data".into();
    let l = &links[0];
    l.send(ToServer::Hello { name: "ape".into() });
    s.tick();
    drain(l);
    let log = BLOCKS.placeable().find(|d| d.name == "log").unwrap().id;
    let here = s.world.block_at(s.clients[0].phys.pos);
    let logs = |s: &Server| (-12..=12).flat_map(|i| (-12..=12).flat_map(move |j| (-4..=14).map(move |k| glam::IVec3::new(i, j, k)))).filter(|d| s.world.block(Block { pos: here.pos + *d, ..here }) == log).count();
    let before = logs(&s);
    for line in ["/structure oak 3", "/structure", "/structure ../Cargo", "/structure no_such", "/structure oak x", "/structure CON"] {
        l.send(ToServer::Chat(line.into()));
    }
    s.tick();
    let got = drain(l);
    let a = chats(&got);
    assert!(a[0].starts_with("placed oak: "), "{a:?}");
    assert!(logs(&s) >= before + 3, "an oak's trunk stands near: {} logs, {before} before", logs(&s));
    // the players are told of every block, as of any edit
    assert!(got.iter().any(|m| matches!(m, ToClient::Edits { .. })), "no edits were sent");
    assert!([1, 2, 4, 5].iter().all(|i| a[*i].starts_with("usage: /structure")), "{a:?}");
    assert!(a[3].starts_with("no structure 'no_such'"), "{a:?}");
}

/// needs.md step 1: the use button held on fresh water within reach fills thirst in two seconds and the
/// client is told; held on the sea (salt), on water out of reach, or let go, it gives nothing.
#[test]
fn a_player_drinks_fresh_water_and_not_the_sea() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Input(Input::default()));
    s.tick();
    // a pit two deep on land filled by hand, and a cell of the sea
    let b = &s.world.bodies[0];
    let (sea, centre) = (b.layer_r(b.sea.unwrap()), b.centre);
    let dirs = || (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) });
    let d = dirs().find(|d| b.height(*d) < sea - 4.0).expect("a sea");
    let land = dirs().find(|d| b.height(*d) > sea + 6.0).expect("land");
    let salt = s.world.block_at(centre + d * (sea - 1.5));
    let top = s.world.air_over_ground(0, land);
    let pit = [Block { pos: top.pos - glam::IVec3::Z, ..top }, Block { pos: top.pos - glam::IVec3::Z * 2, ..top }];
    for c in pit {
        s.world.set_block(c, AIR);
        s.world.regap(c);
    }
    let World { bodies, edits, water, sky, time, .. } = &mut s.world;
    assert!(water.pour(&engine::world::Solid::at(bodies, edits, sky, *time), World::col(pit[1]), pit[1].pos.z, 2 * engine::liquid::U));
    let fresh = pit[1];
    let stand = |s: &mut Server, at: Block| {
        let p = s.world.block_centre(at) + phys::up(&s.world, s.world.block_centre(at)) * 1.5;
        (s.clients[0].phys.pos, s.clients[0].phys.vel) = (p, DVec3::ZERO);
    };
    let sip = |s: &mut Server, at: Option<Block>, secs: f64| {
        l.send(ToServer::Drink(at));
        for _ in 0..(secs * TICK_HZ) as u32 {
            s.tick();
        }
        s.clients[0].vitals.thirst
    };
    stand(&mut s, fresh);
    s.clients[0].vitals.thirst = 0.0;
    drain(l);
    let half = sip(&mut s, Some(fresh), 1.0);
    assert!((9.0..=10.5).contains(&half), "half in a second: {half}");
    assert_eq!(sip(&mut s, Some(fresh), 2.0), vitals::MAX as f64, "full in two");
    let told: Vec<u8> = drain(l).iter().filter_map(|m| if let ToClient::Vitals { thirst, .. } = m { Some(*thirst) } else { None }).collect();
    assert_eq!(told.last(), Some(&vitals::MAX), "the client is told: {told:?}");
    s.clients[0].vitals.thirst = 5.0;
    assert!(sip(&mut s, None, 1.0) < 5.0, "let go, nothing");
    stand(&mut s, salt);
    s.clients[0].vitals.thirst = 5.0;
    assert!(sip(&mut s, Some(salt), 1.0) < 5.0, "the sea is salt");
    assert!(sip(&mut s, Some(fresh), 1.0) < 5.0, "the pond is out of reach");
}

#[test]
fn heal_feed_and_hurt_try_the_overlay_and_the_death_screen() {
    let (mut s, links) = server_with(2);
    let l = &links[0];
    l.send(ToServer::Hello { name: "ape".into() });
    s.tick();
    drain(l);
    drain(&links[1]);
    for line in ["/hurt 5", "/hurt", "/hurt x", "/heal", "/hurt 7", "/playmode creative", "/hurt 3", "/heal", "/playmode survival"] {
        l.send(ToServer::Chat(line.into()));
    }
    s.clients[0].vitals.hunger = 3.0;
    l.send(ToServer::Chat("/feed".into()));
    s.tick();
    let got = drain(l);
    let a = chats(&got);
    assert_eq!(a[0], "took 5: health 15 of 20");
    assert!(a[1].starts_with("usage: /hurt") && a[2].starts_with("usage: /hurt"), "{a:?}");
    assert_eq!((a[3].as_str(), a[4].as_str()), ("health full", "took 7: health 13 of 20"));
    assert!(a[6].starts_with("creative has no health") && a[7].starts_with("creative has no health"), "{a:?}");
    assert_eq!(a[9], "hunger full");
    assert!(s.clients[0].vitals.health == 13 && s.clients[0].vitals.hunger > 19.99, "{:?}", s.clients[0].vitals);
    l.send(ToServer::Chat("/feed 0".into()));
    l.send(ToServer::Chat("/feed 21".into()));
    s.tick();
    let got = drain(l);
    assert_eq!(chats(&got), vec!["hunger 0 of 20".to_string(), "usage: /feed [0 to 20]".to_string()]);
    assert!(vitals_of(&got).contains(&(13, 0, None)), "{:?}", vitals_of(&got));
    l.send(ToServer::Chat("/hurt 40".into()));
    s.tick();
    assert_eq!(s.clients[0].vitals.dead.as_deref(), Some("ape was hurt by a command"));
    assert!(chats(&drain(&links[1])).contains(&"ape was hurt by a command".to_string()));
    assert!(vitals_of(&drain(l)).contains(&(0, 0, Some("ape was hurt by a command".into()))));
}

/// Two clients of one name are two saved players; a second `Hello` changes nothing; a name is cleaned.
#[test]
fn equal_names_are_kept_apart_and_a_second_hello_changes_nothing() {
    let (mut s, links) = server_with(3);
    s.absent.insert("bob".into(), saved(&s.clients[0]));
    links[0].send(ToServer::Hello { name: "ape".into() });
    links[0].send(ToServer::Chat("/give stone 1".into()));
    links[1].send(ToServer::Hello { name: "ape".into() });
    links[1].send(ToServer::Chat("/give dirt 2".into()));
    links[0].send(ToServer::Hello { name: "bob".into() });
    links[2].send(ToServer::Hello { name: "a\npe\u{7}".into() });
    s.tick();
    let names: Vec<&str> = s.clients.iter().map(|c| c.player.name.as_str()).collect();
    assert_eq!(names, ["ape", "ape-2", "ape-3"]);
    assert!(s.absent.contains_key("bob"), "a second Hello took nobody's saved state");
    drop(links);
    s.tick();
    let inv = |n: &str| held(&s.absent[n].slots);
    assert_eq!((inv("ape"), inv("ape-2")), (vec![(BLOCKS.id("stone"), 1)], vec![(BLOCKS.id("dirt"), 2)]));
}

/// The quit save waits for an autosave still writing: the two never share the temp file, and when
/// `save_now` returns the save is whole and nothing writes any more.
#[test]
fn save_now_right_after_an_autosave_leaves_a_whole_save() {
    let (mut s, _links) = server_with(1);
    let path = std::env::temp_dir().join(format!("technoape-autosave-{}.sav", std::process::id()));
    s.autosave = Some((path.clone(), 1));
    for _ in 0..100 {
        s.tick();
        s.save_now();
        assert!(!path.with_extension("tmp").exists(), "an autosave still writes after save_now");
        assert!(Save::read(&path).is_ok());
    }
    std::fs::remove_file(&path).unwrap();
}

/// ADR 0109: the far cells' summary is the client's (its far chunks); the server's World keeps none, and
/// its state hash is the one a world with the summary has.
#[test]
fn the_server_world_keeps_no_far_summary() {
    let (mut s, _links) = server_with(0);
    let mut client = World::new(256, 32, 1);
    client.open_far();
    for z in 0..8 {
        let b = Block { body: 0, face: 0, pos: glam::IVec3::new(100, 98, 3000 + z) };
        s.world.set_block(b, BLOCKS.id("lamp"));
        client.set_block(b, BLOCKS.id("lamp"));
    }
    assert!(s.world.far.is_none(), "the server keeps no far summary");
    assert!(client.far.as_ref().is_some_and(|f| !f.is_empty()));
    assert_eq!(s.world.hash(), client.hash());
}

/// A world of more edited cells than one `Edits` part carries reaches a joining client whole, in parts
/// that each stay under the TCP frame limit.
#[test]
fn a_join_to_a_world_of_many_edits_receives_them_all_in_parts() {
    let (tx, rx) = channel();
    let mut s = Server::new(256, 32, 1, rx);
    for k in 0..EDITS_PART as i32 + 5 {
        s.world.set_block(Block { body: 0, face: 0, pos: glam::IVec3::new(k % 256, (k / 256) % 256, 200 + k / 65536) }, BLOCKS.id("stone"));
    }
    let (c, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    let parts: Vec<usize> = drain(&c).into_iter().filter_map(|m| if let ToClient::Edits(v) = m { Some(v.len()) } else { None }).collect();
    assert!(parts.len() > 1 && parts.iter().all(|n| *n <= EDITS_PART), "{parts:?}");
    assert_eq!(parts.iter().sum::<usize>(), s.world.edit_count);
}

/// The player list goes out when it changes (a join, a name, a leave), not every tick.
#[test]
fn the_player_list_is_sent_when_it_changes_only() {
    let (mut s, mut links) = server_with(2);
    let lists = |l: &Link<ToServer, ToClient>| drain(l).into_iter().filter_map(|m| if let ToClient::Players(v) = m { Some(v.len()) } else { None }).collect::<Vec<_>>();
    assert_eq!(lists(&links[1]), [2], "the join");
    for _ in 0..5 {
        s.tick();
    }
    assert_eq!(lists(&links[1]), [0usize; 0], "nothing changed");
    links[0].send(ToServer::Hello { name: "ape".into() });
    s.tick();
    assert_eq!(lists(&links[1]), [2], "a name");
    drop(links.remove(0));
    s.tick();
    s.tick();
    assert_eq!(lists(&links[0]), [1], "a leave");
}

/// A client that floods the server gets MSGS_PER_TICK messages handled a tick, the rest on later ticks in
/// order; its leave is still seen once they are done.
#[test]
fn a_flood_of_messages_is_handled_a_part_a_tick_in_order() {
    let (mut s, mut links) = server_with(2);
    for k in 0..1000 {
        links[0].send(ToServer::Chat(format!("{k}")));
    }
    drop(links.remove(0));
    drain(&links[0]);
    s.tick();
    let mut got = chats(&drain(&links[0]));
    assert_eq!(got.len(), MSGS_PER_TICK);
    while s.players() == 2 {
        s.tick();
    }
    got.extend(chats(&drain(&links[0])));
    let want: Vec<String> = (0..1000).map(|k| format!("<?> {k}")).collect();
    assert_eq!(got, want);
    assert_eq!(s.tick, 1 + 1000_u64.div_ceil(MSGS_PER_TICK as u64));
}

/// The water tiles a client lacks, found from the cells near it, are the old scan over every tile's:
/// on a random set of tiles, versions, places and what each client has.
#[test]
fn water_wanted_from_near_cells_equals_the_scan_of_every_tile() {
    let (mut s, _links) = server_with(0);
    let mut r = 12345u64;
    let mut rnd = |n: u64| {
        r = r.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (r >> 33) % n
    };
    for t in 0..20 {
        s.tick = t;
        let cols: Vec<Col> = (0..200).map(|_| Col::new(0, rnd(6) as u8, rnd(256) as i32, rnd(256) as i32)).collect();
        s.water_changed(&cols);
    }
    let all: Vec<(Col, (u64, DVec3))> = s.tiles.iter().map(|(t, v)| (*t, *v)).collect();
    for _ in 0..50 {
        let (_, (_, at)) = all[rnd(all.len() as u64) as usize];
        let me = at + DVec3::new(rnd(200) as f64, rnd(200) as f64, rnd(200) as f64) - DVec3::splat(100.0);
        let have: FxMap<Col, u64> = all.iter().filter_map(|(t, _)| (rnd(3) == 0).then_some(*t)).collect::<Vec<_>>().into_iter().map(|t| (t, rnd(22))).collect();
        // the old scan: every tile, nearest first
        let mut old: Vec<(u64, Col)> = all
            .iter()
            .filter(|(t, (v, _))| have.get(t).is_none_or(|s| s < v))
            .map(|(t, (_, at))| ((at.distance(me) * 16.0) as u64, *t))
            .filter(|(d, _)| (*d as f64) < WATER_RANGE * 16.0)
            .collect();
        old.sort_unstable();
        assert!(!old.is_empty());
        assert_eq!(wanted(&s.tiles, &s.near, me, &have), old);
    }
}

#[test]
fn a_closed_link_removes_the_player() {
    let (mut s, mut links) = server_with(2);
    drop(links.remove(0));
    s.tick();
    assert_eq!(s.players(), 1);
}

/// Review of the crafting grid (2026-11-16): a client names a block; one on no body, no face or outside the grid
/// must be refused, not indexed (a made-up `Use` or `Take` took the server down).
#[test]
fn a_block_that_does_not_exist_is_refused_not_indexed() {
    let (mut s, links) = server_with(1);
    let (pole, under) = on_land(&s);
    links[0].send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    for bad in [Block { body: 9, ..under }, Block { face: 7, ..under }, Block { pos: glam::IVec3::new(-1, 0, 1 << 20), ..under }] {
        links[0].send(ToServer::Use(bad));
        links[0].send(ToServer::Take { seq: 1, at: bad });
        s.tick();
        assert!(s.clients[0].bench.is_none());
    }
}

/// The crucible's tests (ADR 0141): `n` survival players on land beside a crucible two cells off
/// (within `BENCH_REACH`), and its cell.
fn at_crucible(n: usize) -> (Server, Vec<Link<ToServer, ToClient>>, Block, DVec3) {
    let (mut s, links) = server_with(n);
    let (pole, under) = on_land(&s);
    for l in &links {
        l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    }
    let at = Block { pos: under.pos + glam::IVec3::new(2, 0, 1), ..under };
    s.world.set_block(at, BLOCKS.id("crucible"));
    s.tick();
    (s, links, at, pole)
}

/// Gives player `i` n of `item` (/give) and moves them into slot k of the furnace at `at`.
fn feed(s: &mut Server, l: &Link<ToServer, ToClient>, i: usize, at: Block, item: &str, n: u16, k: u16) {
    let id = ITEMS.named(item).unwrap();
    l.send(ToServer::Chat(format!("/give {} {n}", item.replace(' ', "_"))));
    s.tick();
    let a = (0..items::APE_SLOTS).find(|&a| s.clients[i].slots[a].is_some_and(|t| t.item == id && t.n >= n)).unwrap();
    l.send(ToServer::Items { seq: 1, from: (Holder::Ape, a as u16), to: (Holder::Furnace(at), k), n });
    s.tick();
}

fn furnace_of(s: &Server, at: Block) -> engine::furnace::Furnace {
    s.furnaces.get(&key(at)).cloned().unwrap_or_default()
}

/// A crucible used and fed copper gravel and a log melts it at 1085 C into ingots, the open window told the heat
/// as it rises, everyone told the fire; a mould and a cast chosen make a pick head and keep the mould. The
/// items' ledger is exact at every step; the fire goes out and the heat falls when the work is done.
#[test]
fn a_crucible_melts_gravel_and_casts_a_head_with_the_ledger_exact() {
    use engine::furnace::{FUEL, IN, MOULD, OUT};
    let (mut s, links, at, _) = at_crucible(2);
    let l = &links[0];
    let id = |n: &str| ITEMS.named(n).unwrap();
    l.send(ToServer::Use(at));
    s.tick();
    assert_eq!(s.clients[0].furnace, Some(at));
    feed(&mut s, l, 0, at, "copper gravel", 4, IN as u16);
    drain(&links[1]);
    assert!(!furnace_of(&s, at).burning(), "no fuel, no fire");
    // the log in: the fire is lit and everyone hears of it; the window's heat rises
    feed(&mut s, l, 0, at, "oak log", 2, FUEL as u16);
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    assert!(drain(&links[1]).iter().any(|m| matches!(m, ToClient::Fires(v) if v.contains(&(at, true)))));
    drain(l);
    wait(&mut s, 5.0);
    let heats: Vec<f32> = drain(l).iter().filter_map(|m| if let ToClient::Furnace { at: b, f } = m { (*b == at).then_some(f.heat) } else { None }).collect();
    assert!(heats.len() > 50 && heats.windows(2).all(|w| w[1] > w[0]) && heats[0] < 1085.0, "{heats:?}");
    // two ingots by 10.7 + 2 x 8 s, the next log not lit
    wait(&mut s, 22.0);
    let f = furnace_of(&s, at);
    assert_eq!((f.slots[IN], f.slots[OUT], f.slots[FUEL]), (None, Some(Stack::new(id("copper ingot"), 2)), Some(Stack::new(id("oak log"), 1))), "{f:?}");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // the ingots back into the input, a mould, the pick head chosen: one head, the mould kept
    l.send(ToServer::Items { seq: 1, from: (Holder::Furnace(at), OUT as u16), to: (Holder::Furnace(at), IN as u16), n: 2 });
    s.tick();
    l.send(ToServer::Chat("/give clay 2".into()));
    s.tick();
    let clay = (0..9).find(|&k| s.clients[0].slots[k].is_some_and(|t| t.item == id("clay"))).unwrap();
    shift(&mut s, l, clay, items::CRAFT_AT, 1);
    shift(&mut s, l, clay, items::CRAFT_AT + 1, 1);
    l.send(ToServer::Craft { recipe: "clay mould".into(), n: 1 });
    s.tick();
    let m = (0..9).find(|&k| s.clients[0].slots[k].is_some_and(|t| t.item == id("clay mould"))).unwrap();
    l.send(ToServer::Items { seq: 1, from: (Holder::Ape, m as u16), to: (Holder::Furnace(at), MOULD as u16), n: 1 });
    l.send(ToServer::Choose { at, cast: Some("cast pick head".into()) });
    wait(&mut s, 6.0);
    let f = furnace_of(&s, at);
    assert_eq!((f.slots[IN], f.slots[OUT], f.slots[MOULD]), (None, Some(Stack::new(id("copper pick head"), 1)), Some(Stack::new(id("clay mould"), 1))), "{f:?}");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // the head taken out; nothing put into the output; the fire out and the heat back at the air's in time
    l.send(ToServer::Items { seq: 1, from: (Holder::Furnace(at), OUT as u16), to: (Holder::Ape, 8), n: 1 });
    s.tick();
    assert_eq!(s.clients[0].slots[8], Some(Stack::new(id("copper pick head"), 1)));
    l.send(ToServer::Items { seq: 1, from: (Holder::Ape, 8), to: (Holder::Furnace(at), OUT as u16), n: 1 });
    s.tick();
    assert_eq!(furnace_of(&s, at).slots[OUT], None);
    drain(&links[1]);
    wait(&mut s, 120.0);
    let f = furnace_of(&s, at);
    assert!(!f.burning() && f.heat == engine::furnace::AIR_C, "{f:?}");
    assert!(drain(&links[1]).iter().any(|m| matches!(m, ToClient::Fires(v) if v.contains(&(at, false)))));
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// Draft ADR tin-and-bronze: over the wire, copper ingots into the second input and tin into the first melt
/// into bronze; a cast chosen by copper's row name casts a bronze head from it. The ledger is exact throughout.
#[test]
fn a_crucible_alloys_bronze_from_both_inputs_and_casts_it_by_shape() {
    use engine::furnace::{FUEL, IN, IN2, MOULD, OUT};
    let (mut s, links, at, _) = at_crucible(1);
    let l = &links[0];
    let id = |n: &str| ITEMS.named(n).unwrap();
    l.send(ToServer::Use(at));
    s.tick();
    feed(&mut s, l, 0, at, "copper ingot", 3, IN2 as u16);
    feed(&mut s, l, 0, at, "oak log", 1, FUEL as u16);
    wait(&mut s, 30.0);
    assert!(!furnace_of(&s, at).burning() && furnace_of(&s, at).slots[OUT].is_none(), "copper alone: nothing");
    feed(&mut s, l, 0, at, "tin ingot", 1, IN as u16);
    wait(&mut s, 30.0);
    let f = furnace_of(&s, at);
    assert_eq!((f.slots[IN], f.slots[IN2], f.slots[OUT]), (None, None, Some(Stack::new(id("bronze ingot"), 4))), "{f:?}");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    // the window told the second input as it changed
    assert!(drain(l).iter().any(|m| matches!(m, ToClient::Furnace { f, .. } if f.slots[IN2] == Some(Stack::new(id("copper ingot"), 3)))));
    l.send(ToServer::Items { seq: 1, from: (Holder::Furnace(at), OUT as u16), to: (Holder::Furnace(at), IN as u16), n: 4 });
    feed(&mut s, l, 0, at, "clay mould", 1, MOULD as u16);
    feed(&mut s, l, 0, at, "oak log", 1, FUEL as u16);
    l.send(ToServer::Choose { at, cast: Some("cast pick head".into()) });
    wait(&mut s, 30.0);
    let f = furnace_of(&s, at);
    assert_eq!((f.slots[IN], f.slots[OUT]), (None, Some(Stack::new(id("bronze pick head"), 2))), "{f:?}");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// The cost of 200 burning crucibles on the server's tick, once (principle 14): `cargo test --release -p server
/// two_hundred_crucibles -- --ignored --nocapture`. Prints ms per tick without and with them.
#[test]
#[ignore]
fn two_hundred_crucibles_cost() {
    use engine::furnace::{FUEL, IN};
    let (mut s, _links, at, _) = at_crucible(1);
    let time = |s: &mut Server| {
        let t = std::time::Instant::now();
        for _ in 0..600 {
            s.tick();
        }
        t.elapsed().as_secs_f64() * 1e3 / 600.0
    };
    let bare = time(&mut s);
    for k in 0..200 {
        let b = Block { pos: at.pos + glam::IVec3::new(k % 20, 0, 3 + k / 20), ..at };
        s.world.set_block(b, BLOCKS.id("crucible"));
        let mut f = engine::furnace::Furnace::default();
        f.slots[IN] = Some(Stack::new(ITEMS.named("copper gravel").unwrap(), 64));
        f.slots[FUEL] = Some(Stack::new(ITEMS.named("oak log").unwrap(), 64));
        s.furnaces.insert(key(b), f);
    }
    s.tick();
    let burning = s.furnaces.values().filter(|f| f.burning()).count();
    let with = time(&mut s);
    println!("{{\"crucibles\":{burning},\"tick_ms_bare\":{bare:.4},\"tick_ms_with\":{with:.4},\"per_crucible_us\":{:.3}}}", (with - bare) * 1e3 / burning as f64);
    assert_eq!(burning, 200);
}

/// Two players at one crucible: both may fill it and take from it, and each open window sees the other's move.
#[test]
fn two_players_work_one_crucible() {
    let (mut s, links, at, _) = at_crucible(2);
    for l in &links {
        l.send(ToServer::Use(at));
    }
    s.tick();
    feed(&mut s, &links[0], 0, at, "copper gravel", 2, 0);
    feed(&mut s, &links[1], 1, at, "copper gravel", 3, 0);
    let gravel = Stack::new(ITEMS.named("copper gravel").unwrap(), 5);
    assert_eq!(furnace_of(&s, at).slots[0], Some(gravel));
    for l in &links {
        let seen = drain(l).into_iter().filter_map(|m| if let ToClient::Furnace { f, .. } = m { Some(f.slots[0]) } else { None }).last();
        assert_eq!(seen, Some(Some(gravel)));
    }
    links[1].send(ToServer::Items { seq: 1, from: (Holder::Furnace(at), 0), to: (Holder::Ape, 5), n: 5 });
    s.tick();
    assert_eq!((furnace_of(&s, at).slots[0], s.clients[1].slots[5]), (None, Some(gravel)));
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// A crucible dug with all five slots full drops everything it held and itself; nothing is lost, the ledger exact.
#[test]
fn a_crucible_dug_full_drops_all_it_held() {
    let (mut s, links, at, _) = at_crucible(1);
    let l = &links[0];
    let id = |n: &str| ITEMS.named(n).unwrap();
    // (flint and tin in: nothing to work, so no log burns while it is dug)
    let held: [(&str, u16); 5] = [("flint", 3), ("oak log", 2), ("clay mould", 1), ("copper ingot", 4), ("tin ingot", 2)];
    let mut f = engine::furnace::Furnace::default();
    for (k, (n, c)) in held.iter().enumerate() {
        f.slots[k] = Some(Stack::new(id(n), *c));
        *s.item_made.entry(id(n)).or_default() += *c as i64;
    }
    s.furnaces.insert(key(at), f);
    assert!(s.item_ledger().is_empty());
    let one = Shape::one(at);
    l.send(ToServer::Dig(Some(one)));
    s.tick();
    let need = edit::dig_time(&s.world, &one);
    wait(&mut s, need + 0.2);
    l.send(ToServer::Edit { seq: 1, shape: one, material: AIR });
    s.tick();
    assert_eq!(s.world.block(at), AIR);
    assert!(s.furnaces.is_empty());
    let all: Vec<(u16, u32)> = {
        let mut m: BTreeMap<u16, u32> = BTreeMap::new();
        for st in s.piles.values().flat_map(|p| p.slots.iter().flatten()).chain(s.clients[0].slots.iter().flatten()) {
            *m.entry(st.item).or_default() += st.n as u32;
        }
        m.into_iter().collect()
    };
    for (n, c) in held.iter().chain(&[("crucible", 1)]) {
        assert!(all.contains(&(id(n), *c as u32)), "{n} x {c}: {all:?}");
    }
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// A save mid-melt loads to the same furnace (slots, heat, fire, work) and both carry on alike; the ledger holds.
#[test]
fn a_save_mid_melt_carries_on() {
    let (mut s, links, at, _) = at_crucible(1);
    feed(&mut s, &links[0], 0, at, "copper nugget", 8, 0);
    feed(&mut s, &links[0], 0, at, "pine log", 1, 1);
    wait(&mut s, 14.0);
    let f = furnace_of(&s, at);
    assert!(f.burning() && f.heat >= 1085.0 && f.work > 0.0 && f.slots[3].is_none(), "{f:?}");
    let mut back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!(furnace_of(&back, at), f);
    assert!(back.item_ledger().is_empty(), "{:?}", back.item_ledger());
    for _ in 0..200 {
        s.tick();
        back.tick();
    }
    assert_eq!(furnace_of(&back, at), furnace_of(&s, at));
    assert!(furnace_of(&back, at).slots[3].is_some_and(|o| o.n >= 1));
    assert!(back.item_ledger().is_empty(), "{:?}", back.item_ledger());
}

/// A furnace's slots and choice are reached only within `BENCH_REACH` of a crucible there: a use, a move or a
/// choice from 20 m, at a block that is no crucible, or at a made-up block, changes nothing and opens nothing.
#[test]
fn a_far_or_made_up_crucible_is_refused() {
    let (mut s, links, at, pole) = at_crucible(1);
    let l = &links[0];
    l.send(ToServer::Chat("/give copper_gravel 4".into()));
    s.tick();
    let near = s.clients[0].slots.clone();
    let tries = |s: &mut Server, b: Block| {
        l.send(ToServer::Use(b));
        l.send(ToServer::Items { seq: 1, from: (Holder::Ape, 0), to: (Holder::Furnace(b), 0), n: 4 });
        l.send(ToServer::Choose { at: b, cast: Some("cast pick head".into()) });
        s.tick();
    };
    let under = Block { pos: at.pos - glam::IVec3::Z, ..at };
    for b in [under, Block { body: 9, ..at }, Block { face: 7, ..at }, Block { pos: glam::IVec3::new(-1, 0, 1 << 20), ..at }] {
        tries(&mut s, b);
    }
    let away = pole + DVec3::X * 20.0;
    l.send(ToServer::Move { pos: away.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    tries(&mut s, at);
    assert!(s.furnaces.is_empty() && s.clients[0].furnace.is_none() && s.clients[0].slots == near);
    // a cast that is no cast is refused at the crucible too
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Choose { at, cast: Some("copper from gravel".into()) });
    s.tick();
    assert!(furnace_of(&s, at).choice.is_none());
    // and walking away closes an open one
    l.send(ToServer::Use(at));
    s.tick();
    assert_eq!(s.clients[0].furnace, Some(at));
    l.send(ToServer::Move { pos: away.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    assert!(s.clients[0].furnace.is_none());
}
