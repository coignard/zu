#!/usr/bin/env python3
import random, sys, os, time, shutil, subprocess, threading, json
import cmp
DEC = cmp.DEC
WORDS = "der die Haus Baum über Straße läuft und nicht Katze Hund grün foo(bar) {baz} [qux] 日本語 x ß €".split()
OPTS = ["", "", "-l", "-S", "-S -l", "-i", "-E -T 4", "-b -r 30", "-b -r 40 -M", "-k", "-A", "-y", "-W", "-Z", "-O", "-q", "-e", "-c", "-x", "-_", "-0",
        "-S -a", "-J 30", "-L", "-u", "-i -b -r 35", "-S -l -q", "-T 3 -l", "-1", "-m"]
def action(r):
    w = lambda: r.choice(WORDS)
    if r.random() < 0.07:
        kind = r.random()
        if kind < 0.4:
            text = (w() + r.choice(["\r", " ", "\t", "\r\r"]) + w()).encode()
            return ["H:1b 5b 32 30 30 7e " + " ".join("%02x" % b for b in text) + " 1b 5b 32 30 31 7e"]
        if kind < 0.7:
            return ["RESIZE:" + r.choice(["50x10", "90x30", "80x24", "33x7", "120x20", "70x3"])]
        x, y = r.randint(1, 40), r.randint(1, 12)
        b = r.choice([0, 0, 64, 65])
        seq = "\x1b[<%d;%d;%dM" % (b, x, y) + ("\x1b[<%d;%d;%dm" % (b, x, y) if b == 0 else "")
        return ["H:" + " ".join("%02x" % c for c in seq.encode())]
    k = r.random()
    if k < 0.22:
        return ["L:" + " ".join(w() for _ in range(r.randint(1, 4))) + r.choice(["", " ", "  "])]
    if k < 0.27: return [r.choice(["Enter", "Tab", "Enter", "L: "])]
    if k < 0.50:
        return [r.choice(["Up", "Down", "Left", "Right", "Home", "End", "PageUp", "PageDown", "C-Left", "C-Right", "C-Up", "C-Down",
                          "C-Home", "C-End", "M-(", "M-)", "M-7", "M-8", "M-Up", "M-Down", "M-<", "M->", "M-]", "C-l", "M-=", "M--",
                          "S-Left", "S-Right", "S-Up", "S-Down", "C-S-Right", "C-S-Left", "S-Home", "S-End", "M-g7\n", "C-a", "C-e"])] * r.choice([1, 1, 2, 5])
    if k < 0.72:
        return [r.choice(["BSpace", "DC", "C-k", "C-u", "M-6", "M-a", "M-u", "M-u", "M-e", "C-j", "M-j", "M-t", "M-}", "M-{", "M-3",
                          "C-Delete", "M-Delete", "M-BSpace", "BSpace", "DC", "C-k", "C-u", "M-u", "S-Delete", "C-S-Delete"])] * r.choice([1, 1, 1, 3])
    if k < 0.80:
        return [r.choice(["C-w", "C-q"]), "L:" + r.choice(WORDS + ["a", "e ", "[a-z]+n", "("]), "Enter"] + r.choice([[], ["M-w"], ["M-q"], ["M-w", "M-w"]])
    if k < 0.85:
        return ["M-r"] + r.choice([[], ["M-r"], ["M-c"], ["M-b"]]) + ["L:" + r.choice(WORDS + ["e", "n ", "u.d"]), "Enter", "L:" + r.choice(["", "X", "ersetzt", "ö ö"]), "Enter"] + \
               r.choice([["L:a"], ["L:j", "L:n", "L:j", "C-c"], ["L:n", "L:a"], ["C-c"]])
    if k < 0.89:
        return ["C-_", "L:%d%s" % (r.randint(-5, 140), r.choice(["", ",3", ",40", ",-2"])), "Enter"]
    if k < 0.94:
        return [r.choice(["M-l", "M-$", "M-i", "M-k", "M-x", "M-c", "M-P", "M-z", "M-m", "M-O", "M-I", "M-K", "M-Z"])]
    if k < 0.96:
        return ["M-v", r.choice(["C-a", "Tab", "L:x", "Escape"])]
    if k < 0.98:
        return ["M-:", "L:" + w(), r.choice(["Down", "End", "Enter"]), "M-:", "M-;", "M-;"]
    return ["C-t", "L:" + r.choice(["echo eins && echo zwei", "printf 'a\\tb'", "seq 3"]), "Enter"]
SAVED = []
def one(seed, n_actions):
    r = random.Random(seed)
    opts = r.choice(OPTS)
    size = r.choice([(80, 24), (80, 24), (60, 16), (100, 30), (40, 12)])
    keys = []
    for i in range(n_actions):
        keys += action(r)
        if i % 6 == 5: keys.append("SNAP")
    keys += ["C-c", "C-c", "SNAP"]
    nk = keys + ["C-o", "Home", "C-k", "L:aus.txt", "Enter", "IF:ANDEREN NAMEN|L:j", "WAIT0.5"]
    kk = keys + ["C-o", "Home", "C-k", "L:aus.txt", "Enter", "IF:ANDEREN NAMEN|L:j", "IF:Neues Passwort|L:" + cmp.PASSWORD, "IF:Neues Passwort|Enter",
                 "IF:Passwort wiederholen|L:" + cmp.PASSWORD, "IF:Passwort wiederholen|Enter", "WAIT1.0"]
    out = {}
    def go(prog, name, kz):
        o = opts
        out[name] = cmp.run(prog, name + str(os.getpid()), o, kz, size=size)
    ta = threading.Thread(target=go, args=(cmp.NANO, "n", nk)); tb = threading.Thread(target=go, args=(cmp.ZU, "k", kk))
    ta.start(); tb.start(); ta.join(); tb.join()
    (a, fa), (b, fb) = out["n"], out["k"]
    problems = []
    for i, (x, y) in enumerate(zip(a[:-1], b[:-1])):
        if not cmp.shots_equal(x, y):
            xl, yl = cmp.norm(x).split("\n"), cmp.norm(y).split("\n")
            d = [(j, p, q) for j, (p, q) in enumerate(zip(xl, yl)) if not cmp.lines_equal(p, q)]
            problems.append("Bild %d:\n" % i + "\n".join(" nano %2d|%s|\n zu %2d|%s|" % (j, p, j, q) for j, p, q in d[:6]))
            break
    na = fa.get("aus.txt")
    kd = None
    kname = [w for w in os.listdir(cmp.WORK) if w.startswith("work-k" + str(os.getpid()))]
    path = cmp.WORK + "/work-k" + str(os.getpid()) + "/aus.txt"
    if os.path.exists(path):
        kd = subprocess.run([DEC, path, cmp.PASSWORD], capture_output=True).stdout
    SAVED.append(na is not None)
    if na != kd:
        problems.append("Dateiinhalt verschieden (nano %s Bytes, zu %s Bytes)" % (None if na is None else len(na), None if kd is None else len(kd)))
    if "datei.txt" in fb and fb["datei.txt"] != open(cmp.SAMPLE, "rb").read():
        problems.append("Zu hat die Ursprungsdatei verändert")
    return opts, size, keys, problems
if __name__ == "__main__":
    start, count = int(sys.argv[1]), int(sys.argv[2]); n_actions = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    bad = 0
    for seed in range(start, start + count):
        opts, size, keys, problems = one(seed, n_actions)
        if problems:
            bad += 1
            print("=== SEED %d opts=%r size=%r" % (seed, opts, size)); print("\n".join(problems)); print(json.dumps(keys)); sys.stdout.flush()
    print("Seeds %d..%d: %d abweichend (gespeichert und verglichen: %d)" % (start, start + count - 1, bad, sum(SAVED)))
