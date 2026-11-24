"""Host tests for zipx: builds zips with Python's zipfile, runs zipx_cli on each."""
import os, shutil, subprocess, sys, tempfile, zipfile

CLI = sys.argv[1]
root = tempfile.mkdtemp(prefix="zipx_")
fails = 0

def run(zp, dest, ow=0):
    return int(subprocess.run([CLI, zp, dest, str(ow)], capture_output=True, text=True).stdout.strip())

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

shutil.rmtree(root)
print("zipx: all passed" if not fails else f"zipx: {fails} FAILED")
sys.exit(1 if fails else 0)
