//! `stop` on the server's stdin (ADR 0130): it saves and ends by itself, so a restart loses nothing.
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The server never outlives its test, whatever fails.
struct Kill(std::process::Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_stop_line_saves_and_ends_the_server() {
    let save = std::env::temp_dir().join(format!("technoape-stop-{}.sav", std::process::id()));
    let _ = std::fs::remove_file(&save);
    let mut child = Kill(Command::new(env!("CARGO_BIN_EXE_technoape-server"))
        .args(["planet=128", "seed=1", "listen=127.0.0.1:0", &format!("save={}", save.display())])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap());
    // it is up once it listens (the bake comes first); the save is not due for 30 s
    let err = BufReader::new(child.0.stderr.take().unwrap()).lines();
    let (up, is_up) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // reads on after, so its stderr stays drained
        for l in err.map_while(Result::ok) {
            if l.contains("listening on") {
                let _ = up.send(());
            }
        }
    });
    is_up.recv_timeout(Duration::from_secs(300)).expect("the server never listened");
    let mut stdin = child.0.stdin.take().unwrap();
    stdin.write_all(b"nonsense\nstop\n").unwrap(); // still open after: an end of input is not what stops it
    let t = Instant::now();
    let status = loop {
        if let Some(s) = child.0.try_wait().unwrap() {
            break s;
        }
        if t.elapsed() > Duration::from_secs(20) {
            panic!("the server did not stop on `stop`");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "{status}");
    assert!(save.exists(), "it stopped without saving");
    drop(stdin);
    let _ = std::fs::remove_file(&save);
    let _ = std::fs::remove_file(save.with_extension("herds"));
}
