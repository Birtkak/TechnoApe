#!/usr/bin/env python
"""Run the game's autopilot checks one after another, off-screen, and print one table.

    python tools/run_checks.py [name ...]        # names from tools/checks.json; none = all
    python tools/run_checks.py --list

Each check is one Godot run (killed after its timeout, strays killed first). The table shows ok,
seconds, budget_ok and the check's own reason; the exit code is the number of failed checks. Logs
and the run's out dir are under game/out/checks/<name>/ (log.txt, strip.png, shots/).
"""
import json, os, re, subprocess, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GAME = os.path.join(ROOT, "game")
GODOT = os.path.expanduser("~/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe")
EXE = os.path.basename(GODOT)


def kill_strays():
    subprocess.run(["taskkill", "/F", "/IM", EXE], capture_output=True)


def run(name, spec):
    out = os.path.join(GAME, "out", "checks", name)
    os.makedirs(out, exist_ok=True)
    args = [GODOT, "--path", GAME, "--position", "3000,3000", "--resolution", "960x540", "--"] + spec["args"] + ["out=res://out/checks/" + name]
    t0 = time.time()
    env = dict(os.environ, MSYS_NO_PATHCONV="1")
    try:
        p = subprocess.run(args, capture_output=True, text=True, errors="replace", timeout=spec.get("timeout_s", 300), env=env)
        text = p.stdout + p.stderr
        status = "exit %d" % p.returncode
    except subprocess.TimeoutExpired as e:
        text = (e.stdout or "") + (e.stderr or "")
        if isinstance(text, bytes):
            text = text.decode(errors="replace")
        status = "timeout"
        kill_strays()
    secs = time.time() - t0
    with open(os.path.join(out, "log.txt"), "w", encoding="utf-8") as f:
        f.write(text)
    row = {"name": name, "s": round(secs), "status": status, "ok": None, "budget_ok": None, "reason": ""}
    m = re.search(r"^debug %s (\{.*\})$" % re.escape(spec.get("json", name)), text, re.M)
    if m:
        d = json.loads(m.group(1))
        row["ok"] = d.get("ok", all(v is not False for v in d.values()) if spec.get("all_true") else None)   # 0 == False in Python
        try:
            row["reason"] = d.get("error") or spec.get("summary", "").format(**d)
        except (KeyError, IndexError):
            row["reason"] = d.get("error", "")
    else:
        row["reason"] = "no 'debug %s' line" % spec.get("json", name)
    m = re.search(r'^(\{.*"budget_ok".*\})$', text, re.M)
    if m:
        row["budget_ok"] = json.loads(m.group(1)).get("budget_ok")
    if re.search(r"SCRIPT ERROR|Parse Error", text):
        row["reason"] = (row["reason"] + " script errors in log").strip()
    return row


def main():
    with open(os.path.join(ROOT, "tools", "checks.json"), encoding="utf-8") as f:
        checks = json.load(f)
    names = [a for a in sys.argv[1:] if not a.startswith("--")]
    if "--list" in sys.argv:
        for n, c in checks.items():
            print("%-10s %s" % (n, c.get("doc", "")))
        return 0
    unknown = [n for n in names if n not in checks]
    if unknown:
        sys.exit("unknown checks: %s (see --list)" % ", ".join(unknown))
    kill_strays()
    rows = [run(n, checks[n]) for n in (names or list(checks))]
    print("%-10s %-5s %5s %-9s %-7s %s" % ("check", "ok", "s", "budget", "exit", "reason"))
    for r in rows:
        print("%-10s %-5s %5d %-9s %-7s %s" % (r["name"], r["ok"], r["s"], r["budget_ok"], r["status"], r["reason"][:120]))
    failed = [r for r in rows if r["ok"] is not True]
    print("%d/%d checks green" % (len(rows) - len(failed), len(rows)))
    return len(failed)


if __name__ == "__main__":
    sys.exit(main())
