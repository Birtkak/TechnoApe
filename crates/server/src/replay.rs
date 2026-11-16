//! Reading and replaying a flight recording (ADR 0011).
use super::*;

/// What a replay found.
pub struct Replayed {
    pub server: Server,
    pub checks: u64,
    /// The first checkpoint that disagreed: (tick, recorded hash, replayed hash).
    pub diverged: Option<(u64, u64, u64)>,
}

/// Reads a recording.
pub fn read_recording(path: &Path) -> std::io::Result<Vec<Ev>> {
    let bytes = std::fs::read(path)?;
    let (mut evs, mut at) = (Vec::new(), 0);
    // a crash can cut the last record short; everything before it stands
    while at + 4 <= bytes.len() {
        let n = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let Some(b) = bytes.get(at + 4..at + 4 + n) else { break };
        evs.push(postcard::from_bytes(b).map_err(std::io::Error::other)?);
        at += 4 + n;
    }
    Ok(evs)
}

/// Runs a recording again headless, to its end or to tick `until`, stopping at the first checkpoint that
/// disagrees.
pub fn replay(path: &Path, until: Option<u64>) -> std::io::Result<Replayed> {
    let evs = read_recording(path)?;
    let Some(Ev::Start { planet_n, moon_n, seed, dt, map }) = evs.first() else {
        return Err(std::io::Error::other("not a recording: no Start"));
    };
    let (tx, rx) = channel();
    let (mut s, rest) = match evs.get(1) {
        // (a recording carries no version of its own; one that starts from an older save is from before
        // 1 m blocks, ADR 0033. shortcut: one that starts from the seed alone is not told apart)
        Some(Ev::Loaded(save)) if save.version < 4 => return Err(std::io::Error::other("a recording of an older world (0.5 m blocks, ADR 0033)")),
        Some(Ev::Loaded(save)) => {
            let mut s = Server::from_save(save.clone(), rx);
            match evs.get(2) {
                Some(Ev::Herds(b)) => {
                    s.herds.load(b).map_err(std::io::Error::other)?;
                    (s, &evs[3..])
                }
                _ => (s, &evs[2..]),
            }
        }
        // shortcut: Start names Terra's map alone, so Luna is this build's bake whenever Terra is baked; a
        // recording made before W6 or with another Luna version replays with other ground there (ADR 0070)
        _ => (Server::on_map(*planet_n, *moon_n, *seed, *map, map.map(|_| engine::bake::luna::get(*seed, *moon_n).hash), rx), &evs[1..]),
    };
    s.dt = *dt;
    let mut links: HashMap<u32, Link<ToServer, ToClient>> = HashMap::new();
    let until = until.unwrap_or(u64::MAX);
    let mut r = Replayed { server: s, checks: 0, diverged: None };
    let run_to = |s: &mut Server, links: &HashMap<u32, Link<ToServer, ToClient>>, t: u64| {
        while s.tick < t.min(until) {
            s.tick();
            for l in links.values() {
                while let Recv::Msg(_) = l.try_recv() {}
            }
        }
    };
    for ev in rest {
        let t = match ev {
            Ev::Join { tick, .. } | Ev::Msg { tick, .. } | Ev::Leave { tick, .. } | Ev::Check { tick, .. } => *tick,
            Ev::Start { .. } | Ev::Loaded(_) | Ev::Herds(_) => return Err(std::io::Error::other("a Start, Loaded or Herds past the start")),
        };
        run_to(&mut r.server, &links, t);
        if r.server.tick >= until {
            break;
        }
        match ev {
            Ev::Join { id, .. } => {
                let (c, sv) = local();
                tx.send(sv).unwrap();
                links.insert(*id, c);
            }
            Ev::Msg { id, m, .. } => {
                if let Some(l) = links.get(id) {
                    l.send(m.clone());
                }
            }
            Ev::Leave { id, .. } => {
                links.remove(id);
            }
            Ev::Check { tick, hash } => {
                r.checks += 1;
                let got = r.server.hash();
                if got != *hash {
                    r.diverged = Some((*tick, *hash, got));
                    break;
                }
            }
            Ev::Start { .. } | Ev::Loaded(_) | Ev::Herds(_) => unreachable!(),
        }
    }
    if until != u64::MAX && r.diverged.is_none() {
        run_to(&mut r.server, &links, until);
    }
    Ok(r)
}
