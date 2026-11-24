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

/// A spectator's edits, digs, moves of items, take and bucket are refused; the world stays as it was.
#[test]
fn a_spectator_cannot_dig_or_place() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    let (pole, _) = on_land(&s);
    let under = s.world.block_at(pole - phys::up(&s.world, pole) * 2.5);
    l.send(ToServer::Chat("/playmode creative".into()));
    spectator(&mut s, l);
    drain(l);
    let was = s.world.block(under);
    l.send(ToServer::Dig(Some(Shape::one(under))));
    l.send(ToServer::Edit { seq: 1, shape: Shape::one(under), material: AIR });
    l.send(ToServer::Edit { seq: 2, shape: Shape::one(under), material: BLOCKS.id("stone") });
    l.send(ToServer::Bucket { seq: 3, at: under, fill: true });
    l.send(ToServer::Take { seq: 4, at: under });
    l.send(ToServer::Items { seq: 5, from: (items::Holder::Ape, 0), to: (items::Holder::Ape, 1), n: 1 });
    s.tick();
    let got = drain(l);
    for seq in 1..=5 {
        assert!(got.contains(&ToClient::Done { seq, ok: false }), "{seq} refused");
    }
    assert_eq!((s.world.block(under), s.world.edit_count), (was, 0));
    assert!(s.clients[0].hold.shape().is_none(), "no dig held");
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

/// Leaving spectator mode inside rock puts the player at the nearest free place above; a save made while it is in
/// it keeps that place and nothing new (no field, so a world where nobody is one saves and hashes as before).
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
    // what a save keeps now: the place above, as a survival player's
    let kept = saved_in(&s.clients[0], &s.world);
    let mut out = s.clients[0].phys;
    phys::onto_ground(&s.world, &mut out);
    assert!(kept.pos == out.pos.to_array() && !kept.creative, "{:?}", kept.pos);
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

/// Creatures do not see a spectator: the herds are woken and stepped by the others only.
#[test]
fn a_spectator_is_no_player_to_the_herds_or_the_harm() {
    let (mut s, links) = server_with(1);
    let l = &links[0];
    spectator(&mut s, l);
    assert!(s.herd_players().is_empty());
    // and no command harms it
    l.send(ToServer::Chat("/hurt 5".into()));
    s.tick();
    assert_eq!(s.clients[0].vitals.health, vitals::MAX);
}
