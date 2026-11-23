//! Store (ii): gas as counted units in the column gaps, the liquid's own structure (`liquid.rs`: a node a gap,
//! links between overlapping gaps of neighbouring columns, each with a speed), made compressible.
//!
//! A node is one well-mixed gas from its floor to its ceiling; an open gap reaches up to the near box's top, where
//! it meets the coarse cell. A link's speed is sped up by the difference of the two nodes' pressures at the middle
//! of their shared window, each node's pressure leaned through its own height by its density against the coarse
//! air's (the same rule as store (i), with one cell a gap). Same units, same ledger, same coarse cell, same
//! slowed sound speed; only the shape of the store differs, so the comparison measures the shape.
use crate::gas::*;

pub struct GNode {
    pub i: i32,
    pub j: i32,
    pub floor: i32,
    /// Exclusive; an open gap's is the box's top.
    pub ceil: i32,
    pub open: bool,
    pub m: [i64; NS],
    pub e: i64,
    /// The speed through an open gap's top, m/s (+ up).
    pub vtop: f64,
}
impl GNode {
    pub fn vol(&self) -> f64 {
        (self.ceil - self.floor) as f64
    }
    fn zmid(&self) -> f64 {
        0.5 * (self.floor + self.ceil) as f64
    }
}

struct GLink {
    a: u32,
    /// BG: the coarse air beyond the box's side.
    b: u32,
    lo: i32,
    hi: i32,
    v: f64,
}

pub struct Gaps {
    pub nodes: Vec<GNode>,
    links: Vec<GLink>,
    pub coarse: Coarse,
    amb: Vec<Amb>,
    top: i32,
}

impl Gaps {
    /// Every gap of the box as a node (shortcut: all of them stored; the sparse version would store the off-rest
    /// columns only, as `liquid.rs` stores touched water, which changes the cost, not what the gas does).
    pub fn new(g: &Ground, coarse: Coarse) -> Gaps {
        let free = coarse.vol - ((g.hi[0] - g.lo[0]) * (g.hi[1] - g.lo[1]) * (g.hi[2] - g.lo[2])) as f64;
        let amb: Vec<Amb> = (0..g.hi[2] + 1).map(|k| coarse.amb(k as f64 + 0.5, free)).collect();
        let mut gaps = Gaps { nodes: Vec::new(), links: Vec::new(), coarse, amb, top: g.hi[2] };
        let mut by_col: FxMap<(i32, i32), Vec<u32>> = FxMap::default();
        for j in g.lo[1]..g.hi[1] {
            for i in g.lo[0]..g.hi[0] {
                let mut k = g.lo[2];
                while k < g.hi[2] {
                    if g.block([i, j, k]) != AIR_BLOCK {
                        k += 1;
                        continue;
                    }
                    let floor = k;
                    while k < g.hi[2] && g.block([i, j, k]) == AIR_BLOCK {
                        k += 1;
                    }
                    let mut n = GNode { i, j, floor, ceil: k, open: k == g.hi[2], m: [0; NS], e: 0, vtop: 0.0 };
                    for kk in floor..k {
                        let a = gaps.amb[kk as usize];
                        for s in 0..NS {
                            n.m[s] += a.m[s];
                            gaps.coarse.m[s] -= a.m[s];
                        }
                        n.e += a.e;
                        gaps.coarse.e -= a.e;
                    }
                    by_col.entry((i, j)).or_default().push(gaps.nodes.len() as u32);
                    gaps.nodes.push(n);
                }
            }
        }
        for a in 0..gaps.nodes.len() {
            let (i, j, f, c) = (gaps.nodes[a].i, gaps.nodes[a].j, gaps.nodes[a].floor, gaps.nodes[a].ceil);
            for (di, dj) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                let (ni, nj) = (i + di, j + dj);
                if let Some(others) = by_col.get(&(ni, nj)) {
                    if di < 0 || dj < 0 {
                        continue; // the other column made this link
                    }
                    for &b in others {
                        let (lo, hi) = (f.max(gaps.nodes[b as usize].floor), c.min(gaps.nodes[b as usize].ceil));
                        if hi > lo {
                            gaps.links.push(GLink { a: a as u32, b, lo, hi, v: 0.0 });
                        }
                    }
                } else {
                    // beyond the box: coarse air over `out_solid_below`
                    let lo = f.max(g.out_solid_below);
                    if c > lo {
                        gaps.links.push(GLink { a: a as u32, b: BG, lo, hi: c, v: 0.0 });
                    }
                }
            }
        }
        gaps
    }

    pub fn total(&self, s: usize) -> i64 {
        self.coarse.m[s] + self.nodes.iter().map(|n| n.m[s]).sum::<i64>()
    }
    pub fn energy(&self) -> i64 {
        self.coarse.e + self.nodes.iter().map(|n| n.e).sum::<i64>()
    }
    pub fn node_at(&self, p: [i32; 3]) -> Option<usize> {
        self.nodes.iter().position(|n| n.i == p[0] && n.j == p[1] && n.floor <= p[2] && p[2] < n.ceil)
    }
    fn amb_rho(&self, z: f64) -> f64 {
        self.amb[(z.floor() as i32).clamp(0, self.top) as usize].rho
    }
    /// The node's over-pressure at height z, Pa.
    fn press(&self, n: usize, rho: f64, pp: f64, z: f64) -> f64 {
        let nd = &self.nodes[n];
        pp - (rho - self.amb_rho(nd.zmid())) * G * (z - nd.zmid())
    }

    pub fn step(&mut self) {
        let n = self.nodes.len();
        let rho: Vec<f64> = self.nodes.iter().map(|d| mass(&d.m) / d.vol()).collect();
        let pp: Vec<f64> = (0..n)
            .map(|i| {
                let d = &self.nodes[i];
                let p_amb: f64 = (d.floor..d.ceil).map(|k| self.amb[k as usize].p).sum();
                rho[i] * C_ART * C_ART * (moles(&d.m) * R * temp(&d.m, d.e) / p_amb - 1.0)
            })
            .collect();
        // links, then open tops: (donor, receiver, volume share of the donor)
        let mut moves: Vec<(u32, u32, f64)> = Vec::new();
        for l in 0..self.links.len() {
            let (a, b, lo, hi) = (self.links[l].a, self.links[l].b, self.links[l].lo, self.links[l].hi);
            let zw = 0.5 * (lo + hi) as f64;
            let pa = self.press(a as usize, rho[a as usize], pp[a as usize], zw);
            let (pb, rb) = if b == BG { (0.0, self.amb_rho(zw)) } else { (self.press(b as usize, rho[b as usize], pp[b as usize], zw), rho[b as usize]) };
            let r = 0.5 * (rho[a as usize] + rb);
            let v = &mut self.links[l].v;
            *v += DT * (pa - pb) / r;
            *v -= *v * FRICTION;
            let vol = v.abs() * DT * (hi - lo) as f64;
            let (d, to) = if *v > 0.0 { (a, b) } else { (b, a) };
            let share = if d == BG { vol } else { vol / self.nodes[d as usize].vol() };
            moves.push((d, to, share));
        }
        for a in 0..n {
            if !self.nodes[a].open {
                continue;
            }
            let zt = self.nodes[a].ceil as f64;
            let r = 0.5 * (rho[a] + self.amb_rho(zt));
            let f = self.press(a, rho[a], pp[a], zt) - (r - self.amb_rho(zt)) * G * 0.5;
            let v = &mut self.nodes[a].vtop;
            *v += DT * f / r;
            *v -= *v * FRICTION;
            let vol = v.abs() * DT;
            moves.push(if *v > 0.0 { (a as u32, BG, vol / self.nodes[a].vol()) } else { (BG, a as u32, vol) });
        }
        let mut out = vec![0.0; n];
        for &(d, _, f) in &moves {
            if d != BG {
                out[d as usize] += f;
            }
        }
        let mut dm = vec![[0i64; NS]; n];
        let mut de = vec![0i64; n];
        let bg = self.amb[(self.top - 1) as usize];
        for (d, to, f) in moves {
            let (m, e, f) = if d == BG { (bg.m, bg.e, f) } else { (self.nodes[d as usize].m, self.nodes[d as usize].e, f * (0.5 / out[d as usize]).min(1.0)) };
            let mut mv = [0i64; NS];
            for s in 0..NS {
                mv[s] = (m[s] as f64 * f).floor() as i64;
            }
            let ev = (e as f64 * f).floor() as i64;
            for (side, sign) in [(d, -1), (to, 1)] {
                if side == BG {
                    for s in 0..NS {
                        self.coarse.m[s] += sign * mv[s];
                    }
                    self.coarse.e += sign * ev;
                } else {
                    for s in 0..NS {
                        dm[side as usize][s] += sign * mv[s];
                    }
                    de[side as usize] += sign * ev;
                }
            }
        }
        for i in 0..n {
            for s in 0..NS {
                self.nodes[i].m[s] += dm[i][s];
            }
            self.nodes[i].e += de[i];
        }
    }
}
