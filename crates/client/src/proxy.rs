//! The players' bodies in the traced scene (ADR 0119): a few boxes from the ape's parts at rest, one small BLAS
//! shared by every player, one TLAS instance a player near the camera (rt.rs), seen by shadow and lamp rays only
//! (mask `16 << body`, shader.wgsl `PROXY_INST`), so a body shades the ground under the sun and the lamps and
//! is never drawn, nor met by a look or the bounce. The camera's own player is the first; a lamp held by a
//! player passes through its own body (its `col.w`, the body's place in the list plus one), and the bare arm
//! and the held item through the camera's own.
use engine::models::Model;
use engine::net::Player;
use engine::phys::HEIGHT;
use engine::world::World;
use glam::{DVec3, Vec3};

/// Players farther than this from the camera cast none (a body's shadow under the sun is a few metres long).
pub const PROXY_M: f64 = 64.0;
/// Bodies at most: the TLAS slots they take.
pub const PROXY_MAX: usize = 64;

/// The ape's parts at rest as boxes, lowest corner and highest, in metres of the player: its feet at y = 0, its
/// middle on x = z = 0, as tall as the player's box (`phys::HEIGHT`), its front +z.
pub fn boxes(m: &Model) -> Vec<[Vec3; 2]> {
    let poses = m.pose(None, 0.0);
    let parts: Vec<[Vec3; 2]> = m
        .parts
        .iter()
        .zip(&poses)
        .map(|(p, x)| {
            let s = Vec3::new(p.size[0] as f32, p.size[1] as f32, p.size[2] as f32);
            let c = (0..8).map(|k| x.transform_point3(Vec3::new((k & 1) as f32, (k >> 1 & 1) as f32, (k >> 2) as f32) * s));
            c.fold([Vec3::INFINITY, Vec3::NEG_INFINITY], |[a, b], v| [a.min(v), b.max(v)])
        })
        .collect();
    let [lo, hi] = parts.iter().fold([Vec3::INFINITY, Vec3::NEG_INFINITY], |[a, b], q| [a.min(q[0]), b.max(q[1])]);
    let k = HEIGHT as f32 / (hi.y - lo.y);
    let mid = Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5);
    parts.iter().map(|q| [(q[0] - mid) * k, (q[1] - mid) * k]).collect()
}

/// A box of the player's size when no ape is loaded.
pub fn plain() -> Vec<[Vec3; 2]> {
    vec![[Vec3::new(-0.3, 0.0, -0.15), Vec3::new(0.3, HEIGHT as f32, 0.15)]]
}

/// The boxes as triangles, 12 a box, for the BLAS.
pub fn triangles(boxes: &[[Vec3; 2]]) -> Vec<[f32; 3]> {
    // each face's corners as bits of (x, y, z) from the box's lowest corner, two triangles a face
    const FACES: [[usize; 4]; 6] = [[0, 2, 6, 4], [1, 5, 7, 3], [0, 4, 5, 1], [2, 3, 7, 6], [0, 1, 3, 2], [4, 6, 7, 5]];
    let mut out = Vec::with_capacity(boxes.len() * 36);
    for [a, b] in boxes {
        let c = |k: usize| [if k & 1 == 0 { a.x } else { b.x }, if k & 2 == 0 { a.y } else { b.y }, if k & 4 == 0 { a.z } else { b.z }];
        for f in FACES {
            out.extend([f[0], f[1], f[2], f[0], f[2], f[3]].map(c));
        }
    }
    out
}

/// One body to cast: its body (the TLAS mask's), its transform from the player's metres into its body's frame
/// from the body's centre (rows of a 3 x 4, as a TLAS instance takes it), and its player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Proxy {
    pub body: u8,
    pub x: [f32; 12],
    pub id: u32,
}

/// The bodies near the camera: `me` first (its id, feet and look, in its frame), then the roster's others,
/// `cams` the camera in each body's frame (frame.rs `cams`); a player between the bodies casts none.
pub fn near(world: &World, me: Option<(u32, DVec3, DVec3)>, others: &[Player], cams: &[DVec3]) -> Vec<Proxy> {
    let me_id = me.map(|m| m.0);
    let rest = others.iter().filter(|o| Some(o.id) != me_id).map(|o| (o.id, DVec3::from_array(o.pos), DVec3::from_array(o.fwd.map(f64::from))));
    me.into_iter()
        .chain(rest)
        .filter_map(|(id, feet, fwd)| {
            let body = world.frame_at(feet);
            let b = world.bodies.get(body)?;
            if (feet - cams.get(body)?).length() > PROXY_M {
                return None;
            }
            let up = (feet - b.centre).normalize_or(DVec3::Z);
            let z = (fwd - up * fwd.dot(up)).normalize_or(up.any_orthonormal_vector());
            let x = up.cross(z);
            let t = feet - b.centre;
            let r = |a: usize| [x[a] as f32, up[a] as f32, z[a] as f32, t[a] as f32];
            let [r0, r1, r2] = [r(0), r(1), r(2)];
            Some(Proxy { body: body as u8, x: [r0, r1, r2].concat().try_into().unwrap(), id })
        })
        .take(PROXY_MAX)
        .collect()
}

/// Whether the bodies in the traced scene stand where they were built: the same players on the same bodies, none
/// moved a millimetre or turned more than a thousandth (its axes' parts). A body eased between rosters (ADR 0209)
/// is given new floats every frame, and the TLAS was built every frame for them (3,200 builds in a quick stand run
/// for 450 before: 0.3 ms more of the frame thread's `rt` and 0.2 of `submit`).
pub fn same(now: &[Proxy], built: &[Proxy]) -> bool {
    // (the place is an f32 from the body's centre: a few of its steps more than that on a big body, past 16 km)
    now.len() == built.len() && now.iter().zip(built).all(|(a, b)| a.id == b.id && a.body == b.body && a.x.iter().zip(&b.x).all(|(p, q)| (p - q).abs() <= 1e-3f32.max(q.abs() * 4.0 * f32::EPSILON)))
}

/// What a lamp held by player `id` carries in its `col.w` (shader.wgsl's lamp loop): its holder's place in
/// `proxies` plus one, so its rays pass through its own body; 0, none, for a lamp no body holds.
pub fn holder(proxies: &[Proxy], id: u32) -> f32 {
    proxies.iter().position(|p| p.id == id).map_or(0.0, |k| k as f32 + 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ape() -> Model {
        let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        engine::models::parse_life("ape.toml", include_str!("../../../data/models/ape.toml"), &pal, &|_| true, &|_| true).unwrap().0
    }

    /// A body that did not move (a float's noise) leaves the TLAS as built; one that moved, turned or changed is a
    /// rebuild.
    #[test]
    fn a_body_that_stood_still_is_not_built_again() {
        let p = Proxy { body: 0, x: [1.0, 0.0, 0.0, 1200.25, 0.0, 1.0, 0.0, -3.5, 0.0, 0.0, 1.0, 40.0], id: 7 };
        let nudged = |k: usize, by: f32| {
            let mut q = p;
            q.x[k] += by;
            q
        };
        assert!(same(&[p], &[nudged(3, 0.0001)]), "a tenth of a millimetre");
        assert!(!same(&[p], &[nudged(3, 0.005)]), "five millimetres");
        assert!(!same(&[p], &[nudged(0, -0.01)]), "a turn");
        assert!(!same(&[p], &[Proxy { id: 8, ..p }]), "another player");
        assert!(!same(&[p], &[]), "one gone");
    }

    /// ADR 0119: the ape's parts at rest, one box each, stand on the floor as tall as the player and no wider
    /// than a block, its front +z.
    #[test]
    fn the_proxy_is_the_apes_parts_standing_in_the_players_box() {
        let m = ape();
        let b = boxes(&m);
        assert_eq!(b.len(), m.parts.len());
        let lo = b.iter().fold(Vec3::INFINITY, |a, q| a.min(q[0]));
        let hi = b.iter().fold(Vec3::NEG_INFINITY, |a, q| a.max(q[1]));
        assert!(lo.y.abs() < 1e-4 && (hi.y - HEIGHT as f32).abs() < 1e-4, "feet on the floor, as tall as the player: {lo} {hi}");
        assert!(lo.x >= -0.5 && hi.x <= 0.5 && lo.z >= -0.5 && hi.z <= 0.5, "within a block across: {lo} {hi}");
        assert!(b.iter().all(|q| q[0].cmplt(q[1]).all()), "every box has room");
        let head = &b[m.parts.iter().position(|p| p.name == "head").unwrap()];
        assert!(head[1].y > 0.95 * HEIGHT as f32, "the head on top: {head:?}");
        assert_eq!(triangles(&b).len(), b.len() * 36);
    }

    /// One instance a player within `PROXY_M` of the camera, this one's first, none farther; placed at its feet
    /// in its body's frame with its up out from the centre.
    #[test]
    fn one_body_a_player_in_range_and_the_cameras_own_first() {
        let w = World::new(768, 256, 1);
        let c = w.bodies[0].centre;
        let feet = w.block_centre(w.air_over_ground(0, w.bodies[0].spawn));
        let up = (feet - c).normalize();
        let cams = [feet + up * 1.6];
        let side = up.any_orthonormal_vector();
        let other = |id: u32, d: f64| Player { id, name: String::new(), pos: (feet + side * d).to_array(), fwd: side.as_vec3().to_array(), pitch: 0.0, hand: 0 };
        let others = [other(7, 10.0), other(8, PROXY_M + 10.0), other(3, 0.0)];
        let p = near(&w, Some((3, feet, side)), &others, &cams);
        assert_eq!(p.iter().map(|p| p.id).collect::<Vec<_>>(), [3, 7], "me first, the near one, not the far one, me once");
        let at = |p: &Proxy| DVec3::new(p.x[3] as f64, p.x[7] as f64, p.x[11] as f64);
        assert!((at(&p[0]) - (feet - c)).length() < 1e-2 && (at(&p[1]) - (feet + side * 10.0 - c)).length() < 1e-2);
        let y = DVec3::new(p[0].x[1] as f64, p[0].x[5] as f64, p[0].x[9] as f64);
        assert!(y.dot(up) > 0.999, "its up is out from the centre");
        assert!(near(&w, None, &others[..1], &cams).len() == 1 && near(&w, None, &[], &cams).is_empty());
        // a held lamp passes through its holder's own body and no other's
        assert_eq!([holder(&p, 3), holder(&p, 7), holder(&p, 8)], [1.0, 2.0, 0.0]);
        let many: Vec<Player> = (0..100).map(|k| other(100 + k, 1.0)).collect();
        assert_eq!(near(&w, None, &many, &cams).len(), PROXY_MAX);
    }
}
