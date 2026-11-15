//! Tables of definitions that a running game may swap (ADR 0134): the block and item rows are read through a
//! `Live`, so a changed data file is in force everywhere at the next read, with no restart and no lock on the
//! reader's side.
//!
//! `apply` is the one way in: it parses both files, refuses a bad or shrinking one by name and leaves the tables
//! as they were, and counts a `rev` the client's streamer and the server's world watch for what they derived from
//! the old rows (meshes, the GPU's look table, the lights and models of placed blocks).
use std::ops::Deref;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::Mutex;

/// A value made on first use and replaced whole by `set`; a reader gets the one in force when it asks and may
/// keep that reference for good.
/// shortcut: a replaced value is never freed (readers may still hold it): some 100 kB a reload of the blocks,
/// a developer's tool; count the readers out and free it if reloads ever come by the thousand.
pub struct Live<T: 'static> {
    cur: AtomicPtr<T>,
    init: fn() -> T,
}

impl<T> Live<T> {
    pub const fn new(init: fn() -> T) -> Self {
        Live { cur: AtomicPtr::new(std::ptr::null_mut()), init }
    }
    pub fn now(&self) -> &'static T {
        let p = self.cur.load(Ordering::Acquire);
        if !p.is_null() {
            // SAFETY: every pointer stored here comes from `Box::leak` and is never freed
            return unsafe { &*p };
        }
        let made = Box::leak(Box::new((self.init)())) as *mut T;
        // two first readers: the loser's value is leaked and the winner's is the one in force
        let p = self.cur.compare_exchange(std::ptr::null_mut(), made, Ordering::AcqRel, Ordering::Acquire).unwrap_or_else(|won| won);
        let p = if p.is_null() { made } else { p };
        // SAFETY: as above
        unsafe { &*p }
    }
    /// Made again by its own maker: for a value derived from another `Live` that changed.
    pub fn again(&self) {
        self.set((self.init)());
    }
    pub fn set(&self, v: T) {
        self.cur.store(Box::leak(Box::new(v)), Ordering::Release);
    }
}

impl<T> Deref for Live<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.now()
    }
}

/// The files' text now in force (the compiled-in ones until a reload), for a client that joins later.
static TEXTS: Mutex<Option<(String, String)>> = Mutex::new(None);
static REV: AtomicU64 = AtomicU64::new(0);

pub const BLOCKS_TOML: &str = include_str!("../../../data/blocks.toml");
pub const ITEMS_TOML: &str = include_str!("../../../data/items.toml");

/// How many reloads changed the rows: whoever derived something from them keeps the number it derived at.
pub fn rev() -> u64 {
    REV.load(Ordering::Acquire)
}

/// The block and item files' text in force, if it is not the compiled-in one.
pub fn reloaded() -> Option<(String, String)> {
    TEXTS.lock().unwrap().clone()
}

/// Puts the two files' text in force: Ok(true) when the rows changed, Ok(false) when they are what was in
/// force already. An error names the file and the row and changes nothing: a file that does not parse, or one
/// that drops, renames or moves a block or an item the world may hold (rows are added and changed in place
/// while a world is up).
pub fn apply(blocks: &str, items: &str) -> Result<bool, String> {
    let mut texts = TEXTS.lock().unwrap();
    let (was_b, was_i) = texts.as_ref().map_or((BLOCKS_TOML, ITEMS_TOML), |t| (t.0.as_str(), t.1.as_str()));
    let same = |a: &str, b: &str| a.replace("\r\n", "\n") == b.replace("\r\n", "\n");
    if same(was_b, blocks) && same(was_i, items) {
        return Ok(false);
    }
    let new = crate::blocks::Registry::parse(blocks).map_err(|e| format!("data/blocks.toml: {e}"))?;
    let old = crate::blocks::BLOCKS.now();
    // a world holds a block by its id and the code asks for some by name: a row keeps both
    if let Some(d) = old.placeable().find(|d| new.get(d.id).is_none_or(|n| n.name != d.name)) {
        return Err(format!("data/blocks.toml: block {} ('{}') is gone or renamed: a running world may hold it", d.id, d.name));
    }
    let old_items = crate::items::ITEMS.now();
    // the items are parsed against the new blocks (a block is an item too), so the blocks go in first; a bad
    // items file puts the old blocks back
    crate::blocks::BLOCKS.set(new);
    let undo = |e: String| {
        crate::blocks::BLOCKS.set(crate::blocks::Registry::parse(was_b).expect("the blocks in force parsed before"));
        e
    };
    let new_items = crate::items::Items::parse(items).map_err(|e| undo(format!("data/items.toml: {e}")))?;
    // (a table of items reads the blocks in force for its block rows, so only its own rows are compared)
    // an item's id is its row's place after the blocks: a row taken out or renamed would turn what the slots
    // hold into its neighbour, so rows are only changed in place and added at the end
    let blocks_now = crate::blocks::BLOCKS.now();
    if let Some(id) = old_items.all().filter(|&id| old.get(id).is_none() && blocks_now.get(id).is_none()).find(|&id| !new_items.all().any(|n| n == id) || new_items.name(id) != old_items.name(id)) {
        return Err(undo(format!("data/items.toml: item {id} ('{}') is gone or moved: a running world may hold it", old_items.name(id))));
    }
    crate::items::ITEMS.set(new_items);
    crate::mesh::tables_again();
    *texts = Some((blocks.to_string(), items.to_string()));
    REV.fetch_add(1, Ordering::AcqRel);
    Ok(true)
}

/// Reads the two files from `dir` (the game's `data` folder) and applies them; a file that cannot be read
/// counts as refused.
pub fn load(dir: &std::path::Path) -> Result<bool, String> {
    let read = |f: &str| std::fs::read_to_string(dir.join(f)).map_err(|e| format!("{}: {e}", dir.join(f).display()));
    apply(&read("blocks.toml")?, &read("items.toml")?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader gets the value in force and keeps what it took; `set` replaces it for the next read.
    #[test]
    fn a_live_value_is_made_once_and_replaced_whole() {
        static MADE: AtomicU64 = AtomicU64::new(0);
        static L: Live<Vec<u32>> = Live::new(|| {
            MADE.fetch_add(1, Ordering::SeqCst);
            vec![1, 2, 3]
        });
        let first: &Vec<u32> = &L;
        assert_eq!((first.len(), L.len(), MADE.load(Ordering::SeqCst)), (3, 3, 1), "made on first use, once");
        L.set(vec![9]);
        assert_eq!((L[0], L.len()), (9, 1), "the next read is the new one");
        assert_eq!(first, &vec![1, 2, 3], "a reference taken before stays what it was");
    }
}
