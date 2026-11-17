#!/usr/bin/env python3
import sys, random, threading, os
import cmp, fuzzt as fuzz
def first_diff(seed, n_actions=30, show=True):
    r = random.Random(seed)
    opts = r.choice(fuzz.OPTS); size = r.choice([(80, 24), (80, 24), (60, 16), (100, 30), (40, 12)])
    acts = [fuzz.action(r) for _ in range(n_actions)]
    keys = []
    for a in acts: keys += a + ["SNAP"]
    out = {}
    def go(prog, name):
        o = opts
        out[name] = cmp.run(prog, name + "r%d" % seed, o, keys, size=size, term=fuzz.TERM)[0]
    ta = threading.Thread(target=go, args=(cmp.NANO, "n")); tb = threading.Thread(target=go, args=(cmp.ZU, "k"))
    ta.start(); tb.start(); ta.join(); tb.join()
    for i, (x, y) in enumerate(zip(out["n"], out["k"])):
        if not cmp.shots_equal(x, y):
            res = "SEED %d opts=%r size=%r: erste Abweichung nach Aktion %d: %r\n  davor: %r\n" % (seed, opts, size, i, acts[i] if i < len(acts) else None, acts[max(0, i - 4):i])
            if show:
                res += "--- nano vorher:\n" + (out["n"][i - 1] if i else "") + "--- nano:\n" + x + "--- zu:\n" + y
            return res
    return "SEED %d: keine Abweichung im Einzelschrittmodus" % seed
if __name__ == "__main__":
    for s in sys.argv[1:]:
        print(first_diff(int(s)))
