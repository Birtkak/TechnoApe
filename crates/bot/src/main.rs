//! `technoape-bot connect=host[:7777] n=1 name=bot script="wait 1; dig 0 -0.5 0; report"`
//! `script=-` reads commands live from stdin, one per line, until `quit` or EOF. With n>1 every bot runs the
//! same script on its own thread, named `name-i`. Output: one JSON line per report or block, on stdout.
use bot::{parse, parse_script, Bot, Cmd};
use engine::net::{tcp, DEFAULT_PORT};
use std::collections::HashMap;
use std::io::BufRead;
use std::net::TcpStream;
use std::sync::mpsc::channel;

/// mimalloc: the Windows heap is slow under many threads (speed-lever spike, ADR 0012).
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: HashMap<String, String> = std::env::args().skip(1).filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    let addr = args.get("connect").cloned().unwrap_or_else(|| format!("127.0.0.1:{DEFAULT_PORT}"));
    let addr = if addr.contains(':') { addr } else { format!("{addr}:{DEFAULT_PORT}") };
    let n: usize = args.get("n").and_then(|v| v.parse().ok()).unwrap_or(1);
    let name = args.get("name").cloned().unwrap_or_else(|| "bot".into());
    // warp=: the server's block-grid warp (ADR 0131), which no message carries
    let warp: f64 = args.get("warp").map_or(0.0, |v| v.parse().expect("warp= is a number 0 to 1"));
    let script_arg = args.get("script").cloned().unwrap_or_else(|| "report".into());
    let live = script_arg == "-";
    let script = if live {
        Vec::new()
    } else {
        match parse_script(&script_arg) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[technoape-bot] {e}");
                std::process::exit(2);
            }
        }
    };
    let out_lock = std::sync::Arc::new(std::sync::Mutex::new(()));
    let mut live_txs = Vec::new();
    let threads: Vec<_> = (0..n)
        .map(|i| {
            let stream = TcpStream::connect(&addr).unwrap_or_else(|e| {
                eprintln!("[technoape-bot] connect {addr}: {e}");
                std::process::exit(1);
            });
            let link = tcp(stream).expect("link");
            let bot_name = if n == 1 { name.clone() } else { format!("{name}-{i}") };
            let script = script.clone();
            let rx = live.then(|| {
                let (tx, rx) = channel::<Cmd>();
                live_txs.push(tx);
                rx
            });
            let lock = out_lock.clone();
            std::thread::spawn(move || {
                let mut b = Bot::new(&bot_name, link);
                b.warp = warp;
                b.run(script, rx, |l| {
                    let _g = lock.lock().unwrap();
                    println!("{l}");
                });
            })
        })
        .collect();
    if live {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            match parse(&line) {
                Ok(Some(c)) => live_txs.retain(|tx| tx.send(c.clone()).is_ok()),
                Ok(None) => {}
                Err(e) => eprintln!("[technoape-bot] {e}"),
            }
        }
        drop(live_txs); // EOF: the bots finish what they have and stop
    }
    for t in threads {
        let _ = t.join();
    }
}
