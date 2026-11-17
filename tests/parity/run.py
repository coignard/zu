#!/usr/bin/env python3
import argparse
import difflib
import importlib.util
import os
import shutil
import subprocess
import sys

import cmp

SUITES = ["sc1.py", "sc2.py", "sc3.py", "sc4.py", "sc5.py", "sc6.py", "sc7.py", "sc8.py"]
GOLDEN = os.path.join(cmp.T, "golden")

INTENDED = {
    "sc1.py": {
        "hilfe": "Hilfetext beschreibt die Verschlüsselung",
        "hilfe-f1": "Hilfetext beschreibt die Verschlüsselung",
        "klein": "Hilfetext beschreibt die Verschlüsselung",
        "modern": "Hilfetext beschreibt die Verschlüsselung",
        "ausfuehren": "kein Linter, Formatierer und keine Weiterleitung",
        "ausfuehren-menu": "kein Linter, Formatierer und keine Weiterleitung",
        "unbekannt": "M-& zeigt die Terminalansteuerung, M-Y ohne Syntaxhervorhebung",
        "maus": "M-K Passwort im Schreibmenü",
        "speichern-abbr": "M-K Passwort im Schreibmenü",
        "restricted": "Speichern fragt nach einem Passwort",
        "kommentar-leer": "Speichern fragt nach einem Passwort",
        "winzig": "Hinweis auf die unverschlüsselte Datei",
    },
    "sc3.py": {
        "anhaengen-B": "M-K Passwort im Schreibmenü",
        "anhaengen": "M-K Passwort im Schreibmenü",
        "anhaengen-mark": "M-K Passwort im Schreibmenü",
    },
    "sc4.py": {
        "dos": "M-K Passwort im Schreibmenü",
        "schreiben-menu": "M-K Passwort im Schreibmenü",
        "backup-dir": "M-K Passwort im Schreibmenü",
        "nurlesen": "Speichern fragt nach einem Passwort",
        "groesse-wechsel": "Hilfetext beschreibt die Verschlüsselung",
        "einzeilig": "Hilfetext beschreibt die Verschlüsselung",
        "hilfe-eng": "Hilfetext beschreibt die Verschlüsselung",
        "farblos-optionen": "Hilfetext beschreibt die Verschlüsselung",
        "zweizeilig": "Hinweis auf die unverschlüsselte Datei",
        "einfuegen-conv": "Übersetzungsfehler von nano berichtigt",
    },
    "sc5.py": {
        "hilfe-voll": "Hilfetext beschreibt die Verschlüsselung",
        "hilfe-voll-modern": "Hilfetext beschreibt die Verschlüsselung",
        "hilfe-schreiben": "Hilfetext beschreibt die Verschlüsselung",
        "hilfe-einfuegen": "Hilfetext beschreibt die Verschlüsselung",
        "hilfe-ausfuehren": "Hilfetext beschreibt die Verschlüsselung",
        "hilfe-cursor": "Hilfetext beschreibt die Verschlüsselung",
        "modern-tasten": "Hilfetext beschreibt die Verschlüsselung",
    },
    "sc6.py": {
        "vt100-maus": "Hilfetext beschreibt die Verschlüsselung",
        "hilfe-maus": "Hilfetext beschreibt die Verschlüsselung",
        "janein-maus": "M-K Passwort im Schreibmenü",
    },
    "sc7.py": {
        "columns-groesser": "COLUMNS breiter als das Terminal",
        "columns-resize": "COLUMNS breiter als das Terminal",
        "term-dumb": "TERM=dumb wird abgelehnt",
        "term-unbekannt": "Meldung von ncurses auf Deutsch",
        "term-leer": "Meldung von ncurses auf Deutsch",
    },
}


def prerequisites():
    problems = []
    for tool in ("tmux", "strace", "screen", "localedef"):
        if not shutil.which(tool):
            problems.append("%s fehlt" % tool)
    try:
        first = subprocess.run([cmp.NANO, "--version"], capture_output=True, text=True).stdout.split("\n")[0]
    except OSError:
        first = ""
    if not first.rstrip().endswith("9.3"):
        problems.append("%s ist nicht GNU nano 9.3 (NANO setzen)" % cmp.NANO)
    for path in (cmp.ZU, cmp.DEC):
        if not os.access(path, os.X_OK):
            problems.append("%s fehlt (cargo build --release)" % path)
    locale = os.path.join(cmp.LOCPATH, "de_DE.UTF-8")
    if not problems and not os.path.isdir(locale):
        os.makedirs(cmp.LOCPATH, exist_ok=True)
        subprocess.run(["localedef", "-i", "de_DE", "-f", "UTF-8", locale], capture_output=True)
        if not os.path.isdir(locale):
            problems.append("Gebietsschema de_DE.UTF-8 kann nicht erzeugt werden")
    return problems


def load(name):
    spec = importlib.util.spec_from_file_location(name[:-3], os.path.join(cmp.T, name))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def golden_check(suite, scenario, update):
    title, opts, keys = scenario[:3]
    kw = scenario[3] if len(scenario) > 3 else {}
    shots, _ = cmp.run(cmp.ZU, "k", opts, keys, **kw)
    text = "\n\f\n".join(cmp.norm(s) for s in shots) + "\n"
    path = os.path.join(GOLDEN, suite[:-3], title + ".txt")
    if update:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)
        print("neu  %s (%s)" % (title, INTENDED[suite][title]))
        return True
    if not os.path.exists(path):
        print("FEHL %s (keine bestätigte Ausgabe, mit --update anlegen)" % title)
        return False
    with open(path, encoding="utf-8") as f:
        expected = f.read()
    if text == expected:
        print("ok   %s (beabsichtigt: %s)" % (title, INTENDED[suite][title]))
        return True
    print("=== UNTERSCHIED zur bestätigten Ausgabe: %s ===" % title)
    sys.stdout.writelines(difflib.unified_diff(expected.splitlines(True), text.splitlines(True), "bestätigt", "jetzt"))
    print("FEHL " + title)
    return False


def scenarios(update, only):
    failed = []
    for suite in SUITES:
        if only and only != suite[:-3]:
            continue
        print("### " + suite)
        for scenario in load(suite).SCENARIOS:
            title = scenario[0]
            if title in INTENDED.get(suite, {}):
                ok = golden_check(suite, scenario, update)
            elif update:
                continue
            else:
                kw = scenario[3] if len(scenario) > 3 else {}
                ok = cmp.compare(title, scenario[1], scenario[2], **kw)
            if not ok:
                failed.append("%s:%s" % (suite[:-3], title))
    return failed


def script(command, title):
    print("### " + title)
    sys.stdout.flush()
    done = subprocess.run(command, cwd=cmp.T)
    return [] if done.returncode == 0 else [title]


def main():
    parser = argparse.ArgumentParser(description="Vergleicht zu mit GNU nano 9.3")
    parser.add_argument("--update", action="store_true", help="bestätigte Ausgaben der beabsichtigten Abweichungen neu schreiben")
    parser.add_argument("--only", help="nur diese Suite, etwa sc1")
    args = parser.parse_args()
    problems = prerequisites()
    if problems:
        for p in problems:
            print("Voraussetzung: " + p)
        sys.exit(2)
    failed = scenarios(args.update, args.only)
    if not args.only and not args.update:
        python = sys.executable
        failed += script([python, "cursor.py"], "cursor")
        failed += script([python, "justify.py"], "justify")
        failed += script(["bash", "scrollback.sh", cmp.ZU], "scrollback")
        if os.geteuid() == 0:
            failed += script([python, "security.py"], "security")
        else:
            print("### security übersprungen (nur als root)")
    print()
    if failed:
        print("Abweichungen: " + ", ".join(failed))
        sys.exit(1)
    print("Alle Prüfungen bestanden.")


if __name__ == "__main__":
    main()
