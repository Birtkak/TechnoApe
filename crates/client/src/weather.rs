//! The planet's air as this client draws it (ADR 0076): the server's columns (`ToClient::Sky`), the step
//! before them, and the cloud map on the GPU blended from one to the other over the grid's step, so clouds
//! move rather than jump every `weather::DT` seconds. Everything here follows from the server's columns and
//! the world's clock, so two clients see the same sky.
use engine::weather::{Cell, DT};

/// Seconds the clouds' cells take to slide through one drift phase (clouds.wgsl `cl_sample`): long enough
/// that one phase holds for minutes, short enough that a wind change does not shear the pattern far.
pub const DRIFT: f64 = 240.0;
/// The blend between the two steps is uploaded in this many steps a grid step (about 6 a second).
const BLEND_STEPS: f64 = 64.0;

#[derive(Default)]
pub struct Sky {
    pub cur: Vec<Cell>,
    prev: Vec<Cell>,
    /// the world time `cur` landed
    t: f64,
    /// the blend step last uploaded (None: upload again)
    shown: Option<u32>,
}

impl Sky {
    /// The server's message: every column, or the ones that changed.
    pub fn take(&mut self, t: f64, all: Vec<Cell>, changed: Vec<(u32, Cell)>) {
        if !all.is_empty() {
            self.prev = all.clone();
            self.cur = all;
        } else {
            self.prev.clone_from(&self.cur);
        }
        for (k, c) in changed {
            if let Some(x) = self.cur.get_mut(k as usize) {
                *x = c;
            }
        }
        self.t = t;
        self.shown = None;
    }

    /// The map's columns a face edge (0: none yet).
    pub fn m(&self) -> usize {
        ((self.cur.len() / 6) as f64).sqrt().round() as usize
    }

    /// The share of the way from the last step to this one at world time `now`.
    fn blend(&self, now: f64) -> f64 {
        ((now - self.t) / DT as f64).clamp(0.0, 1.0)
    }

    /// The map as the GPU holds it at world time `now` (clouds.wgsl's layout): faces 0 to 5 (cover, rain,
    /// wind east and north biased by a half) blended from the last step, then (snow, temperature, 0, 0).
    pub fn texels(&self, now: f64) -> Vec<u8> {
        let b = (self.blend(now) * BLEND_STEPS).round() / BLEND_STEPS;
        let lerp = |a: u8, c: u8| (a as f64 + (c as f64 - a as f64) * b).round() as u8;
        let mut out = Vec::with_capacity(self.cur.len() * 8);
        for (p, c) in self.prev.iter().zip(&self.cur) {
            out.extend_from_slice(&[lerp(p[0], c[0]), lerp(p[1], c[1]), lerp(p[2] ^ 0x80, c[2] ^ 0x80), lerp(p[3] ^ 0x80, c[3] ^ 0x80)]);
        }
        for c in &self.cur {
            out.extend_from_slice(&[c[4], c[5], 0, 0]);
        }
        out
    }

    /// Writes the map when it changed or the blend moved a step; true when it wrote.
    pub fn upload(&mut self, queue: &wgpu::Queue, tex: &wgpu::Texture, now: f64) -> bool {
        let m = self.m();
        if m == 0 || m > crate::gpu::SKY_TEX as usize || 6 * m * m != self.cur.len() {
            return false;
        }
        let step = (self.blend(now) * BLEND_STEPS).round() as u32;
        if self.shown == Some(step) {
            return false;
        }
        self.shown = Some(step);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &self.texels(now),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(m as u32 * 4), rows_per_image: Some(m as u32) },
            wgpu::Extent3d { width: m as u32, height: m as u32, depth_or_array_layers: 12 },
        );
        true
    }

    /// `Globals::cloud` at world time `now` for a planet whose sea lies `sea` metres from its centre.
    pub fn cloud(&self, now: f64, sea: f64) -> [f32; 4] {
        let m = self.m();
        if m == 0 || m > crate::gpu::SKY_TEX as usize {
            return [0.0; 4];
        }
        [(now / DRIFT).rem_euclid(1.0) as f32, DRIFT as f32, sea as f32, m as f32]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A step's columns are blended in from the last over the grid's step, the winds through their sign, and a
    /// delta changes only the columns it names.
    #[test]
    fn the_map_blends_from_the_last_step_to_this_one() {
        let mut s = Sky::default();
        let n = 6 * 4 * 4;
        s.take(0.0, vec![[0, 0, (-4i8) as u8, 4, 9, 128]; n], Vec::new());
        assert_eq!(s.m(), 4);
        s.take(100.0, Vec::new(), vec![(5, [200, 100, 4, (-4i8) as u8, 9, 128])]);
        let half = s.texels(100.0 + DT as f64 / 2.0);
        assert_eq!(&half[5 * 4..5 * 4 + 4], &[100, 50, 128, 128], "halfway, the winds through calm");
        assert_eq!(&half[0..4], &[0, 0, 124, 132]);
        let done = s.texels(100.0 + DT as f64 * 2.0);
        assert_eq!(&done[5 * 4..5 * 4 + 4], &[200, 100, 132, 124]);
        assert_eq!(&done[n * 4..n * 4 + 2], &[9, 128], "the snow layers follow the faces'");
        assert_eq!(s.cloud(0.0, 1000.0)[3], 4.0);
        assert_eq!(Sky::default().cloud(0.0, 1000.0), [0.0; 4]);
    }
}
