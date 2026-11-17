import sys,re
pid=int(sys.argv[1]); needles=[a.encode() for a in sys.argv[2:]]
found={n:0 for n in needles}
try:
    maps=open(f"/proc/{pid}/maps").read().splitlines(); mem=open(f"/proc/{pid}/mem","rb",0)
except Exception as e: print("kein Zugriff:",e); sys.exit()
for l in maps:
    m=re.match(r"([0-9a-f]+)-([0-9a-f]+) (\S+)",l)
    if not m or 'r' not in m.group(3) or '[v' in l: continue
    a,b=int(m.group(1),16),int(m.group(2),16)
    try: mem.seek(a); data=mem.read(b-a)
    except Exception: continue
    for n in needles:
        c=data.count(n)
        if c: found[n]+=c; print("  Treffer",n,c,l.split()[-1] if len(l.split())>5 else "anon")
print(found)
