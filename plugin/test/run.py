"""Host tests for zipx: builds zips with Python's zipfile, runs zipx_cli on each."""
import os, shutil, subprocess, sys, tempfile, zipfile

CLI = sys.argv[1]
root = tempfile.mkdtemp(prefix="zipx_")
fails = 0

def run(zp, dest, ow=0, *extra, free=None):
    env = dict(os.environ, PLAT_FREE=str(free)) if free is not None else None
    return int(subprocess.run([CLI, zp, dest, str(ow), *extra], capture_output=True, text=True, env=env).stdout.strip())

def check(name, cond):
    global fails
    print(("ok   " if cond else "FAIL ") + name)
    fails += 0 if cond else 1

def mk(name, entries, method=zipfile.ZIP_DEFLATED, z64=False):
    p = os.path.join(root, name + ".zip")
    with zipfile.ZipFile(p, "w", method) as z:
        for n, data in entries:
            zi = zipfile.ZipInfo(n)
            zi.compress_type = method
            with z.open(zi, "w", force_zip64=z64) as f:
                f.write(data)
    return p

big = bytes((i * 7) % 251 for i in range(300000))  # spans several 32 KB chunks
files = [("a.txt", b"hello"), ("dir/sub/b.bin", big), ("dir/empty", b""), ("dir/", b"")]

for label, method, z64 in [("deflate", zipfile.ZIP_DEFLATED, False), ("stored", zipfile.ZIP_STORED, False), ("zip64", zipfile.ZIP_DEFLATED, True)]:
    zp = mk("ok_" + label, files, method, z64)
    d = os.path.join(root, "out_" + label)
    check(f"{label}: rc 0", run(zp, d) == 0)
    check(f"{label}: content", open(d + "/a.txt", "rb").read() == b"hello" and open(d + "/dir/sub/b.bin", "rb").read() == big and os.path.getsize(d + "/dir/empty") == 0)
    check(f"{label}: no .vfpart left", not any(f.endswith(".vfpart") for _, _, fs in os.walk(d) for f in fs))

# ZIP Slip: nothing at all may be written, not even the safe entries
zp = mk("slip", [("safe.txt", b"x"), ("../evil.txt", b"x")])
d = os.path.join(root, "out_slip")
check("zip slip: rejected (-3)", run(zp, d) == -3)
check("zip slip: nothing written", not os.path.exists(d + "/safe.txt") and not os.path.exists(os.path.join(root, "evil.txt")))

# Corrupt data: CRC mismatch -> error, no final file
zp = mk("crc", [("c.bin", big)], zipfile.ZIP_STORED)
raw = bytearray(open(zp, "rb").read()); raw[100] ^= 0xFF; open(zp, "wb").write(raw)
d = os.path.join(root, "out_crc")
check("corrupt: CRC error (-5)", run(zp, d) == -5)
check("corrupt: no final or .vfpart file", not os.path.exists(d + "/c.bin") and not os.path.exists(d + "/c.bin.vfpart"))

# Truncated archive
zp = mk("trunc", files)
open(zp, "wb").write(open(zp, "rb").read()[:-40])
check("truncated: format error (-2)", run(zp, os.path.join(root, "out_trunc")) == -2)

# Not a zip / missing file
open(os.path.join(root, "junk.zip"), "wb").write(b"not a zip" * 20)
check("junk: format error (-2)", run(os.path.join(root, "junk.zip"), os.path.join(root, "out_junk")) == -2)
check("missing: io error (-1)", run(os.path.join(root, "nope.zip"), os.path.join(root, "out_nope")) == -1)

# Overwrite protection and resume after an interrupt
zp = mk("ow", [("keep.txt", b"new"), ("other.txt", b"new")])
d = os.path.join(root, "out_ow"); os.makedirs(d)
open(d + "/keep.txt", "w").write("old")
check("no-overwrite: rc 0", run(zp, d, 0) == 0)
check("no-overwrite: existing kept, missing created", open(d + "/keep.txt").read() == "old" and open(d + "/other.txt").read() == "new")
check("overwrite: replaced", run(zp, d, 1) == 0 and open(d + "/keep.txt").read() == "new")
open(d + "/other.txt.vfpart", "w").write("half")  # leftover of a crashed run
os.remove(d + "/other.txt")
check("resume: leftover .vfpart replaced", run(zp, d, 0) == 0 and open(d + "/other.txt").read() == "new" and not os.path.exists(d + "/other.txt.vfpart"))

# Unsafe ':' is rejected; a leading "./" stays inside the destination
check("colon: rejected (-3)", run(mk("colon", [("ux0:evil", b"x")]), os.path.join(root, "out_colon")) == -3)
d = os.path.join(root, "out_dot")
check("dot-slash: extracted inside dest", run(mk("dot", [("./a.txt", b"x")]), d) == 0 and os.path.exists(d + "/a.txt"))

# Cancel from the progress callback (-6) leaves nothing half-written
d = os.path.join(root, "out_cancel")
check("cancel: rc -6", run(mk("cancel", [("c.bin", big)]), d, 0, "cancel") == -6)
check("cancel: no final or .vfpart file", os.listdir(d) == [])

# More than 65535 entries (ZIP64 end of central directory)
zp = mk("many", [(f"m/{i}", b"") for i in range(66000)], zipfile.ZIP_STORED)
d = os.path.join(root, "out_many")
check("many entries: all extracted", run(zp, d) == 0 and len(os.listdir(d + "/m")) == 66000)

# Free space: existing files don't count twice
two = [("a.bin", bytes(3_000_000)), ("b.bin", bytes(3_000_000))]
zp = mk("space", two)
d = os.path.join(root, "out_space")
check("space: too small (-4)", run(zp, d, 0, free=5_000_000) == -4)
check("space: fits", run(zp, d, 0, free=8_000_000) == 0)
check("space: nothing new to write", run(zp, d, 0, free=1_100_000) == 0)
check("space: update needs only one file of headroom", run(zp, d, 1, free=4_200_000) == 0)
check("space: update without headroom (-4)", run(zp, d, 1, free=2_000_000) == -4)

# Paths longer than 512 bytes
long = "/".join(["x" * 100] * 7) + "/f"
d = os.path.join(root, "out_long")
check("long path: extracted", run(mk("long", [(long, b"x")]), d) == 0)

shutil.rmtree(root)
print("zipx: all passed" if not fails else f"zipx: {fails} FAILED")
sys.exit(1 if fails else 0)
