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


# ---------------- ambience (ADR 0120): made here, the CC0 packs hold no birds, insects or wind ----------------

def _env(n: int, attack: float, decay: float) -> np.ndarray:
    t = np.arange(n) / RATE
    return np.minimum(1, t / max(attack, 1e-4)) * np.exp(-t / max(decay, 1e-4))


def _sweep(f0: float, f1: float, dur: float, vib: float = 0.0, vib_hz: float = 0.0) -> np.ndarray:
    t = np.arange(int(RATE * dur)) / RATE
    f = np.geomspace(f0, f1, len(t)) * (1 + vib * np.sin(2 * np.pi * vib_hz * t))
    return np.sin(2 * np.pi * np.cumsum(f) / RATE)


def _phrase(notes: list) -> np.ndarray:
    """notes: [f0, f1, dur, gap, vib, vib_hz]; each a swept whistle with a soft edge."""
    out = []
    for f0, f1, dur, gap, vib, vh in notes:
        w = _sweep(f0, f1, dur, vib, vh)
        n = len(w)
        w = w * np.minimum(1, np.arange(n) / (RATE * 0.006)) * np.minimum(1, (n - np.arange(n)) / (RATE * 0.012))
        out += [w, np.zeros(int(RATE * gap))]
    return np.concatenate(out)


def synth_birds() -> list:
    """Temperate songbirds by day: a falling trill, a two-note whistle, a warble, a chip-chip-chip."""
    rng = np.random.default_rng(7)
    out = [
        _phrase([[4200 - i * 120, 3600 - i * 120, 0.045, 0.025, 0, 0] for i in range(9)]),
        _phrase([[2900, 2950, 0.22, 0.12, 0.01, 6], [2300, 2250, 0.3, 0.0, 0.01, 6]]),
        _phrase([[float(rng.uniform(2400, 4200)), float(rng.uniform(2400, 4200)), 0.07, 0.015, 0.05, 30] for _ in range(10)]),
        _phrase([[5200, 3800, 0.03, 0.09, 0, 0]] * 4),
        _phrase([[3300, 4400, 0.12, 0.06, 0, 0], [4400, 3100, 0.1, 0.05, 0, 0], [3600, 3700, 0.35, 0, 0.04, 22]]),
    ]
    return [o[:, None] * 0.8 for o in out]


def synth_jungle() -> list:
    """Hot wet forest calls: a rising-falling whoop, a hollow toucan croak, a cascade of piping notes."""
    out = []
    w = _sweep(500, 1400, 0.35) * _env(int(RATE * 0.35), 0.05, 0.3)
    out.append(np.concatenate([w, np.zeros(int(RATE * 0.1)), _sweep(1500, 600, 0.5) * _env(int(RATE * 0.5), 0.01, 0.35)]))
    croak = []
    for i in range(3):
        n = int(RATE * 0.12)
        t = np.arange(n) / RATE
        c = np.tanh(3 * np.sin(2 * np.pi * 310 * t)) * (0.5 + 0.5 * np.sign(np.sin(2 * np.pi * 70 * t))) * _env(n, 0.005, 0.08)
        croak += [c, np.zeros(int(RATE * 0.11))]
    out.append(np.concatenate(croak))
    out.append(_phrase([[1900 - i * 90, 1750 - i * 90, 0.11, 0.07, 0.02, 9] for i in range(8)]))
    out.append(_phrase([[800, 2400, 0.25, 0.05, 0, 0], [2400, 2300, 0.4, 0, 0.08, 7]]))
    return [o[:, None] * 0.8 for o in out]


def synth_crickets() -> list:
    """Night crickets: chirps of 3-4 pulses of a 4.4 kHz carrier, a few per sound, each variant its own rate."""
    out = []
    for f, pulses, chirps, gap in [(4400, 3, 4, 0.32), (4700, 4, 3, 0.45), (4100, 3, 5, 0.26)]:
        parts = []
        for _ in range(chirps):
            for _ in range(pulses):
                n = int(RATE * 0.016)
                t = np.arange(n) / RATE
                parts += [np.sin(2 * np.pi * f * t) * np.sin(np.pi * t / t[-1]), np.zeros(int(RATE * 0.01))]
            parts.append(np.zeros(int(RATE * gap)))
        out.append(np.concatenate(parts)[:, None] * 0.7)
    return out


def synth_owl() -> list:
    """An owl at night: a soft hoot, then a long wavering one (two variants at different pitches)."""
    out = []
    for f in [370, 330]:
        h1 = _sweep(f * 1.04, f, 0.28) * _env(int(RATE * 0.28), 0.06, 0.16)
        h2 = _sweep(f * 1.06, f * 0.94, 0.75, 0.015, 7) * _env(int(RATE * 0.75), 0.08, 0.45)
        a = np.concatenate([h1, np.zeros(int(RATE * 0.3)), h2])
        a = a + 0.15 * np.sin(2 * np.pi * 2 * np.cumsum(np.full(len(a), f)) / RATE) * np.abs(a)
        out.append(a[:, None] * 0.8)
    return out


def synth_wind() -> list:
    """An exposed wind bed: brown noise through a slowly moving band, swelling in gusts; loops at 1.5 s."""
    rng = np.random.default_rng(11)
    n = int(RATE * 14)
    t = np.arange(n) / RATE
    white = rng.standard_normal(n)
    out = np.zeros(n)
    centre = 350 + 250 * np.sin(2 * np.pi * t / 7.0) + 120 * np.sin(2 * np.pi * t / 2.3)
    for lo in range(0, n, 2048):   # a band-pass per block: centre frequency follows the gusts
        hi = min(n, lo + 2048 + 1024)
        seg = white[lo:hi]
        spec = np.fft.rfft(seg)
        fr = np.fft.rfftfreq(len(seg), 1 / RATE)
        c = centre[lo]
        spec *= np.exp(-((np.log(np.maximum(fr, 1) / c)) ** 2) / 0.35)
        blk = np.fft.irfft(spec, len(seg)) * np.hanning(len(seg))
        out[lo:hi] += blk
    gust = 0.55 + 0.45 * np.sin(2 * np.pi * t / 7.0) ** 2
    return [(out * gust)[:, None]]


SYNTH.update({"birds": synth_birds, "jungle": synth_jungle, "crickets": synth_crickets, "owl": synth_owl, "wind": synth_wind})


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
