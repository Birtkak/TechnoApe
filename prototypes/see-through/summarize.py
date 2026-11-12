"""Prints a markdown table from out/see-through/<series>.jsonl: the median over reps of each name.
Usage: python -I prototypes/see-through/summarize.py out/see-through/q1.jsonl"""
import json
import statistics
import sys

rows = {}
for line in open(sys.argv[1], encoding='utf-8'):
    try:
        d = json.loads(line)
    except json.JSONDecodeError:
        print('unreadable line:', line[:120], file=sys.stderr)
        continue
    if d['r'].get('frames', 0) > 0:  # a run that measured nothing (it happened once) is left out
        rows.setdefault(d['name'], []).append(d['r'])


def med(rs, f):
    v = [f(r) for r in rs]
    return statistics.median(v)


cols = [
    ('mesh us/chunk', lambda r: r['spike']['mesh_us'], '{:.0f}'),
    ('quads/chunk', lambda r: r['spike']['quads_per_chunk'], '{:.0f}'),
    ('model quads', lambda r: r['spike']['model_quads_per_chunk'], '{:.0f}'),
    # the frame thread's own work at p50, less the driver's waits, as tools/perf.py counts it
    ('frame thread work ms', lambda r: sum(v for k, v in r['cpu_p50'].items() if k not in ('acquire', 'present', 'between')), '{:.3f}'),
    ('frame p50 ms', lambda r: r['frame_p50_ms'], '{:.2f}'),
    ('gbuffer ms', lambda r: r['gpu_p50']['gbuffer'], '{:.3f}'),
    ('trace ms', lambda r: r['gpu_p50']['trace'], '{:.3f}'),
    ('rt ms', lambda r: r['gpu_p50']['rt'], '{:.3f}'),
    ('gpu ms p50', lambda r: r['gpu_ms_p50'], '{:.3f}'),
    ('p99 frame ms (best rep)', None, '{:.2f}'),
    ('VRAM MB', lambda r: r['vram_mb'], '{:.0f}'),
    ('wgpu alloc MB', lambda r: r['gpu_alloc_mb'], '{:.0f}'),
    ('quad buf MB', lambda r: r['quad_buffer_mb'], '{:.0f}'),
]
print('| run | n | ' + ' | '.join(c[0] for c in cols) + ' |')
print('|' + '---|' * (len(cols) + 2))
for name, rs in rows.items():
    cells = []
    for title, f, fmt in cols:
        try:
            cells.append(fmt.format(min(r['frame_p99_ms'] for r in rs) if f is None else med(rs, f)))
        except (KeyError, TypeError):
            cells.append('-')
    print(f'| {name} | {len(rs)} | ' + ' | '.join(cells) + ' |')
