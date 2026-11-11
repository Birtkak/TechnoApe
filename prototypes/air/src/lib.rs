//! The planet's coarse air grid (ADR 0010, `docs/design/air.md`): columns on the cube-sphere, four layers
//! each, one step = sun and ground, wind, carry, lift, water and land. Throwaway spike; the bake prototype
//! runs it to a steady day for its climate (worldgen stage 5).
//!
//! - **Water is integer** (units of 1/4096 of a block, ADR 0020) and only ever moved, so the ledger
//!   (vapour + cloud + soil + snow + `sea`) is exactly constant. Pollutant (`dirt`) the same.
//! - **Heat and wind are f32** in a fixed order: deterministic on one machine (only the server runs it),
//!   not bit-equal across machines. Heat is not a conserved ledger.
//! - **Rayon only maps cells to values;** every cell reads the old state, so the result does not depend on
//!   the thread count.
#[path = "../../world-bake/src/grid.rs"]
pub mod cube;

use glam::DVec3;
use rayon::prelude::*;
use std::f64::consts::TAU;

pub const LAYERS: usize = 4;
/// grams of water in one unit (1/4096 of a 1 m block)
const G_PER_UNIT: f64 = 1.0e6 / 4096.0;
pub const DAY: f64 = 1200.0;
/// the sky's spin at time 0 (`engine::sky::SPIN0`): the sun's place in the body frame follows from it
const SPIN0: f64 = -0.375 * TAU;

/// Every constant of the model (air.md: tuning, later `data/air.toml`). Rates are per second.
#[derive(Clone, Debug)]
pub struct Params {
    /// top of each layer above the ground (surface) or above the sea (the others), metres
    pub tops: [f32; LAYERS],
    /// sunlight at the subsolar point, W/m^2
    pub s0: f32,
    /// outgoing heat A + B T (Budyko), W/m^2 and W/m^2/K
    pub olr_a: f32,
    pub olr_b: f32,
    pub alb_sea: f32,
    pub alb_land: f32,
    pub alb_snow: f32,
    /// share of sunlight full cloud cover takes, and share of outgoing heat it keeps in
    pub cloud_shade: f32,
    pub cloud_trap: f32,
    /// cloud per m^2 of column (units) that makes half cover
    pub cover_half: f32,
    /// ground heat capacity, J/m^2/K
    pub cap_land: f32,
    pub cap_sea: f32,
    /// ground to surface air, W/m^2/K
    pub exchange: f32,
    /// air's heat capacity per metre of layer, J/m^3/K
    pub cap_air: f32,
    /// K per metre the air cools with height near the ground (compressed: a 120 m peak is 12 K colder),
    /// and above 150 m
    pub lapse: f32,
    pub lapse_up: f32,
    /// each upper layer cools by radiation toward the column's radiative balance plus this per layer, K
    /// (a fixed reference: relaxing toward the layer below instead ratchets the planet warmer through
    /// the sinking air)
    pub stable: f32,
    pub relax_up: f32,
    /// mixing rate between layers when the lower one is warmer than the upper (convection)
    pub convect: f32,
    /// wind bands, m/s: east-west amplitude and north-south amplitude per layer. The north-south ones
    /// must balance over the column (sum of thickness times v near 0), or air piles up aloft and its
    /// vapour rains out where the air should sink.
    pub band_u: [f32; LAYERS],
    pub band_v: [f32; LAYERS],
    /// m/s of breeze per K of difference toward the warmer column (surface; the low layer returns half)
    pub thermal: f32,
    /// a height step of this many metres halves the surface wind across it
    pub block_m: f32,
    /// the wind's approach to its target
    pub wind_relax: f32,
    /// vapour: saturation g/m^3 = sat0 * exp(sat_k * T)
    pub sat0: f32,
    pub sat_k: f32,
    /// evaporation toward saturation, and its gain per m/s of wind
    pub evap: f32,
    pub evap_wind: f32,
    /// cloud over this, g/m^3, falls at `rain` of the excess per second
    pub cloud_max: f32,
    pub rain: f32,
    /// soil holds this much water, metres; it seeps to the rivers at `seep`
    pub soil_m: f32,
    pub seep: f32,
    /// K a layer warms per g/m^3 of vapour that condenses (and cools per g/m^3 of cloud that evaporates)
    pub latent: f32,
    /// snow melts at this share per K per second
    pub melt: f32,
    /// rain washes this share of a layer's dirt down per second while it rains
    pub washout: f32,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            tops: [30.0, 150.0, 500.0, 1500.0],
            s0: 1361.0,
            olr_a: 190.0,
            olr_b: 4.0,
            alb_sea: 0.08,
            alb_land: 0.25,
            alb_snow: 0.65,
            cloud_shade: 0.5,
            cloud_trap: 0.25,
            cover_half: 120.0,
            cap_land: 6.0e3,
            cap_sea: 6.0e4,
            exchange: 25.0,
            cap_air: 400.0,
            lapse: 0.1,
            lapse_up: 0.015,
            stable: 1.5,
            relax_up: 1.0 / 2400.0,
            convect: 1.0 / 300.0,
            band_u: [3.0, 4.0, 6.0, 8.0],
            band_v: [1.5, 0.6, -0.09, -0.09],
            thermal: 0.25,
            block_m: 12.0,
            wind_relax: 1.0 / 120.0,
            sat0: 5.0,
            sat_k: 0.0625,
            evap: 1.0 / 900.0,
            evap_wind: 0.3,
            cloud_max: 0.1,
            rain: 1.0 / 40.0,
            soil_m: 0.2,
            seep: 1.0 / 3600.0,
            latent: 2.0,
            melt: 1.0 / 20000.0,
            washout: 1.0 / 300.0,
        }
    }
}

struct Edge {
    a: u32,
    b: u32,
    /// the shared boundary's normal from a to b
    normal: DVec3,
    /// the shared face's width and the distance between the centres, metres
    width: f32,
    /// the band wind along a to b per layer, m/s, and how much of it the ground lets through
    band: [f32; LAYERS],
    gate: [f32; LAYERS],
}

pub struct Air {
    pub p: Params,
    pub m: usize,
    pub n: usize,
    pub r: f64,
    pub dir: Vec<DVec3>,
    pub lat: Vec<f32>,
    pub area: Vec<f32>,
    /// mean land height, metres (0 over water), and the share of open water
    pub ground: Vec<f32>,
    pub water: Vec<f32>,
    edges: Vec<Edge>,
    /// cell to its edges (CSR)
    adj_at: Vec<u32>,
    adj: Vec<u32>,
    /// layer thickness and volume per cell, column-major (c * 4 + k)
    thick: Vec<f32>,
    vol: Vec<f32>,
    /// K the air of each layer is colder than its potential temperature (the lapse to its middle)
    cool: Vec<f32>,
    /// potential temperature (sea level), C, per cell
    pub theta: Vec<f32>,
    pub vap: Vec<u32>,
    pub cloud: Vec<u32>,
    pub dirt: Vec<u32>,
    /// wind along each edge a to b per layer, m/s (e * 4 + k)
    pub wind: Vec<f32>,
    /// ground potential temperature, C
    pub tg: Vec<f32>,
    /// the column's radiative balance by its latitude, C: what the upper layers cool toward
    theta_ref: Vec<f32>,
    pub soil: Vec<u32>,
    pub snow: Vec<u32>,
    /// dirt that came down
    pub dirt_down: Vec<u32>,
    /// the water outside the grid: sea, lakes and rivers (rain on water and runoff go here)
    pub sea: i64,
    pub t: f64,
    /// this step's rain and snow per column, and the runoff to the rivers
    pub fell: Vec<u32>,
    pub runoff: Vec<u32>,
    /// volume each cell-layer gained by convergence in this step's carry, m^3
    conv: Vec<f32>,
}

fn sat_g(p: &Params, t: f32) -> f32 {
    p.sat0 * (p.sat_k * t.clamp(-60.0, 50.0)).exp()
}

impl Air {
    /// `m` columns per face edge on a body of radius `r`; `ground` and `water` per column.
    pub fn new(m: usize, r: f64, ground: Vec<f32>, water: Vec<f32>, p: Params) -> Air {
        let g = cube::Grid::new(m, r);
        let n = g.n();
        assert_eq!(ground.len(), n);
        let dir: Vec<DVec3> = (0..n as u32).map(|c| g.d(c)).collect();
        let lat: Vec<f32> = dir.iter().map(|d| d.z.clamp(-1.0, 1.0).asin() as f32).collect();
        let area: Vec<f32> = (0..n as u32).map(|c| g.area(c)).collect();
        // the four neighbours, past a seam the cell the extended face's point falls in; each edge keeps its
        // shared boundary (two cell corners on the sphere), so the faces of a column close exactly
        let mut pairs: Vec<(u32, u32, [i64; 4], usize)> = Vec::new();
        for c in 0..n as u32 {
            let (f, i, j) = g.fij(c);
            let (i, j) = (i as i64, j as i64);
            for (di, dj, seg) in [(1i64, 0i64, [i + 1, j, i + 1, j + 1]), (-1, 0, [i, j, i, j + 1]), (0, 1, [i, j + 1, i + 1, j + 1]), (0, -1, [i, j, i + 1, j])] {
                let (ii, jj) = (i + di, j + dj);
                let o = if (0..m as i64).contains(&ii) && (0..m as i64).contains(&jj) {
                    g.idx(f, ii as usize, jj as usize)
                } else {
                    g.locate(g.dir_at(f, ii as f64 + 0.5, jj as f64 + 0.5))
                };
                if o != c && c < o {
                    pairs.push((c, o, seg, f));
                } else if o != c {
                    pairs.push((o, c, [i64::MIN; 4], f));
                }
            }
        }
        // keep each edge once, with the boundary as seen from its first cell
        pairs.sort_by_key(|p| (p.0, p.1, p.2[0] == i64::MIN));
        pairs.dedup_by_key(|p| (p.0, p.1));
        assert!(pairs.iter().all(|p| p.2[0] != i64::MIN), "every edge seen from both sides");
        // per cell: layer thickness, volume, and the lapse to the layer's middle
        let mut thick = vec![0f32; n * LAYERS];
        let mut cool = vec![0f32; n * LAYERS];
        for c in 0..n {
            let gz = ground[c].max(0.0);
            let mut bottom = gz;
            for k in 0..LAYERS {
                let top = if k == 0 { gz + p.tops[0] } else { p.tops[k].max(bottom + 30.0) };
                thick[c * LAYERS + k] = top - bottom;
                let mid = 0.5 * (top + bottom);
                cool[c * LAYERS + k] = p.lapse * mid.min(150.0) + p.lapse_up * (mid - 150.0).max(0.0);
                bottom = top;
            }
        }
        let vol: Vec<f32> = (0..n * LAYERS).map(|i| thick[i] * area[i / LAYERS]).collect();
        let east_north = |d: DVec3| {
            let e = DVec3::Z.cross(d).try_normalize().unwrap_or(DVec3::X);
            (e, d.cross(e))
        };
        let edges: Vec<Edge> = pairs
            .iter()
            .map(|&(a, b, seg, f)| {
                let (p0, p1) = (g.dir_at(f, seg[0] as f64, seg[1] as f64), g.dir_at(f, seg[2] as f64, seg[3] as f64));
                let mid = (p0 + p1).normalize();
                // the boundary's normal, pointing from a to b
                let mut along = p0.cross(p1).normalize();
                if along.dot(dir[b as usize] - dir[a as usize]) < 0.0 {
                    along = -along;
                }
                let (e, nn) = east_north(mid);
                let phi = mid.z.asin();
                let width = ((p1 - p0).length() * r) as f32;
                let mut band = [0f32; LAYERS];
                let mut gate = [1f32; LAYERS];
                for k in 0..LAYERS {
                    // trades, westerlies, polar easterlies; toward the equator in the tropics, poleward in
                    // the middle latitudes, back aloft
                    let u = -p.band_u[k] as f64 * (3.0 * phi).cos();
                    let v = -p.band_v[k] as f64 * (6.0 * phi).sin();
                    band[k] = (e * u + nn * v).dot(along) as f32;
                }
                let (ga, gb) = (ground[a as usize].max(0.0), ground[b as usize].max(0.0));
                gate[0] = 1.0 / (1.0 + (ga - gb).abs() / p.block_m);
                let (ta, tb) = (thick[a as usize * LAYERS + 1], thick[b as usize * LAYERS + 1]);
                gate[1] = (ta.min(tb) / (p.tops[1] - p.tops[0])).clamp(0.2, 1.0);
                Edge { a, b, normal: along, width, band, gate }
            })
            .collect();
        let mut deg = vec![0u32; n + 1];
        for e in &edges {
            deg[e.a as usize + 1] += 1;
            deg[e.b as usize + 1] += 1;
        }
        for c in 0..n {
            deg[c + 1] += deg[c];
        }
        let mut fill = deg.clone();
        let mut adj = vec![0u32; edges.len() * 2];
        for (k, e) in edges.iter().enumerate() {
            for c in [e.a, e.b] {
                adj[fill[c as usize] as usize] = k as u32;
                fill[c as usize] += 1;
            }
        }
        // a first guess: the ground at its daily-mean balance, softened toward the mean for the carry of heat
        let tg: Vec<f32> = (0..n)
            .map(|c| {
                let s = p.s0 as f64 / std::f64::consts::PI * (lat[c] as f64).cos();
                let alb = water[c] * p.alb_sea + (1.0 - water[c]) * p.alb_land;
                let eq = (s as f32 * (1.0 - alb) * 0.8 - p.olr_a) / p.olr_b;
                0.6 * eq + 0.4 * 10.0
            })
            .collect();
        let mut theta = vec![0f32; n * LAYERS];
        let mut vap = vec![0u32; n * LAYERS];
        for c in 0..n {
            for k in 0..LAYERS {
                let i = c * LAYERS + k;
                theta[i] = tg[c] + p.stable * k as f32;
                vap[i] = (0.6 * sat_g(&p, theta[i] - cool[i]) as f64 * vol[i] as f64 / G_PER_UNIT) as u32;
            }
        }
        let soil: Vec<u32> = (0..n).map(|c| (0.5 * p.soil_m as f64 * area[c] as f64 * 4096.0 * (1.0 - water[c]) as f64) as u32).collect();
        let ne = edges.len();
        Air {
            p,
            m,
            n,
            r,
            dir,
            lat,
            area,
            ground,
            water,
            edges,
            adj_at: deg,
            adj,
            thick,
            vol,
            cool,
            theta,
            vap,
            cloud: vec![0; n * LAYERS],
            dirt: vec![0; n * LAYERS],
            wind: vec![0.0; ne * LAYERS],
            theta_ref: tg.clone(),
            tg,
            soil,
            snow: vec![0; n],
            dirt_down: vec![0; n],
            sea: 1_000_000_000_000_000,
            t: 0.0,
            fell: vec![0; n],
            runoff: vec![0; n],
            conv: vec![0.0; n * LAYERS],
        }
    }

    /// Zonal means per 10 degrees of latitude and layer: relative humidity, cloud g/m^3, convergence
    /// (share of the layer per step), potential temperature (debugging the circulation).
    pub fn debug_zonal(&self) -> Vec<String> {
        let mut out = Vec::new();
        for b in 0..9 {
            let lo = b as f32 * 10.0;
            let cs: Vec<usize> = (0..self.n).filter(|&c| (lo..lo + 10.0).contains(&self.lat[c].to_degrees().abs())).collect();
            let mut row = format!("{lo:>3}:");
            for k in 0..LAYERS {
                let (mut rh, mut cl, mut cv, mut th) = (0f64, 0f64, 0f64, 0f64);
                for &c in &cs {
                    let i = c * LAYERS + k;
                    let qs = sat_g(&self.p, self.theta[i] - self.cool[i]) as f64 * self.vol[i] as f64 / G_PER_UNIT;
                    rh += self.vap[i] as f64 / qs;
                    cl += self.cloud[i] as f64 * G_PER_UNIT / self.vol[i] as f64;
                    cv += self.conv[i] as f64 / self.vol[i] as f64;
                    th += self.theta[i] as f64;
                }
                let k = cs.len() as f64;
                row += &format!(" | rh {:.2} cl {:.3} cv {:+.4} th {:.1}", rh / k, cl / k, cv / k, th / k);
            }
            out.push(row);
        }
        out
    }

    pub fn edges(&self) -> usize {
        self.edges.len()
    }

    /// The sun's direction in the body frame at time t: no tilt (ADR 0022), so the subsolar point runs
    /// round the equator once a day.
    pub fn sun(t: f64) -> DVec3 {
        let a = -SPIN0 - TAU * t / DAY;
        DVec3::new(a.cos(), a.sin(), 0.0)
    }

    /// The surface air's temperature at the column's mean ground, C.
    pub fn surface_t(&self, c: usize) -> f32 {
        self.theta[c * LAYERS] - self.cool[c * LAYERS]
    }

    /// The surface wind at a column, east and north, m/s (the mean over its edges, for pictures).
    pub fn surface_wind(&self, c: usize) -> (f32, f32) {
        let d = self.dir[c];
        let e = DVec3::Z.cross(d).try_normalize().unwrap_or(DVec3::X);
        let nn = d.cross(e);
        let mut v = DVec3::ZERO;
        let (lo, hi) = (self.adj_at[c] as usize, self.adj_at[c + 1] as usize);
        for &k in &self.adj[lo..hi] {
            v += self.edges[k as usize].normal * self.wind[k as usize * LAYERS] as f64;
        }
        v *= 2.0 / (hi - lo).max(1) as f64;
        (v.dot(e) as f32, v.dot(nn) as f32)
    }

    /// Every unit of water the planet holds: it never changes by itself.
    pub fn ledger(&self) -> i128 {
        let s = |v: &Vec<u32>| v.iter().map(|&x| x as i128).sum::<i128>();
        s(&self.vap) + s(&self.cloud) + s(&self.soil) + s(&self.snow) + self.sea as i128
    }

    pub fn dirt_total(&self) -> u128 {
        self.dirt.iter().chain(&self.dirt_down).map(|&x| x as u128).sum()
    }

    /// One step of `dt` seconds (air.md: 1 s live, the bake's longer).
    pub fn step(&mut self, dt: f32) {
        self.sun_ground(dt);
        self.winds(dt);
        self.carry(dt);
        self.lift(dt);
        self.rain_and_land(dt);
        self.t += dt as f64;
    }

    /// Passes 1 and 2: sunlight by the sun's height less cloud and reflection; the ground's balance; the
    /// ground warms the surface air; the upper layers relax toward a stable column.
    fn sun_ground(&mut self, dt: f32) {
        let sun = Air::sun(self.t);
        let p = &self.p;
        let (dir, water, area, snow, cloud, thick, re) = (&self.dir, &self.water, &self.area, &self.snow, &self.cloud, &self.thick, &self.theta_ref);
        self.tg.par_iter_mut().zip(self.theta.par_chunks_mut(LAYERS)).enumerate().for_each(|(c, (tg, th))| {
            let ctot: u32 = cloud[c * LAYERS..c * LAYERS + LAYERS].iter().sum();
            let per_m2 = ctot as f32 / area[c];
            let cover = per_m2 / (per_m2 + p.cover_half);
            let snowy = snow[c] as f32 / area[c] > 20.0;
            let w = water[c];
            let alb = w * p.alb_sea + (1.0 - w) * if snowy { p.alb_snow } else { p.alb_land };
            let cos = dir[c].dot(sun).max(0.0) as f32;
            let s_abs = p.s0 * cos * (1.0 - alb) * (1.0 - p.cloud_shade * cover);
            let olr = (p.olr_a + p.olr_b * *tg) * (1.0 - p.cloud_trap * cover);
            let h = p.exchange * (*tg - th[0]);
            let cap = w * p.cap_sea + (1.0 - w) * p.cap_land;
            *tg += dt * (s_abs - olr - h) / cap;
            th[0] += dt * h / (p.cap_air * thick[c * LAYERS]);
            for k in 1..LAYERS {
                th[k] += (re[c] + p.stable * k as f32 - th[k]) * (dt * p.relax_up);
            }
        });
    }

    /// Pass 3: the wind relaxes toward the bands plus a breeze toward warmer columns, gated by the ground.
    fn winds(&mut self, dt: f32) {
        let p = &self.p;
        let (edges, theta) = (&self.edges, &self.theta);
        let r = (dt * p.wind_relax).min(1.0);
        self.wind.par_chunks_mut(LAYERS).zip(edges.par_iter()).for_each(|(w, e)| {
            let d = theta[e.b as usize * LAYERS] - theta[e.a as usize * LAYERS];
            for k in 0..LAYERS {
                let breeze = match k {
                    0 => p.thermal * d,
                    1 => -0.5 * p.thermal * d,
                    _ => 0.0,
                };
                let target = ((e.band[k] + breeze) * e.gate[k]).clamp(-12.0, 12.0);
                w[k] += (target - w[k]) * r;
            }
        });
    }

    /// Pass 4: heat, vapour, cloud and dirt move with the wind between neighbours, upwind, in whole units.
    /// A cell gives `frac` of what it holds through an edge; the same number is computed from both sides.
    fn carry(&mut self, dt: f32) {
        let ne = self.edges.len();
        let (edges, wind, vol, thick) = (&self.edges, &self.wind, &self.vol, &self.thick);
        // the share of the donor's volume each edge moves, per layer
        let frac: Vec<f32> = (0..ne * LAYERS)
            .into_par_iter()
            .map(|i| {
                let (e, k) = (&edges[i / LAYERS], i % LAYERS);
                let u = wind[i];
                let d = if u >= 0.0 { e.a } else { e.b } as usize;
                let t = thick[e.a as usize * LAYERS + k].min(thick[e.b as usize * LAYERS + k]);
                u.abs() * dt * e.width * t / vol[d * LAYERS + k]
            })
            .collect();
        let (adj_at, adj) = (&self.adj_at, &self.adj);
        let donor = |i: usize| -> usize {
            let e = &edges[i / LAYERS];
            if wind[i] >= 0.0 { e.a as usize } else { e.b as usize }
        };
        // no cell gives more than 90 % of itself in a step
        let scale: Vec<f32> = (0..self.n * LAYERS)
            .into_par_iter()
            .map(|ck| {
                let (c, k) = (ck / LAYERS, ck % LAYERS);
                let mut out = 0f32;
                for &e in &adj[adj_at[c] as usize..adj_at[c + 1] as usize] {
                    let i = e as usize * LAYERS + k;
                    if donor(i) == c {
                        out += frac[i];
                    }
                }
                if out > 0.9 { 0.9 / out } else { 1.0 }
            })
            .collect();
        let (theta, vap, cloud, dirt) = (&self.theta, &self.vap, &self.cloud, &self.dirt);
        let moved = |amount: &Vec<u32>, i: usize, d: usize, k: usize| -> u32 {
            (amount[d * LAYERS + k] as f64 * (frac[i] * scale[d * LAYERS + k]) as f64) as u32
        };
        let out: Vec<(f32, u32, u32, u32, f32)> = (0..self.n * LAYERS)
            .into_par_iter()
            .map(|ck| {
                let (c, k) = (ck / LAYERS, ck % LAYERS);
                let (mut v, mut cl, mut di) = (vap[ck] as i64, cloud[ck] as i64, dirt[ck] as i64);
                let (mut vout, mut vin, mut heat_in) = (0f32, 0f32, 0f32);
                for &e in &adj[adj_at[c] as usize..adj_at[c + 1] as usize] {
                    let i = e as usize * LAYERS + k;
                    let d = donor(i);
                    let f = frac[i] * scale[d * LAYERS + k];
                    let mv = (moved(vap, i, d, k) as i64, moved(cloud, i, d, k) as i64, moved(dirt, i, d, k) as i64);
                    if d == c {
                        v -= mv.0;
                        cl -= mv.1;
                        di -= mv.2;
                        vout += f * vol[ck];
                    } else {
                        v += mv.0;
                        cl += mv.1;
                        di += mv.2;
                        let vv = f * vol[d * LAYERS + k];
                        vin += vv;
                        heat_in += vv * theta[d * LAYERS + k];
                    }
                }
                let stay = vol[ck] - vout;
                let th = (theta[ck] * stay + heat_in) / (stay + vin);
                (th, v as u32, cl as u32, di as u32, vin - vout)
            })
            .collect();
        for (ck, o) in out.into_iter().enumerate() {
            self.theta[ck] = o.0;
            self.vap[ck] = o.1;
            self.cloud[ck] = o.2;
            self.dirt[ck] = o.3;
            self.conv[ck] = o.4;
        }
    }

    /// Pass 5: air that piled up in a layer rises to the next and cools; air that left is replaced from
    /// above, which sinks and warms (the dry side of a range, the dry belts). Then convection mixes a
    /// layer with the one above where it is warmer.
    fn lift(&mut self, dt: f32) {
        let p = &self.p;
        let (vol, conv) = (&self.vol, &self.conv);
        self.theta
            .par_chunks_mut(LAYERS)
            .zip(self.vap.par_chunks_mut(LAYERS))
            .zip(self.cloud.par_chunks_mut(LAYERS).zip(self.dirt.par_chunks_mut(LAYERS)))
            .enumerate()
            .for_each(|(c, ((th, v), (cl, di)))| {
                let vl = &vol[c * LAYERS..c * LAYERS + LAYERS];
                let mut push = 0f32;
                for k in 0..LAYERS - 1 {
                    let net = conv[c * LAYERS + k] + push;
                    let (from, to, f, vol_moved) = if net > 0.0 {
                        let f = (net / vl[k]).min(0.5);
                        (k, k + 1, f, f * vl[k])
                    } else {
                        let f = (-net / vl[k + 1]).min(0.5);
                        (k + 1, k, f, f * vl[k + 1])
                    };
                    if f > 0.0 {
                        exchange(th, v, cl, di, vl, from, to, f, vol_moved);
                    }
                    push = net;
                }
                // the top cannot rise: what piled up there sinks a layer
                let top = conv[c * LAYERS + LAYERS - 1] + push;
                if top > 0.0 {
                    let f = (top / vl[LAYERS - 1]).min(0.5);
                    exchange(th, v, cl, di, vl, LAYERS - 1, LAYERS - 2, f, f * vl[LAYERS - 1]);
                }
                for k in 0..LAYERS - 1 {
                    if th[k] > th[k + 1] {
                        let x = (p.convect * dt).min(0.5) * vl[k].min(vl[k + 1]);
                        exchange(th, v, cl, di, vl, k, k + 1, x / vl[k], x);
                        exchange(th, v, cl, di, vl, k + 1, k, x / vl[k + 1], x);
                    }
                }
            });
    }

    /// Passes 6 and 7: water and soil give vapour; vapour over saturation becomes cloud; cloud over its
    /// limit falls as rain, or snow below freezing, washing dirt down; rain fills the soil, which seeps to
    /// the rivers; what it cannot hold runs off; snow melts in the warm.
    fn rain_and_land(&mut self, dt: f32) {
        let p = &self.p;
        let (vol, cool, water, area, ground) = (&self.vol, &self.cool, &self.water, &self.area, &self.ground);
        let speed: Vec<f32> = (0..self.n)
            .into_par_iter()
            .map(|c| {
                let (lo, hi) = (self.adj_at[c] as usize, self.adj_at[c + 1] as usize);
                self.adj[lo..hi].iter().map(|&e| self.wind[e as usize * LAYERS].abs()).sum::<f32>() * 2.0 / (hi - lo).max(1) as f32
            })
            .collect();
        let tg = &self.tg;
        let sea_delta: i64 = self
            .theta
            .par_chunks_mut(LAYERS)
            .zip(self.vap.par_chunks_mut(LAYERS))
            .zip(self.cloud.par_chunks_mut(LAYERS).zip(self.dirt.par_chunks_mut(LAYERS)))
            .zip(self.soil.par_iter_mut().zip(self.snow.par_iter_mut()))
            .zip(self.fell.par_iter_mut().zip(self.runoff.par_iter_mut().zip(self.dirt_down.par_iter_mut())))
            .enumerate()
            .map(|(c, ((((th, v), (cl, di)), (soil, snow)), (fell, (runoff, down))))| {
                let units = |g: f32, k: usize| (g as f64 * vol[c * LAYERS + k] as f64 / G_PER_UNIT) as i64;
                let w = water[c];
                let cap = (p.soil_m as f64 * area[c] as f64 * 4096.0 * (1.0 - w) as f64) as i64;
                // evaporation into the surface layer, toward saturation at the ground's temperature
                let t_ground = tg[c] - p.lapse * ground[c].max(0.0);
                let deficit = (units(sat_g(p, t_ground), 0) - v[0] as i64).max(0) as f64;
                let rate = ((p.evap * (1.0 + p.evap_wind * speed[c]) * dt) as f64).min(1.0);
                let from_sea = (deficit * rate * w as f64) as i64;
                let from_soil = ((deficit * rate * (1.0 - w) as f64 * (*soil as f64 / cap.max(1) as f64).min(1.0)) as i64).min(*soil as i64);
                v[0] += (from_sea + from_soil) as u32;
                *soil -= from_soil as u32;
                let mut rain = 0i64;
                for k in 0..LAYERS {
                    let t = th[k] - cool[c * LAYERS + k];
                    let qs = units(sat_g(p, t), k);
                    let (mut vv, mut cc) = (v[k] as i64, cl[k] as i64);
                    let to_cloud = if vv > qs { vv - qs } else { -cc.min(qs - vv) };
                    cc += to_cloud;
                    vv -= to_cloud;
                    th[k] += p.latent * (to_cloud as f64 * G_PER_UNIT / vol[c * LAYERS + k] as f64) as f32;
                    let over = cc - units(p.cloud_max, k);
                    if over > 0 {
                        let r = ((over as f64) * (p.rain * dt).min(1.0) as f64) as i64;
                        cc -= r;
                        rain += r;
                        let wash = (di[k] as f64 * (p.washout * dt).min(1.0) as f64) as u32;
                        di[k] -= wash;
                        *down += wash;
                    }
                    v[k] = vv as u32;
                    cl[k] = cc as u32;
                }
                *fell = rain as u32;
                let on_land = (rain as f64 * (1.0 - w) as f64) as i64;
                let mut to_sea = rain - on_land - from_sea;
                let t_air = th[0] - cool[c * LAYERS];
                let mut s = *soil as i64;
                if t_air < 0.0 {
                    *snow += on_land as u32;
                } else {
                    s += on_land;
                }
                if t_ground > 0.0 && *snow > 0 {
                    let m = ((*snow as f64) * ((p.melt * t_ground * dt) as f64).min(1.0)).ceil() as u32;
                    let m = m.min(*snow);
                    *snow -= m;
                    s += m as i64;
                }
                let seep = (s as f64 * (p.seep * dt).min(1.0) as f64) as i64;
                let over = (s - seep - cap).max(0);
                s -= seep + over;
                *soil = s as u32;
                *runoff = (seep + over) as u32;
                to_sea += seep + over;
                to_sea
            })
            .sum();
        self.sea += sea_delta;
    }
}

/// Move share `f` of layer `from`'s water and dirt to layer `to`, and mix in its heat by volume.
#[allow(clippy::too_many_arguments)]
fn exchange(th: &mut [f32], v: &mut [u32], cl: &mut [u32], di: &mut [u32], vl: &[f32], from: usize, to: usize, f: f32, vol_moved: f32) {
    let mv = |x: &mut [u32]| {
        let m = (x[from] as f64 * f as f64) as u32;
        x[from] -= m;
        x[to] += m;
    };
    mv(v);
    mv(cl);
    mv(di);
    th[to] = (th[to] * vl[to] + th[from] * vol_moved) / (vl[to] + vol_moved);
}

/// A body with land and sea for the tests and the cost runs: noise-free continents from a few caps.
pub fn test_world(m: usize, r: f64) -> (Vec<f32>, Vec<f32>) {
    let g = cube::Grid::new(m, r);
    let caps = [DVec3::new(1.0, 0.2, 0.3), DVec3::new(-0.5, 0.8, -0.2), DVec3::new(0.1, -0.7, 0.6)].map(|d| d.normalize());
    let mut ground = vec![0f32; g.n()];
    let mut water = vec![1f32; g.n()];
    for c in 0..g.n() as u32 {
        let d = g.d(c);
        let best = caps.iter().map(|k| k.dot(d)).fold(-1.0, f64::max);
        if best > 0.8 {
            water[c as usize] = 0.0;
            ground[c as usize] = ((best - 0.8) * 500.0) as f32;
        }
    }
    (ground, water)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Air {
        let (g, w) = test_world(16, 1000.0);
        Air::new(16, 1000.0, g, w, Params::default())
    }

    /// Water's ledger over many steps: exactly constant, rain, rivers and all (air.md, question 4).
    /// Dirt released into the air is conserved too.
    #[test]
    fn water_and_dirt_are_never_made_or_lost() {
        let mut a = small();
        a.dirt[5 * LAYERS] = 1_000_000;
        let (w0, d0) = (a.ledger(), a.dirt_total());
        let mut fell = 0u64;
        for _ in 0..2400 {
            a.step(10.0);
            fell += a.fell.iter().map(|&x| x as u64).sum::<u64>();
            assert_eq!(a.ledger(), w0, "water at t {}", a.t);
        }
        assert!(fell > 0, "it rained");
        assert_eq!(a.dirt_total(), d0);
        assert!(a.dirt_down.iter().any(|&x| x > 0), "rain washed dirt down");
    }

    /// Every column has four edges (the corners too), and a flow that is the same
    /// everywhere moves nothing net into any column: the seams fold like the faces' insides.
    #[test]
    fn the_grid_folds_over_seams_with_four_edges_a_column() {
        let a = small();
        for c in 0..a.n {
            assert_eq!(a.adj_at[c + 1] - a.adj_at[c], 4, "column {c}");
        }
        // a uniform east wind (any steady field of one direction): the volume each column gains
        let gain: Vec<f32> = (0..a.n)
            .map(|c| {
                let mut v = 0f32;
                for &k in &a.adj[a.adj_at[c] as usize..a.adj_at[c + 1] as usize] {
                    let e = &a.edges[k as usize];
                    let mid = (a.dir[e.a as usize] + a.dir[e.b as usize]).normalize();
                    let east = DVec3::Z.cross(mid).try_normalize().unwrap_or(DVec3::X);
                    let u = east.dot(e.normal) as f32 * e.width;
                    v += if e.a as usize == c { -u } else { u };
                }
                v / a.area[c].sqrt()
            })
            .collect();
        // (near a pole the east field itself turns inside a cell: not the grid's error)
        let worst = gain.iter().enumerate().filter(|(c, _)| a.lat[*c].to_degrees().abs() < 75.0).fold((0, 0f32), |b, (c, &g)| if g.abs() > b.1.abs() { (c, g) } else { b });
        assert!(worst.1.abs() < 0.02, "column {} gains {} of its width a second", worst.0, worst.1);
    }

    /// The same start, the same state after a day, on any number of threads.
    #[test]
    fn the_same_world_runs_the_same_weather() {
        let run = |threads: usize| {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
            pool.install(|| {
                let mut a = small();
                for _ in 0..120 {
                    a.step(10.0);
                }
                let mut h = 0xcbf29ce484222325u64;
                for x in a.theta.iter().map(|v| v.to_bits()).chain(a.vap.iter().copied()).chain(a.wind.iter().map(|v| v.to_bits())) {
                    h = (h ^ x as u64).wrapping_mul(0x100000001b3);
                }
                h
            })
        };
        assert_eq!(run(1), run(4));
    }

    /// The bands: easterlies at the equator, westerlies in the middle latitudes, after the wind settled.
    #[test]
    fn trades_blow_from_the_east_and_westerlies_from_the_west() {
        let mut a = small();
        for _ in 0..200 {
            a.step(10.0);
        }
        let mean_east = |lo: f32, hi: f32| {
            let cs: Vec<usize> = (0..a.n).filter(|&c| a.water[c] == 1.0 && (lo..hi).contains(&a.lat[c].to_degrees().abs())).collect();
            cs.iter().map(|&c| a.surface_wind(c).0).sum::<f32>() / cs.len() as f32
        };
        assert!(mean_east(0.0, 15.0) < -1.0, "trades {}", mean_east(0.0, 15.0));
        assert!(mean_east(35.0, 55.0) > 1.0, "westerlies {}", mean_east(35.0, 55.0));
    }
}
