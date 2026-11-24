use super::*;
use engine::net::local;
use std::sync::mpsc::channel;

mod spectator;

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

/// The events among `got`, in order (ADR 0149).
fn events(got: &[ToClient]) -> Vec<Event> {
    got.iter().filter_map(|m| if let ToClient::Events(v) = m { Some(v.clone()) } else { None }).flatten().collect()
}

/// A tick never waits on rayon's global pool: in a game's own server that pool is the client's, full of chunk
/// jobs and of every world build's cave walk (a sim's 50 bots' held the tick 100 to 600 ms). With every thread of
/// the pool held, a tick of a server with many players (the water pass went side by side from 8) and a flood
/// (the water's step from 2048 awake nodes) still ends.
#[test]
fn a_tick_does_not_wait_on_the_global_pool() {
    let (mut s, _links) = server_with(9);
    // and a flood: the water steps side by side from 2048 awake nodes
    flood(&mut s, 25);
    s.world.step_water();
    assert!(s.world.water.awake() > 2048, "{} awake", s.world.water.awake());
    let threads = rayon::current_num_threads();
    let gate = std::sync::Arc::new(std::sync::RwLock::new(()));
    let held = gate.write().unwrap();
    let (started_tx, started) = channel();
    for _ in 0..threads {
        let (gate, started_tx) = (gate.clone(), started_tx.clone());
        rayon::spawn(move || {
            started_tx.send(()).unwrap();
            drop(gate.read().unwrap());
        });
    }
    // every pool thread is in a task that waits on the gate
    for _ in 0..threads {
        started.recv().unwrap();
    }
    let (done_tx, done) = channel();
    let t = std::thread::spawn(move || {
        s.tick();
        done_tx.send(()).unwrap();
    });
    // generous: a tick here is about a millisecond; one that needs the pool never ends while it is held
    let ended = done.recv_timeout(std::time::Duration::from_secs(10)).is_ok();
    drop(held);
    t.join().unwrap();
    assert!(ended, "the tick waited on rayon's global pool");
}

/// The event list (ADR 0149): a dig is an event, with what was there and who dug, sent to every player
/// before the edit that shows it, far ones too; a death is told only within `PERCEIVE`.
#[test]
fn a_dig_is_an_event_for_every_player_before_its_edit_and_a_death_only_near() {
    let (mut s, links) = server_with(3);
    let (pole, over) = on_land(&s);
    let under = Block { pos: over.pos - glam::IVec3::Z, ..over };
    let b = &s.world.bodies[0];
    // high over the pole, in the air
    let far = b.centre + (pole - b.centre) * 2.0;
    for (l, p) in links.iter().zip([pole, pole + DVec3::X * 3.0, far]) {
        l.send(ToServer::Creative(true));
        l.send(ToServer::Move { pos: p.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    }
    s.tick();
    links.iter().for_each(|l| drop(drain(l)));
    assert!(s.clients[2].phys.pos.distance(pole) > engine::events::PERCEIVE + 10.0);
    let was = s.world.block(under);
    assert_ne!(was, AIR);
    links[0].send(ToServer::Edit { seq: 1, shape: Shape::one(under), material: AIR });
    s.tick();
    let dug = Event::Edit { at: under, was, now: AIR, by: Some(s.clients[0].player.id) };
    for (k, l) in links.iter().enumerate() {
        let got = drain(l);
        let edit = got.iter().position(|m| matches!(m, ToClient::Edits(v) if v.contains(&(under, AIR)))).expect("everyone is sent the edit");
        let told = got.iter().position(|m| matches!(m, ToClient::Events(v) if v.contains(&dug)));
        // (the review: an edit's and a fall's event go wherever their edit goes, far too, as before the list
        // every edit played for every client)
        assert!(told.is_some_and(|e| e < edit), "{k}: the event before its edit: {got:?}");
    }
    // the other kinds only within PERCEIVE: a death near the pole is not told high over it
    let id = s.clients[0].player.id;
    s.send_events(&[Event::Died { by: id }]);
    let told: Vec<bool> = links.iter().map(|l| !events(&drain(l)).is_empty()).collect();
    assert_eq!(told, vec![true, true, false]);
}

/// The review of the event list: a tick's events, more than one message holds, come in parts of at most
/// `EDITS_PART`, in order, every one of them.
#[test]
fn many_events_come_in_parts_in_order() {
    let (s, links) = server_with(1);
    let at = |k: i32| Block { body: 0, face: 0, pos: glam::IVec3::new(k % 256, (k / 256) % 256, 200) };
    let ev: Vec<Event> = (0..EDITS_PART as i32 + 5).map(|k| Event::Edit { at: at(k), was: AIR, now: 1, by: None }).collect();
    drain(&links[0]);
    s.send_events(&ev);
    let parts: Vec<Vec<Event>> = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::Events(v) = m { Some(v) } else { None }).collect();
    assert!(parts.len() > 1 && parts.iter().all(|p| p.len() <= EDITS_PART), "{:?}", parts.iter().map(Vec::len).collect::<Vec<_>>());
    assert_eq!(parts.concat(), ev);
}

/// The review of the event list: a falling stack of three (the lowest moves first, each above it into the
/// cell the one under it left, `Falls::step`), through the protocol: every move is a `Fall` sent before its
/// edits, and the edit that brings its block to its cell comes after the edit that emptied the cell.
#[test]
fn a_falling_stack_is_a_fall_for_each_move_before_its_edits() {
    let (mut s, links) = server_with(2);
    let (pole, under) = on_land(&s);
    let shelf = Block { pos: under.pos + glam::IVec3::new(3, 0, 5), ..under };
    let at = |k: i32| Block { pos: shelf.pos + glam::IVec3::Z * k, ..shelf };
    let (stone, sand, gravel) = (BLOCKS.id("stone"), BLOCKS.id("sand"), BLOCKS.id("gravel"));
    s.world.set_block(shelf, stone);
    for (k, m) in [(1, sand), (2, sand), (3, gravel)] {
        s.world.set_block(at(k), m);
    }
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
    let falls: Vec<(usize, Block, Block, u16)> = got.iter().enumerate().filter_map(|(i, m)| if let ToClient::Events(v) = m { Some((i, v.clone())) } else { None }).flat_map(|(i, v)| v.into_iter().filter_map(move |e| if let Event::Fall { from, to, block } = e { Some((i, from, to, block)) } else { None })).collect();
    assert!(falls.iter().any(|f| f.3 == gravel) && falls.iter().filter(|f| f.3 == sand).count() >= 2, "every block of the stack fell: {falls:?}");
    // in the first tick of the fall, the three move together: the stack's cells each emptied and filled
    let first: Vec<_> = falls.iter().filter(|f| f.0 == falls[0].0).collect();
    assert_eq!(first.iter().map(|f| (f.1, f.2, f.3)).collect::<Vec<_>>(), vec![(at(1), shelf, sand), (at(2), at(1), sand), (at(3), at(2), gravel)], "lowest first");
    for &(i, from, to, block) in &falls {
        // the edits after the event's message, up to the next events
        let edits: Vec<(Block, u16)> = got[i + 1..].iter().take_while(|m| !matches!(m, ToClient::Events(_))).filter_map(|m| if let ToClient::Edits(v) = m { Some(v.clone()) } else { None }).flatten().collect();
        let left = edits.iter().position(|e| *e == (from, AIR)).unwrap_or_else(|| panic!("{from:?} left: {edits:?}"));
        let came = edits.iter().position(|e| *e == (to, block)).unwrap_or_else(|| panic!("{to:?} came: {edits:?}"));
        assert_eq!(came, left + 1, "a move's two edits side by side: {from:?} -> {to:?}");
    }
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
        assert!(matches!(drain(l)[0], ToClient::Welcome { seed: 1, ref bodies, .. } if bodies[0].0 == 256));
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
    let edits: Vec<(Block, u16)> = got.iter().filter_map(|m| if let ToClient::Edits(v) = m { Some(v.clone()) } else { None }).flatten().collect();
    assert!(edits.contains(&(over, AIR)) && edits.contains(&(rest, sand)), "{edits:?}");
    // the events (ADR 0149): the shelf's dig, then every move a fall, from the sand's cell to where it lies
    let ev = events(&got);
    assert_eq!(ev.first(), Some(&Event::Edit { at: shelf, was: stone, now: AIR, by: Some(s.clients[0].player.id) }), "{ev:?}");
    let falls: Vec<(Block, Block, u16)> = ev.iter().filter_map(|e| if let Event::Fall { from, to, block } = *e { Some((from, to, block)) } else { None }).collect();
    assert!(falls.first().is_some_and(|f| f.0 == over) && falls.last().is_some_and(|f| f.1 == rest) && falls.iter().all(|f| f.2 == sand), "{falls:?}");
    assert!(falls.windows(2).all(|w| w[0].1 == w[1].0), "one cell to the next: {falls:?}");
    assert_eq!(ev.len(), falls.len() + 1, "nothing else: {ev:?}");
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
    // the dig is the player's event, the torch's drop the world's (ADR 0149)
    let by = Some(s.clients[0].player.id);
    assert_eq!(events(&got), vec![Event::Edit { at: at(3, 3), was: stone, now: AIR, by }, Event::Edit { at: wall, was: torch, now: AIR, by: None }]);
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
    let (mut seq, mut seen) = (0, Vec::new());
    let mut dig = |s: &mut Server, b: Block, secs: f32| {
        seq += 1;
        let one = Shape::one(b);
        l.send(ToServer::Dig(Some(one)));
        s.tick();
        wait(s, secs);
        l.send(ToServer::Edit { seq, shape: one, material: AIR });
        s.tick();
        l.send(ToServer::Dig(None));
        let got = drain(l);
        seen.extend(events(&got));
        got.contains(&ToClient::Done { seq, ok: true })
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
    // the wear is an event, once, at the last use (ADR 0149: the client's snap plays from it)
    let worn: Vec<&Event> = seen.iter().filter(|e| matches!(e, Event::Worn { .. })).collect();
    assert_eq!(worn, vec![&Event::Worn { item: axe, by: s.clients[0].player.id }]);
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
        drain(l);
        l.send(ToServer::Craft { recipe: recipe.into(), n });
        s.tick();
        (held(&s.clients[0].slots), engine::recipes::grid(&s.clients[0].slots))
    };
    assert_eq!(craft(&mut s, "copper axe", 1).0, vec![], "the grid holds no axe: nothing");
    assert!(events(&drain(l)).is_empty(), "nothing made, no event");
    // three asked: two made, then the head's cell is empty
    let (got, cells) = craft(&mut s, "copper pick", 3);
    assert_eq!((got, cells[1], cells[4]), (vec![(pick, 2)], None, Some(stick)));
    assert_eq!(events(&drain(l)), vec![Event::Craft { recipe: "copper pick".into(), n: 2, by: s.clients[0].player.id }], "a craft is an event (ADR 0149)");
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
        name: name.into(), pos: at.to_array(), fwd: [1.0, 0.0, 0.0], creative: false, slots: items::from_counts(&[(2, 5)]), water: 0, vitals: Default::default(), marks: Default::default(), journal: Vec::new(),
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

/// A client or a bot builds its world from the welcome (ADR 0160-generation-number-and-arc-basalt): told
/// generation 1 it has the arcs' tuff, told 2 their basalt flows over part of them, and the same rock in every
/// column of every other province.
#[test]
fn a_client_told_generation_1_2_or_3_builds_that_worlds_rock() {
    let worlds = [1u8, 2, 3].map(|gen| {
        let (tx, rx) = channel();
        let mut s = Server::baked_warp(1536, 32, 7, 0.0, gen, rx);
        let (c, sv) = local();
        tx.send(sv).unwrap();
        s.tick();
        let welcome = drain(&c).into_iter().find_map(|m| match m {
            ToClient::Welcome { seed, bodies, gen, .. } => Some((seed, bodies[0].0, bodies[1].0, gen)),
            _ => None,
        });
        let (seed, planet_n, moon_n, told) = welcome.expect("a welcome");
        assert_eq!(told, gen, "the welcome says the world's generation");
        // as the bot and the client do on a welcome: the same ground as the server's, caves and all
        // the map of the generation told (`get_at`: version 7 before real gravity), as the welcome's hash fetches it
        let w = World::with_grid(planet_n, moon_n, seed, Some(engine::bake::get_at(seed, planet_n, told)), None, 0.0, told);
        assert!(w.bodies[0].caves == s.world.bodies[0].caves, "generation {gen}: the client's caves differ from the server's");
        w
    });
    // generation 3 is generation 2's rock with karst in it
    assert!(worlds[2].bodies[0].caves != worlds[1].bodies[0].caves, "generation 3 has the caves of 2");
    let (b1, b2) = (&worlds[0].bodies[0], &worlds[1].bodies[0]);
    let (mut arc, mut other) = (0, 0);
    for f in 0..6u8 {
        for i in (3..b1.n).step_by(7) {
            for j in (3..b1.n).step_by(7) {
                let (r1, r2, g) = (b1.col_rock(f, i, j), b2.col_rock(f, i, j), b1.col_height(f, i, j));
                let differ = (0..40).any(|k| b1.rock(r1, (g - k as f64) as f32, 0) != b2.rock(r2, (g - k as f64) as f32, 0));
                if r1.province == engine::bake::rock::ARC {
                    arc += differ as usize;
                } else {
                    assert!(!differ, "the crust of a {} column differs at ({f}, {i}, {j})", engine::bake::rock::NAMES.get(r1.province as usize).unwrap_or(&"none"));
                    other += 1;
                }
            }
        }
    }
    assert!(arc > 50 && other > 10_000, "{arc} arc columns differ, {other} others the same");
}

/// A recording from the seed replays on the grid and by the generation it was made by (ADR 0152; ADR 0160-generation-number-and-arc-basalt):
/// generation 0 on the old floor, 1 and 2 on the deep grid, 2 with its own ground; a dig in each at the same cell.
#[test]
fn a_recording_replays_with_its_generation() {
    for gen in [0, 1, 2, 3] {
        let (tx, rx) = channel();
        let mut s = Server::baked_warp(1536, 32, 7, 0.0, gen, rx);
        let rec = std::env::temp_dir().join(format!("technoape-test-{}-{gen}.rec", std::process::id()));
        s.record(&rec).unwrap();
        let (c, sv) = local();
        tx.send(sv).unwrap();
        let b = &s.world.bodies[0];
        let ground = b.centre + b.spawn * b.top(b.spawn);
        let deep = s.world.block_at(ground - b.spawn * 3.0);
        c.send(ToServer::Move { pos: (ground + b.spawn).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        c.send(ToServer::Creative(true));
        c.send(ToServer::Edit { seq: 1, shape: Shape::one(deep), material: AIR });
        for _ in 0..41 {
            s.tick();
        }
        assert_eq!(s.world.block(deep), AIR);
        let r = replay(&rec, None).unwrap();
        std::fs::remove_file(&rec).unwrap();
        assert_eq!((r.diverged, r.server.world.bodies[0].gen, r.server.world.bodies[0].mantle, r.server.world.hash()), (None, gen, s.world.bodies[0].mantle, s.world.hash()), "generation {gen}");
    }
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
    world.body_mut(0).set_ores(vec![lump]);
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
    world.body_mut(0).set_ores(vec![lump]);
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

/// Things on the ground aimed at by their own faces (ADR 0144): a take aimed past a pick-up, at the
/// ground beside it, names a cell where nothing lies and the server refuses it; one aimed at it takes it.
#[test]
fn a_take_aimed_at_a_thing_takes_it_and_one_aimed_past_it_does_not() {
    use engine::clutter::{aim_thing, pickup_at, Still, CLUTTER};
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    drain(&l);
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let pal = engine::palette::Palette::parse(&std::fs::read_to_string(data.join("palette.toml")).unwrap()).unwrap();
    let still = Still::load(&data.join("models"), &data.join("plants"), &pal).0;
    let w = &s.world;
    let b = &w.bodies[0];
    let (f, si, sj) = b.locate(b.spawn);
    let (f, si, sj) = (f as u8, si as i32, sj as i32);
    let mut found = None;
    'search: for (i, j) in (-80..=80).flat_map(|a| (-80..=80).map(move |c| (si + a, sj + c))) {
        let tile = b.tile(f, 0, i.div_euclid(engine::world::CS), j.div_euclid(engine::world::CS));
        let Some((z, _, _)) = engine::clutter::ground(b, &tile, f, i, j) else { continue };
        let at = Block { body: 0, face: f, pos: glam::IVec3::new(i, j, z) };
        let Some(th) = pickup_at(w, at, 0.0) else { continue };
        let up = b.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5);
        let eye = w.block_centre(at) + up * 1.8 + up.any_orthonormal_vector() * 0.3;
        let mid = engine::clutter::thing_faces(w, &still, at, &th).iter().flatten().sum::<DVec3>() / (engine::clutter::thing_faces(w, &still, at, &th).len() * 3) as f64;
        if aim_thing(w, &still, eye, (mid - eye).normalize(), 8.0, 0.0, 0.0, |_, _| true).map(|a| a.0) != Some(at) {
            continue;
        }
        // past it: the ground of a neighbour column where nothing lies, the block ray's cell before it
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let side = Block { pos: at.pos + glam::IVec3::new(di, dj, 0), ..at };
            let dir = (w.block_centre(side) - up * 0.45 - eye).normalize();
            if pickup_at(w, side, 0.0).is_some() || aim_thing(w, &still, eye, dir, 8.0, 0.0, 0.0, |_, _| true).is_some() {
                continue;
            }
            if let Some((_, before)) = engine::edit::aim(w, eye, dir, 8.0) {
                found = Some((at, th, eye - up * engine::phys::EYE, before));
                break 'search;
            }
        }
    }
    let (at, th, feet, past) = found.expect("a pick-up with open ground beside it near the spawn");
    let item = CLUTTER[th.row].item().unwrap().0;
    l.send(ToServer::Move { pos: feet.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    l.send(ToServer::Take { seq: 1, at: past });
    s.tick();
    assert!(drain(&l).contains(&ToClient::Done { seq: 1, ok: false }), "aimed past it: refused");
    assert_eq!(items::count(&s.clients[0].slots, item), 0);
    l.send(ToServer::Take { seq: 2, at });
    s.tick();
    let got = drain(&l);
    assert!(got.contains(&ToClient::Done { seq: 2, ok: true }), "aimed at it: taken");
    assert_eq!(events(&got), vec![Event::Taken { at, row: th.row as u16, by: s.clients[0].player.id }], "a take is an event (ADR 0149)");
    assert_eq!(items::count(&s.clients[0].slots, item), 1);
}

/// What clutter a column holds at time 0, for the plant tests (ADR 0147): its air cell over the ground, a
/// pick-up lying there now, whether a pick-up lies there in any season, and the plant at the most Vegetation. Columns within `r` of the spawn, near first.
struct Col0 {
    at: Block,
    now: Option<engine::clutter::Thing>,
    any: bool,
    plant: Option<engine::clutter::Thing>,
}
fn columns_near_spawn(s: &Server, r: i32) -> Vec<Col0> {
    use engine::clutter::{pick, pickup_at, plant, Kind, CLUTTER, VEGETATION, WADE};
    let w = &s.world;
    let b = &w.bodies[0];
    let seed = b.baked_seed().unwrap();
    let (f, si, sj) = b.locate(b.spawn);
    let (f, si, sj) = (f as u8, si as i32, sj as i32);
    let mut cells: Vec<(i32, i32)> = (-r..=r).flat_map(|a| (-r..=r).map(move |c| (si + a, sj + c))).collect();
    cells.sort_by_key(|&(i, j)| (i - si).pow(2) + (j - sj).pow(2));
    cells
        .into_iter()
        .filter_map(|(i, j)| {
            let tile = b.tile(f, 0, i.div_euclid(engine::world::CS), j.div_euclid(engine::world::CS));
            let (z, top, biome) = engine::clutter::ground_under(b, &tile, f, i, j, WADE)?;
            let at = Block { body: 0, face: f, pos: glam::IVec3::new(i, j, z) };
            // the first air over the generated ground (not a trunk's cell)
            if w.block(at) != 0 || w.block(Block { pos: at.pos - glam::IVec3::Z, ..at }) != top {
                return None;
            }
            let wet = b.water_at(f, i, j).is_some_and(|w| z as f64 <= w.0);
            let any = !wet && (0..4).any(|k| pick(b, seed, f, i, j, biome, top, k, 1.0, &tile.trees).is_some_and(|t| CLUTTER[t.row].kind == Kind::Pickup));
            let up = b.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5);
            let plant = plant(seed, f, i, j, biome, top, engine::clutter::part(up, 0.0, b.year) / 2, VEGETATION[2], &tile.trees).filter(|g| !wet || CLUTTER[g.row].wades);
            Some(Col0 { at, now: pickup_at(w, at, 0.0), any, plant })
        })
        .collect()
}

/// A server with one player standing beside air cell `at`, and that player's link.
fn beside(at_of: impl Fn(&Server) -> Block) -> (Server, Link<ToServer, ToClient>, Block) {
    beside_in(7, |s| Some(at_of(s))).unwrap()
}

/// As `beside` on the world of `seed`, None when `at_of` finds nothing there.
fn beside_in(seed: i32, at_of: impl Fn(&Server) -> Option<Block>) -> Option<(Server, Link<ToServer, ToClient>, Block)> {
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, seed, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    drain(&l);
    let at = at_of(&s)?;
    let b = &s.world.bodies[0];
    let up = b.dir(at.face as usize, at.pos.x as f64 + 0.5, at.pos.y as f64 + 0.5);
    let pos = b.centre + up * (b.layer_r(at.pos.z as f64) + 0.5) + up.any_orthonormal_vector() * 1.5;
    l.send(ToServer::Move { pos: pos.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    drain(&l);
    Some((s, l, at))
}

/// A take, a tick: whether the server took or cut, and whether it sent an edit.
fn take_once(s: &mut Server, l: &Link<ToServer, ToClient>, seq: u32, at: Block) -> (bool, bool) {
    l.send(ToServer::Take { seq, at });
    s.tick();
    let got = drain(l);
    (got.contains(&ToClient::Done { seq, ok: true }), got.iter().any(|m| matches!(m, ToClient::Edits(v) if v.iter().any(|e| e.0 == at))))
}

/// A plant on a column that holds a pick-up in any season is not cut (ADR 0147): the cut's air edit would
/// hide the pick-up for good. The stone under the grass is taken, and the grass then refused; a grass whose
/// column's pick-up is out in another season is refused now. A plant that gives nothing (flowers, wild wheat, a
/// berry bush) is not cut either: no edit, nothing in the slots.
#[test]
fn a_plant_over_a_pick_up_or_giving_nothing_is_not_cut() {
    use engine::clutter::CLUTTER;
    let gives = |c: &Col0| c.plant.is_some_and(|p| CLUTTER[p.row].item().is_some());
    // the stone under the grass: taken, then the grass is refused
    let (mut s, l, at) = beside(|s| columns_near_spawn(s, 80).into_iter().find(|c| c.now.is_some() && gives(c)).expect("a pick-up under a plant that gives").at);
    assert_eq!(take_once(&mut s, &l, 1, at), (true, false), "the pick-up is taken");
    assert_eq!(take_once(&mut s, &l, 2, at), (false, false), "the plant over a taken pick-up is not cut");
    // a pick-up of another season: the plant is refused now
    let (mut s, l, at) = beside(|s| columns_near_spawn(s, 80).into_iter().find(|c| c.now.is_none() && c.any && gives(c)).expect("a plant on a column with a pick-up in another season").at);
    assert_eq!(take_once(&mut s, &l, 1, at), (false, false), "a column whose pick-up is out in another season keeps its plant");
    // a plant that gives nothing
    let (mut s, l, at) = beside(|s| columns_near_spawn(s, 80).into_iter().find(|c| !c.any && c.plant.is_some() && !gives(c)).expect("a plant that gives nothing").at);
    assert_eq!(take_once(&mut s, &l, 1, at), (false, false), "a plant that gives nothing is not cut");
    assert!(s.clients[0].slots.iter().all(|x| x.is_none()), "nothing given");
}

/// Reeds standing in water are cut (ADR 0147): the fibre arrives and the water in and around the cell is
/// as it was, nothing woken.
#[test]
fn reeds_in_water_are_cut_and_the_water_is_unchanged() {
    use engine::clutter::CLUTTER;
    // reeds grow only in wetland, rare on the small planet: every face's fresh water columns (asked first:
    // cheap), seed after seed
    let (mut s, l, at) = (1..40)
        .find_map(|seed| {
            beside_in(seed, |s| {
                let w = &s.world;
                let b = &w.bodies[0];
                let n = b.n;
                (0..6u8)
                    .flat_map(|f| (0..n).flat_map(move |i| (0..n).map(move |j| (f, i, j))))
                    .filter(|&(f, i, j)| b.water_at(f, i, j).is_some_and(|w| w.1 != engine::liquid::SEA))
                    .find_map(|(f, i, j)| {
                        let tile = b.tile(f, 0, i.div_euclid(engine::world::CS), j.div_euclid(engine::world::CS));
                        let (z, _, _) = engine::clutter::ground_under(b, &tile, f, i, j, engine::clutter::WADE)?;
                        let at = Block { body: 0, face: f, pos: glam::IVec3::new(i, j, z) };
                        let th = engine::clutter::thing_at(w, at, 0.0, engine::clutter::VEGETATION[2], false, |_, _| true)?;
                        (CLUTTER[th.row].wades && b.water_at(f, i, j).is_some_and(|w| z as f64 <= w.0)).then_some(at)
                    })
            })
        })
        .expect("reeds in water");
    // the water in and around the cell, as the game sees it: a stored column's surface, else generation's
    let around = |s: &Server| {
        use engine::liquid::Ground;
        let w = &s.world;
        let g = engine::world::Solid::at(&w.bodies, &w.edits, &w.sky, w.time);
        let mut v = Vec::new();
        for (di, dj) in (-1..=1).flat_map(|a| (-1..=1).map(move |c| (a, c))) {
            let c = engine::world::World::col(Block { pos: at.pos + glam::IVec3::new(di, dj, 0), ..at });
            let (lo, hi) = g.generated(c);
            v.push(match w.water.column(c) {
                Some(mut n) => n.find(|n| n.water > 0).map(|n| n.surface()),
                None => (lo < hi).then_some(hi),
            });
        }
        (v, w.water.counted())
    };
    let (before, awake) = (around(&s), s.world.water.awake());
    assert!(before.0[4].is_some(), "water over the reeds' cell");
    let (fibre, n) = CLUTTER[engine::clutter::thing_at(&s.world, at, 0.0, engine::clutter::VEGETATION[2], false, |_, _| true).unwrap().row].item().unwrap();
    assert_eq!(take_once(&mut s, &l, 1, at), (true, true), "the reeds are cut");
    assert_eq!(items::count(&s.clients[0].slots, fibre), n as u32);
    assert_eq!(around(&s), before, "the water in and around the cell is as it was");
    assert!(s.world.water.awake() <= awake, "nothing woken: {} awake, {awake} before", s.world.water.awake());
}

/// A plant cut (ADR 0147): a take named at a plant that gives fibre puts the fibre in the slots, sends
/// the cell to everyone as an edit, leaves nothing there to cut again, and the save keeps it cut.
#[test]
fn a_plant_cut_gives_fibre_is_gone_for_everyone_and_stays_gone_in_the_save() {
    use engine::clutter::{thing_at, Kind, CLUTTER, VEGETATION};
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    let (l2, sv2) = local();
    tx.send(sv2).unwrap();
    s.tick();
    drain(&l);
    drain(&l2);
    let veg = VEGETATION[2];
    let (at, th) = {
        let w = &s.world;
        let b = &w.bodies[0];
        let (f, si, sj) = b.locate(b.spawn);
        let (f, si, sj) = (f as u8, si as i32, sj as i32);
        (-60..=60)
            .flat_map(|a| (-60..=60).map(move |c| (si + a, sj + c)))
            .find_map(|(i, j)| {
                let tile = b.tile(f, 0, i.div_euclid(engine::world::CS), j.div_euclid(engine::world::CS));
                let (z, _, _) = engine::clutter::ground_under(b, &tile, f, i, j, engine::clutter::WADE)?;
                let at = Block { body: 0, face: f, pos: glam::IVec3::new(i, j, z) };
                let th = thing_at(w, at, 0.0, veg, false, |_, _| true)?;
                (CLUTTER[th.row].kind == Kind::Plant && CLUTTER[th.row].item().is_some()).then_some((at, th))
            })
            .expect("a plant giving fibre near the spawn")
    };
    let (fibre, n) = CLUTTER[th.row].item().unwrap();
    assert_eq!(fibre, items::ITEMS.named("plant fibre").unwrap());
    let b = &s.world.bodies[0];
    let up = b.dir(at.face as usize, at.pos.x as f64 + 0.5, at.pos.y as f64 + 0.5);
    let pos = b.centre + up * (b.layer_r(at.pos.z as f64) + 0.5) + up.any_orthonormal_vector() * 1.5;
    l.send(ToServer::Move { pos: pos.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    l.send(ToServer::Take { seq: 1, at });
    s.tick();
    let got = drain(&l);
    assert!(got.contains(&ToClient::Done { seq: 1, ok: true }), "{got:?}");
    // a cut is an event; and its cell's edit too, sent before the edit (the review: as every edit's; the cell was
    // air and is air, so as before the list nothing breaks and nothing is heard)
    let by = s.clients[0].player.id;
    assert_eq!(events(&got), vec![Event::Edit { at, was: engine::blocks::AIR, now: engine::blocks::AIR, by: Some(by) }, Event::Taken { at, row: th.row as u16, by }], "{got:?}");
    let first = |p: &dyn Fn(&ToClient) -> bool| got.iter().position(p).unwrap();
    assert!(first(&|m| matches!(m, ToClient::Events(_))) < first(&|m| matches!(m, ToClient::Edits(v) if v.contains(&(at, engine::blocks::AIR)))));
    assert_eq!(items::count(&s.clients[0].slots, fibre), n as u32);
    assert!(drain(&l2).iter().any(|m| matches!(m, ToClient::Edits(v) if v.contains(&(at, engine::blocks::AIR)))), "the other player is sent the cut");
    assert!(thing_at(&s.world, at, 0.0, veg, false, |_, _| true).is_none(), "nothing left to cut");
    l.send(ToServer::Take { seq: 2, at });
    s.tick();
    assert!(drain(&l).contains(&ToClient::Done { seq: 2, ok: false }), "a second cut takes nothing");
    assert_eq!(items::count(&s.clients[0].slots, fibre), n as u32);
    let path = std::env::temp_dir().join(format!("technoape-fibre-{}.sav", std::process::id()));
    s.snapshot().write(&path).unwrap();
    let back = Server::from_save(Save::read(&path).unwrap(), channel().1);
    std::fs::remove_file(&path).unwrap();
    assert!(thing_at(&back.world, at, 0.0, veg, false, |_, _| true).is_none(), "the save keeps it cut");
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

/// Creative's bucket pours lava (`/fluid lava`) on land: a client told of it over the wire (the messages through
/// postcard, as a socket carries them) holds the very units the server does, and a save and a load keep it, with
/// water's ledger untouched.
#[test]
fn lava_poured_on_land_reaches_the_client_and_the_save() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let b = s.world.bodies.clone();
    let b = &b[0];
    let sea = b.radius - engine::world::SEA_DEPTH;
    let d = (0..2000)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / 2000.0;
            let t = k as f64 * 2.4;
            DVec3::new((1.0 - z * z).sqrt() * t.cos(), z, (1.0 - z * z).sqrt() * t.sin())
        })
        .find(|&d| b.height(d) > sea + 3.0)
        .expect("the planet has land");
    // a pit 3 x 3 and two deep, so the lava comes to rest (on a slope it creeps on for minutes)
    let top = s.world.air_over_ground(0, d);
    for (di, dj, dz) in (-1..=1).flat_map(|i| (-1..=1).flat_map(move |j| [(i, j, 1), (i, j, 2)])) {
        let c = Block { pos: top.pos + glam::IVec3::new(di, dj, -dz), ..top };
        s.world.set_block(c, 0);
        s.world.regap(c);
    }
    let at = Block { pos: top.pos + glam::IVec3::new(0, 0, -2), ..top };
    let lava = engine::fluids::FLUIDS.kind("lava").unwrap();
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Move { pos: s.world.block_centre(top).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Creative(true));
    l.send(ToServer::Chat("/fluid lava".into()));
    for seq in 1..=3 {
        l.send(ToServer::Bucket { seq, at, fill: false });
    }
    let mut client = engine::liquid::Liquids::default();
    let hear = |client: &mut engine::liquid::Liquids, got: Vec<ToClient>| {
        for m in got {
            let bytes = postcard::to_allocvec(&m).unwrap();
            if let ToClient::Water(v) = postcard::from_bytes::<ToClient>(&bytes).unwrap() {
                for (t, cols) in &v {
                    client.set_tile(*t, cols);
                }
            }
        }
    };
    s.tick();
    let got = drain(l);
    assert!((1..=3).all(|seq| got.contains(&ToClient::Done { seq, ok: true })), "{got:?}");
    hear(&mut client, got);
    assert_eq!(s.world.water.counted_of(lava), 3 * U);
    // until it lies still and the client has heard the last of it
    for _ in 0..2000 {
        s.tick();
        hear(&mut client, drain(l));
        if s.world.water.awake() == 0 && client.all() == s.world.water.all() {
            break;
        }
    }
    assert_eq!(s.world.water.awake(), 0, "the lava settles");
    assert_eq!((client.all(), client.counted_of(lava)), (s.world.water.all(), 3 * U), "the client holds what the server does");
    assert_eq!((s.world.water.counted_of(lava), s.water_ledger(), s.world.water.counted()), (3 * U, 0, 0), "water's ledger is untouched");
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!((back.world.water.all(), back.world.water.counted_of(lava)), (s.world.water.all(), 3 * U), "the save keeps the lava");
}

/// Oil poured on water in a pit lies on it, and the client and the save hold the two layers (liquids.md step 4); then
/// lava poured in boils water off: the clients hear of the steam (an `Event::Boil`), basalt is set through the edits,
/// and water's ledger, its steam counted, holds through a save.
#[test]
fn oil_on_water_reaches_the_client_and_lava_into_it_makes_basalt_and_steam() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let b = s.world.bodies.clone();
    let b = &b[0];
    let sea = b.radius - engine::world::SEA_DEPTH;
    let d = (0..2000)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / 2000.0;
            let t = k as f64 * 2.4;
            DVec3::new((1.0 - z * z).sqrt() * t.cos(), z, (1.0 - z * z).sqrt() * t.sin())
        })
        .find(|&d| b.height(d) > sea + 3.0)
        .expect("the planet has land");
    let top = s.world.air_over_ground(0, d);
    for (di, dj, dz) in (-1..=1).flat_map(|i| (-1..=1).flat_map(move |j| [(i, j, 1), (i, j, 2)])) {
        let c = Block { pos: top.pos + glam::IVec3::new(di, dj, -dz), ..top };
        s.world.set_block(c, 0);
        s.world.regap(c);
    }
    let at = Block { pos: top.pos + glam::IVec3::new(0, 0, -2), ..top };
    let fluids = engine::fluids::FLUIDS.now();
    let (oil, lava) = (fluids.kind("oil").unwrap(), fluids.kind("lava").unwrap());
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Move { pos: s.world.block_centre(top).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    l.send(ToServer::Creative(true));
    for seq in 1..=9 {
        l.send(ToServer::Bucket { seq, at, fill: false });
    }
    l.send(ToServer::Chat("/fluid oil".into()));
    l.send(ToServer::Bucket { seq: 10, at, fill: false });
    let mut client = engine::liquid::Liquids::default();
    let mut events: Vec<engine::events::Event> = Vec::new();
    let mut hear = |client: &mut engine::liquid::Liquids, got: Vec<ToClient>| {
        for m in got {
            let bytes = postcard::to_allocvec(&m).unwrap();
            match postcard::from_bytes::<ToClient>(&bytes).unwrap() {
                ToClient::Water(v) => v.iter().for_each(|(t, cols)| client.set_tile(*t, cols)),
                ToClient::Events(v) => events.extend(v),
                _ => {}
            }
        }
    };
    s.tick();
    let got = drain(l);
    assert!((1..=10).all(|seq| got.contains(&ToClient::Done { seq, ok: true })), "{got:?}");
    hear(&mut client, got);
    for _ in 0..3000 {
        s.tick();
        hear(&mut client, drain(l));
        if s.world.water.awake() == 0 && client.all() == s.world.water.all() {
            break;
        }
    }
    let two = |w: &engine::liquid::Liquids| w.all().iter().filter(|(_, g)| g.len() > 1).count();
    assert!(two(&s.world.water) > 0, "a gap of oil on water: {:?}", s.world.water.all());
    assert_eq!((client.all(), client.counted_of(oil)), (s.world.water.all(), U), "the client holds both layers as the server does");
    assert_eq!((s.world.water.counted_of(oil), s.water_ledger()), (U, 0));
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!((back.world.water.all(), back.world.water.counted_of(oil)), (s.world.water.all(), U), "the save keeps the layers");
    // lava into the pool
    l.send(ToServer::Chat("/fluid lava".into()));
    l.send(ToServer::Bucket { seq: 11, at, fill: false });
    let basalt = engine::blocks::BLOCKS.id("basalt");
    for _ in 0..5 * TICK_HZ as usize {
        s.tick();
        hear(&mut client, drain(l));
    }
    let boiled = s.world.water.vapour.get(&engine::fluids::WATER).copied().unwrap_or(0);
    assert!(boiled > 0 && events.iter().any(|e| matches!(e, engine::events::Event::Boil { .. })), "steam: {boiled} units, the client told");
    assert!(events.iter().any(|e| matches!(e, engine::events::Event::Edit { now, .. } if *now == basalt)), "basalt through the edits");
    assert_eq!(s.water_ledger(), 0, "water's ledger, its steam counted");
    assert_eq!(s.world.water.counted_of(lava) + s.world.water.froze.get(&lava).copied().unwrap_or(0), U, "lava's");
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!((back.water_ledger(), back.world.water.vapour.clone()), (0, s.world.water.vapour.clone()), "the steam booked through the save");
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

/// Step 6a: what is laid on the giant (a death's pile, a throw, a dug block's drop) rests at the cloud tops where its
/// player stands, not up to FALL layers under them for ever; a bucket is not poured there (nothing holds water).
#[test]
fn a_death_on_the_giant_leaves_its_pile_on_the_cloud_tops_and_no_water_is_poured() {
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 6, rx);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Input(Input::default()));
    s.tick();
    l.send(ToServer::Chat("/goto jupiter".into()));
    l.send(ToServer::Chat("/playmode survival".into()));
    for _ in 0..60 {
        s.tick();
    }
    let feet = s.clients[0].phys.pos;
    assert_eq!(s.world.frame_at(feet), 3);
    l.send(ToServer::Chat("/give oak_log 5".into()));
    l.send(ToServer::Chat("/hurt 40".into()));
    s.tick();
    assert!(s.clients[0].vitals.dead.is_some(), "dead");
    l.send(ToServer::Respawn);
    s.tick();
    let pile = s.piles.keys().copied().find(|k| k.0 == 3).expect("a pile on the giant");
    // within a pick-up of a player standing on the cloud tops over it
    let (j, at) = (&s.world.bodies[3], s.world.block_centre(unkey(pile)));
    let tops = j.centre + (at - j.centre).normalize() * j.radius;
    assert!(at.distance(tops) < 1.0 && at.distance(tops) < PICK, "the pile lies at the cloud tops: {} m from them", at.distance(tops));
    // the bucket: a creative pour at the feet is refused, no water is made
    l.send(ToServer::Chat("/playmode creative".into()));
    l.send(ToServer::Chat("/goto jupiter".into()));
    s.tick();
    assert_eq!(s.world.frame_at(s.clients[0].phys.pos), 3);
    let at = s.world.block_at(s.clients[0].phys.pos);
    let made = s.made;
    assert!(!s.bucket(0, at, false), "no water is poured on a gas body");
    assert_eq!(s.made, made);
}

/// Step 6a: a world of generation 6 welcomes with four bodies; /goto jupiter stands the player on the giant's cloud
/// tops (they hold it: the dive is not built), in its frame, out of every block; nothing can be dug or placed there;
/// the save keeps the giant as columns and no map, and loads again with the player where it was.
#[test]
fn goto_jupiter_stands_on_the_cloud_tops_and_saves() {
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 6, rx);
    let (c, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    let welcome = drain(&c).into_iter().find_map(|m| if let ToClient::Welcome { bodies, gen, .. } = m { Some((bodies, gen)) } else { None }).unwrap();
    assert_eq!((welcome.0.len(), welcome.1), (4, 6));
    assert_eq!(welcome.0[3], (256 * 11, None), "the giant: its columns' worth of radius and no map");
    assert!(engine::bodies::known_bodies(welcome.0.len(), welcome.1).is_ok());
    c.send(ToServer::Chat("/goto jupiter".into()));
    c.send(ToServer::Input(Input::default())); // walking, so the server steps the fall
    for _ in 0..120 {
        s.tick();
    }
    let got = drain(&c);
    assert!(got.iter().any(|m| matches!(m, ToClient::Chat(t) if t == "gone to Jupiter")), "{got:?}");
    let (w, me) = (&s.world, s.clients[0].phys);
    let j = &w.bodies[3];
    let over = (me.pos - j.centre).length() - j.radius;
    assert!(w.frame_at(me.pos) == 3 && (0.0..2.0).contains(&over) && me.on_ground, "{over} m over the cloud tops");
    assert!(!phys::inside(w, &s.clients[0].phys));
    // nothing to dig or place: /place under the feet does nothing
    c.send(ToServer::Chat("/place stone".into()));
    s.tick();
    assert_eq!(s.world.edit_count, 0);
    let save = s.snapshot();
    assert_eq!((save.gen, save.bodies.len(), save.bodies[3]), (6, 4, (256 * 11, None)));
    let back = Server::from_save(Save::from_bytes(&save.to_bytes()).unwrap(), channel().1);
    assert_eq!(back.world.bodies.len(), 4);
    for p in &save.players {
        let at = glam::DVec3::from_array(p.pos);
        assert_eq!(back.world.frame_at(at), 3, "{}", p.name);
        assert!(!phys::blocked(&back.world, at));
    }
}

/// Step 6b: on a new server Mars is a stub (its hash in the welcome, no map). /goto mars answers that its ground is
/// loading and leaves the player where it was; the ticks meanwhile do not wait, not even with rayon's global pool
/// held; once the ground is in (on a thread of its own) the player stands on Mars. The recording puts it in at the
/// same tick and replays to the same hash.
#[test]
fn goto_mars_waits_for_its_ground_off_the_tick_and_replays() {
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 6, rx);
    let rec = std::env::temp_dir().join(format!("technoape-lazy-{}.rec", std::process::id()));
    s.record(&rec).unwrap();
    assert!(!s.world.loaded(2) && s.world.bodies[2].map().is_none() && s.world.loader(2).is_some(), "Mars a stub at the start");
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Input(Input::default()));
    s.tick();
    let welcome = drain(&c).into_iter().find_map(|m| if let ToClient::Welcome { bodies, .. } = m { Some(bodies) } else { None }).unwrap();
    assert_eq!(welcome[2].1, s.world.map_hash(2), "the welcome says Mars's hash");
    assert!(welcome[2].1.is_some());
    let before = s.clients[0].phys.pos;
    // every pool thread waits on a gate while Mars loads: the ticks still end
    let threads = rayon::current_num_threads();
    let gate = std::sync::Arc::new(std::sync::RwLock::new(()));
    let held = gate.write().unwrap();
    let (started_tx, started) = channel();
    for _ in 0..threads {
        let (gate, started_tx) = (gate.clone(), started_tx.clone());
        rayon::spawn(move || {
            started_tx.send(()).unwrap();
            drop(gate.read().unwrap());
        });
    }
    for _ in 0..threads {
        started.recv().unwrap();
    }
    c.send(ToServer::Chat("/goto mars".into()));
    let (done_tx, done) = channel();
    let t = std::thread::spawn(move || {
        for _ in 0..5 {
            s.tick();
        }
        done_tx.send(()).unwrap();
        s
    });
    let ended = done.recv_timeout(std::time::Duration::from_secs(10)).is_ok();
    drop(held);
    let mut s = t.join().unwrap();
    assert!(ended, "a tick waited on the global pool while Mars loaded");
    let got = drain(&c);
    assert!(got.iter().any(|m| matches!(m, ToClient::Chat(t) if t == "going to Mars: its ground is loading")), "{got:?}");
    assert!(s.clients[0].phys.pos.distance(before) < 1.0, "the player waits where it was");
    let t0 = std::time::Instant::now();
    while !s.world.loaded(2) {
        assert!(t0.elapsed().as_secs() < 120, "Mars never came in");
        s.tick();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for _ in 0..60 {
        s.tick();
    }
    let got = drain(&c);
    assert!(got.iter().any(|m| matches!(m, ToClient::Chat(t) if t == "gone to Mars")), "{got:?}");
    let (w, me) = (&s.world, s.clients[0].phys);
    let m = &w.bodies[2];
    let over = (me.pos - m.centre).length() - m.top((me.pos - m.centre).normalize());
    assert!(w.frame_at(me.pos) == 2 && over.abs() < 3.0 && !phys::inside(w, &me), "{over} m over Mars's ground");
    let r = replay(&rec, None).unwrap();
    std::fs::remove_file(&rec).unwrap();
    // (its checkpoints, every CHECK_EVERY ticks, hash the players too: the last ones are on Mars)
    assert!(r.server.world.loaded(2), "the replay put Mars in");
    assert_eq!((r.diverged, r.server.world.hash()), (None, s.world.hash()));
    assert!(r.checks >= 3, "{} checks", r.checks);
    let at = |s: &Server| s.clients.iter().map(|c| s.world.frame_at(c.phys.pos)).collect::<Vec<_>>();
    assert_eq!(at(&r.server), vec![2], "the replayed player is on Mars too");
}

/// Step 6c: a world of generation 7 welcomes with five bodies, the giant's icy moon a stub; /goto callisto waits for
/// its ground (off the tick) and leaves the player standing on it, in its frame, the giant over it; the save keeps
/// it and loads again with the player there. A generation 6 world still welcomes with four.
#[test]
fn goto_callisto_waits_for_its_ground_and_leaves_the_player_on_it() {
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 7, rx);
    assert!(!s.world.loaded(4) && s.world.loader(4).is_some(), "the icy moon a stub at the start");
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Input(Input::default()));
    s.tick();
    let welcome = drain(&c).into_iter().find_map(|m| if let ToClient::Welcome { bodies, gen, .. } = m { Some((bodies, gen)) } else { None }).unwrap();
    assert_eq!((welcome.0.len(), welcome.1), (5, 7));
    assert_eq!((welcome.0[4].0, welcome.0[4].1), (64, s.world.map_hash(4)));
    assert!(welcome.0[4].1.is_some() && engine::bodies::known_bodies(5, 7).is_ok());
    c.send(ToServer::Chat("/goto callisto".into()));
    s.tick();
    let got = drain(&c);
    assert!(got.iter().any(|m| matches!(m, ToClient::Chat(t) if t == "going to Callisto: its ground is loading")) || s.world.loaded(4), "{got:?}");
    let t0 = std::time::Instant::now();
    while !s.world.loaded(4) {
        assert!(t0.elapsed().as_secs() < 120, "the icy moon never came in");
        s.tick();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for _ in 0..60 {
        s.tick();
    }
    let got = drain(&c);
    assert!(got.iter().any(|m| matches!(m, ToClient::Chat(t) if t == "gone to Callisto")), "{got:?}");
    let (w, me) = (&s.world, s.clients[0].phys);
    let b = &w.bodies[4];
    let over = (me.pos - b.centre).length() - b.top((me.pos - b.centre).normalize());
    // (as Mars's test: stood just over its ground, not in it; with no input sent the player is not stepped down)
    assert!(w.frame_at(me.pos) == 4 && over.abs() < 3.0 && !phys::inside(w, &me), "{over} m over its ground");
    let save = s.snapshot();
    assert_eq!((save.gen, save.bodies.len()), (7, 5));
    let back = Server::from_save(Save::from_bytes(&save.to_bytes()).unwrap(), channel().1);
    assert_eq!(back.world.bodies.len(), 5);
    assert!(save.players.iter().all(|p| back.world.frame_at(glam::DVec3::from_array(p.pos)) == 4));
    // a generation 6 world: four bodies, no moon of the giant
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 6, rx);
    let (c, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    let welcome = drain(&c).into_iter().find_map(|m| if let ToClient::Welcome { bodies, gen, .. } = m { Some((bodies.len(), gen)) } else { None }).unwrap();
    assert_eq!(welcome, (4, 6));
    assert_eq!(engine::bodies::find("callisto"), Some(4));
}

/// Step 6b review: with Mars's map on no disk its loader bakes it, and the bake runs on rayon's global pool; the
/// ticks still never wait for it (the pool held through five of them), and Mars comes in once the pool is free.
#[test]
fn a_mars_baked_on_arrival_does_not_hold_the_tick() {
    let seed = 4000 + (std::process::id() % 1000) as i32;
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, seed, 0.0, 6, rx);
    let file = engine::bodies::of(6)[2].bake.as_ref().unwrap().file(seed, s.world.bodies[2].n);
    let _ = std::fs::remove_file(&file);
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Input(Input::default()));
    s.tick();
    let threads = rayon::current_num_threads();
    let gate = std::sync::Arc::new(std::sync::RwLock::new(()));
    let held = gate.write().unwrap();
    let (started_tx, started) = channel();
    for _ in 0..threads {
        let (gate, started_tx) = (gate.clone(), started_tx.clone());
        rayon::spawn(move || {
            started_tx.send(()).unwrap();
            drop(gate.read().unwrap());
        });
    }
    for _ in 0..threads {
        started.recv().unwrap();
    }
    c.send(ToServer::Chat("/goto mars".into()));
    let (done_tx, done) = channel();
    let t = std::thread::spawn(move || {
        for _ in 0..5 {
            s.tick();
        }
        done_tx.send(()).unwrap();
        s
    });
    let ended = done.recv_timeout(std::time::Duration::from_secs(10)).is_ok();
    std::thread::sleep(std::time::Duration::from_millis(200));
    drop(held);
    let mut s = t.join().unwrap();
    assert!(ended, "a tick waited on the pool while Mars's map was baked");
    let t0 = std::time::Instant::now();
    while !s.world.loaded(2) {
        assert!(t0.elapsed().as_secs() < 300, "Mars never came in");
        s.tick();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    println!("Mars baked and in after {:.2} s with the pool free", t0.elapsed().as_secs_f64());
    let _ = std::fs::remove_file(&file);
}

/// Step 6b review: a body whose ground cannot be loaded (a map this build does not make) does not keep a player
/// held by it for ever: the held one goes back to the spawn, told why; the /goto that waited for it, and one after,
/// answer plainly that it cannot be loaded.
#[test]
fn a_body_that_cannot_be_loaded_lets_its_players_go() {
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 6, rx);
    s.world.lazy[2].as_mut().unwrap().hash ^= 1;
    let rec = std::env::temp_dir().join(format!("technoape-failed-{}.rec", std::process::id()));
    s.record(&rec).unwrap();
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Input(Input::default()));
    s.tick();
    drain(&c);
    let m = &s.world.bodies[2];
    let at = m.centre + DVec3::Y * (m.radius + engine::world::AMP + 50.0);
    c.send(ToServer::Move { pos: at.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    c.send(ToServer::Chat("/goto mars".into()));
    for _ in 0..400 {
        s.tick();
        std::thread::sleep(std::time::Duration::from_millis(5));
        if s.loads.failed.contains(&2) {
            break;
        }
    }
    for _ in 0..3 {
        s.tick();
    }
    let said: Vec<String> = drain(&c).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
    assert_eq!(s.world.frame_at(s.clients[0].phys.pos), 0, "back at the spawn, not held: {said:?}");
    assert!(s.loads.gotos.is_empty() && said.iter().filter(|t| t.contains("Mars's ground cannot be loaded")).count() >= 2, "{said:?}");
    c.send(ToServer::Chat("/goto mars".into()));
    s.tick();
    let said: Vec<String> = drain(&c).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
    assert!(said.iter().any(|t| t.contains("Mars's ground cannot be loaded")) && s.loads.gotos.is_empty(), "{said:?}");
    for _ in 0..60 {
        s.tick();
    }
    // (the recording carries the hash this build cannot make: its replay fails at the same tick, `Ev::Body`)
    let r = replay(&rec, None).unwrap();
    std::fs::remove_file(&rec).unwrap();
    assert_eq!((r.diverged, r.server.clients[0].phys.pos), (None, s.clients[0].phys.pos));
}

/// Step 6b review: a body whose ground is not in yet has no ground: its cells read as air (as a gas body's), and a
/// player there (a client may put itself anywhere) changes nothing on it: edits, buckets, pick-ups and items to the
/// ground are refused with a word why, so no hole or floating pile is left for the real ground to meet.
#[test]
fn nothing_is_changed_on_a_body_not_in_yet() {
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 6, rx);
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Input(Input::default()));
    c.send(ToServer::Creative(true));
    c.send(ToServer::Chat("/give stone 8".into()));
    s.tick();
    drain(&c);
    let deep = Block { body: 2, face: 0, pos: glam::IVec3::new(10, 10, 1) };
    assert_eq!(s.world.block(deep), AIR, "a stub's cells are air, not the seed's noise land");
    let m = &s.world.bodies[2];
    let at = m.centre + DVec3::Y * (m.radius + 2.0);
    let under = s.world.block_at(at - DVec3::Y * 1.5);
    c.send(ToServer::Move { pos: at.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    c.send(ToServer::Edit { seq: 1, shape: Shape::one(under), material: BLOCKS.id("stone") });
    c.send(ToServer::Edit { seq: 2, shape: Shape::one(under), material: AIR });
    c.send(ToServer::Items { seq: 3, from: (Holder::Ape, 0), to: (Holder::Ground, 0), n: 8 });
    c.send(ToServer::Bucket { seq: 4, at: under, fill: false });
    c.send(ToServer::Take { seq: 5, at: under });
    s.tick();
    let got = drain(&c);
    for seq in 1..=5 {
        assert!(got.contains(&ToClient::Done { seq, ok: false }), "{seq} refused: {:?}", got.iter().filter(|m| matches!(m, ToClient::Done { .. })).collect::<Vec<_>>());
    }
    assert!(got.iter().any(|m| matches!(m, ToClient::Chat(t) if t.contains("Mars's ground is not in yet"))), "told why");
    assert!(s.world.edits.keys().all(|k| k.body != 2) && s.piles.keys().all(|b| b.0 != 2), "nothing left on the stub");
}

/// Step 6b review: a server whose bodies are all in from the start (`Server::baked`: the soak, the bots' tests)
/// records no `Ev::Lazy`, so its replay has Mars in from the start too: a player gone to Mars falls the same.
/// (The lazy recording's replay: `goto_mars_waits_for_its_ground_off_the_tick_and_replays`.)
#[test]
fn an_eager_recording_with_a_player_on_mars_replays() {
    let (tx, rx) = channel();
    let mut s = Server::baked(256, 64, 3, rx);
    assert!(s.world.loaded(2) && s.world.bodies[2].map().is_some(), "Mars in from the start");
    let rec = std::env::temp_dir().join(format!("technoape-eager-{}.rec", std::process::id()));
    s.record(&rec).unwrap();
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Input(Input::default()));
    s.tick();
    c.send(ToServer::Chat("/goto mars".into()));
    for _ in 0..200 {
        s.tick();
    }
    assert_eq!(s.world.frame_at(s.clients[0].phys.pos), 2, "on Mars");
    let r = replay(&rec, None).unwrap();
    std::fs::remove_file(&rec).unwrap();
    assert_eq!((r.diverged, r.server.world.hash(), r.server.clients[0].phys.pos), (None, s.world.hash(), s.clients[0].phys.pos));
}

/// Step 6b review: a map asked for again and again while its body loads is queued once and sent once; one whose
/// body cannot be loaded is dropped, not kept in the queue for ever.
#[test]
fn a_map_asked_for_a_loading_body_is_queued_and_sent_once() {
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 6, rx);
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Input(Input::default()));
    s.tick();
    drain(&c);
    let h = s.world.map_hash(2).unwrap();
    for _ in 0..5 {
        c.send(ToServer::Map(h));
    }
    s.tick();
    assert_eq!(s.loads.maps.len(), 1, "one entry for five asks");
    let t0 = std::time::Instant::now();
    while !s.world.loaded(2) {
        assert!(t0.elapsed().as_secs() < 120, "Mars never came in");
        s.tick();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    s.tick();
    c.send(ToServer::Map(h));
    s.tick();
    let firsts = drain(&c).into_iter().filter(|m| matches!(m, ToClient::Map { piece: 0, .. })).count();
    assert_eq!((firsts, s.loads.maps.len()), (1, 0), "the map sent once");
    // a body this build cannot load (another map's hash): its asks are dropped
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(256, 64, 3, 0.0, 6, rx);
    s.world.lazy[2].as_mut().unwrap().hash ^= 1;
    let (c, sv) = local();
    tx.send(sv).unwrap();
    c.send(ToServer::Input(Input::default()));
    s.tick();
    c.send(ToServer::Map(s.world.map_hash(2).unwrap()));
    for _ in 0..200 {
        s.tick();
        std::thread::sleep(std::time::Duration::from_millis(5));
        if s.loads.failed.contains(&2) {
            break;
        }
    }
    s.tick();
    assert!(s.loads.failed.contains(&2) && s.loads.maps.is_empty(), "the failed body's asks are dropped: {:?}", s.loads.maps);
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
    // the usage names this world's bodies, the help Mars too (a new world's)
    let names: Vec<String> = s.world.bodies.iter().map(|b| b.name.to_lowercase()).collect();
    assert!(a[0].starts_with(&format!("usage: /goto {}|sun\n", names.join("|"))), "{a:?}");
    assert!(a[2].contains("luna, mars"), "{a:?}");
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
    let got = drain(&links[1]);
    assert!(chats(&got).contains(&cause), "everyone is told");
    assert_eq!(events(&got), vec![Event::Died { by: s.clients[0].player.id }], "a death is an event (ADR 0149)");
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
    let got = drain(&c);
    assert_eq!(vitals_of(&got), vec![(20, 20, None)]);
    // the pile laid is an event, at the death mark's cell (ADR 0149)
    assert_eq!(events(&got), vec![Event::Laid { at: me.marks.death[0], by: me.player.id }]);
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
    // the review: only a truly empty hand drinks (a knapping stone or a tool in hand strikes instead)
    stand(&mut s, fresh);
    sip(&mut s, None, 0.1);
    let hand = s.clients[0].hand as usize;
    s.clients[0].slots[hand] = Some(items::Stack::new(ITEMS.named("flint").unwrap(), 1));
    s.clients[0].vitals.thirst = 5.0;
    assert!(sip(&mut s, Some(fresh), 1.0) < 5.0, "something in hand: no drink");
    s.clients[0].slots[hand] = None;
    assert!(sip(&mut s, None, 0.1) < 5.0);
    // and a drink held does not outlive a death: the respawned ape is not drinking
    l.send(ToServer::Drink(Some(fresh)));
    l.send(ToServer::Chat("/hurt 40".into()));
    s.tick();
    assert!(s.clients[0].vitals.dead.is_some() && s.clients[0].drink.is_none(), "death lets go");
    l.send(ToServer::Respawn);
    l.send(ToServer::Input(Input::default()));
    s.tick();
    stand(&mut s, fresh);
    s.clients[0].vitals.thirst = 5.0;
    for _ in 0..TICK_HZ as u32 {
        s.tick();
    }
    assert!(s.clients[0].vitals.thirst < 5.0, "no drink after the respawn");
}

/// Breath through the protocol (draft breath): held under the sea, the breath goes in 30 s and then health a
/// point a second, and the client is told both; surfaced, the breath is back in 2 s; held under, it drowns.
#[test]
fn under_water_the_breath_goes_then_health_and_air_brings_it_back() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Input(Input::default()));
    s.tick();
    let b = &s.world.bodies[0];
    let (sea, centre) = (b.layer_r(b.sea.unwrap()), b.centre);
    let dirs = || (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) });
    let d = dirs().find(|d| b.height(*d) < sea - 4.0).expect("a sea");
    let land = dirs().find(|d| b.height(*d) > sea + 6.0).expect("land");
    // (the dry spot in the open air over the land, clear of any plant)
    let (deep, dry) = (centre + d * (b.height(d) + 0.1), centre + land * (b.height(land) + 4.0));
    let hold = |s: &mut Server, at: DVec3, secs: f64| {
        for _ in 0..(secs * TICK_HZ) as u32 {
            (s.clients[0].phys.pos, s.clients[0].phys.vel) = (at, DVec3::ZERO);
            s.tick();
        }
    };
    let breaths = |got: &[ToClient]| got.iter().filter_map(|m| if let ToClient::Vitals { breath, .. } = m { Some(*breath) } else { None }).collect::<Vec<u8>>();
    drain(l);
    hold(&mut s, deep, 29.0);
    let got = drain(l);
    assert!(s.clients[0].vitals.health == vitals::MAX && breaths(&got).last().is_some_and(|&b| b > 0 && b < 5), "held: {:?}", breaths(&got));
    hold(&mut s, deep, 5.0);
    let h = s.clients[0].vitals.health;
    assert!((15..=17).contains(&h) && breaths(&drain(l)).last() == Some(&0), "then a point a second: health {h}");
    hold(&mut s, dry, 2.5);
    assert_eq!((s.clients[0].vitals.health, breaths(&drain(l)).last()), (h, Some(&vitals::MAX)), "air: the breath back in 2 s");
    hold(&mut s, deep, 60.0);
    assert_eq!(s.clients[0].vitals.dead.as_deref(), Some("ape drowned"));
}

/// The review: `ToServer::Drink` keeps its tag on the wire (postcard numbers variants in order: appended
/// last, after `Choose`, it is 21).
#[test]
fn the_drink_message_keeps_its_tag() {
    let at = Block { body: 0, face: 2, pos: glam::IVec3::new(1, 2, 3) };
    assert_eq!(postcard::to_allocvec(&ToServer::Drink(Some(at))).unwrap(), vec![21, 1, 0, 2, 2, 4, 6]);
    assert_eq!(postcard::to_allocvec(&ToServer::Drink(None)).unwrap(), vec![21, 0]);
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

/// A player whose connection went quiet (a laptop asleep) and who joins again under its name takes itself
/// over, place and slots, instead of becoming `name-2`; the quiet connection is closed.
#[test]
fn a_hello_takes_over_its_own_quiet_ghost() {
    let (tx, rx) = channel();
    let mut s = Server::new(256, 32, 1, rx);
    let rec = std::env::temp_dir().join(format!("technoape-ghost-{}.rec", std::process::id()));
    s.record(&rec).unwrap();
    let links: Vec<Link<ToServer, ToClient>> = (0..2)
        .map(|_| {
            let (c, sv) = local();
            tx.send(sv).unwrap();
            c
        })
        .collect();
    s.tick();
    links[0].send(ToServer::Hello { name: "ape".into() });
    links[0].send(ToServer::Chat("/give stone 3".into()));
    s.tick();
    let at = s.clients[0].phys.pos;
    for _ in 0..GHOST_TICKS {
        s.tick();
    }
    links[1].send(ToServer::Hello { name: "ape".into() });
    s.tick();
    let names: Vec<&str> = s.clients.iter().map(|c| c.player.name.as_str()).collect();
    assert_eq!(names, ["ape"]);
    assert_eq!(s.clients[0].player.id, 2, "the new connection is the one kept");
    assert_eq!(held(&s.clients[0].slots), vec![(BLOCKS.id("stone"), 3)]);
    assert!((s.clients[0].phys.pos - at).length() < 1.0, "{} {}", s.clients[0].phys.pos, at);
    drain(&links[0]);
    assert!(matches!(links[0].try_recv(), Recv::Closed), "the ghost's link is closed");
    // the takeover is judged in ticks: its replay takes over the same way
    s.tick();
    let r = replay(&rec, Some(s.tick)).unwrap();
    std::fs::remove_file(&rec).unwrap();
    assert_eq!((r.diverged, r.server.clients.len(), r.server.hash()), (None, 1, s.hash()));
}

/// A takeover handles every other client once in its tick, whichever side of the ghost and the new
/// connection it stands: no one is read twice or skipped because someone else took over.
#[test]
fn a_takeover_neither_skips_nor_repeats_another_client() {
    // (the ghost's, the other's and the new connection's link)
    for (a, c, b) in [(0, 1, 2), (0, 2, 1)] {
        let (mut s, links) = server_with(3);
        links[a].send(ToServer::Hello { name: "ape".into() });
        links[c].send(ToServer::Hello { name: "cat".into() });
        s.tick();
        for _ in 0..GHOST_TICKS {
            links[c].send(ToServer::Hand(0));
            s.tick();
        }
        let cat = s.clients.iter().find(|x| x.player.name == "cat").unwrap().player.id;
        links[b].send(ToServer::Hello { name: "ape".into() });
        for _ in 0..200 {
            links[c].send(ToServer::Hand(0));
        }
        s.tick();
        let names: Vec<&str> = s.clients.iter().map(|x| x.player.name.as_str()).collect();
        assert!(names.len() == 2 && names.contains(&"ape") && names.contains(&"cat"), "{names:?}");
        let other = s.clients.iter().find(|x| x.player.id == cat).unwrap();
        let mut left = 0;
        while let Recv::Msg(_) = other.link.try_recv() {
            left += 1;
        }
        assert_eq!(200 - left, MSGS_PER_TICK, "messages taken from the other client, order {:?}", (a, c, b));
    }
}

/// Singleplayer's recordings keep the newest few; files of any other name stay.
#[test]
fn old_play_recordings_are_pruned_and_nothing_else() {
    let dir = std::env::temp_dir().join(format!("technoape-rec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("play-5.rec")).unwrap(); // a folder of the name: left alone
    for t in [100, 20, 3000, 400, 50] {
        std::fs::write(dir.join(format!("play-{t}.rec")), b"x").unwrap();
    }
    for other in ["server-1.rec", "play-1.txt", "play-x.rec", "notes.md"] {
        std::fs::write(dir.join(other), b"x").unwrap();
    }
    assert_eq!(prune_recordings(&dir, 2).unwrap(), 3);
    let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    left.sort();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(left, ["notes.md", "play-1.txt", "play-3000.rec", "play-400.rec", "play-5.rec", "play-x.rec", "server-1.rec"]);
}

/// A client gets each map once a connection, however often it asks: 64 asks a tick would queue a GB.
#[test]
fn a_map_is_sent_once_however_often_it_is_asked() {
    let (tx, rx) = channel();
    let mut s = Server::baked_warp(1536, 32, 7, 0.0, 0, rx);
    let (c, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    let h = s.world.bodies[0].map().unwrap().hash;
    for _ in 0..3 {
        c.send(ToServer::Map(h));
    }
    s.tick();
    c.send(ToServer::Map(h));
    s.tick();
    let pieces = drain(&c).iter().filter(|m| matches!(m, ToClient::Map { .. })).count();
    assert_eq!(pieces, engine::net::map_pieces(s.world.bodies[0].map().unwrap()).len());
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
    let got = drain(&links[1]);
    assert!(got.iter().any(|m| matches!(m, ToClient::Fires(v) if v.contains(&(at, true)))));
    assert!(events(&got).contains(&Event::Fire { at, on: true }), "a fire lit is an event (ADR 0149)");
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

/// The items as (item, count) over some slots.
fn counted<'a>(slots: impl Iterator<Item = &'a Slot>) -> std::collections::BTreeMap<u16, u32> {
    let mut m = std::collections::BTreeMap::new();
    for st in slots.flatten() {
        *m.entry(st.item).or_insert(0) += st.n as u32;
    }
    m
}

/// Client i's slots emptied by the test's own hand, the items' ledger told.
fn empty_hands(s: &mut Server, i: usize) {
    for st in s.clients[i].slots.iter_mut().filter_map(Option::take) {
        *s.item_made.entry(st.item).or_default() -= st.n as i64;
    }
    s.clients[i].inv_dirty = true;
}

/// The last marks a client was told of.
fn marks_of(got: &[ToClient]) -> Option<(Option<Block>, Option<Block>)> {
    got.iter().filter_map(|m| if let ToClient::Marks { death, home } = m { Some((*death, *home)) } else { None }).last()
}

/// Death's cost (needs.md, ADR 0047): what a survival ape carried lies where it died, exactly that, nothing lost
/// or made, on the ground, on the sea bed, in the air and out of the block it was buried in; its client is told
/// the pile's cell as its death mark, which goes when another empties the pile.
#[test]
fn a_death_leaves_exactly_what_was_carried_in_a_marked_pile_wherever_it_died() {
    let (mut s, links) = server_with(2);
    let (l, other) = (&links[0], &links[1]);
    l.send(ToServer::Hello { name: "ape".into() });
    other.send(ToServer::Hello { name: "other".into() });
    l.send(ToServer::Input(Input::default()));
    other.send(ToServer::Input(Input::default()));
    s.tick();
    l.send(ToServer::Chat("/playmode survival".into()));
    s.tick();
    let b = &s.world.bodies[0];
    let (sea, centre) = (b.layer_r(b.sea.unwrap()), b.centre);
    let dirs = || (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) });
    let d = dirs().find(|d| b.height(*d) < sea - 4.0).expect("a sea");
    // (dry land away from the spawn, where the respawned ape would pick its pile up at once)
    let land = dirs().find(|d| b.height(*d) > sea + 6.0 && (*d - DVec3::Y).length() > 0.2).expect("land");
    let ground = centre + land * (b.height(land) + 0.1);
    let up = phys::up(&s.world, ground);
    let places = [("ground", ground), ("sea bed", centre + d * (b.height(d) + 0.1)), ("air", ground + up * 200.0), ("buried", ground)];
    let hold = |s: &mut Server, at: DVec3| {
        (s.clients[0].phys.pos, s.clients[0].phys.vel) = (at, DVec3::ZERO);
        s.tick();
    };
    let mut cell = None;
    for (name, at) in places {
        l.send(ToServer::Chat("/give oak_log 5".into()));
        hold(&mut s, at);
        if name == "buried" {
            let stone = BLOCKS.id("stone");
            for h in [0.5, 1.5] {
                let c = s.world.block_at(at + up * h);
                s.world.set_block(c, stone);
            }
        }
        let carried = counted(s.clients[0].slots.iter());
        assert!(!carried.is_empty(), "{name}: it carries something");
        let lying = counted(s.piles.values().flat_map(|p| p.slots.iter()));
        l.send(ToServer::Chat("/hurt 40".into()));
        hold(&mut s, at);
        assert!(s.clients[0].vitals.dead.is_some(), "{name}: dead");
        drain(l);
        l.send(ToServer::Respawn);
        s.tick();
        let now = counted(s.piles.values().flat_map(|p| p.slots.iter()));
        let added: std::collections::BTreeMap<u16, u32> = now.iter().map(|(k, n)| (*k, n - lying.get(k).copied().unwrap_or(0))).filter(|(_, n)| *n > 0).collect();
        assert_eq!(added, carried, "{name}: the piles hold exactly what it carried");
        assert!(s.clients[0].slots.iter().all(Option::is_none), "{name}: it wakes with empty hands");
        assert!(s.item_ledger().is_empty(), "{name}: {:?}", s.item_ledger());
        let (death, _) = marks_of(&drain(l)).expect("its marks are told");
        let c = death.expect("a death mark");
        assert!(s.piles.contains_key(&key(c)), "{name}: the mark is on its pile");
        assert!(!BLOCKS.solid(s.world.block(c)), "{name}: the pile lies in an open cell");
        assert!(s.world.block_centre(c).distance(at) < 3.0 || name == "air", "{name}: where it died");
        cell = Some(c);
    }
    // another player empties the last pile: the mark goes
    let c = cell.unwrap();
    for _ in 0..5 {
        (s.clients[1].phys.pos, s.clients[1].phys.vel) = (s.world.block_centre(c), DVec3::ZERO);
        s.tick();
    }
    assert!(!s.piles.contains_key(&key(c)), "taken");
    assert_eq!(marks_of(&drain(l)).map(|m| m.0), Some(None), "the death mark goes with its pile");
}

/// The sleeping place (needs.md): the use button on a bed of leaves beside a fire claims it, a respawn wakes the
/// ape on it while it stands, and at the spawn once it is gone.
#[test]
fn a_respawn_wakes_at_the_claimed_sleeping_place_while_it_stands_else_at_the_spawn() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Input(Input::default()));
    s.tick();
    l.send(ToServer::Chat("/playmode survival".into()));
    s.tick();
    let spawn = s.spawn();
    let feet = s.world.block_at(spawn + phys::up(&s.world, spawn) * 0.5);
    let at = |d: glam::IVec3| Block { pos: feet.pos + d, ..feet };
    let (bed, torch) = (at(glam::IVec3::new(3, 0, 0)), at(glam::IVec3::new(4, 0, 0)));
    for z in 1..4 {
        s.world.set_block(at(glam::IVec3::new(3, 0, z)), AIR);
    }
    s.world.set_block(bed, BLOCKS.id("oak leaves"));
    s.world.set_block(torch, BLOCKS.id("torch"));
    let joined = drain(l);
    assert!(marks_of(&joined).is_none(), "no empty marks on join");
    let said = |got: &[ToClient], what: &str| got.iter().any(|m| matches!(m, ToClient::Chat(t) if t.starts_with(what)));
    // the review: through a wall it is no claim
    let wall: Vec<Block> = (-1..=1).flat_map(|j| (0..4).map(move |z| glam::IVec3::new(2, j, z))).map(at).collect();
    let stone = BLOCKS.id("stone");
    for &c in &wall {
        s.world.set_block(c, stone);
    }
    l.send(ToServer::Use(bed));
    s.tick();
    let got = drain(l);
    assert!(said(&got, "no sleeping place") && marks_of(&got).is_none() && s.clients[0].marks.home.is_none(), "not through a wall");
    for &c in &wall {
        s.world.set_block(c, AIR);
    }
    let claim = |s: &mut Server| {
        l.send(ToServer::Use(bed));
        s.tick();
        let got = drain(l);
        assert!(said(&got, "you will wake here"), "claimed");
        assert_eq!(marks_of(&got).map(|m| m.1), Some(Some(bed)));
    };
    claim(&mut s);
    let die = |s: &mut Server| {
        l.send(ToServer::Chat("/hurt 40".into()));
        s.tick();
        l.send(ToServer::Respawn);
        s.tick();
        s.clients[0].phys.pos
    };
    let woke = die(&mut s);
    assert!(woke.distance(vitals::wake_at(&s.world, bed)) < 0.01, "on the bed: {woke:?}");
    // the review: a block placed on the bed after the claim leaves no room: the spawn, and Home is forgotten
    let top = at(glam::IVec3::new(3, 0, 1));
    s.world.set_block(top, stone);
    drain(l);
    let woke = die(&mut s);
    assert!(woke.distance(s.spawn()) < 0.01, "at the spawn: {woke:?}");
    assert_eq!(marks_of(&drain(l)).map(|m| m.1), Some(None));
    // room again, claimed again; the fire gone, it is no sleeping place
    s.world.set_block(top, AIR);
    claim(&mut s);
    s.world.set_block(torch, AIR);
    let woke = die(&mut s);
    assert!(woke.distance(s.spawn()) < 0.01, "at the spawn: {woke:?}");
    assert_eq!(marks_of(&drain(l)).map(|m| m.1), Some(None));
}

/// The review: the death mark follows its pile. Dug under, the pile falls and the mark is on its new cell; once
/// it is taken, another's item dropped in the old cell does not bring the mark back.
#[test]
fn the_death_mark_follows_its_pile_down_and_goes_when_it_is_taken() {
    let (mut s, links) = server_with(2);
    let (l, other) = (&links[0], &links[1]);
    l.send(ToServer::Hello { name: "ape".into() });
    other.send(ToServer::Hello { name: "other".into() });
    l.send(ToServer::Input(Input::default()));
    other.send(ToServer::Input(Input::default()));
    s.tick();
    l.send(ToServer::Chat("/playmode survival".into()));
    s.tick();
    let b = &s.world.bodies[0];
    let (sea, centre) = (b.layer_r(b.sea.unwrap()), b.centre);
    let dirs = || (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) });
    let land = dirs().find(|d| b.height(*d) > sea + 6.0 && (*d - DVec3::Y).length() > 0.2).expect("land");
    let ground = centre + land * (b.height(land) + 0.1);
    // (it carries five logs only: one pile; the other stands far off, at the spawn)
    empty_hands(&mut s, 0);
    l.send(ToServer::Chat("/give oak_log 5".into()));
    s.tick();
    let carried = counted(s.clients[0].slots.iter());
    let log = *carried.keys().next().unwrap();
    l.send(ToServer::Chat("/hurt 40".into()));
    (s.clients[0].phys.pos, s.clients[0].phys.vel) = (ground, DVec3::ZERO);
    s.tick();
    l.send(ToServer::Respawn);
    s.tick();
    assert_eq!(s.clients[0].marks.death.len(), 1);
    let pile = s.clients[0].marks.death[0];
    assert_eq!(counted(s.piles[&key(pile)].slots.iter()), carried);
    // dug under (as the tick does for a dug cell): two cells down
    for k in 1..=2 {
        let under = Block { pos: pile.pos - glam::IVec3::Z * k, ..pile };
        s.world.set_block(under, AIR);
        s.resettle(Block { pos: under.pos + glam::IVec3::Z, ..under });
    }
    let fell = Block { pos: pile.pos - glam::IVec3::Z * 2, ..pile };
    assert_eq!(counted(s.piles[&key(fell)].slots.iter()), carried, "the pile fell two cells");
    drain(l);
    s.tick();
    assert_eq!(marks_of(&drain(l)).map(|m| m.0), Some(Some(fell)), "the mark went with it");
    // taken by the other, then the other drops a thing in the old and the new cell: no stale mark
    for _ in 0..3 {
        (s.clients[1].phys.pos, s.clients[1].phys.vel) = (s.world.block_centre(fell), DVec3::ZERO);
        s.tick();
    }
    assert!(!s.piles.contains_key(&key(fell)) && s.clients[0].marks.death.is_empty(), "taken: the mark goes");
    s.clients[1].phys.pos = s.spawn();
    s.lay(fell, vec![Stack::new(log, 1)], (0, 0));
    s.lay(pile, vec![Stack::new(log, 1)], (0, 0));
    *s.item_made.entry(log).or_default() += 2;
    s.tick();
    assert!(s.clients[0].marks.death.is_empty(), "another's pile in its cell does not keep the mark");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// The review: what is in a workbench's grid when the ape dies goes into its pile, and an ape that leaves
/// while dead keeps its things until it comes back and respawns: nothing lost, nothing doubled.
#[test]
fn a_death_with_the_grid_full_and_a_leave_while_dead_lose_and_double_nothing() {
    let (tx, rx) = channel();
    let mut s = Server::new(256, 32, 1, rx);
    let join = |s: &mut Server| {
        let (c, sv) = local();
        tx.send(sv).unwrap();
        c.send(ToServer::Hello { name: "ape".into() });
        c.send(ToServer::Input(Input::default()));
        s.tick();
        c
    };
    let l = join(&mut s);
    l.send(ToServer::Chat("/playmode survival".into()));
    s.tick();
    empty_hands(&mut s, 0);
    l.send(ToServer::Chat("/give oak_log 4".into()));
    s.tick();
    // the logs into a grid cell (as a workbench's window holds them)
    let k = s.clients[0].slots.iter().position(Option::is_some).unwrap();
    s.clients[0].slots[items::CRAFT_AT + 2] = s.clients[0].slots[k].take();
    let all = counted(s.clients[0].slots.iter());
    assert_eq!(all.values().sum::<u32>(), 4);
    // (it dies on land away from the spawn, where the respawned ape would pick its pile up at once)
    let b = &s.world.bodies[0];
    let sea = b.layer_r(b.sea.unwrap());
    let land = (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) }).find(|d| b.height(*d) > sea + 6.0 && (*d - DVec3::Y).length() > 0.2).expect("land");
    let ground = b.centre + land * (b.height(land) + 0.1);
    l.send(ToServer::Chat("/hurt 40".into()));
    (s.clients[0].phys.pos, s.clients[0].phys.vel) = (ground, DVec3::ZERO);
    s.tick();
    assert!(s.clients[0].vitals.dead.is_some() && s.item_ledger().is_empty());
    // gone while dead: kept with its name, still dead, ledger exact
    drop(l);
    s.tick();
    s.tick();
    assert!(s.clients.is_empty() && s.item_ledger().is_empty(), "{:?}", s.item_ledger());
    let l = join(&mut s);
    assert!(s.clients[0].vitals.dead.is_some(), "back, still dead");
    // (what it got on joining again, before its name, was laid at its feet: it is in `before`)
    let before = counted(s.piles.values().flat_map(|p| p.slots.iter()));
    l.send(ToServer::Respawn);
    s.tick();
    let after = counted(s.piles.values().flat_map(|p| p.slots.iter()));
    let laid: std::collections::BTreeMap<u16, u32> = after.iter().map(|(k, n)| (*k, n - before.get(k).copied().unwrap_or(0))).filter(|(_, n)| *n > 0).collect();
    assert_eq!(laid, all, "the pile holds all of it, the grid's too");
    assert!(s.item_ledger().is_empty(), "{:?}", s.item_ledger());
}

/// Creative loses nothing (needs.md: creative is unhurt): held under the sea it does not drown, and so never
/// leaves a pile.
#[test]
fn creative_never_dies_so_never_leaves_a_pile() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Input(Input::default()));
    s.tick();
    l.send(ToServer::Chat("/playmode creative".into()));
    l.send(ToServer::Chat("/hurt 40".into()));
    s.tick();
    let b = &s.world.bodies[0];
    let (sea, centre) = (b.layer_r(b.sea.unwrap()), b.centre);
    let dirs = || (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) });
    let d = dirs().find(|d| b.height(*d) < sea - 4.0).expect("a sea");
    let deep = centre + d * (b.height(d) + 0.1);
    let slots = s.clients[0].slots.clone();
    for _ in 0..(60.0 * TICK_HZ) as u32 {
        (s.clients[0].phys.pos, s.clients[0].phys.vel) = (deep, DVec3::ZERO);
        s.tick();
    }
    assert!(s.clients[0].vitals.dead.is_none() && s.clients[0].slots == slots && s.piles.is_empty());
}

/// `ToClient::Marks` keeps its tag on the wire (appended last, after `Fires`: 21).
#[test]
fn the_marks_message_keeps_its_tag() {
    let at = Block { body: 0, face: 2, pos: glam::IVec3::new(1, 2, 3) };
    assert_eq!(postcard::to_allocvec(&ToClient::Marks { death: Some(at), home: None }).unwrap(), vec![21, 1, 0, 2, 2, 4, 6, 0]);
}

/// The journal's cost (draft ADR journal-data): 50 named survival apes looking at the ground with full slots,
/// on a look tick with every inventory changed (its dearest tick), timed; `--release` for the number.
#[test]
fn the_journal_costs_little_a_player() {
    let (mut s, links) = server_with(50);
    let (pole, _) = on_land(&s);
    for (k, l) in links.iter().enumerate() {
        l.send(ToServer::Hello { name: format!("ape{k}") });
        l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: -1.55 });
        l.send(ToServer::Chat("/give stone 2304".into()));
    }
    s.tick();
    assert!(s.clients.iter().all(|c| !c.creative && c.slots.iter().take(9).all(|x| x.is_some())), "survival, the hotbar full");
    let n = 200;
    let t0 = std::time::Instant::now();
    for _ in 0..n {
        s.tick = 0;
        for c in &mut s.clients {
            c.inv_dirty = true;
        }
        s.learn();
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / (n * 50) as f64;
    eprintln!("the journal: {us:.2} us a player on a look tick with its slots changed");
    assert!(us < 200.0, "{us} us a player");
}

/// The slots an ape arrives with count for `have:` (draft ADR journal-data): what it got before its name, and
/// a returning ape's saved slots with an empty journal (a save from before the journal).
#[test]
fn the_slots_an_ape_arrives_with_teach_it() {
    use engine::journal::State::*;
    let (mut s, links) = server_with(2);
    let (new, back) = (&links[0], &links[1]);
    let flint = ITEMS.named("flint").unwrap();
    new.send(ToServer::Chat("/give flint 1".into()));
    s.tick();
    assert_eq!(held(&s.clients[0].slots), vec![(flint, 1)], "given before its name");
    let mut sv = saved(&s.clients[1]);
    sv.slots[3] = Some(items::Stack { item: flint, n: 1, payload: 0 });
    s.absent.insert("bob".into(), sv);
    new.send(ToServer::Hello { name: "ape".into() });
    back.send(ToServer::Hello { name: "bob".into() });
    s.tick();
    for (k, c) in s.clients.iter().enumerate() {
        assert_eq!(c.journal[engine::journal::JOURNAL.index("flint").unwrap()], Known, "client {k}");
    }
}

/// The state hash of a server whose journals are all empty is the hash from before the journal (draft ADR
/// journal-data), so old recordings replay: an absent player hashes as the 18 layout (no journal field), and a
/// present one's journal of unknowns adds nothing; one entry noticed changes it.
#[test]
fn an_empty_journal_leaves_the_state_hash_as_before() {
    let (mut s, links) = server_with(1);
    links[0].send(ToServer::Hello { name: "ape".into() });
    s.tick();
    let p = saved(&s.clients[0]);
    let old = save::Saved::<engine::vitals::Vitals, save::Marks, ()> { name: p.name.clone(), pos: p.pos, fwd: p.fwd, creative: p.creative, slots: p.slots.clone(), water: p.water, vitals: p.vitals.clone(), marks: p.marks.clone(), journal: () };
    assert_eq!(state::hashed(&p), postcard::to_allocvec(&old).unwrap(), "an absent player as before the journal");
    let h = s.hash();
    s.clients[0].journal.clear();
    assert_eq!(s.hash(), h, "a journal of unknowns adds nothing to the hash");
    s.clients[0].journal = engine::journal::JOURNAL.fresh();
    s.clients[0].journal[0] = engine::journal::State::Noticed;
    assert_ne!(s.hash(), h);
}

/// The journal changes among `got` (draft ADR journal-data), one list a message.
fn journals(got: &[ToClient]) -> Vec<Vec<(String, engine::journal::State)>> {
    got.iter().filter_map(|m| if let ToClient::Journal(v) = m { Some(v.clone()) } else { None }).collect()
}

/// The journal (draft ADR journal-data): a named survival ape is told its whole journal once, then each
/// change, and the other player nothing. A flint in its slots makes flint known and notices the hand axe it
/// points at; the clay its look meets (the look the server holds, judged twice a second) notices clay, and
/// clay in the slots makes it known; a command sets a state; a creative ape learns nothing. The journal is in
/// the state hash and a player's save, and a returning player is told the journal it left with.
#[test]
fn the_journal_learns_from_the_slots_and_the_look_and_tells_only_its_owner() {
    use engine::journal::State::{self, *};
    let (mut s, links) = server_with(3);
    let (l, friend, late) = (&links[0], &links[1], &links[2]);
    l.send(ToServer::Hello { name: "ape".into() });
    friend.send(ToServer::Hello { name: "friend".into() });
    let cell = over_dirt(&mut s, l);
    assert!(journals(&drain(friend)) == vec![vec![]], "a new ape is told its empty journal once");
    let e = |v: &[(&str, State)]| v.iter().map(|(n, t)| (n.to_string(), *t)).collect::<Vec<_>>();
    l.send(ToServer::Chat("/give flint 1".into()));
    s.tick();
    assert_eq!(journals(&drain(l)), vec![e(&[("flint", Known), ("hand-axe", Noticed)])]);
    // the look: straight down at the cell under its feet, made clay
    s.world.set_block(cell, BLOCKS.id("clay"));
    wait(&mut s, 0.6);
    assert!(journals(&drain(l)).is_empty(), "looking ahead, not at it");
    l.send(ToServer::Move { pos: s.clients[0].phys.pos.to_array(), fwd: [1.0, 0.0, 0.0], pitch: -1.55 });
    wait(&mut s, 0.6);
    assert_eq!(journals(&drain(l)), vec![e(&[("clay", Noticed)])]);
    l.send(ToServer::Chat("/give clay 1".into()));
    s.tick();
    assert_eq!(journals(&drain(l)), vec![e(&[("clay", Known), ("crucible", Noticed)])], "known, and what it points at noticed");
    assert!(journals(&drain(friend)).is_empty(), "the other player is told nothing of it");
    // a command sets one; the hash holds the journal
    let h = s.hash();
    l.send(ToServer::Chat("/journal flint unknown".into()));
    s.tick();
    assert_eq!(journals(&drain(l)), vec![e(&[("flint", Unknown)])]);
    assert_ne!(s.hash(), h, "the journal is in the state hash");
    let sv = saved(&s.clients[0]);
    assert_eq!(sv.journal, e(&[("hand-axe", Noticed), ("clay", Known), ("crucible", Noticed)]));
    // creative learns nothing
    friend.send(ToServer::Chat("/playmode creative".into()));
    friend.send(ToServer::Chat("/give flint 1".into()));
    wait(&mut s, 0.6);
    assert!(journals(&drain(friend)).is_empty() && s.clients[1].journal.iter().all(|t| *t == Unknown), "creative learns nothing");
    // back from the save: the whole journal, at once
    s.absent.insert("bob".into(), sv.clone());
    late.send(ToServer::Hello { name: "bob".into() });
    s.tick();
    // (and the flint it still holds is known again: its slots came with it)
    assert_eq!(journals(&drain(late)), vec![e(&[("flint", Known), ("hand-axe", Noticed), ("clay", Known), ("crucible", Noticed)])], "a returning ape is told its journal once");
}

/// The feel run's correction after a dig (ADR 0015, 0019): the steps a client sent before its edit were taken
/// in the world before it, so the server takes them before the edit too, however few of them it had stepped
/// when the edit came. It had stepped none of them here, dug first, and they fell where the client stood.
#[test]
fn the_steps_sent_before_an_edit_are_taken_before_it() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, over) = on_land(&s);
    let ground = Block { pos: over.pos - glam::IVec3::Z, ..over };
    l.send(ToServer::Creative(true));
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    // steps standing, all taken: it lands on the ground
    l.send(ToServer::Steps { first: s.clients[0].acked + 1, inputs: vec![Input::default(); QUEUE_MAX] });
    wait(&mut s, 2.0);
    drain(l);
    assert!(s.clients[0].phys.on_ground && s.clients[0].queue.is_empty() && s.world.block(ground) != AIR);
    // what the client predicted: six steps standing on the ground, then the dig under its feet
    let (first, inputs) = (s.clients[0].acked + 1, vec![Input::default(); 6]);
    let mut truth = s.clients[0].phys;
    for inp in &inputs {
        phys::step_at(&mut s.world, &mut truth, inp, phys::DT);
    }
    let last = first + inputs.len() as u32 - 1;
    l.send(ToServer::Steps { first, inputs });
    l.send(ToServer::Edit { seq: 1, shape: Shape::one(ground), material: AIR });
    for _ in 0..20 {
        s.tick();
        if s.clients[0].acked == last {
            break;
        }
    }
    assert!(drain(l).contains(&ToClient::Done { seq: 1, ok: true }));
    assert_eq!((s.clients[0].acked, s.world.block(ground)), (last, AIR));
    let d = s.clients[0].phys.pos.distance(truth.pos);
    assert!(d < 1e-9, "after step {last} the server's player is {d:.3} m from where the client's steps took it");
}

/// The same for a place (the feel run's hammer box over the feet): a cell the player walked into before the
/// place is kept clear on the server as in the client's prediction, judged where the steps took it.
#[test]
fn a_place_is_kept_clear_of_where_the_steps_before_it_took_the_player() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, _) = on_land(&s);
    l.send(ToServer::Creative(true));
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    l.send(ToServer::Steps { first: s.clients[0].acked + 1, inputs: vec![Input::default(); QUEUE_MAX] });
    wait(&mut s, 2.0);
    drain(l);
    // half a second walking ahead, then stone placed in the cell its body is in by then
    let (first, inputs) = (s.clients[0].acked + 1, vec![Input { mv: [1.0, 0.0], ..Default::default() }; 30]);
    let (start, mut truth) = (s.clients[0].phys, s.clients[0].phys);
    for inp in &inputs {
        phys::step_at(&mut s.world, &mut truth, inp, phys::DT);
    }
    let cell = s.world.block_at(truth.pos + phys::up(&s.world, truth.pos) * 0.5);
    assert!(s.world.block(cell) == AIR && phys::holds(&s.world, &truth, cell) && !phys::holds(&s.world, &start, cell));
    l.send(ToServer::Steps { first, inputs });
    l.send(ToServer::Edit { seq: 1, shape: Shape::one(cell), material: BLOCKS.id("stone") });
    wait(&mut s, 1.0);
    assert_eq!(s.world.block(cell), AIR, "stone placed in the cell the player had walked into");
}

/// The review: steps taken ahead of the credit at an edit stop at its floor, so a flood of a second of steps
/// and an edit, again and again in one tick, takes no more steps in that tick than a full queue and the credit.
#[test]
fn a_flood_of_steps_and_edits_takes_a_bounded_number_of_steps() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, over) = on_land(&s);
    l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    s.tick();
    let before = s.clients[0].acked;
    let mut first = before + 1;
    for k in 0..MSGS_PER_TICK as u32 / 2 {
        l.send(ToServer::Steps { first, inputs: vec![Input { mv: [1.0, 0.0], ..Default::default() }; QUEUE_MAX] });
        // refused (survival, nothing held), which must not matter
        l.send(ToServer::Edit { seq: k + 1, shape: Shape::one(over), material: BLOCKS.id("stone") });
        first += QUEUE_MAX as u32;
    }
    s.tick();
    let taken = (s.clients[0].acked - before) as usize;
    assert!(taken <= QUEUE_MAX + CREDIT_MAX as usize, "{taken} steps in one tick");
}

/// The mantle (ADR 0152): on a new world a player in a hollow at the planet's floor, hundreds of metres down,
/// digs a cell and is sent it back; a dig under the floor is refused. The save keeps the deep grid and the dug
/// cell; a save from before (no mantle) loads on its 192 m floor with its edits at the layers they were at.
#[test]
fn a_player_digs_at_the_mantles_floor_and_not_under_it() {
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let b = &s.world.bodies[0];
    assert!(b.mantle > 0, "a new world's planet has a mantle");
    let (f, i, j) = b.locate(b.spawn);
    let (f, i, j) = (f as u8, i as i32, j as i32);
    let at = |di: i32, z: i32| Block { body: 0, face: f, pos: glam::IVec3::new(i + di, j, z) };
    let feet = b.centre + b.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5) * (b.layer_r(1.0) + 0.05);
    let depth = b.radius - b.layer_r(1.0);
    for z in 1..5 {
        s.world.set_block(at(0, z), AIR);
    }
    let (c, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    drain(&c);
    c.send(ToServer::Move { pos: feet.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
    c.send(ToServer::Creative(true));
    let (wall, under) = (at(1, 2), at(0, -1));
    assert!(BLOCKS.solid(s.world.block(wall)), "the hollow's wall is rock");
    c.send(ToServer::Edit { seq: 1, shape: Shape::one(wall), material: AIR });
    c.send(ToServer::Edit { seq: 2, shape: Shape::one(under), material: AIR });
    s.tick();
    let got = drain(&c);
    assert!(got.contains(&ToClient::Done { seq: 1, ok: true }) && got.contains(&ToClient::Edits(vec![(wall, AIR)])), "{depth:.0} m down: {:?}", got.iter().filter(|m| matches!(m, ToClient::Done { .. })).collect::<Vec<_>>());
    assert!(got.contains(&ToClient::Done { seq: 2, ok: false }), "a dig under the floor is refused");
    assert!(depth > 300.0, "{depth} m");
    // the save keeps the deep grid; one from before the mantle loads on today's floor, its cells where they were
    let snap = s.snapshot();
    let back = Server::from_save(Save::from_bytes(&snap.to_bytes()).unwrap(), channel().1);
    assert_eq!((back.world.bodies[0].mantle, back.world.block(wall)), (s.world.bodies[0].mantle, AIR));
    // (a world of generation 0 has Terra and Luna alone)
    let old = Server::from_save(Save::from_bytes(&Save { gen: 0, bodies: snap.bodies[..2].to_vec(), ..snap }.to_bytes()).unwrap(), channel().1);
    let ob = &old.world.bodies[0];
    assert_eq!((ob.mantle, ob.r0.to_bits(), ob.layers), (0, engine::world::Body::new(ob.centre, ob.n, 7).r0.to_bits(), engine::world::Body::new(ob.centre, ob.n, 7).layers));
    assert_eq!((old.world.edit(wall), old.world.edit(at(0, 3))), (Some(AIR), Some(AIR)));
}

/// The metal sea burns (draft the metal sea): a survival player whose feet are in a layer-0 cell opened down to it
/// loses a point every `vitals::BURN` steps and dies of it; on the floor where it is whole, and in creative, nothing.
#[test]
fn the_metal_sea_burns_a_player_who_steps_into_it() {
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let b = &s.world.bodies[0];
    let (f, i, j) = b.locate(b.spawn);
    let (f, i, j) = (f as u8, i as i32, j as i32);
    let at = |di: i32, z: i32| Block { body: 0, face: f, pos: glam::IVec3::new(i + di, j, z) };
    let feet = |di: i32, z: f64| b.centre + b.dir(f as usize, (i + di) as f64 + 0.5, j as f64 + 0.5) * (b.layer_r(z) + 0.05);
    let (hole, floor) = (feet(0, 0.0), feet(4, 1.0));
    for z in 0..5 {
        s.world.set_block(at(0, z), AIR);
        s.world.set_block(at(4, z + 1), AIR);
    }
    let (l, sv) = local();
    tx.send(sv).unwrap();
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Input(Input::default()));
    s.tick();
    let hold = |s: &mut Server, at: DVec3, secs: f64| {
        for _ in 0..(secs * TICK_HZ) as u32 {
            (s.clients[0].phys.pos, s.clients[0].phys.vel) = (at, DVec3::ZERO);
            s.tick();
        }
    };
    hold(&mut s, floor, 2.0);
    assert_eq!(s.clients[0].vitals.health, vitals::MAX, "the whole floor does not burn");
    l.send(ToServer::Creative(true));
    hold(&mut s, hole, 2.0);
    assert_eq!(s.clients[0].vitals.health, vitals::MAX, "creative is unhurt");
    l.send(ToServer::Creative(false));
    s.tick();
    hold(&mut s, hole, 1.0);
    let lost = vitals::MAX - s.clients[0].vitals.health;
    let want = (1.0 / phys::DT) as u32 / vitals::BURN;
    assert!((want - 1..=want + 1).contains(&(lost as u32)), "lost {lost} in a second, want {want}");
    hold(&mut s, hole, 3.0);
    assert_eq!(s.clients[0].vitals.dead.as_deref(), Some("ape burned in the liquid metal"));
}

/// Lava burns by the metal sea's rule (volcanoes.md step 2): a survival player whose feet are in lava loses a point
/// every `vitals::BURN` steps and dies of it; with dry feet in the pit beside it, nothing.
#[test]
fn lava_burns_a_player_whose_feet_are_in_it() {
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let lava = engine::fluids::FLUIDS.now().kind("lava").unwrap();
    let b = s.world.bodies[0].clone();
    let (f, i, j) = b.locate(b.spawn);
    let (f, i, j) = (f as u8, i as i32, j as i32);
    let z0 = b.layer_of(b.col_height(f, i, j)).floor() as i32 - 3;
    // two pits three deep, walled apart: lava poured in one, the other dry
    for (di, z) in (0..3).flat_map(|z| [(0, z), (3, z)]) {
        s.world.set_block(Block { body: 0, face: f, pos: glam::IVec3::new(i + di, j, z0 + z) }, AIR);
    }
    {
        let World { bodies, edits, water, sky, time, .. } = &mut s.world;
        let g = engine::world::Solid::at(bodies, edits, sky, *time);
        assert!(water.pour_kind(&g, engine::liquid::Col::new(0, f, i, j), z0, 2 * U, lava));
    }
    let feet = |di: i32| b.centre + b.dir(f as usize, (i + di) as f64 + 0.5, j as f64 + 0.5) * (b.layer_r(z0 as f64) + 0.3);
    let (l, sv) = local();
    tx.send(sv).unwrap();
    l.send(ToServer::Hello { name: "ape".into() });
    l.send(ToServer::Input(Input::default()));
    s.tick();
    let hold = |s: &mut Server, at: DVec3, secs: f64| {
        for _ in 0..(secs * TICK_HZ) as u32 {
            (s.clients[0].phys.pos, s.clients[0].phys.vel) = (at, DVec3::ZERO);
            s.tick();
        }
    };
    hold(&mut s, feet(3), 2.0);
    assert_eq!(s.clients[0].vitals.health, vitals::MAX, "dry feet do not burn");
    hold(&mut s, feet(0), 1.0);
    let lost = vitals::MAX - s.clients[0].vitals.health;
    let want = (1.0 / phys::DT) as u32 / vitals::BURN;
    assert!((want - 1..=want + 1).contains(&(lost as u32)), "lost {lost} in a second, want {want}");
    hold(&mut s, feet(0), 3.0);
    assert_eq!(s.clients[0].vitals.dead.as_deref(), Some("ape burned in lava"));
}

/// The metal sea lights the hole (draft the metal sea): a layer-0 cell opened down to it is a light, a cell over it
/// is not, and a save keeps it so.
#[test]
fn a_hole_to_the_metal_sea_is_a_light_and_stays_one_through_a_save() {
    let mut s = Server::baked(1536, 32, 7, channel().1);
    let b = &s.world.bodies[0];
    let (f, i, j) = b.locate(b.spawn);
    let at = |z: i32| Block { body: 0, face: f as u8, pos: glam::IVec3::new(i as i32, j as i32, z) };
    s.world.set_block(at(0), AIR);
    s.world.set_block(at(1), AIR);
    assert_eq!((s.world.lights.get(&at(0)), s.world.lights.get(&at(1))), (Some(&engine::world::CORE_LIGHT), None));
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!(back.world.lights.get(&at(0)), Some(&engine::world::CORE_LIGHT));
}

/// A save and two recordings written by the build before the bodies were a list (save version 21,
/// `tests/fixtures`, 0163-n-bodies) load and replay as Terra and Luna to the bit: the save's world hashes as
/// it did then and is written again as this version with the same two bodies; each recording replays with no
/// checkpoint off, to the world it ended on then.
#[test]
fn a_version_21_save_and_recordings_load_as_terra_and_luna() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let save = Save::read(&dir.join("v21.sav")).unwrap();
    assert_eq!((save.version, &save.bodies, save.gen), (21, &vec![(256, None), (32, None)], 0));
    let s = Server::from_save(save, channel().1);
    assert_eq!(s.world.hash(), 0x6a8c4af3e597fcf6, "the v21 save's world as the build before made it");
    let again = s.snapshot();
    assert_eq!((again.version, &again.bodies), (save::VERSION, &vec![(256, None), (32, None)]));
    assert_eq!(Server::from_save(Save::from_bytes(&again.to_bytes()).unwrap(), channel().1).world.hash(), 0x6a8c4af3e597fcf6);
    // (v21-moving.rec: main 2fc59dbc's server on the noise land, a player walking and jumping on Terra for 200
    // ticks, then /goto luna and falling there for 300: every checkpoint hashes the player's motion)
    for (name, checks, hash) in [("v21-save.rec", 2, 0xe96d62133385dd09u64), ("v21-seed.rec", 2, 0x169db21dd9b67e54)] {
        let r = replay(&dir.join(name), None).unwrap();
        assert_eq!((r.diverged, r.checks, r.server.world.hash()), (None, checks, hash), "{name}");
        assert_eq!(r.server.world.bodies.len(), 2);
    }
    // The walk and jumps on Terra replay to the bit. The fall on that small Luna, whose pull is under FREE_G, does
    // not since bug 2026-11-21_18-02-54 was fixed: a body falling near a weak moon is held upright now.
    let r = replay(&dir.join("v21-moving.rec"), None).unwrap();
    assert!(r.diverged.map_or(true, |d| d.0 > 200), "the Terra part: {:?}", r.diverged);
    assert_eq!((r.server.world.hash(), r.server.world.bodies.len()), (0xcbf29ce484222325, 2));
    assert_eq!(r.server.world.frame_at(r.server.clients[0].phys.pos), 1, "the player ends in Luna's frame");
}

/// A corrupt save with no bodies is refused with the count, not an index out of range.
#[test]
#[should_panic(expected = "this world has 0 bodies")]
fn a_save_without_bodies_says_so() {
    Server::on_map(&[], 3, engine::world::GENERATION, channel().1);
}

/// A body's generated blocks (edits not seen): an FNV over every 97th column, 30 m under its ground to a metre over,
/// each block as a u16.
fn ground_hash(w: &engine::world::World, body: usize) -> u64 {
    use engine::world::{fnv, Block, FNV0};
    let (b, n) = (&w.bodies[body], w.bodies[body].n);
    let mut h = FNV0;
    for c in (0..6 * n * n).step_by(97) {
        let (f, i, j) = ((c / (n * n)) as u8, c % n, (c / n) % n);
        let at = |z| Block { body: body as u8, face: f, pos: glam::IVec3::new(i, j, z) };
        let top = w.column_height(at(0));
        for z in b.layer_of(top - 30.0).floor() as i32..=b.layer_of(top + 1.0).ceil() as i32 {
            h = fnv(h, &(w.generated_block(at(z)) as u16).to_le_bytes());
        }
    }
    h
}

/// A baked world of generation 4 saved by the build before Mars (`tests/fixtures/v22-gen4.sav`: seed 3, Terra
/// 256 and Luna 64, one block dug) loads as its two bodies, to the bit, and is written again with two; a new world
/// of generation 5 has Mars as its third body and its save round-trips with three.
#[test]
fn a_generation_4_save_keeps_two_bodies_and_a_generation_5_world_has_mars() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let save = Save::read(&dir.join("v22-gen4.sav")).unwrap();
    assert_eq!((save.version, save.gen, save.bodies.len()), (22, 4, 2));
    let s = Server::from_save(save, channel().1);
    assert_eq!((s.world.bodies.len(), s.world.hash()), (2, 0xf920c8a94873c097), "as the build before made it");
    // the generated ground of both bodies, taken with this sampler on the build before Mars (7367e176)
    let grounds = [0, 1].map(|b| format!("{:016x}", ground_hash(&s.world, b)));
    assert_eq!(grounds, ["19f7bbb8ae2d0bc6", "c6c6da9d37d2afbe"],"Terra's and Luna's generated blocks as the build before made them");
    assert_eq!(s.snapshot().bodies.len(), 2);
    // (a world of generation 5 has no giant: 6's, step 6a)
    let s = Server::baked_warp(256, 64, 3, 0.0, 5, channel().1);
    assert_eq!(s.world.bodies.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["Terra", "Luna", "Mars"]);
    let save = s.snapshot();
    assert_eq!((save.gen, save.bodies.len()), (5, 3));
    let back = Server::from_save(Save::from_bytes(&save.to_bytes()).unwrap(), channel().1);
    assert_eq!((back.world.bodies.len(), back.world.hash(), back.snapshot().bodies), (3, s.world.hash(), save.bodies));
}

/// Pours a fresh U into each land column of a square `r` columns round the first land off the spawn.
fn flood(s: &mut Server, r: i32) {
    let (_, k) = on_land(s);
    for (di, dj) in (-r..=r).flat_map(|a| (-r..=r).map(move |c| (a, c))) {
        let b = &s.world.bodies[0];
        let d = b.dir(k.face as usize, (k.pos.x + di) as f64 + 0.5, (k.pos.y + dj) as f64 + 0.5);
        let at = s.world.block_at(b.centre + d * (b.height(d) + 0.6));
        let World { bodies, edits, water, sky, time, .. } = &mut s.world;
        water.pour(&engine::world::Solid::at(bodies, edits, sky, *time), World::col(at), at.pos.z, 4 * engine::liquid::U);
    }
}

/// Frozen lava short of a block survives a save (the review of the two constants): a sheet of half a block frozen
/// through, with no lava beside it, is a column's remainder, booked, that a save and a load keep.
#[test]
fn a_frozen_sheet_short_of_a_block_survives_a_save() {
    let lava = engine::fluids::FLUIDS.now().kind("lava").unwrap();
    let mut s = Server::baked(1536, 32, 7, channel().1);
    let b = s.world.bodies[0].clone();
    let (f, i, j) = b.locate(b.spawn);
    let (f, i, j) = (f as u8, i as i32, j as i32);
    let z0 = b.layer_of(b.col_height(f, i, j)).floor() as i32 - 3;
    for z in 0..3 {
        s.world.set_block(Block { body: 0, face: f, pos: glam::IVec3::new(i, j, z0 + z) }, AIR);
    }
    let col = engine::liquid::Col::new(0, f, i, j);
    {
        let World { bodies, edits, water, sky, time, .. } = &mut s.world;
        let g = engine::world::Solid::at(bodies, edits, sky, *time);
        assert!(water.pour_kind(&g, col, z0, U / 2, lava));
    }
    for _ in 0..3600 {
        s.world.cool_fluids(1.0, |_| false);
        if s.world.water.total_of(lava) == 0 {
            break;
        }
    }
    assert_eq!(s.world.water.total_of(lava), 0, "it froze through");
    let held = s.world.water.crust.clone();
    assert_eq!(held.values().map(|x| x.1).sum::<i64>(), s.world.water.froze[&lava], "what froze is held, short of a block: {held:?}");
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!(back.world.water.crust, held, "through the save");
}

/// Lava's heat survives a save (lava basics, draft decision): poured lava cooled a minute keeps its temperature and
/// what froze through a save and a load, as bytes after the layout; a world of water alone writes the layout's very
/// bytes, nothing after them.
#[test]
fn poured_lavas_heat_survives_a_save_and_water_alone_writes_the_bytes_it_did() {
    let lava = engine::fluids::FLUIDS.now().kind("lava").unwrap();
    for kind in [lava, engine::fluids::WATER] {
        let mut s = Server::baked(1536, 32, 7, channel().1);
        let b = s.world.bodies[0].clone();
        let (f, i, j) = b.locate(b.spawn);
        let (f, i, j) = (f as u8, i as i32, j as i32);
        let z0 = b.layer_of(b.col_height(f, i, j)).floor() as i32 - 3;
        for z in 0..3 {
            s.world.set_block(Block { body: 0, face: f, pos: glam::IVec3::new(i, j, z0 + z) }, AIR);
        }
        let col = engine::liquid::Col::new(0, f, i, j);
        {
            let World { bodies, edits, water, sky, time, .. } = &mut s.world;
            let g = engine::world::Solid::at(bodies, edits, sky, *time);
            assert!(water.pour_kind(&g, col, z0, U / 2, kind));
        }
        // (a minute: on the world's heat, 2026-11-24, half a block of lava freezes through in minutes)
        for _ in 0..60 {
            s.world.cool_fluids(1.0, |_| false);
        }
        let snap = s.snapshot();
        let bytes = snap.to_bytes();
        if kind == engine::fluids::WATER {
            assert!(snap.heat == save::Heat::default() && bytes == postcard::to_allocvec(&snap).unwrap(), "water alone: the layout's bytes");
            continue;
        }
        let t = s.world.water.temp_at(col, z0).expect("the lava's heat");
        assert!(t < 1150.0, "cooled from its pour: {t} C");
        let back = Server::from_save(Save::from_bytes(&bytes).unwrap(), channel().1);
        assert_eq!(back.world.water.temp_at(col, z0), Some(t), "its heat through the save");
        assert_eq!(back.world.water.froze, s.world.water.froze, "what froze through the save");
        assert_eq!(back.world.water.counted_of(lava), s.world.water.counted_of(lava));
        // a short or corrupt heat tail loads the save without it, and the lava takes its pour heat again rather
        // than lying liquid for ever with none
        let layout = postcard::to_allocvec(&snap).unwrap();
        // (one cut in the last tail, the heats' fluids, keeps the heats: a save before them names none, lava's)
        let cut = Server::from_save(Save::from_bytes(&bytes[..bytes.len() - 1]).unwrap(), channel().1);
        assert_eq!(cut.world.water.temp_at(col, z0), Some(t), "its heat without the fluids' tail");
        let heat = postcard::to_allocvec(&snap.heat).unwrap();
        for bad in [[layout.clone(), heat[..heat.len() - 1].to_vec()].concat(), [layout.clone(), vec![0xff; 3]].concat()] {
            let back = Server::from_save(Save::from_bytes(&bad).expect("loads without the heat"), channel().1);
            assert_eq!(back.world.water.temp_at(col, z0), Some(1150.0), "the pour heat on a lava with none");
            assert_eq!(back.world.water.counted_of(lava), s.world.water.counted_of(lava));
        }
    }
}


/// Ice blocks of the edits are followed again on load (water's phases): saved and restored twice, each is followed
/// once, the list grows by nothing, and water's ledger is exact.
#[test]
fn ice_edits_reloaded_twice_are_each_followed_once_and_the_ledger_holds() {
    let (mut s, _links) = server_with(1);
    let ice = engine::blocks::BLOCKS.id("ice");
    let b = s.world.bodies[0].clone();
    let (f, i, j) = b.locate(b.spawn);
    let (f, i, j) = (f as u8, i as i32, j as i32);
    let top = b.layer_of(b.col_height(f, i, j)).ceil() as i32 + 2;
    let at = |k: i32| Block { body: 0, face: f, pos: glam::IVec3::new(i + k, j, top + 2) };
    for k in 0..3 {
        s.world.set_block(at(k), ice);
    }
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    let again = Server::from_save(Save::from_bytes(&back.snapshot().to_bytes()).unwrap(), channel().1);
    let watched = |s: &Server| (0..3).filter(|&k| s.world.water.ice_melted(engine::world::World::col(at(k)), top + 2).is_some()).count();
    assert_eq!((watched(&back), back.world.water.watched_ice(), back.water_ledger()), (3, 3, 0));
    assert_eq!((watched(&again), again.world.water.watched_ice(), again.water_ledger()), (3, 3, 0));
}

/// Eruptions on the server (draft eruptions): `/erupt` fills the nearest live chamber to bursting, the next heat step
/// opens it and its lava pours by the world's clock, told in the chat; two servers of one seed erupt the same to the
/// tick.
#[test]
fn an_eruption_on_the_server_comes_the_same_to_the_tick_twice() {
    let lava = engine::fluids::FLUIDS.now().kind("lava").unwrap();
    let run = || {
        let (tx, rx) = channel();
        let mut s = Server::baked(768, engine::bodies::moon_columns(768), 3, rx);
        let (l, sv) = local();
        tx.send(sv).unwrap();
        l.send(ToServer::Chat("/erupt".into()));
        let mut log = Vec::new();
        for _ in 0..120 * TICK_HZ as usize {
            s.tick();
            log.extend(drain(&l).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some((s.tick, t)) } else { None }));
            log.push((s.tick, format!("{} {}", s.world.water.erupted, s.world.water.total_of(lava))));
        }
        (log, s.world.water.all(), s.world.water.erupted)
    };
    let (a, b) = (run(), run());
    let said: Vec<&(u64, String)> = a.0.iter().filter(|l| !l.1.starts_with(|c: char| c.is_ascii_digit())).collect();
    eprintln!("{said:?}, {} m3 poured", a.2 as f64 / U as f64);
    assert!(said.iter().any(|l| l.1.contains("chamber is full")) && said.iter().any(|l| l.1.ends_with("erupts")) && said.iter().any(|l| l.1.ends_with("quiet again")), "{said:?}");
    assert!(a.2 > 10 * U, "lava poured: {}", a.2);
    assert!(a == b, "the same eruption twice");
}

/// A recording started from a save taken mid-eruption replays to the same hash: the recording carries the save's
/// tails (the chambers, the heats, what was booked), so the replay's chamber is open and pours as the recorded one.
#[test]
fn a_recording_from_a_save_taken_mid_eruption_replays_to_the_same_hash() {
    let mut s = Server::baked(768, engine::bodies::moon_columns(768), 3, channel().1);
    let k = s.world.force_eruption(s.world.bodies[0].centre).expect("a live volcano");
    while !(s.world.water.chambers[k].1 && s.world.water.erupted > 4 * U && s.tick % TICK_HZ as u64 == 0) {
        s.tick();
        assert!(s.tick < 60 * TICK_HZ as u64, "it erupted");
    }
    let mut back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    let dir = std::env::temp_dir().join(format!("technoape-erupt-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let rec = dir.join("e.rec");
    back.record(&rec).unwrap();
    for _ in 0..40 * TICK_HZ as usize {
        back.tick();
    }
    let r = replay(&rec, None).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(r.checks >= 39 && r.diverged.is_none(), "{} checks, diverged {:?}", r.checks, r.diverged);
    assert_eq!((r.server.hash(), r.server.world.water.erupted), (back.hash(), back.world.water.erupted));
}

/// The chambers through a save (draft eruptions): saved mid-eruption, the loaded server carries the fill, the open
/// conduit and the lava poured on as the running one does; a save without the chambers' tail (one from before
/// eruptions) loads with every chamber where the seed starts it; a world with no live volcano saves no chamber.
#[test]
fn a_chamber_survives_a_save_and_a_save_without_it_loads() {
    let lava = engine::fluids::FLUIDS.now().kind("lava").unwrap();
    let mut s = Server::baked(768, engine::bodies::moon_columns(768), 3, channel().1);
    let k = s.world.force_eruption(s.world.bodies[0].centre).expect("a live volcano");
    // (saved on a second, so the loaded server's heat steps, from its tick 0, fall where the running one's do)
    while !(s.world.water.chambers[k].1 && s.world.water.erupted > 4 * U && s.tick % TICK_HZ as u64 == 0) {
        s.tick();
        assert!(s.tick < 60 * TICK_HZ as u64, "it erupted");
    }
    let snap = s.snapshot();
    let bytes = snap.to_bytes();
    let mut back = Server::from_save(Save::from_bytes(&bytes).unwrap(), channel().1);
    // (the chambers to the bit, and lava's ledger whole in both: where a loaded server's stored lava runs after a load
    // is the liquid save's, not this)
    let state = |s: &Server| {
        let w = &s.world.water;
        let booked = w.counted_of(lava) + w.froze.get(&lava).copied().unwrap_or(0) + w.vapour.get(&lava).copied().unwrap_or(0);
        (w.chambers.iter().map(|c| (c.0.to_bits(), c.1)).collect::<Vec<_>>(), w.erupted, booked - w.erupted)
    };
    assert_eq!(state(&back), state(&s), "through the save");
    for _ in 0..60 * TICK_HZ as usize {
        s.tick();
        back.tick();
        assert_eq!(state(&back), state(&s), "carried on, tick {}", s.tick);
    }
    assert!(!s.world.water.chambers[k].1, "and ended");
    let tail = postcard::to_allocvec(&snap.chambers).unwrap();
    let old = Server::from_save(Save::from_bytes(&bytes[..bytes.len() - tail.len()]).unwrap(), channel().1);
    assert!(old.world.water.chambers.is_empty() && old.world.water.erupted == 0, "an old save: no chambers");
    assert_eq!(old.world.chamber(k).map(|c| c.fill), Server::baked(768, engine::bodies::moon_columns(768), 3, channel().1).world.chamber(k).map(|c| c.fill), "where the seed starts it");
    let mut quiet = Server::new(256, 32, 1, channel().1);
    for _ in 0..5 * TICK_HZ as usize {
        quiet.tick();
    }
    let snap = quiet.snapshot();
    assert!(snap.chambers == save::Chambers::default() && snap.to_bytes() == postcard::to_allocvec(&snap).unwrap(), "no live volcano: the layout's bytes");
}

/// Lays three eighths on every live column of `s` with a place, taken from its air column's account as a snowfall
/// takes them (the test's stand-in for an hour's heavy snow at a cold place).
fn snow_on(s: &mut Server) -> Vec<Col> {
    let e = 3 * engine::snow::EIGHTH;
    let cols: Vec<Col> = s.world.snow.cols.iter().filter(|(_, l)| l.floor != engine::snow::NOWHERE).map(|(c, _)| *c).collect();
    for &k in &cols {
        let l = s.world.snow.cols.get_mut(&k).unwrap();
        l.units += e;
        let cell = l.cell;
        s.world.snow.lent += e;
        s.world.snow.owed.entry(cell).or_default().0 += e;
    }
    cols
}

/// Snow in eighths on the server (ADR 0189): the columns round a player are live, three eighths laid on them
/// are sent to the client, survive a save and a load, and leave the live set with every unit back to the air on a
/// server with no player; two runs of one seed agree to the hash; both ledgers hold every tick; a dig on the ground
/// under three eighths takes the snow and leaves the block.
#[test]
fn live_snow_is_sent_saved_reloaded_and_replays_to_the_same_hash() {
    let start = || {
        let (tx, rx) = channel();
        let mut s = Server::baked(1536, 32, 7, rx);
        let (c, sv) = local();
        tx.send(sv).unwrap();
        for _ in 0..TICK_HZ as u64 + 1 {
            s.tick();
        }
        (s, c)
    };
    let (mut s, c) = start();
    assert_eq!(s.world.snow.cols.len(), 33 * 33, "the live square round the player");
    let cols = snow_on(&mut s);
    drain(&c);
    s.tick();
    while s.tick % TICK_HZ as u64 != 1 {
        s.tick();
    }
    assert_eq!((s.air_ledger(), s.water_ledger()), (Some(0), 0));
    let sent: Vec<_> = drain(&c).into_iter().filter_map(|m| if let ToClient::Layers(v) = m { Some(v) } else { None }).flatten().collect();
    assert!(sent.len() >= cols.len() * 9 / 10 && sent.iter().all(|(_, x)| x.is_some_and(|x| x.eighths >= 2)), "{} of {} columns sent", sent.len(), cols.len());
    let snap = s.snapshot();
    assert!(snap.snow != engine::snow::State::default() && snap.to_bytes().len() > postcard::to_allocvec(&snap).unwrap().len(), "a tail");
    let mut back = Server::from_save(Save::from_bytes(&snap.to_bytes()).unwrap(), channel().1);
    assert_eq!(back.world.snow.state(), s.world.snow.state(), "the snow through the save");
    assert_eq!(back.world.snow.seen, s.world.snow.seen, "and what it shows");
    assert_eq!((back.air_ledger(), back.water_ledger()), (Some(0), 0));
    for _ in 0..3 * TICK_HZ as u64 {
        back.tick();
        assert_eq!((back.air_ledger(), back.water_ledger()), (Some(0), 0), "tick {}", back.tick);
    }
    assert!(back.world.snow.cols.is_empty() && back.world.snow.lent == back.world.snow.owing(), "no player: all of it given back");
    // two runs of one seed agree
    let (mut again, _c2) = start();
    snow_on(&mut again);
    while again.tick < s.tick {
        again.tick();
    }
    for _ in 0..30 * TICK_HZ as u64 {
        s.tick();
        again.tick();
        assert_eq!((s.air_ledger(), s.water_ledger()), (Some(0), 0), "tick {}", s.tick);
    }
    assert_eq!((again.hash(), again.world.snow.state()), (s.hash(), s.world.snow.state()), "two runs of one seed");
    // a dig of the ground under three eighths takes the snow, not the block
    let me = s.world.block_at(s.clients[0].phys.pos);
    let (&k, l) = s.world.snow.cols.iter().filter(|(_, l)| l.units >= engine::snow::EIGHTH).min_by_key(|(k, _)| (k.i() - me.pos.x).abs() + (k.j() - me.pos.y).abs() + 1000 * (k.i() == me.pos.x && k.j() == me.pos.y) as i32).expect("snow left after 30 s");
    let b = Block { body: 0, face: k.face(), pos: glam::IVec3::new(k.i(), k.j(), l.floor - 1) };
    let was = s.world.block(b);
    let mut changed = Vec::new();
    assert!(s.edit(0, &engine::edit::Shape::one(b), AIR, &mut changed));
    assert_eq!((s.world.block(b), s.world.snow.cols[&k].units, changed.len()), (was, 0, 0), "the snow went, the block stayed");
    assert_eq!(s.water_ledger(), 0);
}

/// A snapshot early in a snowfall, when the columns hold only parts of a unit, keeps them (the review: a bare save
/// dropped `Lay::part`, so a reload parted from the run): the snow through the save is the snow.
#[test]
fn a_save_keeps_the_snow_short_of_a_unit() {
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let (_c, sv) = local();
    tx.send(sv).unwrap();
    for _ in 0..TICK_HZ as u64 + 1 {
        s.tick();
    }
    let k = *s.world.snow.cols.keys().next().expect("live columns");
    s.world.snow.cols.get_mut(&k).unwrap().part = 0.5;
    let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
    assert_eq!(back.world.snow.state(), s.world.snow.state(), "the part of a unit through the save");
}

/// Snow through `Server::tick` (the review; plants under snow, the developer's call 2026-11-24): `/weather snow` at a
/// time warp lays eighths round a player until a column with a clutter plant is a block of snow, the plant not drawn
/// while buried; a torch's cell and the player's own feet keep theirs at seven eighths (nothing erased, no one buried).
/// The world is saved and loaded while the plant is buried, a player joins it, and warm air melts it all back: the
/// cell generation's again (no edit), the same plant there, the torch still there. Both ledgers hold at every tick.
#[test]
fn snow_buries_a_plant_through_a_save_and_melts_back_to_the_same_plant() {
    use engine::clutter::{thing_at, VEGETATION};
    let (tx, rx) = channel();
    let mut s = Server::baked(1536, 32, 7, rx);
    let (c, sv) = local();
    tx.send(sv).unwrap();
    for _ in 0..TICK_HZ as u64 + 1 {
        s.tick();
    }
    let me = s.world.block_at(s.clients[0].phys.pos + engine::phys::up(&s.world, s.clients[0].phys.pos) * 0.2);
    let plant = |s: &Server, b: Block| thing_at(&s.world, b, 0.0, VEGETATION[2], false, |_, _| true).map(|t| (t.row, t.h));
    let cols: Vec<(Col, Block)> = (-12..=12)
        .flat_map(|i| (-12..=12).map(move |j| (i, j)))
        .filter(|&(i, j)| i * i + j * j <= 144 && (i, j) != (0, 0))
        .filter_map(|(i, j)| {
            let k = Col::new(0, me.face, me.pos.x + i, me.pos.y + j);
            let z = s.world.snow.cols.get(&k)?.floor;
            let b = Block { body: 0, face: me.face, pos: glam::IVec3::new(k.i(), k.j(), z) };
            (z != engine::snow::NOWHERE && s.world.block(b) == AIR && s.world.edit(b).is_none()).then_some((k, b))
        })
        .collect();
    let &(green, green_b) = cols.iter().find(|(_, b)| plant(&s, *b).is_some()).expect("a column with a plant");
    let grows = plant(&s, green_b);
    let mut bare = cols.iter().filter(|(k, _)| *k != green);
    let (lit, lit_b) = *bare.next().unwrap();
    let (mine, mine_b) = *bare.next().unwrap();
    let torch = BLOCKS.id("torch");
    s.world.set_block(lit_b, torch);
    // the player stands on another open column, feet in its floor's cell
    let up = engine::phys::up(&s.world, s.world.block_centre(mine_b));
    s.clients[0].phys.pos = s.world.block_centre(mine_b) - up * 0.45;
    for _ in 0..TICK_HZ as u64 {
        s.tick();
    }
    assert_eq!(s.world.block_at(s.clients[0].phys.pos + up * 0.2), mine_b, "the feet stand in the floor's cell");
    c.send(ToServer::Chat("/weather snow".into()));
    c.send(ToServer::Chat("/timewarp 10000".into()));
    let snow = BLOCKS.id("snow");
    let mut t = 0;
    while s.world.block(green_b) != snow && t < 120 * TICK_HZ as u64 {
        s.tick();
        t += 1;
        assert_eq!((s.air_ledger(), s.water_ledger()), (Some(0), 0), "tick {}", s.tick);
    }
    assert_eq!((s.world.block(green_b), plant(&s, green_b)), (snow, None), "a block of snow over the plant, the plant not drawn");
    for _ in 0..20 * TICK_HZ as u64 {
        s.tick();
    }
    let l = |s: &Server, k: Col| s.world.snow.cols[&k];
    assert_eq!((s.world.block(lit_b), l(&s, lit).units), (torch, engine::snow::BLOCK - 1), "the torch kept, the snow at seven eighths");
    assert_eq!((l(&s, mine).floor, l(&s, mine).units, s.world.block(mine_b)), (mine_b.pos.z, engine::snow::BLOCK - 1, AIR), "no one buried");
    assert_eq!((s.air_ledger(), s.water_ledger()), (Some(0), 0));
    // saved and loaded while buried; a player joins the loaded world
    let (tx, rx) = channel();
    let mut s = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), rx);
    assert_eq!((s.world.block(green_b), s.world.snow.packed.get(&(green, green_b.pos.z)).map(|p| p.1)), (snow, Some(true)), "buried through the save");
    let (c, sv) = local();
    tx.send(sv).unwrap();
    s.tick();
    drain(&c);
    // warm air melts it back
    s.force = Some(engine::weather::Force { temp: Some(20.0), ..engine::weather::Force::named("clear").unwrap().unwrap() });
    c.send(ToServer::Chat("/timewarp 10000".into()));
    let mut t = 0;
    // (blocks packed out of this player's live square stay blocks: only live columns melt)
    while s.world.block(green_b) != AIR && t < 60 * TICK_HZ as u64 {
        s.tick();
        t += 1;
        assert_eq!((s.air_ledger(), s.water_ledger()), (Some(0), 0), "tick {}", s.tick);
    }
    assert!(drain(&c).iter().any(|m| matches!(m, ToClient::Unedit(v) if v.contains(&green_b))), "the client is told the cell is generation's again");
    assert_eq!((s.world.block(green_b), s.world.edit(green_b), plant(&s, green_b)), (AIR, None, grows), "melted back, the same plant there");
    assert_eq!((s.world.block(lit_b), s.world.snow.cols[&lit].units), (torch, 0), "the torch still there, its snow melted");
}
