//! Health and hunger (ADR 0040, docs/design/survival.md): the server's, per survival player, 0 to `MAX`
//! points each (ten segments of the HUD, a half segment a point). Health falls with an impact: the speed a
//! body loses when it hits something solid, in any direction, whatever the pull. Hunger falls with the
//! player's own physics steps, faster walking and with every block dug. Full hunger heals, empty hunger
//! hurts, down to 1. The client only draws what the server sends.
use crate::phys::{self, Input};
use glam::DVec3;
use serde::{Deserialize, Serialize};

pub const MAX: u8 = 20;
/// The impact scale, in metres of a fall on Terra: a hit at the speed a SAFE_FALL lands at costs nothing,
/// and each metre of such a fall more costs a point. In speed: points = v^2 / (2 G) - SAFE_FALL, rounded,
/// with G Terra's pull at the surface (`phys::G`): 10.8 m/s is free, 30 m/s (a 23 m fall) kills from full.
pub const SAFE_FALL: f64 = 3.0;
/// Hunger lost per physics step standing: the whole of it in one game day.
pub const DRAIN: f64 = MAX as f64 / (crate::sky::DAY / phys::DT);
/// Walking drains this many times as fast; a block dug costs DIG points.
pub const WALK_DRAIN: f64 = 2.0;
pub const DIG: f64 = 0.05;
/// Sprinting (ADR 0118) drains this many times the walk's drain, and stops at SPRINT_FLOOR hunger or less.
pub const SPRINT_DRAIN: f64 = 3.0;
pub const SPRINT_FLOOR: f64 = 6.0;
/// Above this hunger health comes back, a point every REGEN steps (8 s); at none it goes, a point every
/// STARVE steps (10 s), down to 1.
pub const FED: f64 = 14.0;
pub const REGEN: u32 = (8.0 / phys::DT) as u32;
pub const STARVE: u32 = (10.0 / phys::DT) as u32;
/// Out of breath, a point of health goes every CHOKE steps (a second), to death (ADR 0079, needs.md "Breath").
pub const CHOKE: u32 = (1.0 / phys::DT) as u32;
/// Breath comes back from none to full in this many seconds of air (needs.md).
pub const BREATHE_S: f64 = 2.0;

/// Why the head cannot breathe (needs.md "Breath"): buried to the eyes (ADR 0079), under water, or above the
/// air (`world::AIR_TOP_M` over a body with air, so anywhere on Luna). One breath and one choke rule for all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choke {
    Buried,
    Water,
    Space,
}

impl Choke {
    /// How long the breath lasts, seconds: buried, a mouth full of sand, ADR 0079's 10 s; else needs.md's 30.
    pub fn lasts(self) -> f64 {
        match self {
            Choke::Buried => 10.0,
            Choke::Water | Choke::Space => 30.0,
        }
    }
    /// The death it gives, for the death screen and the chat.
    pub fn death(self, name: &str) -> String {
        match self {
            Choke::Buried => format!("{name} was buried"),
            Choke::Water => format!("{name} drowned"),
            Choke::Space => format!("{name} ran out of air"),
        }
    }
}

/// The breath (needs.md): 0 to MAX points like the other needs, and the steps since none last cost a point. It
/// is the server's per player and not saved: a load starts it anew, as ADR 0079's buried count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breath {
    pub left: f64,
    pub clock: u32,
}

impl Default for Breath {
    fn default() -> Breath {
        Breath { left: MAX as f64, clock: 0 }
    }
}

impl Breath {
    /// What the HUD shows, whole points (a point started counts); the bar shows while it is under MAX.
    pub fn shown(&self) -> u8 {
        self.left.ceil() as u8
    }
}
/// Thirst (ADR 0145, docs/design/needs.md) lost per physics step standing: the whole of it in two thirds of a game day
/// (1,200 s); it takes hunger's rate (`rate`: a walk twice, a sprint more).
pub const THIRST_DRAIN: f64 = MAX as f64 / (crate::sky::DAY * 2.0 / 3.0 / phys::DT);
/// Thirst a step of drinking fresh water gives: empty to full in two seconds.
pub const DRINK: f64 = MAX as f64 / (2.0 / phys::DT);
/// At no thirst health goes a point every PARCH steps (10 s), to death.
pub const PARCH: u32 = (10.0 / phys::DT) as u32;

/// The fastest impact that costs nothing, m/s.
pub fn safe_speed() -> f64 {
    (2.0 * phys::G * SAFE_FALL).sqrt()
}

/// The points an impact at `speed` m/s costs.
pub fn impact_damage(speed: f64) -> u8 {
    (speed * speed / (2.0 * phys::G) - SAFE_FALL).round().clamp(0.0, u8::MAX as f64) as u8
}

/// One physics step's impact, read from its two ends: `was` is the body's motion the step began with (m/s,
/// what the step before returned), `before` and `after` the player before and after the step. Returns the
/// motion it ends with and the speed it lost to something solid: 0 when nothing stopped it.
///
/// A step that ran into something moved less than its velocity says (or the physics took the velocity into
/// the floor away): the part of the velocity that points into what was not moved is stopped, and the impact
/// is the difference to what the body came in with. Pressing against a wall loses nothing, as nothing was
/// moving into it.
pub fn impact(was: DVec3, before: &phys::Player, after: &phys::Player, dt: f64) -> (DVec3, f64) {
    let d = after.pos - before.pos;
    if d.length() > 3.0 {
        return (after.vel, 0.0); // a teleport, or a change of frame that tells the place in other axes
    }
    let left = after.vel * dt - d;
    let landed = after.on_ground && !before.on_ground;
    if left.length() < 1e-6 && !landed {
        return (after.vel, 0.0);
    }
    // (only the loss along what stopped it: turning round on the floor, which the floor's contact makes a
    // stopped step too, is no impact)
    let n = left.normalize_or_zero();
    let now = after.vel - n * after.vel.dot(n).max(0.0);
    // (a landing the physics settled without anything left over: what was lost along the body's up; the
    // walk changing way in the same step is no part of it)
    let along = if n != DVec3::ZERO { n } else { after.up };
    (now, if along == DVec3::ZERO { (was - now).length() } else { (was - now).dot(along).abs() })
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Vitals {
    pub health: u8,
    pub hunger: f64,
    /// Steps since health last came back or went with hunger.
    pub clock: u32,
    /// How the player died, while it is dead (health 0); a respawn clears it.
    pub dead: Option<String>,
    pub thirst: f64,
    /// Steps since empty thirst last cost a point.
    pub dry: u32,
}

impl Default for Vitals {
    fn default() -> Vitals {
        Vitals { health: MAX, hunger: MAX as f64, clock: 0, dead: None, thirst: MAX as f64, dry: 0 }
    }
}

impl Vitals {
    /// One physics step of a living survival player: hunger drains (twice as fast `walking`), then health
    /// comes back or goes with it.
    pub fn step(&mut self, walking: bool) {
        self.drain(if walking { WALK_DRAIN } else { 1.0 });
    }

    /// One step at `rate` times the standing drain (`step`'s, or `rate`'s).
    pub fn drain(&mut self, rate: f64) {
        if self.dead.is_some() {
            return;
        }
        self.hunger = drained(self.hunger, rate);
        // shortcut: hunger alone never kills (the floor at 1) because there is no food yet; let it kill once
        // food can be found or made
        let (every, d) = match (self.hunger > FED, self.hunger <= 0.0) {
            // (no healing while thirst is empty, however fed: `parch` takes health then)
            (true, _) if self.health < MAX && self.thirst > 0.0 => (REGEN, 1i32),
            (_, true) if self.health > 1 => (STARVE, -1),
            _ => {
                self.clock = 0;
                return;
            }
        };
        self.clock += 1;
        if self.clock >= every {
            self.clock = 0;
            self.health = (self.health as i32 + d) as u8;
        }
    }

    /// One step of thirst at `rate` times its standing drain; at none a point of health goes every PARCH
    /// steps, to death by `cause`. True when that killed.
    pub fn parch(&mut self, rate: f64, cause: impl FnOnce() -> String) -> bool {
        if self.dead.is_some() {
            return false;
        }
        self.thirst = (self.thirst - THIRST_DRAIN * rate).max(0.0);
        if self.thirst > 0.0 {
            self.dry = 0;
            return false;
        }
        self.dry += 1;
        self.dry >= PARCH && {
            self.dry = 0;
            self.hurt(1, cause)
        }
    }

    /// One step of drinking fresh water (the caller asks `World::salt`: the sea's does nothing).
    pub fn drink(&mut self) {
        if self.dead.is_none() {
            self.thirst = (self.thirst + DRINK).min(MAX as f64);
        }
    }

    /// Hunger enough to sprint (ADR 0118): over SPRINT_FLOOR (`gate` asks it of every step).
    pub fn can_sprint(hunger: f64) -> bool {
        hunger > SPRINT_FLOOR
    }

    /// `n` blocks dug.
    pub fn dug(&mut self, n: usize) {
        if self.dead.is_none() {
            self.hunger = (self.hunger - DIG * n as f64).max(0.0);
        }
    }

    /// Takes `n` points; true when that killed, with `cause` as how.
    pub fn hurt(&mut self, n: u8, cause: impl FnOnce() -> String) -> bool {
        if self.dead.is_some() || n == 0 {
            return false;
        }
        self.health = self.health.saturating_sub(n);
        if self.health == 0 {
            self.dead = Some(cause());
        }
        self.dead.is_some()
    }

    /// One step of breath `b` with the head in what it `under` cannot breathe, or in air (None). Under, the
    /// breath lasts `Choke::lasts`, then a point of health goes every `CHOKE`, to the death `Choke::death` of
    /// `name` gives; in air it comes back in `BREATHE_S`. True when that killed.
    pub fn smother(&mut self, b: &mut Breath, under: Option<Choke>, name: &str) -> bool {
        let Some(c) = under else {
            b.left = (b.left + MAX as f64 / (BREATHE_S / phys::DT)).min(MAX as f64);
            b.clock = 0;
            return false;
        };
        b.left -= MAX as f64 / (c.lasts() / phys::DT);
        // (a hair over none is none: the last step of a breath lands there to within rounding)
        if b.left > 1e-9 {
            return false;
        }
        b.left = 0.0;
        let hit = b.clock % CHOKE == 0;
        b.clock += 1;
        hit && self.hurt(1, || c.death(name))
    }

    /// What the HUD shows: health, and hunger and thirst in whole points (a point started counts).
    pub fn shown(&self) -> (u8, u8, u8) {
        (self.health, self.hunger.ceil() as u8, self.thirst.ceil() as u8)
    }
}

/// Hunger after one step at `rate` times the standing drain: the server's `Vitals::drain`, and the client's
/// prediction of it across the steps the server has not answered (ADR 0118).
pub fn drained(hunger: f64, rate: f64) -> f64 {
    (hunger - DRAIN * rate).max(0.0)
}

/// The drain of one step with `inp` that `sprinted` (what `phys::step` returned): a sprint's, a walk's or
/// standing's.
pub fn rate(inp: &Input, sprinted: bool) -> f64 {
    if sprinted {
        WALK_DRAIN * SPRINT_DRAIN
    } else if inp.mv != [0.0, 0.0] {
        WALK_DRAIN
    } else {
        1.0
    }
}

/// The input a step takes at this hunger (ADR 0118): the sprint bit only over the floor, or in creative. The
/// server asks it of each step at the hunger it has then; the client of each step it predicts, at the hunger
/// it predicts from the server's last word (`ToClient::You`), so both refuse the same step.
pub fn gate(inp: Input, hunger: f64, creative: bool) -> Input {
    Input { sprint: inp.sprint && (creative || Vitals::can_sprint(hunger)), ..inp }
}

/// A dead player's input: the look stays, nothing moves it. The server steps a dead player with this and
/// so does its client, so the two stay together.
pub fn still(inp: Input) -> Input {
    Input { mv: [0.0, 0.0], jump: false, fly: 0.0, down: false, turn: 0.0, tilt: 0.0, roll: 0.0, fwd: [0.0; 3], sprint: false, ..inp }
}

/// A sleeping place (needs.md "What death costs"): a bed of leaves (any leaves block) with a fire in one of
/// the 26 cells around it, where the ape fits: the two cells over it empty (no block, no torch) and its eye,
/// standing there, out of the water. shortcut: a torch is the only fire until the cooking fire (needs step 4),
/// and only the bed's own face is looked at; the fire block joins here when it comes.
pub fn sleeping_place(w: &crate::world::World, bed: crate::world::Block) -> bool {
    let is = |b: crate::world::Block, f: fn(&str) -> bool| crate::blocks::BLOCKS.get(w.block(b)).is_some_and(|d| f(&d.name));
    let over = |k: i32| crate::world::Block { pos: bed.pos + glam::IVec3::Z * k, ..bed };
    let feet = wake_at(w, bed);
    let eye = feet + (feet - w.block_centre(bed)).normalize() * phys::EYE;
    is(bed, |n| n.ends_with(" leaves"))
        && (1..=2).all(|k| w.block(over(k)) == crate::blocks::AIR)
        && w.water_surface(eye).is_none()
        && (-1..=1).flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| glam::IVec3::new(x, y, z)))).any(|d| d != glam::IVec3::ZERO && is(crate::world::Block { pos: bed.pos + d, ..bed }, |n| n == "torch"))
}

/// Where an ape wakes on its bed: its feet just over the bed's top, in the cell over it.
pub fn wake_at(w: &crate::world::World, bed: crate::world::Block) -> DVec3 {
    let (top, mid) = (w.block_centre(crate::world::Block { pos: bed.pos + glam::IVec3::Z, ..bed }), w.block_centre(bed));
    top + (mid - top) * 0.45
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn a_bed_of_leaves_beside_a_fire_is_a_sleeping_place() {
        use crate::blocks::BLOCKS;
        let mut w = World::new(256, 32, 1);
        let bed = w.air_over_ground(0, glam::DVec3::Y);
        let at = |d: glam::IVec3| crate::world::Block { pos: bed.pos + d, ..bed };
        let (leaves, torch, stone) = (BLOCKS.id("oak leaves"), BLOCKS.id("torch"), BLOCKS.id("stone"));
        w.set_block(bed, leaves);
        assert!(!sleeping_place(&w, bed), "leaves alone are no sleeping place");
        w.set_block(at(glam::IVec3::new(1, 1, 0)), torch);
        assert!(sleeping_place(&w, bed), "a torch beside it (corner too)");
        w.set_block(bed, stone);
        assert!(!sleeping_place(&w, bed), "stone is no bed");
        w.set_block(bed, leaves);
        w.set_block(at(glam::IVec3::new(1, 1, 0)), crate::blocks::AIR);
        w.set_block(at(glam::IVec3::new(2, 0, 0)), torch);
        assert!(!sleeping_place(&w, bed), "a torch two cells off is not beside it");
        // the review: the ape must fit on it. A torch on top (beside the bed, but where the body goes), a block
        // placed on it, and a bed under water are no sleeping place
        w.set_block(at(glam::IVec3::new(2, 0, 0)), crate::blocks::AIR);
        let top = at(glam::IVec3::Z);
        w.set_block(top, torch);
        assert!(!sleeping_place(&w, bed), "a torch on top");
        w.set_block(top, crate::blocks::AIR);
        w.set_block(at(glam::IVec3::new(1, 0, 0)), torch);
        assert!(sleeping_place(&w, bed));
        w.set_block(at(glam::IVec3::new(0, 0, 2)), stone);
        assert!(!sleeping_place(&w, bed), "a block over it, where the head goes");
        w.set_block(at(glam::IVec3::new(0, 0, 2)), crate::blocks::AIR);
        let b = &w.bodies[0];
        let sea = b.layer_r(b.sea.unwrap());
        let d = (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); glam::DVec3::new(r * a.cos(), r * a.sin(), z) }).find(|d| b.height(*d) < sea - 4.0).expect("a sea");
        let wet = w.air_over_ground(0, d);
        let wet = crate::world::Block { pos: wet.pos - glam::IVec3::Z, ..wet };
        w.set_block(wet, leaves);
        w.set_block(crate::world::Block { pos: wet.pos + glam::IVec3::X, ..wet }, torch);
        assert!(!sleeping_place(&w, wet), "a bed under water");
        // where the ape wakes: its feet on the bed's top, inside the cell over it
        let (feet, over) = (wake_at(&w, bed), w.block_centre(top));
        assert!(feet.distance(over) < 0.5 && feet.distance(w.block_centre(bed)) > 0.5 && w.block_at(feet) == top, "{feet:?}");
    }

    /// The speed a fall of `h` metres lands at under a pull of `g`.
    fn from(h: f64, g: f64) -> f64 {
        (2.0 * g * h).sqrt()
    }

    #[test]
    fn an_impact_costs_a_point_a_metre_of_terra_fall_past_three() {
        let t = |h: f64| impact_damage(from(h, phys::G));
        assert_eq!((impact_damage(0.0), t(1.2), t(3.0), impact_damage(safe_speed())), (0, 0, 0, 0));
        assert_eq!((t(4.0), t(8.0), t(13.0)), (1, 5, 10));
        assert_eq!(t(23.0), MAX, "about 23 m kills from full");
        assert!((impact_damage(30.0) as i32 - 20).abs() <= 1, "30 m/s kills: {}", impact_damage(30.0));
        assert_eq!(impact_damage(1e6), u8::MAX, "a hit at orbital speed saturates");
    }

    #[test]
    fn a_cliff_on_luna_lands_slowly_and_costs_little() {
        // Luna's pull is about a sixth of Terra's: a 20 m cliff lands at 8 m/s, a 13 m one at 6.5
        let luna = phys::G / 6.0;
        assert_eq!(impact_damage(from(13.0, luna)), 0);
        assert!(impact_damage(from(20.0, luna)) <= 1);
        assert!(impact_damage(from(60.0, luna)) < impact_damage(from(20.0, phys::G)));
    }

    /// A world, a player on its ground at the spawn and the way it faces there.
    fn ground() -> (World, phys::Player, DVec3) {
        let w = World::new(256, 32, 1);
        let at = w.bodies[0].spawn_point();
        let up = phys::up(&w, at);
        let fwd = phys::tangent(DVec3::X, up, DVec3::Z);
        (w, phys::Player::at(at, fwd), up)
    }

    /// Steps the player with `inp` for `n` steps, through `impact`; the largest impact seen and the last.
    fn run(w: &World, p: &mut phys::Player, inp: &Input, n: usize) -> (f64, f64) {
        let (mut was, mut most, mut last) = (p.vel, 0f64, 0.0);
        for _ in 0..n {
            let before = *p;
            phys::step(w, p, inp, phys::DT);
            let (now, hit) = impact(was, &before, p, phys::DT);
            (was, most, last) = (now, most.max(hit), hit);
        }
        (most, last)
    }

    /// The review: turning round in the step that lands is no part of the impact. Down at 6 m/s and
    /// forward at 4, landing with the walk reversed, is a hit of 6, not of the 10 the whole change is.
    #[test]
    fn turning_round_as_you_land_adds_nothing() {
        let (up, fwd) = (DVec3::Y, DVec3::X);
        let before = phys::Player { pos: DVec3::ZERO, vel: fwd * 4.0 - up * 6.0, fwd, on_ground: false, up };
        let vel = fwd * -4.0;
        let after = phys::Player { pos: vel * phys::DT, vel, fwd, on_ground: true, up };
        let (_, lost) = impact(before.vel, &before, &after, phys::DT);
        assert!((lost - 6.0).abs() < 1e-9, "{lost}");
    }

    #[test]
    fn a_fall_hits_at_the_speed_it_fell_with() {
        let (w, mut p, up) = ground();
        run(&w, &mut p, &Input::default(), 60);
        assert!(p.on_ground);
        let (stand, _) = run(&w, &mut p, &Input::default(), 60);
        assert!(stand < 0.5, "standing loses nothing: {stand}");
        p.pos += up * 13.0;
        let (most, _) = run(&w, &mut p, &Input::default(), 150);
        // (the pull is weaker up there, more so on this small test planet: ADR 0037; the landing lies
        // between what the pull at the top and the pull at the ground would give)
        let (lo, hi) = (from(13.0, phys::pull(&w, p.pos + up * 13.0).1), from(13.0, phys::pull(&w, p.pos).1));
        assert!(p.on_ground && most > lo - 0.5 && most < hi + 0.5, "{most} m/s, a 13 m fall lands at {lo} to {hi}");
        assert_eq!(impact_damage(most), impact_damage(hi).min(impact_damage(most)), "no more than the full pull's");
        assert!(impact_damage(most) >= 7 && impact_damage(most) <= 11, "{}", impact_damage(most));
    }

    #[test]
    fn a_sideways_hit_counts_as_much_as_a_fall() {
        // a wall two blocks ahead; flying straight at it at 30 m/s
        let (mut w, mut p, up) = ground();
        run(&w, &mut p, &Input::default(), 60);
        let fwd = p.fwd;
        let stone = crate::blocks::BLOCKS.id("stone");
        for (a, h) in [(2.5, 0.5), (2.5, 1.5), (2.5, 2.5), (3.5, 0.5), (3.5, 1.5), (3.5, 2.5)] {
            for s in [-1.0, 0.0, 1.0] {
                let b = w.block_at(p.pos + fwd * a + up * h + fwd.cross(up) * s);
                w.set_block(b, stone);
            }
        }
        p.pos += up * 0.6;
        let fly = Input { mv: [1.0, 0.0], fly: 30.0, ..Default::default() };
        let (most, last) = run(&w, &mut p, &fly, 30);
        assert!((most - 30.0).abs() < 1.0, "it hit the wall at {most} m/s");
        assert!(last < 0.5, "pressed against the wall it loses nothing: {last}");
        assert_eq!(impact_damage(most), impact_damage(from(30f64.powi(2) / (2.0 * phys::G), phys::G)));
        // walking into the same wall is not an impact worth a point
        let (walk, _) = run(&w, &mut p, &Input { mv: [-1.0, 0.0], ..Default::default() }, 30);
        let (back, _) = run(&w, &mut p, &Input { mv: [1.0, 0.0], ..Default::default() }, 60);
        assert!(walk.max(back) <= phys::WALK + 0.5 && impact_damage(walk.max(back)) == 0, "{walk} {back}");
    }

    #[test]
    fn hunger_lasts_a_day_standing_and_half_a_day_walking() {
        let day = (crate::sky::DAY / phys::DT) as u32;
        let mut v = Vitals::default();
        for _ in 0..day / 2 {
            v.step(false);
        }
        assert!((v.hunger - 10.0).abs() < 1e-6, "{}", v.hunger);
        let mut w = Vitals::default();
        for _ in 0..day / 2 {
            w.step(true);
        }
        assert!(w.hunger.abs() < 1e-6, "{}", w.hunger);
        let mut d = Vitals::default();
        d.dug(20);
        assert!((d.hunger - 19.0).abs() < 1e-9 && d.shown().1 == 19);
        d.dug(10_000);
        assert_eq!(d.hunger, 0.0, "never below none");
    }

    #[test]
    fn full_hunger_heals_and_empty_hunger_hurts_down_to_one() {
        let mut v = Vitals { health: 10, ..Default::default() };
        for _ in 0..REGEN * 3 {
            v.step(false);
        }
        assert_eq!(v.health, 13, "a point every 8 s while fed");
        let mut v = Vitals { health: 5, hunger: FED - 0.5, ..Default::default() };
        for _ in 0..REGEN * 3 {
            v.step(false);
        }
        assert_eq!(v.health, 5, "no healing at 14 or less");
        let mut v = Vitals { health: 3, hunger: 0.0, ..Default::default() };
        for _ in 0..STARVE {
            v.step(false);
        }
        assert_eq!(v.health, 2, "a point every 10 s while starving");
        for _ in 0..STARVE * 10 {
            v.step(true);
        }
        assert_eq!((v.health, v.dead.clone()), (1, None), "hunger alone never kills");
        let mut v = Vitals::default();
        v.step(false);
        assert_eq!(v.health, MAX, "never over full");
    }

    /// Steps in `s` seconds.
    fn steps(s: f64) -> u32 {
        (s / phys::DT).round() as u32
    }

    /// needs.md "Breath", one rule for three causes: the breath lasts (10 s buried, ADR 0079; 30 s under water
    /// or above the air), then a point a second, to the cause's own death.
    #[test]
    fn each_cause_holds_its_breath_then_costs_a_point_a_second_to_its_own_death() {
        for (c, lasts, death) in [(Choke::Buried, 10.0, "ape was buried"), (Choke::Water, 30.0, "ape drowned"), (Choke::Space, 30.0, "ape ran out of air")] {
            let (mut v, mut b) = (Vitals::default(), Breath::default());
            for _ in 0..steps(lasts) - 1 {
                assert!(!v.smother(&mut b, Some(c), "ape"));
            }
            assert!(v.health == MAX && b.left > 0.0 && b.shown() == 1, "{c:?}: the breath held, its last point left: {b:?}");
            for _ in 0..1 + 3 * CHOKE {
                v.smother(&mut b, Some(c), "ape");
            }
            assert_eq!((v.health, b.shown()), (MAX - 4, 0), "{c:?}: then a point a second");
            let killed = (0..100 * CHOKE).any(|_| v.smother(&mut b, Some(c), "ape"));
            assert!(killed && v.dead.as_deref() == Some(death), "{c:?}: {:?}", v.dead);
        }
    }

    /// Air starts the count again: the breath comes back in 2 s (needs.md), and a whole breath is held anew.
    #[test]
    fn air_brings_the_breath_back_in_two_seconds_and_the_count_starts_again() {
        let (mut v, mut b) = (Vitals::default(), Breath::default());
        for _ in 0..steps(30.0) - 1 {
            v.smother(&mut b, Some(Choke::Water), "ape");
        }
        for _ in 0..steps(1.0) {
            v.smother(&mut b, None, "ape");
        }
        assert!((b.left - 10.0).abs() < 0.05, "half back in a second: {}", b.left);
        for _ in 0..steps(1.0) {
            v.smother(&mut b, None, "ape");
        }
        assert_eq!(b, Breath::default(), "full in two");
        for _ in 0..steps(30.0) - 1 {
            v.smother(&mut b, Some(Choke::Water), "ape");
        }
        assert_eq!(v.health, MAX, "a whole breath held again");
        // a breath cut short by air before it ran out: the choke's clock starts anew too
        let mut d = Breath::default();
        for _ in 0..steps(10.0) + CHOKE / 2 {
            v.smother(&mut d, Some(Choke::Buried), "ape");
        }
        v.smother(&mut d, None, "ape");
        assert_eq!(d.clock, 0);
    }

    #[test]
    fn health_at_none_is_death_with_its_cause() {
        let mut v = Vitals::default();
        assert!(!v.hurt(5, || "never".into()));
        assert_eq!((v.health, v.dead.clone()), (15, None));
        assert!(v.hurt(200, || "ape hit the ground at 30 m/s".into()));
        assert_eq!((v.health, v.dead.as_deref()), (0, Some("ape hit the ground at 30 m/s")));
        assert!(!v.hurt(3, || "again".into()), "the dead die once");
        assert_eq!(v.dead.as_deref(), Some("ape hit the ground at 30 m/s"));
        let (h, th) = (v.hunger, v.thirst);
        v.step(true);
        v.parch(2.0, || "again".into());
        assert_eq!((v.health, v.hunger, v.thirst), (0, h, th), "nothing drains a dead player");
    }

    /// needs.md: thirst lasts two thirds of a day standing (1,200 s), a third walking.
    #[test]
    fn thirst_lasts_two_thirds_of_a_day_standing_and_a_third_walking() {
        let steps = |s: f64| (s / phys::DT).round() as u32;
        let mut v = Vitals::default();
        for _ in 0..steps(600.0) {
            v.parch(1.0, || "thirst".into());
        }
        assert!((v.thirst - 10.0).abs() < 1e-6, "{}", v.thirst);
        let mut w = Vitals::default();
        for _ in 0..steps(600.0) {
            w.parch(WALK_DRAIN, || "thirst".into());
        }
        assert!(w.thirst.abs() < 1e-6, "{}", w.thirst);
        assert_eq!(w.health, MAX, "empty only just now: nothing lost yet");
    }

    /// Drinking fresh water fills thirst in two seconds, never over full; the dead do not drink.
    #[test]
    fn drinking_fills_thirst_in_two_seconds() {
        let mut v = Vitals { thirst: 0.0, ..Default::default() };
        for _ in 0..(1.0 / phys::DT).round() as u32 {
            v.drink();
        }
        assert!((v.thirst - 10.0).abs() < 1e-6, "half in a second: {}", v.thirst);
        for _ in 0..(5.0 / phys::DT) as u32 {
            v.drink();
        }
        assert_eq!(v.thirst, MAX as f64);
        let mut d = Vitals { thirst: 3.0, health: 0, dead: Some("gone".into()), ..Default::default() };
        d.drink();
        assert_eq!(d.thirst, 3.0);
    }

    /// needs.md: empty thirst costs a point every 10 s, to death (an ape that never drinks nor eats dies at
    /// 1,400 s), and no health comes back while it is empty, however fed.
    #[test]
    fn empty_thirst_costs_a_point_every_ten_seconds_and_kills() {
        let mut v = Vitals { thirst: 0.0, health: 5, ..Default::default() };
        for _ in 0..PARCH - 1 {
            assert!(!v.parch(1.0, || "ape died of thirst".into()));
            v.drain(1.0);
        }
        assert_eq!(v.health, 5, "not yet, and full hunger does not heal");
        v.parch(1.0, || "ape died of thirst".into());
        assert_eq!(v.health, 4, "a point at 10 s");
        let killed = (0..PARCH * 10).any(|_| v.parch(1.0, || "ape died of thirst".into()));
        assert!(killed && v.dead.as_deref() == Some("ape died of thirst"), "{v:?}");
        let mut v = Vitals::default();
        let mut t = 0u32;
        while v.dead.is_none() && t < 10_000_000 {
            v.drain(1.0);
            v.parch(1.0, || "thirst".into());
            t += 1;
        }
        assert!((t as f64 * phys::DT - 1400.0).abs() < 1.0, "dies at {} s", t as f64 * phys::DT);
    }

    #[test]
    fn a_dead_player_s_input_moves_nothing() {
        let inp = Input { mv: [1.0, 0.5], jump: true, fly: 30.0, down: true, turn: 0.3, tilt: 0.1, roll: 0.1, fwd: [1.0, 0.0, 0.0], pitch: 0.4, t: 7.0, sprint: true };
        assert_eq!(still(inp), Input { pitch: 0.4, t: 7.0, ..Default::default() });
    }
}
