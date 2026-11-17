import sys, threading, queue
import cmp
def sequences():
    seqs = set()
    finals = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcde~^$@"
    for p in [""] + [str(i) for i in range(1, 10)]:
        for f in finals: seqs.add("\x1b[" + p + f)
    for m in range(2, 9):
        for f in "ABCDEFHPQRS~": seqs.add("\x1b[1;%d%s" % (m, f))
        for n in "2345678": seqs.add("\x1b[%s;%d~" % (n, m))
    for n in range(10, 35):
        seqs.add("\x1b[%d~" % n)
        for f in "^$@": seqs.add("\x1b[%d%s" % (n, f))
    for n in range(11, 25):
        for m in range(2, 9): seqs.add("\x1b[%d;%d~" % (n, m))
    for n in range(1, 9):
        for f in "^$@": seqs.add("\x1b[%d%s" % (n, f))
    for f in "ABCDE": seqs.add("\x1b[[" + f)
    for f in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz": seqs.add("\x1bO" + f)
    for d in "2345678":
        for f in "ABCDEFHMPQRSjklmnopqrstuvwxy": seqs.add("\x1bO" + d + f)
    for m in range(2, 9):
        for f in "ABCDPQRS": seqs.add("\x1bO1;%d%s" % (m, f))
    for f in "ABCDabcd": seqs.add("\x1b\x1b[" + f)
    for f in "ABCD": seqs.add("\x1b\x1bO" + f)
    for f in "ABCD": seqs.add("\x1b\x1b[1;5" + f)
    return sorted(seqs)
def run_all(items, opts, term="xterm-256color", jobs=4):
    q = queue.Queue(); [q.put(x) for x in items]; bad = []; lock = threading.Lock()
    def worker(wid):
        while True:
            try: seq = q.get_nowait()
            except queue.Empty: return
            hexs = " ".join("%02x" % ord(c) for c in seq)
            keys = ["C-_", "L:9,6", "Enter", "WAIT0.2", "H:" + hexs, "SNAP", "C-c", "SNAP", "L:Z"]
            for attempt in range(2):
                a, _ = cmp.run(cmp.NANO, "fn%d" % wid, opts, keys, term=term)
                b, _ = cmp.run(cmp.ZU, "fk%d" % wid, opts, keys, term=term)
                if all(cmp.shots_equal(x, y) for x, y in zip(a, b)):
                    break
            diffs = []
            for i, (x, y) in enumerate(zip(a, b)):
                if not cmp.shots_equal(x, y):
                    xl, yl = cmp.norm(x).split("\n"), cmp.norm(y).split("\n")
                    d = [(j, p, q2) for j, (p, q2) in enumerate(zip(xl, yl)) if not cmp.lines_equal(p, q2)]
                    diffs.append("   Bild %d: " % i + " ; ".join("Z%d nano|%s| zu|%s|" % (j, p.strip()[:60], q2.strip()[:60]) for j, p, q2 in d[:3]))
            if diffs:
                with lock: bad.append((seq, diffs))
    ts = [threading.Thread(target=worker, args=(w,)) for w in range(jobs)]
    [t.start() for t in ts]; [t.join() for t in ts]
    return bad
if __name__ == "__main__":
    items = sequences()
    bad = run_all(items, "-K")
    print("=== Fallback (-K): %d Folgen, %d abweichend" % (len(items), len(bad)))
    for seq, diffs in sorted(bad):
        print("  %r" % seq); print("\n".join(diffs))
