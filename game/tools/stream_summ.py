# ADR 0135: medians of stream_bench.sh runs (stream_probe=1 JSON lines): holes in the box by 24 m band (of the probe's
# ground patches), queued generation, frame p99 / max, frames over 8.3 ms, data blocks near the player never loaded,
# generation us per task, worker time per stage a second (our godot_voxel build only).
import json, statistics as st, sys

rows = []
for f in sys.argv[1:]:
    d = json.load(open(f))
    ser = [r for r in d.get('stream', {}).get('series', []) if r['t'] >= 0]
    m = lambda k: st.mean([r.get(k, 0) for r in ser]) if ser else -1
    hole = lambda b: st.mean([r['holes'].get(b, 0) for r in ser]) if ser else -1
    rows.append({'run': f, 'holes0': hole('0'), 'holes24': hole('24'), 'holes48': hole('48'), 'gen_q': m('gen_q'),
                 'p99': d['p99_frame_ms'], 'max': d['max_frame_ms'], 'over8': d['frames_over_8_3'],
                 'near_unloaded': sum(d['near_unloaded'].get('by_band_m', {}).values()),
                 'gen_us': d['generation']['task_us'] / max(1, d['generation']['tasks']),
                 'gen_ms': m('gen_ms'), 'mesh_ms': m('mesh_ms'), 'idle_ms': m('idle_ms'),
                 'threads': d['voxel_backlog']['threads'], 'others': d['other_godot_processes'], 'patches': ser[0]['patches'] if ser else 0})
for r in rows:
    print(' '.join(f'{k} {v:.1f}' if isinstance(v, float) else f'{k} {v}' for k, v in r.items()))
if len(rows) > 1:
    print('median', {k: round(st.median([r[k] for r in rows]), 2) for k in rows[0] if k != 'run'})
