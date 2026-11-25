#!/usr/bin/env bash
# Fixed viewpoints compared by number (ADR 0018): renders every view in tests/views/views.txt, counts its
# cracks and the pixels that differ from tests/views/<name>.png, prints one JSON verdict, exit 1 on a failure.
# Usage: bash tools/views.sh            check (about 10 s)
#        bash tools/views.sh accept     store the shots just taken as the new baselines
cd "$(dirname "$0")/.."
if [ "$1" = accept ]; then
  for n in $(grep -v '^#' tests/views/views.txt | grep '|' | cut -d'|' -f1); do cp out/views/$n.png tests/views/ || exit 1; done
  ls -la tests/views/*.png; exit 0
fi
mkdir -p out/views
# one view: its verdict row in out/views/<name>.row (stderr kept: a view with no shot says why in its .err)
view() {
  local name=$1 allowed=$2 args=$3 t0=$SECONDS line
  # (the full gate gives each view longer: it shares the PC with every test binary, and two at a time took 255 s)
  line=$(timeout ${VIEW_TIMEOUT:-240} ${TECHNOAPE:-target/release/technoape.exe} run=stand $args shot=out/views/$name.png baseline=tests/views/$name.png 2>out/views/$name.err | tail -1)
  [ -n "$line" ] && [ "${line:0:1}" = "{" ] || line="took $((SECONDS - t0)) s; stderr's end: $(tail -c 200 out/views/$name.err | tr '\n|' '  ')"
  echo "$name|$allowed|$args|$line" >out/views/$name.row
}
# two at a time: a view is mostly its client's load (about 25 s), and one after another the 12 were the quick
# gate's longest step (5 minutes)
names=()
k=0
while IFS='|' read -r name allowed args; do
  name=$(echo $name); allowed=$(echo $allowed)
  names+=("$name")
  view "$name" "$allowed" "$args" &
  k=$((k + 1)); [ $((k % 2)) = 0 ] && wait
done < <(grep -v '^#' tests/views/views.txt | grep '|')
wait
for n in "${names[@]}"; do cat out/views/$n.row; done | python -I -c "
import json, sys
views, fails = {}, []
for row in sys.stdin.read().splitlines():
    name, allowed, args, line = row.split('|', 3)
    try:
        s = json.loads(line)['shot']
    except Exception:
        fails.append(f'{name}: no shot, {line.strip()[:240]} (repro: technoape.exe run=stand {args.strip()} shot=out/views/{name}.png)'); continue
    views[name] = {'cracks': s['cracks'], 'allowed': int(allowed), 'diff_pct': s['diff_pct']}
    repro = f'technoape.exe run=stand {args.strip()} shot=out/views/{name}.png baseline=tests/views/{name}.png'
    if s['cracks'] > int(allowed):
        fails.append(f'{name}: {s[\"cracks\"]} crack pixels, {allowed} allowed, first at {s[\"crack_px\"][:3]} (repro: {repro})')
    if s['diff_pct'] is None:
        fails.append(f'{name}: no baseline; look at out/views/{name}.png, then bash tools/views.sh accept')
    elif s['diff_pct'] > 0.5:
        fails.append(f'{name}: {s[\"diff_pct\"]}% of pixels differ from the baseline; compare out/views/{name}.png with tests/views/{name}.png (repro: {repro})')
print(json.dumps({'ok': not fails, 'views': views, 'failures': fails}))
sys.exit(1 if fails else 0)
"
