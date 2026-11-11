"""out/raw.jsonl -> a markdown table: median and worst rep of every (label, run) the benchmark measured."""
import json, pathlib, statistics

rows = [json.loads(l) for l in (pathlib.Path(__file__).parent.parent / "out/raw.jsonl").read_text().splitlines() if l.strip()]
T = ["fps_avg", "frame_p50_ms", "frame_p99_ms", "frame_max_ms", "cpu_ms_p99", "gpu_ms_p99", "ram_mb", "vram_mb",
     "chunks_meshed_per_s", "missing_chunks_max", "holes_max", "draws_avg", "quads_avg"]
HIGHER_IS_BETTER = {"fps_avg", "chunks_meshed_per_s"}
groups = {}
for r in rows:
    if r["label"] != "strip":
        groups.setdefault((r["label"], r["planet_radius_m"], r["run"]), []).append(r)
print("| series | planet m | run | reps | | " + " | ".join(T) + " | budget | spikes |\n|" + "---|" * (len(T) + 7))
for (label, radius, run), rs in groups.items():
    med = [f"{statistics.median(r[f] for r in rs):g}" for f in T]
    worst = [f"{(min if f in HIGHER_IS_BETTER else max)(r[f] for r in rs):g}" for f in T]
    ok = "ok" if all(r["budget_ok"] for r in rs) else "FAIL"
    print(f"| {label} | {radius:g} | {run} | {len(rs)} | median | " + " | ".join(med) + " | | |")
    print(f"| | | | | worst | " + " | ".join(worst) + f" | {ok} | {sum(len(r['spikes']) for r in rs)} |")

# the runs with a simulation in them (run=sim, ADR 0025 S): the server's tick and the workers' queue
S = ["players", "tick_ms_p50", "tick_ms_p99", "tick_ms_max", "tick_cores", "job_wait_ms_p99", "worker_cores", "frame_cores"]
sims = {k: rs for k, rs in groups.items() if all("tick_ms_p99" in r and r["players"] > 1 for r in rs)}
if sims:
    print("\n| series | planet m | run | reps | | " + " | ".join(S) + " |\n|" + "---|" * (len(S) + 5))
    for (label, radius, run), rs in sims.items():
        print(f"| {label} | {radius:g} | {run} | {len(rs)} | median | " + " | ".join(f"{statistics.median(r[f] for r in rs):g}" for f in S) + " |")
        print(f"| | | | | worst | " + " | ".join(f"{max(r[f] for r in rs):g}" for f in S) + " |")
