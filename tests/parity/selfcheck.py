import sys, random, threading
import cmp, fuzz
def selfcheck(seed, prog_a, prog_b, n_actions=30):
    r = random.Random(seed)
    opts = r.choice(fuzz.OPTS); size = r.choice([(80, 24), (80, 24), (60, 16), (100, 30), (40, 12)])
    keys = []
    for i in range(n_actions):
        keys += fuzz.action(r)
        if i % 6 == 5: keys.append("SNAP")
    out = {}
    def go(prog, name):
        o = opts
        out[name] = cmp.run(prog, name + "s%d" % seed, o, keys, size=size)[0]
    ta = threading.Thread(target=go, args=(prog_a, "a")); tb = threading.Thread(target=go, args=(prog_b, "b"))
    ta.start(); tb.start(); ta.join(); tb.join()
    return [i for i, (x, y) in enumerate(zip(out["a"], out["b"])) if not cmp.shots_equal(x, y)]
P = {"n": cmp.NANO, "k": cmp.ZU}
for s in sys.argv[3:]:
    print(s, sys.argv[1], sys.argv[2], selfcheck(int(s), P[sys.argv[1]], P[sys.argv[2]]))
