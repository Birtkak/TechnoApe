//! A furnace placed in the world (ADR 0141): its five slots (what goes in, the fuel, the mould, what
//! comes out, and a second input for an alloy: draft ADR tin-and-bronze), its heat and the fire burning in it. The server steps it every tick; a client draws the same
//! struct in its window. The rows it works by are `recipes::RECIPES`' `melt`, `cast`, `fuel` and `furnace`.
use crate::items::{Slot, Stack, ITEMS};
use crate::recipes::{FurnaceDef, Recipes, Work};
use serde::{Deserialize, Serialize};

/// The slots, by index.
pub const IN: usize = 0;
pub const FUEL: usize = 1;
pub const MOULD: usize = 2;
pub const OUT: usize = 3;
/// The second input, beside the first (draft ADR tin-and-bronze): last, so the first four keep their numbers on
/// the wire and in recordings.
pub const IN2: usize = 4;
pub const SLOTS: usize = 5;
/// The air's heat (C), what a furnace with no fire falls back to. shortcut: one number, not the weather's air
/// where it stands; a cold planet's furnace would cool to its own air.
pub const AIR_C: f32 = 20.0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Furnace {
    pub slots: Vec<Slot>,
    /// C.
    pub heat: f32,
    /// Seconds the fuel item burning now has left, and its fire's heat (C).
    pub burn: f32,
    pub flame: f32,
    /// Seconds the current work has had at its heat, and that work's name (a change of it starts again).
    pub work: f32,
    pub doing: Option<String>,
    /// The cast chosen in the window (a `cast` row's name); None: ingots stay ingots.
    pub choice: Option<String>,
}

impl Default for Furnace {
    fn default() -> Furnace {
        Furnace { slots: vec![None; SLOTS], heat: AIR_C, burn: 0.0, flame: 0.0, work: 0.0, doing: None, choice: None }
    }
}

/// Whether `out` can take `s` whole (one stack of its item).
fn room(out: Slot, s: (u16, u16)) -> bool {
    out.map_or(s.1 <= ITEMS.stack(s.0), |o| o.item == s.0 && o.payload == 0 && o.n + s.1 <= ITEMS.stack(s.0))
}

impl Furnace {
    /// The work its slots hold now, whatever the heat: of the works `f` reaches, its inputs there (enough of
    /// them: a work `with` a second input one in each input slot, either order; any other in one input slot with
    /// the other empty), for a cast its mould there and its shape chosen, and room for the output.
    pub fn wants<'a>(&self, f: &FurnaceDef, r: &'a Recipes) -> Option<&'a Work> {
        let (a, b) = (self.slots[IN], self.slots[IN2]);
        let has = |s: Slot, (item, n): (u16, u16)| s.is_some_and(|s| s.item == item && s.n >= n);
        let shape = self.choice.as_deref().and_then(|c| r.shape(c));
        r.works.iter().filter(|w| w.heat <= f.top).find(|w| {
            let held = match (w.with, a.is_some(), b.is_some()) {
                (Some(x), true, true) => has(a, w.input) && has(b, x) || has(b, w.input) && has(a, x),
                (None, true, false) => has(a, w.input),
                (None, false, true) => has(b, w.input),
                _ => false,
            };
            held && w.mould.is_none_or(|m| self.slots[MOULD].is_some_and(|s| s.item == m) && shape.is_some() && w.shape.as_deref() == shape) && room(self.slots[OUT], w.out)
        })
    }

    /// Takes `n` of `item` from the input slot that holds it (`wants` found it there).
    fn take(&mut self, (item, n): (u16, u16)) {
        let k = if self.slots[IN].is_some_and(|s| s.item == item) { IN } else { IN2 };
        let s = self.slots[k].unwrap();
        self.slots[k] = (s.n > n).then_some(Stack { n: s.n - n, ..s });
    }

    /// Its fire burns.
    pub fn burning(&self) -> bool {
        self.burn > 0.0
    }

    /// Nothing in it, cold, no fire and nothing chosen: no different from a fresh one.
    pub fn idle(&self) -> bool {
        self.slots.iter().all(Option::is_none) && !self.burning() && self.heat <= AIR_C && self.choice.is_none()
    }

    /// One step of `dt` seconds: a fuel item is lit when the fire is out and there is work (none is burned with
    /// nothing to work); the heat rises toward the fire's own heat held to the furnace's top, or falls toward the
    /// air without a fire; the work counts its seconds while the heat is at its own, then takes its input and
    /// gives its output. Returns each item it used (-) or made (+), for the items' ledger.
    pub fn step(&mut self, f: &FurnaceDef, r: &Recipes, dt: f32) -> Vec<(u16, i64)> {
        let mut moved = Vec::new();
        let want = self.wants(f, r);
        if !self.burning() && want.is_some() {
            if let Some((s, fuel)) = self.slots[FUEL].and_then(|s| Some((s, r.fuel(s.item)?))) {
                self.slots[FUEL] = (s.n > 1).then_some(Stack { n: s.n - 1, ..s });
                (self.burn, self.flame) = (fuel.secs, fuel.heat);
                moved.push((s.item, -1));
            }
        }
        if self.burning() {
            let to = self.flame.min(f.top);
            self.heat = if self.heat < to { (self.heat + f.rise * dt).min(to) } else { (self.heat - f.fall * dt).max(to) };
            self.burn = (self.burn - dt).max(0.0);
        } else {
            self.heat = (self.heat - f.fall * dt).max(AIR_C);
        }
        let Some(w) = want else {
            (self.work, self.doing) = (0.0, None);
            return moved;
        };
        if self.doing.as_deref() != Some(w.name.as_str()) {
            (self.work, self.doing) = (0.0, Some(w.name.clone()));
        }
        if self.heat >= w.heat {
            self.work += dt;
            // (a whole number of ticks makes the seconds; the sum of f32 steps lands a hair either side)
            if self.work >= w.secs - 1e-3 {
                for i in std::iter::once(w.input).chain(w.with) {
                    self.take(i);
                    moved.push((i.0, -(i.1 as i64)));
                }
                self.slots[OUT] = Some(Stack::new(w.out.0, self.slots[OUT].map_or(0, |o| o.n) + w.out.1));
                moved.push((w.out.0, w.out.1 as i64));
                self.work = 0.0;
            }
        }
        moved
    }
}

/// Whether a move leaves the slots `new` (were `old`) as a furnace keeps them: five (either input slot any item); the fuel slot a fuel or
/// nothing, the mould slot a mould or nothing; nothing put into what comes out (only taken from it).
pub fn fits(new: &[Slot], old: &[Slot], r: &Recipes) -> bool {
    new.len() == SLOTS
        && new[FUEL].is_none_or(|s| r.fuel(s.item).is_some())
        && new[MOULD].is_none_or(|s| r.mould(s.item))
        && new[OUT].is_none_or(|s| old[OUT].is_some_and(|o| o.item == s.item && s.n <= o.n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::RECIPES;

    fn id(n: &str) -> u16 {
        ITEMS.named(n).unwrap_or_else(|| panic!("no item {n}"))
    }

    fn crucible() -> &'static FurnaceDef {
        RECIPES.furnace(crate::blocks::BLOCKS.id("crucible")).expect("the crucible is a furnace")
    }

    /// Steps `f` for `secs` at the server's tick, adding up what it used and made.
    fn run(f: &mut Furnace, secs: f32, ledger: &mut std::collections::BTreeMap<u16, i64>) {
        for _ in 0..(secs * 20.0).round() as u32 {
            for (k, d) in f.step(crucible(), &RECIPES, 0.05) {
                *ledger.entry(k).or_default() += d;
            }
        }
    }

    fn with(slots: [(usize, &str, u16); 2]) -> Furnace {
        let mut f = Furnace::default();
        for (k, n, c) in slots {
            if c > 0 {
                f.slots[k] = Some(Stack::new(id(n), c));
            }
        }
        f
    }

    fn holding(slots: &[(usize, &str, u16)]) -> Furnace {
        let mut f = Furnace::default();
        for &(k, n, c) in slots {
            f.slots[k] = Some(Stack::new(id(n), c));
        }
        f
    }

    type Ledger = std::collections::BTreeMap<u16, i64>;

    /// Draft ADR tin-and-bronze: 3 copper ingots with 1 tin, one in each input slot either way round, melt into
    /// 4 bronze ingots, taking exactly those; the copper left over alone works nothing.
    #[test]
    fn bronze_needs_both_inputs_either_way_round_and_takes_exactly_three_and_one() {
        let (cu, sn, bz, log) = (id("copper ingot"), id("tin ingot"), id("bronze ingot"), id("oak log"));
        let w = RECIPES.works.iter().find(|w| w.out.0 == bz && w.mould.is_none()).expect("a bronze melt");
        assert_eq!((w.input, w.with, w.out.1, w.heat, w.secs), ((cu, 3), Some((sn, 1)), 4, 1085.0, 12.0));
        for (a, b) in [(IN, IN2), (IN2, IN)] {
            let mut f = holding(&[(a, "copper ingot", 7), (b, "tin ingot", 1), (FUEL, "oak log", 1)]);
            let mut led = Ledger::new();
            run(&mut f, 60.0, &mut led);
            assert_eq!((f.slots[a], f.slots[b], f.slots[OUT]), (Some(Stack::new(cu, 4)), None, Some(Stack::new(bz, 4))), "{a}, {b}: {f:?}");
            assert_eq!(led, [(log, -1), (cu, -3), (sn, -1), (bz, 4)].into_iter().collect::<Ledger>(), "{a}, {b}");
        }
    }

    /// Draft ADR tin-and-bronze: one of an alloy's inputs alone, too little of one, or a one-input melt beside
    /// another item works nothing, burns nothing and loses nothing; a one-input melt works from either slot.
    #[test]
    fn one_input_of_an_alloy_alone_does_nothing_and_one_input_melts_never_share() {
        for slots in [
            vec![(IN, "copper ingot", 3)],
            vec![(IN2, "copper ingot", 3)],
            vec![(IN, "tin ingot", 5)],
            vec![(IN, "copper ingot", 2), (IN2, "tin ingot", 1)],
            vec![(IN, "copper ingot", 3), (IN2, "tin pebble", 4)],
            vec![(IN, "copper gravel", 2), (IN2, "tin gravel", 2)],
        ] {
            let mut f = holding(&[slots.as_slice(), &[(FUEL, "oak log", 2)]].concat());
            let before = f.slots.clone();
            let mut led = Ledger::new();
            run(&mut f, 60.0, &mut led);
            assert!(f.slots == before && !f.burning() && led.is_empty(), "{slots:?}: {f:?}");
        }
        let mut f = holding(&[(IN2, "copper gravel", 2), (FUEL, "oak log", 1)]);
        run(&mut f, 40.0, &mut Ledger::new());
        assert_eq!((f.slots[IN2], f.slots[OUT]), (None, Some(Stack::new(id("copper ingot"), 1))), "{f:?}");
    }

    /// Draft ADR tin-and-bronze: tin melts at 232 C in 4 s from 4 pebbles, 2 gravel or 1 cassiterite rubble; a
    /// stick's fire is enough.
    #[test]
    fn tin_melts_from_pebbles_gravel_and_rubble_on_a_stick_fire() {
        let sn = id("tin ingot");
        for (n, k) in [("tin pebble", 4), ("tin gravel", 2), ("cassiterite rubble", 1)] {
            let w = RECIPES.works.iter().find(|w| w.input.0 == id(n)).unwrap_or_else(|| panic!("no melt of {n}"));
            assert_eq!((w.input.1, w.with, w.out, w.heat, w.secs), (k, None, (sn, 1), 232.0, 4.0), "{n}");
            let mut f = holding(&[(IN, n, k), (FUEL, "stick", 1)]);
            let mut led = Ledger::new();
            run(&mut f, 8.0, &mut led);
            assert_eq!((f.slots[IN], f.slots[OUT]), (None, Some(Stack::new(sn, 1))), "{n}: {f:?}");
            assert_eq!(led, [(id("stick"), -1), (id(n), -(k as i64)), (sn, 1)].into_iter().collect::<Ledger>());
        }
    }

    /// Draft ADR tin-and-bronze: the window offers each shape once; the copper row's name (what a recording
    /// chooses) casts bronze from bronze ingots and copper from copper.
    #[test]
    fn a_bronze_cast_is_chosen_by_its_shape_and_the_metal_picks_the_row() {
        let top = crucible().top;
        let shapes: Vec<&str> = RECIPES.shapes(top).iter().map(|w| w.shape.as_deref().unwrap()).collect();
        assert_eq!(shapes, ["pick head", "axe head", "shovel head", "knife blade", "hammer head"]);
        assert_eq!(RECIPES.casts(top).count(), 10);
        for (metal, head) in [("bronze ingot", "bronze pick head"), ("copper ingot", "copper pick head")] {
            let mut f = holding(&[(IN, metal, 5), (MOULD, "clay mould", 1), (FUEL, "oak log", 1)]);
            f.choice = Some("cast pick head".into());
            let mut led = Ledger::new();
            run(&mut f, 30.0, &mut led);
            assert_eq!((f.slots[IN], f.slots[OUT]), (Some(Stack::new(id(metal), 1)), Some(Stack::new(id(head), 2))), "{metal}: {f:?}");
            assert_eq!(led.get(&id(metal)), Some(&-4));
        }
        let w = RECIPES.work_making(id("bronze hammer head")).unwrap();
        assert_eq!((w.input, w.heat, w.mould, w.shape.as_deref()), ((id("bronze ingot"), 3), 950.0, Some(id("clay mould")), Some("hammer head")));
    }

    #[test]
    fn heat_rises_with_fuel_to_the_cap_and_falls_without() {
        let c = crucible();
        assert_eq!(c.top, 1100.0);
        // a log's fire is hotter than the cap: the heat stops at 1100
        let mut f = with([(IN, "copper gravel", 64.min(crate::items::ITEMS.stack(id("copper gravel")))), (FUEL, "oak log", 1)]);
        let mut led = Default::default();
        run(&mut f, 1.0, &mut led);
        assert!(f.burning() && f.heat > AIR_C && f.heat < c.top, "{f:?}");
        let rising = f.heat;
        run(&mut f, 1.0, &mut led);
        assert!(f.heat > rising);
        run(&mut f, 15.0, &mut led);
        assert_eq!(f.heat, c.top, "held to the furnace's top");
        // a coal lump is hotter still: still 1100
        assert!(RECIPES.fuel(id("coal lump")).unwrap().heat > c.top && RECIPES.fuel(id("oak log")).unwrap().heat >= 1100.0);
        // a stick's fire is cool: the heat heads for its own and no further
        let stick = RECIPES.fuel(id("stick")).unwrap();
        assert!(stick.heat < 1085.0 && stick.secs < RECIPES.fuel(id("oak log")).unwrap().secs);
        let mut s = with([(IN, "copper gravel", 4), (FUEL, "stick", 1)]);
        run(&mut s, stick.secs - 0.1, &mut led);
        assert!(s.heat <= stick.heat && s.heat > AIR_C);
        assert_eq!(s.slots[OUT], None, "a stick never melts copper");
        // no fire: back down to the air's
        f.slots[FUEL] = None;
        f.slots[IN] = None;
        run(&mut f, 120.0, &mut led);
        assert!(!f.burning() && f.heat == AIR_C, "{f:?}");
        assert!(f.idle() || f.slots[OUT].is_some());
    }

    #[test]
    fn a_melt_happens_only_at_its_heat_takes_its_seconds_and_counts_every_item() {
        let (gravel, ingot, log) = (id("copper gravel"), id("copper ingot"), id("oak log"));
        let melt = RECIPES.works.iter().find(|w| w.input.0 == gravel).unwrap();
        assert_eq!((melt.input.1, melt.out, melt.heat, melt.mould), (2, (ingot, 1), 1085.0, None));
        let mut f = with([(IN, "copper gravel", 4), (FUEL, "oak log", 2)]);
        let mut led = std::collections::BTreeMap::new();
        // until the heat reaches 1085 nothing counts
        let c = crucible();
        let warm = (melt.heat - AIR_C) / c.rise;
        run(&mut f, warm - 0.5, &mut led);
        assert!(f.heat < melt.heat && f.work == 0.0 && f.slots[OUT].is_none(), "{f:?}");
        // then its seconds, and one ingot from two gravel
        run(&mut f, 0.5 + melt.secs - 0.2, &mut led);
        assert!(f.slots[OUT].is_none(), "not yet: {f:?}");
        run(&mut f, 0.3, &mut led);
        assert_eq!((f.slots[OUT], f.slots[IN]), (Some(Stack::new(ingot, 1)), Some(Stack::new(gravel, 2))));
        run(&mut f, melt.secs + 0.1, &mut led);
        assert_eq!((f.slots[OUT], f.slots[IN]), (Some(Stack::new(ingot, 2)), None));
        // the ledger: 4 gravel used, 2 ingots made, one log burned (the next is not lit: nothing to work)
        run(&mut f, 60.0, &mut led);
        assert_eq!(led, [(log, -1), (gravel, -4), (ingot, 2)].into_iter().collect::<std::collections::BTreeMap<_, _>>(), "{f:?}");
        assert_eq!(f.slots[FUEL], Some(Stack::new(log, 1)));
        // the other melts: nuggets, and the two ores' rubble
        for (n, k) in [("copper nugget", 4), ("malachite rubble", 1), ("chalcopyrite rubble", 1)] {
            let w = RECIPES.works.iter().find(|w| w.input.0 == id(n)).unwrap_or_else(|| panic!("no melt of {n}"));
            assert_eq!((w.input.1, w.out, w.heat), (k, (ingot, 1), 1085.0), "{n}");
        }
    }

    #[test]
    fn no_fuel_burns_with_nothing_to_work() {
        let mut led = std::collections::BTreeMap::new();
        // fuel alone, or with too little to melt, or a full output: no fire
        for mut f in [with([(FUEL, "oak log", 3), (IN, "", 0)]), with([(FUEL, "oak log", 3), (IN, "copper gravel", 1)]), with([(FUEL, "oak log", 3), (IN, "flint", 5)])] {
            run(&mut f, 30.0, &mut led);
            assert!(!f.burning() && f.slots[FUEL] == Some(Stack::new(id("oak log"), 3)), "{f:?}");
        }
        let mut full = with([(FUEL, "oak log", 3), (IN, "copper gravel", 2)]);
        full.slots[OUT] = Some(Stack::new(id("copper ingot"), ITEMS.stack(id("copper ingot"))));
        run(&mut full, 30.0, &mut led);
        assert!(!full.burning() && led.is_empty());
        // ingots with no cast chosen, or no mould: they stay, and nothing burns
        let mut ingots = with([(FUEL, "oak log", 3), (IN, "copper ingot", 2)]);
        run(&mut ingots, 30.0, &mut led);
        ingots.slots[MOULD] = Some(Stack::new(id("clay mould"), 1));
        run(&mut ingots, 30.0, &mut led);
        assert!(!ingots.burning() && ingots.slots[IN] == Some(Stack::new(id("copper ingot"), 2)) && led.is_empty());
    }

    #[test]
    fn a_cast_needs_the_mould_and_the_choice_and_keeps_the_mould() {
        let (ingot, head, mould) = (id("copper ingot"), id("copper pick head"), id("clay mould"));
        let casts: Vec<(&str, u16, u16)> = RECIPES.shapes(crucible().top).iter().map(|w| (ITEMS.name(w.out.0), w.input.1, w.mould.unwrap())).collect();
        assert_eq!(casts, [("copper pick head", 2, mould), ("copper axe head", 2, mould), ("copper shovel head", 1, mould), ("copper knife blade", 1, mould), ("copper hammer head", 3, mould)]);
        let pick = RECIPES.works.iter().find(|w| w.out.0 == head).unwrap();
        let mut led = std::collections::BTreeMap::new();
        let mut f = with([(IN, "copper ingot", 5), (FUEL, "oak log", 1)]);
        f.choice = Some(pick.name.clone());
        run(&mut f, 30.0, &mut led);
        assert!(f.slots[OUT].is_none() && !f.burning(), "no mould: nothing");
        f.slots[MOULD] = Some(Stack::new(mould, 1));
        run(&mut f, 40.0, &mut led);
        assert_eq!((f.slots[OUT], f.slots[IN], f.slots[MOULD]), (Some(Stack::new(head, 2)), Some(Stack::new(ingot, 1)), Some(Stack::new(mould, 1))), "{f:?}");
        assert_eq!((led.get(&ingot), led.get(&head), led.get(&mould)), (Some(&-4), Some(&2), None));
        // the moves a furnace keeps: fuel only in the fuel slot, a mould only in the mould slot, nothing put into
        // the output; taking from the output is a move like any other
        let old = f.slots.clone();
        let mut s = old.clone();
        s[FUEL] = Some(Stack::new(ingot, 1));
        assert!(!fits(&s, &old, &RECIPES));
        let mut s = old.clone();
        s[MOULD] = Some(Stack::new(id("stick"), 1));
        assert!(!fits(&s, &old, &RECIPES));
        let mut s = old.clone();
        s[OUT] = Some(Stack::new(head, 3));
        assert!(!fits(&s, &old, &RECIPES));
        s[OUT] = Some(Stack::new(ingot, 1));
        assert!(!fits(&s, &old, &RECIPES));
        s[OUT] = Some(Stack::new(head, 1));
        assert!(fits(&s, &old, &RECIPES));
        s[OUT] = None;
        s[FUEL] = Some(Stack::new(id("stick"), 4));
        s[IN] = Some(Stack::new(id("flint"), 4));
        assert!(fits(&s, &old, &RECIPES));
    }
}
