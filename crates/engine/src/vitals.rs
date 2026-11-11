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
/// with G Terra's pull at the surface (`phys::G`): 7.7 m/s is free, 21 m/s (a 23 m fall) kills from full.
pub const SAFE_FALL: f64 = 3.0;
/// Hunger lost per physics step standing: the whole of it in one game day.
pub const DRAIN: f64 = MAX as f64 / (crate::sky::DAY / phys::DT);
/// Walking drains this many times as fast; a block dug costs DIG points.
pub const WALK_DRAIN: f64 = 2.0;
pub const DIG: f64 = 0.05;
/// Above this hunger health comes back, a point every REGEN steps (8 s); at none it goes, a point every
/// STARVE steps (10 s), down to 1.
pub const FED: f64 = 14.0;
pub const REGEN: u32 = (8.0 / phys::DT) as u32;
pub const STARVE: u32 = (10.0 / phys::DT) as u32;

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
    (now, if n == DVec3::ZERO { (was - now).length() } else { (was - now).dot(n).abs() })
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Vitals {
    pub health: u8,
    pub hunger: f64,
    /// Steps since health last came back or went with hunger.
    pub clock: u32,
    /// How the player died, while it is dead (health 0); a respawn clears it.
    pub dead: Option<String>,
}

impl Default for Vitals {
    fn default() -> Vitals {
        Vitals { health: MAX, hunger: MAX as f64, clock: 0, dead: None }
    }
}

impl Vitals {
    /// One physics step of a living survival player: hunger drains (twice as fast `walking`), then health
    /// comes back or goes with it.
    pub fn step(&mut self, walking: bool) {
        if self.dead.is_some() {
            return;
        }
        self.hunger = (self.hunger - DRAIN * if walking { WALK_DRAIN } else { 1.0 }).max(0.0);
        // shortcut: hunger alone never kills (the floor at 1) because there is no food yet; let it kill once
        // food can be found or made
        let (every, d) = match (self.hunger > FED, self.hunger <= 0.0) {
            (true, _) if self.health < MAX => (REGEN, 1i32),
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

    /// What the HUD shows: health, and hunger in whole points (a point started counts).
    pub fn shown(&self) -> (u8, u8) {
        (self.health, self.hunger.ceil() as u8)
    }
}

/// A dead player's input: the look stays, nothing moves it. The server steps a dead player with this and
/// so does its client, so the two stay together.
pub fn still(inp: Input) -> Input {
    Input { mv: [0.0, 0.0], jump: false, fly: 0.0, down: false, turn: 0.0, tilt: 0.0, roll: 0.0, fwd: [0.0; 3], ..inp }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

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
        assert!((impact_damage(21.0) as i32 - 20).abs() <= 1, "21 m/s kills: {}", impact_damage(21.0));
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

    #[test]
    fn a_fall_hits_at_the_speed_it_fell_with() {
        let (w, mut p, up) = ground();
        run(&w, &mut p, &Input::default(), 60);
        assert!(p.on_ground);
        let (stand, _) = run(&w, &mut p, &Input::default(), 60);
        assert!(stand < 0.5, "standing loses nothing: {stand}");
        p.pos += up * 13.0;
        let (most, _) = run(&w, &mut p, &Input::default(), 150);
        let want = from(13.0, phys::pull(&w, p.pos).1);
        assert!(p.on_ground && (most - want).abs() < 0.5, "{most} m/s, a 13 m fall lands at {want}");
        assert!((impact_damage(most) as i32 - 10).abs() <= 1);
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

    #[test]
    fn health_at_none_is_death_with_its_cause() {
        let mut v = Vitals::default();
        assert!(!v.hurt(5, || "never".into()));
        assert_eq!((v.health, v.dead.clone()), (15, None));
        assert!(v.hurt(200, || "ape hit the ground at 30 m/s".into()));
        assert_eq!((v.health, v.dead.as_deref()), (0, Some("ape hit the ground at 30 m/s")));
        assert!(!v.hurt(3, || "again".into()), "the dead die once");
        assert_eq!(v.dead.as_deref(), Some("ape hit the ground at 30 m/s"));
        let h = v.hunger;
        v.step(true);
        assert_eq!((v.health, v.hunger), (0, h), "nothing drains a dead player");
    }

    #[test]
    fn a_dead_player_s_input_moves_nothing() {
        let inp = Input { mv: [1.0, 0.5], jump: true, fly: 30.0, down: true, turn: 0.3, tilt: 0.1, roll: 0.1, fwd: [1.0, 0.0, 0.0], pitch: 0.4, t: 7.0 };
        assert_eq!(still(inp), Input { pitch: 0.4, t: 7.0, ..Default::default() });
    }
}
