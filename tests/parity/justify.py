import re, subprocess, sys, cmp
base = ["M-Down", "M-Down", "C-k", "M-a", "M-a", "M-a", "C-w", "L:der", "Enter", "M-w", "M-j", "SNAP", "M-a"]
save = ["C-o", "Home", "C-k", "L:aus.txt", "Enter", "IF:ANDEREN NAMEN|L:j"]
nk = base + save + ["WAIT0.5"]
kk = base + save + ["IF:Neues Passwort|L:" + cmp.PASSWORD, "IF:Neues Passwort|Enter",
                    "IF:Passwort wiederholen|L:" + cmp.PASSWORD, "IF:Passwort wiederholen|Enter", "WAIT1.0"]
a, fa = cmp.run(cmp.NANO, "jn", "", nk)
b, fb = cmp.run(cmp.ZU, "jk", "", kk)
orig = open(cmp.SAMPLE, "rb").read()
nano_out = fa.get("aus.txt", b"")
zu_out = subprocess.run([cmp.DEC, cmp.WORK + "/work-jk/aus.txt", cmp.PASSWORD], capture_output=True).stdout
def words(b):
    return sorted(b.decode("utf-8", "replace").split())
ow = words(orig); ow.remove("das"); ow.remove("eine"); ow.remove("eine")
def squash(b):
    return re.sub(r"[\s>]", "", b.decode("utf-8", "replace"))
lines = orig.split(b"\n")
ok = b"\0" not in zu_out and any(squash(b"\n".join(lines[:i] + lines[i + 1:])) == squash(zu_out) for i in range(len(lines)))
missing = [w for w in set(ow) if words(nano_out).count(w) < ow.count(w)]
print("nano 9.3 verliert %d Wörter, zum Beispiel %s" % (len(ow) - len(words(nano_out)), sorted(missing)[:6]))
print(("ok   " if ok else "FEHL ") + "Absatz ausrichten mit kürzerem Präfix")
sys.exit(0 if ok else 1)
