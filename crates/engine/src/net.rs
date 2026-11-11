//! The player protocol (ADR 0006). A client, a bot and the server all speak it; the transport is a pair of
//! channels, fed either directly (the local server in the same process) or by a TCP connection.
use crate::edit::Shape;
use crate::phys::Input;
use crate::world::Block;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

pub const DEFAULT_PORT: u16 = 7777;
pub const TICK_HZ: f64 = 20.0;
/// How far from the player an edit may be; the server drops anything farther.
pub const REACH: f64 = 16.0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ToServer {
    Hello { name: String },
    /// Fly or teleport: the position is taken as sent (creative mode; no check yet).
    Move { pos: [f64; 3], fwd: [f32; 3], pitch: f32 },
    /// Walk: the server moves the player with `phys::step` from this input every tick until a `Move`.
    Input(Input),
    /// Walk, predicted (ADR 0019): one input per physics step, numbered from `first`. The server takes each
    /// exactly once, in order, at most as fast as real time, and answers `You` with the last one it took.
    Steps { first: u32, inputs: Vec<Input> },
    /// Dig (`material` 0) or place a shape. The server applies all of it or none and answers `Done` with the
    /// same `seq`, after the `Edits` that carry what it changed.
    Edit { seq: u32, shape: Shape, material: u8 },
    /// Survival or creative (counts nothing). shortcut: anyone may switch; an operator role when servers go public.
    Creative(bool),
    /// The bucket (ADR 0020): `fill` scoops up to a block of water from the gap that holds block `at`, else
    /// the bucket is poured out into it. Answered by `Done` with the same `seq`, like an edit.
    Bucket { seq: u32, at: Block, fill: bool },
    /// A chat line, or a command when it starts with '/' (ADR 0023): the server answers a command to this
    /// player alone and passes any other line to everyone, with the sender's name.
    Chat(String),
    /// A dead player back at the spawn with full health and hunger (ADR 0040); nothing while alive.
    Respawn,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ToClient {
    /// First message: the world is rebuilt from these numbers, then `Edits` brings what changed. `blocks` is
    /// the block registry's hash (`BLOCKS.hash`): a client from a build with other blocks refuses to play.
    Welcome { id: u32, seed: i32, planet_n: i32, moon_n: i32, blocks: u64 },
    Edits(Vec<(Block, u8)>),
    Players(Vec<Player>),
    /// The answer to the `Edit` with this `seq`: applied, or refused whole (reach, size, inventory).
    Done { seq: u32, ok: bool },
    /// This player's whole physics state after the step numbered `step` of its `Steps` (0: none yet), every
    /// tick, and the world's time that poses the sky (ADR 0022). The client replays its later inputs from here
    /// (ADR 0019).
    You { step: u32, me: crate::phys::Player, time: f64, warp: f64 },
    /// This player's mode, every block kind it holds a nonzero count of, and the water in its bucket (units,
    /// `liquid::U` a block); sent on join and on change.
    Inventory { creative: bool, counts: Vec<(u8, u32)>, water: u32 },
    /// Tiles of water (ADR 0020), each whole: the tile (`Col::tile`) and its stored columns, (floor, ceil,
    /// units) per gap; a column not listed is as generation gives it. A tile is sent when it changed by more
    /// than the dead band, or settled, since this client last had it: nearest first, within reach and a byte
    /// budget per tick.
    Water(Vec<(crate::liquid::Col, Vec<crate::liquid::ColWater>)>),
    /// A line for the chat (ADR 0023): a player's words, an answer to a command, someone joining or leaving.
    Chat(String),
    /// This player's health and hunger, 0 to `vitals::MAX` (ADR 0040), and how it died while it is dead;
    /// sent on join and when what the HUD shows changes.
    Vitals { health: u8, hunger: u8, dead: Option<String> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Player {
    pub id: u32,
    pub name: String,
    /// The feet (bottom centre of the player's box).
    pub pos: [f64; 3],
    pub fwd: [f32; 3],
    pub pitch: f32,
}

/// One end of a connection: `O` goes out, `I` comes in.
pub struct Link<O, I> {
    pub tx: Sender<O>,
    pub rx: Receiver<I>,
}

pub enum Recv<I> {
    Msg(I),
    Empty,
    Closed,
}

impl<O, I> Link<O, I> {
    /// Sending to a closed peer is not an error here; the next `try_recv` reports the close.
    pub fn send(&self, m: O) {
        let _ = self.tx.send(m);
    }
    pub fn try_recv(&self) -> Recv<I> {
        match self.rx.try_recv() {
            Ok(m) => Recv::Msg(m),
            Err(TryRecvError::Empty) => Recv::Empty,
            Err(TryRecvError::Disconnected) => Recv::Closed,
        }
    }
}

/// Two ends joined in memory: the local server's connection.
pub fn local<A, B>() -> (Link<A, B>, Link<B, A>) {
    let (a_tx, a_rx) = channel();
    let (b_tx, b_rx) = channel();
    (Link { tx: a_tx, rx: b_rx }, Link { tx: b_tx, rx: a_rx })
}

/// A TCP connection as a link: one thread writes outgoing messages, one reads incoming ones, as frames of a
/// little-endian u32 length and postcard bytes. Either thread ending closes the link.
pub fn tcp<O, I>(stream: TcpStream) -> std::io::Result<Link<O, I>>
where
    O: Serialize + Send + 'static,
    I: DeserializeOwned + Send + 'static,
{
    stream.set_nodelay(true)?;
    let mut w = stream.try_clone()?;
    let mut r = stream;
    let (out_tx, out_rx) = channel::<O>();
    let (in_tx, in_rx) = channel::<I>();
    std::thread::spawn(move || {
        for m in out_rx {
            let bytes = match postcard::to_stdvec(&m) {
                Ok(b) => b,
                Err(_) => break,
            };
            if w.write_all(&(bytes.len() as u32).to_le_bytes()).and_then(|_| w.write_all(&bytes)).is_err() {
                break;
            }
        }
        let _ = w.shutdown(std::net::Shutdown::Both);
    });
    std::thread::spawn(move || {
        let mut len = [0u8; 4];
        let mut buf = Vec::new();
        while r.read_exact(&mut len).is_ok() {
            let n = u32::from_le_bytes(len) as usize;
            if n > 1 << 24 {
                break; // nothing legitimate is 16 MB; a bad peer gets dropped
            }
            buf.resize(n, 0);
            if r.read_exact(&mut buf).is_err() {
                break;
            }
            match postcard::from_bytes::<I>(&buf) {
                Ok(m) => {
                    if in_tx.send(m).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = r.shutdown(std::net::Shutdown::Both);
    });
    Ok(Link { tx: out_tx, rx: in_rx })
}

/// The command line's completions (ADR 0023): where the word being typed starts in `line`, and what could
/// stand there. The server's `command` runs the commands; `commands_complete_to_known_words` there checks
/// that this list names only what it knows.
pub fn complete(line: &str) -> (usize, Vec<String>) {
    let Some(rest) = line.strip_prefix('/') else { return (line.len(), Vec::new()) };
    let words: Vec<String> = rest.split(' ').filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
    let typed = if rest.ends_with(' ') || rest.is_empty() { 0 } else { 1 };
    let before = &words[..words.len() - typed];
    // a block's name has spaces: after /give and /place everything typed is one name
    let (at, options): (usize, Vec<String>) = match before.first().map(String::as_str) {
        Some("give" | "place") => {
            // the command is a whole word, so a space follows it
            let at = line.len() - rest[rest.find(' ').unwrap_or(0)..].trim_start().len();
            (at, crate::blocks::BLOCKS.placeable().map(|d| d.name.clone()).collect())
        }
        first => {
            // /goto takes every body's name and words (data/bodies.toml), and the sun
            let names: Vec<String> = crate::bodies::BODIES.iter().flat_map(|r| std::iter::once(r.name.to_lowercase()).chain(r.words.clone())).collect();
            let places: Vec<&str> = names.iter().map(String::as_str).chain(["sun"]).collect();
            let words: &[&str] = match (first, before.get(1).map(String::as_str), before.len()) {
                (None, _, _) => &["time", "playmode", "give", "place", "goto", "timewarp", "heal", "feed", "hurt", "help"],
                (Some("timewarp" | "warp"), None, 1) => &["1", "10", "100", "1000", "10000", "up", "down"],
                (Some("time"), None, 1) => &["set", "add"],
                (Some("time"), Some("set"), 2) => &["sunrise", "day", "noon", "sunset", "night", "midnight"],
                (Some("playmode" | "gamemode"), None, 1) => &["creative", "survival"],
                (Some("goto"), None, 1) => &places,
                _ => &[],
            };
            (line.len() - rest.rsplit(' ').next().unwrap_or("").len(), words.iter().map(|w| w.to_string()).collect())
        }
    };
    let part = line[at..].to_lowercase();
    // a word typed out in full needs no list
    (at, options.into_iter().filter(|o| o.to_lowercase().starts_with(&part) && o.to_lowercase() != part).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    #[test]
    fn tcp_round_trip() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let client: Link<ToServer, ToClient> = tcp(TcpStream::connect(addr).unwrap()).unwrap();
        let server: Link<ToClient, ToServer> = tcp(l.accept().unwrap().0).unwrap();
        let e = ToServer::Edit { seq: 4, shape: Shape::one(Block { body: 0, face: 3, pos: IVec3::new(5, -7, 9) }), material: 2 };
        client.send(ToServer::Hello { name: "bot".into() });
        client.send(e.clone());
        assert_eq!(server.rx.recv().unwrap(), ToServer::Hello { name: "bot".into() });
        assert_eq!(server.rx.recv().unwrap(), e);
        server.send(ToClient::Welcome { id: 1, seed: 7, planet_n: 1536, moon_n: 512, blocks: 9 });
        assert_eq!(client.rx.recv().unwrap(), ToClient::Welcome { id: 1, seed: 7, planet_n: 1536, moon_n: 512, blocks: 9 });
        drop(server);
        assert!(client.rx.recv().is_err(), "a dropped peer closes the link");
    }

    #[test]
    fn commands_complete() {
        let c = |l: &str| complete(l);
        assert_eq!(c("/"), (1, ["time", "playmode", "give", "place", "goto", "timewarp", "heal", "feed", "hurt", "help"].map(String::from).to_vec()));
        assert_eq!(c("/h"), (1, ["heal", "hurt", "help"].map(String::from).to_vec()));
        assert_eq!(c("/g"), (1, vec!["give".to_string(), "goto".to_string()]));
        assert_eq!(c("/time "), (6, vec!["set".to_string(), "add".to_string()]));
        assert_eq!(c("/time set N"), (10, vec!["noon".to_string(), "night".to_string()]));
        assert_eq!(c("/PlayMode c"), (10, vec!["creative".to_string()]));
        assert_eq!(c("/goto luna "), (11, vec![]));
        let s = |v: &[&str]| v.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        assert_eq!(c("/goto "), (6, s(&["terra", "home", "spawn", "luna", "sun"])));
        assert_eq!(c("/goto L"), (6, s(&["luna"])));
        assert_eq!(c("/goto t"), (6, s(&["terra"])));
        assert_eq!(c("hello"), (5, vec![]));
        // a block's name is one word for this, spaces and all
        let (at, v) = c("/give blue l");
        assert_eq!((at, v), (6, vec!["blue lamp".to_string()]));
        assert_eq!(c("/give stone 5").1, Vec::<String>::new());
        assert_eq!(c("/place blue l"), (7, vec!["blue lamp".to_string()]));
        assert_eq!(c("/p"), (1, vec!["playmode".to_string(), "place".to_string()]));
        // the completion put in place completes nothing more
        let (at, v) = c("/time set no");
        assert_eq!(format!("{}{}", &"/time set no"[..at], v[0]), "/time set noon");
    }
}
