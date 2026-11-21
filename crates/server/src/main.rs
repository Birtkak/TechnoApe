//! The dedicated server: `technoape-server listen=0.0.0.0:7777 planet=6144 seed=1337` (moon= overrides Luna's size, `bodies::columns`).
//! Prints one JSON line every 5 s: tick, players, edits. With `record=1` it records the session to
//! `out/rec/server-<unix s>.rec` (`record=file` elsewhere; by default not at all). It saves every 30 s to `saves/server.sav` (`save=file`,
//! `save=0` not at all) and carries on from that save when it exists. A line `stop` on its stdin saves and ends it.
//!
//! Test harness (ADR 0011), each printing one JSON line and exiting 1 on a failure:
//! - `soak=1 bots=50 minutes=10 seed=1 planet=256` fast-forwards seeded random bots and checks invariants every
//!   tick; `only=3,7` runs just those bots, `ticks=` overrides `minutes=`, `record=` keeps the run's recording.
//!   A failure is shrunk to the fewest bots, recorded to `out/soak-fail.rec` and printed with its repro line.
//! - `report=1 seed=1337 planet=6144` prints what the ground holds (`report.rs`).
//! - `replay=file until=tick` runs a recording again and reports the first checkpoint that disagrees.
//! - `bakemap=data/bodies.toml body=Mars seed=1` bakes a planet's map and writes its pictures.
use engine::net::{tcp, DEFAULT_PORT, TICK_HZ};
use server::{soak, Server};
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// mimalloc: the Windows heap is slow under many threads (speed-lever spike, ADR 0012).
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: HashMap<String, String> = std::env::args().skip(1).filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    let num = |k: &str, d: i32| args.get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    if args.contains_key("report") {
        let planet = num("planet", 6144);
        let t = Instant::now();
        let w = engine::world::World::baked_warp(planet, engine::bodies::moon_columns(planet), num("seed", 1337), 0.0, server::gen_arg(num("mantle", 1), num("gen", -1)));
        eprintln!("[report] world in {:.1} s", t.elapsed().as_secs_f64());
        println!("{}", server::report::report(&w));
        return;
    }
    if let Some(rows) = args.get("bakemap") {
        return bakemap(rows, args.get("body").map_or("Mars", String::as_str), num("seed", 1), args.get("n").and_then(|v| v.parse().ok()), args.get("out"));
    }
    if args.contains_key("soak") {
        return run_soak(&args);
    }
    if let Some(path) = args.get("replay") {
        return run_replay(path, args.get("until").and_then(|v| v.parse().ok()));
    }
    let listen = args.get("listen").cloned().unwrap_or_else(|| format!("0.0.0.0:{DEFAULT_PORT}"));
    // `stop` typed in the terminal, or written by the container's wrapper on docker stop (ADR 0130), saves
    // and ends the server. The end of stdin does not: a server started without one runs on.
    // Read from the start, so a stop during the bake is taken once the world is up.
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stopped = stop.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            if line.trim() == "stop" {
                stopped.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    });
    let (tx, rx) = channel();
    // saves (ADR 0016): save= names the file, carried on from when it exists; save=0 keeps none.
    // shortcut: Ctrl-C loses up to the 30 s since the last save; `stop` on stdin is the way to end it
    let save = args.get("save").cloned().unwrap_or_else(|| "saves/server.sav".into());
    // warp=0..1: the conformal grid's trial (ADR 0131), on a new world only, kept in no save
    let warp: f64 = args.get("warp").map_or(0.0, |v| v.parse().expect("warp= is a number 0 to 1"));
    assert!(warp == 0.0 || save == "0", "warp= is a trial: run it with save=0, the warp is kept in no save");
    let mut server = match Some(Path::new(&save)).filter(|p| save != "0" && p.exists()) {
        Some(p) => {
            let s = server::save::Save::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            eprintln!("[technoape-server] carrying on from {save} ({} edited cells)", s.edits.len());
            let mut s = Server::from_save(s, rx);
            s.read_herds(p);
            s
        }
        None => {
            let planet = num("planet", engine::bodies::BODIES[0].n);
            // a new world of the newest generation, or `gen=` (mantle=0: generation 0, the 192 m floor before ADR 0152)
            Server::baked_warp(planet, num("moon", engine::bodies::moon_columns(planet)), num("seed", 1337), warp, server::gen_arg(num("mantle", 1), num("gen", -1)), rx)
        }
    };
    server.autosave = (save != "0").then(|| (save.clone().into(), 30 * TICK_HZ as u64));
    server.live_data = true;
    let unix = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if let Some(rec) = record_path(args.get("record").map(String::as_str), unix) {
        match server.record(Path::new(&rec)) {
            Ok(()) => eprintln!("[technoape-server] recording to {rec}"),
            Err(e) => eprintln!("[technoape-server] not recording: {e}"),
        }
    }
    let listener = TcpListener::bind(&listen).expect("listen address");
    eprintln!("[technoape-server] listening on {listen}");
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            if let Ok(link) = tcp(s) {
                if tx.send(link).is_err() {
                    break;
                }
            }
        }
    });
    let step = Duration::from_secs_f64(1.0 / TICK_HZ);
    let mut next = Instant::now();
    let mut report = Instant::now();
    loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            server.save_now();
            eprintln!("[technoape-server] stopped");
            return;
        }
        server.tick();
        if report.elapsed() >= Duration::from_secs(5) {
            report = Instant::now();
            let w = &server.world.water;
            println!(
                "{{\"tick\":{},\"players\":{},\"edits\":{},\"water\":{{\"stored\":{},\"awake\":{},\"skipped\":{},\"bytes\":{}}},\"air\":{}}}",
                server.tick,
                server.players(),
                server.world.edit_count,
                w.stored(),
                w.awake(),
                server.water_skipped,
                server.water_bytes,
                // the live air (W3): its clock and the last job's time on its worker
                server.weather.as_ref().map_or("null".into(), |w| format!("{{\"t\":{:.0},\"job_ms\":{:.3}}}", w.view().t, w.view().ms))
            );
        }
        next += step;
        match next.checked_duration_since(Instant::now()) {
            Some(d) => std::thread::sleep(d),
            None => next = Instant::now(),
        }
    }
}

/// `bakemap=<rows file> body=Mars seed=1 [n=] [out=prefix]`: bakes a planet's crust of a row of a bodies file
/// (`data/bodies.toml`, or a draft copy of it), keeps it where the game would find it, writes its pictures
/// (`bake::pictures`) and prints one JSON line of its numbers (planet-model.md step 5a).
fn bakemap(path: &str, body: &str, seed: i32, n: Option<i32>, out: Option<&String>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let rows = engine::bodies::parse(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let row = rows.iter().find(|r| r.named(body)).unwrap_or_else(|| panic!("{path}: no body '{body}'"));
    let bake = row.bake.as_ref().unwrap_or_else(|| panic!("{body} has no stages"));
    let planet = bake.2.as_ref().filter(|p| p.icy.is_none()).unwrap_or_else(|| panic!("bakemap= pictures a planet's crust (stages from 'crust' on a body that circles the sun, not an icy one)"));
    let n = n.unwrap_or(row.n);
    if n <= 0 || n % 32 != 0 {
        eprintln!("bakemap=: n={n} is not a planet's size (whole chunks of 32 columns a face edge)");
        std::process::exit(2);
    }
    let (map, stats) = engine::bake::mars::run(&bake.0, seed, n, planet.age);
    let prefix = out.cloned().unwrap_or_else(|| format!("out/{}-seed-{seed}", body.to_lowercase()));
    let pictures = engine::bake::mars::pictures(&map, &stats, &prefix);
    let line = stats.json(&map);
    engine::bake::mars::keep(&bake.0, seed, n, planet, map);
    println!("{}, \"file\": {:?}, \"pictures\": {pictures:?}}}", &line[..line.len() - 1], bake.file(seed, n).display().to_string());
}

fn run_soak(args: &HashMap<String, String>) {
    let get = |k: &str, d: u64| args.get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let seed = get("seed", 1);
    let planet = get("planet", 256) as i32;
    let ticks = get("ticks", (get("minutes", 10) as f64 * 60.0 * TICK_HZ) as u64);
    let bots: Vec<u32> = match args.get("only") {
        Some(l) => l.split(',').filter_map(|v| v.trim().parse().ok()).collect(),
        None => (0..get("bots", 50) as u32).collect(),
    };
    let record = args.get("record").filter(|r| *r != "0").map(Path::new);
    let warp = get("warp", 1).clamp(1, 10_000); // /timewarp's range: a bigger one would be refused in chat
    let t0 = Instant::now();
    match soak::run_warp(seed, &bots, ticks, planet, record, warp) {
        Ok((hash, w)) => println!(
            "{{\"soak\":\"ok\",\"seed\":{seed},\"bots\":{},\"ticks\":{ticks},\"sim_minutes\":{:.1},\"secs\":{:.2},\"hash\":\"{hash:016x}\",\"water\":{{\"stored_peak\":{},\"awake_peak\":{},\"skipped\":{},\"bytes\":{},\"dams\":{},\"broken\":{}}},\"falls\":{{\"heaps\":{},\"moves\":{}}}}}",
            bots.len(),
            ticks as f64 / TICK_HZ / 60.0,
            t0.elapsed().as_secs_f64(),
            w.stored,
            w.awake,
            w.skipped,
            w.bytes,
            w.dams,
            w.broken,
            w.heaps,
            w.falls
        ),
        Err(f) => {
            // the shrink can take long: the first failure shows at once
            eprintln!("soak failed at tick {} after {:.0} s: {}; shrinking", f.tick, t0.elapsed().as_secs_f64(), f.what);
            let (keep, f) = soak::shrink(seed, bots, f, planet, warp);
            let rec = "out/soak-fail.rec";
            let _ = soak::run_warp(seed, &keep, f.tick + 1, planet, Some(Path::new(rec)), warp);
            let only: Vec<String> = keep.iter().map(|b| b.to_string()).collect();
            println!(
                "{{\"soak\":\"fail\",\"seed\":{seed},\"tick\":{},\"what\":{:?},\"repro\":\"technoape-server soak=1 seed={seed} planet={planet} warp={warp} only={} ticks={}\",\"replay\":\"{rec}\",\"secs\":{:.2}}}",
                f.tick,
                f.what,
                only.join(","),
                f.tick + 1,
                t0.elapsed().as_secs_f64()
            );
            std::process::exit(1);
        }
    }
}

fn run_replay(path: &str, until: Option<u64>) {
    let r = match server::replay(Path::new(path), until) {
        Ok(r) => r,
        Err(e) => {
            println!("{{\"replay\":{path:?},\"error\":{:?}}}", e.to_string());
            std::process::exit(1);
        }
    };
    let s = &r.server;
    let diverged = r.diverged.map_or("null".into(), |(t, want, got)| format!("{{\"tick\":{t},\"recorded\":\"{want:016x}\",\"replayed\":\"{got:016x}\"}}"));
    println!(
        "{{\"replay\":{path:?},\"ticks\":{},\"checks\":{},\"diverged\":{diverged},\"hash\":\"{:016x}\",\"players\":{},\"edits\":{},\"time\":{:.1},\"positions\":{}}}",
        s.tick,
        r.checks,
        s.hash(),
        s.players(),
        s.world.edit_count,
        s.world.time,
        s.positions()
    );
    if r.diverged.is_some() {
        std::process::exit(1);
    }
}

/// Where the dedicated server records: nowhere unless `record=1` (to `out/rec/`) or `record=file`, as nothing
/// cleans `out/rec/` and a long-running server would fill the disk.
fn record_path(arg: Option<&str>, unix: u64) -> Option<String> {
    match arg {
        None | Some("0") => None,
        Some("1") => Some(format!("out/rec/server-{unix}.rec")),
        Some(file) => Some(file.to_string()),
    }
}

#[cfg(test)]
mod tests {
    /// A dedicated server records only when asked: `record=1` to `out/rec/`, `record=file` there.
    #[test]
    fn the_dedicated_server_records_only_when_asked() {
        let r = |a| super::record_path(a, 7);
        assert_eq!([r(None), r(Some("0")), r(Some("1")), r(Some("x.rec"))], [None, None, Some("out/rec/server-7.rec".into()), Some("x.rec".into())]);
    }
}
