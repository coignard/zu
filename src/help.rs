use crate::curses::Win;
use crate::definitions::{
    BLIND, END_OF_PASTE, Flag, Func, HIGHEST_POSITIVE, KEY_MOUSE, MBROWSER, MEXECUTE, MGOTODIR,
    MGOTOLINE, MHELP, MINSERTFILE, MMAIN, MREPLACE, MREPLACEWITH, MWHEREIS, MWHEREISFILE,
    MWRITEFILE, Msg, START_OF_PASTE, THE_WINDOW_RESIZED, TOGGLE_FLAGS,
};
use crate::global::Zu;
use crate::utils::i64_of;

impl Zu {
    fn help_init(&mut self) {
        let menu = self.currmenu;
        let intro: String = if menu & (MWHEREIS | MREPLACE) != 0 {
            String::from(
                "Hilfetext für den Suchbefehl\n\n Geben Sie die Wörter oder Buchstaben ein, nach denen Sie suchen möchten, und drücken Sie die Eingabetaste. Wenn eine Übereinstimmung für den eingegebenen Text gefunden wird, dann wird die Anzeige zur nächsten Fundstelle bewegt.\n\n Die vorhergehende Suchzeichenkette wird in eckigen Klammern nach der Such-Aufforderung angezeigt. Drücken der Eingabetaste ohne Eingabe irgendwelchen Textes führt die vorhergehende Suche durch. ",
            ) + "Wenn Sie Text markiert haben und dann suchen, um zu ersetzen, werden nur Treffer in der ausgewählten Region ersetzt.\n\n Die folgenden Funktionstasten sind im Suchmodus verfügbar:\n\n"
        } else if menu == MREPLACEWITH {
            String::from(
                "=== Ersetzung ===\n\n Geben Sie die Zeichen ein, welche das ersetzen sollen, was Sie in der vorherigen Eingabeaufforderung eingegeben haben, und drücken Sie die Eingabetaste.\n\n",
            ) + " Die folgenden Funktionstasten sind in dieser\n Eingabeaufforderung verfügbar:\n\n"
        } else if menu == MGOTOLINE {
            String::from(
                "Hilfetext für „Zu Zeile“\n\n Geben Sie die Nummer der gewünschten Zeile ein und drücken Sie die Eingabetaste. Gibt es weniger Zeilen im Text, so springt Zu zur letzten Zeile der Datei.\n\n Die folgenden Funktionstasten sind im Zu-Zeile-Modus verfügbar:\n\n",
            )
        } else if menu == MINSERTFILE {
            String::from(
                "Hilfetext für „Datei einfügen“\n\n Geben Sie den Namen einer Datei ein, die an der momentanen Cursorposition in den momentanen Dateipuffer eingefügt werden soll. Für eine verschlüsselte Datei wird das Passwort abgefragt.\n\n Wenn Sie mehrere Dateipuffer mit den Befehlszeilenoptionen -F oder --newbuffer, mit der Tastenkombination Meta-F oder mittels einer zurc-Datei eingeschaltet haben, so wird die Einfügeoperation die Datei in einen separaten Puffer laden. (Benutzen Sie M-, und M-., um zwischen den Dateipuffern umzuschalten.) ",
            ) + "Wenn Sie einen weiteren leeren Puffer benötigen, drücken Sie einfach die Eingabetaste, ohne einen Dateinamen einzugeben, oder geben Sie einen nichtexistierenden Dateinamen an.\n\n Die folgenden Funktionstasten sind im Datei-Einfügen-Modus verfügbar:\n\n"
        } else if menu == MWRITEFILE {
            String::from(
                "Hilfetext für „Datei schreiben“\n\n Geben Sie den Namen ein, unter dem Sie die momentane Datei speichern wollen, und drücken Sie die Eingabetaste. Die Datei wird verschlüsselt. Fehlt ein Passwort, wird es abgefragt.\n\n Wenn Sie Text markiert haben, so werden Sie gefragt, ob Sie nur den ausgewählten Teil des Textes in eine separate Datei schreiben wollen. Um die Gefahr zu minimieren, dass eine Datei mit einem Teil von ihr selbst überschrieben wird, ist der aktuelle Dateiname in diesem Modus nicht die Vorgabe.\n\n Die folgenden Funktionstasten sind im Datei-Schreiben-Modus verfügbar:\n\n",
            )
        } else if menu == MBROWSER {
            String::from(
                "Hilfetext für den Dateibrowser\n\n Mit dem Dateiwähler kann die Verzeichnisstruktur visuell durchsucht werden, um eine Datei für Lese- und Schreiboperationen auszuwählen. Sie können die Pfeiltasten oder Bild-Auf/Ab benutzen, um durch die Dateien zu blättern, und S oder die Eingabe, um die markierte Datei bzw. das markierte Verzeichnis auszuwählen. Um sich eine Ebene aufwärts zu bewegen, wählen Sie das Verzeichnis namens „..“ ganz oben in der Dateiliste.\n\n Die folgenden Funktionstasten sind im Dateibrowser verfügbar:\n\n",
            )
        } else if menu == MWHEREISFILE {
            String::from(
                "Hilfetext für den Suchbefehl des Dateibrowsers\n\n Geben Sie die Wörter oder Zeichen ein, nach denen Sie suchen möchten, und drücken Sie die Eingabetaste. Wenn eine Übereinstimmung für den eingegebenen Text gefunden wird, dann wird die Anzeige zur nächsten Fundstelle bewegt.\n\n Die vorhergehende Suchzeichenkette wird in eckigen Klammern nach der Such-Aufforderung angezeigt. Drücken der Eingabetaste ohne Eingabe irgendwelchen Textes führt die vorhergehende Suche durch.\n\n",
            ) + " Die folgenden Funktionstasten sind in dieser\n Eingabeaufforderung verfügbar:\n\n"
        } else if menu == MGOTODIR {
            String::from(
                "Hilfetext für Gehe-zu-Verzeichnis im Dateibrowser\n\n Geben Sie den Namen des Verzeichnisses ein, das Sie durchsuchen wollen.\n\n Wenn Tabulator-Vervollständigung nicht abgeschaltet worden ist, so können Sie die Tabulator-Taste benutzen, um (zu versuchen) den Verzeichnisnamen automatisch vervollständigen zu lassen.\n\n Die folgenden Funktionstasten sind im Gehe-zu-Verzeichnis-Modus des Dateibrowsers verfügbar:\n\n",
            )
        } else if menu == MEXECUTE {
            String::from(
                "Hilfetext für Externen Befehl\n\n Dieses Menü erlaubt es, die Ausgabe eines externen Befehls, der in der Shell ausgeführt wird, in den aktuellen Puffer einzufügen (oder in einen neuen Puffer). Der Puffer kann nicht an einen Befehl weitergeleitet werden. ",
            ) + "Falls Sie nur einen weiteren leeren Puffer benötigen, geben Sie keinen Befehl ein.\n\n Sie können auch die gesamte Datei ausrichten, ein großes Stück des Puffers ausschneiden oder den Editor schlafen schicken.\n\n"
                + " Die folgenden Funktionstasten sind in dieser\n Eingabeaufforderung verfügbar:\n\n"
        } else {
            String::from(
                "Haupt-Hilfe für Zu\n\n Zu ist ein Nachbau von GNU nano, der Dateien verschlüsselt speichert. Es gibt vier Hauptbereiche: Die Titelzeile zeigt die Version des Programms, den Namen der momentan editierten Datei und ob die Datei verändert wurde oder nicht. Das Hauptfenster enthält die zu bearbeitende Datei. Die Statuszeile (die dritte Zeile von unten) zeigt wichtige Meldungen. ",
            ) + "Die untersten zwei Zeilen listen die meistgebrauchten Tastenkombinationen von Zu auf.\n\n Tastenkombinationen werden wie folgt abgekürzt: Kombinationen mit der Strg-Taste werden mit einem ^ ausgedrückt und können auch eingegeben werden, indem Esc zweimal gedrückt wird. Escape-Sequenzen werden mit dem Meta-Symbol (M) angegeben und können mit Alt, Cmd oder Meta eingegeben werden, abhängig von Ihrer Tastatureinstellung. "
                + "Zudem kann durch zweimaliges Drücken von Esc und anschließender Eingabe einer dreistelligen Zahl von 000 bis 255 das Zeichen mit dem entsprechenden Wert eingegeben werden. Die folgenden Tasten(-kombinationen) sind im Hauptfenster verfügbar. Alternative Tasten stehen in Klammern:\n\n"
        };
        let mut text: Vec<u8> = intro.into_bytes();
        self.end_of_intro = text.len();
        let funcs: Vec<(Func, &'static str, bool, u32)> = self
            .allfuncs
            .iter()
            .map(|f| (f.func, f.phrase, f.blank_after, f.menus))
            .collect();
        for (func, phrase, blank_after, menus) in funcs {
            if menus & menu == 0 {
                continue;
            }
            let mut tally = 0;
            for sc in &self.sclist {
                if sc.menus & menu != 0 && sc.func == func && !sc.keystr.is_empty() {
                    tally += 1;
                    if tally == 1 {
                        let w = 7;
                        let s = format!("{:<w$}", sc.keystr, w = w);
                        text.extend_from_slice(s.as_bytes());
                    } else {
                        let w = 10;
                        let s = format!("{:<w$}", format!("({})", sc.keystr), w = w);
                        text.extend_from_slice(s.as_bytes());
                        break;
                    }
                }
            }
            if tally == 0 {
                text.extend_from_slice(b"\t\t ");
            } else if tally == 1 {
                text.extend_from_slice(b"          ");
            }
            text.extend_from_slice(phrase.as_bytes());
            text.push(b'\n');
            if blank_after {
                text.push(b'\n');
            }
        }
        if menu == MMAIN {
            for flag in &TOGGLE_FLAGS {
                if let Some(sc) = self.sclist.iter().find(|s| s.toggle == Some(*flag)) {
                    let key = if sc.menus & MMAIN != 0 {
                        sc.keystr.clone()
                    } else {
                        String::new()
                    };
                    let s = format!(
                        "{}\t\t {} {}\n",
                        key,
                        crate::global::epithet_of_flag(*flag),
                        "aktivieren/deaktivieren"
                    );
                    text.extend_from_slice(s.as_bytes());
                    if matches!(flag, Flag::LineNumbers | Flag::SoftWrap | Flag::AutoIndent) {
                        text.push(b'\n');
                    }
                }
            }
        }
        self.help_text = text;
    }

    pub fn wrap_help_text_into_buffer(&mut self) {
        let cols = self.cols();
        let mut wrapping_point = cols.clamp(40, 74) - self.sidebar;
        let text = self.help_text.clone();
        let mut ptr = self.start_of_body;
        let mut sum = 0usize;
        self.make_new_buffer();
        if (self.isset(Flag::MiniBar) || !self.isset(Flag::EmptyLine)) && self.lines() > 6 {
            let c = self.current();
            self.nodes[c].data = b" ".to_vec();
            let n = self.make_new_node(Some(c));
            self.nodes[c].next = Some(n);
            self.ofile_mut().current = n;
        }
        while ptr < text.len() {
            if ptr == self.end_of_intro {
                wrapping_point = (if cols < 40 { 40 } else { cols }) - self.sidebar;
            }

            let length: usize;
            let oneline: Vec<u8> = if ptr < self.end_of_intro || text[ptr - 1] == b'\n' {
                length = (self
                    .break_line(&text[ptr..], wrapping_point.cast_signed(), true)
                    .max(1))
                .cast_unsigned();
                let shim = usize::from(text[ptr + length - 1] != b' ');
                let take = (length + shim).saturating_sub(1).min(length);
                text[ptr..ptr + take].to_vec()
            } else {
                length = (self
                    .break_line(
                        &text[ptr..],
                        (if cols < 40 { 22 } else { cols - 18 }).cast_signed()
                            - self.sidebar.cast_signed(),
                        true,
                    )
                    .max(1))
                .cast_unsigned();
                let mut v = b"\t\t  ".to_vec();
                v.extend_from_slice(&text[ptr..ptr + length]);
                v
            };
            let c = self.current();
            self.nodes[c].data = oneline;
            ptr += length;
            if ptr >= text.len() || text[ptr] != b'\n' {
                ptr = ptr.saturating_sub(1);
            }
            loop {
                let c = self.current();
                let n = self.make_new_node(Some(c));
                self.nodes[c].next = Some(n);
                self.nodes[n].data = Vec::new();
                self.ofile_mut().current = n;
                ptr += 1;
                if !(ptr < text.len() && text[ptr] == b'\n') {
                    break;
                }
            }
        }
        let c = self.current();
        let top = self.filetop();
        let of = self.ofile_mut();
        of.filebot = c;
        of.current = top;
        self.remove_magicline();
        self.prepare_for_display();
        loop {
            let c = self.current();
            sum += self.datalen(c);
            if sum > self.location {
                break;
            }
            match self.next(c) {
                Some(n) => self.ofile_mut().current = n,
                None => break,
            }
        }
        let c = self.current();
        self.ofile_mut().edittop = c;
    }

    fn show_help(&mut self) {
        let oldmenu = self.currmenu;
        let was_margin = self.margin;
        let was_tabsize = self.tabsize;
        let was_lighted = self.spotlighted;
        let saved_answer = self.answer.clone();
        let stash = self.flags;
        if self.isset(Flag::NoHelp) || self.isset(Flag::Zero) {
            self.flags.unset(Flag::NoHelp);
            self.flags.unset(Flag::Zero);
            self.window_init();
        } else {
            self.blank_statusbar();
        }
        self.flags.unset(Flag::BackwardsSearch);
        self.flags.unset(Flag::CaseSensitive);
        self.flags.unset(Flag::UseRegexp);
        self.flags.unset(Flag::WhitespaceDisplay);
        self.editwincols = self.cols() - self.sidebar;
        self.margin = 0;
        self.tabsize = 8;
        self.spotlighted = false;
        self.term.curs_set(false);
        self.help_init();
        self.inhelp = true;
        self.location = 0;
        self.didfind = 0;
        self.bottombars(MHELP);
        let text = self.help_text.clone();
        let length = (self
            .break_line(&text, HIGHEST_POSITIVE.cast_signed(), true)
            .max(0))
        .cast_unsigned();
        self.title = Some(text[..length].to_vec());
        let t = self.title.clone();
        self.titlebar(t.as_deref());
        let mut sob = length;
        while sob < text.len() && text[sob] == b'\n' {
            sob += 1;
        }
        self.start_of_body = sob;
        self.wrap_help_text_into_buffer();
        self.edit_refresh();
        loop {
            self.lastmessage = Msg::Vacuum;
            self.focusing = true;
            let kbinput =
                self.get_kbinput(Win::Mid, self.didfind == 1 || self.isset(Flag::ShowCursor));
            self.didfind = 0;
            self.spotlighted = false;
            let function = self.interpret(kbinput);
            if self.isset(Flag::ShowCursor)
                && matches!(
                    function,
                    Some(Func::Left | Func::Right | Func::Up | Func::Down)
                )
            {
                self.execute_function(function.unwrap());
            } else if matches!(function, Some(Func::Up | Func::ScrollUp)) {
                self.do_scroll_up();
            } else if matches!(function, Some(Func::Down | Func::ScrollDown)) {
                if self.lineno(self.edittop()) + i64_of(self.editwinrows) - 1
                    < self.lineno(self.filebot())
                {
                    self.do_scroll_down();
                }
            } else if matches!(
                function,
                Some(Func::PageUp | Func::PageDown | Func::FirstLine | Func::LastLine)
            ) {
                self.execute_function(function.unwrap());
            } else if matches!(
                function,
                Some(
                    Func::SearchBackward
                        | Func::SearchForward
                        | Func::FindPrevious
                        | Func::FindNext
                )
            ) {
                self.execute_function(function.unwrap());
                self.bottombars(MHELP);
            } else if function == Some(Func::Implant) {
                if let Some(exp) = self
                    .first_sc_for(MHELP, Func::Implant)
                    .and_then(|s| s.expansion.clone())
                {
                    self.implant(&exp);
                }
            } else if kbinput == KEY_MOUSE {
                let _ = self.get_mouseinput();
            } else if kbinput == START_OF_PASTE {
                while self.get_kbinput(Win::Mid, BLIND) != END_OF_PASTE {}
                self.statusline(Msg::Ahem, "Einfügen wird ignoriert");
            } else if kbinput == THE_WINDOW_RESIZED {
            } else if function == Some(Func::FullRefresh) {
                self.full_refresh();
            } else if function == Some(Func::Exit) {
                break;
            } else {
                self.unbound_key(kbinput);
            }
            self.edit_refresh();
            self.location = 0;
            let mut line = Some(self.filetop());
            while let Some(l) = line {
                if l == self.edittop() {
                    break;
                }
                self.location += self.datalen(l);
                line = self.next(l);
            }
        }
        self.close_buffer();
        self.flags = stash;
        self.margin = was_margin;
        self.editwincols = self.cols() - self.margin - self.sidebar;
        self.tabsize = was_tabsize;
        self.spotlighted = was_lighted;
        self.title = None;
        self.answer = saved_answer;
        self.help_text.clear();
        self.inhelp = false;
        self.term.curs_set(false);
        if self.isset(Flag::NoHelp) || self.isset(Flag::Zero) {
            self.window_init();
        } else {
            self.blank_statusbar();
        }
        self.bottombars(oldmenu);
        if oldmenu & (MBROWSER | MGOTODIR | MWHEREISFILE) != 0 {
            self.browser_refresh();
        } else {
            self.titlebar(None);
            self.edit_refresh();
        }
    }

    pub fn do_help(&mut self) {
        self.show_help();
    }
}
