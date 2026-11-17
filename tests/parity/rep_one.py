import sys, random, threading, json
import cmp, fuzzt as fuzz
seed = int(sys.argv[1]); upto = int(sys.argv[2]); term = sys.argv[3]
r = random.Random(seed)
opts = r.choice(fuzz.OPTS); size = r.choice([(80, 24), (80, 24), (60, 16), (100, 30), (40, 12)])
acts = [fuzz.action(r) for _ in range(30)]
keys = [k for a in acts[:upto + 1] for k in a] + ["SNAP"]
out = {}
def go(prog, name):
    o = opts
    out[name] = cmp.run(prog, name + "q%d" % seed, o, keys, size=size, term=term)[0]
ta = threading.Thread(target=go, args=(cmp.NANO, "n")); tb = threading.Thread(target=go, args=(cmp.ZU, "k"))
ta.start(); tb.start(); ta.join(); tb.join()
print(json.dumps(acts[:upto + 1]))
print("gleich" if cmp.shots_equal(out["n"][-1], out["k"][-1]) else "VERSCHIEDEN")
print(out["n"][-1]); print(out["k"][-1])
