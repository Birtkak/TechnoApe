"""Live reload, timed end to end (tools/reload-check.sh runs this with out/.lock held).

One off-screen run=stand at the spawn reads its textures, models, effects and sounds from scratch copies
under out/reload/; at fixed times of the run's clock this changes a file, writes a broken one, then a good
one again, and checks by number: the filmstrip's pixels change (textures, models, effects) or the card's
reload names the rows whose samples changed (sounds); the swap's stderr line comes within 1.5 s of the write;
no frame over 16.7 ms near it; the broken file is named on stderr and nothing changes. One JSON verdict line.
"""
import json, os, re, shutil, struct, subprocess, sys, threading, time, zlib

from PIL import Image, ImageChops

R = 'out/reload'
EXE = os.path.abspath('target/release/technoape.exe')
BOUND, HITCH, CAP = 1.5, 16.7, 0.5  # s, ms, filmstrip step in s


def png(path, w, h, rgb):
    """An 8-bit RGBA PNG of one colour, written whole (a temp file renamed over, as an editor saves)."""
    raw = b''.join(b'\0' + bytes(rgb + (255,)) * w for _ in range(h))
    chunk = lambda k, d: struct.pack('>I', len(d)) + k + d + struct.pack('>I', zlib.crc32(k + d))
    put(path, b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


def put(path, data):
    tmp = path + '.part'
    with open(tmp, 'wb') as f:
        f.write(data if isinstance(data, bytes) else data.encode())
    os.replace(tmp, path)


def setup():
    shutil.rmtree(R, ignore_errors=True)
    for d in ('textures', 'models', 'fx', 'snd'):
        os.makedirs(f'{R}/{d}')
    shutil.copy('data/models/furnace.toml', f'{R}/models/furnace.toml')
    shutil.copy('data/effects.toml', f'{R}/fx/effects.toml')
    shutil.copy('data/sounds.toml', f'{R}/snd/sounds.toml')
    if os.path.isdir('data/sounds'):
        shutil.copytree('data/sounds', f'{R}/snd/sounds')


def text(p):
    return open(p, encoding='utf-8').read()


FURNACE, FX, SND = f'{R}/models/furnace.toml', f'{R}/fx/effects.toml', f'{R}/snd/sounds.toml'
fx0, model0, snd0 = None, None, None

# (kind, step, t of the run's clock, the write, the swap line that answers it)
def steps():
    tex = f'{R}/textures/grass_top.png'
    flames = lambda s: s.replace('size = [0.1, 0.2]\nsize_curve = [[0.0, 0.6]', 'size = [0.5, 0.8]\nsize_curve = [[0.0, 0.6]', 1)
    return [
        ('textures', 'change', 5.0, lambda: png(tex, 16, 16, (255, 0, 255)), r'textures: \d+ blocks painted'),
        ('textures', 'bad', 7.5, lambda: png(tex, 16, 17, (0, 255, 255)), r'textures: grass_top\.png: .*16 high'),
        ('textures', 'restore', 10.0, lambda: png(tex, 16, 16, (0, 255, 255)), r'textures: \d+ blocks painted'),
        ('models', 'change', 12.5, lambda: put(FURNACE, model0.replace('"stone dark", "stone", "stone light"', '"copper", "copper", "copper"', 1)), r'models: \d+ loaded'),
        ('models', 'bad', 15.0, lambda: put(FURNACE, model0.replace('"stone dark"', '"no such colour"', 1)), r'models: .*furnace\.toml.*no such colour.*last good model stays'),
        ('models', 'restore', 17.5, lambda: put(FURNACE, model0), r'models: \d+ loaded'),
        ('effects', 'change', 20.0, lambda: put(FX, flames(fx0)), r'effects: \d+ rows'),
        ('effects', 'bad', 22.5, lambda: put(FX, fx0.replace('spin = 2.0', 'spin = nan', 1)), r'effects: .*effects\.toml.*the rows before stay'),
        ('effects', 'restore', 25.0, lambda: put(FX, fx0), r'effects: \d+ rows'),
        ('sounds', 'change', 27.5, lambda: put(SND, snd0.replace('decay = 0.11 }', 'decay = 0.4 }', 1)), r'sounds made again .*changed: \[dig\.stone\]'),
        ('sounds', 'bad', 30.0, lambda: put(SND, snd0 + '\n[[sound]]\nname = "zz.check"\nbus = "world"\nfrom = [{ base = "base.nope" }]\n'), r'sounds\.toml refused.*base\.nope'),
        ('sounds', 'restore', 32.5, lambda: put(SND, snd0), r'sounds made again .*changed: \[dig\.stone\]'),
    ]


def main():
    global fx0, model0, snd0
    setup()
    model0, fx0, snd0 = text(FURNACE), text(FX), text(SND)
    assert 'size = [0.1, 0.2]\nsize_curve = [[0.0, 0.6]' in fx0 and 'spin = 2.0' in fx0 and 'decay = 0.11 }' in snd0 and '"stone dark", "stone", "stone light"' in model0, 'the data files no longer hold the lines this check edits'
    args = [EXE, 'run=play', 'seconds=37', 'settings=out/reload/settings.txt', 'warmup=4', 'fx=1', 'fxplay=fire', 'model=furnace', 'sound=1', 'save=0',
            f'textures={R}/textures', f'models={R}/models', f'effects={FX}', f'sound_dir={R}/snd',
            f'capture_ms={int(CAP * 1000)}', f'sheet={R}/sheet.png', 'capture_w=640']
    p = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8', errors='replace')
    lines, t0 = [], [None]  # (wall, text); the wall time of t=0 ("loaded in")

    def read():
        for l in p.stderr:
            now = time.perf_counter()
            lines.append((now, l.rstrip()))
            if t0[0] is None and '] loaded in ' in l:
                t0[0] = now
    th = threading.Thread(target=read, daemon=True)
    th.start()
    deadline = time.perf_counter() + 180
    while t0[0] is None and p.poll() is None and time.perf_counter() < deadline:
        time.sleep(0.02)
    out, wrote = {}, []
    if t0[0] is not None:
        for kind, step, at, write, _ in steps():
            while time.perf_counter() < t0[0] + at:
                time.sleep(0.005)
            n = len(lines)
            write()
            wrote.append((kind, step, time.perf_counter(), n))
    try:
        stdout = p.stdout.read()
        p.wait(timeout=120)
    except subprocess.TimeoutExpired:
        p.kill()
        stdout = ''
    th.join(5)
    try:
        run = json.loads(stdout.strip().splitlines()[-1])
    except Exception:
        print(json.dumps({'ok': False, 'error': 'the run gave no JSON line', 'stderr': [l for _, l in lines[-15:]]}))
        return 1
    with open(f'{R}/stderr.txt', 'w', encoding='utf-8') as f:
        f.write('\n'.join(l for _, l in lines))
    sheet = Image.open(f'{R}/sheet.png').convert('RGB')
    frames = cut(sheet)
    spikes = [s for s in run.get('spikes', []) if s.get('frame_ms', 0) > HITCH]
    pat = {(k, s): rx for k, s, _, _, rx in steps()}
    for kind, step, wall, n in wrote:
        o = out.setdefault(kind, {})
        tw = wall - t0[0]
        got = next(((w, l) for w, l in lines[n:] if re.search(pat[(kind, step)], l)), None)
        r = {'write_t': round(tw, 2)}
        if got:
            r['delay_ms'] = round((got[0] - wall) * 1000)
            r['line'] = got[1][got[1].find('] ') + 2:][:200]
        else:
            r['delay_ms'], r['line'] = None, None
        # a line after the write that answers a different step (a refusal taken as a load) is a failure
        r['spikes'] = [round(s['frame_ms'], 1) for s in spikes if tw - 0.5 <= s['t'] <= tw + BOUND + 0.5]
        if kind == 'textures':
            # (the model and the fire stand below this view's frame: their swap is checked by its line only)
            # the frame just before the write, against one before that (the scene's own motion) and one 2 s after
            # (frame k is taken at about t = k * CAP, a little later each time: one frame back for the drift)
            i = int(tw / CAP) - 1
            j = int((tw + 2.0) / CAP)
            if 1 <= i and j < len(frames):
                r['diff_pct'], r['control_pct'] = changed(frames[i], frames[j]), changed(frames[i - 1], frames[i])
        o[step] = r
    fails = []
    for kind, o in out.items():
        for step, r in o.items():
            if r['delay_ms'] is None:
                fails.append(f'{kind} {step}: no stderr line /{pat[(kind, step)]}/ after the write')
            elif step != 'bad' and r['delay_ms'] > BOUND * 1000:
                fails.append(f'{kind} {step}: {r["delay_ms"]} ms from the write to the swap, bound {BOUND * 1000:.0f}')
            if r['spikes']:
                fails.append(f'{kind} {step}: frames over {HITCH} ms near the reload: {r["spikes"]}')
            if 'diff_pct' in r:
                moved = r['diff_pct'] > max(5.0, 3 * r['control_pct'])  # the scene's own motion (trees, water) moves ~3%
                if step == 'bad' and moved:
                    fails.append(f'{kind} bad: the picture changed ({r["diff_pct"]}% against {r["control_pct"]}%)')
                if step != 'bad' and not moved:
                    fails.append(f'{kind} {step}: the picture did not change ({r["diff_pct"]}% against {r["control_pct"]}%)')
            elif kind == 'textures':
                fails.append(f'{kind} {step}: no filmstrip frames around t={r["write_t"]}')
    if t0[0] is None:
        fails.append('the run never loaded')
    verdict = {'ok': not fails, **out, 'frames': len(frames), 'spikes_over_16_7': len(spikes), 'budget_ok': run.get('budget_ok'), 'audio': run.get('audio'), 'fails': fails}
    print(json.dumps(verdict))
    return 0 if not fails else 1


def cut(sheet):
    """The filmstrip's frames: 6 a row of 16:9 frames, left to right and top to bottom (bench.rs); the
    black tiles after the last frame dropped."""
    w = sheet.width // 6
    h = w * 9 // 16
    out = [sheet.crop((x, y, x + w, y + h)) for y in range(0, sheet.height - h + 1, h) for x in range(0, 6 * w, w)]
    while out and out[-1].getbbox() is None:
        out.pop()
    return out


def changed(a, b):
    """The share of pixels, in percent, that differ by more than 24 of 255 in a channel."""
    r, g, b_ = ImageChops.difference(a, b).split()
    d = ImageChops.lighter(ImageChops.lighter(r, g), b_).point(lambda v: 255 if v > 24 else 0)
    return round(sum(d.histogram()[255:]) / (a.width * a.height) * 100, 2)


if __name__ == '__main__':
    sys.exit(main())
