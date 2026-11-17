//! The event list (journal.md, "What the engine needs"; draft-event-list): what happened in the world in a
//! tick, as small typed records. The server's systems push them in the tick's fixed order, so a replay makes
//! the same ones; each client is sent those it can perceive (`ToClient::Events`, before the tick's edits) and
//! its effects and sounds play from them instead of guessing what happened from the edits. The journal (step 2)
//! will listen to the same list.
use crate::world::Block;
use serde::{Deserialize, Serialize};

/// One thing that happened. On the wire: new kinds go at the end, a kind's fields never change.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Event {
    /// A cell's block changed from `was` to `now`: a player's dig, place or command (`by` its id), or the world
    /// (None: a torch left with nothing to hang on, or put out by water). A loose block's move is a `Fall`.
    Edit { at: Block, was: u16, now: u16, by: Option<u32> },
    /// A loose block moved one cell, falling or sliding (ADR 0079, 0106): `from` is air now, `to` holds `block`.
    Fall { from: Block, to: Block, block: u16 },
    /// The tool `item` in player `by`'s hand wore out (ADR 0110).
    Worn { item: u16, by: u32 },
    /// Player `by` took the pick-up lying in `at`, or cut the plant growing there: `row` of `clutter::CLUTTER`
    /// (ADR 0082, 0147).
    Taken { at: Block, row: u16, by: u32 },
    /// Player `by` crafted `recipe` (`recipes::RECIPES`' name) `n` times (ADR 0140).
    Craft { recipe: String, n: u32, by: u32 },
    /// The fire of the furnace at `at` was lit (`on`) or went out (ADR 0141).
    Fire { at: Block, on: bool },
    /// Player `by` died (ADR 0040).
    Died { by: u32 },
}

impl Event {
    /// Where it happened, a cell; None when it happened to a player (`by`), where that player is.
    pub fn cell(&self) -> Option<Block> {
        match self {
            Event::Edit { at, .. } | Event::Taken { at, .. } | Event::Fire { at, .. } => Some(*at),
            Event::Fall { to, .. } => Some(*to),
            Event::Worn { .. } | Event::Craft { .. } | Event::Died { .. } => None,
        }
    }
    /// The player it happened to or who caused it.
    pub fn by(&self) -> Option<u32> {
        match self {
            Event::Edit { by, .. } => *by,
            Event::Worn { by, .. } | Event::Taken { by, .. } | Event::Craft { by, .. } | Event::Died { by } => Some(*by),
            Event::Fall { .. } | Event::Fire { .. } => None,
        }
    }
}

/// A client is sent the events within this many metres of it (`net::SEE`, as the animals), and every one that
/// happened to itself; beyond it no effect shows and no sound carries (a sound's reach is at most 40 m).
pub const PERCEIVE: f64 = crate::net::SEE;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    /// The wire's bytes of each kind, pinned: a kind added at the end leaves these as they are, a field
    /// changed or a kind moved breaks them (and every client older than the change).
    #[test]
    fn each_kind_keeps_its_bytes_on_the_wire() {
        let at = Block { body: 0, face: 2, pos: IVec3::new(3, -4, 200) };
        let to = Block { pos: IVec3::new(3, -4, 199), ..at };
        let bytes = |e: Event| postcard::to_stdvec(&e).unwrap();
        let cases = [
            (Event::Edit { at, was: 5, now: 0, by: Some(7) }, vec![0, 0, 2, 6, 7, 144, 3, 5, 0, 1, 7]),
            (Event::Edit { at, was: 0, now: 5, by: None }, vec![0, 0, 2, 6, 7, 144, 3, 0, 5, 0]),
            (Event::Fall { from: at, to, block: 8 }, vec![1, 0, 2, 6, 7, 144, 3, 0, 2, 6, 7, 142, 3, 8]),
            (Event::Worn { item: 300, by: 7 }, vec![2, 172, 2, 7]),
            (Event::Taken { at, row: 12, by: 7 }, vec![3, 0, 2, 6, 7, 144, 3, 12, 7]),
            (Event::Craft { recipe: "cord".into(), n: 2, by: 7 }, vec![4, 4, b'c', b'o', b'r', b'd', 2, 7]),
            (Event::Fire { at, on: true }, vec![5, 0, 2, 6, 7, 144, 3, 1]),
            (Event::Died { by: 7 }, vec![6, 7]),
        ];
        for (e, want) in cases {
            assert_eq!(bytes(e.clone()), want, "{e:?}");
            assert_eq!(postcard::from_bytes::<Event>(&want).unwrap(), e);
        }
        // the message is ToClient's last (21), after Fires: the 21 before it keep their bytes
        let m = crate::net::ToClient::Events(vec![Event::Died { by: 7 }]);
        assert_eq!(postcard::to_stdvec(&m).unwrap(), vec![21, 1, 6, 7]);
        assert_eq!(postcard::to_stdvec(&crate::net::ToClient::Fires(vec![])).unwrap(), vec![20, 0]);
    }
}
