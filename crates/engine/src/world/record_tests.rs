//! Terra's and Luna's interiors from their rows (0162-planet-interior) against the numbers of the build before
//! them (main at 63c92f37, the 4 km planet and its 1664 moon, generation 1), recorded by that build: the pull
//! to 1e-12 (only physics reads it), the weight of rock to the bit (it decides the mantle's rock and ores).
use super::*;

/// (radius, the pull there by bits) of the recorded build: the ground, 100 m down, the crust's floor, the middle
/// of the mantle, R/sqrt 2, a metre under it, the middle of the core, R/4, R/1000, the centre, 1.5 R, 3 R.
const PULL: [[(f64, u64); 12]; 2] = [
    [
        (4245.432108703771, 0x40339eb851eb851f),
        (4145.432108703771, 0x4033a0c83d995f9a),
        (4053.4321087037706, 0x4033a80c84d800cc),
        (3527.7029709176554, 0x4034503fad42f2da),
        (3001.9738331315402, 0x403645a917db39cb),
        (3000.9738331315402, 0x403643c2df13f105),
        (1500.9869165657701, 0x402645a917db39ca),
        (1061.3580271759427, 0x401f7f5d9a4efd70),
        (4.2454321087037705, 0x3fa0207175eb00bc),
        (0.0, 0x0000000000000000),
        (6368.1481630556555, 0x402170a3d70a3d71),
        (12736.296326111311, 0x400170a3d70a3d71),
    ],
    [
        (1149.8045294406045, 0x40154147ae147ae2),
        (1049.8045294406045, 0x40156c4baa4d27b6),
        (957.8045294406045, 0x4015fc2cac8d348d),
        (885.4195546235317, 0x4016d0e32ad44742),
        (813.0345798064589, 0x401820cc8482d3f2),
        (812.0345798064589, 0x40181933a165b0e1),
        (406.5172899032294, 0x400820cc8482d3f1),
        (287.4511323601511, 0x40010fa808eac948),
        (1.1498045294406045, 0x3f81787aea6940cc),
        (0.0, 0x0000000000000000),
        (1724.7067941609066, 0x4002e4b17e4b17e5),
        (3449.413588321813, 0x3fe2e4b17e4b17e5),
    ],
];

/// (radius, the weight of rock from the sea level down to it, MPa by bits) of the recorded build: 10, 100 m
/// under the sea, the crust's floor, 300 and 700 m under the sea, the grid's floor, R/sqrt 2.
const WEIGHT: [[(f64, u64); 7]; 2] = [
    [
        (4231.432108703771, 0x3fe0f396f47df4d7),
        (4141.432108703771, 0x40153139048e4b31),
        (4053.4321087037706, 0x4023edf7af74e48c),
        (3941.4321087037706, 0x40313d9bdd54643c),
        (3541.4321087037706, 0x4045ca3991ec6089),
        (2997.758007435433, 0x40545ffd7b5346a0),
        (3001.9738331315402, 0x40544c2c636a50ee),
    ],
    [
        (1135.8045294406045, 0x3fe0f3b2569c82e6),
        (1045.8045294406045, 0x40153fa0062df605),
        (957.8045294406045, 0x402421db8bc195ba),
        (845.8045294406045, 0x4031c9e690219d75),
        (445.8045294406045, 0x4044876ff096ca88),
        (957.2101009194489, 0x4024363dfcb5685a),
        (813.0345798064589, 0x403429b9a6f5cb34),
    ],
];

#[test]
fn terra_and_luna_pull_and_weigh_as_before() {
    let w = World::with_grid(6144, 1664, 1, None, None, 0.0, 1);
    let mut same = [0; 2];
    for i in 0..2 {
        let a = &w.bodies[i].interior;
        for (r, g) in PULL[i].map(|(r, g)| (r, f64::from_bits(g))) {
            let got = a.gravity(r);
            assert!((got - g).abs() <= 1e-12 * g, "body {i} at {r} m: {got} m/s^2, {g} before");
            same[i] += (got.to_bits() == g.to_bits()) as usize;
        }
        let sea = w.bodies[i].radius - SEA_DEPTH;
        for (r, p) in WEIGHT[i] {
            let got = w.bodies[i].rock_weight(sea, r);
            assert_eq!(got.to_bits(), p, "body {i} at {r} m: {got} MPa, {} before", f64::from_bits(p));
        }
    }
    // Terra's pull to the bit: recordings made earlier replay through it; Luna's comes from her mass since ADR 0162
    assert_eq!(same[0], 12, "Terra's pull changed in its last bits: old recordings would drift on replay");
    // the grid and the metal sea as before
    let t = &w.bodies[0];
    assert_eq!((t.mantle, t.core, t.r0, w.bodies[1].r0), (1280, Some(0.0), 2997.758007435433, 957.2101009194489));
    assert_eq!(t.interior.surface_g, crate::interior::G, "G on Terra to the bit");
}

/// The world answers each body's field from its interior: Terra's is the unit, Luna's the interior's small one.
#[test]
fn the_worlds_field_is_the_interiors() {
    let w = World::with_grid(6144, 1664, 1, None, None, 0.0, 1);
    assert_eq!(w.field(0), 1.0, "Terra is the unit");
    assert_eq!(w.field(1), w.bodies[1].interior.field, "Luna's is her interior's");
    assert!((w.field(1) - 0.0119).abs() < 5e-4, "Luna {}", w.field(1));
}

/// Today's surface pull was G times the radius over Terra's; now it is the mass over the square of the radius.
#[test]
fn surface_gravity_is_the_mass_over_the_radius_squared() {
    for n in [768, 6144, 12288] {
        let w = World::with_grid(n, crate::bodies::columns(1, n), 1, None, None, 0.0, 1);
        let [t, l] = [&w.bodies[0].interior, &w.bodies[1].interior];
        let before = [crate::interior::G, crate::interior::G * w.bodies[1].radius / w.bodies[0].radius];
        for (a, want) in [t, l].into_iter().zip(before) {
            let from_mass = a.k * a.mass / (a.radius * a.radius);
            assert!((from_mass - want).abs() <= 1e-12 * want && (a.surface_g - want).abs() <= 1e-12 * want, "planet {n}: {from_mass} {} {want}", a.surface_g);
            assert_eq!(a.k, t.k, "one constant a world");
        }
    }
}
