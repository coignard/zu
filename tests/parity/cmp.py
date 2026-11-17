#!/usr/bin/env python3
import subprocess, sys, os, time, shutil, re, tempfile
T = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(T))
WORK = os.environ.get("ZUTEST_WORK", os.path.join(tempfile.gettempdir(), "zu-parity"))
NANO = os.environ.get("NANO", "nano")
ZU = os.environ.get("ZU", os.path.join(ROOT, "target", "release", "zu"))
DEC = os.environ.get("ZUDEC", os.path.join(T, "zudec", "target", "release", "zudec"))
LOCPATH = os.environ.get("ZUTEST_LOCPATH", os.path.join(WORK, "locale"))
PASSWORD = "Kohlrabi-Gewitter-17"
os.makedirs(WORK, exist_ok=True)
SAMPLE = T + "/sample.txt"
SOCK = "cmpzu-%d" % os.getpid()
def tm(*a, out=False):
    r = subprocess.run(["tmux", "-L", SOCK] + list(a), capture_output=True, text=True)
    return r.stdout
ATTRS = os.environ.get("ATTRS") == "1"
BUSY = ("Schlüssel wird abgeleitet", "Entschlüsseln …")
def shot(name):
    for _ in range(50):
        if not any(b in tm("capture-pane", "-p", "-t", "=" + name + ":") for b in BUSY):
            break
        time.sleep(0.1)
    return tm("capture-pane", "-p", *(["-e", "-N"] if ATTRS else []), "-t", "=" + name + ":") + tm("display-message", "-p", "-t", "=" + name + ":", "CURSOR #{cursor_x},#{cursor_y} sichtbar=#{cursor_flag}")
def run(prog, name, opts, keys, size=(80, 24), nofile=False, file=None, rc=False, lock=False, term="xterm-256color", env_extra=""):
    d = WORK + "/work-" + name
    shutil.rmtree(d, ignore_errors=True); os.makedirs(d)
    shutil.copy(SAMPLE, d + "/datei.txt")
    open(d + "/zwei.txt", "w").write("zweite Datei\nmit zwei Zeilen\n")
    for extra in ("dos.txt", "bin.txt", "lang.txt"):
        shutil.copy(T + "/" + extra, d + "/" + extra)
    open(d + "/ro.txt", "w").write("nur lesen\n"); os.chmod(d + "/ro.txt", 0o444)
    os.makedirs(d + "/bak", exist_ok=True)
    if lock:
        open(d + "/.datei.txt.swp", "wb").write(b"b0nano 9.3\0" + b"\0" * 13 + bytes([1, 0, 0, 0]) + b"fremder" + b"\0" * 33 + b"anderswo" + b"\0" * 24 + b"\0" * 932)
    env = "env -i HOME=%s%s LOCPATH=%s LANG=de_DE.UTF-8 PATH=/usr/bin:/bin SHELL=/bin/sh%s" % (d, "" if term is None else " TERM=" + term, LOCPATH, (" " + env_extra) if env_extra else "")
    cmd = "cd %s && %s %s %s %s %s; echo ENDE=$?; sleep 30" % (d, env, prog, "" if rc else "-I", opts, "datei.txt nichtda/x.txt zwei.txt" if nofile else (file or "datei.txt"))
    tm("kill-session", "-t", "=" + name + ":")
    tm("new-session", "-d", "-s", name, "-x", str(size[0]), "-y", str(size[1]), cmd)
    time.sleep(0.5)
    for _ in range(30):
        if tm("capture-pane", "-p", "-t", "=" + name + ":").strip():
            break
        time.sleep(0.1)
    shots = []
    for k in keys:
        if k == "SNAP":
            time.sleep(0.25); shots.append(shot(name)); continue
        if k.startswith("WAIT"):
            time.sleep(float(k[4:])); continue
        if k.startswith("RESIZE:"):
            w, h = k[7:].split("x"); tm("resize-window", "-t", "=" + name + ":", "-x", w, "-y", h); time.sleep(0.3); continue
        if k.startswith("IF:"):
            pattern, k = k[3:].split("|", 1)
            time.sleep(0.3)
            if pattern not in tm("capture-pane", "-p", "-t", "=" + name + ":"):
                continue
        if k.startswith("L:"):
            tm("send-keys", "-t", "=" + name + ":", "-l", k[2:])
        elif k.startswith("H:"):
            tm("send-keys", "-t", "=" + name + ":", "-H", *k[2:].split())
        else:
            tm("send-keys", "-t", "=" + name + ":", k)
        time.sleep(0.06)
    time.sleep(0.35)
    shots.append(shot(name))
    tm("kill-session", "-t", "=" + name + ":")
    res = {}
    for f in sorted(os.listdir(d)):
        try: res[f] = open(d + "/" + f, "rb").read()
        except Exception: res[f] = b"<dir>"
    return shots, res
def cells(s):
    out = []; attr = ()
    for line in s.split("\n"):
        if line.startswith("CURSOR "):
            out.append(line); continue
        row = []; i = 0
        while i < len(line):
            if line[i] == "\x1b" and line[i + 1:i + 2] == "[":
                j = line.index("m", i); codes = [c for c in line[i + 2:j].split(";")]
                st = set(attr); k = 0
                while k < len(codes):
                    c = codes[k] or "0"
                    if c == "0": st = set()
                    elif c in ("38", "48"):
                        n = 3 if codes[k + 1] == "5" else 5
                        st = {x for x in st if not x.startswith(c + ":")}; st.add(c + ":" + ";".join(codes[k + 1:k + n])); k += n - 1
                    elif c == "39": st = {x for x in st if not (x.startswith("38:") or x[:1] in "3" and len(x) == 2 or x[:1] == "9" and len(x) == 2)}
                    elif c == "49": st = {x for x in st if not (x.startswith("48:") or x[:1] == "4" and len(x) == 2 or x[:2] == "10" and len(x) == 3)}
                    elif len(c) == 2 and c[0] == "3" or len(c) == 2 and c[0] == "9": st = {x for x in st if not (x.startswith("38:") or len(x) == 2 and x[0] in "39")}; st.add(c)
                    elif len(c) == 2 and c[0] == "4" and c != "4" or len(c) == 3 and c[:2] == "10": st = {x for x in st if not (x.startswith("48:") or len(x) == 2 and x[0] == "4" or len(x) == 3 and x[:2] == "10")}; st.add(c)
                    elif c in ("22",): st -= {"1", "2"}
                    elif c in ("23",): st -= {"3"}
                    elif c in ("24",): st -= {"4"}
                    elif c in ("27",): st -= {"7"}
                    else: st.add(c)
                    k += 1
                attr = tuple(sorted(st)); i = j + 1
            else:
                a = attr
                if line[i] == " " and "7" not in a:
                    a = tuple(x for x in a if not (x in ("1", "2", "3") or x.startswith("38:") or (len(x) == 2 and x[0] in "39")))
                row.append((line[i], a)); i += 1
        while row and row[-1] == (" ", ()): row.pop()
        out.append("".join(ch + ("{" + ",".join(a) + "}" if a else "") for ch, a in row))
    return "\n".join(out)
def norm(s):
    s = s.replace("work-n", "work-X").replace("work-k", "work-X")
    if ATTRS:
        s = cells(s)
        l = s.split("\n")
        enc = lambda t, a: "".join(ch + "{" + a + "}" for ch in t)
        for a in ("7", "1", "1,7"):
            l[0] = l[0].replace(enc("GNU nano 9.3", a), "")
            l[0] = re.sub("".join(re.escape(ch) + r"\{%s\}" % a for ch in "zu ") + r"(?:[0-9.]\{%s\})+" % a, "", l[0])
            l[0] = re.sub(r"( \{%s\})+" % a, " {%s}" % a, l[0])
        s = "\n".join(l)
        return s
    s = s.replace("work-n", "work-X").replace("work-k", "work-X")
    l = s.split("\n")
    l[0] = re.sub(r"zu [0-9]+\.[0-9]+\.[0-9]+", "", l[0].replace("GNU nano 9.3", ""))
    l[0] = re.sub(r"^.*/work-X/", "WORK/", l[0])
    s = "\n".join(l)
    l = s.split("\n"); l[0] = re.sub(r" +", " ", l[0]); s = "\n".join(l)
    s = re.sub(r"\bnano\b", "zu", s)
    s = re.sub(r"\bNano\b", "Zu", s)
    return s
import gettext
_MO = [p for p in (os.environ.get("ZUTEST_LIBC_MO", ""), "/usr/share/locale-langpack/de/LC_MESSAGES/libc.mo", "/usr/share/locale/de/LC_MESSAGES/libc.mo") if p and os.path.exists(p)]
_cat = gettext.GNUTranslations(open(_MO[0], "rb"))._catalog if _MO else {}
EN2DE = {}
for _n in range(1, 134):
    _en = os.strerror(_n)
    if not _en.startswith("Unknown") and _en in _cat:
        EN2DE[_en] = _cat[_en]
NOTES = (" (unverschlüsselt)",)
def plain(line):
    return re.sub(r"\{[^}]*\}", "", line)
def status_text(line):
    t = plain(line).strip()
    for en, de in EN2DE.items():
        t = t.replace(en, de)
    for n in NOTES:
        t = t.replace(n, "")
    return t
def special(line):
    t = plain(line)
    return any(en in t for en in EN2DE) or any(n.strip(" –()") in t for n in NOTES)
def lines_equal(p, q):
    if p == q:
        return True
    if p.startswith("CURSOR ") and q.startswith("CURSOR ") and p.endswith("sichtbar=0") and q.endswith("sichtbar=0"):
        return True
    if special(p) or special(q):
        if plain(q).strip() == "[ Datei ist unverschlüsselt ]":
            return True
        return status_text(p) == status_text(q)
    return False
def shots_equal(x, y):
    xl, yl = norm(x).split("\n"), norm(y).split("\n")
    return len(xl) == len(yl) and all(lines_equal(p, q) for p, q in zip(xl, yl))
def compare(title, opts, keys, **kw):
    a, _ = run(NANO, "n", opts, keys, **kw)
    b, _ = run(ZU, "k", opts, keys, **kw)
    ok = True
    for i, (x, y) in enumerate(zip(a, b)):
        if not shots_equal(x, y):
            ok = False
            print("=== UNTERSCHIED: %s (Bild %d) ===" % (title, i))
            xl, yl = x.split("\n"), y.split("\n")
            for j in range(max(len(xl), len(yl))):
                p = xl[j] if j < len(xl) else ""; q = yl[j] if j < len(yl) else ""
                if not lines_equal(norm(p), norm(q)):
                    print(" nano %2d|%s|" % (j, p)); print(" zu %2d|%s|" % (j, q))
    print(("ok   " if ok else "FEHL ") + title)
    return ok
if __name__ == "__main__":
    import importlib.util
    spec = importlib.util.spec_from_file_location("sc", sys.argv[1]); m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
    only = sys.argv[2] if len(sys.argv) > 2 else None
    bad = 0; n = 0
    for sc in m.SCENARIOS:
        if only and only not in sc[0]: continue
        n += 1
        kw = sc[3] if len(sc) > 3 else {}
        if not compare(sc[0], sc[1], sc[2], **kw): bad += 1
    print("%d Szenarien, %d abweichend" % (n, bad))
