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
    /// tick. The client replays its later inputs from here (ADR 0019).
    You { step: u32, me: crate::phys::Player },
    /// This player's mode and every block kind it holds a nonzero count of; sent on join and on change.
    Inventory { creative: bool, counts: Vec<(u8, u32)> },
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
}
