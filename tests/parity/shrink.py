import sys, threading, json
import cmp
opts = sys.argv[1]; size = tuple(int(x) for x in sys.argv[2].split("x")); acts = json.loads(open(sys.argv[3]).read())
def differs(acts, tag="z"):
    keys = [k for a in acts for k in a] + ["SNAP"]
    out = {}
    def go(prog, name):
        o = opts
        out[name] = cmp.run(prog, name + tag, o, keys, size=size)[0]
    ta = threading.Thread(target=go, args=(cmp.NANO, "n")); tb = threading.Thread(target=go, args=(cmp.ZU, "k"))
    ta.start(); tb.start(); ta.join(); tb.join()
    return not cmp.shots_equal(out["n"][-1], out["k"][-1]), out
assert differs(acts)[0]
i = 0
while i < len(acts) - 1:
    trial = acts[:i] + acts[i + 1:]
    if differs(trial)[0]:
        acts = trial
    else:
        i += 1
d, out = differs(acts)
print(json.dumps(acts)); print(d); print(out["n"][-1]); print(out["k"][-1])
