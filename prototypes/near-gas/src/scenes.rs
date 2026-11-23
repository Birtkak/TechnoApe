//! The places the tests and the measurements run in.
use crate::gas::*;
use crate::heat::*;

pub const T_AIR: f64 = 288.15;

impl Sim {
    /// Release `kg` of species `s` into the open block `p` at the coarse air's temperature (booked as a source).
    pub fn release(&mut self, p: [i32; 3], s: usize, kg: f64) {
        self.release_units(p, s, (kg * MU_PER_KG as f64) as i64);
    }
    pub fn release_units(&mut self, p: [i32; 3], s: usize, u: i64) {
        let i = self.gas.activate(&self.g, p).unwrap();
        self.gas.add(i, s, u);
        self.released += u;
        let e = (u as f64 / MU_PER_KG as f64 * SPECIES[s].cp * self.gas.coarse.temp() * 1000.0) as i64;
        self.gas.cells[i as usize].e += e;
        self.supplied += e;
    }
    pub fn run(&mut self, secs: f64) {
        let n = (secs / DT).round() as u64;
        for _ in 0..n {
            self.step();
            self.check();
        }
    }
    /// Units of species `s` over the coarse air's share in the fine cells of the box lo..hi.
    pub fn excess_in(&self, s: usize, lo: [i32; 3], hi: [i32; 3]) -> i64 {
        let mut x = 0;
        for c in &self.gas.cells {
            if (0..3).all(|a| c.p[a] >= lo[a] && c.p[a] < hi[a]) {
                x += c.m[s] - self.gas.amb_at(c.p[2]).m[s];
            }
        }
        x
    }
    pub fn ignited(&self, p: [i32; 3]) -> Option<f64> {
        self.block_at(p).and_then(|b| b.caught)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hole {
    None,
    Roof,
    Floor,
}
/// The room's inside: 6 x 6 x 3 on a slab 3 m over the ground.
pub const ROOM_LO: [i32; 3] = [4, 4, 4];
pub const ROOM_HI: [i32; 3] = [10, 10, 7];
pub const ROOM_FIRE: [i32; 3] = [6, 6, 4];

/// A stone room on a slab with a wood block burning on its floor, and one hole of a block in its roof or floor.
pub fn room(hole: Hole) -> Sim {
    let mut g = room_ground(hole);
    g.set(ROOM_FIRE, WOOD);
    let mut s = Sim::new(g, T_AIR);
    s.heat_to(ROOM_FIRE, 700.0);
    s.check();
    s
}

/// A smoke maker standing in for a fire (no oxygen taken): `kw` of heat and the products of wood burning at that
/// heat, in units a step: CO2, water vapour, soot.
pub fn smoke_maker(kw: f64) -> [(usize, i64); 3] {
    let fuel = kw * 1000.0 / 15e6 * DT * MU_PER_KG as f64;
    [(CO2, (fuel * 1.33) as i64), (H2O, (fuel * 0.8) as i64), (SOOT, (fuel * 0.015) as i64)]
}
pub const MAKER_KW: f64 = 100.0;

/// The room with a 100 kW smoke maker on its floor in place of the wood block: the stores compared on one source.
pub fn room_maker(hole: Hole) -> Sim {
    let mut s = Sim::new(room_ground(hole), T_AIR);
    s.heaters.push((ROOM_FIRE, MAKER_KW * 1000.0));
    for (sp, u) in smoke_maker(MAKER_KW) {
        s.vents.push((ROOM_FIRE, sp, u, u64::MAX));
    }
    s.check();
    s
}

pub fn room_ground(hole: Hole) -> Ground {
    let mut g = Ground::new([0, 0, 0], [14, 14, 16], 1);
    g.fill([0, 0, 0], [14, 14, 1], STONE);
    g.fill([3, 3, 3], [11, 11, 8], STONE);
    g.fill(ROOM_LO, ROOM_HI, AIR_BLOCK);
    match hole {
        Hole::None => {}
        Hole::Roof => g.set([7, 7, 7], AIR_BLOCK),
        Hole::Floor => g.set([7, 7, 3], AIR_BLOCK),
    }
    g
}

/// A closed valley open to the sky: a plateau, a slope of 1 m steps down to a floor, a pit 3 deep in the floor.
pub const PIT_LO: [i32; 3] = [20, 0, 0];
pub const PIT_HI: [i32; 3] = [24, 3, 3];
pub fn valley() -> Sim {
    let mut s = Sim::new(valley_ground(), T_AIR);
    // a vent on the plateau: 10 kg of CO2 at 0.1 kg/s
    s.vents.push((VENT, CO2, VENT_UNITS, VENT_STEPS));
    s.check();
    s
}
pub const VENT: [i32; 3] = [2, 1, 10];
pub const VENT_UNITS: i64 = (0.1 * DT * MU_PER_KG as f64) as i64;
pub const VENT_STEPS: u64 = (100.0 / DT) as u64;

pub fn valley_ground() -> Ground {
    let mut g = Ground::new([0, 0, 0], [32, 3, 16], 16);
    for i in 0..32 {
        let top = if i < 4 { 10 } else if i < 11 { 13 - i } else if (20..24).contains(&i) { 0 } else { 3 };
        g.fill([i, 0, 0], [i + 1, 3, top], STONE);
    }
    g
}

/// A pot of 41 units (10 kg) of water on the ground over a 3 kW fire.
pub const POT_AT: [i32; 3] = [2, 2, 1];
pub fn pot() -> Sim {
    let mut g = Ground::new([0, 0, 0], [5, 5, 6], 1);
    g.fill([0, 0, 0], [5, 5, 1], STONE);
    g.set(POT_AT, POT);
    let mut s = Sim::new(g, T_AIR);
    s.wet.insert(key(POT_AT), 41);
    let i = s.join(POT_AT).unwrap() as usize;
    s.blocks[i].watts = 3000.0;
    s.check();
    s
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Line {
    Dry,
    Gap,
    Wet,
    Wind,
}
/// A line of wood blocks on stone ground, open to the sky, along +i from i = 3 to 15.
pub const LINE: std::ops::Range<i32> = 3..16;
pub const GAP: i32 = 9;
pub fn line(kind: Line) -> Sim {
    let mut g = Ground::new([0, 0, 0], [20, 5, 10], 1);
    g.fill([0, 0, 0], [20, 5, 1], STONE);
    for i in LINE {
        g.set([i, 2, 1], if kind == Line::Gap && i == GAP { STONE } else { WOOD });
    }
    let mut s = Sim::new(g, T_AIR);
    if kind == Line::Wet {
        // 30 % moisture on the dry mass: 150 kg, 614 units a block; the first block, lit, is dry
        for i in LINE.start + 1..LINE.end {
            s.wet.insert(key([i, 2, 1]), 614);
        }
    }
    if kind == Line::Wind {
        s.gas.wind = [3.0, 0.0];
        s.heat_to([GAP, 2, 1], 700.0);
    } else {
        s.heat_to([LINE.start, 2, 1], 700.0);
    }
    s.check();
    s
}
