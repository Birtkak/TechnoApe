"""The review page (tools/review/workbench.html) filled with the game's own data: `python -I tools/review_pages.py`
writes out/review/workbench.html, one page with a bench for blocks and one for sounds. The sounds are made in the page by a port of
crates/client/src/audio.rs `render`; the texels by a port of shader.wgsl `texel`."""
import json, os, subprocess, tomllib

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
load = lambda p: tomllib.load(open(os.path.join(root, p), 'rb'))
built = subprocess.run(['git', 'log', '-1', '--format=%h %cs'], cwd=root, capture_output=True, text=True).stdout.strip()
data = json.dumps({'sounds': load('data/sounds.toml')['sound'], 'blocks': load('data/blocks.toml')['block'], 'built': built})
os.makedirs(os.path.join(root, 'out/review'), exist_ok=True)
for name in ['workbench.html']:
    src = os.path.join(root, 'tools/review', name)
    if not os.path.exists(src):
        continue
    page = open(src, encoding='utf-8').read()
    a, b = page.index('/*DATA*/'), page.index('/*END*/')
    out = os.path.join(root, 'out/review', name)
    open(out, 'w', encoding='utf-8', newline='\n').write(page[:a] + data.replace('</', '<' + chr(92) + '/') + page[b + 7:])
    filled = open(out, encoding='utf-8').read()
    # (the published page gets its head from the host; this copy brings its own, to open from disk)
    open(out.replace('.html', '.local.html'), 'w', encoding='utf-8', newline='\n').write(
        '<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">'
        '<style>body{margin:0;font:14px system-ui}[hidden]{display:none!important}img{max-width:100%}</style></head><body>' + filled + '</body></html>')
    print(out, len(page))
