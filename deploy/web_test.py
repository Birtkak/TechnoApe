"""python -I deploy/web_test.py: web.py around a stand-in server; the page, the log and the download answer."""
import os, subprocess, sys, tempfile, time, urllib.error, urllib.request

here = os.path.dirname(os.path.abspath(__file__))
tmp = tempfile.mkdtemp()
with open(os.path.join(tmp, "TechnoApe.zip"), "wb") as f:
    f.write(b"PK-stand-in")
with open(os.path.join(tmp, "version.txt"), "w") as f:
    f.write("abc1234 <test>")
fake = "import time\nprint('[server] hello', flush=True)\nprint('{\"tick\":7,\"players\":2}', flush=True)\ntime.sleep(30)"
env = dict(os.environ, WEB_PORT="17780", CLIENT_ZIP=os.path.join(tmp, "TechnoApe.zip"))
p = subprocess.Popen([sys.executable, os.path.join(here, "web.py"), sys.executable, "-c", fake], env=env, stdout=subprocess.DEVNULL)
try:
    get = lambda path: urllib.request.urlopen("http://127.0.0.1:17780" + path, timeout=5).read()
    for _ in range(50):
        time.sleep(0.1)
        try:
            log = get("/log").decode()
            if "hello" in log and "tick" in log:
                break
        except OSError:
            pass
    status, rest = log.split("\n", 1)
    assert status == '{"tick":7,"players":2}', status  # the stats line is the status, not a log line
    assert rest.endswith("[server] hello\n") and "tick" not in rest, rest
    page = get("/").decode()
    assert "abc1234 &lt;test&gt;" in page and "/TechnoApe.zip" in page, page
    assert get("/TechnoApe.zip") == b"PK-stand-in"
    assert get("/version.txt") == b"abc1234 <test>"
    try:
        get("/../web.py")
        raise AssertionError("served a path outside the three")
    except urllib.error.HTTPError as e:
        assert e.code == 404
    print("ok")
finally:
    p.kill()
