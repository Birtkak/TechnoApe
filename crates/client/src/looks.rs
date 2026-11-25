//! The players' looks and their figures' motion (ADR 0209): which species and colour of ape a player is in a
//! universe, and where another player's figure is between two rosters.
use glam::DVec3;
use std::time::Instant;

/// A look's model is named `ape <species> <colour>` (data/models, painted by tools/paint_figures.py).
pub const LOOKS: &str = "ape ";
/// The clips a player's figure plays (models.rs makes them from the ape's row at the player's own speeds).
pub const IDLE: &str = "p.idle";
pub const WALK: &str = "p.walk";
pub const RUN: &str = "p.run";

/// The look of the player `name` in the universe of `seed`, one of `n`: as good as random, the same on every
/// client and at every visit, another in another universe.
// shortcut: worked out from the seed and the name, so a player's look moves when looks are added to data/models;
// keep it in the server's save (and send it with the roster) once the looks are settled
pub fn look_of(seed: i32, name: &str, n: usize) -> usize {
    // FNV-1a over the seed and the name, then a mix so names that differ in their last letter spread
    let mut h = 0xcbf29ce484222325u64;
    for b in seed.to_le_bytes().into_iter().chain(name.bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h ^= h >> 32;
    (h.wrapping_mul(0x9e3779b97f4a7c15) >> 33) as usize % n.max(1)
}

/// The clip of a figure moving at `speed` m/s over the ground, and the clip's time after `strode` metres walked
/// in all and `t` seconds: a step's length is the same at any speed, so the feet do not slide.
pub fn clip_at(speed: f64, grounded: bool, strode: f64, t: f64) -> (&'static str, f64) {
    use engine::phys::{SPRINT, WALK as V};
    match speed {
        _ if !grounded => (IDLE, t),
        v if v > V * (1.0 + SPRINT) * 0.5 => (RUN, strode / (V * SPRINT)),
        v if v > 0.3 => (WALK, strode / V),
        _ => (IDLE, t),
    }
}

/// Another player as the rosters tell it (once a second, ADR 0117): eased from where its figure was at the last
/// roster to where that roster put it, over `SPAN`.
// shortcut: its figure is up to a roster late; send the players' moves at the tick's rate to those near when
// the server's writer is free
#[derive(Clone, Copy, Debug)]
pub struct Seen {
    pub from: (DVec3, DVec3),
    pub to: (DVec3, DVec3),
    pub at: Instant,
    pub span: f64,
    /// Metres its figure has walked in all: its walk clip's time.
    pub strode: f64,
}

/// Seconds a figure takes to the newest roster's place: a little over the second between rosters, so a walker
/// is still on its way when the next comes and never stands in between (the review of ADR 0209).
pub const SPAN: f64 = 1.1;

/// A jump this long between two rosters is a teleport: the figure is put there, not walked there.
const JUMP_M: f64 = 30.0;

impl Seen {
    /// The roster's newest `(feet, fwd)` at `now`.
    pub fn take(was: Option<Seen>, to: (DVec3, DVec3), now: Instant) -> Seen {
        match was {
            Some(s) if s.to.0.distance(to.0) < JUMP_M => {
                let from = (s.at(now).0, s.at(now).1);
                Seen { from, to, at: now, span: SPAN, strode: s.strode }
            }
            _ => Seen { from: to, to, at: now, span: SPAN, strode: 0.0 },
        }
    }

    /// Its feet, its look and its speed at `now`.
    pub fn at(&self, now: Instant) -> (DVec3, DVec3, f64) {
        let k = now.duration_since(self.at).as_secs_f64() / self.span;
        if k >= 1.0 {
            return (self.to.0, self.to.1, 0.0);
        }
        (self.from.0.lerp(self.to.0, k), self.from.1.lerp(self.to.1, k).normalize_or(self.to.1), self.from.0.distance(self.to.0) / self.span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_look_is_kept_per_universe_and_spread_over_the_players() {
        let names: Vec<String> = (0..200).map(|k| format!("bot{k}")).collect();
        let n = 20;
        let got: Vec<usize> = names.iter().map(|s| look_of(1, s, n)).collect();
        assert!(got.iter().all(|&k| k < n));
        assert_eq!(got, names.iter().map(|s| look_of(1, s, n)).collect::<Vec<_>>(), "the same at every visit");
        let mut count = vec![0; n];
        got.iter().for_each(|&k| count[k] += 1);
        assert!(count.iter().all(|&c| (3..=20).contains(&c)), "every look is worn, none by a crowd: {count:?}");
        let moved = names.iter().filter(|s| look_of(1, s, n) != look_of(2, s, n)).count();
        assert!(moved > 150, "another universe, another look: {moved} of 200 changed");
        assert_eq!(look_of(1, "player", 0), 0, "no looks loaded: the plain ape");
    }

    #[test]
    fn a_figure_walks_from_where_it_was_to_the_newest_roster() {
        let t0 = Instant::now();
        let fwd = DVec3::X;
        let a = Seen::take(None, (DVec3::ZERO, fwd), t0);
        assert_eq!(a.at(t0 + Duration::from_secs(5)), (DVec3::ZERO, fwd, 0.0), "first seen: it stands where it is");
        // a second later it is 4 m on: half a span after that roster the figure is half way
        let t1 = t0 + Duration::from_secs(1);
        let b = Seen::take(Some(a), (DVec3::X * 4.0, fwd), t1);
        let (p, _, v) = b.at(t1 + Duration::from_secs_f64(SPAN / 2.0));
        assert!((p.x - 2.0).abs() < 1e-9 && (v - 4.0 / SPAN).abs() < 1e-9, "at {p} going {v}");
        // past the span it stands at the roster's place
        assert_eq!(b.at(t1 + Duration::from_secs(3)), (DVec3::X * 4.0, fwd, 0.0));
        // a roster that comes early (a change of hand) starts from where the figure is, not from the last roster
        let t2 = t1 + Duration::from_millis(500);
        let c = Seen::take(Some(b), (DVec3::X * 6.0, fwd), t2);
        assert!((c.at(t2).0.x - b.at(t2).0.x).abs() < 1e-9, "no jump back or ahead: {}", c.at(t2).0);
        // and does not make it lurch: it goes on at about its speed (the review of ADR 0209)
        assert!((3.0..5.0).contains(&c.at(t2).2), "going {}", c.at(t2).2);
        // a teleport is not walked
        let d = Seen::take(Some(c), (DVec3::X * 500.0, fwd), t2 + Duration::from_secs(1));
        assert_eq!(d.at(t2 + Duration::from_secs(1)).0, DVec3::X * 500.0);
    }

    /// The review of ADR 0209: a roster a frame late left the figure standing until it came, once a second.
    #[test]
    fn a_walker_never_stands_between_its_rosters() {
        let t0 = Instant::now();
        let mut s = Seen::take(None, (DVec3::ZERO, DVec3::X), t0);
        for k in 1..8u64 {
            // rosters a second apart, give or take a frame; just before each the figure still walks
            let late = Duration::from_millis(1000 * k + if k % 2 == 0 { 16 } else { 0 });
            if k > 3 {
                let v = s.at(t0 + late - Duration::from_millis(1)).2;
                assert!((3.0..5.0).contains(&v), "before roster {k} it goes {v} m/s");
            }
            s = Seen::take(Some(s), (DVec3::X * 4.0 * k as f64, DVec3::X), t0 + late);
        }
    }

    #[test]
    fn the_clip_is_by_the_speed_and_its_time_by_the_metres_walked() {
        use engine::phys::{SPRINT, WALK as V};
        assert_eq!(clip_at(0.0, true, 7.0, 3.0), (IDLE, 3.0));
        assert_eq!(clip_at(V, true, V * 2.0, 3.0), (WALK, 2.0), "two seconds of walking is two seconds of the walk");
        assert_eq!(clip_at(V * SPRINT, true, V * SPRINT * 2.0, 3.0), (RUN, 2.0));
        assert_eq!(clip_at(V, false, 7.0, 3.0), (IDLE, 3.0), "in the air it does not walk");
    }
}
