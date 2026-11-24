//! Spectator mode (draft ADR spectator-and-sky): `/playmode spectator` flies through everything, acts on nothing and is sent to nobody.
use super::*;

/// A player put in spectator mode on dry land, looking along a tangent there: its feet and that tangent.
fn spectator(s: &mut Server, l: &Link<ToServer, ToClient>) -> (DVec3, DVec3) {
    let (pole, _) = on_land(s);
    let t = phys::tangent(DVec3::X, phys::up(&s.world, pole), DVec3::Z);
    l.send(ToServer::Move { pos: pole.to_array(), fwd: t.as_vec3().to_array(), pitch: 0.0 });
    l.send(ToServer::Chat("/playmode spectator".into()));
    s.tick();
    (pole, t)
}

/// Where the open ground of `pole`'s column is: its first solid cell from the sky down, the feet on it.
fn ground(s: &Server, pole: DVec3) -> DVec3 {
    phys::open_ground(&s.world, pole).expect("land")
}

/// A spectator flies through a wall at the asked speed and hangs where it lets go: no box, no gravity.
#[test]
fn a_spectator_passes_through_a_wall_and_does_not_fall() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, t) = spectator(&mut s, l);
    let up = phys::up(&s.world, pole);
    // a wall of stone 3 m ahead, 5 x 5
    let stone = BLOCKS.id("stone");
    let side = t.cross(up);
    for a in -2..=2 {
        for h in -1..=3 {
            let b = s.world.block_at(pole + t * 3.0 + side * a as f64 + up * h as f64);
            s.world.set_block(b, stone);
        }
    }
    let fwd = t.as_vec3().to_array();
    l.send(ToServer::Input(Input { mv: [1.0, 0.0], fwd, fly: 5.0, ..Default::default() }));
    wait(&mut s, 2.0);
    let p = s.clients[0].phys.pos;
    assert!((p - pole).dot(t) > 9.0, "through the wall: {} m on", (p - pole).dot(t));
    assert!((p - pole).dot(up).abs() < 0.5, "level: {} m", (p - pole).dot(up));
    // let go, it hangs there
    l.send(ToServer::Input(Input { fwd, ..Default::default() }));
    s.tick();
    let at = s.clients[0].phys.pos;
    wait(&mut s, 1.0);
    assert!((s.clients[0].phys.pos - at).length() < 1e-9, "fell {} m", (s.clients[0].phys.pos - at).length());
}

/// A spectator's edits, digs and moves of items are refused, and each is shown taken from a survival twin standing
/// with it, so the refusal is the mode's, not the message's (the review); its take and bucket are refused too.
#[test]
fn a_spectator_cannot_dig_or_place() {
    let (mut s, links) = server_with(2);
    let dirt = over_dirt(&mut s, &links[1]);
    let (pole, t) = spectator(&mut s, &links[0]);
    for l in &links {
        l.send(ToServer::Chat("/give stone 5".into()));
    }
    s.tick();
    assert_eq!(held(&s.clients[0].slots), vec![], "a spectator is given nothing");
    links[0].send(ToServer::Chat("/playmode survival".into()));
    links[0].send(ToServer::Chat("/give stone 5".into()));
    links[0].send(ToServer::Chat("/playmode spectator".into()));
    s.tick();
    drain(&links[0]);
    drain(&links[1]);
    let (stone, edits) = (BLOCKS.id("stone"), s.world.edit_count);
    let air = s.world.block_at(pole + t * 2.0);
    for l in &links {
        l.send(ToServer::Dig(Some(Shape::one(dirt))));
        l.send(ToServer::Edit { seq: 2, shape: Shape::one(air), material: stone });
        l.send(ToServer::Items { seq: 5, from: (items::Holder::Ape, 0), to: (items::Holder::Ape, 1), n: 1 });
        l.send(ToServer::Bucket { seq: 3, at: dirt, fill: true });
        l.send(ToServer::Take { seq: 4, at: dirt });
    }
    s.tick();
    let (spec, twin) = (drain(&links[0]), drain(&links[1]));
    for seq in [2, 3, 4, 5] {
        assert!(spec.contains(&ToClient::Done { seq, ok: false }), "{seq} refused: {spec:?}");
    }
    for seq in [2, 5] {
        assert!(twin.contains(&ToClient::Done { seq, ok: true }), "{seq} the twin's taken: {twin:?}");
    }
    assert_eq!((s.world.block(air), s.world.edit_count), (stone, edits + 1), "the twin's block only");
    assert!(s.clients[0].hold.shape().is_none() && s.clients[1].hold.shape().is_some(), "the twin's dig is held");
}

/// Entering the mode mid-dig ends the dig (the others' cracks with it); a spectator cannot turn creative on under it,
/// nor /place or /structure (the review).
#[test]
fn a_spectator_ends_its_dig_and_is_refused_the_world_commands() {
    let (mut s, links) = server_with(2);
    let l = &links[0];
    let dirt = over_dirt(&mut s, l);
    let edits = s.world.edit_count;
    l.send(ToServer::Dig(Some(Shape::one(dirt))));
    s.tick();
    assert!(s.clients[0].hold.shape().is_some());
    drain(&links[1]);
    l.send(ToServer::Chat("/playmode spectator".into()));
    s.tick();
    let id = s.clients[0].player.id;
    assert!(s.clients[0].hold.shape().is_none(), "the dig ends");
    assert!(drain(&links[1]).contains(&ToClient::Digging { id, at: None }), "and its cracks");
    l.send(ToServer::Creative(true));
    l.send(ToServer::Chat("/place stone".into()));
    l.send(ToServer::Chat("/structure hut".into()));
    s.tick();
    assert!(!s.clients[0].creative, "creative is not turned on under it");
    let said: Vec<String> = drain(l).into_iter().filter_map(|m| if let ToClient::Chat(c) = m { Some(c) } else { None }).collect();
    assert_eq!(said.iter().filter(|c| c.starts_with("a spectator")).count(), 2, "{said:?}");
    assert_eq!(s.world.edit_count, edits);
}

/// Other clients are not sent a spectator in the roster; it is sent itself, and told its mode.
#[test]
fn a_spectator_is_not_in_what_another_client_receives() {
    let (mut s, links) = server_with(2);
    spectator(&mut s, &links[0]);
    let me = s.clients[0].player.id;
    drain(&links[1]);
    assert!(drain(&links[0]).contains(&ToClient::Spectator(true)), "told its mode");
    wait(&mut s, 1.5);
    let rosters = |got: Vec<ToClient>| got.into_iter().filter_map(|m| if let ToClient::Players(v) = m { Some(v) } else { None }).collect::<Vec<_>>();
    let other = rosters(drain(&links[1]));
    assert!(!other.is_empty() && other.iter().all(|v| v.len() == 1 && v.iter().all(|p| p.id != me)), "{other:?}");
    assert!(rosters(drain(&links[0])).iter().all(|v| v.iter().any(|p| p.id == me)));
    // out of it, it is everyone's again
    links[0].send(ToServer::Chat("/playmode survival".into()));
    wait(&mut s, 1.5);
    assert!(drain(&links[0]).contains(&ToClient::Spectator(false)));
    assert!(rosters(drain(&links[1])).last().is_some_and(|v| v.iter().any(|p| p.id == me)));
}

/// A spectator is in no cell: another player builds a block where it hangs (the review: it held edits off).
#[test]
fn a_spectator_is_in_no_cell() {
    let (mut s, links) = server_with(2);
    let dirt = over_dirt(&mut s, &links[1]);
    let (pole, t) = spectator(&mut s, &links[0]);
    let at = pole + t * 2.0;
    links[0].send(ToServer::Move { pos: at.to_array(), fwd: t.as_vec3().to_array(), pitch: 0.0 });
    links[1].send(ToServer::Chat("/give stone 5".into()));
    s.tick();
    drain(&links[1]);
    let cell = s.world.block_at(at + phys::up(&s.world, at) * 0.5);
    assert!(phys::holds(&s.world, &s.clients[0].phys, cell) && !phys::holds(&s.world, &s.clients[1].phys, cell));
    let _ = dirt;
    links[1].send(ToServer::Edit { seq: 1, shape: Shape::one(cell), material: BLOCKS.id("stone") });
    s.tick();
    assert!(drain(&links[1]).contains(&ToClient::Done { seq: 1, ok: true }));
    assert_eq!(s.world.block(cell), BLOCKS.id("stone"), "built where the spectator hangs");
}

/// Leaving the mode in the air puts the player on the open ground below, standing, unhurt (the review: it fell).
#[test]
fn leaving_spectator_in_the_air_lands_on_the_ground_unhurt() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, t) = spectator(&mut s, l);
    let up = phys::up(&s.world, pole);
    l.send(ToServer::Move { pos: (pole + up * 40.0).to_array(), fwd: t.as_vec3().to_array(), pitch: 0.0 });
    l.send(ToServer::Chat("/playmode survival".into()));
    s.tick();
    let on = ground(&s, pole);
    assert!((s.clients[0].phys.pos - on).length() < 0.5, "{} m from the ground", (s.clients[0].phys.pos - on).length());
    wait(&mut s, 3.0);
    assert_eq!(s.clients[0].vitals.health, vitals::MAX, "no fall");
}

/// Leaving the mode in a cave comes out on the open ground over it, not on the cave's floor.
#[test]
fn leaving_spectator_in_a_cave_comes_out_on_the_open_ground() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, t) = spectator(&mut s, l);
    let up = phys::up(&s.world, pole);
    let side = t.cross(up);
    for a in -1..=1 {
        for b in -1..=1 {
            for h in 9..=12 {
                let c = s.world.block_at(pole + t * a as f64 + side * b as f64 - up * h as f64);
                s.world.set_block(c, AIR);
            }
        }
    }
    l.send(ToServer::Move { pos: (pole - up * 11.5).to_array(), fwd: t.as_vec3().to_array(), pitch: 0.0 });
    s.tick();
    assert!(!phys::inside(&s.world, &s.clients[0].phys), "in the cave's air");
    l.send(ToServer::Chat("/playmode survival".into()));
    s.tick();
    let p = s.clients[0].phys;
    assert!((p.pos - ground(&s, pole)).length() < 0.5, "on the open ground: {} m under the pole", (pole - p.pos).dot(up));
    assert!(!s.clients[0].spectator && !s.clients[0].creative);
}

/// Leaving spectator mode inside rock comes out on the open ground; a save made while it is in it keeps that place
/// and nothing new (no field, so a world where nobody is one saves and hashes as before).
#[test]
fn leaving_spectator_inside_rock_comes_out_above_ground() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    s.clients[0].player.name = "ghost".into();
    let (pole, t) = spectator(&mut s, l);
    let up = phys::up(&s.world, pole);
    let fwd = t.as_vec3().to_array();
    l.send(ToServer::Input(Input { down: true, fly: 10.0, fwd, ..Default::default() }));
    wait(&mut s, 1.0);
    l.send(ToServer::Input(Input { fwd, ..Default::default() }));
    s.tick();
    assert!(phys::inside(&s.world, &s.clients[0].phys), "deep in the rock: {} m down", (pole - s.clients[0].phys.pos).dot(up));
    // what a save keeps now: the open ground over it
    let kept = saved_in(&s.clients[0], &s.world);
    assert!(DVec3::from_array(kept.pos).distance(ground(&s, s.clients[0].phys.pos)) < 1e-9 && !kept.creative, "{:?}", kept.pos);
    l.send(ToServer::Chat("/playmode survival".into()));
    s.tick();
    let p = s.clients[0].phys;
    assert!(!phys::inside(&s.world, &p), "out of the rock");
    assert!((p.pos - pole).dot(up) > -3.0, "at the ground, not under it: {} m", (p.pos - pole).dot(up));
    assert!(!s.clients[0].spectator && !s.clients[0].creative);
    // /playmode alone names the mode and lists them; an unknown one says so
    l.send(ToServer::Chat("/playmode".into()));
    l.send(ToServer::Chat("/playmode ghost".into()));
    s.tick();
    let said: Vec<String> = drain(l).into_iter().filter_map(|m| if let ToClient::Chat(c) = m { Some(c) } else { None }).collect();
    assert!(said.iter().any(|c| c.starts_with("you play in survival mode") && c.contains("spectator")), "{said:?}");
    assert!(said.iter().any(|c| c.starts_with("no play mode 'ghost'")), "{said:?}");
}

/// A spectator gone quiet and taken over by its own name's Hello comes back on the open ground, as a save keeps it
/// (the review: the takeover kept it where it hung, so it fell or stood in rock).
#[test]
fn a_spectator_taken_over_comes_back_on_the_open_ground() {
    let (mut s, links) = server_with(2);
    links[0].send(ToServer::Hello { name: "ape".into() });
    let (pole, t) = spectator(&mut s, &links[0]);
    let up = phys::up(&s.world, pole);
    links[0].send(ToServer::Move { pos: (pole + up * 30.0).to_array(), fwd: t.as_vec3().to_array(), pitch: 0.0 });
    s.tick();
    for _ in 0..GHOST_TICKS {
        s.tick();
    }
    links[1].send(ToServer::Hello { name: "ape".into() });
    s.tick();
    let c = s.clients.iter().find(|c| c.player.name == "ape").unwrap();
    assert!(!c.spectator && (c.phys.pos - ground(&s, pole)).length() < 0.5, "{} m up", (c.phys.pos - pole).dot(up));
}

/// Creatures do not see a spectator: they flee and wake for the others only; but the herds near it are kept
/// awake, as near any player (the overseer's call, draft ADR spectator-and-sky).
#[test]
fn a_spectator_is_no_player_to_the_herds_or_the_harm_but_keeps_them_awake() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (_, t) = spectator(&mut s, l);
    assert!(s.herd_players().is_empty());
    // far from where it joined (the spawn's sites fall asleep), it keeps its own awake
    let b = &s.world.bodies[0];
    let d = -b.spawn;
    let far = b.centre + d * (b.height(d).max(b.radius) + 10.0);
    l.send(ToServer::Move { pos: far.to_array(), fwd: t.as_vec3().to_array(), pitch: 0.0 });
    wait(&mut s, 0.5);
    let site = engine::herds::Herds::site_of(&s.world, far - s.world.bodies[0].centre);
    assert!(s.herds.sites.get(&site).is_some_and(|h| s.herds.clock - h.near < 5), "the site under it is awake");
    // and no command harms it
    l.send(ToServer::Chat("/hurt 5".into()));
    s.tick();
    assert_eq!(s.clients[0].vitals.health, vitals::MAX);
}
