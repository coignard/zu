import sys, cmp, keysweep
for spec in sys.argv[1:]:
    term, cap = spec.split(":")
    seq = dict(keysweep.caps(term))[cap]
    hexs = " ".join("%02x" % x for x in seq)
    keys = ["C-_", "L:9,6", "Enter", "WAIT0.2", "H:" + hexs, "SNAP", "C-c", "SNAP", "L:Z"]
    a, _ = cmp.run(cmp.NANO, "on", "", keys, term=term)
    b, _ = cmp.run(cmp.ZU, "ok", "", keys, term=term)
    same = all(cmp.shots_equal(x, y) for x, y in zip(a, b))
    print(term, cap, "gleich" if same else "VERSCHIEDEN")
    if not same:
        for i, (x, y) in enumerate(zip(a, b)):
            if not cmp.shots_equal(x, y):
                print("--- Bild", i); print(x); print(y)
