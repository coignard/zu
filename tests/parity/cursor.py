import os
import re
import shutil
import subprocess
import sys
import time
import unicodedata

import cmp

SOCK = "flimmer"
WIDTH = 80


def tm(*a):
    return subprocess.run(["tmux", "-L", SOCK] + list(a), capture_output=True, text=True).stdout


def record(prog, keys, opts, size):
    d = cmp.WORK + "/work-cursor"
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    shutil.copy(cmp.SAMPLE, d + "/datei.txt")
    trace = d + "/trace"
    env = "env -i HOME=%s TERM=xterm-256color LOCPATH=%s LANG=de_DE.UTF-8 PATH=/usr/bin:/bin" % (d, cmp.LOCPATH)
    cmd = "cd %s && %s strace -ttt -e trace=write,read -e write=1 -s 65535 -o %s %s -I %s datei.txt" % (d, env, trace, prog, opts)
    tm("kill-server")
    time.sleep(0.2)
    tm("new-session", "-d", "-s", "f", "-x", str(size[0]), "-y", str(size[1]), cmd)
    time.sleep(1.2)
    marks = []
    for k in keys:
        marks.append(time.time())
        if k.startswith("L:"):
            tm("send-keys", "-t", "f", "-l", k[2:])
        else:
            tm("send-keys", "-t", "f", k)
        time.sleep(0.5)
    tm("kill-server")
    time.sleep(0.3)
    return open(trace, encoding="latin-1").read(), marks


def unescape(s):
    return bytes(s, "latin-1").decode("unicode_escape").encode("latin-1")


def writes(trace):
    out = []
    for line in trace.splitlines():
        m = re.match(r"(?:\d+ +)?(\d+\.\d+) write\(1, \"(.*)\", \d+\) = \d+", line)
        if m:
            out.append((float(m.group(1)), unescape(m.group(2))))
    return out


class Screen:
    def __init__(self, cols, rows):
        self.cols, self.rows = cols, rows
        self.r, self.c, self.visible = 1, 1, True
        self.saved = (1, 1)
        self.alt_saved = (1, 1)

    def feed(self, data):
        text = data.decode("utf-8", "replace")
        i = 0
        while i < len(text):
            ch = text[i]
            if ch == "\x1b":
                m = re.match(r"\x1b\[([?>]?)([\d;]*)([A-Za-z@])", text[i:])
                if m:
                    private, params, final = m.groups()
                    nums = [int(p) if p else 0 for p in params.split(";")] if params else []
                    n = nums[0] if nums and nums[0] else 1
                    if private == "?" and final in "hl" and "25" in params.split(";"):
                        self.visible = final == "h"
                    elif private == "?" and final in "hl" and "1049" in params.split(";"):
                        if final == "h":
                            self.alt_saved = (self.r, self.c)
                        else:
                            self.r, self.c = self.alt_saved
                    elif not private:
                        if final == "H" or final == "f":
                            self.r = nums[0] if len(nums) > 0 and nums[0] else 1
                            self.c = nums[1] if len(nums) > 1 and nums[1] else 1
                        elif final == "d":
                            self.r = n
                        elif final == "G" or final == "`":
                            self.c = n
                        elif final == "A":
                            self.r = max(1, self.r - n)
                        elif final == "B":
                            self.r = min(self.rows, self.r + n)
                        elif final == "C":
                            self.c = min(self.cols, self.c + n)
                        elif final == "D":
                            self.c = max(1, self.c - n)
                    i += m.end()
                    continue
                if text.startswith("\x1b7", i):
                    self.saved = (self.r, self.c)
                elif text.startswith("\x1b8", i):
                    self.r, self.c = self.saved
                i += 3 if text.startswith(("\x1b(", "\x1b)"), i) else 2
                continue
            if ch == "\r":
                self.c = 1
            elif ch == "\n":
                self.r = min(self.rows, self.r + 1)
            elif ch == "\x08":
                self.c = max(1, self.c - 1)
            elif ch >= " ":
                w = 0 if unicodedata.category(ch) in ("Mn", "Me") else 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1
                self.c = min(self.cols + 1, self.c + w)
            i += 1
        return self.visible, self.r, min(self.c, self.cols)


def steps(trace, marks, size):
    scr = Screen(*size)
    result = [set() for _ in marks]
    for t, data in writes(trace):
        visible, r, c = scr.feed(data)
        step = sum(1 for m in marks if m <= t) - 1
        if step >= 0 and visible:
            result[step].add((r, c))
    return result


def compare(name, keys, opts="", size=(80, 24)):
    nt, nm = record(cmp.NANO, keys, opts, size)
    zt, zm = record(cmp.ZU, keys, opts, size)
    ns, zs = steps(nt, nm, size), steps(zt, zm, size)
    bad = []
    for i, (n, z) in enumerate(zip(ns, zs)):
        extra = z - n
        if extra:
            bad.append("  nach %-14s zu zeigt den Cursor bei %s, nano nur bei %s" % (keys[i], sorted(extra), sorted(n)))
    print(("ok   " if not bad else "FEHL ") + name)
    for b in bad:
        print(b)
    return not bad


CASES = [
    ("speichern-abbrechen", ["L:x", "C-x", "C-c", "C-x", "L:n"]),
    ("suche-abbrechen", ["C-w", "L:Haus", "C-c", "C-w", "L:Haus", "Enter"]),
    ("gehezu", ["C-_", "L:9", "Enter", "C-_", "C-c"]),
    ("hilfe", ["C-g", "PageDown", "C-x"]),
    ("schreiben-abbrechen", ["C-o", "C-c", "L:y", "C-o", "C-c"]),
    ("ersetzen", ["M-r", "L:Haus", "Enter", "L:Hof", "Enter", "L:j", "C-c"]),
    ("ausschneiden-einfuegen", ["C-k", "C-k", "C-u", "M-u", "M-e"]),
    ("markieren", ["M-a", "Down", "Down", "M-6", "M-a", "C-u"]),
    ("position", ["C-c", "M-x", "M-x", "C-c"]),
    ("datei-einfuegen", ["C-r", "C-c", "C-r", "C-t", "C-c", "C-c"]),
]

if __name__ == "__main__":
    only = sys.argv[1] if len(sys.argv) > 1 else None
    bad = 0
    for name, keys in CASES:
        if only and only not in name:
            continue
        bad += not compare(name, keys)
    print("%d Fälle abweichend" % bad)
    sys.exit(1 if bad else 0)
