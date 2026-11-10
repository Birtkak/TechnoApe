//! The dedicated server: `technoape-server listen=0.0.0.0:7777 planet=3072 moon=512 seed=1337`.
//! Prints one JSON line every 5 s: tick, players, edits. It records the session to `out/rec/server-<unix s>.rec`
//! (`record=file` elsewhere, `record=0` not at all).
//!
//! Test harness (ADR 0011), each printing one JSON line and exiting 1 on a failure:
//! - `soak=1 bots=50 minutes=10 seed=1 planet=256` fast-forwards seeded random bots and checks invariants every
//!   tick; `only=3,7` runs just those bots, `ticks=` overrides `minutes=`, `record=` keeps the run's recording.
//!   A failure is shrunk to the fewest bots, recorded to `out/soak-fail.rec` and printed with its repro line.
//! - `replay=file until=tick` runs a recording again and reports the first checkpoint that disagrees.
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
    if args.contains_key("soak") {
        return run_soak(&args);
    }
    if let Some(path) = args.get("replay") {
        return run_replay(path, args.get("until").and_then(|v| v.parse().ok()));
    }
    let listen = args.get("listen").cloned().unwrap_or_else(|| format!("0.0.0.0:{DEFAULT_PORT}"));
    let (tx, rx) = channel();
    let mut server = Server::new(num("planet", 3072), num("moon", 512), num("seed", 1337), rx);
    let unix = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let rec = args.get("record").cloned().unwrap_or_else(|| format!("out/rec/server-{unix}.rec"));
    if rec != "0" {
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
        server.tick();
        if report.elapsed() >= Duration::from_secs(5) {
            report = Instant::now();
            println!("{{\"tick\":{},\"players\":{},\"edits\":{}}}", server.tick, server.players(), server.world.edit_count);
        }
        next += step;
        match next.checked_duration_since(Instant::now()) {
            Some(d) => std::thread::sleep(d),
            None => next = Instant::now(),
        }
    }
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
    let t0 = Instant::now();
    match soak::run(seed, &bots, ticks, planet, record) {
        Ok(hash) => println!(
            "{{\"soak\":\"ok\",\"seed\":{seed},\"bots\":{},\"ticks\":{ticks},\"sim_minutes\":{:.1},\"secs\":{:.2},\"hash\":\"{hash:016x}\"}}",
            bots.len(),
            ticks as f64 / TICK_HZ / 60.0,
            t0.elapsed().as_secs_f64()
        ),
        Err(f) => {
            let (keep, f) = soak::shrink(seed, bots, f, planet);
            let rec = "out/soak-fail.rec";
            let _ = soak::run(seed, &keep, f.tick + 1, planet, Some(Path::new(rec)));
            let only: Vec<String> = keep.iter().map(|b| b.to_string()).collect();
            println!(
                "{{\"soak\":\"fail\",\"seed\":{seed},\"tick\":{},\"what\":{:?},\"repro\":\"technoape-server soak=1 seed={seed} planet={planet} only={} ticks={}\",\"replay\":\"{rec}\",\"secs\":{:.2}}}",
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
        "{{\"replay\":{path:?},\"ticks\":{},\"checks\":{},\"diverged\":{diverged},\"hash\":\"{:016x}\",\"players\":{},\"edits\":{},\"positions\":{}}}",
        s.tick,
        r.checks,
        s.hash(),
        s.players(),
        s.world.edit_count,
        s.positions()
    );
    if r.diverged.is_some() {
        std::process::exit(1);
    }
}
