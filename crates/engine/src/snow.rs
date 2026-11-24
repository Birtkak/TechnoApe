//! Snow and ice in eighths of a block (ADR 0189; the draft "Snow and ice lie in eighths", sky-weather-and-water.md).
//!
//! - **Snow is counted water.** A column's snow is a number of water units (`Lay::units`) lying on its ground, shown in
//!   eighths of a block of snow at `DENSITY` (an eighth holds exactly `EIGHTH` units). Eight make a block of snow, set
//!   through the edits (`packed`, booked), and the layer goes on over it; melting down to it takes it back.
//! - **Only columns near a player are live** (`LIVE` columns round each one on the planet): elsewhere the snow is the
//!   formula, the live air grid's lying snow over its column (what the ground is drawn white by from afar). A column
//!   joining takes the formula's units from the air's account, one leaving gives back what it holds: no water is made
//!   or lost (`lent`, `owed`: settled with the air when its job lands, `Weather::tick_owing`).
//! - **It builds and melts by the world's heat** (ADR 0200: every flow at `HEAT_PACE`, stored heats over `LIGHT`): the
//!   air's snowfall at the column lays units while the air there is under freezing; air over it melts it at
//!   h (T - 0) over the latent heat, the water soaking in to the air column's soil (the rain's account); under the
//!   triple point's pressure it sublimates to `Liquids::vapour` as an ice block does.
//! - What is shown (`seen`) is that and the ice frozen on stored water short of a block and lava's held crust
//!   (`show`): the wire's deltas (`ToClient::Layers`), the player's sinking (`phys`) and the look-at panel.
use crate::blocks::BLOCKS;
use crate::liquid::phases::Phases;
use crate::liquid::{Col, Ground, OPEN, U};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Fresh snow's density, kg/m^3 (new snow 50 to 200, settled 200 to 300): an eighth of a block of it is 15.625 kg of
/// water on its square metre, exactly `EIGHTH` units.
pub const DENSITY: f64 = 125.0;
/// Water units an eighth of a block of snow holds: 1/8 m x 125 kg/m^3 = 15.625 mm of water = U / 64.
pub const EIGHTH: i64 = U / 64;
/// A block of snow, eight eighths: U / 8 units (a tenth and a quarter of its block in water).
pub const BLOCK: i64 = 8 * EIGHTH;
/// Columns live round each player, every way along its face: 33 x 33 a player.
/// shortcut: a face's edge ends the square (the columns over it stay the formula's until the player crosses).
pub const LIVE: i32 = 16;
/// Walking in snow one sinks into is slower by this share of the walk for each block of its depth (3 eighths: 19 %).
pub const DRAG: f64 = 0.5;
/// A column whose snow has no place (water lies there, no open sky, no air): it holds none.
pub const NOWHERE: i32 = i32::MIN;
/// Water units in a millimetre of water over a square metre.
pub const PER_MM: f64 = U as f64 / 1000.0;

/// One live column's snow.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lay {
    /// The cell it lies in: its column's open gap's floor (`NOWHERE`: none).
    pub floor: i32,
    pub units: i64,
    /// What fell, melted or sublimated short of a whole unit, units.
    pub part: f64,
    /// The air column whose account it trades with.
    pub cell: u32,
}

/// What a column shows in eighths: the cell, the top's height in it (0 to 1), its eighths and its block (snow, ice,
/// basalt). Snow's lies on the cell's floor; ice's top is its water's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    pub z: i32,
    pub top: f32,
    pub eighths: u8,
    pub block: u16,
}

/// The air at a live column: its account's air column, the formula's units a joining column takes (the air column's
/// lying snow over a square metre), and the snowfall there now, units a second (0 where it rains or is dry).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Here {
    pub cell: u32,
    pub formula: f64,
    pub fall: f64,
}

/// What the snow owes each air column (`Weather::tick_owing` settles it when a job lands): units its lying snow gives
/// (positive: the snow took them) and units its soil gets back (never positive: melt and digging soak in).
pub type Owed = BTreeMap<u32, (i64, i64)>;

/// The snow store's saved part: the live columns, the blocks it packed (with their air column), `lent`, `owed`.
pub type State = (Vec<(Col, Lay)>, Vec<((Col, i32), u32)>, i64, Vec<(u32, (i64, i64))>);

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Snow {
    /// The live columns (the server's).
    pub cols: BTreeMap<Col, Lay>,
    /// Blocks of snow eight eighths made, (column, cell): `BLOCK` units each, with the air column they owe.
    pub packed: BTreeMap<(Col, i32), u32>,
    /// Units taken from the air's account less those given back (the server's): its lying snow and packed blocks plus
    /// what of it sublimated to vapour. The water ledger counts `held - lent`, the air's `lent - owed`.
    pub lent: i64,
    pub owed: Owed,
    /// What every column shows (the server's and, from the wire, a client's).
    pub seen: BTreeMap<Col, Seen>,
    /// The ice and rock in eighths the liquids show (`show`).
    shown: BTreeMap<Col, Seen>,
    /// Columns an edit changed: their floor is found again.
    touched: BTreeSet<Col>,
    /// Columns whose `seen` changed since the last `changes`.
    dirty: BTreeSet<Col>,
}

/// The cell a column's snow lies in: its open gap's floor, None where generation puts water there.
fn open_floor(g: &impl Ground, c: Col) -> Option<i32> {
    let (lo, hi) = g.generated(c);
    let top = g.gaps(c).last().copied()?;
    (lo >= hi && top.1 == OPEN).then_some(top.0)
}

impl Snow {
    /// The water the snow holds: its live columns and its packed blocks.
    pub fn held(&self) -> i64 {
        self.cols.values().map(|l| l.units).sum::<i64>() + BLOCK * self.packed.len() as i64
    }
    /// Units owed the air in all.
    pub fn owing(&self) -> i64 {
        self.owed.values().map(|o| o.0 + o.1).sum()
    }
    fn take(&mut self, cell: u32, n: i64) {
        if n == 0 {
            return;
        }
        self.lent += n;
        self.owed.entry(cell).or_default().0 += n;
    }
    fn soak(&mut self, cell: u32, n: i64) {
        if n == 0 {
            return;
        }
        self.lent -= n;
        self.owed.entry(cell).or_default().1 -= n;
    }
    /// An edit changed column c: its floor is found again at the next step.
    pub fn touch(&mut self, c: Col) {
        if self.cols.contains_key(&c) {
            self.touched.insert(c);
        }
    }
    /// The block at cell z of column c was taken away by an edit (dug, or set over): a packed block of snow's units
    /// soak in (its block, if dug, is an item, outside the ledger as ice's is).
    pub fn unpacked(&mut self, c: Col, z: i32) {
        if let Some(cell) = self.packed.remove(&(c, z)) {
            self.soak(cell, BLOCK);
        }
    }
    /// A dig at cell z of column c: the snow lying on it goes (its water soaks in), and true, if there is an eighth or
    /// more; the block is left. shortcut: a layer dug gives no item (there is no snowball; a block of snow dug does).
    pub fn dig(&mut self, c: Col, z: i32) -> bool {
        let Some(l) = self.cols.get_mut(&c) else { return false };
        if l.floor != z + 1 || l.units < EIGHTH {
            return false;
        }
        let (cell, n) = (l.cell, l.units);
        (l.units, l.part) = (0, 0.0);
        self.soak(cell, n);
        self.reshow(c);
        true
    }
    /// How deep the snow one sinks into in cell z of column c is, blocks (0: none, or a full block one stands on).
    pub fn sunk(&self, c: Col, z: i32) -> f64 {
        match self.seen.get(&c) {
            Some(s) if s.z == z && s.block == BLOCKS.id("snow") => s.eighths as f64 / 8.0,
            _ => 0.0,
        }
    }
    /// The ice frozen on stored water short of a block and the rock frozen short of one (from the liquids each second),
    /// shown on their columns where no snow is: every column's, sorted.
    pub fn show(&mut self, v: Vec<(Col, Seen)>) {
        let new: BTreeMap<Col, Seen> = v.into_iter().collect();
        let gone: Vec<Col> = self.shown.keys().filter(|c| !new.contains_key(c)).copied().collect();
        let changed: Vec<Col> = new.iter().filter(|(c, s)| self.shown.get(c) != Some(s)).map(|(c, _)| *c).collect();
        self.shown = new;
        for c in gone.into_iter().chain(changed) {
            self.reshow(c);
        }
    }
    fn reshow(&mut self, c: Col) {
        let snow = BLOCKS.id("snow");
        let s = self
            .cols
            .get(&c)
            .filter(|l| l.floor != NOWHERE && l.units >= EIGHTH)
            .map(|l| {
                let e = (l.units / EIGHTH) as u8;
                Seen { z: l.floor, top: e as f32 / 8.0, eighths: e, block: snow }
            })
            .or_else(|| self.shown.get(&c).copied());
        if self.seen.get(&c).copied() != s {
            match s {
                Some(s) => self.seen.insert(c, s),
                None => self.seen.remove(&c),
            };
            self.dirty.insert(c);
        }
    }
    /// The columns whose `seen` changed since the last call, for the wire (None: it shows nothing now), sorted. A
    /// snowfall sends a column only when it passes an eighth.
    pub fn changes(&mut self) -> Vec<(Col, Option<Seen>)> {
        std::mem::take(&mut self.dirty).into_iter().map(|c| (c, self.seen.get(&c).copied())).collect()
    }
    /// A client's copy: what the server sent.
    pub fn apply(&mut self, v: &[(Col, Option<Seen>)]) {
        for (c, s) in v {
            match s {
                Some(s) => self.seen.insert(*c, *s),
                None => self.seen.remove(c),
            };
        }
    }

    /// `dt` seconds of snow on the live columns `live` (every other one leaves, its units back to the air; a new one
    /// joins with the formula's units, `here`). `here` None: no snow can lie there (water does; a column with no air);
    /// what it held soaks in. Each live column takes the snowfall `here` gives, and loses by the heat at its place
    /// (`Ground::air`): over its freeze point h (T - 0) / Lf units a second (melt, soaking in), under the triple point's
    /// pressure its sublimation (to `vapour`). Eight eighths become a block of snow, set; one melting down takes back
    /// the packed block under it. Returns the cells to set, (column, cell, block).
    pub fn step(&mut self, g: &impl Ground, dt: f64, live: &BTreeSet<Col>, here: &dyn Fn(Col, i32) -> Option<Here>, ph: &Phases, vapour: &mut i64) -> Vec<(Col, i32, u16)> {
        let snow = BLOCKS.id("snow");
        let mut edits = Vec::new();
        let gone: Vec<Col> = self.cols.keys().filter(|c| !live.contains(c)).copied().collect();
        for c in gone {
            let l = self.cols.remove(&c).expect("listed");
            self.take(l.cell, -l.units);
            self.reshow(c);
        }
        for &c in live {
            if self.cols.contains_key(&c) {
                continue;
            }
            // shortcut: a column joins with the formula's snow short of a block; what the air holds over that stays its
            let lay = match open_floor(g, c).and_then(|f| here(c, f).map(|h| (f, h))) {
                Some((floor, h)) => {
                    let units = (h.formula.max(0.0) as i64).min(BLOCK - 1);
                    self.take(h.cell, units);
                    Lay { floor, units, part: 0.0, cell: h.cell }
                }
                None => Lay { floor: NOWHERE, units: 0, part: 0.0, cell: u32::MAX },
            };
            self.cols.insert(c, lay);
            self.reshow(c);
        }
        for c in std::mem::take(&mut self.touched) {
            let Some(mut l) = self.cols.get(&c).copied() else { continue };
            if l.floor == NOWHERE {
                // (it holds nothing: it joins again at the next step, from the formula)
                self.cols.remove(&c);
                continue;
            }
            match open_floor(g, c) {
                // dug under it: it falls
                Some(f) if f < l.floor => l.floor = f,
                Some(f) if f == l.floor => {}
                // a block set in its cell, or water there: what it held soaks in
                f => {
                    self.soak(l.cell, l.units);
                    (l.units, l.part, l.floor) = (0, 0.0, f.unwrap_or(NOWHERE));
                }
            }
            self.cols.insert(c, l);
            self.reshow(c);
        }
        let cols: Vec<Col> = self.cols.keys().copied().collect();
        for c in cols {
            let mut l = self.cols[&c];
            if l.floor == NOWHERE {
                continue;
            }
            let Some(h) = here(c, l.floor) else {
                self.soak(l.cell, l.units);
                (l.units, l.part, l.floor) = (0, 0.0, NOWHERE);
                self.cols.insert(c, l);
                self.reshow(c);
                continue;
            };
            let (ta, p) = g.air(c, l.floor as f64);
            let subl = p < ph.pt;
            let lose = if subl {
                ph.sublimation(ph.h(ta, p), 1.0, ta, p, 0.0)
            } else if ta > ph.tf {
                ph.h(ta, p) * (ta - ph.tf) / ph.lf
            } else {
                0.0
            };
            l.part += (h.fall.max(0.0) - lose) * dt;
            let n = l.part.trunc() as i64;
            l.part -= n as f64;
            if n > 0 {
                self.take(l.cell, n);
                l.units += n;
            } else if n < 0 {
                // melting down to a block it packed takes the block back
                while -n > l.units {
                    let Some(cell) = self.packed.remove(&(c, l.floor - 1)) else { break };
                    l.floor -= 1;
                    l.units += BLOCK;
                    edits.push((c, l.floor, crate::blocks::AIR));
                    // (its units came back from `packed` to the layer: still lent to that air column)
                    if cell != l.cell {
                        self.take(cell, -BLOCK);
                        self.take(l.cell, BLOCK);
                    }
                }
                let m = (-n).min(l.units);
                l.units -= m;
                if subl {
                    // (still lent: the vapour is the liquids' to count, not given back to the air)
                    *vapour += m;
                } else {
                    self.soak(l.cell, m);
                }
            }
            if l.units == 0 && !self.packed.contains_key(&(c, l.floor - 1)) {
                // nothing lies there to lose
                l.part = l.part.max(0.0);
            }
            while l.units >= BLOCK {
                self.packed.insert((c, l.floor), l.cell);
                edits.push((c, l.floor, snow));
                l.floor += 1;
                l.units -= BLOCK;
            }
            self.cols.insert(c, l);
            self.reshow(c);
        }
        edits
    }

    /// The saved part (`State`); a world without snow saves nothing.
    pub fn state(&self) -> State {
        (self.cols.iter().map(|(c, l)| (*c, *l)).collect(), self.packed.iter().map(|(k, c)| (*k, *c)).collect(), self.lent, self.owed.iter().map(|(k, o)| (*k, *o)).collect())
    }
    pub fn restore(&mut self, s: &State) {
        self.cols = s.0.iter().copied().collect();
        self.packed = s.1.iter().copied().collect();
        self.lent = s.2;
        self.owed = s.3.iter().copied().collect();
        let cols: Vec<Col> = self.cols.keys().copied().collect();
        for c in cols {
            self.reshow(c);
        }
    }
}

/// The columns live round `players` (positions in the world) on body 0, the planet with weather (`LIVE`).
pub fn near(world: &crate::world::World, players: &[glam::DVec3]) -> BTreeSet<Col> {
    let mut out = BTreeSet::new();
    let n = world.bodies[0].n;
    for &p in players {
        let b = world.block_at(p);
        if b.body != 0 {
            continue;
        }
        for i in (b.pos.x - LIVE).max(0)..=(b.pos.x + LIVE).min(n - 1) {
            for j in (b.pos.y - LIVE).max(0)..=(b.pos.y + LIVE).min(n - 1) {
                out.insert(Col::new(0, b.face, i, j));
            }
        }
    }
    out
}

impl crate::world::World {
    /// `dt` seconds of the snow on the live columns `live` (`Snow::step`), the air's temperature at a place given by
    /// `temp` as `cool_fluids_in` takes it, the snow's air by `here`; every cell eight eighths packed (or a melt took
    /// back) is set through the edits, as (cell, what was there, the block). Then the ice and rock the liquids hold short
    /// of a block are shown.
    pub fn step_snow(&mut self, dt: f64, live: &BTreeSet<Col>, temp: Option<&dyn Fn(Col, f64) -> Option<f64>>, here: &dyn Fn(Col, i32) -> Option<Here>) -> Vec<(crate::world::Block, u16, u16)> {
        let ph = Phases::water();
        let water = crate::fluids::WATER;
        let cells = {
            let crate::world::World { bodies, edits, water: liquids, sky, time, snow, .. } = self;
            let mut g = crate::world::Solid::at(bodies, edits, sky, *time);
            g.air = Some(crate::world::Air { sky, t: *time, temp });
            let mut vap = 0;
            let cells = snow.step(&g, dt, live, here, &ph, &mut vap);
            if vap != 0 {
                *liquids.vapour.entry(water).or_default() += vap;
            }
            cells
        };
        let mut out = Vec::new();
        for (c, z, m) in cells {
            let b = crate::world::Block { body: c.body(), face: c.face(), pos: glam::IVec3::new(c.i(), c.j(), z) };
            let was = self.block(b);
            self.set_block(b, m);
            self.regap(b);
            out.push((b, was, m));
        }
        let shown = self.shown_layers();
        self.snow.show(shown);
        out
    }

    /// What the liquids hold frozen short of a block, in eighths: ice on stored water (its top the water's) and each
    /// fluid's held crust (`Liquids::crust`, its row's `freeze` block on the floor it froze at).
    pub fn shown_layers(&self) -> Vec<(Col, Seen)> {
        let rows = crate::fluids::FLUIDS.now();
        let ice = rows.get(crate::fluids::WATER).freeze.as_ref().map(|f| BLOCKS.id(&f.into));
        let mut v: Vec<(Col, Seen)> = Vec::new();
        if let Some(ice) = ice {
            v.extend(self.water.ice_layers().into_iter().map(|(c, top, e)| {
                let z = (top - 1).div_euclid(U) as i32;
                (c, Seen { z, top: (top - z as i64 * U) as f32 / U as f32, eighths: e, block: ice })
            }));
        }
        for (&(c, kind), &(floor, units)) in &self.water.crust {
            let e = (units * 8 / U).clamp(0, 7) as u8;
            if let (true, Some(f)) = (e > 0, rows.get(kind).freeze.as_ref()) {
                v.push((c, Seen { z: floor, top: e as f32 / 8.0, eighths: e, block: BLOCKS.id(&f.into) }));
            }
        }
        v.sort_by_key(|x| x.0);
        v.dedup_by_key(|x| x.0);
        v
    }
}

#[cfg(test)]
#[path = "snow_tests.rs"]
mod tests;
