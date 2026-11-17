use crate::crypto::KdfParams;
use crate::definitions::{
    ALT_DELETE, ALT_DOWN, ALT_END, ALT_HOME, ALT_INSERT, ALT_LEFT, ALT_PAGEDOWN, ALT_PAGEUP,
    ALT_RIGHT, ALT_UP, COLUMNS_FROM_EOL, CONTROL_DELETE, CONTROL_DOWN, CONTROL_END, CONTROL_HOME,
    CONTROL_LEFT, CONTROL_RIGHT, CONTROL_SHIFT_DELETE, CONTROL_UP, Completion, END_OF_PASTE, Flag,
    Flags, Func, FuncItem, HIGHEST_POSITIVE, History, KEY_BACKSPACE, KEY_CANCEL, KEY_CENTER,
    KEY_DC, KEY_DOWN, KEY_END, KEY_ENTER, KEY_F, KEY_F0, KEY_FRESH, KEY_HOME, KEY_IC, KEY_LEFT,
    KEY_NPAGE, KEY_PPAGE, KEY_RIGHT, KEY_SIC, KEY_UP, LineNode, Lp, MBROWSER, MEXECUTE,
    MFINDINHELP, MGOTOLINE, MHELP, MINSERTFILE, MLINTER, MMAIN, MMOST, MREPLACE, MREPLACEWITH,
    MSOME, MWHEREIS, MWHEREISFILE, MWRITEFILE, MYESNO, Msg, NUMBER_OF_ELEMENTS, OpenFile,
    PLANTED_A_COMMAND, Position, SHIFT_DELETE, SHIFT_TAB, START_OF_PASTE, Shortcut,
};
use crate::regex::Regex;
use crate::utils::u8_of;

use crate::curses::{Attr, Term};
use std::collections::VecDeque;

#[expect(
    clippy::struct_excessive_bools,
    reason = "holds the global boolean state of GNU nano"
)]
pub struct Zu {
    pub term: Term,
    pub nodes: Vec<LineNode>,
    pub free_nodes: Vec<Lp>,
    pub files: Vec<OpenFile>,
    pub cur: usize,
    pub undo_serial: u64,

    pub the_window_resized: bool,
    pub resized_for_browser: bool,
    pub shifted_metas: bool,
    pub meta_key: bool,
    pub shift_held: bool,
    pub mute_modifiers: bool,
    pub on_a_vt: bool,
    pub we_are_running: bool,
    pub more_than_one: bool,
    pub report_size: bool,
    pub ran_a_tool: bool,
    pub foretext: Vec<u8>,
    pub final_status: i32,
    pub inhelp: bool,
    pub title: Option<Vec<u8>>,
    pub refresh_needed: bool,
    pub united_sidescroll: bool,
    pub focusing: bool,
    pub as_an_at: bool,
    pub control_c_was_pressed: bool,
    pub lastmessage: Msg,
    pub pletion_line: Option<Lp>,
    pub also_the_last: bool,
    pub answer: Vec<u8>,
    pub last_search: Vec<u8>,
    pub didfind: i32,
    pub present_path: Vec<u8>,
    pub flags: Flags,
    pub fill: isize,
    pub wrap_at: usize,
    pub editwinrows: usize,
    pub editwincols: usize,
    pub margin: usize,
    pub sidebar: usize,
    pub bardata: Vec<Attr>,
    pub stripe_column: isize,
    pub cycling_aim: i32,
    pub cutbuffer: Option<Lp>,
    pub cutbottom: Option<Lp>,
    pub keep_cutbuffer: bool,
    pub inherited_anchor: bool,
    pub matchbrackets: Vec<u8>,
    pub whitespace: Vec<u8>,
    pub whitelen: [usize; 2],
    pub punct: Vec<u8>,
    pub brackets: Vec<u8>,
    pub quotestr: Vec<u8>,
    pub quotereg: Option<Regex>,
    pub word_chars: Vec<u8>,
    pub tabsize: isize,
    pub backup_dir: Option<Vec<u8>>,
    pub operating_dir: Option<Vec<u8>>,
    pub currmenu: u32,
    pub sclist: Vec<Shortcut>,
    pub allfuncs: Vec<FuncItem>,
    pub exitfunc: usize,
    pub search_history: History,
    pub replace_history: History,
    pub execute_history: History,
    pub search_regexp: Option<Regex>,
    pub regmatches: [libc::regmatch_t; 10],
    pub hilite_attribute: Attr,
    pub color_combo: [Option<Attr>; NUMBER_OF_ELEMENTS],
    pub interface_color_pair: [Attr; NUMBER_OF_ELEMENTS],
    pub homedir: Option<Vec<u8>>,
    pub statedir: Option<Vec<u8>>,
    pub startup_problem: Option<String>,
    pub custom_nanorc: Option<Vec<u8>>,
    pub commandname: Vec<u8>,
    pub planted_shortcut: Option<Shortcut>,
    pub spotlighted: bool,
    pub light_from_col: usize,
    pub light_to_col: usize,

    pub key_buffer: VecDeque<i32>,
    pub plants: Vec<u8>,
    pub plants_pos: usize,
    pub digit_count: i32,
    pub reveal_cursor: bool,
    pub countdown: i32,
    pub from_x: usize,
    pub till_x: usize,
    pub has_more: bool,
    pub is_shorter: bool,
    pub sequel_column: usize,
    pub sw_state: crate::winio::SoftwrapState,
    pub recording: bool,
    pub macro_buffer: Vec<i32>,
    pub previous_macro: Vec<i32>,
    pub milestone: usize,
    pub escapes: i32,
    pub first_escape_was_alone: bool,
    pub last_escape_was_alone: bool,
    pub byte_code: i32,
    pub give_a_hint: bool,
    pub puddle: Vec<u8>,
    pub prompt_puddle: Vec<u8>,
    pub statusline_start_col: usize,

    pub prompt: Vec<u8>,
    pub typing_x: usize,

    pub came_full_circle: bool,

    pub history_changed: bool,
    pub registername: Option<Vec<u8>>,
    pub latest_timestamp: i64,
    pub positions_register: Vec<Position>,

    pub filelist: Vec<Vec<u8>>,
    pub usable_rows: usize,
    pub piles: usize,
    pub gauge: usize,
    pub selected: usize,

    pub help_text: Vec<u8>,
    pub start_of_body: usize,
    pub end_of_intro: usize,
    pub location: usize,

    pub list_of_completions: Vec<Completion>,
    pub pletion_x: usize,
    pub scouring: usize,

    pub rc_errors: Vec<String>,
    pub nanorc: Option<Vec<u8>>,
    pub rc_lineno: usize,

    pub keytable: crate::curses::terminfo::KeyTable,
    pub pending_input: Vec<u8>,
    pub keypad_on: bool,
    pub did_credits: bool,
    pub terminfo_kbs: Option<Vec<u8>>,
    pub oldinterval: i32,
    pub colors: i32,
    pub term_has_colors: bool,
    pub reading_plaintext: bool,
    pub kdf: KdfParams,
    pub original_termios: Option<libc::termios>,
    pub stdin_flags: i32,
}

impl Zu {
    pub fn new() -> Self {
        let term = Term::new();
        let mut k = Self {
            term,
            nodes: Vec::new(),
            free_nodes: Vec::new(),
            files: Vec::new(),
            cur: 0,
            undo_serial: 0,
            the_window_resized: false,
            resized_for_browser: false,
            shifted_metas: false,
            meta_key: false,
            shift_held: false,
            mute_modifiers: false,
            on_a_vt: false,
            we_are_running: false,
            more_than_one: false,
            report_size: true,
            ran_a_tool: false,
            foretext: Vec::new(),
            final_status: 0,
            inhelp: false,
            title: None,
            refresh_needed: false,
            united_sidescroll: true,
            focusing: true,
            as_an_at: true,
            control_c_was_pressed: false,
            lastmessage: Msg::Vacuum,
            pletion_line: None,
            also_the_last: false,
            answer: Vec::new(),
            last_search: Vec::new(),
            didfind: 0,
            present_path: Vec::new(),
            flags: Flags::default(),
            fill: -COLUMNS_FROM_EOL,
            wrap_at: 0,
            editwinrows: 0,
            editwincols: 0,
            margin: 0,
            sidebar: 0,
            bardata: Vec::new(),
            stripe_column: 0,
            cycling_aim: 0,
            cutbuffer: None,
            cutbottom: None,
            keep_cutbuffer: false,
            inherited_anchor: false,
            matchbrackets: Vec::new(),
            whitespace: Vec::new(),
            whitelen: [0, 0],
            punct: Vec::new(),
            brackets: Vec::new(),
            quotestr: Vec::new(),
            quotereg: None,
            word_chars: Vec::new(),
            tabsize: -1,
            backup_dir: None,
            operating_dir: None,
            currmenu: MMOST,
            sclist: Vec::new(),
            allfuncs: Vec::new(),
            exitfunc: 0,
            search_history: History::new(),
            replace_history: History::new(),
            execute_history: History::new(),
            search_regexp: None,
            regmatches: unsafe { std::mem::zeroed() },
            hilite_attribute: Attr::REVERSE,
            color_combo: [None; NUMBER_OF_ELEMENTS],
            interface_color_pair: [Attr::NORMAL; NUMBER_OF_ELEMENTS],
            homedir: None,
            statedir: None,
            startup_problem: None,
            custom_nanorc: None,
            commandname: Vec::new(),
            planted_shortcut: None,
            spotlighted: false,
            light_from_col: 0,
            light_to_col: 0,
            key_buffer: VecDeque::new(),
            plants: Vec::new(),
            plants_pos: 0,
            digit_count: 0,
            reveal_cursor: false,
            countdown: 0,
            from_x: 0,
            till_x: 0,
            has_more: false,
            is_shorter: true,
            sequel_column: 0,
            sw_state: crate::winio::SoftwrapState::default(),
            recording: false,
            macro_buffer: Vec::new(),
            previous_macro: Vec::new(),
            milestone: 0,
            escapes: 0,
            first_escape_was_alone: false,
            last_escape_was_alone: false,
            byte_code: 0,
            give_a_hint: true,
            puddle: Vec::new(),
            prompt_puddle: Vec::new(),
            statusline_start_col: 0,
            prompt: Vec::new(),
            typing_x: HIGHEST_POSITIVE,
            came_full_circle: false,
            history_changed: false,
            registername: None,
            latest_timestamp: 942_927_132,
            positions_register: Vec::new(),
            filelist: Vec::new(),
            usable_rows: 0,
            piles: 0,
            gauge: 0,
            selected: 0,
            help_text: Vec::new(),
            start_of_body: 0,
            end_of_intro: 0,
            location: 0,
            list_of_completions: Vec::new(),
            pletion_x: 0,
            scouring: 0,
            rc_errors: Vec::new(),
            nanorc: None,
            rc_lineno: 0,
            kdf: KdfParams::default(),
            reading_plaintext: false,
            keytable: crate::curses::terminfo::KeyTable::default(),
            pending_input: Vec::new(),
            keypad_on: false,
            did_credits: false,
            terminfo_kbs: None,
            oldinterval: -1,
            colors: 8,
            term_has_colors: false,
            original_termios: None,
            stdin_flags: -1,
        };
        k.nodes.reserve(1024);
        k
    }

    #[inline]
    pub fn node(&self, l: Lp) -> &LineNode {
        &self.nodes[l]
    }

    #[inline]
    pub fn node_mut(&mut self, l: Lp) -> &mut LineNode {
        &mut self.nodes[l]
    }

    #[inline]
    pub fn data(&self, l: Lp) -> &[u8] {
        &self.nodes[l].data
    }

    #[inline]
    pub fn next(&self, l: Lp) -> Option<Lp> {
        self.nodes[l].next
    }

    #[inline]
    pub fn prev(&self, l: Lp) -> Option<Lp> {
        self.nodes[l].prev
    }

    #[inline]
    pub fn lineno(&self, l: Lp) -> i64 {
        self.nodes[l].lineno
    }

    #[inline]
    pub fn datalen(&self, l: Lp) -> usize {
        self.nodes[l].data.len()
    }

    #[inline]
    pub fn byte_at(&self, l: Lp, x: usize) -> u8 {
        *self.nodes[l].data.get(x).unwrap_or(&0)
    }

    #[inline]
    pub fn ofile(&self) -> &OpenFile {
        &self.files[self.cur]
    }

    #[inline]
    pub fn ofile_mut(&mut self) -> &mut OpenFile {
        &mut self.files[self.cur]
    }

    #[inline]
    pub fn current(&self) -> Lp {
        self.files[self.cur].current
    }

    #[inline]
    pub fn current_x(&self) -> usize {
        self.files[self.cur].current_x
    }

    #[inline]
    pub fn filetop(&self) -> Lp {
        self.files[self.cur].filetop
    }

    #[inline]
    pub fn filebot(&self) -> Lp {
        self.files[self.cur].filebot
    }

    #[inline]
    pub fn edittop(&self) -> Lp {
        self.files[self.cur].edittop
    }

    #[inline]
    pub fn cur_data(&self) -> &[u8] {
        &self.nodes[self.files[self.cur].current].data
    }

    #[inline]
    pub const fn isset(&self, f: Flag) -> bool {
        self.flags.isset(f)
    }

    #[inline]
    pub const fn lines(&self) -> usize {
        self.term.rows
    }

    #[inline]
    pub const fn cols(&self) -> usize {
        self.term.cols
    }

    pub const fn next_buffer_index(&self, i: usize) -> usize {
        (i + 1) % self.files.len()
    }

    pub const fn prev_buffer_index(&self, i: usize) -> usize {
        (i + self.files.len() - 1) % self.files.len()
    }
}

pub const EXIT_TAG: &str = "Beenden";
pub const CLOSE_TAG: &str = "Schließen";

#[allow(dead_code)]
const NOVIEW: bool = false;
const BLANKAFTER: bool = true;
const TOGETHER: bool = false;
const ALMOST_ALL: u32 = (MMOST | MBROWSER | MYESNO) & !MFINDINHELP;

pub fn keycode_from_string(keystring: &str, shifted_metas: &mut bool) -> i32 {
    let b = keystring.as_bytes();
    if b.is_empty() {
        return -1;
    }
    if b[0] == b'^' {
        if b.len() == 2 {
            if b[1] == b'/' || b[1] == b'-' {
                return 31;
            }
            if b[1] <= b'_' {
                return i32::from(b[1]) - 64;
            }
            if b[1] == b'`' {
                return 0;
            }
            return -1;
        } else if keystring.eq_ignore_ascii_case("^Space") {
            return 0;
        }
        return -1;
    } else if b[0] == b'M' {
        if b.len() == 3 && b[1] == b'-' {
            if b[2].is_ascii_uppercase() {
                return i32::from(b[2] | 0x20);
            }
            return i32::from(b[2]);
        }
        let lower = keystring.to_ascii_lowercase();
        return match lower.as_str() {
            "m-space" => ' ' as i32,
            "m-left" => ALT_LEFT,
            "m-right" => ALT_RIGHT,
            "m-up" => ALT_UP,
            "m-down" => ALT_DOWN,
            "m-ins" => ALT_INSERT,
            "m-del" => ALT_DELETE,
            _ => -1,
        };
    } else if b.len() == 6
        && keystring[..5].eq_ignore_ascii_case("Sh-M-")
        && (b[5] | 0x20).is_ascii_lowercase()
    {
        *shifted_metas = true;
        return i32::from(b[5] & 0x5F);
    } else if b[0] == b'F' {
        let fn_ = keystring[1..].parse::<i32>().unwrap_or(0);
        if !(1..=24).contains(&fn_) {
            return -1;
        }
        return KEY_F0 + fn_;
    } else if keystring.eq_ignore_ascii_case("Ins") {
        return KEY_IC;
    } else if keystring.eq_ignore_ascii_case("Del") {
        return KEY_DC;
    }
    -1
}

pub const fn epithet_of_flag(flag: Flag) -> &'static str {
    match flag {
        Flag::Zero => "Oberfläche verbergen",
        Flag::NoHelp => "Hilfe-Modus",
        Flag::ConstantShow => "Cursorposition ständig anzeigen",
        Flag::LineNumbers => "Zeilennummerierung",
        Flag::WhitespaceDisplay => "Leerzeichenanzeige",
        Flag::NoSyntax => "Farbige Syntaxhervorhebung",
        Flag::SoftWrap => "Überlange Zeilen fließend umbrechen",
        Flag::BreakLongLines => "Überlange Zeilen hart umbrechen",
        Flag::TabsToSpaces => "Umwandlungen eingegebener Tabulatoren in Leerzeichen",
        Flag::AutoIndent => "Automatische Einrückung",
        Flag::SmartHome => "Intelligente Pos1-Taste",
        Flag::CutFromCursor => "Bis Ende ausschneiden",
        Flag::UseMouse => "Mausunterstützung",
        _ => "Ehm...",
    }
}

impl Zu {
    fn add_to_funcs(
        &mut self,
        func: Func,
        menus: u32,
        tag: &'static str,
        phrase: &'static str,
        blank_after: bool,
    ) {
        self.allfuncs.push(FuncItem {
            func,
            tag,
            phrase,
            blank_after,
            menus,
        });
    }

    pub fn add_to_sclist(&mut self, menus: u32, scstring: &str, keycode: i32, func: Func) {
        let code = if keycode != 0 {
            keycode
        } else {
            keycode_from_string(scstring, &mut self.shifted_metas)
        };
        self.sclist.push(Shortcut {
            keystr: scstring.to_string(),
            keycode: code,
            menus,
            func,
            toggle: None,
            expansion: None,
        });
    }

    fn add_toggle(&mut self, scstring: &str, flag: Flag) {
        let menus = if flag == Flag::NoHelp {
            ALMOST_ALL
        } else {
            MMAIN
        };
        self.add_to_sclist(menus, scstring, 0, Func::Toggle);
        self.sclist.last_mut().unwrap().toggle = Some(flag);
    }

    pub fn first_sc_for(&self, menu: u32, func: Func) -> Option<&Shortcut> {
        self.sclist
            .iter()
            .find(|sc| (sc.menus & menu) != 0 && sc.func == func && !sc.keystr.is_empty())
    }

    pub fn shown_entries_for(&self, menu: u32) -> usize {
        let maximum = ((self.cols() + 40) / 20) * 2;
        let mut count = 0;
        let mut idx = 0;
        while count < maximum && idx < self.allfuncs.len() {
            if self.allfuncs[idx].menus & menu != 0 {
                count += 1;
            }
            idx += 1;
        }
        if menu == MWRITEFILE
            && idx >= self.allfuncs.len()
            && self.first_sc_for(menu, Func::DiscardBuffer).is_none()
        {
            count -= 1;
        }
        count
    }

    pub fn get_shortcut(&self, keycode: i32) -> Option<Shortcut> {
        if !self.meta_key && (0x20..=0xFF).contains(&keycode) {
            return None;
        }
        if self.meta_key && keycode < 0x20 {
            return None;
        }
        if keycode == PLANTED_A_COMMAND {
            return self.planted_shortcut.clone();
        }
        self.sclist
            .iter()
            .find(|sc| (sc.menus & self.currmenu) != 0 && keycode == sc.keycode)
            .cloned()
    }

    pub fn func_from_key(&self, keycode: i32) -> Option<Func> {
        self.get_shortcut(keycode).map(|s| s.func)
    }

    pub fn interpret(&self, keycode: i32) -> Option<Func> {
        if !self.meta_key && (0..0x7F).contains(&keycode) {
            if keycode == 'N' as i32 {
                return Some(Func::FindPrevious);
            }
            if keycode == 'n' as i32 {
                return Some(Func::FindNext);
            }
            match (u8_of(i64::from(keycode))).to_ascii_lowercase() {
                b'b' | b'-' => return Some(Func::PageUp),
                b' ' => return Some(Func::PageDown),
                b'w' | b'/' => return Some(Func::SearchForward),
                b'g' => return Some(Func::GotoDir),
                b'?' => return Some(Func::Help),
                b's' => return Some(Func::Enter),
                b'e' | b'q' | b'x' => return Some(Func::Exit),
                _ => {}
            }
        }
        self.func_from_key(keycode)
    }

    pub fn shortcut_init(&mut self) {
        let restricted = self.isset(Flag::Restricted);
        let view = self.isset(Flag::ViewMode);
        let modern = self.isset(Flag::ModernBindings);
        let preserve = self.isset(Flag::Preserve);

        self.allfuncs.clear();
        self.sclist.clear();

        let backspace = self.terminfo_kbs.as_ref().and_then(|k| k.first().copied());
        let help_key = if backspace.is_some() && backspace != Some(0x08) {
            "^H"
        } else {
            "^N"
        };

        self.add_to_funcs(
            Func::Help,
            (MMOST | MBROWSER) & !MFINDINHELP,
            "Hilfe",
            "Diese Hilfe anzeigen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Cancel,
            (MMOST & !MMAIN) | MYESNO,
            "Abbrechen",
            "Funktion abbrechen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Exit,
            MMAIN,
            EXIT_TAG,
            "Aktuellen Puffer schließen / Zu beenden",
            TOGETHER,
        );
        self.exitfunc = self.allfuncs.len() - 1;
        self.add_to_funcs(
            Func::Exit,
            MBROWSER,
            CLOSE_TAG,
            "Den Dateibrowser beenden",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Writeout,
            MMAIN,
            "Speichern",
            "Den aktuellen Puffer (oder den markierten Bereich) auf Festplatte speichern",
            TOGETHER,
        );
        if restricted {
            self.add_to_funcs(
                Func::Justify,
                MMAIN,
                "Ausrichten",
                "Absatz ausrichten",
                BLANKAFTER,
            );
        } else {
            self.add_to_funcs(
                Func::Insertfile,
                MMAIN,
                "Öffnen",
                "Weitere Datei in den aktuellen (oder einen neuen) Puffer einfügen",
                BLANKAFTER,
            );
        }
        self.add_to_funcs(Func::FullRefresh, MHELP, "Auffrischen", "x", false);
        self.add_to_funcs(Func::Exit, MHELP, CLOSE_TAG, "x", false);
        self.add_to_funcs(
            Func::SearchForward,
            MMAIN | MHELP,
            "Wo ist",
            "Nach einer Zeichenkette oder einem regulären Ausdruck vorwärts suchen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Replace,
            MMAIN,
            "Ersetzen",
            "Eine Zeichenkette oder einen regulären Ausdruck ersetzen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::CutText,
            MMAIN,
            "Ausschneiden",
            "Die aktuelle Zeile (oder den markierten Bereich) ausschneiden und in der Zwischenablage speichern",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::PasteText,
            MMAIN,
            "Einfügen",
            "Inhalt der Zwischenablage an der aktuellen Cursorposition einfügen",
            BLANKAFTER,
        );
        if !restricted {
            self.add_to_funcs(
                Func::Execute,
                MMAIN,
                "Ausführen",
                "Funktion oder externen Befehl ausführen",
                TOGETHER,
            );
            self.add_to_funcs(
                Func::Justify,
                MMAIN,
                "Ausrichten",
                "Absatz ausrichten",
                BLANKAFTER,
            );
        }
        self.add_to_funcs(
            Func::Location,
            MMAIN,
            "Position",
            "Die aktuelle Cursorposition anzeigen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::GotoLineColumn,
            MMAIN,
            "Zu Zeile",
            "Zu einer bestimmten Zeile und Spalte springen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Undo,
            MMAIN,
            "Rückgängig",
            "Letzte Operation rückgängig machen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Redo,
            MMAIN,
            "Wiederholen",
            "Rückgängig gemachte Operation wiederholen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Mark,
            MMAIN,
            "Markierung",
            "Text beginnend an Cursorposition markieren",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::CopyText,
            MMAIN,
            "Kopieren",
            "Die aktuelle Zeile (oder den markierten Bereich) kopieren und in der Zwischenablage speichern",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::CaseSens,
            MWHEREIS | MREPLACE,
            "Groß/Klein",
            "Unterscheidung von Groß- und Kleinschreibung bei Suche umschalten",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Regexp,
            MWHEREIS | MREPLACE,
            "Reg. Ausdruck",
            "Benutzung regulärer Ausdrücke umschalten",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Backwards,
            MWHEREIS | MREPLACE,
            "Rückwärts",
            "Die Richtung der Suche umkehren",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::FlipReplace,
            MWHEREIS,
            "Ersetzen",
            "Eine Zeichenkette oder einen regulären Ausdruck ersetzen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::FlipReplace,
            MREPLACE,
            "Keine Ersetzung",
            "Nach einer Zeichenkette oder einem regulären Ausdruck vorwärts suchen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::OlderItem,
            MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE,
            "Älter",
            "Den vorhergehenden Suchen-/Ersetzen-Begriff hervorholen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::NewerItem,
            MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE,
            "Neuer",
            "Den nächsten Suchen-/Ersetzen-Begriff hervorholen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::OlderItem,
            MEXECUTE,
            "Älter",
            "Den vorhergehenden Befehl erneut aufrufen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::NewerItem,
            MEXECUTE,
            "Neuer",
            "Den nächsten Befehl erneut aufrufen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::GotoDir,
            MBROWSER,
            "Gehe zu Verz.",
            "Zu Verzeichnis gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::FullRefresh,
            MBROWSER,
            "Auffrischen",
            "Dateiliste aktualisieren",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::SearchForward,
            MBROWSER,
            "Wo ist",
            "Vorwärts nach einer Zeichenkette suchen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::SearchBackward,
            MBROWSER,
            "Wo war",
            "Rückwärts nach einer Zeichenkette suchen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::FindPrevious,
            MBROWSER,
            "Vorige",
            "Nächstes Vorkommen rückwärts suchen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::FindNext,
            MBROWSER,
            "Nächste",
            "Nächstes Vorkommen vorwärts suchen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::FindBracket,
            MMAIN,
            "Zu Klammer",
            "Zur passenden Klammer springen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::SearchBackward,
            MMAIN | MHELP,
            "Wo war",
            "Nach einer Zeichenkette oder einem regulären Ausdruck rückwärts suchen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::FindPrevious,
            MMAIN | MHELP,
            "Vorige",
            "Nächstes Vorkommen rückwärts suchen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::FindNext,
            MMAIN | MHELP,
            "Nächste",
            "Nächstes Vorkommen vorwärts suchen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Left,
            MMAIN,
            "Zurück",
            "Ein Zeichen rückwärts gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Right,
            MMAIN,
            "Vorwärts",
            "Ein Zeichen vorwärts gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Left,
            MBROWSER,
            "Zurück",
            "Zur vorigen Datei der Liste springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Right,
            MBROWSER,
            "Vorwärts",
            "Zur nächsten Datei der Liste springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::PrevWord,
            MMAIN,
            "Vorheriges Wort",
            "Ein Wort rückwärts gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::NextWord,
            MMAIN,
            "Nächstes Wort",
            "Ein Wort vorwärts gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Home,
            MMAIN,
            "Pos 1",
            "Zum Anfang der aktuellen Zeile springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::End,
            MMAIN,
            "Ende",
            "Zum Ende der aktuellen Zeile springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::ScrollLeft,
            MMAIN,
            "Nach links",
            "Ansicht eine Tabulatorbreite nach links rollen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::ScrollRight,
            MMAIN,
            "Nach rechts",
            "Ansicht eine Tabulatorbreite nach rechts rollen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Up,
            MMAIN | MBROWSER | MHELP,
            "Zeile zurück",
            "Zur vorhergehenden Zeile gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Down,
            MMAIN | MBROWSER | MHELP,
            "Zeile vor",
            "Zur folgenden Zeile gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::ScrollUp,
            MMAIN,
            "Hochrollen",
            "Eine Zeile hochrollen, ohne den Cursor im Text zu bewegen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::ScrollDown,
            MMAIN,
            "Herunterrollen",
            "Eine Zeile hinunterrollen, ohne den Cursor im Text zu bewegen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::PrevBlock,
            MMAIN,
            "Vorherig. Block",
            "Zum vorhergehenden Textblock gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::NextBlock,
            MMAIN,
            "Nächster Block",
            "Zum folgenden Textblock gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::ParaBegin,
            MMAIN | MGOTOLINE,
            "Beginn des Absatzes",
            "Zum Anfang des aktuellen Absatzes springen; dann des vorigen Absatzes",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::ParaEnd,
            MMAIN | MGOTOLINE,
            "Ende des Absatzes",
            "Hinter das Ende des aktuellen Absatzes springen; dann des nächsten Absatzes",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::TopRow,
            MMAIN,
            "Oberste Zeile",
            "Zur ersten Zeile in dieser Ansicht springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::BottomRow,
            MMAIN,
            "Unterste Zeile",
            "Zur letzten Zeile in dieser Ansicht springen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::PageUp,
            MMAIN | MHELP,
            "Seite zurück",
            "Eine Bildschirmseite hoch gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::PageDown,
            MMAIN | MHELP,
            "Seite vor",
            "Eine Bildschirmseite runter gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::FirstLine,
            MMAIN | MHELP | MGOTOLINE,
            "Erste Zeile",
            "Zur ersten Zeile der Datei springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::LastLine,
            MMAIN | MHELP | MGOTOLINE,
            "Letzte Zeile",
            "Zur letzten Zeile der Datei springen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::PrevBuffer,
            MMAIN,
            "Vorh. Datei",
            "Zum vorhergehenden Dateipuffer umschalten",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::NextBuffer,
            MMAIN,
            "Nächste Datei",
            "Zum folgenden Dateipuffer umschalten",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Tab,
            MMAIN,
            "Tab",
            "Einen Tabulator an der Cursorposition einfügen (oder die markierten Zeilen einrücken)",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Enter,
            MMAIN,
            "Enter",
            "Einen Zeilenumbruch an der Cursorposition einfügen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Backspace,
            MMAIN,
            "Rücktaste",
            "Zeichen links vom Cursor löschen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Delete,
            MMAIN,
            "Löschen",
            "Zeichen an der Cursorposition löschen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::ChopPrevWord,
            MMAIN,
            "Wort links",
            "Vom Cursor rückwärts bis zum Beginn des Wortes ausschneiden",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::ChopNextWord,
            MMAIN,
            "Wort rechts",
            "Vom Cursor vorwärts bis zum Beginn des nächsten Wortes ausschneiden",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::CutTillEof,
            MMAIN,
            "Bis Ende ausschneiden",
            "Vom Cursor bis zum Dateiende ausschneiden",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::FullJustify,
            MMAIN,
            "Kompl. Ausr.",
            "Die gesamte Datei ausrichten",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::WordCount,
            MMAIN,
            "Wortanzahl",
            "Die Zeilen, Wörter und Zeichen zählen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Verbatim,
            MMAIN,
            "Direkt",
            "Nächstes Zeichen direkt (nicht interpretiert) einfügen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Indent,
            MMAIN,
            "Einrücken",
            "Die aktuelle Zeile oder die markierten Zeilen einrücken",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Unindent,
            MMAIN,
            "Ausrücken",
            "Die aktuelle Zeile oder die markierten Zeilen ausrücken",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Comment,
            MMAIN,
            "Kommentarzeilen",
            "Die aktuelle Zeile oder die markierten Zeilen auskommentieren oder Kommentierung aufheben",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Complete,
            MMAIN,
            "Vervollständigen",
            "Versuchen, das aktuelle Wort zu vervollständigen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::RecordMacro,
            MMAIN,
            "Aufzeichnen",
            "Aufzeichnung eines Makros starten/stoppen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::RunMacro,
            MMAIN,
            "Makro ausführen",
            "Zuletzt aufgezeichnetes Makro ausführen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::ZapText,
            MMAIN,
            "Löschen",
            "Die aktuelle Zeile oder den markierten Bereich verwerfen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Anchor,
            MMAIN,
            "Anker",
            "Einen Anker in der aktuellen Zeile setzen oder daraus entfernen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::PrevAnchor,
            MMAIN,
            "Nach oben zum Anker",
            "Zurück zum nächstliegenden Anker springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::NextAnchor,
            MMAIN,
            "Nach unten zum Anker",
            "Vorwärts zum nächstliegenden Anker springen",
            BLANKAFTER,
        );
        if restricted {
            self.add_to_funcs(
                Func::Execute,
                MMAIN,
                "Ausführen",
                "Funktion oder externen Befehl ausführen",
                TOGETHER,
            );
        }
        self.add_to_funcs(
            Func::Suspend,
            MMAIN,
            "In den Hintergrund",
            "Den Editor in den Hintergrund schieben (zur Shell zurückkehren)",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::FullRefresh,
            MMAIN,
            "Auffrischen",
            "Bildschirm auffrischen (neu zeichnen)",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Center,
            MMAIN,
            "Zentrieren",
            "Zeile zentrieren, die den Cursor enthält",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::Cycle,
            MMAIN,
            "Abwechseln",
            "Zeile, die den Cursor enthält, in die Mitte, dann nach oben, dann nach unten schieben",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Savefile,
            MMAIN,
            "Speichern",
            "Datei ohne Nachfrage speichern",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::ChangePassword,
            MMAIN,
            "Passwort",
            "Passwort ändern",
            BLANKAFTER,
        );
        if !restricted && !view {
            self.add_to_funcs(
                Func::FlipNewbuffer,
                MINSERTFILE | MEXECUTE,
                "Neuer Puffer",
                "Benutzung eines neuen Puffers umschalten",
                TOGETHER,
            );
        }
        self.add_to_funcs(
            Func::FullJustify,
            MEXECUTE,
            "Kompl. Ausr.",
            "Die gesamte Datei ausrichten",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::DosFormat,
            MWRITEFILE,
            "DOS-Format",
            "Schreiben im DOS-Format umschalten",
            TOGETHER,
        );
        if !restricted {
            self.add_to_funcs(
                Func::BackItUp,
                MWRITEFILE,
                "Sicherungsdatei",
                "Sicherung existierender Dateien beim Speichern umschalten",
                TOGETHER,
            );
            self.add_to_funcs(
                Func::AppendIt,
                MWRITEFILE,
                "Anhängen",
                "Anhängen umschalten",
                TOGETHER,
            );
            self.add_to_funcs(
                Func::PrependIt,
                MWRITEFILE,
                "Vorn anfügen",
                "Vorn-Anfügen umschalten",
                BLANKAFTER,
            );
        }
        self.add_to_funcs(
            Func::ChangePassword,
            MWRITEFILE,
            "Passwort",
            "Passwort ändern",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::FlipConvert,
            MINSERTFILE,
            "Keine Umwandlung",
            "Keine Umwandlung aus DOS-Format",
            BLANKAFTER,
        );
        if !restricted && !view {
            self.add_to_funcs(
                Func::FlipExecute,
                MINSERTFILE,
                "Befehl ausführen",
                "Funktion oder externen Befehl ausführen",
                BLANKAFTER,
            );
        }
        self.add_to_funcs(
            Func::CutTillEof,
            MEXECUTE,
            "Bis Ende ausschneiden",
            "Vom Cursor bis zum Dateiende ausschneiden",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::Suspend,
            MEXECUTE,
            "In den Hintergrund",
            "Den Editor in den Hintergrund schieben (zur Shell zurückkehren)",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::DiscardBuffer,
            MWRITEFILE,
            "Puffer verwerfen",
            "Puffer schließen, ohne zu speichern",
            BLANKAFTER,
        );
        if !restricted {
            self.add_to_funcs(
                Func::ToFiles,
                MWRITEFILE | MINSERTFILE,
                "Dateien …",
                "Zum Dateibrowser gehen",
                BLANKAFTER,
            );
        }
        self.add_to_funcs(
            Func::PageUp,
            MBROWSER,
            "Seite zurück",
            "Eine Bildschirmseite hoch gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::PageDown,
            MBROWSER,
            "Seite vor",
            "Eine Bildschirmseite runter gehen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::FirstFile,
            MBROWSER | MWHEREISFILE,
            "Erste Datei",
            "Zur ersten Datei der Liste springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::LastFile,
            MBROWSER | MWHEREISFILE,
            "Letzte Datei",
            "Zur letzten Datei der Liste springen",
            BLANKAFTER,
        );
        self.add_to_funcs(
            Func::PrevWord,
            MBROWSER,
            "Linke Spalte",
            "Zur linken Spalte springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::NextWord,
            MBROWSER,
            "Rechte Spalte",
            "Zur rechten Spalte springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::PrevBlock,
            MBROWSER,
            "Oberste Zeile",
            "Zur ersten Zeile in dieser Spalte springen",
            TOGETHER,
        );
        self.add_to_funcs(
            Func::NextBlock,
            MBROWSER,
            "Unterste Zeile",
            "Zur letzten Zeile in dieser Spalte springen",
            BLANKAFTER,
        );

        self.add_to_sclist(MMOST | MBROWSER, "^M", '\r' as i32, Func::Enter);
        self.add_to_sclist(MMOST | MBROWSER, "Enter", KEY_ENTER, Func::Enter);
        self.add_to_sclist(MMOST, "^I", '\t' as i32, Func::Tab);
        self.add_to_sclist(MMOST, "Tab", '\t' as i32, Func::Tab);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^B", 0, Func::SearchBackward);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^F", 0, Func::SearchForward);
        if modern {
            self.add_to_sclist((MMOST | MBROWSER) & !MFINDINHELP, help_key, 0, Func::Help);
            self.add_to_sclist(MHELP, help_key, 0, Func::Exit);
            self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^Q", 0, Func::Exit);
            self.add_to_sclist(MMAIN, "^S", 0, Func::Savefile);
            self.add_to_sclist(MMAIN, "^W", 0, Func::Writeout);
            self.add_to_sclist(MMAIN, "^O", 0, Func::Insertfile);
            self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^D", 0, Func::FindPrevious);
            self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^G", 0, Func::FindNext);
            self.add_to_sclist(MMAIN, "^R", 0, Func::Replace);
            self.add_to_sclist(MMAIN, "^T", 0, Func::GotoLineColumn);
            self.add_to_sclist(MMAIN, "^P", 0, Func::Location);
            self.add_to_sclist(MMAIN, "^Z", 0, Func::Undo);
            self.add_to_sclist(MMAIN, "^Y", 0, Func::Redo);
            self.add_to_sclist(MMAIN, "^A", 0, Func::Mark);
            self.add_to_sclist(MMAIN, "^X", 0, Func::CutText);
            self.add_to_sclist(MMAIN, "^C", 0, Func::CopyText);
            self.add_to_sclist(MMAIN, "^V", 0, Func::PasteText);
        } else {
            self.add_to_sclist((MMOST | MBROWSER) & !MFINDINHELP, "^G", 0, Func::Help);
            self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^X", 0, Func::Exit);
            if !preserve {
                self.add_to_sclist(MMAIN, "^S", 0, Func::Savefile);
            }
            self.add_to_sclist(MMAIN, "^O", 0, Func::Writeout);
            self.add_to_sclist(MMAIN, "^R", 0, Func::Insertfile);
            if !preserve {
                self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^Q", 0, Func::SearchBackward);
            }
            self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^W", 0, Func::SearchForward);
            self.add_to_sclist(MMOST, "^A", 0, Func::Home);
            self.add_to_sclist(MMOST, "^E", 0, Func::End);
            self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^P", 0, Func::Up);
            self.add_to_sclist(MMAIN | MBROWSER | MHELP, "^N", 0, Func::Down);
            self.add_to_sclist(MMAIN | MBROWSER | MHELP | MLINTER, "^Y", 0, Func::PageUp);
            self.add_to_sclist(MMAIN | MBROWSER | MHELP | MLINTER, "^V", 0, Func::PageDown);
            self.add_to_sclist(MMAIN, "^C", 0, Func::Location);
            self.add_to_sclist(MMOST, "^H", 8, Func::Backspace);
            self.add_to_sclist(MMOST, "^D", 0, Func::Delete);
        }
        self.add_to_sclist(MMOST, "Bsp", KEY_BACKSPACE, Func::Backspace);
        self.add_to_sclist(MMOST, "Sh-Del", SHIFT_DELETE, Func::Backspace);
        self.add_to_sclist(MMOST, "Del", KEY_DC, Func::Delete);
        self.add_to_sclist(MMAIN, "Ins", KEY_IC, Func::Insertfile);
        self.add_to_sclist(MMAIN, "^\\", 0, Func::Replace);
        self.add_to_sclist(MMAIN, "M-R", 0, Func::Replace);
        self.add_to_sclist(MMOST, "^K", 0, Func::CutText);
        self.add_to_sclist(MMOST, "M-6", 0, Func::CopyText);
        self.add_to_sclist(MMOST, "M-^", 0, Func::CopyText);
        self.add_to_sclist(MMOST, "^U", 0, Func::PasteText);
        self.add_to_sclist(MMAIN, if modern { "^E" } else { "^T" }, 0, Func::Execute);
        self.add_to_sclist(MMAIN, "^J", '\n' as i32, Func::Justify);
        let slash_or_dash = if cfg!(target_os = "linux") && self.on_a_vt {
            "^-"
        } else {
            "^/"
        };
        self.add_to_sclist(MMAIN, slash_or_dash, 0, Func::GotoLineColumn);
        self.add_to_sclist(MMAIN, "M-G", 0, Func::GotoLineColumn);
        self.add_to_sclist(MMAIN, "^_", 0, Func::GotoLineColumn);
        self.add_to_sclist(
            MMAIN | MBROWSER | MHELP | MLINTER,
            "PgUp",
            KEY_PPAGE,
            Func::PageUp,
        );
        self.add_to_sclist(
            MMAIN | MBROWSER | MHELP | MLINTER,
            "PgDn",
            KEY_NPAGE,
            Func::PageDown,
        );
        self.add_to_sclist(MBROWSER | MHELP, "Bsp", KEY_BACKSPACE, Func::PageUp);
        self.add_to_sclist(MBROWSER | MHELP, "Sh-Del", SHIFT_DELETE, Func::PageUp);
        self.add_to_sclist(MBROWSER | MHELP, "Space", 0x20, Func::PageDown);
        self.add_to_sclist(MMAIN | MHELP, "M-\\", 0, Func::FirstLine);
        self.add_to_sclist(MMAIN | MHELP, "^Home", CONTROL_HOME, Func::FirstLine);
        self.add_to_sclist(MMAIN | MHELP, "M-/", 0, Func::LastLine);
        self.add_to_sclist(MMAIN | MHELP, "^End", CONTROL_END, Func::LastLine);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "M-B", 0, Func::FindPrevious);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "M-F", 0, Func::FindNext);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "M-W", 0, Func::FindNext);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "M-Q", 0, Func::FindPrevious);
        self.add_to_sclist(MMAIN, "M-]", 0, Func::FindBracket);
        self.add_to_sclist(MMAIN, "M-A", 0, Func::Mark);
        self.add_to_sclist(MMAIN, "^6", 0, Func::Mark);
        self.add_to_sclist(MMAIN, "^^", 0, Func::Mark);
        self.add_to_sclist(MMAIN, "M-}", 0, Func::Indent);
        self.add_to_sclist(MMAIN, "M-{", 0, Func::Unindent);
        self.add_to_sclist(MMAIN, "Sh-Tab", SHIFT_TAB, Func::Unindent);
        self.add_to_sclist(MMAIN, "M-:", 0, Func::RecordMacro);
        self.add_to_sclist(MMAIN, "M-;", 0, Func::RunMacro);
        self.add_to_sclist(MMAIN, "M-U", 0, Func::Undo);
        self.add_to_sclist(MMAIN, "M-E", 0, Func::Redo);
        self.add_to_sclist(MMAIN, "M-Bsp", CONTROL_SHIFT_DELETE, Func::ChopPrevWord);
        self.add_to_sclist(MMAIN, "Sh-^Del", CONTROL_SHIFT_DELETE, Func::ChopPrevWord);
        self.add_to_sclist(MMAIN, "^Del", CONTROL_DELETE, Func::ChopNextWord);
        self.add_to_sclist(MMAIN, "M-Del", ALT_DELETE, Func::ZapText);
        self.add_to_sclist(MMAIN, "M-Ins", ALT_INSERT, Func::Anchor);
        self.add_to_sclist(MMAIN, "M-Home", ALT_HOME, Func::TopRow);
        self.add_to_sclist(MMAIN, "M-End", ALT_END, Func::BottomRow);
        self.add_to_sclist(MMAIN, "M-PgUp", ALT_PAGEUP, Func::PrevAnchor);
        self.add_to_sclist(MMAIN, "M-PgDn", ALT_PAGEDOWN, Func::NextAnchor);
        self.add_to_sclist(MMAIN, "M-\"", 0, Func::Anchor);
        self.add_to_sclist(MMAIN, "M-'", 0, Func::NextAnchor);
        self.add_to_sclist(MMAIN, "^]", 0, Func::Complete);
        self.add_to_sclist(MMAIN, "M-3", 0, Func::Comment);
        self.add_to_sclist(MMOST & !MMAIN, "^B", 0, Func::Left);
        self.add_to_sclist(MMOST & !MMAIN, "^F", 0, Func::Right);
        self.add_to_sclist(MMOST | MBROWSER | MHELP, "\u{25C2}", KEY_LEFT, Func::Left);
        self.add_to_sclist(MMOST | MBROWSER | MHELP, "\u{25B8}", KEY_RIGHT, Func::Right);
        self.add_to_sclist(MSOME, "^\u{25C2}", CONTROL_LEFT, Func::PrevWord);
        self.add_to_sclist(MSOME, "^\u{25B8}", CONTROL_RIGHT, Func::NextWord);
        self.add_to_sclist(MMAIN, "M-\u{25C2}", ALT_LEFT, Func::PrevBuffer);
        self.add_to_sclist(MMAIN, "M-\u{25B8}", ALT_RIGHT, Func::NextBuffer);
        self.add_to_sclist(MMOST, "M-Space", 0, Func::PrevWord);
        self.add_to_sclist(MMOST, "^Space", 0, Func::NextWord);
        self.add_to_sclist(MMOST, "Home", KEY_HOME, Func::Home);
        self.add_to_sclist(MMOST, "End", KEY_END, Func::End);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "\u{25B4}", KEY_UP, Func::Up);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "\u{25BE}", KEY_DOWN, Func::Down);
        self.add_to_sclist(
            MMAIN | MBROWSER | MLINTER,
            "^\u{25B4}",
            CONTROL_UP,
            Func::PrevBlock,
        );
        self.add_to_sclist(
            MMAIN | MBROWSER | MLINTER,
            "^\u{25BE}",
            CONTROL_DOWN,
            Func::NextBlock,
        );
        self.add_to_sclist(MMAIN, "M-7", 0, Func::PrevBlock);
        self.add_to_sclist(MMAIN, "M-8", 0, Func::NextBlock);
        self.add_to_sclist(MMAIN, "M-(", 0, Func::ParaBegin);
        self.add_to_sclist(MMAIN, "M-9", 0, Func::ParaBegin);
        self.add_to_sclist(MMAIN, "M-)", 0, Func::ParaEnd);
        self.add_to_sclist(MMAIN, "M-0", 0, Func::ParaEnd);
        self.add_to_sclist(MMAIN | MHELP, "M-\u{25B4}", ALT_UP, Func::ScrollUp);
        self.add_to_sclist(MMAIN | MHELP, "M-\u{25BE}", ALT_DOWN, Func::ScrollDown);
        self.add_to_sclist(MMAIN | MHELP, "M--", 0, Func::ScrollUp);
        self.add_to_sclist(MMAIN | MHELP, "M-_", 0, Func::ScrollUp);
        self.add_to_sclist(MMAIN | MHELP, "M-+", 0, Func::ScrollDown);
        self.add_to_sclist(MMAIN | MHELP, "M-=", 0, Func::ScrollDown);
        self.add_to_sclist(MMAIN, "M-,", 0, Func::PrevBuffer);
        self.add_to_sclist(MMAIN, "M-.", 0, Func::NextBuffer);
        self.add_to_sclist(MMOST, "M-V", 0, Func::Verbatim);
        self.add_to_sclist(MMAIN, "M-T", 0, Func::CutTillEof);
        self.add_to_sclist(MEXECUTE, "^V", 0, Func::CutTillEof);
        self.add_to_sclist(MEXECUTE, "^Z", 0, Func::Suspend);
        self.add_to_sclist(MMAIN, "^Z", 0, Func::SuggestCtrlTZ);
        self.add_to_sclist(MMAIN, "M-D", 0, Func::WordCount);
        self.add_to_sclist(MMAIN, "M-J", 0, Func::FullJustify);
        self.add_to_sclist(MEXECUTE, "^J", 0, Func::FullJustify);
        self.add_to_sclist(MMAIN, "M-<", 0, Func::ScrollLeft);
        self.add_to_sclist(MMAIN, "M->", 0, Func::ScrollRight);
        self.add_to_sclist(MMAIN, "^L", 0, Func::Center);
        self.add_to_sclist(MMAIN, "M-%", 0, Func::Cycle);
        self.add_to_sclist(
            (MMOST | MBROWSER | MHELP | MYESNO) & !MMAIN,
            "^L",
            0,
            Func::FullRefresh,
        );

        self.add_toggle("M-Z", Flag::Zero);
        self.add_toggle("M-X", Flag::NoHelp);
        self.add_toggle("M-C", Flag::ConstantShow);
        self.add_toggle("M-N", Flag::LineNumbers);
        self.add_toggle("M-#", Flag::LineNumbers);
        self.add_toggle("M-P", Flag::WhitespaceDisplay);
        self.add_toggle("M-S", Flag::SoftWrap);
        self.add_toggle("M-$", Flag::SoftWrap);
        self.add_toggle("M-L", Flag::BreakLongLines);
        self.add_toggle("M-O", Flag::TabsToSpaces);
        self.add_toggle("M-I", Flag::AutoIndent);
        self.add_toggle("M-H", Flag::SmartHome);
        self.add_toggle("M-K", Flag::CutFromCursor);
        self.add_toggle("M-M", Flag::UseMouse);

        self.add_to_sclist((MMOST & !MMAIN) | MYESNO, "^C", 0, Func::Cancel);
        self.add_to_sclist(MWHEREIS | MREPLACE, "M-C", 0, Func::CaseSens);
        self.add_to_sclist(MWHEREIS | MREPLACE, "M-R", 0, Func::Regexp);
        self.add_to_sclist(MWHEREIS | MREPLACE, "M-B", 0, Func::Backwards);
        self.add_to_sclist(MWHEREIS | MREPLACE, "^R", 0, Func::FlipReplace);
        self.add_to_sclist(MWHEREIS | MGOTOLINE, "^T", 0, Func::FlipGoto);
        let histmenus = MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE | MFINDINHELP | MEXECUTE;
        self.add_to_sclist(histmenus, "^P", 0, Func::OlderItem);
        self.add_to_sclist(histmenus, "^N", 0, Func::NewerItem);
        self.add_to_sclist(histmenus, "\u{25B4}", KEY_UP, Func::OlderItem);
        self.add_to_sclist(histmenus, "\u{25BE}", KEY_DOWN, Func::NewerItem);
        self.add_to_sclist(MGOTOLINE, "^W", 0, Func::ParaBegin);
        self.add_to_sclist(MGOTOLINE, "^O", 0, Func::ParaEnd);
        self.add_to_sclist(MGOTOLINE | MWHEREIS | MFINDINHELP, "^Y", 0, Func::FirstLine);
        self.add_to_sclist(MGOTOLINE | MWHEREIS | MFINDINHELP, "^V", 0, Func::LastLine);
        self.add_to_sclist(MWHEREISFILE, "^Y", 0, Func::FirstFile);
        self.add_to_sclist(MWHEREISFILE, "^V", 0, Func::LastFile);
        self.add_to_sclist(MBROWSER | MWHEREISFILE, "M-\\", 0, Func::FirstFile);
        self.add_to_sclist(MBROWSER | MWHEREISFILE, "M-/", 0, Func::LastFile);
        self.add_to_sclist(MBROWSER, "Home", KEY_HOME, Func::FirstFile);
        self.add_to_sclist(MBROWSER, "End", KEY_END, Func::LastFile);
        self.add_to_sclist(MBROWSER, "^Home", CONTROL_HOME, Func::FirstFile);
        self.add_to_sclist(MBROWSER, "^End", CONTROL_END, Func::LastFile);
        self.add_to_sclist(
            MBROWSER,
            if cfg!(target_os = "linux") && self.on_a_vt {
                "^-"
            } else {
                "^/"
            },
            0,
            Func::GotoDir,
        );
        self.add_to_sclist(MBROWSER, "M-G", 0, Func::GotoDir);
        self.add_to_sclist(MBROWSER, "^_", 0, Func::GotoDir);
        if !preserve {
            self.add_to_sclist(MWRITEFILE, "^Q", 0, Func::DiscardBuffer);
        }
        self.add_to_sclist(MWRITEFILE, "M-D", 0, Func::DosFormat);
        if !restricted && !view {
            self.add_to_sclist(MWRITEFILE, "M-B", 0, Func::BackItUp);
            self.add_to_sclist(MWRITEFILE, "M-A", 0, Func::AppendIt);
            self.add_to_sclist(MWRITEFILE, "M-P", 0, Func::PrependIt);
            self.add_to_sclist(MINSERTFILE | MEXECUTE, "^X", 0, Func::FlipExecute);
        }
        self.add_to_sclist(MWRITEFILE, "M-K", 0, Func::ChangePassword);
        self.add_to_sclist(MINSERTFILE, "M-N", 0, Func::FlipConvert);
        if !restricted && !view {
            self.add_to_sclist(MINSERTFILE | MEXECUTE, "M-F", 0, Func::FlipNewbuffer);
        }
        self.add_to_sclist(MBROWSER | MHELP, "^C", 0, Func::Exit);
        if !restricted {
            self.add_to_sclist(MWRITEFILE | MINSERTFILE, "^T", 0, Func::ToFiles);
        }
        self.add_to_sclist(MBROWSER, "^T", 0, Func::Exit);
        self.add_to_sclist(MHELP, "^G", 0, Func::Exit);
        self.add_to_sclist(MHELP, "F1", KEY_F(1), Func::Exit);
        self.add_to_sclist(MHELP, "Home", KEY_HOME, Func::FirstLine);
        self.add_to_sclist(MHELP, "End", KEY_END, Func::LastLine);
        self.add_to_sclist(MMOST & !MFINDINHELP, "F1", KEY_F(1), Func::Help);
        self.add_to_sclist(MMAIN | MBROWSER | MHELP, "F2", KEY_F(2), Func::Exit);
        self.add_to_sclist(MMAIN, "F3", KEY_F(3), Func::Writeout);
        self.add_to_sclist(MMAIN, "F4", KEY_F(4), Func::Justify);
        self.add_to_sclist(MMAIN, "F5", KEY_F(5), Func::Insertfile);
        self.add_to_sclist(
            MMAIN | MBROWSER | MHELP,
            "F6",
            KEY_F(6),
            Func::SearchForward,
        );
        self.add_to_sclist(
            MMAIN | MBROWSER | MHELP | MLINTER,
            "F7",
            KEY_F(7),
            Func::PageUp,
        );
        self.add_to_sclist(
            MMAIN | MBROWSER | MHELP | MLINTER,
            "F8",
            KEY_F(8),
            Func::PageDown,
        );
        self.add_to_sclist(MMOST, "F9", KEY_F(9), Func::CutText);
        self.add_to_sclist(MMOST, "F10", KEY_F(10), Func::PasteText);
        self.add_to_sclist(MMAIN, "F11", KEY_F(11), Func::Location);
        self.add_to_sclist(MMAIN, "M-&", 0, Func::CursesVersion);
        self.add_to_sclist((MMOST & !MMAIN) | MYESNO, "", KEY_CANCEL, Func::Cancel);
        self.add_to_sclist(MMAIN, "", KEY_CENTER, Func::Center);
        self.add_to_sclist(MMAIN, "", KEY_SIC, Func::Insertfile);
        self.add_to_sclist(MMAIN, "", START_OF_PASTE, Func::SuckUpPaste);
        self.add_to_sclist(MMOST, "", START_OF_PASTE, Func::Nothing);
        self.add_to_sclist(MMOST, "", END_OF_PASTE, Func::Nothing);
        self.add_to_sclist(
            MMOST | MBROWSER | MHELP | MYESNO,
            "",
            KEY_FRESH,
            Func::FullRefresh,
        );
    }

    pub fn set_exit_tag(&mut self, tag: &'static str) {
        self.allfuncs[self.exitfunc].tag = tag;
    }
}
