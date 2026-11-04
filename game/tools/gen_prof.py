# Reads out/gen_prof.json (gen_bench.gd prof=1: [module base, n, n frame addresses innermost first, n, ...]) and
# prints where the samples fall, inclusive of callees: by function, and by source line of fine_gen.cpp /
# world_gen.cpp / rock_types.cpp (inlined calls are frames too). Needs the technosim DLL built with debug_symbols=yes.
#   python -I tools/gen_prof.py [dll] [json]
import collections, json, os, re, subprocess, sys

dll = sys.argv[1] if len(sys.argv) > 1 else 'addons/technosim/bin/libtechnosim.windows.template_debug.x86_64.dll'
d = json.load(open(sys.argv[2] if len(sys.argv) > 2 else 'out/gen_prof.json'))
base = d[0]
pe = subprocess.run(['objdump', '-p', dll], capture_output=True, text=True).stdout
image = int(next(l.split()[1] for l in pe.splitlines() if l.startswith('ImageBase')), 16)
samples, i = [], 1
while i < len(d):
    n = d[i]
    samples.append([a - base + image for a in d[i + 1:i + 1 + n] if 0 <= a - base < 64 << 20])
    i += 1 + n
addrs = sorted({a for s in samples for a in s})
out = subprocess.run(['addr2line', '-f', '-C', '-i', '-a', '-e', dll] + [hex(a) for a in addrs], capture_output=True, text=True).stdout.splitlines()
frames, cur = {}, None   # address -> [(function, file:line)] innermost first, inlined frames included
for l in out:
    if l.startswith('0x'): cur = int(l, 16); frames[cur] = []
    elif frames[cur] and frames[cur][-1][1] is None: frames[cur][-1] = (frames[cur][-1][0], l)
    else: frames[cur].append((l, None))
def loc(l):
    m = re.match(r'(.*):(\d+|\?)', l or '')
    return os.path.basename(m.group(1)) + ':' + m.group(2) if m else str(l)
total = len(samples)
by_fn, by_line, self_line = collections.Counter(), collections.Counter(), collections.Counter()
for s in samples:
    fns, lines = set(), set()
    for k, a in enumerate(s):
        for j, (f, l) in enumerate(frames.get(a, [])):
            fns.add(f[:90])
            L = loc(l)
            if L.split(':')[0] in ('fine_gen.cpp', 'world_gen.cpp', 'rock_types.cpp', 'ruins.cpp'):
                lines.add(L)
                if k == 0 and not any(loc(x[1]).startswith(('fine_gen', 'world_gen', 'rock_types')) for x in frames[a][:j]): self_line[L] += 1
    for f in fns: by_fn[f] += 1
    for L in lines: by_line[L] += 1
print(f'samples {total}')
for title, c, n in (('function, inclusive', by_fn, 45), ('our line, inclusive', by_line, 60), ('our line, innermost', self_line, 30)):
    print(f'--- by {title}')
    for k, v in c.most_common(n): print(f'{100 * v / total:5.1f}%  {k}')
