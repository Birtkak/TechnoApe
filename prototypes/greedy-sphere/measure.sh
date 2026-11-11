#!/usr/bin/env bash
# Measures one build of the client with greedy merging off and on (README.md): the quads the mesher makes
# (meshbench), the quick runs' GPU memory and frame stages, and the crack pixels at the fixed viewpoints.
# Usage, from the repository: bash prototypes/greedy-sphere/measure.sh <tag>   (writes out/greedy/<tag>-*)
cd "$(dirname "$0")/../.."
tag=${1:?tag}
mkdir -p out/greedy
exe=target/release/technoape.exe
for g in 0 1; do
  $exe run=meshbench greedy=$g chunks=6 2>/dev/null | tail -1 > out/greedy/$tag-mesh-$g.json
  for r in stand fly orbit; do
    $exe run=$r quick=1 greedy=$g settings=out/x.txt out=out/greedy/$tag-$r-$g.jsonl >/dev/null 2>&1
  done
  grep -v '^#' tests/views/views.txt | grep '|' | while IFS='|' read -r name allowed args; do
    name=$(echo $name)
    line=$(timeout 120 $exe run=stand $args greedy=$g shot=out/greedy/$tag-$name-$g.png baseline=tests/views/$name.png 2>/dev/null | tail -1)
    echo "$name $line"
  done > out/greedy/$tag-views-$g.txt
done
python -I - "$tag" <<'EOF'
import json, sys
tag = sys.argv[1]
for g in '01':
    m = json.loads(open(f'out/greedy/{tag}-mesh-{g}.json').read())
    print(f'greedy={g} quads/chunk {m["quads_per_chunk"]} mesh_us {m["mesh_us"]}')
    for r in ['stand', 'fly', 'orbit']:
        s = json.loads(open(f'out/greedy/{tag}-{r}-{g}.jsonl').read().splitlines()[-1])
        gp, g99 = s['gpu_p50'], s['gpu_p99']
        print(f'  {r}: frame p50/p99 {s["frame_p50_ms"]}/{s["frame_p99_ms"]} gbuffer {gp["gbuffer"]} trace {gp["trace"]}'
              f' rt p99 {g99["rt"]} alloc {s.get("gpu_alloc_mb")} MB job_us {s["job_us_avg"]}')
    for row in open(f'out/greedy/{tag}-views-{g}.txt').read().splitlines():
        name, line = row.split(' ', 1)
        try:
            sh = json.loads(line)['shot']; print(f'  view {name}: cracks {sh["cracks"]} diff {sh["diff_pct"]}%')
        except Exception:
            print(f'  view {name}: no shot')
EOF
