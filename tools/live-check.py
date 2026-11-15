"""Live definitions end to end (ADR 0134): changes `data/blocks.toml` under a running game (the stone's colour, the
lamp's light) and says how long the server took to put it in force and the client to make its looks again.

    python -I tools/live-check.py [exe]        -> one JSON line; exit 1 when the game did not take it

Run from the repository's root. The file is put back as it was, also when the run fails.
"""
import json, os, subprocess, sys, threading, time

exe = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else "target/play/technoape.exe")
path = "data/blocks.toml"
was = open(path, encoding="utf-8", newline="").read()
new = was.replace("colour = [0.50, 0.50, 0.52]\npattern = \"stone\"", "colour = [0.95, 0.10, 0.10]\npattern = \"stone\"").replace("colour = [0.50, 0.50, 0.52]\r\npattern = \"stone\"", "colour = [0.95, 0.10, 0.10]\r\npattern = \"stone\"").replace("light = [1.0, 0.72, 0.42]", "light = [0.15, 0.3, 1.0]")
assert new != was and new.count("0.95, 0.10, 0.10") == 1, "the stone row or the lamp row is not as this check expects"

args = [exe, "run=play", "planet=768", "seed=1", "save=0", "mode=creative", "seconds=18", "acts=1:say:/place lamp"]
p = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, errors="replace")
log, t0 = [], time.time()


def read():
    for line in p.stderr:
        log.append((time.time() - t0, line.rstrip()))


outs = []
threading.Thread(target=read, daemon=True).start()
threading.Thread(target=lambda: outs.extend(p.stdout), daemon=True).start()
try:
    # the frame loop is up when the textures' watcher reports the first time
    while p.poll() is None and not any("textures:" in l for _, l in log):
        time.sleep(0.1)
    up = time.time() - t0
    time.sleep(4.0)
    open(path, "w", encoding="utf-8", newline="").write(new)
    wrote = time.time() - t0
    p.wait(timeout=240)
    time.sleep(0.3)
    out = "".join(outs)
finally:
    open(path, "w", encoding="utf-8", newline="").write(was)
    if p.poll() is None:
        p.kill()
took = next((t for t, l in log if t > wrote and "blocks and items reloaded" in l), None)
looks = next((t for t, l in log if t > wrote and "textures:" in l), None)
refused = [l for _, l in log if "refused" in l]
last = (out.strip().splitlines() or ["{}"])[-1]
if os.environ.get("LIVE_LOG"):
    for t, l in log:
        print(f"{t:6.2f} {l}", file=sys.stderr)
print(json.dumps({"frames_up_s": round(up, 1), "file_written_s": round(wrote, 1), "server_took_it_after_s": took and round(took - wrote, 2),
                  "looks_made_again_after_s": looks and round(looks - wrote, 2), "refused": refused, "exit": p.returncode,
                  "run": json.loads(last).get("run") if last.startswith("{") else last[:200]}))
sys.exit(0 if took is not None and looks is not None and not refused and p.returncode == 0 else 1)
