import sys, random, threading
import cmp, fuzz
def differs(opts, size, keys, tag):
    out = {}
    def go(prog, name):
        o = opts
        out[name] = cmp.run(prog, name + tag, o, keys, size=size)[0]
    ta = threading.Thread(target=go, args=(cmp.NANO, "n")); tb = threading.Thread(target=go, args=(cmp.ZU, "k"))
    ta.start(); tb.start(); ta.join(); tb.join()
    return not cmp.shots_equal(out["n"][-1], out["k"][-1]), out
seed = int(sys.argv[1]); r = random.Random(seed)
opts = r.choice(fuzz.OPTS); size = r.choice([(80, 24), (80, 24), (60, 16), (100, 30), (40, 12)])
acts = [fuzz.action(r) for _ in range(30)]
lo, hi = 0, len(acts)
flat = lambda a: [k for x in a for k in x]
while lo < hi:
    mid = (lo + hi) // 2
    d, _ = differs(opts, size, flat(acts[:mid + 1]), "b%d" % seed)
    if d: hi = mid
    else: lo = mid + 1
keys = acts[:lo + 1]
i = 0
while i < len(keys) - 1:
    trial = keys[:i] + keys[i + 1:]
    d, _ = differs(opts, size, flat(trial), "b%d" % seed)
    if d: keys = trial
    else: i += 1
d, out = differs(opts, size, flat(keys), "b%d" % seed)
print("SEED", seed, "opts=%r size=%r" % (opts, size), "minimal:", keys, d)
print(out["n"][-1]); print(out["k"][-1])
