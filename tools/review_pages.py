"""The review pages (tools/review/*.html) filled with the game's own data: `python -I tools/review_pages.py`
writes out/review/sound-bench.html and block-bench.html. The sounds are made in the page by a port of
crates/client/src/audio.rs `render`; the texels by a port of shader.wgsl `texel`."""
import json, os, subprocess, tomllib

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
load = lambda p: tomllib.load(open(os.path.join(root, p), 'rb'))
built = subprocess.run(['git', 'log', '-1', '--format=%h %cs'], cwd=root, capture_output=True, text=True).stdout.strip()
data = json.dumps({'sounds': load('data/sounds.toml')['sound'], 'blocks': load('data/blocks.toml')['block'], 'built': built})
os.makedirs(os.path.join(root, 'out/review'), exist_ok=True)
for name in ['sound-bench.html', 'block-bench.html']:
    src = os.path.join(root, 'tools/review', name)
    if not os.path.exists(src):
        continue
    page = open(src, encoding='utf-8').read()
    a, b = page.index('/*DATA*/'), page.index('/*END*/')
    out = os.path.join(root, 'out/review', name)
    open(out, 'w', encoding='utf-8', newline='\n').write(page[:a] + data.replace('</', '<' + chr(92) + '/') + page[b + 7:])
    print(out, len(page))
