//! What the map keeps of the bake's air (W3): the air columns' ground, the year's means per column, and the
//! grid's state at the year's end, world time 0, from which the live grid (`crate::weather`) starts. A cell's
//! baked temperature is the means brought to it the way the bake brought them (`land::upsample_at`) less the
//! lapse by its height, so the map need not keep 3.5 million of them.
use super::air::{Air, Params, State};
use super::grid::Grid;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Climate {
    /// Air columns per face edge (`LifeParams::air_cell_m` wide), and the bake's radius, metres.
    pub m: u32,
    pub r: f64,
    /// Per air column: the mean land height (0 over water) and the share of open water.
    pub ground: Vec<f32>,
    pub water: Vec<f32>,
    /// The year's means per air column: the surface air's potential temperature (C at sea level), its day
    /// swing (K) and rain and snow (mm a day).
    pub theta: Vec<f32>,
    pub swing: Vec<f32>,
    pub rain: Vec<f32>,
    /// The grid at the year's end.
    pub state: State,
}

impl Climate {
    /// Checks a climate from a file or a server: sizes that fit its grid, numbers that are numbers.
    pub fn check(&self) -> Result<(), String> {
        if !(2..=4096).contains(&self.m) || !(self.r.is_finite() && self.r > 0.0) {
            return Err(format!("a climate of {} columns a face edge that does not fit", self.m));
        }
        let n = 6 * self.m as usize * self.m as usize;
        let fine = [&self.ground, &self.water, &self.theta, &self.swing, &self.rain].iter().all(|v| v.len() == n && v.iter().all(|x| x.is_finite()));
        if !fine {
            return Err(format!("a climate of {} columns a face edge that does not fit", self.m));
        }
        // the state against the grid it belongs to
        self.air().map(|_| ())
    }
    /// The air columns' grid.
    pub fn grid(&self) -> Grid {
        Grid::new(self.m as usize, self.r)
    }
    /// The live grid at the state the bake left it in.
    pub fn air(&self) -> Result<Air, String> {
        let mut a = Air::new(self.m as usize, self.r, self.ground.clone(), self.water.clone(), Params::default());
        a.restore(&self.state)?;
        Ok(a)
    }
    /// The year's means at unit direction `d`, smoothed over the air columns as the bake smoothed them to its
    /// cells: (potential temperature C, day swing K, rain mm a day). `g` is `grid()`.
    pub fn at(&self, g: &Grid, d: glam::DVec3) -> (f32, f32, f32) {
        let d32 = d.as_vec3().to_array();
        let k = g.locate(glam::DVec3::from_array(d32.map(f64::from)));
        let f = |v: &[f32]| super::land::upsample_at(g, d32, k, v);
        (f(&self.theta), f(&self.swing), f(&self.rain))
    }
}
