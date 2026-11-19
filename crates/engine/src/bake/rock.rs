//! Stage 9 of `docs/design/worldgen.md`, its map part (W4, ADR 0063): each cell's rock province, from what
//! the plates did there. The rocks a column holds under its soil come from the province's layers in
//! `data/rocks.toml` (`crate::rock`).
use super::grid::Grid;
use super::land::{at, dijkstra, noise, Params, Tectonics};

/// Old continental interior: granite over gneiss.
pub const SHIELD: u8 = 0;
/// Continental interior under a sedimentary cover: sandstone, shale, limestone.
pub const BASIN: u8 = 1;
/// Where continents collided: slate, marble, quartzite, schist.
pub const FOLDED: u8 = 2;
/// The continental side over a sinking ocean: a chain of old volcanoes, tuff over andesite.
pub const ARC: u8 = 3;
/// Where a continent is pulled apart: basalt.
pub const RIFT: u8 = 4;
/// Oceanic crust: basalt over gabbro.
pub const OCEAN: u8 = 5;
/// Luna's (0165-bake-stages; a world of generation 4 on): her old pale crust, anorthosite.
pub const LUNA_HIGHLAND: u8 = 6;
/// Luna's maria: the basins' dark basalt fill over the anorthosite.
pub const LUNA_MARE: u8 = 7;
/// Luna's thorium province round the largest mare (KREEP rock over the anorthosite).
pub const LUNA_KREEP: u8 = 8;
/// The rows of `data/rocks.toml`, in id order: Terra's (`SHIELD` .. `OCEAN`, what `provinces` lays), then Luna's
/// (`LUNA_HIGHLAND` .. `LUNA_KREEP`, read from her map's kinds, `super::luna::province`).
pub const NAMES: [&str; 9] = ["shield", "basin", "folded belt", "volcanic arc", "rift", "ocean floor", "highland crust", "mare", "kreep terrane"];

/// Each cell's province. A belt is about as wide as the uplift that boundary raised (`Params`), so the folded
/// rock lies under the ranges, and the ancient sutures inside the continents are old folded belts; shields and
/// basins share the rest of the continents by a broad noise.
pub fn provinces(g: &Grid, t: &Tectonics, seed: u64, p: &Params) -> Vec<u8> {
    let (mut collide, mut arc, mut rift) = (Vec::new(), Vec::new(), Vec::new());
    for c in 0..g.n() as u32 {
        let pc = t.plate[c as usize];
        match t.boundary[c as usize] {
            1 => {
                let other_land = g.around(c).filter(|&o| t.plate[o as usize] != pc).any(|o| t.continental[o as usize]);
                if t.continental[c as usize] && other_land { collide.push((c, 0.0)) } else { arc.push((c, 0.0)) }
            }
            2 => rift.push((c, 0.0)),
            _ => {}
        }
    }
    let near = |src: &[(u32, f64)], w: f64| dijkstra(g, src, w, |_, _, d, _| d).0;
    let (dc, da, dr) = (near(&collide, 1.5 * p.collision_w * g.r), near(&arc, 1.5 * p.arc_w * g.r), near(&rift, 2.0 * p.rift_w * g.r));
    let shields = noise(seed as i32 ^ 0x5a1d, 3);
    // the old worn ranges along ancient sutures (`land::tectonics`, the same noise): old folded belts
    let old = noise(seed as i32 ^ 0x0e1d, 3);
    (0..g.n())
        .map(|c| match () {
            _ if !t.continental[c] => OCEAN,
            _ if dc[c].is_finite() || 1.0 - 4.0 * at(&old, g.d(c as u32), 2.2).abs() > 0.8 => FOLDED,
            _ if da[c].is_finite() => ARC,
            _ if dr[c].is_finite() => RIFT,
            _ if at(&shields, g.d(c as u32), 2.5) > 0.05 => SHIELD,
            _ => BASIN,
        })
        .collect()
}
