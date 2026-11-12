"""The workspace's tests, every test binary at once (tools/check.sh): `cargo test --workspace` runs them one after
another, and the engine's, the client's and the Studio's are each most of a minute.

  python -I tools/partest.py [log]     builds the test binaries, runs them side by side, each in its package's
                                       folder as cargo does; all output to the log (default out/check/test.log);
                                       exit 1 if a build or a binary fails. Doc-tests are not run (there are none).
"""
import json, pathlib, subprocess, sys

log = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "out/check/test.log")
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
runs = [(exe, subprocess.Popen([exe], cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")) for exe, cwd in bins]
ok, out = True, [build.stderr]
for exe, p in runs:
    text, _ = p.communicate()
    out.append(f"     Running {exe}\n{text}")
    ok &= p.returncode == 0
log.write_text("\n".join(out), encoding="utf-8")
sys.exit(0 if ok else 1)
