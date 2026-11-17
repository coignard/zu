def H(s):
    return "H:" + " ".join("%02x" % b for b in s.encode("latin-1"))
def sgr(b, x, y, last="M"):
    return "\x1b[<%d;%d;%d%s" % (b, x, y, last)
def click(x, y):
    return sgr(0, x, y) + sgr(0, x, y, "m")
def x10(b, x, y):
    return "\x1b[M" + chr(32 + b) + chr(32 + x) + chr(32 + y)
def xclick(x, y):
    return x10(0, x, y) + x10(3, x, y)
SCR = {"term": "screen"}
TMX = {"term": "tmux-256color"}
VT = {"term": "vt100"}
SCENARIOS = [
 ("sgr-aus", "", [H(click(10, 5)), "SNAP", H(sgr(64, 10, 5)), "SNAP", "L:x", "SNAP"]),
 ("sgr-an-klick", "-m", [H(click(10, 5)), "SNAP", H(click(3, 8)), "L:X", "SNAP", H(click(70, 3)), "SNAP"]),
 ("sgr-an-rad", "-m", [H(sgr(65, 10, 5)), H(sgr(65, 10, 5)), H(sgr(65, 10, 5)), "SNAP", H(sgr(64, 10, 5)), "SNAP", H(sgr(65, 10, 23)), "SNAP", H(sgr(65, 10, 22)), "SNAP"]),
 ("sgr-an-hilfe", "-m", [H(click(15, 24)), "SNAP", "C-c", H(click(30, 23)), "SNAP", "C-c", H(click(79, 24)), "SNAP"]),
 ("sgr-umschalten", "-m", ["M-m", "SNAP", H(click(10, 5)), "SNAP", "L:a", "SNAP", "M-m", H(click(4, 6)), "SNAP", "M-m", H(sgr(65, 10, 5)), "SNAP", "L:b", "SNAP"]),
 ("sgr-doppelklick", "-m", [H(click(10, 5) + click(10, 5)), "SNAP", "L:Z", "SNAP", H(click(10, 5) + click(10, 5) + click(10, 5)), "SNAP", H(click(10, 5) + click(12, 7)), "SNAP"]),
 ("sgr-druck-taste-los", "-m", [H(sgr(0, 10, 5) + "x" + sgr(0, 10, 5, "m")), "SNAP", H(sgr(0, 20, 6)), "SNAP", "L:y", "SNAP", H(sgr(0, 20, 6, "m")), "SNAP"]),
 ("sgr-seltsam", "-m", [H("\x1b[<0;10;5x"), "SNAP", H("\x1b[<0;1\x010;5M\x1b[<0;10;5m"), "SNAP", H("\x1b[<2;10;5M\x1b[<2;10;5m"), "SNAP", H("\x1b[<1;1;1;1;1;1;1;1;1;1;1;1M"), "SNAP", H("\x1b[<132;10;5M"), "SNAP", H("\x1b[<4;10;5M\x1b[<4;10;5m"), "SNAP", H("\x1b[<0;0;0M\x1b[<0;0;0m"), "SNAP", H("\x1b[<35;9;9M"), "SNAP"]),
 ("sgr-aus-seltsam", "", [H("\x1b[<0;1\x010;5M"), "SNAP", H("\x1b[<\x1b[A"), "SNAP", H("\x1b[<"), "L:q", "SNAP"]),
 ("x10-aus", "", [H(xclick(10, 5)), "SNAP", H(x10(64, 10, 5)), "SNAP"], SCR),
 ("x10-an", "-m", [H(xclick(10, 5)), "SNAP", H(xclick(3, 8)), "L:X", "SNAP", H(x10(65, 10, 5)), H(x10(65, 10, 5)), "SNAP", H(x10(64, 10, 5)), "SNAP", H(xclick(15, 24)), "SNAP"], SCR),
 ("x10-tmux", "-m", [H(xclick(10, 5)), "SNAP", H(x10(65, 10, 5)), "SNAP", "M-m", H(xclick(4, 6)), "SNAP", "L:c", "SNAP"], TMX),
 ("x10-seltsam", "-m", [H(x10(3, 10, 5)), "SNAP", H(x10(0, 10, 5) + "a"), "SNAP", H(x10(70, 10, 5)), "SNAP", H(x10(0, 200, 5) + x10(3, 200, 5)), "SNAP", H("\x1b[M\x01\x02\x03"), "SNAP"], SCR),
 ("vt100-maus", "-m", [H(click(10, 5)), "SNAP", H(xclick(10, 5)), "SNAP"], VT),
 ("roh-maus", "-m -K", [H(click(10, 5)), "SNAP", H(sgr(65, 10, 5)), "SNAP"]),
 ("leiste-klick", "-m -q", [H(click(80, 12)), "SNAP", H(click(80, 2)), "SNAP", H(click(80, 22)), "SNAP"], {"size": (80, 24), "file": "lang.txt"}),
 ("browser-maus", "-m", ["C-r", "C-t", "SNAP", H(click(5, 4)), "SNAP", H(click(5, 4)), "SNAP", H(sgr(65, 5, 4)), "SNAP", "C-c", "C-c"]),
 ("hilfe-maus", "-m", ["C-g", H(sgr(65, 5, 4)), "SNAP", H(click(5, 23)), "SNAP", "C-x"]),
 ("prompt-maus", "-m", ["C-w", "L:Haus und Baum", H(click(12, 22)), "SNAP", "L:X", "SNAP", H(sgr(65, 5, 4)), "SNAP", "C-c"]),
 ("janein-maus", "-m", ["L:x", "C-x", H(click(5, 23)), "SNAP", H(click(5, 24)), "SNAP"]),
]
