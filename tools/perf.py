"""The speed guard (ADR 0025, K): a quick run's numbers against tests/perf/baseline.json, so a change that makes
the game slower fails the gate on the day it is made. Not a benchmark (that is tools/bench.sh).

  perf.py check RUN < json lines          the median of the lines against RUN's baseline; prints what is worse
                                          than its tolerance, exit 1 if anything is
  perf.py accept < "RUN<TAB>json" lines   writes the medians per run as the new baseline
  perf.py test                            the self-check

It holds our own work, not the frame: on the dev PC in daily use (a moving wallpaper, browsers, the NVIDIA
overlay; the GPU 27-41 % busy without the game) the frame's p50 rose 12-17 % while the GPU stages' total moved
0-9 % and the frame thread's work not at all; the difference was the driver's wait for a screen image. So:
  work_ms      the frame thread's stages at p50, less the driver's waits (acquire, present, between)
  gpu_ms_p50   the GPU stages' total at p50
  job_us_avg   a mesh job's time on a worker
  vram_mb
  tick_ms_p99  the server's tick (run=sim only)
A number fails past its baseline plus the larger of a share and an amount: about twice the spread of ten quick
`stand` runs and three to five of each other (GPU total 1-4 %, `stand` 15 % in its two modes; work 0.25-0.5 ms
spread 0.1-0.18 ms; jobs up to 8 %; VRAM 0.2 %; tick p99 30 %). A 0.5 ms stall in the frame thread fails it.
"""
import json, pathlib, statistics, sys

BASE = pathlib.Path(__file__).resolve().parent.parent / "tests/perf/baseline.json"
# per number: (share, amount) over the baseline that still passes
TOL = {"work_ms": (0.30, 0.25), "gpu_ms_p50": (0.15, 0.15), "job_us_avg": (0.20, 50.0), "vram_mb": (0.10, 64.0), "tick_ms_p99": (0.60, 1.0)}
DRIVER = {"acquire", "present", "between"}


def measure(d):
    m = {k: d[k] for k in ("gpu_ms_p50", "job_us_avg", "vram_mb", "tick_ms_p99") if k in d}
    if "cpu_p50" in d:
        m["work_ms"] = round(sum(v for k, v in d["cpu_p50"].items() if k not in DRIVER), 3)
    return m


def keys(run):
    return [k for k in TOL if k != "tick_ms_p99" or run == "sim"]


def numbers(run, lines):
    try:
        ms = [measure(json.loads(l)) for l in lines if l.strip().startswith("{")]
    except ValueError as e:
        return {"error": f"unreadable run line ({e})"}
    return {k: statistics.median(m[k] for m in ms) for k in keys(run) if ms and all(k in m for m in ms)}


def complete(run, lines):
    """Why `lines` cannot make RUN's baseline (a run that crashed, a number missing), or None."""
    good = [l for l in lines if l.strip().startswith("{")]
    if len(good) < len(lines):
        return f"{run}: {len(good)} of {len(lines)} runs gave a line"
    got = numbers(run, good)
    missing = [k for k in keys(run) if k not in got]
    return f"{run}: no {', '.join(missing)}" if missing else None


def worse(base, got):
    out = []
    for k in base:
        share, amount = TOL[k]
        allowed = base[k] + max(base[k] * share, amount)
        if k in got and got[k] > allowed:
            out.append(f"{k} {got[k]:g} against {base[k]:g} (up to {allowed:g} passes)")
    return out


if __name__ == "__main__":
    if sys.argv[1] == "check":
        run = sys.argv[2]
        base = json.loads(BASE.read_text()).get(run)
        got = numbers(run, sys.stdin.read().splitlines())
        if base is None:
            print(f"no baseline for {run} in {BASE.name}: bash tools/check.sh accept-perf")
            sys.exit(1)
        if not got or "error" in got:
            print(got.get("error", "no run line"))
            sys.exit(1)
        bad = worse(base, got)
        print("; ".join(bad))
        sys.exit(1 if bad else 0)
    elif sys.argv[1] == "accept":
        by = {}
        for l in sys.stdin.read().splitlines():
            if "\t" in l:
                run, j = l.split("\t", 1)
                by.setdefault(run, []).append(j)
        why = [w for w in (complete(run, ls) for run, ls in by.items()) if w]
        if why:
            print("no baseline written: " + "; ".join(why))
            sys.exit(1)
        base = {run: numbers(run, ls) for run, ls in by.items()}
        BASE.parent.mkdir(parents=True, exist_ok=True)
        BASE.write_text(json.dumps(base, indent=2) + "\n")
        print(json.dumps(base))
    elif sys.argv[1] == "test":
        line = lambda gpu, work, **k: json.dumps({"gpu_ms_p50": gpu, "job_us_avg": 400, "vram_mb": 800, "cpu_p50": {"encode": work, "acquire": 9.0}, **k})
        base = numbers("stand", [line(1.5, 0.4)])
        assert base == {"work_ms": 0.4, "gpu_ms_p50": 1.5, "job_us_avg": 400, "vram_mb": 800}, base
        # the driver's wait and the tick (outside sim) are not held; a little slower passes
        assert not worse(base, numbers("stand", [line(1.6, 0.6, tick_ms_p99=50)]))
        # a 0.5 ms stall in the frame thread fails, on the median of three
        bad = worse(base, numbers("stand", [line(1.5, 0.9), line(1.5, 0.95), line(1.5, 0.4)]))
        assert len(bad) == 1 and bad[0].startswith("work_ms 0.9 "), bad
        assert worse(base, numbers("stand", [line(1.8, 0.4)]))[0].startswith("gpu_ms_p50")
        assert "tick_ms_p99" in numbers("sim", [line(1.5, 0.4, tick_ms_p99=3)])
        # a crashed run or a missing number refuses a baseline rather than leaving that run unguarded
        assert complete("stand", [line(1.5, 0.4)] * 3) is None
        assert "2 of 3" in complete("stand", [line(1.5, 0.4)] * 2 + [""])
        assert "tick_ms_p99" in complete("sim", [line(1.5, 0.4)] * 3)
        # a broken line names itself instead of a traceback
        assert numbers("stand", ['{"gpu_ms_p50": 1.'])["error"].startswith("unreadable")
        print("ok")
