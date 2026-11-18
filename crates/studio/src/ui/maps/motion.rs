//! The preview's motion (docs/design/studio-maps.md, step 3): the previewed node's whole loop worked out once
//! on a worker and kept until the graph changes; Particles drifting in its push; Sway bending a row of blades
//! by it. Plain 2D painting, no game renderer.
use crate::map::{Data, Graph, Map, Pic};
use std::sync::mpsc::{Receiver, TryRecvError};

/// What a loop is worked out for: the node's stamp (new whenever something before it changes) and the side.
pub type Key = (u64, usize);

/// A node's whole loop at one side: its frames and each frame's push.
pub struct Strip {
    pub key: Key,
    pub side: usize,
    pub kind: Data,
    pub frames: Vec<Pic>,
    pub push: Vec<Vec<[f32; 2]>>,
}

impl Strip {
    pub fn work(m: Map, node: usize, key: Key) -> Strip {
        let side = key.1;
        let mut g = Graph::new(m);
        let kind = g.kinds[node];
        let frames: Vec<Pic> = (0..g.map.frames).map(|f| g.pic(node, f, side).clone()).collect();
        let push = frames.iter().map(|p| push(p, side, kind)).collect();
        Strip { key, side, kind, frames, push }
    }
}

/// A picture's push at each pixel: a direction as it is; a number, or a colour's brightness, curled (it circles
/// round its highs and lows); the strongest push is 1.
pub fn push(p: &Pic, side: usize, kind: Data) -> Vec<[f32; 2]> {
    if kind == Data::Dir {
        return p.iter().map(|c| [c[0], c[1]]).collect();
    }
    let v = |x: usize, y: usize| {
        let c = p[(y % side) * side + x % side];
        (c[0] + c[1] + c[2]) / 3.0
    };
    let f: Vec<[f32; 2]> = (0..side * side)
        .map(|i| {
            let (x, y) = (i % side, i / side);
            let (dx, dy) = (v(x + 1, y) - v(x + side - 1, y), v(x, y + 1) - v(x, y + side - 1));
            [dy, -dx]
        })
        .collect();
    let top = f.iter().map(|d| d[0].hypot(d[1])).fold(0.0, f32::max);
    if top == 0.0 { f } else { f.iter().map(|d| [d[0] / top, d[1] / top]).collect() }
}

/// The value at a point (0..1 across and down) of a picture `side` a side, its nearest pixel.
fn at<T: Copy>(p: &[T], side: usize, u: f32, v: f32) -> T {
    let i = |t: f32| ((t.rem_euclid(1.0) * side as f32) as usize).min(side - 1);
    p[i(v) * side + i(u)]
}

/// One step of a dot (0..1 across and down) along the push where it stands, `speed` sides a second at the
/// strongest push, wrapping at the edges.
pub fn step(p: [f32; 2], push: &[[f32; 2]], side: usize, speed: f32, dt: f32) -> [f32; 2] {
    let d = at(push, side, p[0], p[1]);
    [(p[0] + d[0] * speed * dt).rem_euclid(1.0), (p[1] + d[1] * speed * dt).rem_euclid(1.0)]
}

/// How far a blade standing at `u` (0..1 across) bends, times `strength`: a direction's x part; a number, or a
/// colour's brightness, around its middle (middle grey stands straight). Read along the picture's middle row.
pub fn bend(p: &Pic, side: usize, kind: Data, u: f32, strength: f32) -> f32 {
    let c = at(p, side, u, 0.5);
    let v = if kind == Data::Dir { c[0] } else { ((c[0] + c[1] + c[2]) / 3.0 - 0.5) * 2.0 };
    (v * strength).clamp(-1.5, 1.5)
}

/// A dot of Particles: where it is, where it was (its trail, newest last), how long it has lived and will.
pub struct Dot {
    pub at: [f32; 2],
    pub trail: Vec<[f32; 2]>,
    age: f32,
    life: f32,
    k: u32,
}
const TRAIL: usize = 10;

/// Dots strewn at random (a fixed hash, so a shot is the same each time), each living 2 to 4 seconds before it starts again elsewhere,
/// so dots that pile up do not stay piled.
pub fn dots(n: usize) -> Vec<Dot> {
    (0..n as u32).map(|k| Dot { at: spot(k), trail: vec![], age: (k % 40) as f32 * 0.1, life: 2.0 + (k % 7) as f32 / 3.0, k }).collect()
}
fn spot(k: u32) -> [f32; 2] {
    let h = |x: u32| {
        let x = x.wrapping_mul(0x9E37_79B9) ^ (x >> 15);
        let x = x.wrapping_mul(0x85EB_CA6B) ^ (x >> 13);
        (x.wrapping_mul(0xC2B2_AE35) >> 8) as f32 / (1 << 24) as f32
    };
    [h(2 * k + 1), h(2 * k + 2)]
}

/// The dots a step on.
pub fn drift(dots: &mut [Dot], push: &[[f32; 2]], side: usize, speed: f32, dt: f32) {
    for d in dots {
        d.age += dt;
        if d.age > d.life {
            d.k += 997;
            (d.at, d.age) = (spot(d.k), 0.0);
            d.trail.clear();
        }
        d.trail.push(d.at);
        if d.trail.len() > TRAIL {
            d.trail.remove(0);
        }
        d.at = step(d.at, push, side, speed, dt);
    }
}

/// The kept loop, worked out on a worker, one at a time.
#[derive(Default)]
pub struct Player {
    pub strip: Option<Strip>,
    job: Option<(Key, Receiver<Strip>)>,
    /// Loops sent to be worked out (the tests count the rework).
    pub started: usize,
}

impl Player {
    /// The kept loop. When it is not the one for `key`, that one is started on a worker (unless one is on its
    /// way already; then once that is in) and the old one is given meanwhile.
    pub fn get(&mut self, key: Key, m: &Map, node: usize) -> Option<&Strip> {
        if let Some((_, rx)) = &self.job {
            match rx.try_recv() {
                Ok(s) => (self.strip, self.job) = (Some(s), None),
                Err(TryRecvError::Disconnected) => self.job = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        let kept = self.strip.as_ref().is_some_and(|s| s.key == key);
        if !kept && self.job.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            let m = m.clone();
            std::thread::spawn(move || {
                let _ = tx.send(Strip::work(m, node, key));
            });
            self.job = Some((key, rx));
            self.started += 1;
        }
        self.strip.as_ref()
    }
    /// For tests and shots: the loop on its way, in.
    pub fn wait(&mut self) {
        if let Some((_, rx)) = self.job.take() {
            if let Ok(s) = rx.recv() {
                self.strip = Some(s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(side: usize, c: [f32; 3]) -> Pic {
        vec![c; side * side]
    }

    #[test]
    fn a_dot_steps_along_the_push_and_wraps() {
        let side = 8;
        let right = push(&flat(side, [1.0, 0.0, 0.0]), side, Data::Dir);
        let a = step([0.5, 0.5], &right, side, 1.0, 0.1);
        assert!((a[0] - 0.6).abs() < 1e-5 && a[1] == 0.5, "{a:?}");
        let b = step([0.95, 0.5], &right, side, 1.0, 0.1);
        assert!((b[0] - 0.05).abs() < 1e-5, "wraps at the right edge: {b:?}");
        let up = push(&flat(side, [0.0, -1.0, 0.0]), side, Data::Dir);
        let c = step([0.5, 0.02], &up, side, 1.0, 0.1);
        assert!((c[1] - 0.92).abs() < 1e-5 && c[0] == 0.5, "up the picture, round to its foot: {c:?}");
        // a number is curled: a hill's push circles it, at right angles to the slope
        let hill: Pic = (0..side * side).map(|i| {
            let (x, y) = ((i % side) as f32 - 4.0, (i / side) as f32 - 4.0);
            let v = (-(x * x + y * y) / 8.0).exp();
            [v, v, v]
        }).collect();
        let f = push(&hill, side, Data::Num);
        let east = f[4 * side + 6];
        assert!(east[0].abs() < 1e-5 && east[1] != 0.0, "east of the top the push runs up or down: {east:?}");
        assert!(f.iter().all(|d| d[0].hypot(d[1]) <= 1.0 + 1e-5));
    }

    #[test]
    fn a_blade_stands_straight_on_middle_grey_and_bends_with_the_field() {
        let side = 8;
        assert_eq!(bend(&flat(side, [0.5; 3]), side, Data::Num, 0.3, 2.0), 0.0);
        assert!(bend(&flat(side, [0.9; 3]), side, Data::Num, 0.3, 1.0) > 0.0);
        assert!(bend(&flat(side, [0.1; 3]), side, Data::Num, 0.3, 1.0) < 0.0);
        assert!(bend(&flat(side, [0.6, -0.2, 0.0]), side, Data::Dir, 0.7, 1.0) > 0.0);
        assert!(bend(&flat(side, [-0.6, 0.9, 0.0]), side, Data::Dir, 0.7, 1.0) < 0.0, "a direction's x part only");
        assert_eq!(bend(&flat(side, [0.6, 0.0, 0.0]), side, Data::Dir, 0.7, 0.0), 0.0, "no strength, no bend");
    }

    #[test]
    fn the_kept_loop_is_not_worked_out_again_while_it_plays() {
        let pal = engine::palette::Palette::parse(&std::fs::read_to_string("../../data/palette.toml").unwrap()).unwrap();
        let m = crate::map::parse("smoke swirl", &std::fs::read_to_string("../../data/maps/smoke swirl.toml").unwrap(), &pal).unwrap();
        let g = Graph::new(m.clone());
        let node = g.out();
        let key = (g.stamp[node], 16);
        let mut p = Player::default();
        for _ in 0..100 {
            p.get(key, &m, node);
            p.wait();
        }
        assert_eq!(p.started, 1);
        let s = p.strip.as_ref().unwrap();
        assert_eq!((s.frames.len(), s.push.len(), s.frames[0].len()), (m.frames, m.frames, 16 * 16));
        p.get((key.0 + 1, 16), &m, node);
        assert_eq!(p.started, 2, "a changed graph: worked out anew");
    }
}
