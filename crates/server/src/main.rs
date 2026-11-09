//! The dedicated server: `technoape-server listen=0.0.0.0:7777 planet=3072 moon=512 seed=1337`.
//! Prints one JSON line every 5 s: tick, players, edits.
use engine::net::{tcp, DEFAULT_PORT, TICK_HZ};
use server::Server;
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

fn main() {
    let args: HashMap<String, String> = std::env::args().skip(1).filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    let num = |k: &str, d: i32| args.get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let listen = args.get("listen").cloned().unwrap_or_else(|| format!("0.0.0.0:{DEFAULT_PORT}"));
    let (tx, rx) = channel();
    let mut server = Server::new(num("planet", 3072), num("moon", 512), num("seed", 1337), rx);
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
