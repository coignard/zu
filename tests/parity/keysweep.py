import sys, subprocess, re, threading, os, queue
import cmp
def caps(term):
    out = subprocess.run(["infocmp", "-1", "-x", term], capture_output=True, text=True).stdout
    res = []
    for line in out.splitlines():
        m = re.match(r"\s*(k\w+)=(.*),$", line)
        if m: res.append((m.group(1), decode(m.group(2))))
    return res
def decode(v):
    b = bytearray(); i = 0
    while i < len(v):
        c = v[i]
        if c == "\\":
            n = v[i + 1]
            if n in "Ee": b.append(27); i += 2
            elif n == "s": b.append(32); i += 2
            elif n in "nl": b.append(10); i += 2
            elif n == "r": b.append(13); i += 2
            elif n == "t": b.append(9); i += 2
            elif n == "b": b.append(8); i += 2
            elif n == "f": b.append(12); i += 2
            elif n.isdigit():
                o = v[i + 1:i + 4]; b.append(int(o, 8) & 0xFF); i += 4
            else: b.append(ord(n)); i += 2
        elif c == "^":
            n = v[i + 1]; b.append(127 if n == "?" else ord(n.upper()) & 0x1F); i += 2
        else: b.extend(c.encode()); i += 1
    return bytes(b)
def sweep(term, jobs=4):
    items = caps(term)
    q = queue.Queue(); [q.put(x) for x in items]; bad = []; lock = threading.Lock()
    def worker(wid):
        while True:
            try: cap, seq = q.get_nowait()
            except queue.Empty: return
            hexs = " ".join("%02x" % x for x in seq)
            keys = ["C-_", "L:9,6", "Enter", "WAIT0.2", "H:" + hexs, "SNAP", "C-c", "SNAP", "L:Z"]
            for attempt in range(2):
                a, _ = cmp.run(cmp.NANO, "sn%d" % wid, "", keys, term=term)
                b, _ = cmp.run(cmp.ZU, "sk%d" % wid, "", keys, term=term)
                if all(cmp.shots_equal(x, y) for x, y in zip(a, b)):
                    break
            diffs = []
            for i, (x, y) in enumerate(zip(a, b)):
                if not cmp.shots_equal(x, y):
                    xl, yl = cmp.norm(x).split("\n"), cmp.norm(y).split("\n")
                    d = [(j, p, q2) for j, (p, q2) in enumerate(zip(xl, yl)) if not cmp.lines_equal(p, q2)]
                    diffs.append("   Bild %d: " % i + " ; ".join("Z%d nano|%s| zu|%s|" % (j, p.strip()[:70], q2.strip()[:70]) for j, p, q2 in d[:3]))
            if diffs:
                with lock: bad.append((cap, seq, diffs))
    ts = [threading.Thread(target=worker, args=(w,)) for w in range(jobs)]
    [t.start() for t in ts]; [t.join() for t in ts]
    return items, bad
if __name__ == "__main__":
    for term in sys.argv[1:]:
        items, bad = sweep(term)
        print("=== %s: %d Tasten, %d abweichend" % (term, len(items), len(bad)))
        for cap, seq, diffs in sorted(bad):
            print("  %s %r" % (cap, seq)); print("\n".join(diffs))
        sys.stdout.flush()
