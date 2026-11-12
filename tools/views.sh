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
grep -v '^#' tests/views/views.txt | grep '|' | while IFS='|' read -r name allowed args; do
  name=$(echo $name); allowed=$(echo $allowed)
  line=$(timeout 240 ${TECHNOAPE:-target/release/technoape.exe} run=stand $args shot=out/views/$name.png baseline=tests/views/$name.png 2>/dev/null | tail -1)
  echo "$name|$allowed|$args|$line"
done | python -I -c "
import json, sys
views, fails = {}, []
for row in sys.stdin.read().splitlines():
    name, allowed, args, line = row.split('|', 3)
    try:
        s = json.loads(line)['shot']
    except Exception:
        fails.append(f'{name}: no shot (repro: technoape.exe run=stand {args.strip()} shot=out/views/{name}.png)'); continue
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
