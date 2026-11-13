//! Peaceful animals in the world (ADR 0083, `docs/design/creatures.md`): herds that are a pure function of the
//! seed per site of `SITE` x `SITE` columns until a player comes near, then simulated by the server with the
//! player's own physics, and kept (the delta) once met.
use crate::phys::{self, Input, Player};
use crate::world::{fnv, World, FNV0};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Columns a site is wide, on its face.
pub const SITE: i32 = 64;
/// A site wakes when a player is this near (metres, sampled), and sleeps `SLEEP_TICKS` after the last one left.
pub const WAKE: f64 = 96.0;
pub const SLEEP_TICKS: u64 = 200;
/// At most this many animals are stepped a tick: the tick's cost cap.
pub const MAX_AWAKE: usize = 600;
/// A player this near who comes at an animal faster than `SCARE` m/s makes it flee.
pub const FEAR: f64 = 10.0;
pub const SCARE: f64 = 2.5;
/// Farther than this from home a wanderer heads home.
pub const LEASH: f64 = 24.0;

/// What an animal is doing; also its `act` on the wire (the discriminant).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mind {
    Stand,
    Graze,
    Wander,
    Flee,
    Sleep,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Animal {
    pub id: u32,
    pub kind: u16,
    pub p: Player,
    pub mind: Mind,
    /// The world tick its mind ends.
    pub until: u64,
    /// Where it walks (a tangent), for wander and flee.
    pub go: DVec3,
    /// Counts its calls: a client plays the voice when it changes.
    pub call: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Herd {
    pub home: DVec3,
    pub animals: Vec<Animal>,
    /// The world tick a player was last near.
    pub near: u64,
}

/// A creature kind the server needs: its speeds.
#[derive(Clone, Debug, PartialEq)]
pub struct Kind {
    pub name: String,
    pub walk: f64,
    pub run: f64,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub creature: String,
    pub biomes: Vec<String>,
    /// The chance a site of one of its biomes holds this herd.
    pub share: f32,
    pub size: [u32; 2],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    herd: Vec<Row>,
}

/// A site: (face, x, y) in units of `SITE` columns.
pub type Site = (u8, i32, i32);

pub struct Herds {
    pub kinds: Vec<Kind>,
    /// Per biome id, (kind, share, size) in order.
    by_biome: Vec<Vec<(u16, f32, [u32; 2])>>,
    seed: i32,
    /// The kept delta: every met site's herd (an empty one too, so it is not made again), and the next id.
    pub sites: BTreeMap<Site, Herd>,
    pub next: u32,
    /// Animals stepped in the last tick, for the stats.
    pub awake: usize,
}

/// The creatures and herds files as compiled in (shortcut: a changed file needs a rebuild of the server; read
/// them at the server's start when they are edited live).
pub fn compiled(seed: i32) -> Herds {
    Herds::parse(include_str!("../../../data/creatures.toml"), include_str!("../../../data/herds.toml"), seed).unwrap_or_else(|e| panic!("{e}"))
}

fn h64(parts: &[u64]) -> u64 {
    let h = parts.iter().fold(FNV0, |h, p| fnv(h, &p.to_le_bytes()));
    // a final mix, so nearby inputs give unrelated rolls
    let mut x = h ^ (h >> 33);
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^ (x >> 33)
}
fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

impl Herds {
    /// `creatures`: `data/creatures.toml`'s text (speeds), `herds`: `data/herds.toml`'s. Refuses an unknown
    /// creature or biome, a share outside 0..1, a size out of order or over 12.
    pub fn parse(creatures: &str, herds: &str, seed: i32) -> Result<Herds, String> {
        let rows = crate::creatures::rows(creatures)?;
        let kinds: Vec<Kind> = rows.iter().map(|c| Kind { name: c.name.clone(), walk: c.walk_speed as f64, run: c.run_speed as f64 }).collect();
        let f: File = toml::from_str(herds).map_err(|e| format!("data/herds.toml: {e}"))?;
        let names = &crate::biomes::ROWS.biomes;
        let mut by_biome = vec![Vec::new(); names.len()];
        for r in &f.herd {
            let k = kinds.iter().position(|k| k.name == r.creature).ok_or(format!("data/herds.toml: no creature '{}' in data/creatures.toml", r.creature))?;
            if !(0.0..=1.0).contains(&r.share) || r.size[0] == 0 || r.size[0] > r.size[1] || r.size[1] > 12 {
                return Err(format!("data/herds.toml: herd '{}': share 0..1 and size [lo, hi] with 1 <= lo <= hi <= 12", r.creature));
            }
            for b in &r.biomes {
                let i = names.iter().position(|n| &n.name == b).ok_or(format!("data/herds.toml: herd '{}': no biome '{b}'", r.creature))?;
                by_biome[i].push((k as u16, r.share, r.size));
            }
        }
        Ok(Herds { kinds, by_biome, seed, sites: BTreeMap::new(), next: 1, awake: 0 })
    }

    /// The site direction `d` (from body 0's centre) lies in.
    pub fn site_of(world: &World, d: DVec3) -> Site {
        let (f, i, j) = world.bodies[0].locate(d);
        (f as u8, (i as i32).div_euclid(SITE), (j as i32).div_euclid(SITE))
    }

    /// The herd the seed puts in a site: its home and its animals (none, most often), ids from `next`.
    fn generate(&self, world: &World, s: Site, next: &mut u32, tick: u64) -> Herd {
        let b = &world.bodies[0];
        let hs = h64(&[self.seed as u64, s.0 as u64, s.1 as u64, s.2 as u64]);
        let (i, j) = ((s.1 * SITE) as f64 + unit(h64(&[hs, 1])) * SITE as f64, (s.2 * SITE) as f64 + unit(h64(&[hs, 2])) * SITE as f64);
        let d = b.dir(s.0 as usize, i, j);
        let home = b.centre + d * b.height(d);
        let mut herd = Herd { home, animals: Vec::new(), near: tick };
        if i >= b.n as f64 || j >= b.n as f64 || i < 0.0 || j < 0.0 {
            return herd;
        }
        // the noise land has no biomes: it counts as grassland
        let biome = b.biome_at(s.0 as usize, i, j, d).unwrap_or(crate::bake::life::GRASSLAND) as usize;
        let Some(rows) = self.by_biome.get(biome) else { return herd };
        let mut roll = unit(h64(&[hs, 3])) as f32;
        let Some(&(kind, _, size)) = rows.iter().find(|r| {
            roll -= r.1;
            roll < 0.0
        }) else {
            return herd;
        };
        let n = size[0] + (h64(&[hs, 4]) % (size[1] - size[0] + 1) as u64) as u32;
        let (e1, e2) = (d.any_orthonormal_vector(), d.cross(d.any_orthonormal_vector()));
        for k in 0..n as u64 {
            let (a, r) = (unit(h64(&[hs, 10 + k])) * std::f64::consts::TAU, 1.0 + 5.0 * unit(h64(&[hs, 30 + k])));
            let dk = (d * b.height(d) + (e1 * a.cos() + e2 * a.sin()) * r).normalize();
            if let Some(feet) = stand(world, dk) {
                let fwd = phys::tangent(e1 * (a + 1.0).cos() + e2 * (a + 1.0).sin(), dk, e1);
                herd.animals.push(Animal { id: *next, kind, p: Player::at(feet, fwd), mind: Mind::Stand, until: tick + 20 + k * 7, go: fwd, call: 0 });
                *next += 1;
            }
        }
        herd
    }

    /// Wakes the sites near these feet (body 0's frame), making each from the seed the first time.
    fn wake(&mut self, world: &World, feet: DVec3, tick: u64) {
        let b = &world.bodies[0];
        let up = (feet - b.centre).normalize_or(DVec3::Z);
        let (e1, e2) = (up.any_orthonormal_vector(), up.cross(up.any_orthonormal_vector()));
        for x in -2..=2 {
            for y in -2..=2 {
                let p = feet + (e1 * x as f64 + e2 * y as f64) * (WAKE / 2.0);
                let s = Herds::site_of(world, p - b.centre);
                if let Some(h) = self.sites.get_mut(&s) {
                    h.near = tick;
                } else {
                    let mut next = self.next;
                    let h = self.generate(world, s, &mut next, tick);
                    self.next = next;
                    self.sites.insert(s, h);
                }
            }
        }
    }

    /// One tick: wake the sites near `players` (feet and velocity in body 0's frame), then step every awake
    /// animal by `dt` (0: no physics this tick) up to `MAX_AWAKE`. `tick` is the world's age in ticks.
    pub fn tick(&mut self, world: &World, players: &[(DVec3, DVec3)], tick: u64, dt: f64) {
        for (feet, _) in players {
            self.wake(world, *feet, tick);
        }
        let sun = world.sky.sun_dir(world.time);
        let centre = world.bodies[0].centre;
        let mut stepped = 0;
        let seed = self.seed as u64;
        for herd in self.sites.values_mut() {
            if tick.saturating_sub(herd.near) > SLEEP_TICKS || herd.animals.is_empty() {
                continue;
            }
            for a in &mut herd.animals {
                if stepped >= MAX_AWAKE {
                    break;
                }
                stepped += 1;
                let kind = &self.kinds[a.kind as usize];
                think(a, herd.home, players, sun, centre, seed, tick);
                if dt > 0.0 {
                    move_on(world, a, kind, dt, tick);
                }
            }
        }
        self.awake = stepped;
    }

    /// A real animal of `kind` standing at direction `d`, in the site it stands in (made first, so the seed's
    /// herd is not made again on top of it). None on water or with no such kind.
    pub fn add(&mut self, world: &World, kind: &str, d: DVec3, fwd: DVec3, tick: u64) -> Option<u32> {
        let k = self.kinds.iter().position(|k| k.name == kind)? as u16;
        let feet = stand(world, d)?;
        let b = &world.bodies[0];
        self.wake(world, feet, tick);
        let s = Herds::site_of(world, feet - b.centre);
        let id = self.next;
        self.next += 1;
        let fwd = phys::tangent(fwd, d, d.any_orthonormal_vector());
        self.sites.get_mut(&s)?.animals.push(Animal { id, kind: k, p: Player::at(feet, fwd), mind: Mind::Stand, until: tick + 40, go: fwd, call: 0 });
        Some(id)
    }

    /// Every animal of an awake site.
    pub fn awake_animals(&self, tick: u64) -> impl Iterator<Item = &Animal> {
        self.sites.values().filter(move |h| tick.saturating_sub(h.near) <= SLEEP_TICKS).flat_map(|h| h.animals.iter())
    }
    pub fn all(&self) -> impl Iterator<Item = &Animal> {
        self.sites.values().flat_map(|h| h.animals.iter())
    }

    /// The kept delta, for the save and the hash.
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(&(self.next, &self.sites)).expect("herds serialise")
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        let (next, sites): (u32, BTreeMap<Site, Herd>) = postcard::from_bytes(bytes).map_err(|e| format!("herds: {e}"))?;
        if sites.values().flat_map(|h| &h.animals).any(|a| a.kind as usize >= self.kinds.len()) {
            return Err("herds: an animal of a kind data/creatures.toml no longer has".into());
        }
        (self.next, self.sites) = (next, sites);
        Ok(())
    }
}

/// The feet of an animal standing at direction `d` on body 0: the air over the ground, None on water.
fn stand(world: &World, d: DVec3) -> Option<DVec3> {
    let b = &world.bodies[0];
    let c = world.air_over_ground(0, d);
    let up = (world.block_centre(c) - b.centre).normalize();
    let feet = b.centre + up * (b.layer_r(c.pos.z as f64) + 0.02);
    if world.water_surface(feet + up * 0.1).is_some() {
        return None;
    }
    Some(feet)
}

/// The ground's floor radius and the water's depth over it at direction `d`.
fn ground_at(world: &World, d: DVec3) -> (f64, f64) {
    let b = &world.bodies[0];
    let c = world.air_over_ground(0, d);
    let floor = b.layer_r(c.pos.z as f64);
    let water = world.water_surface(b.centre + d * (floor + 0.05)).map_or(0.0, |r| (r - floor).max(0.0));
    (floor, water)
}

/// Whether walking `dir` from `a` is safe for `look` metres: no drop over 2.5 m, no rise over a block, no water
/// over 0.7 m deep.
pub fn safe(world: &World, feet: DVec3, dir: DVec3, look: f64) -> bool {
    let b = &world.bodies[0];
    let up = (feet - b.centre).normalize();
    let here = (feet - b.centre).dot(up);
    let (floor, water) = ground_at(world, (feet + dir * look - b.centre).normalize());
    here - floor <= 2.5 && floor - here <= 1.05 && water <= 0.7
}

/// The heading `angle` radians about `up` from `fwd`.
fn turn(fwd: DVec3, up: DVec3, angle: f64) -> DVec3 {
    glam::DQuat::from_axis_angle(up, angle) * fwd
}

fn think(a: &mut Animal, home: DVec3, players: &[(DVec3, DVec3)], sun: DVec3, centre: DVec3, seed: u64, tick: u64) {
    let up = (a.p.pos - centre).normalize_or(DVec3::Z);
    let r = |k: u64| h64(&[seed, a.id as u64, tick, k]);
    // a player coming at it: flee, away from them
    for (feet, vel) in players {
        let rel = a.p.pos - *feet;
        let dist = rel.length();
        if dist < FEAR && dist > 1e-6 && vel.dot(rel / dist) > SCARE {
            a.go = phys::tangent(rel, up, a.p.fwd);
            if a.mind != Mind::Flee {
                a.until = tick + 80 + r(1) % 41;
            }
            a.mind = Mind::Flee;
            return;
        }
    }
    if a.mind == Mind::Flee && tick < a.until {
        return;
    }
    let night = sun.dot(up) < -0.05;
    if night {
        a.mind = Mind::Sleep;
        return;
    }
    if a.mind == Mind::Sleep || tick >= a.until {
        let roll = r(2) % 10;
        a.mind = match roll {
            0..=2 => Mind::Stand,
            3..=6 => Mind::Graze,
            _ => Mind::Wander,
        };
        a.until = tick + match a.mind {
            Mind::Graze => 80 + r(3) % 121,
            _ => 60 + r(3) % 101,
        };
        if a.mind == Mind::Wander {
            let away = home - a.p.pos;
            a.go = if away.reject_from(up).length() > LEASH {
                phys::tangent(away, up, a.p.fwd)
            } else {
                turn(phys::tangent(a.p.fwd, up, up.any_orthonormal_vector()), up, (unit(r(4)) - 0.5) * std::f64::consts::TAU)
            };
        }
    }
    // a call now and then, about every 30 s
    if r(5) % 600 == 0 {
        a.call = a.call.wrapping_add(1);
    }
}

fn move_on(world: &World, a: &mut Animal, kind: &Kind, dt: f64, tick: u64) {
    let up = phys::up(world, a.p.pos);
    // a block placed on it, or ground grown over it: it climbs out on top, as `air_over_ground` stands it
    if phys::inside(world, &a.p) {
        if let Some(feet) = stand(world, (a.p.pos - world.bodies[0].centre).normalize()) {
            (a.p.pos, a.p.vel) = (feet, DVec3::ZERO);
        }
        return;
    }
    let speed = match a.mind {
        Mind::Wander => kind.walk,
        Mind::Flee => kind.run,
        _ => 0.0,
    };
    if speed == 0.0 {
        // at rest on the ground: no physics, but once a second, so the ground dug away lets it fall
        if a.p.on_ground && a.p.vel.reject_from(up).length() < 1e-3 && (tick + a.id as u64) % 20 != 0 {
            return;
        }
        phys::step(world, &mut a.p, &Input::default(), dt);
        return;
    }
    // every 4 ticks it looks ahead: a cliff, a wall or deep water turns it, either way
    if (tick + a.id as u64) % 4 == 0 {
        let look = (speed * 0.6).max(1.5);
        if !safe(world, a.p.pos, a.go, look) {
            let side = if h64(&[a.id as u64, tick, 9]) % 2 == 0 { 1.0 } else { -1.0 };
            let left = turn(a.go, up, side * std::f64::consts::FRAC_PI_2);
            a.go = if safe(world, a.p.pos, left, look) { left } else { turn(a.go, up, -side * std::f64::consts::FRAC_PI_2) };
            if !safe(world, a.p.pos, a.go, look) {
                a.go = -turn(a.go, up, -side * std::f64::consts::FRAC_PI_2);
                if !safe(world, a.p.pos, a.go, look) {
                    // boxed in: it stands
                    (a.mind, a.until) = (Mind::Stand, tick + 40);
                    return;
                }
            }
        }
    }
    let go = phys::tangent(a.go, up, a.p.fwd);
    let mut inp = Input { mv: [(speed / phys::WALK) as f32, 0.0], fwd: go.as_vec3().to_array(), ..Input::default() };
    inp.jump = phys::ledge_ahead(world, &a.p);
    phys::step(world, &mut a.p, &inp, dt);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        World::new(256, 32, 1337)
    }
    fn herds() -> Herds {
        compiled(1337)
    }
    fn spawn(w: &World) -> DVec3 {
        let b = &w.bodies[0];
        stand(w, b.spawn).unwrap()
    }
    /// A tick at noon at the spawn (the clock's 0 is 9 am there): the sky's time is set to it.
    fn noon(w: &mut World) {
        w.time = 1800.0 * 3.0 / 24.0;
    }

    #[test]
    fn the_files_parse_and_refuse_by_name() {
        let h = herds();
        assert!(h.kinds.iter().any(|k| k.name == "deer") && h.kinds.iter().any(|k| k.name == "hare"));
        let c = include_str!("../../../data/creatures.toml");
        let bad = |t: &str| Herds::parse(c, t, 1).err().unwrap();
        assert!(bad("[[herd]]\ncreature = \"unicorn\"\nbiomes = [\"grassland\"]\nshare = 0.5\nsize = [1, 2]\n").contains("unicorn"));
        assert!(bad("[[herd]]\ncreature = \"deer\"\nbiomes = [\"moon\"]\nshare = 0.5\nsize = [1, 2]\n").contains("moon"));
        assert!(bad("[[herd]]\ncreature = \"deer\"\nbiomes = [\"grassland\"]\nshare = 1.5\nsize = [1, 2]\n").contains("share"));
        assert!(bad("[[herd]]\ncreature = \"deer\"\nbiomes = [\"grassland\"]\nshare = 0.5\nsize = [3, 2]\n").contains("size"));
    }

    #[test]
    fn herds_come_from_the_seed_alone_and_stand_on_land() {
        let w = world();
        let (mut a, mut b) = (herds(), herds());
        let p = spawn(&w);
        a.tick(&w, &[(p, DVec3::ZERO)], 0, 0.0);
        b.tick(&w, &[(p, DVec3::ZERO)], 0, 0.0);
        assert_eq!(a.to_bytes(), b.to_bytes(), "the same seed, the same herds");
        let n = a.all().count();
        assert!(n >= 3, "25 sites near the spawn hold some animals: {n}");
        for x in a.all() {
            let c = w.block_at(x.p.pos + phys::up(&w, x.p.pos) * 0.3);
            assert_eq!(w.block(c), crate::blocks::AIR, "animal {} stands in air", x.id);
            assert!(w.water_surface(x.p.pos + phys::up(&w, x.p.pos) * 0.1).is_none());
        }
        let mut other = compiled(99);
        other.tick(&w, &[(p, DVec3::ZERO)], 0, 0.0);
        assert_ne!(other.to_bytes(), a.to_bytes(), "another seed, other herds");
    }

    #[test]
    fn nobody_near_nothing_is_made_or_stepped_and_a_met_herd_keeps() {
        let mut w = world();
        noon(&mut w);
        let mut h = herds();
        h.tick(&w, &[], 0, 0.05);
        assert!(h.sites.is_empty());
        let p = spawn(&w);
        h.tick(&w, &[(p, DVec3::ZERO)], 0, 0.05);
        let met = h.sites.len();
        for t in 1..400 {
            h.tick(&w, &[(p, DVec3::ZERO)], t, 0.05);
        }
        let seen = h.to_bytes();
        // gone: after SLEEP_TICKS the herds freeze
        for t in 400..400 + SLEEP_TICKS + 1 {
            h.tick(&w, &[], t, 0.05);
        }
        let frozen = h.to_bytes();
        for t in 1000..1400 {
            h.tick(&w, &[], t, 0.05);
        }
        assert_eq!((h.awake, h.sites.len()), (0, met));
        assert_eq!(frozen, h.to_bytes(), "a sleeping site does not change");
        assert_ne!(seen, frozen, "the herd moved on in the ticks after the player left, then froze");
        let mut back = herds();
        back.load(&h.to_bytes()).unwrap();
        assert_eq!(back.to_bytes(), h.to_bytes());
    }

    #[test]
    fn they_graze_and_wander_by_day_and_sleep_at_night() {
        let mut w = world();
        noon(&mut w);
        let mut h = herds();
        let p = spawn(&w);
        let mut minds = std::collections::BTreeSet::new();
        let start: Vec<DVec3> = {
            h.tick(&w, &[(p, DVec3::ZERO)], 0, 0.05);
            h.all().map(|a| a.p.pos).collect()
        };
        for t in 1..1200 {
            h.tick(&w, &[(p, DVec3::ZERO)], t, 0.05);
            minds.extend(h.all().map(|a| a.mind as u8));
        }
        assert!(minds.contains(&(Mind::Graze as u8)) && minds.contains(&(Mind::Wander as u8)) && !minds.contains(&(Mind::Sleep as u8)), "{minds:?}");
        let moved = h.all().zip(&start).filter(|(a, s)| (a.p.pos - **s).length() > 1.0).count();
        assert!(moved * 2 >= start.len(), "most wandered: {moved} of {}", start.len());
        for a in h.all() {
            let home = h.sites.values().find(|s| s.animals.iter().any(|b| b.id == a.id)).unwrap().home;
            assert!((a.p.pos - home).length() < LEASH + 30.0, "animal {} strayed {:.1} m", a.id, (a.p.pos - home).length());
            assert!(a.p.pos.is_finite());
        }
        // midnight
        w.time += 900.0;
        h.tick(&w, &[(p, DVec3::ZERO)], 1200, 0.05);
        assert!(h.all().all(|a| a.mind == Mind::Sleep));
        let calls: u32 = h.all().map(|a| a.call as u32).sum();
        assert!(calls > 0, "some called in a minute");
    }

    #[test]
    fn a_player_running_at_one_makes_it_flee_away() {
        let mut w = world();
        noon(&mut w);
        let mut h = herds();
        let p = spawn(&w);
        h.tick(&w, &[(p, DVec3::ZERO)], 0, 0.05);
        let id = h.add(&w, "deer", (p - w.bodies[0].centre).normalize(), DVec3::X, 0).unwrap();
        let get = |h: &Herds| h.all().find(|a| a.id == id).unwrap().clone();
        let a = get(&h);
        let up = phys::up(&w, a.p.pos);
        let toward = phys::tangent(DVec3::X, up, DVec3::Y);
        let me = a.p.pos - toward * 6.0;
        h.tick(&w, &[(me, toward * 4.3)], 1, 0.05);
        assert_eq!(get(&h).mind, Mind::Flee);
        let d0 = (get(&h).p.pos - me).length();
        for t in 2..40 {
            h.tick(&w, &[(me, DVec3::ZERO)], t, 0.05);
        }
        let b = get(&h);
        assert_eq!(b.mind, Mind::Flee, "it keeps running a while");
        assert!((b.p.pos - me).length() > d0 + 3.0, "ran away: {d0:.1} -> {:.1}", (b.p.pos - me).length());
        // walking past slowly scares nothing
        let mut calm = herds();
        calm.tick(&w, &[(p, DVec3::ZERO)], 0, 0.05);
        let id = calm.add(&w, "deer", (p - w.bodies[0].centre).normalize(), DVec3::X, 0).unwrap();
        calm.tick(&w, &[(me, toward * 1.0)], 1, 0.05);
        assert_ne!(calm.all().find(|a| a.id == id).unwrap().mind, Mind::Flee);
    }

    #[test]
    fn a_cliff_or_deep_water_ahead_is_not_safe() {
        let mut w = world();
        let p = spawn(&w);
        let up = phys::up(&w, p);
        let ahead = phys::tangent(DVec3::X, up, DVec3::Y);
        assert!(safe(&w, p, ahead, 1.5) || safe(&w, p, -ahead, 1.5), "flat ground at the spawn");
        let dir = if safe(&w, p, ahead, 1.5) { ahead } else { -ahead };
        // dig a pit 4 deep across the way
        let centre = w.bodies[0].centre;
        for k in 0..5 {
            for s in [-1.0, 0.0, 1.0] {
                let q = p + dir * (1.0 + k as f64 * 0.5) + up.cross(dir) * s;
                let c = w.air_over_ground(0, (q - centre).normalize());
                for z in 1..=5 {
                    let mut cell = c;
                    cell.pos.z -= z;
                    w.set_block(cell, crate::blocks::AIR);
                }
            }
        }
        assert!(!safe(&w, p, dir, 1.5), "a pit 5 deep ahead");
    }

    #[test]
    fn a_herd_by_a_pit_never_falls_in() {
        let mut w = world();
        noon(&mut w);
        let p = spawn(&w);
        let centre = w.bodies[0].centre;
        let up = phys::up(&w, p);
        // a moat 4 deep, 2 wide, in a ring 6 m out
        let (e1, e2) = (up.any_orthonormal_vector(), up.cross(up.any_orthonormal_vector()));
        for k in 0..240 {
            let a = k as f64 / 240.0 * std::f64::consts::TAU;
            for r in [6.0, 6.5, 7.0, 7.5] {
                let q = p + (e1 * a.cos() + e2 * a.sin()) * r;
                let c = w.air_over_ground(0, (q - centre).normalize());
                for z in 1..=4 {
                    let mut cell = c;
                    cell.pos.z -= z;
                    w.set_block(cell, crate::blocks::AIR);
                }
            }
        }
        let mut h = herds();
        h.tick(&w, &[(p, DVec3::ZERO)], 0, 0.05);
        h.sites.values_mut().for_each(|s| s.animals.clear());
        let ids: Vec<u32> = (0..6).filter_map(|k| h.add(&w, "deer", (p + e1 * (k as f64 * 0.5 - 1.5) - centre).normalize(), e2, 0)).collect();
        assert_eq!(ids.len(), 6);
        let start: Vec<f64> = h.all().map(|a| (a.p.pos - centre).length()).collect();
        let mut low = f64::MAX;
        for t in 1..2400 {
            h.tick(&w, &[(p + up * 200.0, DVec3::ZERO)], t, 0.05);
            for (a, s0) in h.all().zip(&start) {
                low = low.min((a.p.pos - centre).length() - s0);
            }
        }
        // the moat is 4 deep; the ground around it rolls by a block or two
        assert!(low > -2.5, "one fell in the moat: {low:.2} m");
        assert!(h.all().zip(&start).any(|(a, _)| (a.p.pos - p).reject_from(up).length() > 2.5), "they did wander inside the ring");
    }

    /// The tick's cost with a few hundred animals awake (ADR 0083): players spread over the noise land of the
    /// 4 km planet, each waking its 25 sites. `cargo test --release -p engine herds_tick_cost -- --ignored`.
    #[test]
    #[ignore]
    fn herds_tick_cost() {
        let mut w = World::new(6144, 1664, 1337);
        noon(&mut w);
        let b = &w.bodies[0];
        let mut h = herds();
        let players: Vec<(DVec3, DVec3)> = (0..400).filter_map(|k| stand(&w, (b.spawn + DVec3::new(0.06 * (k % 20) as f64 - 0.6, 0.06 * (k / 20) as f64 - 0.6, 0.0)).normalize()).map(|p| (p, DVec3::ZERO))).step_by(5).take(40).collect();
        for t in 0..200 {
            h.tick(&w, &players, t, 0.05);
        }
        let mut ms = Vec::new();
        for t in 200..800 {
            let s = std::time::Instant::now();
            h.tick(&w, &players, t, 0.05);
            ms.push(s.elapsed().as_secs_f64() * 1000.0);
        }
        ms.sort_by(f64::total_cmp);
        let walking = h.awake_animals(800).filter(|a| matches!(a.mind, Mind::Wander | Mind::Flee)).count();
        println!("{{\"herds_tick\":{{\"players\":{},\"awake\":{},\"walking\":{walking},\"mean_ms\":{:.3},\"p99_ms\":{:.3},\"max_ms\":{:.3}}}}}", players.len(), h.awake, ms.iter().sum::<f64>() / ms.len() as f64, ms[ms.len() * 99 / 100], ms[ms.len() - 1]);
        assert!(h.awake >= 200, "a few hundred awake: {}", h.awake);
    }
}
