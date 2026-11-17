import os, re, subprocess, sys, time, shutil, signal

import cmp
ZU = cmp.ZU
DEC = cmp.DEC
LOCPATH = cmp.LOCPATH
PW = "Geheimpasswort4711"
MARK = "KLARTEXTMARKE"
SOCK = "zusec"
ok_all = True


def tm(*a):
    return subprocess.run(["tmux", "-L", SOCK] + list(a), capture_output=True, text=True).stdout


def check(name, cond, detail=""):
    global ok_all
    ok_all &= bool(cond)
    print(("ok   " if cond else "FEHL ") + name + (" (" + detail + ")" if detail and not cond else ""))


def start(d, args, flags="-I"):
    env = "env -i HOME=%s TERM=xterm-256color LOCPATH=%s LANG=de_DE.UTF-8 PATH=/usr/bin:/bin" % (d, LOCPATH)
    tm("kill-session", "-t", "s")
    tm("new-session", "-d", "-s", "s", "-x", "80", "-y", "24", "cd %s && exec %s %s %s %s" % (d, env, ZU, flags, args))
    time.sleep(0.8)


def keys(*ks):
    for k in ks:
        if k.startswith("L:"):
            tm("send-keys", "-t", "s", "-l", k[2:])
        elif k.startswith("H:"):
            tm("send-keys", "-t", "s", "-H", *k[2:].split())
        else:
            tm("send-keys", "-t", "s", k)
        time.sleep(0.15)


def tty_flags():
    tty = tm("display-message", "-p", "-t", "s", "#{pane_tty}").strip()
    out = subprocess.run(["stty", "-a", "-F", tty], capture_output=True, text=True).stdout
    return set(out.replace(";", " ").split())


def zu_pid():
    out = subprocess.run(["pgrep", "-n", "-f", "^" + re.escape(ZU) + " "], capture_output=True, text=True).stdout.split()
    return int(out[0]) if out else None


def memory_hits(pid, needles):
    hits = {n: 0 for n in needles}
    maps = open("/proc/%d/maps" % pid).read().splitlines()
    mem = open("/proc/%d/mem" % pid, "rb", 0)
    for line in maps:
        m = re.match(r"([0-9a-f]+)-([0-9a-f]+) (\S+)", line)
        if not m or "r" not in m.group(3) or "[v" in line:
            continue
        a, b = int(m.group(1), 16), int(m.group(2), 16)
        try:
            mem.seek(a)
            data = mem.read(b - a)
        except Exception:
            continue
        for n in needles:
            hits[n] += data.count(n)
    return hits


def decrypt(path, pw=PW):
    r = subprocess.run([DEC, path, pw], capture_output=True)
    return r.stdout if r.returncode == 0 else None


d = cmp.WORK + "/work-security"
shutil.rmtree(d, ignore_errors=True)
os.makedirs(d)
open(d + "/klar.txt", "w").write("Erste Zeile\n")

start(d, "klar.txt")
keys("L:" + MARK, "C-o", "Home", "C-k", "L:geheim.zu", "Enter", "L:j", "L:falsch", "C-u", "L:" + PW + "X", "BSpace", "Enter", "L:" + PW, "Enter")
time.sleep(1.5)
pid = zu_pid()
data = open(d + "/geheim.zu", "rb").read() if os.path.exists(d + "/geheim.zu") else b""
check("Datei gespeichert", len(data) > 0)
check("Signaturbyte 47 und Version 1", data[:2] == bytes([47, 1]), repr(data[:2]))
check("Vorspann 54 Bytes + Tag", len(data) == 54 + len(MARK + "Erste Zeile\n") + 16, str(len(data)))
check("kein Klartext in der Datei", MARK.encode() not in data and b"Erste" not in data)
check("Entschlüsselung liefert den Text", decrypt(d + "/geheim.zu") == (MARK + "Erste Zeile\n").encode())
if pid:
    hits = memory_hits(pid, [PW.encode(), PW[:8].encode(), PW[-8:].encode()])
    check("Passwort nicht im Speicher", not any(hits.values()), str(hits))
else:
    check("Prozess gefunden", False)
for name in os.listdir(d):
    if name not in ("klar.txt", "geheim.zu"):
        check("keine weitere Datei: " + name, False)

keys("L:NACHTRAG")
time.sleep(0.3)
if pid:
    os.kill(pid, signal.SIGTERM)
    time.sleep(1.5)
save = d + "/geheim.zu.save"
sdata = open(save, "rb").read() if os.path.exists(save) else b""
check("Notsicherung bei SIGTERM angelegt", len(sdata) > 0)
check("Notsicherung verschlüsselt (47, 1)", sdata[:2] == bytes([47, 1]))
check("Notsicherung ohne Klartext", b"NACHTRAG" not in sdata and MARK.encode() not in sdata)
check("Notsicherung entschlüsselbar", decrypt(save) is not None and b"NACHTRAG" in decrypt(save))

shutil.rmtree(d, ignore_errors=True)
os.makedirs(d)
open(d + "/klar.txt", "w").write("Erste Zeile\n")
start(d, "klar.txt")
keys("L:" + MARK)
time.sleep(0.3)
pid = zu_pid()
if pid:
    os.kill(pid, signal.SIGHUP)
    time.sleep(1.5)
files = sorted(os.listdir(d))
check("ohne Passwort keine Notsicherung", files == ["klar.txt"], str(files))
check("Original unverändert", open(d + "/klar.txt").read() == "Erste Zeile\n")
leak = [f for f in files if MARK.encode() in open(d + "/" + f, "rb").read()]
check("kein Klartext auf dem Datenträger", not leak, str(leak))

shutil.rmtree(d, ignore_errors=True)
os.makedirs(d)
GONE = "VERSCHWINDEMARKE"
open(d + "/a.txt", "w").write(GONE + " Zeile\n" * 3)
open(d + "/b.txt", "w").write("andere Datei\n")
start(d, "a.txt b.txt")
time.sleep(0.5)
pid = zu_pid()
if pid:
    before = memory_hits(pid, [GONE.encode()])[GONE.encode()]
    keys("C-x")
    time.sleep(0.8)
    after = memory_hits(pid, [GONE.encode()])[GONE.encode()]
    check("Klartext einer geschlossenen Datei aus dem Speicher getilgt", before > 0 and after == 0, "vorher %d, nachher %d" % (before, after))
keys("C-x")
time.sleep(0.5)

shutil.rmtree(d, ignore_errors=True)
os.makedirs(d)
open(d + "/code.rs", "w").write("// ein Kommentar\nfn main() {}\n")
start(d, "code.rs")
screen = tm("capture-pane", "-p", "-t", "s")
check("„//“-Datei öffnet als Klartext", "// ein Kommentar" in screen and "Passwort" not in screen, screen.split("\n")[-4] if screen else "")
keys("C-x")
time.sleep(0.5)

shutil.rmtree(d, ignore_errors=True)
os.makedirs(d)
NAME = "tagebuch-geheim-2026.zu"
REST = "ZWEITEZEILE"
start(d, "")
keys("L:" + MARK, "C-o", "L:" + NAME, "Enter")
time.sleep(0.5)
flags = tty_flags()
check("Passwortabfrage im kanonischen Modus ohne Echo (Secure Input)", {"icanon", "-echo", "-isig"} <= flags, " ".join(sorted(flags)))
for bad, hint in (("kurz", "Mindestens 15 Zeichen!"),
                  ("1Q2W3E4R5T6Y7U8I", "Zu verbreitet!"),
                  ("Tagebuch-Geheim-2026", "Gleicht dem Dateinamen!")):
    keys("L:" + bad, "Enter")
    time.sleep(0.3)
    footer = tm("capture-pane", "-p", "-t", "s").split("\n")[-4]
    check("Passwort abgelehnt: " + hint, hint + " Neues Passwort:" in footer, footer)
hexed = lambda text: " ".join("%02x" % b for b in text.encode())
keys("H:1b 5b 32 30 30 7e " + hexed(PW) + " 0d 0a " + hexed(REST) + " 1b 5b 32 30 31 7e")
time.sleep(0.3)
footer = tm("capture-pane", "-p", "-t", "s").split("\n")[-4]
check("Zeilenumbruch in eingefügtem Passwort bestätigt nicht", "Neues Passwort:" in footer and "wiederholen" not in footer, footer)
keys("Enter", "L:" + PW + REST, "Enter")
time.sleep(1.5)
check("eingefügtes Passwort ohne Zeilenumbruch", decrypt(d + "/" + NAME, PW + REST) == (MARK + "\n").encode())
screen = tm("capture-pane", "-p", "-t", "s")
check("Rest der Einfügung nicht im Text", REST not in screen.split("\n")[2], screen.split("\n")[2])
flags = tty_flags()
check("nach der Abfrage wieder Rohmodus", {"-icanon", "-echo"} <= flags, " ".join(sorted(flags)))
keys("C-x")
time.sleep(0.5)

shutil.rmtree(d, ignore_errors=True)
os.makedirs(d)
start(d, "")
keys("L:" + MARK, "C-o", "L:abbruch.zu", "Enter", "L:" + PW, "C-c")
time.sleep(0.3)
footer = tm("capture-pane", "-p", "-t", "s").split("\n")[-4]
check("^C bricht die Passwortabfrage ab", "Abgebrochen" in footer and not os.path.exists(d + "/abbruch.zu"), footer)
flags = tty_flags()
check("nach dem Abbruch wieder Rohmodus", {"-icanon", "-echo"} <= flags, " ".join(sorted(flags)))
keys("C-x", "L:n")
time.sleep(0.5)

shutil.rmtree(d, ignore_errors=True)
os.makedirs(d)
open(d + "/zurc", "w").write("set argonmemory 8\nset argonpasses 1\n")
start(d, "schwach.zu", flags="-f " + d + "/zurc")
keys("L:" + MARK, "C-o", "Enter", "L:" + PW, "Enter", "L:" + PW, "Enter")
time.sleep(1.5)
keys("C-x")
time.sleep(0.5)
data = open(d + "/schwach.zu", "rb").read() if os.path.exists(d + "/schwach.zu") else b""
memory = int.from_bytes(data[2:6], "little") if len(data) > 14 else 0
passes = int.from_bytes(data[6:10], "little") if len(data) > 14 else 0
check("zu-Konfiguration kann Argon2 nicht unter das Minimum senken", memory >= 64 * 1024 and passes >= 3, "%d KiB, %d Durchläufe" % (memory, passes))
start(d, "schwach.zu")
keys("L:" + PW, "Enter")
time.sleep(1.5)
screen = tm("capture-pane", "-p", "-t", "s")
check("so gespeicherte Datei öffnet wieder", MARK in screen, screen.split("\n")[-4] if screen else "")
keys("C-x")
time.sleep(0.5)

tm("kill-server")
shutil.rmtree(d, ignore_errors=True)
print("ALLES OK" if ok_all else "FEHLER GEFUNDEN")
sys.exit(0 if ok_all else 1)
