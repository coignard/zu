import cmp, difflib, importlib.util, os, sys
spec = importlib.util.spec_from_file_location("sc", os.path.join(cmp.T, "sc5.py")); m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
def table(shots):
    rows = []
    for s in shots:
        for l in s.split("\n")[1:-4]:
            l = cmp.norm(l).rstrip()
            if l and l not in rows: rows.append(l)
    for i, l in enumerate(rows):
        if len(l) > 17 and (l[0] in "^MF◂▸▴▾" or l[:3] in ("Tab", "Ent", "Bsp", "Del", "Ins", "PgU", "Hom")) and not l.startswith("Meta") and l[16:17] == " ":
            return rows[i:]
    return rows
for sc in m.SCENARIOS:
    if not sc[0].startswith("hilfe-") or sc[0] in ("hilfe-cursor",): continue
    kw = sc[3] if len(sc) > 3 else {}
    a, _ = cmp.run(cmp.NANO, "n", sc[1], sc[2], **kw); b, _ = cmp.run(cmp.ZU, "k", sc[1], sc[2], **kw)
    ta, tb = table(a), table(b)
    d = [l for l in difflib.unified_diff(ta, tb, "nano", "zu", lineterm="", n=0) if not l.startswith(("---", "+++", "@@"))]
    print("==", sc[0], "nano %d Zeilen, zu %d Zeilen, %d Unterschiede" % (len(ta), len(tb), len(d)))
    for l in d: print("   ", l[:150])
