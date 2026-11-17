BIG = {"size": (120, 100)}
PG = ["PageDown", "SNAP"] * 4
SCENARIOS = [
 ("hilfe-voll", "", ["C-g", "SNAP"] + PG, BIG),
 ("hilfe-voll-modern", "-/", ["C-h", "SNAP"] + PG, BIG),
 ("hilfe-suche", "", ["C-w", "C-g", "SNAP"] + PG[:2], BIG),
 ("hilfe-ersetzen", "", ["M-r", "C-g", "SNAP", "C-x", "L:a", "Enter", "C-g", "SNAP"], BIG),
 ("hilfe-gehezu", "", ["C-_", "C-g", "SNAP"], BIG),
 ("hilfe-schreiben", "", ["C-o", "C-g", "SNAP"], BIG),
 ("hilfe-einfuegen", "", ["C-r", "C-g", "SNAP"], BIG),
 ("hilfe-ausfuehren", "", ["C-t", "C-g", "SNAP"], BIG),
 ("hilfe-browser", "", ["C-r", "C-t", "C-g", "SNAP", "C-x", "C-w", "C-g", "SNAP", "C-x", "C-c", "C-_", "C-g"], BIG),
 ("hilfe-janein", "", ["L:x", "C-x", "SNAP"], BIG),
 ("browser-cursor", "-g", ["C-r", "C-t", "SNAP", "Down", "SNAP", "Right", "SNAP", "C-End", "SNAP", "C-Home", "PageDown", "SNAP", "C-w", "L:zwei", "Enter"], {"size": (30, 9)}),
 ("hilfe-cursor", "-g", ["C-g", "SNAP", "Down", "Down", "SNAP", "PageDown", "SNAP", "C-End", "SNAP", "C-w", "L:Hilfe", "Enter"]),
 ("modern-tasten", "-/", ["C-f", "L:Hund", "Enter", "SNAP", "C-g", "SNAP", "C-b", "SNAP", "C-x", "C-x", "SNAP", "C-v", "C-v", "SNAP", "C-z", "SNAP", "C-y", "SNAP", "C-a", "C-c", "SNAP", "C-t", "L:5", "Enter", "SNAP", "C-r", "L:Haus", "Enter", "L:H", "Enter", "L:a", "SNAP", "C-e", "SNAP", "C-c", "C-p", "SNAP", "C-d", "C-l", "C-o", "SNAP", "C-c", "C-q"]),
]
