"""The workspace's tests, every test binary at once (tools/check.sh): `cargo test --workspace` runs them one after
another, and the engine's, the client's and the Studio's are each most of a minute.

  python -I tools/partest.py [log] [--fast]
      builds the test binaries, runs them side by side, each in its package's folder as cargo does; all output to the
      log (default out/check/test.log), each test with its seconds, and the 15 slowest at its end; exit 1 if a build
      or a binary fails. Doc-tests are not run (there are none). --fast leaves out the tests tools/fast-skip.txt names
      (the quick gate's tier; the full gate runs them all).
"""
import json, os, pathlib, re, subprocess, sys

args = [a for a in sys.argv[1:] if a != "--fast"]
fast = "--fast" in sys.argv[1:]
log = pathlib.Path(args[0] if args else "out/check/test.log")
build = subprocess.run(["cargo", "test", "--workspace", "--no-run", "--message-format=json"], capture_output=True, text=True, encoding="utf-8", errors="replace")
bins = []
for line in build.stdout.splitlines():
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("profile", {}).get("test") and m.get("executable"):
        bins.append((m["executable"], str(pathlib.Path(m["manifest_path"]).parent)))
if build.returncode != 0 or not bins:
    log.write_text(build.stderr, encoding="utf-8")
    sys.exit(1)
skip = []
if fast:
    names = pathlib.Path(__file__).with_name("fast-skip.txt").read_text(encoding="utf-8").splitlines()
    for n in names:
        if n.strip() and not n.startswith("#"):
            skip += ["--skip", n.strip()]
# each test's seconds (libtest's --report-time, unstable: RUSTC_BOOTSTRAP lets the stable runner take it)
env = dict(os.environ, RUSTC_BOOTSTRAP="1")
timed = ["-Z", "unstable-options", "--report-time"]
runs = [(exe, subprocess.Popen([exe, *timed, *skip], cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")) for exe, cwd in bins]
ok, out = True, [build.stderr]
for exe, p in runs:
    text, _ = p.communicate()
    out.append(f"     Running {exe}\n{text}")
    ok &= p.returncode == 0
times = sorted(((float(s), name) for name, s in re.findall(r"^test (\S+) \.\.\. \w+ <([\d.]+)s>", "\n".join(out), re.M)), reverse=True)
out.append("slowest tests:\n" + "\n".join(f"  {s:8.1f} s  {name}" for s, name in times[:15]))
log.write_text("\n".join(out), encoding="utf-8")
sys.exit(0 if ok else 1)
