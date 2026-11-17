use crate::utils::i16_of;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use crate::chars::{char_length, has_blank_char, mbstrlen};
use crate::curses::{A_BOLD, A_ITALIC, Attr};
use crate::definitions::{
    COLUMNS_FROM_EOL, ESC_CODE, Elem, Flag, Func, MBROWSER, MEXECUTE, MFINDINHELP, MGOTODIR,
    MGOTOLINE, MHELP, MINSERTFILE, MLINTER, MMAIN, MMOST, MREPLACE, MREPLACEWITH, MSPELL, MWHEREIS,
    MWHEREISFILE, MWRITEFILE, MYESNO, Shortcut,
};
use crate::global::Zu;
use crate::utils::{bytes_to_path, concatenate, io_errstr, parse_num};

const RCFILE_NAME: &[u8] = b"zurc";
const HOME_RC_NAME: &[u8] = b".zurc";
const SYSCONFDIR: &[u8] = b"/etc";

const RCOPTS: &[(&str, Option<Flag>)] = &[
    ("boldtext", Some(Flag::BoldText)),
    ("brackets", None),
    ("breaklonglines", Some(Flag::BreakLongLines)),
    ("casesensitive", Some(Flag::CaseSensitive)),
    ("constantshow", Some(Flag::ConstantShow)),
    ("fill", None),
    ("historylog", Some(Flag::HistoryLog)),
    ("linenumbers", Some(Flag::LineNumbers)),
    ("mouse", Some(Flag::UseMouse)),
    ("multibuffer", Some(Flag::NewBuffer)),
    ("newbuffer", Some(Flag::NewBuffer)),
    ("nohelp", Some(Flag::NoHelp)),
    ("nonewlines", Some(Flag::NoNewlines)),
    ("nowrap", Some(Flag::NoWrap)),
    ("operatingdir", None),
    ("positionlog", Some(Flag::PositionLog)),
    ("preserve", Some(Flag::Preserve)),
    ("punct", None),
    ("quotestr", None),
    ("quickblank", Some(Flag::QuickBlank)),
    ("rawsequences", Some(Flag::RawSequences)),
    ("rebinddelete", Some(Flag::RebindDelete)),
    ("regexp", Some(Flag::UseRegexp)),
    ("saveonexit", Some(Flag::SaveOnExit)),
    ("afterends", Some(Flag::AfterEnds)),
    ("allow_insecure_backup", Some(Flag::InsecureBackup)),
    ("atblanks", Some(Flag::AtBlanks)),
    ("autoindent", Some(Flag::AutoIndent)),
    ("backup", Some(Flag::MakeBackup)),
    ("backupdir", None),
    ("bookstyle", Some(Flag::BookStyle)),
    ("colonparsing", Some(Flag::ColonParsing)),
    ("cutfromcursor", Some(Flag::CutFromCursor)),
    ("emptyline", Some(Flag::EmptyLine)),
    ("guidestripe", None),
    ("indicator", Some(Flag::Indicator)),
    ("jumpyscrolling", Some(Flag::JumpyScrolling)),
    ("locking", Some(Flag::Locking)),
    ("matchbrackets", None),
    ("minibar", Some(Flag::MiniBar)),
    ("noconvert", Some(Flag::NoConvert)),
    ("showcursor", Some(Flag::ShowCursor)),
    ("smarthome", Some(Flag::SmartHome)),
    ("softwrap", Some(Flag::SoftWrap)),
    ("solosidescroll", Some(Flag::SoloSidescroll)),
    ("stateflags", Some(Flag::StateFlags)),
    ("tabsize", None),
    ("tabstospaces", Some(Flag::TabsToSpaces)),
    ("trimblanks", Some(Flag::TrimBlanks)),
    ("unix", Some(Flag::MakeItUnix)),
    ("whitespace", None),
    ("whitespacedisplay", Some(Flag::WhitespaceDisplay)),
    ("wordbounds", Some(Flag::WordBounds)),
    ("wordchars", None),
    ("zap", Some(Flag::LetThemZap)),
    ("zero", Some(Flag::Zero)),
    ("titlecolor", None),
    ("numbercolor", None),
    ("stripecolor", None),
    ("scrollercolor", None),
    ("selectedcolor", None),
    ("spotlightcolor", None),
    ("minicolor", None),
    ("promptcolor", None),
    ("statuscolor", None),
    ("errorcolor", None),
    ("keycolor", None),
    ("functioncolor", None),
    ("argonmemory", None),
    ("argonpasses", None),
];

const MENUNAMES: &[(&str, u32)] = &[
    ("main", MMAIN),
    ("search", MWHEREIS),
    ("replace", MREPLACE),
    ("replacewith", MREPLACEWITH),
    ("yesno", MYESNO),
    ("gotoline", MGOTOLINE),
    ("writeout", MWRITEFILE),
    ("insert", MINSERTFILE),
    ("execute", MEXECUTE),
    ("help", MHELP),
    ("spell", MSPELL),
    ("linter", MLINTER),
    ("browser", MBROWSER),
    ("whereisfile", MWHEREISFILE),
    ("gotodir", MGOTODIR),
    ("all", MMOST | MBROWSER | MHELP | MYESNO),
];

const HUES: &[(&str, i16)] = &[
    ("red", 1),
    ("green", 2),
    ("blue", 4),
    ("yellow", 3),
    ("cyan", 6),
    ("magenta", 5),
    ("white", 7),
    ("black", 0),
    ("normal", -1),
    ("pink", 204),
    ("purple", 163),
    ("mauve", 134),
    ("lagoon", 38),
    ("mint", 48),
    ("lime", 148),
    ("peach", 215),
    ("orange", 208),
    ("latte", 137),
    ("rosy", 175),
    ("beet", 127),
    ("plum", 98),
    ("sea", 32),
    ("sky", 111),
    ("slate", 66),
    ("teal", 35),
    ("sage", 107),
    ("brown", 100),
    ("ocher", 142),
    ("sand", 186),
    ("tawny", 136),
    ("brick", 166),
    ("crimson", 161),
    ("grey", 8),
    ("gray", 8),
];

fn parse_next_word(line: &[u8], pos: &mut usize) -> Vec<u8> {
    let start = *pos;
    while *pos < line.len() && line[*pos] != b' ' && line[*pos] != b'\t' {
        *pos += 1;
    }
    let word = line[start..*pos].to_vec();
    while *pos < line.len() && (line[*pos] == b' ' || line[*pos] == b'\t') {
        *pos += 1;
    }
    word
}

impl Zu {
    pub fn display_rcfile_errors(&self) {
        for e in &self.rc_errors {
            warn!("{}\n", e);
        }
    }

    pub fn jot_error(&mut self, msg: &str) {
        if self.startup_problem.is_none() {
            self.startup_problem = Some(self.nanorc.as_ref().map_or_else(
                || "Probleme mit der Verlaufsdatei".to_string(),
                |rc| {
                    fmtb!("Fehler in „%s“", rc)
                        .iter()
                        .map(|&b| b as char)
                        .collect::<String>()
                        .replace('\u{0}', "")
                },
            ));
            if let Some(rc) = &self.nanorc {
                self.startup_problem =
                    Some(String::from_utf8_lossy(&fmtb!("Fehler in „%s“", rc)).into_owned());
            }
        }
        let mut text = if self.rc_lineno > 0
            && let Some(rc) = &self.nanorc
        {
            String::from_utf8_lossy(&fmtb!("Fehler in %s in Zeile %zu: ", rc, self.rc_lineno))
                .into_owned()
        } else {
            String::new()
        };
        text.push_str(msg);
        self.rc_errors.push(text);
    }

    fn name_to_menu(name: &str) -> u32 {
        MENUNAMES
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(0, |(_, m)| *m)
    }

    fn menu_to_name(menu: u32) -> &'static str {
        MENUNAMES
            .iter()
            .find(|(_, m)| *m == menu)
            .map_or("boooo", |(n, _)| *n)
    }

    const fn is_universal(func: Func) -> bool {
        matches!(
            func,
            Func::Left
                | Func::Right
                | Func::Home
                | Func::End
                | Func::PrevWord
                | Func::NextWord
                | Func::Delete
                | Func::Backspace
                | Func::CutText
                | Func::PasteText
                | Func::Tab
                | Func::Enter
                | Func::Verbatim
        )
    }

    fn parse_binding(&mut self, line: &[u8], mut pos: usize, dobind: bool) {
        if pos >= line.len() {
            self.jot_error("Fehlender Schlüsselname");
            return;
        }
        let keyword = parse_next_word(line, &mut pos);
        let mut keycopy = String::from_utf8_lossy(&keyword).into_owned();
        let kb: Vec<char> = keycopy.chars().collect();
        if kb[0] == '^' {
            if kb.len() > 1 && kb[1].is_ascii_lowercase() {
                keycopy = format!(
                    "^{}{}",
                    kb[1].to_ascii_uppercase(),
                    kb[2..].iter().collect::<String>()
                );
            }
        } else if kb[0].is_ascii_lowercase() {
            keycopy = format!(
                "{}{}",
                kb[0].to_ascii_uppercase(),
                kb[1..].iter().collect::<String>()
            );
        }
        let kb: Vec<char> = keycopy.chars().collect();
        if kb.len() < 2 || (kb[0] == 'M' && kb.len() < 3) {
            self.jot_error(&fmt!("Schlüsselname „%s“ ist ungültig", &keycopy));
            return;
        }
        let keycode = crate::global::keycode_from_string(&keycopy, &mut self.shifted_metas);
        if keycode < 0 {
            self.jot_error(&fmt!("Schlüsselname „%s“ ist ungültig", &keycopy));
            return;
        }
        let mut funcstr: Vec<u8> = Vec::new();
        if dobind {
            if pos >= line.len() {
                self.jot_error("Die an die Taste zu bindende Funktion muss angegeben werden");
                return;
            }
            if line[pos] == b'"' {
                let rest = &line[pos..];
                let last_quote = if let Some(q) = rest[1..].iter().rposition(|&c| c == b'"') {
                    q + 1
                } else {
                    self.jot_error(&fmt!(
                        "Argument „%s“ hat nicht beendetes \"",
                        &String::from_utf8_lossy(rest).into_owned()
                    ));
                    return;
                };
                funcstr = rest[..last_quote].to_vec();
                pos += last_quote + 1;
                while pos < line.len() && (line[pos] == b' ' || line[pos] == b'\t') {
                    pos += 1;
                }
            } else {
                funcstr = parse_next_word(line, &mut pos);
            }
        }
        let menuptr = parse_next_word(line, &mut pos);
        if menuptr.is_empty() {
            self.jot_error(
                "Ein Menü (oder „all“) muss angegeben werden, um Tasten zu binden/zu lösen",
            );
            return;
        }
        let menuname = String::from_utf8_lossy(&menuptr).into_owned();
        let mut menu = Self::name_to_menu(&menuname);
        if menu == 0 {
            self.jot_error(&fmt!("Unbekanntes Menü: %s", &menuname));
            return;
        }
        let mut newsc: Option<Shortcut> = None;
        if dobind {
            if funcstr.first() == Some(&b'"') {
                newsc = Some(Shortcut {
                    keystr: String::new(),
                    keycode: 0,
                    menus: 0,
                    func: Func::Implant,
                    toggle: None,
                    expansion: Some(funcstr[1..].to_vec()),
                });
            } else {
                newsc = strtosc(&String::from_utf8_lossy(&funcstr));
            }
            if newsc.is_none() {
                self.jot_error(&fmt!(
                    "Unbekannte Funktion: %s",
                    &String::from_utf8_lossy(&funcstr).into_owned()
                ));
                return;
            }
        }
        for s in &mut self.sclist {
            if s.menus & menu != 0 && s.keycode == keycode {
                s.menus &= !menu;
            }
        }
        if !dobind {
            return;
        }
        let mut newsc = newsc.unwrap();
        if Self::is_universal(newsc.func) {
            menu &= MMOST | MBROWSER;
        } else if newsc.func == Func::Toggle && newsc.toggle == Some(Flag::NoHelp) {
            menu &= (MMOST | MBROWSER | MYESNO) & !MFINDINHELP;
        } else if newsc.func == Func::Toggle {
            menu &= MMAIN;
        } else if newsc.func == Func::FullRefresh {
            menu &= MMOST | MBROWSER | MHELP | MYESNO;
        } else if newsc.func == Func::Implant {
            menu &= MMOST | MBROWSER | MHELP;
        } else {
            let mut mask = 0;
            for f in &self.allfuncs {
                if f.func == newsc.func {
                    mask |= f.menus;
                }
            }
            menu &= mask;
        }
        if menu == 0 {
            if !self.isset(Flag::Restricted) && !self.isset(Flag::ViewMode) {
                self.jot_error(&fmt!(
                    "Funktion „%s“ existiert im Menü „%s“ nicht",
                    &String::from_utf8_lossy(&funcstr).into_owned(),
                    &menuname
                ));
            }
            return;
        }
        newsc.menus = menu;
        newsc.keystr.clone_from(&keycopy);
        newsc.keycode = keycode;
        if keycode == ESC_CODE {
            self.jot_error(&fmt!("Taste %s kann nicht neu zugewiesen werden", &keycopy));
            return;
        }
        self.sclist.insert(0, newsc);
    }

    fn is_good_file(&mut self, file: &[u8]) -> bool {
        let Ok(c) = std::ffi::CString::new(file) else {
            return false;
        };
        if unsafe { libc::access(c.as_ptr(), libc::R_OK) } < 0 {
            return false;
        }
        if let Ok(md) = fs::metadata(bytes_to_path(file)) {
            use std::os::unix::fs::FileTypeExt;
            let ft = md.file_type();
            if ft.is_dir() || ft.is_char_device() || ft.is_block_device() {
                let m = if ft.is_dir() {
                    fmtb!("„%s“ ist ein Verzeichnis", file)
                } else {
                    fmtb!("„%s“ ist eine Gerätedatei", file)
                };
                self.jot_error(&String::from_utf8_lossy(&m));
                return false;
            }
        }
        true
    }

    fn color_to_short(&mut self, colorname: &str) -> Option<(i16, bool, bool)> {
        let (name, vivid, thick) = colorname
            .strip_prefix("bright")
            .filter(|r| !r.is_empty())
            .map_or_else(
                || {
                    colorname
                        .strip_prefix("light")
                        .filter(|r| !r.is_empty())
                        .map_or((colorname, false, false), |rest| (rest, true, false))
                },
                |rest| (rest, true, true),
            );
        if name.starts_with('#') && name.len() == 4 {
            if vivid {
                self.jot_error(&fmt!("Farbe „%s“ akzeptiert kein Präfix", name));
                return None;
            }
            let digits: Vec<Option<u32>> = name[1..].chars().map(|c| c.to_digit(16)).collect();
            if let (Some(r), Some(g), Some(b)) = (digits[0], digits[1], digits[2]) {
                if self.colors != 256 {
                    return Some((-1, vivid, thick));
                }
                let level = [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5];
                let gray = [1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 15, 18, 21, 23];
                let idx = if r == g && g == b && r > 0 && r < 0xF {
                    232 + gray[(r - 1) as usize]
                } else {
                    36 * level[r as usize] + 6 * level[g as usize] + level[b as usize] + 16
                };
                return Some((i16_of(i64::from(idx)), vivid, thick));
            }
        }
        for (i, (h, idx)) in HUES.iter().enumerate() {
            if *h == name {
                if i > 7 && vivid {
                    self.jot_error(&fmt!("Farbe „%s“ akzeptiert kein Präfix", name));
                    return None;
                } else if i > 8 && self.colors < 255 {
                    return Some((-1, vivid, thick));
                }
                return Some((*idx, vivid, thick));
            }
        }
        self.jot_error(&fmt!("Farbe „%s“ nicht verstanden", name));
        None
    }

    fn parse_combination(&mut self, combotext: &str) -> Option<Attr> {
        let mut bits = 0u8;
        let mut text = combotext;
        if let Some(rest) = text.strip_prefix("bold") {
            bits |= A_BOLD;
            if !rest.starts_with(',') {
                self.jot_error("Auf ein Argument muss ein Komma folgen");
                return None;
            }
            text = &rest[1..];
        }
        if let Some(rest) = text.strip_prefix("italic") {
            bits |= A_ITALIC;
            if !rest.starts_with(',') {
                self.jot_error("Auf ein Argument muss ein Komma folgen");
                return None;
            }
            text = &rest[1..];
        }
        let (fgname, bgname) = text
            .find(',')
            .map_or((text, None), |i| (&text[..i], Some(&text[i + 1..])));
        let mut fg: i16 = -1;
        let mut bg: i16 = -1;
        if !fgname.is_empty() {
            let (c, vivid, thick) = self.color_to_short(fgname)?;
            fg = c;
            if vivid && !thick && self.colors > 8 {
                fg += 8;
            } else if vivid {
                bits |= A_BOLD;
            }
        }
        if let Some(bgname) = bgname {
            let (c, vivid, _) = self.color_to_short(bgname)?;
            bg = c;
            if vivid && self.colors > 8 {
                bg += 8;
            }
        }
        Some(Attr::colored(fg, bg, bits))
    }

    fn set_interface_color(&mut self, element: Elem, combotext: &str) {
        if let Some(a) = self.parse_combination(combotext) {
            self.color_combo[element as usize] = Some(a);
        }
    }

    fn check_vitals_mapped(&mut self) {
        let vitals = [
            (Func::Exit, MMAIN),
            (Func::Exit, MBROWSER),
            (Func::Exit, MHELP),
            (Func::Cancel, MYESNO),
        ];
        for (func, menu) in vitals {
            let item = self
                .allfuncs
                .iter()
                .find(|f| f.func == func && f.menus & menu != 0)
                .map(|f| f.tag);
            if let Some(tag) = item
                && self.first_sc_for(menu, func).is_none()
            {
                self.jot_error(&fmt!(
                    "Der Funktion „%s“ im Menü „%s“ ist keine Taste zugeordnet. Abbruch.\n",
                    tag,
                    Self::menu_to_name(menu)
                ));
                self.die(
                        "Bitte starten Sie zu bei Bedarf mit der Option „-I“, um Ihre zurc-Einstellungen anzupassen\n",
                    );
            }
        }
    }

    pub fn parse_rcfile(&mut self, content: &[u8]) {
        self.rc_lineno = 0;
        for raw in content.split(|&b| b == b'\n') {
            self.rc_lineno += 1;
            let mut line = raw.to_vec();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let mut pos = 0;
            while pos < line.len() && (line[pos] == b' ' || line[pos] == b'\t') {
                pos += 1;
            }
            if pos >= line.len() || line[pos] == b'#' {
                continue;
            }
            let keyword = parse_next_word(&line, &mut pos);
            let kw = String::from_utf8_lossy(&keyword).into_owned();

            let set = match kw.as_str() {
                "set" => 1,
                "unset" => -1,
                "bind" => {
                    self.parse_binding(&line, pos, true);
                    continue;
                }
                "unbind" => {
                    self.parse_binding(&line, pos, false);
                    continue;
                }
                "syntax" | "header" | "magic" | "color" | "icolor" | "comment" | "tabgives"
                | "linter" | "formatter" | "include" | "extendsyntax" => continue,
                _ => {
                    self.jot_error(&fmt!("Befehl „%s“ nicht verstanden", &kw));
                    continue;
                }
            };
            if pos >= line.len() {
                self.jot_error("Fehlende Option");
                continue;
            }
            let option = String::from_utf8_lossy(&parse_next_word(&line, &mut pos)).into_owned();
            let Some(entry) = RCOPTS.iter().find(|(n, _)| *n == option) else {
                self.jot_error(&fmt!("Unbekannte Option: %s", &option));
                continue;
            };
            if let Some(flag) = entry.1 {
                if set == 1 {
                    self.flags.set(flag);
                } else {
                    self.flags.unset(flag);
                }
                continue;
            }
            if set == -1 {
                self.jot_error(&fmt!(
                    "Option „%s“ kann nicht zurückgesetzt werden",
                    &option
                ));
                continue;
            }
            if pos >= line.len() {
                self.jot_error(&fmt!("Option „%s“ erfordert ein Argument", &option));
                continue;
            }
            let argument: Vec<u8> = if line[pos] == b'"' {
                let rest = &line[pos + 1..];
                if let Some(q) = rest.iter().rposition(|&c| c == b'"') {
                    rest[..q].to_vec()
                } else {
                    self.jot_error(&fmt!(
                        "Argument „%s“ hat nicht beendetes \"",
                        &String::from_utf8_lossy(&line[pos..]).into_owned()
                    ));
                    continue;
                }
            } else {
                parse_next_word(&line, &mut pos)
            };
            if std::str::from_utf8(&argument).is_err() {
                self.jot_error("Argument ist keine gültige Multibyte-Zeichenkette");
                continue;
            }
            let arg = String::from_utf8_lossy(&argument).into_owned();
            match option.as_str() {
                "titlecolor" => self.set_interface_color(Elem::TitleBar, &arg),
                "numbercolor" => self.set_interface_color(Elem::LineNumber, &arg),
                "stripecolor" => self.set_interface_color(Elem::GuideStripe, &arg),
                "scrollercolor" => self.set_interface_color(Elem::ScrollBar, &arg),
                "selectedcolor" => self.set_interface_color(Elem::SelectedText, &arg),
                "spotlightcolor" => self.set_interface_color(Elem::Spotlighted, &arg),
                "minicolor" => self.set_interface_color(Elem::MiniInfobar, &arg),
                "promptcolor" => self.set_interface_color(Elem::PromptBar, &arg),
                "statuscolor" => self.set_interface_color(Elem::StatusBar, &arg),
                "errorcolor" => self.set_interface_color(Elem::ErrorMessage, &arg),
                "keycolor" => self.set_interface_color(Elem::KeyCombo, &arg),
                "functioncolor" => self.set_interface_color(Elem::FunctionTag, &arg),
                "operatingdir" => self.operating_dir = Some(argument),
                "fill" => {
                    if let Some(n) = parse_num(&argument) {
                        self.fill = n;
                    } else {
                        self.jot_error(&fmt!("Angeforderte Füllgröße „%s“ ist ungültig", &arg));
                        self.fill = -COLUMNS_FROM_EOL;
                    }
                }
                "matchbrackets" => {
                    if has_blank_char(&argument) {
                        self.jot_error("Nicht-Leerraum-Zeichen erforderlich");
                    } else if !mbstrlen(&argument).is_multiple_of(2) {
                        self.jot_error("Gerade Zeichenanzahl erforderlich");
                    } else {
                        self.matchbrackets = argument;
                    }
                }
                "whitespace" => {
                    if mbstrlen(&argument) != 2 || self.breadth(&argument) != 2 {
                        self.jot_error("Zwei einspaltige Zeichen erforderlich");
                    } else {
                        self.whitespace = argument.clone();
                        self.whitelen[0] = char_length(&argument);
                        self.whitelen[1] = char_length(&argument[self.whitelen[0]..]);
                    }
                }
                "punct" => {
                    if has_blank_char(&argument) {
                        self.jot_error("Nicht-Leerraum-Zeichen erforderlich");
                    } else {
                        self.punct = argument;
                    }
                }
                "brackets" => {
                    if has_blank_char(&argument) {
                        self.jot_error("Nicht-Leerraum-Zeichen erforderlich");
                    } else {
                        self.brackets = argument;
                    }
                }
                "quotestr" => self.quotestr = argument,
                "backupdir" => self.backup_dir = Some(argument),
                "wordchars" => self.word_chars = argument,
                "guidestripe" => match parse_num(&argument) {
                    Some(n) if n > 0 => self.stripe_column = n,
                    _ => {
                        self.jot_error(&fmt!("Markierungsspalte „%s“ ist ungültig", &arg));
                        self.stripe_column = 0;
                    }
                },
                "tabsize" => match parse_num(&argument) {
                    Some(n) if n > 0 => self.tabsize = n,
                    _ => {
                        self.jot_error(&fmt!(
                            "Angeforderte Tabulatorbreite „%s“ ist ungültig",
                            &arg
                        ));
                        self.tabsize = -1;
                    }
                },
                "argonmemory" => match parse_num(&argument).and_then(|n| u32::try_from(n).ok()) {
                    Some(n)
                        if (crate::crypto::MIN_MEMORY_MIB..=crate::crypto::MAX_MEMORY_MIB)
                            .contains(&n) =>
                    {
                        self.kdf.mem_kib = n * 1024;
                    }
                    _ => self.jot_error(&fmt!(
                        "Argon2-Speichergröße „%s“ ist ungültig (64 bis 4096 MiB)",
                        &arg
                    )),
                },
                "argonpasses" => match parse_num(&argument).and_then(|n| u32::try_from(n).ok()) {
                    Some(n)
                        if (crate::crypto::MIN_PASSES..=crate::crypto::MAX_PASSES).contains(&n) =>
                    {
                        self.kdf.passes = n;
                    }
                    _ => self.jot_error(&fmt!(
                        "Argon2-Durchlaufzahl „%s“ ist ungültig (3 bis 100)",
                        &arg
                    )),
                },
                _ => {}
            }
        }
        self.rc_lineno = 0;
    }

    fn parse_one_nanorc(&mut self) {
        let rc = self.nanorc.clone().unwrap();
        match fs::read(bytes_to_path(&rc)) {
            Ok(c) => self.parse_rcfile(&c),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                let m = fmtb!("Fehler beim Lesen von %s: %s", &rc, &io_errstr(&e));
                self.jot_error(&String::from_utf8_lossy(&m));
            }
        }
    }

    fn have_nanorc(&mut self, path: Option<&[u8]>, name: &[u8]) -> bool {
        let Some(p) = path else { return false };
        let rc = concatenate(p, name);
        self.nanorc = Some(rc.clone());
        self.is_good_file(&rc)
    }

    pub fn do_rcfiles(&mut self) {
        if let Some(custom) = self.custom_nanorc.clone() {
            let rc = self.get_full_path(&custom);
            match rc {
                Some(r) if fs::metadata(bytes_to_path(&r)).is_ok() => {
                    self.nanorc = Some(r.clone());
                    if self.is_good_file(&r) {
                        self.parse_one_nanorc();
                    }
                }
                _ => self.die("Angegebene Konfigurationsdatei existiert nicht\n"),
            }
        } else {
            if self.have_nanorc(Some(SYSCONFDIR), &concatenate(b"/", RCFILE_NAME)) {
                self.parse_one_nanorc();
            }
            self.get_homedir();
            let home = self.homedir.clone();
            let xdg = std::env::var_os("XDG_CONFIG_HOME").map(|v| v.as_bytes().to_vec());
            let home_rc = concatenate(b"/", HOME_RC_NAME);
            let xdg_rc = concatenate(b"/zu/", RCFILE_NAME);
            let conf_rc = concatenate(b"/.config/zu/", RCFILE_NAME);
            if self.have_nanorc(home.as_deref(), &home_rc)
                || self.have_nanorc(xdg.as_deref(), &xdg_rc)
                || self.have_nanorc(home.as_deref(), &conf_rc)
            {
                self.parse_one_nanorc();
            } else if home.is_none() && xdg.is_none() {
                self.jot_error("Heimatverzeichnis kann nicht gefunden werden! Autsch!");
            }
        }
        self.check_vitals_mapped();
        self.nanorc = None;
    }
}

pub fn strtosc(input: &str) -> Option<Shortcut> {
    let mut toggle: Option<Flag> = None;
    let func = match input {
        "cancel" => Func::Cancel,
        "help" => Func::Help,
        "exit" => Func::Exit,
        "discardbuffer" => Func::DiscardBuffer,
        "writeout" => Func::Writeout,
        "savefile" => Func::Savefile,
        "insert" => Func::Insertfile,
        "whereis" => Func::SearchForward,
        "wherewas" => Func::SearchBackward,
        "findprevious" => Func::FindPrevious,
        "findnext" => Func::FindNext,
        "replace" => Func::Replace,
        "cut" => Func::CutText,
        "copy" => Func::CopyText,
        "paste" => Func::PasteText,
        "execute" => Func::Execute,
        "cutrestoffile" => Func::CutTillEof,
        "zap" => Func::ZapText,
        "mark" => Func::Mark,
        "location" => Func::Location,
        "gotoline" => Func::GotoLineColumn,
        "justify" => Func::Justify,
        "fulljustify" => Func::FullJustify,
        "beginpara" => Func::ParaBegin,
        "endpara" => Func::ParaEnd,
        "comment" => Func::Comment,
        "complete" => Func::Complete,
        "indent" => Func::Indent,
        "unindent" => Func::Unindent,
        "chopwordleft" => Func::ChopPrevWord,
        "chopwordright" => Func::ChopNextWord,
        "findbracket" => Func::FindBracket,
        "wordcount" => Func::WordCount,
        "recordmacro" => Func::RecordMacro,
        "runmacro" => Func::RunMacro,
        "anchor" => Func::Anchor,
        "prevanchor" => Func::PrevAnchor,
        "nextanchor" => Func::NextAnchor,
        "undo" => Func::Undo,
        "redo" => Func::Redo,
        "suspend" => Func::Suspend,
        "left" | "back" => Func::Left,
        "right" | "forward" => Func::Right,
        "up" | "prevline" => Func::Up,
        "down" | "nextline" => Func::Down,
        "scrollleft" => Func::ScrollLeft,
        "scrollright" => Func::ScrollRight,
        "scrollup" => Func::ScrollUp,
        "scrolldown" => Func::ScrollDown,
        "prevword" => Func::PrevWord,
        "nextword" => Func::NextWord,
        "home" => Func::Home,
        "end" => Func::End,
        "prevblock" => Func::PrevBlock,
        "nextblock" => Func::NextBlock,
        "toprow" => Func::TopRow,
        "bottomrow" => Func::BottomRow,
        "center" => Func::Center,
        "cycle" => Func::Cycle,
        "pageup" | "prevpage" => Func::PageUp,
        "pagedown" | "nextpage" => Func::PageDown,
        "firstline" => Func::FirstLine,
        "lastline" => Func::LastLine,
        "prevbuf" => Func::PrevBuffer,
        "nextbuf" => Func::NextBuffer,
        "verbatim" => Func::Verbatim,
        "tab" => Func::Tab,
        "enter" => Func::Enter,
        "delete" => Func::Delete,
        "backspace" => Func::Backspace,
        "refresh" => Func::FullRefresh,
        "casesens" => Func::CaseSens,
        "regexp" => Func::Regexp,
        "backwards" => Func::Backwards,
        "flipreplace" => Func::FlipReplace,
        "older" => Func::OlderItem,
        "newer" => Func::NewerItem,
        "dosformat" => Func::DosFormat,
        "append" => Func::AppendIt,
        "prepend" => Func::PrependIt,
        "backup" => Func::BackItUp,
        "flipexecute" => Func::FlipExecute,
        "flipconvert" => Func::FlipConvert,
        "flipnewbuffer" => Func::FlipNewbuffer,
        "tofiles" | "browser" => Func::ToFiles,
        "gotodir" => Func::GotoDir,
        "firstfile" => Func::FirstFile,
        "lastfile" => Func::LastFile,
        "changepassword" | "password" => Func::ChangePassword,
        _ => {
            toggle = Some(match input {
                "nohelp" => Flag::NoHelp,
                "zero" => Flag::Zero,
                "constantshow" => Flag::ConstantShow,
                "softwrap" => Flag::SoftWrap,
                "linenumbers" => Flag::LineNumbers,
                "whitespacedisplay" => Flag::WhitespaceDisplay,
                "smarthome" => Flag::SmartHome,
                "autoindent" => Flag::AutoIndent,
                "cutfromcursor" => Flag::CutFromCursor,
                "breaklonglines" => Flag::BreakLongLines,
                "tabstospaces" => Flag::TabsToSpaces,
                "mouse" => Flag::UseMouse,
                _ => return None,
            });
            Func::Toggle
        }
    };
    Some(Shortcut {
        keystr: String::new(),
        keycode: 0,
        menus: 0,
        func,
        toggle,
        expansion: None,
    })
}
