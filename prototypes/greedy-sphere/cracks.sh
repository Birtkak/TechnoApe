#!/usr/bin/env bash
# Hunts cracks close up (README.md): stands at a face seam, a cube corner and a level border, at eye height
# and higher, looking four ways a little down, and counts the crack pixels of each shot (ADR 0018's counter).
# Usage, from the repository: bash prototypes/greedy-sphere/cracks.sh <exe> <tag> [greedy=1 ...]
cd "$(dirname "$0")/../.."
exe=${1:?exe}; tag=${2:?tag}; shift 2
mkdir -p out/greedy/$tag
total=0
for spot in seam:-1,1,-0.85 corner:-1,1,-1 border:0.3,1,0.2; do
  name=${spot%%:*}; at=${spot#*:}
  for eye in ${EYES:-1.7 12 60}; do
    for yaw in 0 90 180 270; do
      line=$(timeout 120 $exe run=stand at=$at eye=$eye yaw=$yaw pitch=${PITCH:--12} "$@" shot=out/greedy/$tag/$name-$eye-$yaw.png 2>/dev/null | tail -1)
      c=$(echo "$line" | python -I -c "import json,sys; print(json.loads(sys.stdin.read())['shot']['cracks'])" 2>/dev/null || echo x)
      echo "$name eye=$eye yaw=$yaw cracks=$c"
    done
  done
done
