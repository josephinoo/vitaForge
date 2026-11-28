"""Host tests for the queue state machine (worker.c) using file:// downloads."""
import hashlib, os, shutil, subprocess, sys, tempfile, zipfile

CLI = sys.argv[1]
root = tempfile.mkdtemp(prefix="worker_")
fails = 0
n_case = 0

def check(name, cond):
    global fails
    print(("ok   " if cond else "FAIL ") + name)
    fails += 0 if cond else 1

def new_case():
    global n_case
    n_case += 1
    q = os.path.join(root, f"q{n_case}"); os.makedirs(q)
    return q + "/", os.path.join(root, f"work{n_case}")

def zipfile_at(path, entries):
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for n, d in entries: z.writestr(n, d)
    return path

def job(q, work, jid, url, kind="data", md5="", size=0, ow=0):
    os.makedirs(work, exist_ok=True)
    open(f"{q}{jid}.job", "w").write(
        f"url={url}\ndest={work}/{jid}.zip\nextract_to={work}/out\nname=T\nmd5={md5}\nkind={kind}\nsize={size}\noverwrite={ow}\ncreated=1\n")

def state(q, jid):
    kv = {}
    for l in open(f"{q}{jid}.state").read().splitlines():
        k, _, v = l.partition("="); kv[k] = v
    return kv

def run(q, ticks=10, fail=0):
    out = subprocess.run([CLI, q, str(ticks), str(fail)], capture_output=True, text=True, env={**os.environ, "PLAT_FAST": "1"}).stdout.split()
    return int(out[0]), int(out[1])

big = bytes((i * 13) % 255 for i in range(200000))
good = zipfile_at(os.path.join(root, "good.zip"), [("a.txt", b"hi"), ("d/b.bin", big)])
good_md5 = hashlib.md5(open(good, "rb").read()).hexdigest()
good_size = os.path.getsize(good)

# 1. happy path: download + md5 + extract, zip cleaned up, one tick does it all
q, w = new_case(); job(q, w, "j1", "file://" + good, md5=good_md5.upper(), size=good_size)
run(q)
s = state(q, "j1")
check("happy: completed", s["status"] == "completed")
check("happy: extracted and zip removed", open(w + "/out/d/b.bin", "rb").read() == big and not os.path.exists(w + "/j1.zip") and not os.path.exists(w + "/j1.zip.part"))
check("happy: progress recorded", int(s["downloaded"]) == good_size and int(s["extract_done"]) > 0)
check("happy: heartbeat written", os.path.getsize(q + "heartbeat") > 0)
check("happy: completed job isn't re-run", run(q)[0] == 0)

# 2. md5 mismatch -> failed, bad download removed
q, w = new_case(); job(q, w, "j2", "file://" + good, md5="0" * 32)
run(q); s = state(q, "j2")
check("md5 mismatch: failed", s["status"] == "failed" and "md5" in s["error"] and not os.path.exists(w + "/j2.zip"))

# 3. HTTP error is fatal, no retries
q, w = new_case(); job(q, w, "j3", "file://FAIL_404")
check("404: failed after one call", run(q) == (1, 1) and state(q, "j3")["status"] == "failed")

# 4. transient network errors are retried
q, w = new_case(); job(q, w, "j4", "file://" + good)
run(q, fail=2); check("retry: recovers after 2 net errors", state(q, "j4")["status"] == "completed")
q, w = new_case(); job(q, w, "j4b", "file://FAIL_NET")
check("retry: gives up after 5 tries", run(q) == (1, 5) and state(q, "j4b")["status"] == "failed" and "network" in state(q, "j4b")["error"])

# 5. cancel before start
q, w = new_case(); job(q, w, "j5", "file://" + good); open(q + "j5.ctl", "w").write("cancel")
run(q); check("cancel: cancelled and ctl consumed", state(q, "j5")["status"] == "cancelled" and not os.path.exists(q + "j5.ctl") and not os.path.exists(w + "/out"))

# 6. pause, then resume
q, w = new_case(); job(q, w, "j6", "file://" + good); open(q + "j6.ctl", "w").write("pause")
run(q); check("pause: paused, not started", state(q, "j6")["status"] == "paused" and not os.path.exists(w + "/j6.zip.part"))
check("pause: stays paused", run(q)[0] == 0 and state(q, "j6")["status"] == "paused")
open(q + "j6.ctl", "w").write("resume")
run(q); check("resume: completes", state(q, "j6")["status"] == "completed" and os.path.exists(w + "/out/a.txt"))

# 7. vpk kind keeps the file
q, w = new_case(); job(q, w, "j7", "file://" + good, kind="vpk")
run(q); check("vpk: completed, file kept, not extracted", state(q, "j7")["status"] == "completed" and os.path.exists(w + "/j7.zip") and not os.path.exists(w + "/out"))

# 8. resume a partial download from the .part file
q, w = new_case(); job(q, w, "j8", "file://" + good, md5=good_md5); os.makedirs(w, exist_ok=True)
open(w + "/j8.zip.part", "wb").write(open(good, "rb").read()[:70000])
run(q); check("partial: resumed, md5 ok, extracted", state(q, "j8")["status"] == "completed" and open(w + "/out/d/b.bin", "rb").read() == big)

# 9. zip slip -> failed, nothing extracted
evil = zipfile_at(os.path.join(root, "evil.zip"), [("ok.txt", b"x"), ("../../pwned", b"x")])
q, w = new_case(); job(q, w, "j9", "file://" + evil)
run(q); s = state(q, "j9")
check("zip slip: failed with message", s["status"] == "failed" and "unsafe" in s["error"] and not os.path.exists(w + "/out/ok.txt") and not os.path.exists(root + "/pwned"))

# 10. crash recovery: state says extracting, zip already on disk, no .part
q, w = new_case(); job(q, w, "j10", "file://" + good, size=good_size); os.makedirs(w, exist_ok=True)
shutil.copy(good, w + "/j10.zip")
open(q + "j10.state", "w").write("status=extracting\ndownloaded=1\ntotal=1\nextract_done=5\nextract_total=9\nupdated=1\nerror=\n")
r = run(q); check("recovery: restarts extraction without downloading", state(q, "j10")["status"] == "completed" and r[1] == 0)

# 11. garbage job file
q, w = new_case(); open(q + "bad.job", "w").write("nonsense")
run(q); check("invalid job: reported failed", state(q, "bad")["status"] == "failed")

# 12. native-bar job: waits without downloading, extracts once the file is delivered
q, w = new_case(); job(q, w, "j12", "file://" + good, size=good_size)
open(q + "j12.job", "a").write("bgdl=1\n")
r = run(q); check("native: waits for the shell", r == (0, 0) and state(q, "j12")["status"] == "downloading")
shutil.copy(good, w + "/j12.zip")
r = run(q); check("native: extracts the delivered file", state(q, "j12")["status"] == "completed" and r[1] == 0 and os.path.exists(w + "/out/a.txt"))

shutil.rmtree(root)
print("worker: all passed" if not fails else f"worker: {fails} FAILED")
sys.exit(1 if fails else 0)
