import sys, cmp
seqs = ["\x1b\x1b[1;5A", "\x1bOU", "\x1bOV", "\x1bOW", "\x1bOX", "\x1b[13;6~", "\x1b[16$", "\x1b[4N"]
for seq in seqs:
    hexs = " ".join("%02x" % ord(c) for c in seq)
    keys = ["C-_", "L:9,6", "Enter", "WAIT0.2", "H:" + hexs, "SNAP", "C-c", "SNAP", "L:Z"]
    a, _ = cmp.run(cmp.NANO, "on", "-K", keys)
    b, _ = cmp.run(cmp.ZU, "ok", "-K", keys)
    same = all(cmp.shots_equal(x, y) for x, y in zip(a, b))
    print(repr(seq), "gleich" if same else "VERSCHIEDEN")
