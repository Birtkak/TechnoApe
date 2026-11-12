//! The variations wall without a window (docs/design/studio-tools.md "The variations wall"): a grid of
//! candidates made around the current thing by a generator and a spread, "more like this" and "back". The same
//! seed makes the same wall, so a shot and a test see what the developer saw.

/// A small seeded random source (splitmix64): no dependency, the same numbers on every machine.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// 0..1
    pub fn f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// -1..1
    pub fn signed(&mut self) -> f32 {
        self.f32() * 2.0 - 1.0
    }
    /// 0..n
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Makes one candidate around `around`, `spread` (0..1) saying how far it may go.
pub type Gen<'a, T> = &'a dyn Fn(&T, &mut Rng, f32) -> T;

/// One wall: the first item is what it was made around (taking it changes nothing), the others candidates.
pub struct Wall<T> {
    pub around: T,
    pub items: Vec<T>,
    pub seed: u64,
    pub spread: f32,
    back: Vec<(T, u64, Vec<T>)>,
}

impl<T: Clone> Wall<T> {
    pub fn new(around: T, n: usize, spread: f32, seed: u64, gen: Gen<T>) -> Wall<T> {
        let mut w = Wall { around, items: vec![], seed, spread, back: vec![] };
        w.fill(n, gen);
        w
    }
    /// Makes the wall again from its seed (after the spread moved).
    pub fn fill(&mut self, n: usize, gen: Gen<T>) {
        let mut rng = Rng::new(self.seed);
        self.items = std::iter::once(self.around.clone()).chain((1..n.max(1)).map(|_| gen(&self.around, &mut rng, self.spread))).collect();
    }
    /// "More like this": a new wall around item `i`; the old one is kept for "back".
    pub fn more_like(&mut self, i: usize, gen: Gen<T>) {
        let Some(pick) = self.items.get(i).cloned() else { return };
        let n = self.items.len();
        self.back.push((std::mem::replace(&mut self.around, pick), self.seed, std::mem::take(&mut self.items)));
        self.seed = Rng::new(self.seed).next();
        self.fill(n, gen);
    }
    /// A new wall around the same thing (other candidates).
    pub fn shuffle(&mut self, gen: Gen<T>) {
        let n = self.items.len();
        self.seed = Rng::new(self.seed ^ 0x5bd1_e995).next();
        self.fill(n, gen);
    }
    /// The last wall again; false when there is none.
    pub fn back(&mut self) -> bool {
        let Some((around, seed, items)) = self.back.pop() else { return false };
        (self.around, self.seed, self.items) = (around, seed, items);
        true
    }
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(x: &f32, r: &mut Rng, spread: f32) -> f32 {
        x + r.signed() * spread
    }

    #[test]
    fn the_rng_is_the_same_for_a_seed_and_stays_in_range() {
        let a: Vec<u64> = (0..5).scan(Rng::new(7), |r, _| Some(r.next())).collect();
        let b: Vec<u64> = (0..5).scan(Rng::new(7), |r, _| Some(r.next())).collect();
        assert_eq!(a, b);
        assert_ne!(a, (0..5).scan(Rng::new(8), |r, _| Some(r.next())).collect::<Vec<_>>());
        let mut r = Rng::new(1);
        assert!((0..1000).map(|_| r.f32()).all(|v| (0.0..1.0).contains(&v)));
        assert!((0..1000).map(|_| r.below(3)).all(|v| v < 3));
    }

    #[test]
    fn a_wall_is_the_same_for_a_seed_more_like_this_and_back() {
        let a = Wall::new(1.0f32, 9, 0.5, 42, &gen);
        let b = Wall::new(1.0f32, 9, 0.5, 42, &gen);
        assert_eq!(a.items, b.items, "deterministic for a seed");
        assert_eq!(a.items.len(), 9);
        assert_eq!(a.items[0], 1.0, "the first is what it was made around");
        assert!(a.items[1..].iter().all(|v| (v - 1.0).abs() <= 0.5 && *v != 1.0), "within the spread: {:?}", a.items);
        assert_ne!(Wall::new(1.0f32, 9, 0.5, 43, &gen).items, a.items, "another seed, another wall");
        let mut w = Wall::new(1.0f32, 9, 0.5, 42, &gen);
        let pick = w.items[3];
        w.more_like(3, &gen);
        assert_eq!((w.around, w.items[0], w.items.len()), (pick, pick, 9));
        assert!(w.can_go_back());
        assert!(w.back());
        assert_eq!((w.around, &w.items), (1.0, &a.items), "back gives the last wall exactly");
        assert!(!w.back(), "nothing further back");
        let mut z = Wall::new(1.0f32, 4, 0.0, 1, &gen);
        z.fill(4, &gen);
        assert!(z.items.iter().all(|v| *v == 1.0), "no spread, no change");
    }
}
