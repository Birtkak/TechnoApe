"""Builds game/assets/sounds/**.ogg from CC0 recordings (ADR 0044).

The raw downloads live outside the repo in ~/Tools/downloads/sounds/<pack>/ (see the
recipe file's "_sources"). Each recipe in tools/sound_recipes.json names a source and
how to cut it:

  "step/grass": {"src": "kenney/.../footstep_grass_00*.ogg"}       every matching file is a variant
  "step/gravel": {"src": "bigsoundbank/0510.ogg", "slice": {"count": 6, "len": 0.35}}
                                                                    the strongest onsets, one variant each
  "loop/fire": {"src": "bigsoundbank/0031.ogg", "start": 4, "dur": 10, "loop": 1.5}
                                                                    a seamless loop (crossfade in s)
  "music/x": {"src": "...", "music": true}                          stereo, levelled as a whole
  "loop/monolith_hum": {"synth": "hum", "loop": 0.5}                made here (SYNTH below), no recording

Optional keys: start / dur (s, before slicing), af (an extra ffmpeg filter), pitch (playback
rate: 0.7 is lower and slower), gain_db (after levelling), level (target dBFS of the loudest
50 ms; default -12 one-shots, -20 loops, -16 music), max (variants kept), trim (false keeps silence).

Writes assets/sounds/<name>.ogg (one variant) or <name>_<i>.ogg, plus assets/sounds/index.json
(name -> files) that the game loads, and out/sound_sheet.png (every variant's waveform) with --sheet.

    python tools/make_sounds.py [--only step/] [--sheet]
"""
import argparse, glob, json, math, os, re, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

GAME = Path(__file__).resolve().parent.parent
RECIPES = GAME / "tools" / "sound_recipes.json"
OUT = GAME / "assets" / "sounds"
RATE = 44100
ROOT = Path.home() / "Tools" / "downloads" / "sounds"
FFMPEG = Path.home() / "Tools" / "ffmpeg" / "ffmpeg-master-latest-win64-gpl" / "bin" / "ffmpeg.exe"


def ffmpeg() -> str:
    if FFMPEG.exists():
        return str(FFMPEG)
    found = shutil.which("ffmpeg")
    if not found:
        sys.exit("ffmpeg not found: unpack it to ~/Tools/ffmpeg (see game/README.md)")
    return found


def decode(path: Path, channels: int, r: dict) -> np.ndarray:
    filters = []
    if "pitch" in r:
        filters.append(f"asetrate={int(RATE * float(r['pitch']))},aresample={RATE}")
    if r.get("af"):
        filters.append(r["af"])
    cmd = [ffmpeg(), "-v", "error"]
    if "start" in r:
        cmd += ["-ss", str(r["start"])]
    if "dur" in r:
        cmd += ["-t", str(r["dur"])]
    cmd += ["-i", str(path), "-ar", str(RATE), "-ac", str(channels)]
    if filters:
        cmd += ["-af", ",".join(filters)]
    cmd += ["-f", "f32le", "-"]
    raw = subprocess.run(cmd, capture_output=True, check=True).stdout
    a = np.frombuffer(raw, dtype=np.float32).copy()
    return a.reshape(-1, channels)


def encode(a: np.ndarray, path: Path, quality: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    a = np.clip(a, -1.0, 1.0).astype(np.float32)
    cmd = [ffmpeg(), "-v", "error", "-y", "-f", "f32le", "-ar", str(RATE), "-ac", str(a.shape[1]), "-i", "-",
           "-c:a", "libvorbis", "-q:a", str(quality), str(path)]
    subprocess.run(cmd, input=a.tobytes(), check=True)


def envelope(a: np.ndarray, win: int) -> np.ndarray:
    m = np.abs(a).max(axis=1)
    n = len(m) // win
    if n == 0:
        return np.array([m.max() if len(m) else 0.0])
    return np.sqrt((m[: n * win].reshape(n, win) ** 2).mean(axis=1))


def onsets(a: np.ndarray, count: int, length: float, gap: float, thresh: float) -> list:
    """Starts (samples) of the `count` strongest hits at least `gap` s apart, in time order."""
    win = int(RATE * 0.005)
    env = envelope(a, win)
    if env.max() <= 0:
        return []
    env = env / env.max()
    # a rise: this window is loud and clearly louder than the 30 ms before it
    back = max(1, int(0.03 / 0.005))
    cand = []
    for i in range(back, len(env)):
        if env[i] >= thresh and env[i] > 1.8 * env[i - back:i].mean() + 0.02:
            cand.append(i)
    # collapse runs: keep the first window of each rise and its peak strength
    hits = []
    for i in cand:
        if hits and i - hits[-1][0] < gap / 0.005:
            continue
        peak = env[i:i + int(length / 0.005)].max()
        hits.append((i, peak))
    hits.sort(key=lambda h: -h[1])
    starts = []
    for i in sorted(h[0] for h in hits[:count]):
        # a noise rise can trigger early: start where this hit first reaches half its peak
        seg = env[i:i + int(length / 0.005)]
        j = i + int(np.argmax(seg >= seg.max() * 0.5))
        starts.append(max(0, j * win - int(RATE * 0.012)))
    return sorted(set(starts))


def trim(a: np.ndarray, rel_db: float = -45.0) -> np.ndarray:
    m = np.abs(a).max(axis=1)
    if m.max() <= 0:
        return a
    over = np.where(m > m.max() * 10 ** (rel_db / 20))[0]
    s = max(0, over[0] - int(RATE * 0.004))
    e = min(len(a), over[-1] + int(RATE * 0.04))
    return a[s:e]


def fade(a: np.ndarray, fin: float, fout: float) -> np.ndarray:
    a = a.copy()
    ni, no = min(len(a), int(RATE * fin)), min(len(a), int(RATE * fout))
    if ni:
        a[:ni] *= np.linspace(0, 1, ni)[:, None]
    if no:
        a[-no:] *= (np.linspace(1, 0, no) ** 2)[:, None]
    return a


def level(a: np.ndarray, target_db: float) -> np.ndarray:
    """Loudest 50 ms RMS to target_db, then a fast limiter at -1 dBFS: one sharp click must
    not turn a whole step or rustle down."""
    env = envelope(a, int(RATE * 0.05))
    loud = env.max()
    if loud <= 0:
        return a
    a = a * (10 ** (target_db / 20) / loud)
    ceil = 10 ** (-1 / 20)
    m = np.abs(a).max(axis=1)
    if m.max() <= ceil:
        return a
    g = np.minimum(1.0, ceil / np.maximum(m, 1e-9))
    w = int(RATE * 0.003)   # hold the lowest gain 3 ms around each peak, then smooth it (no clicks)
    pad = np.pad(g, (w, w), constant_values=1.0)
    held = np.min(np.lib.stride_tricks.sliding_window_view(pad, 2 * w + 1), axis=1)
    gain = np.minimum(np.convolve(held, np.ones(w) / w, mode="same"), held).clip(0.0, 1.0)
    a = a * gain[:, None]
    peak = np.abs(a).max()
    return a * (ceil / peak) if peak > ceil else a   # what the smoothing let through


def make_loop(a: np.ndarray, xf: float) -> np.ndarray:
    n = int(RATE * xf)
    if len(a) <= 2 * n:
        return a
    body, tail = a[:-n].copy(), a[-n:]
    t = np.linspace(0, 1, n)[:, None]
    body[:n] = body[:n] * np.sqrt(t) + tail * np.sqrt(1 - t)   # equal power: the end flows into the start
    return body


def synth_hum() -> list:
    """The Gelvin monolith: a deep drone on 41.25 Hz with harmonics, two partials 0.125 Hz apart beating
    once per 8 s and a faint shimmer. Every frequency fits whole cycles in 8 s, so 8.5 s loops at 0.5 s."""
    t = np.arange(int(RATE * 8.5)) / RATE
    a = sum(g * np.sin(2 * np.pi * f * t) for f, g in [(41.25, 1.0), (41.375, 0.6), (82.5, 0.7), (123.75, 0.35),
                                                         (165.0, 0.25), (247.5, 0.08)])
    a = a * (0.85 + 0.15 * np.sin(2 * np.pi * 0.25 * t)) + 0.04 * np.sin(2 * np.pi * 660 * t) * np.sin(2 * np.pi * 0.125 * t) ** 2
    return [a[:, None]]


def synth_blips() -> list:
    """Machine blips while the apetion is rebuilt: short sine/square chirps, some rising, some falling."""
    out = []
    for f0, f1, dur, sq in [(880, 1320, 0.05, 0), (1320, 990, 0.045, 1), (660, 1760, 0.07, 0), (1480, 1480, 0.035, 1),
                            (990, 740, 0.06, 0), (520, 1040, 0.08, 1)]:
        t = np.arange(int(RATE * dur)) / RATE
        ph = 2 * np.pi * np.cumsum(np.geomspace(f0, f1, len(t))) / RATE
        w = np.sin(ph) if not sq else np.tanh(4 * np.sin(ph)) * 0.6
        out.append((w * np.exp(-t / (dur * 0.5)))[:, None])
    return out


def synth_done() -> list:
    """The rebuild is done: a bloop down, then two rising tones."""
    parts = []
    for f0, f1, dur in [(700, 260, 0.12), (520, 520, 0.08), (780, 1040, 0.16)]:
        t = np.arange(int(RATE * dur)) / RATE
        ph = 2 * np.pi * np.cumsum(np.geomspace(f0, f1, len(t))) / RATE
        parts.append(np.sin(ph) * np.minimum(1, t / 0.005) * np.exp(-t / (dur * 0.6)))
    return [np.concatenate(parts)[:, None]]


SYNTH = {"hum": synth_hum, "blips": synth_blips, "done": synth_done}


def build(name: str, r: dict) -> list:
    music = bool(r.get("music"))
    channels = 2 if music else 1
    srcs = [] if "synth" in r else (r["src"] if isinstance(r["src"], list) else [r["src"]])
    files = []
    for s in srcs:
        found = sorted(glob.glob(str(ROOT / s)))
        if not found:
            raise FileNotFoundError(f"{name}: no source {ROOT / s}")
        files += found
    parts = []
    for f in files:
        a = decode(Path(f), channels, r)
        if "slice" in r:
            sl = r["slice"]
            length = float(sl.get("len", 0.4))
            starts = onsets(a, int(sl.get("count", 4)), length, float(sl.get("gap", length * 0.8)), float(sl.get("thresh", 0.2)))
            for i, st in enumerate(starts):
                end = min(len(a), st + int(RATE * length))
                if i + 1 < len(starts):
                    end = min(end, starts[i + 1] - int(RATE * 0.005))
                parts.append(a[st:end])
        else:
            parts.append(a)
    if "synth" in r:
        parts = [p.astype(np.float32) for p in SYNTH[r["synth"]]()]
    parts = parts[: int(r.get("max", 99))]
    loop = r.get("loop")
    target = float(r.get("level", -16.0 if music else (-20.0 if loop else -12.0)))
    out = []
    for i, p in enumerate(parts):
        if loop:
            p = make_loop(p, float(loop))
        elif not music:
            if r.get("trim", True):
                p = trim(p)
            p = fade(p, 0.003, min(0.06, len(p) / RATE * 0.3))
        else:
            p = fade(p, 0.01, 2.0)
        p = level(p, target) * 10 ** (float(r.get("gain_db", 0.0)) / 20)
        rel = f"{name}.ogg" if len(parts) == 1 else f"{name}_{i}.ogg"
        encode(p, OUT / rel, 4 if music else 5)
        out.append({"file": rel, "len": round(len(p) / RATE, 3), "peak": round(float(np.abs(p).max()), 3), "wave": p[:, 0]})
    return out


def sheet(results: dict, path: Path) -> None:
    """Every variant's waveform, one row per sound: check slicing and levels without ears."""
    from PIL import Image, ImageDraw
    names = sorted(results)
    cw, ch, lw = 150, 34, 170
    cols = max(len(results[n]) for n in names)
    img = Image.new("RGB", (lw + cols * cw, ch * len(names)), (24, 24, 28))
    d = ImageDraw.Draw(img)
    for row, n in enumerate(names):
        y0 = row * ch
        d.text((4, y0 + 10), n, fill=(220, 220, 220))
        for c, v in enumerate(results[n]):
            w = v["wave"]
            x0 = lw + c * cw
            d.rectangle([x0, y0 + 1, x0 + cw - 4, y0 + ch - 2], fill=(36, 36, 44))
            if len(w):
                step = max(1, len(w) // (cw - 4))
                for x in range(min(cw - 4, len(w) // step)):
                    seg = w[x * step:(x + 1) * step]
                    hi, lo = float(seg.max()), float(seg.min())
                    mid = y0 + ch // 2
                    d.line([x0 + x, mid - hi * (ch // 2 - 2), x0 + x, mid - lo * (ch // 2 - 2)], fill=(120, 200, 140))
            d.text((x0 + 2, y0 + 1), f"{v['len']:.2f}s", fill=(150, 150, 160))
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path)


def write_credits(data: dict) -> None:
    """assets/sounds/CREDITS.md: every sound's sources. All CC0, so this is for tracing, not a duty."""
    lines = ["# Sound credits", "", "Every file here is cut from CC0 (public domain) recordings by "
             "`tools/make_sounds.py` (ADR 0044). No attribution is required; this list traces each sound "
             "to its source.", "", "## Sources", ""]
    lines += [f"- `{k}`: {v}" for k, v in data["_sources"].items()]
    lines += ["", "## Sounds", "", "| Sound | Source |", "|---|---|"]
    for n, r in data["sounds"].items():
        srcs = r["src"] if isinstance(r.get("src"), list) else [r.get("src", f"synth: {r.get('synth')}, made in make_sounds.py")]
        lines.append(f"| {n} | {', '.join(f'`{s}`' for s in srcs)} |")
    (OUT / "CREDITS.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", default="", help="build recipes whose name starts with this")
    ap.add_argument("--sheet", action="store_true", help="write out/sound_sheet.png")
    args = ap.parse_args()
    recipes = json.loads(RECIPES.read_text(encoding="utf-8"))["sounds"]
    todo = {n: r for n, r in recipes.items() if not n.startswith("_") and n.startswith(args.only)}
    index_path = OUT / "index.json"
    index = json.loads(index_path.read_text(encoding="utf-8")) if index_path.exists() and args.only else {}
    results, failed = {}, []
    with ThreadPoolExecutor(max_workers=os.cpu_count() or 4) as pool:
        futs = {n: pool.submit(build, n, r) for n, r in todo.items()}
        for n, f in futs.items():
            try:
                results[n] = f.result()
            except Exception as e:  # report every broken recipe, not just the first
                failed.append(f"{n}: {e}")
    for n, vs in results.items():
        # stale variants of this sound only: mob/chicken_3.ogg, never mob/chicken_hurt_0.ogg
        stale = [p for p in glob.glob(str(OUT / f"{n}_*.ogg")) if re.fullmatch(r"\d+", Path(p).stem[len(Path(n).name) + 1:])]
        for old in stale + glob.glob(str(OUT / f"{n}.ogg")):
            if Path(old).relative_to(OUT).as_posix() not in [v["file"] for v in vs]:
                os.remove(old)
                imp = Path(old + ".import")
                if imp.exists():
                    imp.unlink()
        index[n] = [v["file"] for v in vs]
        if not vs:
            failed.append(f"{n}: no variants (slice found no onsets?)")
    index = {k: index[k] for k in sorted(index) if k in recipes}
    index_path.write_text(json.dumps(index, indent=1) + "\n", encoding="utf-8")
    write_credits(json.loads(RECIPES.read_text(encoding="utf-8")))
    total = sum(len(v) for v in results.values())
    print(f"{len(results)} sounds, {total} files -> {OUT}")
    if args.sheet:
        sheet(results, GAME / "out" / "sound_sheet.png")
        print("sheet: out/sound_sheet.png")
    for f in failed:
        print("FAILED", f)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
