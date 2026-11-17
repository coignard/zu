#![expect(
    clippy::too_many_lines,
    reason = "functions mirror the structure of their GNU nano counterparts"
)]

#[macro_use]
mod utils;
mod browser;
mod chars;
mod color;
mod crypto;
mod curses;
mod cut;
mod definitions;
mod files;
mod global;
mod help;
mod history;
mod r#move;
mod prompt;
mod rcfile;
mod regex;
mod search;
mod text;
mod winio;

use crate::files::get_next_filename;
use crate::utils::{i32_of, i64_of, u8_of, usize_of};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use zeroize::{Zeroize, Zeroizing};

use crate::chars::mbstrlen;
use crate::curses::{Attr, Win};
use crate::definitions::{
    BLIND, CUSHION, DEL_CODE, END_OF_PASTE, ESC_CODE, Elem, FOREIGN_SEQUENCE, FORWARD, Flag, Func,
    JUSTFIND, KEY_F0, KEY_MOUSE, LineNode, Lp, MEXECUTE, MISSING_BRACE, MMAIN, MWRITEFILE, Msg, NO,
    NO_SUCH_FUNCTION, NONOTES, THE_WINDOW_RESIZED, UndoType, Update, VERSION, VISIBLE,
    WIDTH_OF_TAB, Writing, YES, YESORNO,
};
use crate::global::Zu;
use crate::utils::{digits, io_errstr, parse_line_column, parse_num, tail};

#[global_allocator]
static GLOBAL: crypto::ZeroizingAllocator = crypto::ZeroizingAllocator;

pub const NANO_VERSION: &str = "9.3";

static GOT_HUPTERM: AtomicBool = AtomicBool::new(false);
static GOT_TSTP: AtomicBool = AtomicBool::new(false);
static GOT_WINCH: AtomicBool = AtomicBool::new(false);
static PANIC_PLACE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static GOT_CONT: AtomicBool = AtomicBool::new(false);
pub(crate) static CONTROL_C_WAS_PRESSED: AtomicBool = AtomicBool::new(false);
static STABS: AtomicU32 = AtomicU32::new(0);

static CRASH_TERMIOS: std::sync::OnceLock<libc::termios> = std::sync::OnceLock::new();
static CRASH_KEYPAD_LOCAL: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

fn erase_row_sequence(buf: &mut [u8; 24], row: usize) -> usize {
    let mut digits = [0u8; 8];
    let mut n = 0;
    let mut r = row.max(1);
    while r > 0 && n < digits.len() {
        digits[n] = b'0' + u8_of(i64_of(r % 10));
        r /= 10;
        n += 1;
    }
    let mut len = 0;
    for &b in b"\x1B[" {
        buf[len] = b;
        len += 1;
    }
    for i in (0..n).rev() {
        buf[len] = digits[i];
        len += 1;
    }
    for &b in b";1H\x1B[2K" {
        buf[len] = b;
        len += 1;
    }
    len
}

extern "C" fn handle_crash(signal: libc::c_int) {
    unsafe {
        let w = |b: &[u8]| {
            libc::write(1, b.as_ptr().cast::<libc::c_void>(), b.len());
        };
        if crate::curses::IN_CURSES.load(Ordering::SeqCst) {
            w(b"\x1B[?1006l\x1B[?1000l");
            if let Some(k) = CRASH_KEYPAD_LOCAL.get() {
                w(k);
            }
            if crate::curses::FOCUS_REPORTS.load(Ordering::SeqCst) {
                w(b"\x1B[?1004l");
            }
            w(b"\x1B[?2004l\x1B[0m");
            let mut ws: libc::winsize = std::mem::zeroed();
            let rows = if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_row > 0 {
                ws.ws_row as usize
            } else {
                24
            };
            let mut buf = [0u8; 24];
            for row in 1..=rows {
                let n = erase_row_sequence(&mut buf, row);
                w(&buf[..n]);
            }
            w(b"\x1B[H\x1B[?1049l\x1B[?25h");
        }
        if let Some(t) = CRASH_TERMIOS.get() {
            libc::tcsetattr(0, libc::TCSANOW, t);
        }
        let mut msg = [0u8; 200];
        let mut len = 0;
        let mut put = |b: &[u8]| {
            for &c in b {
                if len < msg.len() {
                    msg[len] = c;
                    len += 1;
                }
            }
        };
        put("Hoppla! Zu ist abgestürzt  Code: ".as_bytes());
        let mut digits = [0u8; 4];
        let mut n = 0;
        let mut v = signal.max(0).cast_unsigned();
        loop {
            digits[n] = b'0' + (v % 10) as u8;
            n += 1;
            v /= 10;
            if v == 0 || n == digits.len() {
                break;
            }
        }
        for i in (0..n).rev() {
            put(&digits[i..=i]);
        }
        put(".  Bitte melden Sie dies als Fehler.\n".as_bytes());
        put("Änderungen nicht gesichert.\n".as_bytes());
        libc::write(2, msg.as_ptr().cast::<libc::c_void>(), len);
        libc::signal(signal, libc::SIG_DFL);
        libc::raise(signal);
    }
}

static QUIET_FD: AtomicI32 = AtomicI32::new(-1);

extern "C" fn handle_hupterm(signal: libc::c_int) {
    GOT_HUPTERM.store(true, Ordering::SeqCst);
    #[cfg(target_os = "linux")]
    unsafe {
        if signal == libc::SIGHUP {
            let saved_errno = *libc::__errno_location();
            let mut pfd = libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            };
            if libc::poll(&raw mut pfd, 1, 0) > 0
                && pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
            {
                let quiet = QUIET_FD.load(Ordering::SeqCst);
                if quiet >= 0 {
                    libc::dup2(quiet, 0);
                }
            }
            *libc::__errno_location() = saved_errno;
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = signal;
}

extern "C" fn handle_sigwinch(_signal: libc::c_int) {
    GOT_WINCH.store(true, Ordering::SeqCst);
}

extern "C" fn handle_tstp(_signal: libc::c_int) {
    GOT_TSTP.store(true, Ordering::SeqCst);
}

extern "C" fn handle_cont(_signal: libc::c_int) {
    GOT_CONT.store(true, Ordering::SeqCst);
}

extern "C" fn make_a_note(_signal: libc::c_int) {
    CONTROL_C_WAS_PRESSED.store(true, Ordering::SeqCst);
}

fn set_handler(signal: libc::c_int, handler: libc::sighandler_t, fill_mask: bool) {
    unsafe {
        let mut deed: libc::sigaction = std::mem::zeroed();
        deed.sa_sigaction = handler;
        if fill_mask {
            libc::sigfillset(&raw mut deed.sa_mask);
        } else {
            libc::sigemptyset(&raw mut deed.sa_mask);
        }
        deed.sa_flags = 0;
        libc::sigaction(signal, &raw const deed, std::ptr::null_mut());
    }
}

fn set_up_signal_handlers() {
    unsafe {
        let mut ends = [-1i32; 2];
        if QUIET_FD.load(Ordering::SeqCst) < 0 && libc::pipe(ends.as_mut_ptr()) == 0 {
            for fd in ends {
                libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
                libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK);
            }
            QUIET_FD.store(ends[0], Ordering::SeqCst);
        }
    }
    set_handler(libc::SIGINT, libc::SIG_IGN, false);
    set_handler(libc::SIGQUIT, libc::SIG_IGN, false);
    set_handler(
        libc::SIGHUP,
        handle_hupterm as *const () as libc::sighandler_t,
        false,
    );
    set_handler(
        libc::SIGTERM,
        handle_hupterm as *const () as libc::sighandler_t,
        false,
    );
    set_handler(
        libc::SIGTSTP,
        handle_tstp as *const () as libc::sighandler_t,
        true,
    );
    set_handler(
        libc::SIGCONT,
        handle_cont as *const () as libc::sighandler_t,
        true,
    );
    if std::env::var_os("ZU_NOCATCH").is_none() {
        unsafe {
            let mut deed: libc::sigaction = std::mem::zeroed();
            deed.sa_sigaction = handle_crash as *const () as libc::sighandler_t;
            libc::sigemptyset(&raw mut deed.sa_mask);
            deed.sa_flags = libc::SA_RESETHAND | libc::SA_ONSTACK;
            libc::sigaction(libc::SIGSEGV, &raw const deed, std::ptr::null_mut());
            libc::sigaction(libc::SIGABRT, &raw const deed, std::ptr::null_mut());
        }
    }
}

fn isatty(fd: i32) -> bool {
    unsafe { libc::isatty(fd) == 1 }
}

fn change_lflag(set: libc::tcflag_t, clear: libc::tcflag_t) {
    unsafe {
        let mut settings: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &raw mut settings) == 0 {
            settings.c_lflag |= set;
            settings.c_lflag &= !clear;
            libc::tcsetattr(0, libc::TCSANOW, &raw const settings);
        }
    }
}

fn disable_kb_interrupt() {
    change_lflag(0, libc::ISIG);
}

fn enable_kb_interrupt() {
    change_lflag(libc::ISIG, 0);
}

pub(crate) fn install_handler_for_ctrl_c() {
    enable_kb_interrupt();
    set_handler(
        libc::SIGINT,
        make_a_note as *const () as libc::sighandler_t,
        false,
    );
}

pub(crate) fn restore_handler_for_ctrl_c() {
    set_handler(libc::SIGINT, libc::SIG_IGN, false);
    disable_kb_interrupt();
}

macro_rules! say {
    ($($arg:tt)*) => {{
        let _ = write!(std::io::stdout(), $($arg)*);
    }};
}

fn print_opt(k: &Zu, shortflag: &str, longflag: &str, description: &str) {
    let firstwidth = k.breadth(shortflag.as_bytes());
    let secondwidth = k.breadth(longflag.as_bytes());
    let mut line = format!(" {shortflag}");
    if firstwidth < 14 {
        line.push_str(&" ".repeat(14 - firstwidth));
    }
    line.push(' ');
    line.push_str(longflag);
    if secondwidth < 24 {
        line.push_str(&" ".repeat(24 - secondwidth));
    }
    say!("{}{}\n", line, description);
}

fn usage(k: &Zu) {
    let restricted = k.isset(Flag::Restricted);
    say!("Aufruf: zu [OPTIONEN] [[+ZEILE[,SPALTE]] DATEI]...\n\n");
    say!(
        "Um den Cursor in einer bestimmten Zeile der Datei zu platzieren, geben Sie\ndie Zeilennummer mit einem „+“ vor dem Dateinamen an. Die Spaltennummer kann\nnach einem Komma hinzugefügt werden.\n",
    );
    say!("Wenn ein Dateiname „-“ ist, liest Zu aus der Standardeingabe.\n");
    say!("Dateien werden verschlüsselt gespeichert.\n\n",);
    print_opt(k, "Option", "Lange Option", "Bedeutung");
    print_opt(k, "-A", "--smarthome", "Intelligente Pos1-Taste aktivieren");
    if !restricted {
        print_opt(
            k,
            "-B",
            "--backup",
            "Existierende Dateien beim Speichern sichern",
        );
        print_opt(
            k,
            "-C <Verz.>",
            "--backupdir=<Verz.>",
            "Verzeichnis zum Speichern von Sicherungsdateien",
        );
    }
    print_opt(
        k,
        "-D",
        "--boldtext",
        "Fettdruck anstelle von inversem Text benutzen",
    );
    print_opt(
        k,
        "-E",
        "--tabstospaces",
        "Eingegebene Tabulatoren in Leerzeichen umwandeln",
    );
    if !restricted {
        print_opt(
            k,
            "-F",
            "--newbuffer",
            "Eine Datei standardmäßig in einen neuen Puffer einlesen",
        );
    }
    print_opt(
        k,
        "-G",
        "--locking",
        "Sperrdateien benutzen (nach Art von Vim)",
    );
    print_opt(k, "-I", "--ignorercfiles", "Die zurc-Dateien ignorieren");
    print_opt(
        k,
        "-J <Nummer>",
        "--guidestripe=<Nummer>",
        "Einen Markierungsstreifen in dieser Spalte einblenden",
    );
    print_opt(
        k,
        "-K",
        "--rawsequences",
        "Verwirrung um Zehnertastatur lösen",
    );
    print_opt(
        k,
        "-L",
        "--nonewlines",
        "Keine neue Zeile automatisch einfügen",
    );
    print_opt(
        k,
        "-M",
        "--trimblanks",
        "Bei hartem Umbruch angehängte Leerzeichen entfernen",
    );
    print_opt(
        k,
        "-N",
        "--noconvert",
        "Dateien nicht aus dem DOS-Format umwandeln",
    );
    print_opt(
        k,
        "-O",
        "--bookstyle",
        "Vorangestellter Leerraum beginnt einen neuen Absatz",
    );
    if !restricted {
        print_opt(
            k,
            "-P",
            "--positionlog",
            "Die Cursorposition speichern und erneut laden",
        );
    }
    print_opt(
        k,
        "-Q <Ausdruck>",
        "--quotestr=<Ausdruck>",
        "Als Zitat zu betrachtender regulärer Ausdruck",
    );
    if !restricted {
        print_opt(k, "-R", "--restricted", "Dateisystemzugriff beschränken");
    }
    print_opt(
        k,
        "-S",
        "--softwrap",
        "Überlange Zeilen in mehreren Zeilen anzeigen",
    );
    print_opt(
        k,
        "-T <Zahl>",
        "--tabsize=<Zahl>",
        "Tabulator in dieser Breite in Spalten erstellen",
    );
    print_opt(
        k,
        "-U",
        "--quickblank",
        "Statuszeile beim nächsten Tastendruck leeren",
    );
    print_opt(k, "-V", "--version", "Versionsinfo ausgeben und beenden");
    print_opt(k, "-W", "--wordbounds", "Wortgrenzen genauer erkennen");
    print_opt(
        k,
        "-X <Z.kette>",
        "--wordchars=<Z.kette>",
        "Welche anderen Zeichen Wortteile sind",
    );
    print_opt(
        k,
        "-Z",
        "--zap",
        "Mit Rücktaste und Entf einen markierten Bereich löschen",
    );
    print_opt(
        k,
        "-a",
        "--atblanks",
        "Fließenden Zeilenumbruch am Leerzeichen vornehmen",
    );
    print_opt(
        k,
        "-b",
        "--breaklonglines",
        "Lange Zeilen automatisch hart umbrechen",
    );
    print_opt(k, "-c", "--constantshow", "Cursorposition ständig anzeigen");
    print_opt(
        k,
        "-d",
        "--rebinddelete",
        "Vertauschung von Rücktaste/Entf beheben",
    );
    print_opt(
        k,
        "-e",
        "--emptyline",
        "Zeile unter der Titelleiste leer lassen",
    );
    print_opt(
        k,
        "-f <Datei>",
        "--rcfile=<Datei>",
        "Nur diese Datei für die Konfiguration von Zu verwenden",
    );
    print_opt(
        k,
        "-g",
        "--showcursor",
        "Cursor im Dateibrowser und Hilfetext anzeigen",
    );
    print_opt(k, "-h", "--help", "Diese Hilfe anzeigen und beenden");
    print_opt(k, "-i", "--autoindent", "Neue Zeilen automatisch einrücken");
    print_opt(
        k,
        "-j",
        "--jumpyscrolling",
        "Halbbildschirmweise statt zeilenweise rollen",
    );
    print_opt(
        k,
        "-k",
        "--cutfromcursor",
        "Vom Cursor bis zum Zeilenende ausschneiden",
    );
    print_opt(
        k,
        "-l",
        "--linenumbers",
        "Zeilennummern vor dem Text anzeigen",
    );
    print_opt(k, "-m", "--mouse", "Benutzung der Maus aktivieren");
    print_opt(k, "-n", "--noread", "Datei nicht lesen (nur schreiben)");
    print_opt(
        k,
        "-o <Verz.>",
        "--operatingdir=<Verz.>",
        "Arbeitsverzeichnis setzen",
    );
    print_opt(
        k,
        "-p",
        "--preserve",
        "Tasten XON (^Q) und XOFF (^S) beibehalten",
    );
    print_opt(
        k,
        "-q",
        "--indicator",
        "Indikator für Position und Teil anzeigen",
    );
    print_opt(
        k,
        "-r <Zahl>",
        "--fill=<Zahl>",
        "Breite für harte Umbrüche und Ausrichtung festlegen",
    );
    print_opt(
        k,
        "-t",
        "--saveonexit",
        "Änderungen beim Beenden ohne Rückfrage speichern",
    );
    print_opt(
        k,
        "-u",
        "--unix",
        "Eine Datei standardmäßig im Unix-Format speichern",
    );
    print_opt(
        k,
        "-v",
        "--view",
        "Nur-Lesen-Modus (keine Veränderungen möglich)",
    );
    print_opt(
        k,
        "-w",
        "--nowrap",
        "Lange Zeilen nicht hart umbrechen [Standard]",
    );
    print_opt(k, "-x", "--nohelp", "Die zwei Hilfe-Zeilen nicht anzeigen");
    print_opt(
        k,
        "-y",
        "--afterends",
        "Strg+Rechts als Stopp am Wortende einrichten",
    );
    print_opt(
        k,
        "-@",
        "--colonparsing",
        "„Dateiname:Zeilennummer“-Notation akzeptieren",
    );
    print_opt(
        k,
        "-%",
        "--stateflags",
        "Statusanzeigen in der Titelleiste aktivieren",
    );
    print_opt(
        k,
        "-_",
        "--minibar",
        "Einen Rückmeldungsstreifen am unteren Rand einblenden",
    );
    print_opt(
        k,
        "-0",
        "--zero",
        "Alle Leisten verbergen und das gesamte Terminal verwenden",
    );
    print_opt(
        k,
        "-1",
        "--solosidescroll",
        "Nur die aktuelle Zeile seitlich rollen",
    );
    print_opt(
        k,
        "-/",
        "--modernbindings",
        "Besser bekannte Tastenkürzel verwenden",
    );
    print_opt(
        k,
        "",
        "--argonmemory=<MiB>",
        "Argon2-Speicher für neue Passwörter",
    );
    print_opt(
        k,
        "",
        "--argonpasses=<Zahl>",
        "Argon2-Durchläufe für neue Passwörter",
    );
}

fn version() {
    say!(" zu, Version {}\n", VERSION);
    say!(
        " Nachbau von GNU nano {} mit Verschlüsselung\n",
        NANO_VERSION
    );
    say!(
        " (C) {} The Free Software Foundation und weitere Mitwirkende\n",
        "2026"
    );
    say!(" Verschlüsselung: Argon2id, XChaCha20-Poly1305\n");
    say!(
        " Speichersperre: {}\n",
        if crypto::MEMORY_LOCKED.load(Ordering::SeqCst) {
            "ganzer Prozess"
        } else if cfg!(target_os = "linux") {
            "nur Schlüssel (für alles: setcap cap_ipc_lock=ep)"
        } else {
            "nur Schlüssel"
        }
    );
    say!(" Kompilierte Optionen: --disable-libmagic --disable-speller --enable-utf8\n");
}

struct LongOpt {
    name: &'static str,
    has_arg: bool,
    val: i32,
}

const fn lo(name: &'static str, has_arg: bool, val: i32) -> LongOpt {
    LongOpt { name, has_arg, val }
}

const OPT_WHITESPACE: i32 = 0xCC;
const OPT_ARGON_MEMORY: i32 = 0x1A0;
const OPT_ARGON_PASSES: i32 = 0x1A1;

const LONG_OPTIONS: &[LongOpt] = &[
    lo("boldtext", false, 'D' as i32),
    lo("multibuffer", false, 'F' as i32),
    lo("newbuffer", false, 'F' as i32),
    lo("ignorercfiles", false, 'I' as i32),
    lo("rawsequences", false, 'K' as i32),
    lo("trimblanks", false, 'M' as i32),
    lo("quotestr", true, 'Q' as i32),
    lo("restricted", false, 'R' as i32),
    lo("quickblank", false, 'U' as i32),
    lo("version", false, 'V' as i32),
    lo("breaklonglines", false, 'b' as i32),
    lo("constantshow", false, 'c' as i32),
    lo("rebinddelete", false, 'd' as i32),
    lo("rcfile", true, 'f' as i32),
    lo("showcursor", false, 'g' as i32),
    lo("help", false, 'h' as i32),
    lo("linenumbers", false, 'l' as i32),
    lo("mouse", false, 'm' as i32),
    lo("operatingdir", true, 'o' as i32),
    lo("preserve", false, 'p' as i32),
    lo("fill", true, 'r' as i32),
    lo("saveonexit", false, 't' as i32),
    lo("view", false, 'v' as i32),
    lo("nowrap", false, 'w' as i32),
    lo("nohelp", false, 'x' as i32),
    lo("modernbindings", false, '/' as i32),
    lo("smarthome", false, 'A' as i32),
    lo("backup", false, 'B' as i32),
    lo("backupdir", true, 'C' as i32),
    lo("tabstospaces", false, 'E' as i32),
    lo("locking", false, 'G' as i32),
    lo("historylog", false, 'H' as i32),
    lo("guidestripe", true, 'J' as i32),
    lo("nonewlines", false, 'L' as i32),
    lo("noconvert", false, 'N' as i32),
    lo("bookstyle", false, 'O' as i32),
    lo("positionlog", false, 'P' as i32),
    lo("softwrap", false, 'S' as i32),
    lo("tabsize", true, 'T' as i32),
    lo("wordbounds", false, 'W' as i32),
    lo("wordchars", true, 'X' as i32),
    lo("zap", false, 'Z' as i32),
    lo("atblanks", false, 'a' as i32),
    lo("emptyline", false, 'e' as i32),
    lo("autoindent", false, 'i' as i32),
    lo("jumpyscrolling", false, 'j' as i32),
    lo("cutfromcursor", false, 'k' as i32),
    lo("noread", false, 'n' as i32),
    lo("indicator", false, 'q' as i32),
    lo("unix", false, 'u' as i32),
    lo("afterends", false, 'y' as i32),
    lo("whitespacedisplay", false, OPT_WHITESPACE),
    lo("colonparsing", false, '@' as i32),
    lo("stateflags", false, '%' as i32),
    lo("minibar", false, '_' as i32),
    lo("zero", false, '0' as i32),
    lo("solosidescroll", false, '1' as i32),
    lo("argonmemory", true, OPT_ARGON_MEMORY),
    lo("argonpasses", true, OPT_ARGON_PASSES),
];

const SHORT_OPTIONS: &str = "ABC:DEFGHIJ:KLMNOPQ:RST:UVWX:Zabcdef:ghijklmno:pqr:tuvwxy@%_01/";

type OptionFound = (i32, Option<Vec<u8>>);

struct Getopt {
    args: Vec<Vec<u8>>,
    index: usize,
    cluster: usize,
    only_operands: bool,
    operands: Vec<Vec<u8>>,
    progname: String,
}

impl Getopt {
    const fn new(args: Vec<Vec<u8>>, progname: String) -> Self {
        Self {
            args,
            index: 1,
            cluster: 0,
            only_operands: false,
            operands: Vec::new(),
            progname,
        }
    }

    fn next(&mut self) -> Option<Result<OptionFound, ()>> {
        loop {
            if self.index >= self.args.len() {
                return None;
            }
            if self.cluster > 0 {
                return Some(self.next_short());
            }
            let arg = self.args[self.index].clone();
            if self.only_operands || arg.len() < 2 || arg[0] != b'-' {
                if std::env::var_os("POSIXLY_CORRECT").is_some() {
                    self.only_operands = true;
                }
                self.operands.push(arg);
                self.index += 1;
                continue;
            }
            if arg == b"--" {
                self.only_operands = true;
                self.index += 1;
                continue;
            }
            if arg[1] == b'-' {
                self.index += 1;
                return Some(self.long_option(&arg[2..]));
            }
            self.cluster = 1;
        }
    }

    fn next_short(&mut self) -> Result<(i32, Option<Vec<u8>>), ()> {
        let arg = self.args[self.index].clone();
        let c = arg[self.cluster];
        self.cluster += 1;
        let at_end = self.cluster >= arg.len();
        let spot = if c == b':' {
            None
        } else {
            SHORT_OPTIONS.bytes().position(|o| o == c)
        };
        let Some(spot) = spot else {
            let mut msg = format!("{}: Ungültige Option -- ", self.progname).into_bytes();
            msg.push(c);
            msg.push(b'\n');
            let _ = std::io::stderr().write_all(&msg);
            if at_end {
                self.cluster = 0;
                self.index += 1;
            }
            return Err(());
        };
        let wants_argument = SHORT_OPTIONS.as_bytes().get(spot + 1) == Some(&b':');
        if !wants_argument {
            if at_end {
                self.cluster = 0;
                self.index += 1;
            }
            return Ok((i32::from(c), None));
        }
        let rest = arg[self.cluster..].to_vec();
        self.cluster = 0;
        self.index += 1;
        if !rest.is_empty() {
            return Ok((i32::from(c), Some(rest)));
        }
        if self.index < self.args.len() {
            let value = self.args[self.index].clone();
            self.index += 1;
            return Ok((i32::from(c), Some(value)));
        }
        warn!(
            "{}: Diese Option benötigt ein Argument -- »{}«\n",
            self.progname, c as char
        );
        Err(())
    }

    fn long_option(&mut self, body: &[u8]) -> Result<(i32, Option<Vec<u8>>), ()> {
        let (name, value) = body
            .iter()
            .position(|&b| b == b'=')
            .map_or((body, None), |i| (&body[..i], Some(body[i + 1..].to_vec())));
        let shown = String::from_utf8_lossy(body).into_owned();
        let exact = LONG_OPTIONS.iter().find(|o| o.name.as_bytes() == name);
        let chosen = if let Some(o) = exact {
            o
        } else {
            let candidates: Vec<&LongOpt> = LONG_OPTIONS
                .iter()
                .filter(|o| o.name.as_bytes().starts_with(name))
                .collect();
            match candidates.first().copied() {
                None => {
                    warn!("{}: Unbekannte Option »--{}«\n", self.progname, shown);
                    return Err(());
                }
                Some(first) => {
                    if candidates
                        .iter()
                        .any(|o| o.val != first.val || o.has_arg != first.has_arg)
                    {
                        let mut list = String::new();
                        for o in &candidates {
                            if std::ptr::eq(*o, first)
                                || o.val != first.val
                                || o.has_arg != first.has_arg
                            {
                                list.push_str(" '--");
                                list.push_str(o.name);
                                list.push('\'');
                            }
                        }
                        warn!(
                            "{}: Die Option »--{}« ist nicht eindeutig; möglich wären:{}\n",
                            self.progname, shown, list
                        );
                        return Err(());
                    }
                    first
                }
            }
        };
        if !chosen.has_arg {
            if value.is_some() {
                warn!(
                    "{}: Die Option »--{}« erlaubt kein Argument\n",
                    self.progname, chosen.name
                );
                return Err(());
            }
            return Ok((chosen.val, None));
        }
        if let Some(v) = value {
            return Ok((chosen.val, Some(v)));
        }
        if self.index < self.args.len() {
            let v = self.args[self.index].clone();
            self.index += 1;
            return Ok((chosen.val, Some(v)));
        }
        warn!(
            "{}: Die Option »--{}« erfordert ein Argument\n",
            self.progname, chosen.name
        );
        Err(())
    }
}

impl Zu {
    pub fn suggest_ctrlt_ctrlz(&mut self) {
        let execute_is_default =
            self.first_sc_for(MMAIN, Func::Execute).map(|s| s.keycode) == Some(0x14);
        let suspend_is_default = self
            .first_sc_for(MEXECUTE, Func::Suspend)
            .map(|s| s.keycode)
            == Some(0x1A);
        if execute_is_default && suspend_is_default {
            self.statusline(
                Msg::Ahem,
                "Um den Editor in den Hintergrund zu schieben, geben Sie ^T^Z ein",
            );
        }
    }

    pub fn terminal_init(&mut self) {
        if self.term.isendwin() {
            self.term.reenter();
        }
        if self.isset(Flag::Preserve) {
            crate::curses::set_flow_control(true);
        }
        disable_kb_interrupt();
    }

    pub fn restore_terminal(&mut self) {
        self.term.curs_set(true);
        self.term.endwin();
        if let Some(t) = self.original_termios {
            crate::curses::set_termios(&t);
        }
    }

    pub fn wipe_secrets(&mut self) {
        self.term.wipe();
        for file in &mut self.files {
            file.keymat = None;
            file.undo = Vec::new();
        }
        self.files = Vec::new();
        self.nodes = Vec::new();
        self.free_nodes = Vec::new();
        self.cutbuffer = None;
        self.cutbottom = None;
        self.answer.zeroize();
        self.last_search.zeroize();
        self.foretext.zeroize();
        self.puddle.zeroize();
        self.prompt_puddle.zeroize();
        self.prompt.zeroize();
        self.help_text.zeroize();
        for code in &mut self.key_buffer {
            *code = 0;
        }
        self.key_buffer = std::collections::VecDeque::new();
        self.pending_input.zeroize();
        self.pending_input = Vec::new();
        self.macro_buffer.zeroize();
        self.previous_macro.zeroize();
        self.plants.zeroize();
        self.search_history = crate::definitions::History::new();
        self.replace_history = crate::definitions::History::new();
        self.execute_history = crate::definitions::History::new();
        self.list_of_completions = Vec::new();
        self.filelist = Vec::new();
    }

    pub fn finish(&mut self) -> ! {
        self.blank_statusbar();
        self.blank_bottombars();
        self.term.wrefresh(Win::Foot);
        self.term.delwin(Win::Top);
        self.term.delwin(Win::Mid);
        self.term.delwin(Win::Foot);
        self.restore_terminal();
        self.display_rcfile_errors();
        let status = self.final_status;
        self.wipe_secrets();
        std::process::exit(status);
    }

    pub fn close_and_go(&mut self) {
        if let Some(lockname) = self.ofile().lock_filename.clone() {
            self.delete_lockfile(&lockname);
        }
        if self.isset(Flag::PositionLog) && !self.ofile().filename.is_empty() {
            self.update_positions_register();
        }
        if self.files.len() > 1 {
            self.switch_to_next_buffer();
            self.cur = self.prev_buffer_index(self.cur);
            self.close_buffer();
            self.cur = self.next_buffer_index(self.cur);
            self.titlebar(None);
        } else {
            self.finish();
        }
    }

    pub fn do_exit(&mut self) {
        let choice = if !self.ofile().modified || self.isset(Flag::ViewMode) {
            NO
        } else if self.isset(Flag::SaveOnExit) && !self.ofile().filename.is_empty() {
            YES
        } else {
            if self.isset(Flag::SaveOnExit) {
                self.warn_and_briefly_pause("Kein Dateiname");
            }
            self.ask_user(YESORNO, "Geänderten Puffer speichern? ")
        };
        if choice == NO || (choice == YES && self.write_it_out(true, true) > 0) {
            self.close_and_go();
        } else if choice != YES {
            self.statusbar("Abgebrochen");
        }
    }

    fn emergency_save(&mut self, filename: &[u8]) {
        let plainname: Vec<u8> = if filename.is_empty() {
            format!("zu.{}", std::process::id()).into_bytes()
        } else {
            filename.to_vec()
        };
        if self.ofile().keymat.is_none() {
            let shown = String::from_utf8_lossy(&plainname).into_owned();
            warn!("\nPuffer „{}“ nicht gesichert (kein Passwort)\n", shown);
            return;
        }
        let targetname = get_next_filename(&plainname, b".save");
        if targetname.is_empty() {
            warn!("\nZu viele .save-Dateien\n");
        } else if self.write_file(&targetname, Writing::Special, NONOTES) {
            warn!(
                "\nPuffer in {} geschrieben\n",
                String::from_utf8_lossy(&targetname)
            );
        }
    }

    pub fn die(&mut self, msg: &str) -> ! {
        self.die_bytes(msg.as_bytes())
    }

    pub fn die_bytes(&mut self, msg: &[u8]) -> ! {
        if STABS.fetch_add(1, Ordering::SeqCst) > 0 {
            std::process::exit(11);
        }
        self.restore_terminal();
        self.display_rcfile_errors();
        let _ = std::io::stderr().write_all(msg);
        let count = self.files.len();
        let firstone = self.cur;
        for step in 0..count {
            self.cur = (firstone + step) % count;
            if let Some(lockname) = self.ofile().lock_filename.clone() {
                self.delete_lockfile(&lockname);
            }
            if self.ofile().modified && !self.isset(Flag::Restricted) {
                let name = self.ofile().filename.clone();
                self.emergency_save(&name);
            }
        }
        self.wipe_secrets();
        std::process::exit(1);
    }

    pub fn window_init(&mut self) {
        self.keypad_on = !self.isset(Flag::RawSequences);
        let lines = self.lines();
        self.term.delwin(Win::Top);
        self.term.delwin(Win::Mid);
        self.term.delwin(Win::Foot);
        if lines < 3 {
            self.editwinrows = if self.isset(Flag::Zero) { lines } else { 1 };
            self.term.newwin(Win::Mid, self.editwinrows, 0);
            self.term.newwin(Win::Foot, 1, lines - 1);
        } else {
            let minimum = if self.isset(Flag::Zero) {
                3
            } else if self.isset(Flag::MiniBar) {
                4
            } else {
                5
            };
            let mut toprows = if self.isset(Flag::EmptyLine) && lines > minimum {
                2
            } else {
                1
            };
            let bottomrows = if self.isset(Flag::NoHelp) || lines < minimum {
                1
            } else {
                3
            };
            if self.isset(Flag::MiniBar) || self.isset(Flag::Zero) {
                toprows = 0;
            }
            self.editwinrows = lines - toprows - bottomrows + usize::from(self.isset(Flag::Zero));
            if toprows > 0 {
                self.term.newwin(Win::Top, toprows, 0);
            }
            self.term.newwin(Win::Mid, self.editwinrows, toprows);
            self.term.newwin(Win::Foot, bottomrows, lines - bottomrows);
        }
        for w in [Win::Top, Win::Mid, Win::Foot] {
            if self.term.win_exists(w) {
                self.term.werase(w);
            }
        }
        self.term.wnoutrefresh(Win::Foot);
        let cols = self.cols().cast_signed();
        if cols + self.fill < 0 {
            self.wrap_at = 0;
        } else if self.fill <= 0 {
            self.wrap_at = (cols + self.fill).cast_unsigned();
        } else {
            self.wrap_at = self.fill.cast_unsigned();
        }
    }

    fn disable_mouse_support(&mut self) {
        let mut out = Vec::new();
        self.term.mouse.mousemask(0, &mut out);
        self.term.mouse.mouseinterval(self.oldinterval);
        self.term.emit_bytes(&out);
    }

    fn enable_mouse_support(&mut self) {
        let mut out = Vec::new();
        self.term
            .mouse
            .mousemask(crate::curses::mouse::ALL_MOUSE_EVENTS, &mut out);
        if self.term.mouse.take_xterm_kmous_request() {
            self.keytable.add(b"\x1B[M", KEY_MOUSE);
        }
        self.oldinterval = self.term.mouse.mouseinterval(50);
        self.term.emit_bytes(&out);
    }

    pub fn mouse_init(&mut self) {
        if self.isset(Flag::UseMouse) {
            self.enable_mouse_support();
        } else {
            self.disable_mouse_support();
        }
    }

    fn scoop_stdin(&mut self) -> bool {
        self.restore_terminal();
        if isatty(0) {
            warn!(
                "Daten werden von der Tastatur gelesen, geben Sie ^D oder ^D^D zum Beenden ein.\n"
            );
        }
        install_handler_for_ctrl_c();
        let mut input: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
        let mut chunk = Zeroizing::new([0u8; 8192]);
        let mut failure: Option<std::io::Error> = None;
        let mut stdin = std::io::stdin();
        loop {
            if CONTROL_C_WAS_PRESSED.load(Ordering::SeqCst) {
                break;
            }
            match stdin.read(&mut chunk[..]) {
                Ok(0) => break,
                Ok(n) => input.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }
        restore_handler_for_ctrl_c();
        if !isatty(0) {
            self.reconnect_and_store_state();
        }
        self.terminal_init();
        self.term.doupdate();
        if let Some(e) = failure {
            let m = fmt!(
                "Standardeingabe kann nicht geöffnet werden: %s",
                &io_errstr(&e)
            );
            self.statusline(Msg::Alert, &m);
            return false;
        }
        if CONTROL_C_WAS_PRESSED.swap(false, Ordering::SeqCst) {
            self.control_c_was_pressed = true;
            self.statusline(Msg::Alert, "Unterbrochen");
        }
        self.make_new_buffer();
        self.read_file(&input, b"stdin", false);
        if !self.isset(Flag::ViewMode) && self.ofile().totsize > 0 {
            self.set_modified();
        }
        true
    }

    fn reconnect_and_store_state(&mut self) {
        let thetty =
            unsafe { libc::open(c"/dev/tty".as_ptr().cast::<libc::c_char>(), libc::O_RDONLY) };
        if thetty < 0 || unsafe { libc::dup2(thetty, 0) } < 0 {
            self.die(
                "Standardeingabe konnte leider nicht erneut mit der Tastatur verbunden werden.\n",
            );
        }
        unsafe {
            libc::close(thetty);
        }
        if !CONTROL_C_WAS_PRESSED.load(Ordering::SeqCst) {
            self.original_termios = crate::curses::get_termios();
        }
    }

    fn suspend_zu(&mut self) {
        self.disable_mouse_support();
        self.restore_terminal();
        say!("\n\nBenutzen Sie „fg“, um zu Zu zurückzukehren.\n");
        let _ = std::io::stdout().flush();
        self.lastmessage = Msg::Hush;
        GOT_CONT.store(false, Ordering::SeqCst);
        unsafe {
            libc::kill(0, libc::SIGSTOP);
        }
        GOT_CONT.store(false, Ordering::SeqCst);
        self.continue_zu();
    }

    pub fn do_suspend(&mut self) {
        if self.in_restricted_mode() {
            return;
        }
        self.suspend_zu();
        self.ran_a_tool = true;
    }

    fn continue_zu(&mut self) {
        self.terminal_init();
        if self.isset(Flag::UseMouse) {
            self.enable_mouse_support();
        }
        self.term.refresh_size();
        self.the_window_resized = true;
    }

    pub fn check_signals(&mut self) {
        if GOT_HUPTERM.swap(false, Ordering::SeqCst) {
            self.die("SIGHUP oder SIGTERM empfangen\n");
        }
        if GOT_WINCH.swap(false, Ordering::SeqCst) {
            self.the_window_resized = true;
            self.resized_for_browser = true;
        }
        if GOT_TSTP.swap(false, Ordering::SeqCst) {
            self.suspend_zu();
        }
        if GOT_CONT.swap(false, Ordering::SeqCst) {
            self.continue_zu();
        }
        if self.term.hangup {
            self.die("Zu viele Fehler aus der Standardeingabe\n");
        }
        if self.term.read_errors > 12 {
            self.die("Zu viele Fehler aus der Standardeingabe\n");
        }
    }

    pub fn regenerate_screen(&mut self) {
        self.the_window_resized = false;
        self.term.refresh_size();
        let (lines, cols) = (self.lines(), self.cols());
        self.sidebar = usize::from(self.isset(Flag::Indicator) && lines > 5 && cols > 9);
        self.bardata = vec![Attr::NORMAL; lines];
        self.editwincols = cols.saturating_sub(self.margin + self.sidebar).max(1);
        self.terminal_init();
        self.window_init();
        if self.we_are_running {
            self.ensure_firstcolumn_is_aligned();
            self.draw_all_subwindows();
        }
    }

    pub fn toggle_this(&mut self, flag: Flag) {
        let mut enabled = !self.isset(flag);
        self.flags.toggle(flag);
        self.focusing = false;
        match flag {
            Flag::Zero => {
                self.window_init();
                self.draw_all_subwindows();
                return;
            }
            Flag::NoHelp => {
                let minimum = if self.isset(Flag::Zero) {
                    3
                } else if self.isset(Flag::MiniBar) {
                    4
                } else {
                    5
                };
                if self.lines() < minimum {
                    self.statusline(Msg::Ahem, "Zu klein");
                    self.flags.toggle(flag);
                    return;
                }
                self.window_init();
                self.draw_all_subwindows();
            }
            Flag::ConstantShow => {
                if self.lines() == 1 {
                    self.statusline(Msg::Ahem, "Zu klein");
                    self.flags.toggle(flag);
                } else if self.isset(Flag::Zero) {
                    self.flags.set(Flag::ConstantShow);
                    self.toggle_this(Flag::Zero);
                } else if !self.isset(Flag::MiniBar) {
                    self.wipe_statusbar();
                }
                return;
            }
            Flag::SoftWrap => {
                if !self.isset(Flag::SoftWrap) {
                    self.ofile_mut().firstcolumn = 0;
                }
                self.refresh_needed = true;
            }
            Flag::WhitespaceDisplay => {
                self.titlebar(None);
                self.refresh_needed = true;
            }
            Flag::UseMouse => self.mouse_init(),
            _ => {}
        }
        if matches!(
            flag,
            Flag::AutoIndent | Flag::BreakLongLines | Flag::SoftWrap
        ) {
            if self.isset(Flag::MiniBar) && !self.isset(Flag::Zero) && self.isset(Flag::StateFlags)
            {
                return;
            }
            if self.isset(Flag::StateFlags) {
                self.titlebar(None);
            }
        }
        if matches!(
            flag,
            Flag::NoHelp | Flag::LineNumbers | Flag::WhitespaceDisplay
        ) && (self.isset(Flag::MiniBar) || self.isset(Flag::Zero) || self.lines() == 1)
        {
            return;
        }
        if flag == Flag::NoHelp || flag == Flag::NoSyntax {
            enabled = !enabled;
        }
        let m = fmt!(
            "%s %s",
            crate::global::epithet_of_flag(flag),
            if enabled { "aktiviert" } else { "deaktiviert" }
        );
        self.statusline(Msg::Remark, &m);
    }

    pub fn confirm_margin(&mut self) {
        let mut needed_margin = digits(self.lineno(self.filebot())) + 1;
        if !self.isset(Flag::LineNumbers) || needed_margin + 4 > self.cols() {
            needed_margin = 0;
        }
        if needed_margin != self.margin {
            let keep_focus = self.margin > 0 && self.focusing;
            self.margin = needed_margin;
            self.editwincols = self
                .cols()
                .saturating_sub(self.margin + self.sidebar)
                .max(1);
            self.ensure_firstcolumn_is_aligned();
            self.focusing = keep_focus;
            self.refresh_needed = true;
        }
    }

    pub fn unbound_key(&mut self, code: i32) {
        if code == FOREIGN_SEQUENCE {
            self.statusline(Msg::Ahem, "Unbekannte Sequenz");
        } else if code == NO_SUCH_FUNCTION {
            let name = self.commandname.clone();
            let m = fmtb!("Unbekannte Funktion: %s", &name);
            self.statusline_bytes(Msg::Ahem, &m);
        } else if code == MISSING_BRACE {
            self.statusline(Msg::Ahem, "Fehlende }");
        } else if code > KEY_F0 && code < KEY_F0 + 25 {
            let m = fmt!("Nicht zugewiesene Taste: F%i", code - KEY_F0);
            self.statusline(Msg::Ahem, &m);
        } else if code > 0x7F {
            self.statusline(Msg::Ahem, "Nicht zugewiesen");
        } else if self.meta_key {
            if code < 0x20 {
                let m = fmt!(
                    "Nicht zuweisbar: M-^%c",
                    char::from(u8_of(i64::from(code + 0x40)))
                );
                self.statusline(Msg::Ahem, &m);
            } else if self.shifted_metas && ('A' as i32..='Z' as i32).contains(&code) {
                let m = fmt!(
                    "Nicht zugewiesene Taste: %s%c",
                    "Sh-M-",
                    char::from(u8_of(i64::from(code)))
                );
                self.statusline(Msg::Ahem, &m);
            } else {
                let m = fmt!(
                    "Nicht zugewiesene Taste: %s%c",
                    "M-",
                    char::from(u8_of(i64::from(code))).to_ascii_uppercase()
                );
                self.statusline(Msg::Ahem, &m);
            }
        } else if code == ESC_CODE {
            self.statusline(Msg::Ahem, "Nicht zuweisbar: ^[");
        } else if (0..0x20).contains(&code) {
            let m = fmt!(
                "Nicht zugewiesene Taste: %s%c",
                "^",
                char::from(u8_of(i64::from(code + 0x40)))
            );
            self.statusline(Msg::Ahem, &m);
        } else if code >= 0x20 {
            let m = fmt!(
                "Nicht zugewiesene Taste: %s%c",
                "",
                char::from(u8_of(i64::from(code)))
            );
            self.statusline(Msg::Ahem, &m);
        }
        self.set_blankdelay_to_one();
    }

    fn process_click(&mut self) -> i32 {
        let (retval, mut click_row, click_col) = self.get_mouseinput();
        if retval != 0 {
            return retval;
        }
        if self.wmouse_trafo(Win::Mid, &mut click_row, click_col) {
            let was_current = self.current();
            let row_count = i64::from(click_row) - self.ofile().cursor_row;
            let was_x = self.current_x();
            let mut leftedge = if self.isset(Flag::SoftWrap) {
                self.leftedge_for(self.xplustabs(), was_current)
            } else {
                self.get_page_start(self.xplustabs())
            };
            let mut line = was_current;
            if row_count < 0 {
                self.go_back_chunks(i32_of(-row_count), &mut line, &mut leftedge);
            } else {
                self.go_forward_chunks(i32_of(row_count), &mut line, &mut leftedge);
            }
            self.ofile_mut().current = line;
            let column = self.actual_last_column(leftedge, usize_of(i64::from(click_col.max(0))));
            let x = self.actual_x(self.data(line), column);
            self.ofile_mut().current_x = x;
            if row_count == 0 && x == was_x {
                self.do_mark();
                if self.isset(Flag::StateFlags) {
                    self.titlebar(None);
                }
            } else {
                self.keep_cutbuffer = false;
            }
            self.edit_redraw(was_current, Update::Centering);
        }
        2
    }

    pub fn execute_function(&mut self, func: Func) {
        match func {
            Func::Help => self.do_help(),
            Func::Exit => self.do_exit(),
            Func::Writeout => self.do_writeout(),
            Func::Savefile => self.do_savefile(),
            Func::Insertfile => self.do_insertfile(),
            Func::SearchForward => self.do_search_forward(),
            Func::SearchBackward => self.do_search_backward(),
            Func::FindPrevious => self.do_findprevious(),
            Func::FindNext => self.do_findnext(),
            Func::Replace => self.do_replace(),
            Func::CutText => self.cut_text(),
            Func::CopyText => self.copy_text(),
            Func::PasteText => self.paste_text(),
            Func::Execute => self.do_execute(),
            Func::CutTillEof => self.cut_till_eof(),
            Func::ZapText => self.zap_text(),
            Func::Mark => self.do_mark(),
            Func::Location => self.report_cursor_position(),
            Func::GotoLineColumn => self.do_gotolinecolumn(),
            Func::Justify => self.do_justify(),
            Func::FullJustify => self.do_full_justify(),
            Func::ParaBegin => self.to_para_begin(),
            Func::ParaEnd => self.to_para_end(),
            Func::Comment => self.do_comment(),
            Func::Complete => self.complete_a_word(),
            Func::Indent => self.do_indent(),
            Func::Unindent => self.do_unindent(),
            Func::ChopPrevWord => self.chop_previous_word(),
            Func::ChopNextWord => self.chop_next_word(),
            Func::FindBracket => self.do_find_bracket(),
            Func::WordCount => self.count_lines_words_and_characters(),
            Func::RecordMacro => self.record_macro(),
            Func::RunMacro => self.run_macro(),
            Func::Anchor => self.put_or_lift_anchor(),
            Func::PrevAnchor => self.to_prev_anchor(),
            Func::NextAnchor => self.to_next_anchor(),
            Func::Undo => self.do_undo(),
            Func::Redo => self.do_redo(),
            Func::Suspend => self.do_suspend(),
            Func::Left => self.do_left(),
            Func::Right => self.do_right(),
            Func::Up => self.do_up(),
            Func::Down => self.do_down(),
            Func::ScrollLeft => self.do_scroll_left(),
            Func::ScrollRight => self.do_scroll_right(),
            Func::ScrollUp => self.do_scroll_up(),
            Func::ScrollDown => self.do_scroll_down(),
            Func::PrevWord => self.to_prev_word(),
            Func::NextWord => self.to_next_word(),
            Func::Home => self.do_home(),
            Func::End => self.do_end(),
            Func::PrevBlock => self.to_prev_block(),
            Func::NextBlock => self.to_next_block(),
            Func::TopRow => self.to_top_row(),
            Func::BottomRow => self.to_bottom_row(),
            Func::Center => self.do_center(),
            Func::CursesVersion => self.statusline(
                Msg::Notice,
                concat!(
                    "Zu-Terminalansteuerung ",
                    env!("CARGO_PKG_VERSION"),
                    " (ohne ncurses)"
                ),
            ),
            Func::Cycle => self.do_cycle(),
            Func::PageUp => self.do_page_up(),
            Func::PageDown => self.do_page_down(),
            Func::FirstLine => self.to_first_line(),
            Func::LastLine => self.to_last_line(),
            Func::PrevBuffer => self.switch_to_prev_buffer(),
            Func::NextBuffer => self.switch_to_next_buffer(),
            Func::Verbatim => self.do_verbatim_input(),
            Func::Tab => self.do_tab(),
            Func::Enter => self.do_enter(),
            Func::Delete => self.do_delete(),
            Func::Backspace => self.do_backspace(),
            Func::FullRefresh => self.full_refresh(),
            Func::FirstFile => self.to_first_file(),
            Func::LastFile => self.to_last_file(),
            Func::SuggestCtrlTZ => self.suggest_ctrlt_ctrlz(),
            Func::SuckUpPaste => self.suck_up_input_and_paste_it(),
            Func::ChangePassword => {
                if self.currmenu != MWRITEFILE {
                    self.change_password();
                }
            }
            Func::Cancel
            | Func::DiscardBuffer
            | Func::CaseSens
            | Func::Regexp
            | Func::Backwards
            | Func::FlipReplace
            | Func::FlipGoto
            | Func::OlderItem
            | Func::NewerItem
            | Func::DosFormat
            | Func::AppendIt
            | Func::PrependIt
            | Func::BackItUp
            | Func::FlipExecute
            | Func::FlipConvert
            | Func::FlipNewbuffer
            | Func::ToFiles
            | Func::GotoDir
            | Func::Toggle
            | Func::Implant
            | Func::Nothing => {}
        }
    }

    fn suck_up_input_and_paste_it(&mut self) {
        let was_cutbuffer = self.cutbuffer;
        let mut line = self.make_new_node(None);
        let mut input;
        self.cutbuffer = Some(line);
        loop {
            input = self.get_kbinput(Win::Mid, BLIND);
            if ((0x20..=0xFF).contains(&input) && input != DEL_CODE) || input == '\t' as i32 {
                self.nodes[line].data.push(u8_of(i64::from(input)));
            } else if input == '\r' as i32 || input == '\n' as i32 {
                let fresh = self.make_new_node(Some(line));
                self.nodes[line].next = Some(fresh);
                line = fresh;
            } else {
                break;
            }
        }
        if self.isset(Flag::ViewMode) {
            self.print_view_warning();
        } else {
            self.paste_text();
        }
        if input != END_OF_PASTE {
            self.statusline(Msg::Alert, "Fehlerhaftes Einfügen");
        }
        let pasted = self.cutbuffer;
        self.free_lines(pasted);
        self.cutbuffer = was_cutbuffer;
    }

    fn process_a_keystroke(&mut self) {
        let was_mark = self.ofile().mark;
        let mut input = self.get_kbinput(Win::Mid, VISIBLE);
        self.lastmessage = Msg::Vacuum;
        if input == THE_WINDOW_RESIZED {
            return;
        }
        if input == KEY_MOUSE {
            if self.process_click() == 1 {
                input = self.get_kbinput(Win::Mid, BLIND);
            } else {
                return;
            }
        }
        let shortcut = self.get_shortcut(input);
        let function = shortcut.as_ref().map(|s| s.func);
        if function.is_none() {
            if !(0x20..=0xFF).contains(&input) || self.meta_key {
                self.unbound_key(input);
            } else if self.isset(Flag::ViewMode) {
                self.print_view_warning();
            } else {
                if self.ofile().mark.is_some() && self.ofile().softmark {
                    self.ofile_mut().mark = None;
                    self.refresh_needed = true;
                }
                self.puddle.push(u8_of(i64::from(input)));
            }
        }
        if !self.puddle.is_empty() && (function.is_some() || self.waiting_keycodes() == 0) {
            let mut burst = std::mem::take(&mut self.puddle);
            self.inject(&mut burst);
            burst.zeroize();
        }
        if function != Some(Func::Cycle) {
            self.cycling_aim = 0;
        }
        let Some(function) = function else {
            self.pletion_line = None;
            self.keep_cutbuffer = false;
            return;
        };
        if self.isset(Flag::ViewMode) && changes_something(function) {
            self.print_view_warning();
            return;
        }
        if input == 0x08
            && self.give_a_hint
            && self.current_x() == 0
            && self.current() == self.filetop()
            && !self.isset(Flag::NoHelp)
        {
            self.statusbar("^W = Strg+W    M-W = Alt+W");
            self.give_a_hint = false;
        } else if self.meta_key {
            self.give_a_hint = false;
        }
        if function == Func::Implant {
            if let Some(expansion) = shortcut.as_ref().and_then(|s| s.expansion.clone()) {
                self.implant(&expansion);
            }
            return;
        }
        if function == Func::Toggle {
            if let Some(flag) = shortcut.as_ref().and_then(|s| s.toggle) {
                self.toggle_this(flag);
                if flag == Flag::CutFromCursor {
                    self.keep_cutbuffer = false;
                }
            }
            return;
        }
        if function != Func::CutText
            && function != Func::CopyText
            && function != Func::ZapText
            && function != Func::RecordMacro
            && function != Func::RunMacro
        {
            self.keep_cutbuffer = false;
        }
        if function != Func::Complete {
            self.pletion_line = None;
        }
        let was_current = self.current();
        let was_x = self.current_x();
        if self.shift_held && self.ofile().mark.is_none() {
            let of = self.ofile_mut();
            of.mark = Some(of.current);
            of.mark_x = of.current_x;
            of.softmark = true;
        }
        self.execute_function(function);
        if self.ofile().mark.is_some()
            && self.ofile().softmark
            && !self.shift_held
            && (self.current() != was_current
                || self.current_x() != was_x
                || wanted_to_move(function))
        {
            self.ofile_mut().mark = None;
            self.refresh_needed = true;
        } else if self.current() != was_current {
            self.also_the_last = false;
        }
        if self.isset(Flag::StateFlags) && self.ofile().mark != was_mark {
            self.titlebar(None);
        }
    }
}

fn bad_argument(format: &str, argument: &[u8]) -> ! {
    let _ = std::io::stderr().write_all(&fmtb!(format, argument));
    warn!("\n");
    std::process::exit(1);
}

fn main() {
    if !crate::chars::setup_locale() {
        warn!("Zu benötigt ein UTF-8-Gebietsschema.\n");
        std::process::exit(1);
    }
    crypto::harden_process();

    let mut k = Zu::new();

    std::panic::set_hook(Box::new(|info| {
        if let (Some(l), Ok(mut place)) = (info.location(), PANIC_PLACE.lock())
            && place.is_none()
        {
            *place = Some(format!("{}:{}", l.file(), l.line()));
        }
    }));

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut k)));
    if outcome.is_err() {
        let place = PANIC_PLACE
            .lock()
            .ok()
            .and_then(|p| p.clone())
            .unwrap_or_else(|| "?".to_string());
        let message =
            format!("Hoppla! Zu ist abgestürzt ({place}).  Bitte melden Sie dies als Fehler.\n");
        let rescue = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            k.die(&message);
        }));
        if rescue.is_err() {
            if crate::curses::IN_CURSES.load(Ordering::SeqCst) {
                crypto::raw_write(1, b"\x1B[?1006l\x1B[?1000l");
                let rows = crate::curses::terminal_size().map_or(24, |(r, _)| r);
                crypto::raw_write(1, &crate::curses::leave_sequence(rows));
            }
            if let Some(t) = k.original_termios {
                crate::curses::set_termios(&t);
            }
            warn!("{}", message);
        }
        std::process::exit(1);
    }
}

fn run(k: &mut Zu) {
    let mut ignore_rcfiles = false;
    let mut fill_used = false;
    let mut hardwrap = -2;
    let mut argon_memory: Option<u32> = None;
    let mut argon_passes: Option<u32> = None;

    #[cfg(target_os = "linux")]
    {
        const VT_GETSTATE: libc::c_ulong = 0x5603;
        let mut dummy = [0u16; 3];
        k.on_a_vt = unsafe { libc::ioctl(1, VT_GETSTATE as _, dummy.as_mut_ptr()) } == 0;
    }

    k.original_termios = crate::curses::get_termios();
    if let Some(t) = k.original_termios {
        let _ = CRASH_TERMIOS.set(t);
    }

    unsafe {
        k.stdin_flags = libc::fcntl(0, libc::F_GETFL, 0);
        if k.stdin_flags != -1 {
            libc::fcntl(0, libc::F_SETFL, k.stdin_flags & !libc::O_NONBLOCK);
        }
    }

    k.flags.set(Flag::NoWrap);

    let cmdline: Vec<Vec<u8>> = std::env::args_os()
        .map(std::os::unix::ffi::OsStringExt::into_vec)
        .collect();
    let progname = match cmdline.first() {
        Some(a) if !a.is_empty() => String::from_utf8_lossy(a).into_owned(),
        _ => "zu".to_string(),
    };

    if tail(progname.as_bytes()).first() == Some(&b'r') {
        k.flags.set(Flag::Restricted);
    }

    let mut getopt = Getopt::new(cmdline, progname.clone());
    while let Some(option) = getopt.next() {
        let Ok((optchr, optarg)) = option else {
            say!(
                "Benutzen Sie „{} -h“ für eine Liste verfügbarer Optionen.\n",
                progname
            );
            std::process::exit(1);
        };
        let optarg = optarg.unwrap_or_default();
        match optchr {
            OPT_WHITESPACE => k.flags.set(Flag::WhitespaceDisplay),
            OPT_ARGON_MEMORY => match parse_num(&optarg).and_then(|n| u32::try_from(n).ok()) {
                Some(n) if (crypto::MIN_MEMORY_MIB..=crypto::MAX_MEMORY_MIB).contains(&n) => {
                    argon_memory = Some(n * 1024);
                }
                _ => bad_argument(
                    "Argon2-Speichergröße „%s“ ist ungültig (64 bis 4096 MiB)",
                    &optarg,
                ),
            },
            OPT_ARGON_PASSES => match parse_num(&optarg).and_then(|n| u32::try_from(n).ok()) {
                Some(n) if (crypto::MIN_PASSES..=crypto::MAX_PASSES).contains(&n) => {
                    argon_passes = Some(n);
                }
                _ => bad_argument(
                    "Argon2-Durchlaufzahl „%s“ ist ungültig (3 bis 100)",
                    &optarg,
                ),
            },
            _ => match u8_of(i64::from(optchr)) {
                b'A' => k.flags.set(Flag::SmartHome),
                b'B' => k.flags.set(Flag::MakeBackup),
                b'C' => k.backup_dir = Some(optarg),
                b'D' => k.flags.set(Flag::BoldText),
                b'E' => k.flags.set(Flag::TabsToSpaces),
                b'F' => k.flags.set(Flag::NewBuffer),
                b'G' => k.flags.set(Flag::Locking),
                b'H' => k.flags.set(Flag::HistoryLog),
                b'I' => ignore_rcfiles = true,
                b'J' => match parse_num(&optarg) {
                    Some(n) if n > 0 => k.stripe_column = n,
                    _ => bad_argument("Markierungsspalte „%s“ ist ungültig", &optarg),
                },
                b'K' => k.flags.set(Flag::RawSequences),
                b'L' => k.flags.set(Flag::NoNewlines),
                b'M' => k.flags.set(Flag::TrimBlanks),
                b'N' => k.flags.set(Flag::NoConvert),
                b'O' => k.flags.set(Flag::BookStyle),
                b'P' => k.flags.set(Flag::PositionLog),
                b'Q' => k.quotestr = optarg,
                b'R' => k.flags.set(Flag::Restricted),
                b'S' => k.flags.set(Flag::SoftWrap),
                b'T' => match parse_num(&optarg) {
                    Some(n) if n > 0 => k.tabsize = n,
                    _ => bad_argument("Angeforderte Tabulatorbreite „%s“ ist ungültig", &optarg),
                },
                b'U' => k.flags.set(Flag::QuickBlank),
                b'V' => {
                    version();
                    std::process::exit(0);
                }
                b'W' => k.flags.set(Flag::WordBounds),
                b'X' => k.word_chars = optarg,
                b'Z' => k.flags.set(Flag::LetThemZap),
                b'a' => k.flags.set(Flag::AtBlanks),
                b'b' => hardwrap = 1,
                b'c' => k.flags.set(Flag::ConstantShow),
                b'd' => k.flags.set(Flag::RebindDelete),
                b'e' => k.flags.set(Flag::EmptyLine),
                b'f' => k.custom_nanorc = Some(optarg),
                b'g' => k.flags.set(Flag::ShowCursor),
                b'h' => {
                    usage(k);
                    std::process::exit(0);
                }
                b'i' => k.flags.set(Flag::AutoIndent),
                b'j' => k.flags.set(Flag::JumpyScrolling),
                b'k' => k.flags.set(Flag::CutFromCursor),
                b'l' => k.flags.set(Flag::LineNumbers),
                b'm' => k.flags.set(Flag::UseMouse),
                b'n' => k.flags.set(Flag::NoreadMode),
                b'o' => k.operating_dir = Some(optarg),
                b'p' => k.flags.set(Flag::Preserve),
                b'q' => k.flags.set(Flag::Indicator),
                b'r' => match parse_num(&optarg) {
                    Some(n) => {
                        k.fill = n;
                        fill_used = true;
                    }
                    None => bad_argument("Angeforderte Füllgröße „%s“ ist ungültig", &optarg),
                },
                b't' => k.flags.set(Flag::SaveOnExit),
                b'u' => k.flags.set(Flag::MakeItUnix),
                b'v' => k.flags.set(Flag::ViewMode),
                b'w' => hardwrap = 0,
                b'x' => k.flags.set(Flag::NoHelp),
                b'y' => k.flags.set(Flag::AfterEnds),
                b'@' => k.flags.set(Flag::ColonParsing),
                b'%' => k.flags.set(Flag::StateFlags),
                b'_' => k.flags.set(Flag::MiniBar),
                b'0' => k.flags.set(Flag::Zero),
                b'/' => k.flags.set(Flag::ModernBindings),
                b'1' => k.flags.set(Flag::SoloSidescroll),
                _ => {
                    say!(
                        "Benutzen Sie „{} -h“ für eine Liste verfügbarer Optionen.\n",
                        progname
                    );
                    std::process::exit(1);
                }
            },
        }
    }
    let operands: Vec<Vec<u8>> = std::mem::take(&mut getopt.operands);
    drop(getopt);

    set_handler(
        libc::SIGWINCH,
        handle_sigwinch as *const () as libc::sighandler_t,
        false,
    );

    if !isatty(1) {
        k.die("Standardausgabe ist kein Terminal\n");
    }

    if std::env::var_os("TERM").is_none() {
        unsafe { std::env::set_var("TERM", "vt220") };
    }
    let term_name = std::env::var("TERM")
        .ok()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let Some(ti) = crate::curses::terminfo::TermInfo::load(&term_name)
        .filter(|ti| !ti.flag("gn") && !ti.flag("hc"))
    else {
        warn!("Fehler beim Öffnen des Terminals: {}.\n", term_name);
        std::process::exit(1);
    };
    if ti.get("cup").is_none_or(<[u8]>::is_empty) {
        warn!(
            "Terminal „{}“ kann den Cursor nicht positionieren.\n",
            term_name
        );
        std::process::exit(1);
    }
    k.colors = ti.colors();
    k.term_has_colors = ti.has_colors();
    k.terminfo_kbs = ti.get("kbs").map(<[u8]>::to_vec);
    k.keytable = crate::curses::terminfo::KeyTable::build(&ti);
    k.term.pad = crate::curses::terminfo::PadInfo::new(
        &ti,
        crate::curses::baudrate(),
        std::env::var_os("NCURSES_NO_PADDING").is_some(),
    );
    k.term.bell = ti.get("bel").map(<[u8]>::to_vec);
    k.term.flash = ti.get("flash").map(<[u8]>::to_vec);
    k.term.mouse.configure(&ti, k.term.pad);
    k.term.terminfo_lines = ti.lines();
    k.term.terminfo_cols = ti.cols();
    k.term.cursor_invisible = ti
        .get("civis")
        .filter(|v| !v.is_empty())
        .map(<[u8]>::to_vec);
    k.term.cursor_normal = ti
        .get("cnorm")
        .filter(|v| !v.is_empty())
        .map(<[u8]>::to_vec);
    k.term.keypad_xmit = ti.get("smkx").map_or_default(<[u8]>::to_vec);
    k.term.keypad_local = ti.get("rmkx").map_or_default(<[u8]>::to_vec);
    let mut crash_keypad_local = Vec::new();
    crate::curses::terminfo::tputs(
        &mut crash_keypad_local,
        &k.term.keypad_local,
        false,
        &crate::curses::terminfo::PadInfo::default(),
    );
    let _ = CRASH_KEYPAD_LOCAL.set(crash_keypad_local);

    k.term.initscr();

    k.shortcut_init();

    if !ignore_rcfiles {
        let fill_cmdline = k.fill;
        let backup_dir_cmdline = k.backup_dir.take();
        let word_chars_cmdline = std::mem::take(&mut k.word_chars);
        let stripeclm_cmdline = k.stripe_column;
        let tabsize_cmdline = k.tabsize;
        let operating_dir_cmdline = k.operating_dir.take();
        let quotestr_cmdline = std::mem::take(&mut k.quotestr);
        let flags_cmdline = k.flags;

        k.do_rcfiles();

        if fill_used {
            k.fill = fill_cmdline;
        }
        if backup_dir_cmdline.is_some() {
            k.backup_dir = backup_dir_cmdline;
        }
        if !word_chars_cmdline.is_empty() {
            k.word_chars = word_chars_cmdline;
        }
        if stripeclm_cmdline > 0 {
            k.stripe_column = stripeclm_cmdline;
        }
        if tabsize_cmdline != -1 {
            k.tabsize = tabsize_cmdline;
        }
        if operating_dir_cmdline.is_some() || k.isset(Flag::Restricted) {
            k.operating_dir = operating_dir_cmdline;
        }
        if !quotestr_cmdline.is_empty() {
            k.quotestr = quotestr_cmdline;
        }
        if !k.isset(Flag::NoWrap) {
            k.flags.set(Flag::BreakLongLines);
        }
        k.flags.0 |= flags_cmdline.0;
    }

    if let Some(m) = argon_memory {
        k.kdf.mem_kib = m;
    }
    if let Some(p) = argon_passes {
        k.kdf.passes = p;
    }

    if hardwrap == 0 {
        k.flags.unset(Flag::BreakLongLines);
    } else if hardwrap == 1 {
        k.flags.set(Flag::BreakLongLines);
    }

    if k.isset(Flag::BoldText) {
        k.hilite_attribute = Attr::BOLD;
    }

    if k.isset(Flag::Restricted) {
        k.flags.unset(Flag::MakeBackup);
        k.flags.unset(Flag::HistoryLog);
        k.flags.unset(Flag::PositionLog);
    }

    if k.isset(Flag::RawSequences) {
        k.flags.unset(Flag::UseMouse);
    }

    if k.isset(Flag::ModernBindings) {
        k.flags.unset(Flag::Preserve);
    }

    if k.isset(Flag::Zero) {
        k.flags.set(Flag::NoHelp);
    }

    if (k.isset(Flag::HistoryLog) || k.isset(Flag::PositionLog)) && !k.have_statedir() {
        k.flags.unset(Flag::HistoryLog);
        k.flags.unset(Flag::PositionLog);
    }
    if k.isset(Flag::PositionLog) {
        k.load_positions_register();
    }

    if k.backup_dir.is_some() && !k.isset(Flag::Restricted) {
        k.init_backup_dir();
    }
    if k.operating_dir.is_some() {
        k.init_operating_dir();
    }

    if k.punct.is_empty() {
        k.punct = b"!.?".to_vec();
    }
    if k.brackets.is_empty() {
        k.brackets = b"\"')>]}".to_vec();
    }
    if k.quotestr.is_empty() {
        k.quotestr = b"^([ \t]*([!#%:;>|}]|/{2}))+".to_vec();
    }

    match crate::regex::compile(&k.quotestr, crate::regex::REG_EXTENDED) {
        Ok(re) => k.quotereg = Some(re),
        Err(message) => {
            let quotestr = k.quotestr.clone();
            let m = fmtb!(
                "Ungültiger regulärer Zitatausdruck „%s“: %s\n",
                &quotestr,
                &message
            );
            k.die_bytes(&m);
        }
    }

    if k.matchbrackets.is_empty() {
        k.matchbrackets = b"(<[{)>]}".to_vec();
    }

    if k.whitespace.is_empty() {
        k.whitespace = "\u{00BB}\u{00B7}".as_bytes().to_vec();
        k.whitelen = [2, 2];
    }

    k.foretext.clear();

    k.last_search.clear();
    k.flags.unset(Flag::BackwardsSearch);

    if k.tabsize == -1 {
        k.tabsize = WIDTH_OF_TAB;
    }

    if k.term_has_colors {
        k.set_interface_colorpairs();
    } else {
        let hilite = k.hilite_attribute;
        k.interface_color_pair[Elem::TitleBar as usize] = hilite;
        k.interface_color_pair[Elem::LineNumber as usize] = hilite;
        k.interface_color_pair[Elem::GuideStripe as usize] = Attr::REVERSE;
        k.interface_color_pair[Elem::ScrollBar as usize] = Attr::NORMAL;
        k.interface_color_pair[Elem::SelectedText as usize] = hilite;
        k.interface_color_pair[Elem::Spotlighted as usize] = Attr::REVERSE;
        k.interface_color_pair[Elem::MiniInfobar as usize] = hilite;
        k.interface_color_pair[Elem::PromptBar as usize] = hilite;
        k.interface_color_pair[Elem::StatusBar as usize] = hilite;
        k.interface_color_pair[Elem::ErrorMessage as usize] = hilite;
        k.interface_color_pair[Elem::KeyCombo as usize] = hilite;
        k.interface_color_pair[Elem::FunctionTag as usize] = Attr::NORMAL;
    }

    k.terminal_init();

    k.window_init();
    k.term.curs_set(false);

    let (lines, cols) = (k.lines(), k.cols());
    k.sidebar = usize::from(k.isset(Flag::Indicator) && lines > 5 && cols > 9);
    k.bardata = vec![Attr::NORMAL; lines];
    k.editwincols = cols.saturating_sub(k.sidebar).max(1);

    set_up_signal_handlers();

    k.mouse_init();

    let operand_count = operands.len();
    let mut optind = 0;
    while optind < operand_count {
        let mut givenline: isize = 0;
        let mut givencol: isize = 0;
        let mut searchstring: Option<Vec<u8>> = None;

        if optind + 1 < operand_count && operands[optind].first() == Some(&b'+') {
            let arg = &operands[optind];
            let mut n = 1;
            while n < arg.len() && arg[n].is_ascii_alphabetic() {
                match arg[n] {
                    b'c' => k.flags.set(Flag::CaseSensitive),
                    b'C' => k.flags.unset(Flag::CaseSensitive),
                    b'r' => k.flags.set(Flag::UseRegexp),
                    b'R' => k.flags.unset(Flag::UseRegexp),
                    other => {
                        let m = fmt!("Ungültiger Suchmodifikator „%c“", char::from(other));
                        k.statusline(Msg::Alert, &m);
                    }
                }
                n += 1;
            }
            optind += 1;
            if n < arg.len() && (arg[n] == b'/' || arg[n] == b'?') {
                if n + 1 < arg.len() {
                    searchstring = Some(arg[n + 1..].to_vec());
                    if arg[n] == b'?' {
                        k.flags.set(Flag::BackwardsSearch);
                    }
                } else {
                    k.statusline(Msg::Alert, "Leere Suchzeichenkette");
                }
            } else if arg.len() == 1 {
                givenline = -1;
            } else if !parse_line_column(&arg[1..], &mut givenline, &mut givencol) {
                k.statusline(Msg::Alert, "Ungültige Zeilen- oder Spaltennummer");
            }
        }

        if operands[optind] == b"-" {
            optind += 1;
            if !k.scoop_stdin() {
                continue;
            }
        } else {
            let mut filename = operands[optind].clone();
            optind += 1;
            if k.isset(Flag::ColonParsing)
                && givenline == 0
                && filename.contains(&b':')
                && givencol == 0
                && crate::files::stat_info(&filename).is_none()
            {
                let mut coda = filename.len();
                loop {
                    loop {
                        coda = coda.saturating_sub(1);
                        if !(coda > 1 && filename[coda].is_ascii_digit()) {
                            break;
                        }
                    }
                    if filename[coda] == b':'
                        && filename.get(coda + 1).is_some_and(u8::is_ascii_digit)
                    {
                        if crate::files::stat_info(&filename[..coda]).is_none() {
                            if !filename[coda + 1..].contains(&b':') && coda > 0 {
                                continue;
                            }
                        } else if !parse_line_column(
                            &filename[coda + 1..],
                            &mut givenline,
                            &mut givencol,
                        ) {
                            k.die("Ungültige Zahl\n");
                        } else {
                            filename.truncate(coda);
                        }
                    }
                    break;
                }
            }
            if !k.open_buffer(&filename, true) {
                continue;
            }
        }

        if k.isset(Flag::PositionLog) && !k.ofile().filename.is_empty() {
            k.restore_cursor_position_if_any();
        }

        if givenline != 0 || givencol != 0 {
            let top = k.filetop();
            let of = k.ofile_mut();
            of.current = top;
            of.placewewant = 0;
            k.goto_line_and_column(givenline, givencol, true);
        } else if let Some(needle) = searchstring {
            let top = k.filetop();
            let of = k.ofile_mut();
            of.current = top;
            of.current_x = 0;
            if k.isset(Flag::UseRegexp) {
                k.regexp_init(&needle);
            }
            let backwards = k.isset(Flag::BackwardsSearch);
            if k.findnextstr(&needle, false, JUSTFIND, None, backwards, Some(top), 0) == 0 {
                k.not_found_msg(&needle);
            } else if k.lastmessage <= Msg::Remark {
                k.wipe_statusbar();
            }
            let pww = k.xplustabs();
            k.ofile_mut().placewewant = pww;
            k.adjust_viewport(Update::Centering);
            if k.isset(Flag::UseRegexp) {
                k.tidy_up_after_search();
            }
            k.last_search = needle;
        }
    }

    k.flags.unset(Flag::NoreadMode);

    if !isatty(0) {
        k.die("Standardeingabe ist kein Terminal\n");
    }

    if k.files.is_empty() {
        k.open_buffer(b"", true);
        k.flags.unset(Flag::ViewMode);
    } else {
        k.cur = k.next_buffer_index(k.cur);
        if k.more_than_one {
            k.mention_name_and_linecount();
        }
        if k.isset(Flag::ViewMode) {
            k.flags.set(Flag::NewBuffer);
        }
    }

    k.prepare_for_display();

    if let Some(problem) = k.startup_problem.clone() {
        k.statusline(Msg::Alert, &problem);
    }

    let help_not_rebound = k.first_sc_for(MMAIN, Func::Help).map(|s| s.keycode) == Some(0x07);
    if k.ofile().filename.is_empty()
        && k.ofile().totsize == 0
        && k.files.len() == 1
        && !k.isset(Flag::NoHelp)
        && help_not_rebound
    {
        k.statusbar("Willkommen bei Zu. Grundlegende Hilfe erhalten Sie mit Strg+G.");
    }

    k.margin = 12345;

    k.we_are_running = true;

    loop {
        k.confirm_margin();
        if k.on_a_vt && k.waiting_keycodes() == 0 {
            k.mute_modifiers = false;
        }
        if k.currmenu != MMAIN {
            k.bottombars(MMAIN);
        }

        let wanted = !k.isset(Flag::SoloSidescroll)
            && !k.isset(Flag::SoftWrap)
            && k.editwincols > 2 * CUSHION + 2;
        if k.united_sidescroll != wanted {
            k.united_sidescroll = wanted;
            k.refresh_needed = true;
        }

        if k.isset(Flag::MiniBar)
            && !k.isset(Flag::Zero)
            && k.lines() > 1
            && k.lastmessage < Msg::Remark
        {
            k.minibar();
        } else if k.isset(Flag::ConstantShow)
            && k.lastmessage == Msg::Vacuum
            && k.lines() > 1
            && !k.isset(Flag::Zero)
            && k.waiting_keycodes() == 0
        {
            k.report_cursor_position();
        }

        k.as_an_at = true;

        if k.current_x() == 0 && k.cur_data().starts_with(&[0xEF, 0xBB, 0xBF]) {
            k.statusline(Msg::Notice, "Bytereihenfolge-Markierung");
            k.set_blankdelay_to_one();
        }

        if (k.refresh_needed && k.lines() > 1) || (k.lines() == 1 && k.lastmessage <= Msg::Hush) {
            k.edit_refresh();
        } else {
            k.place_the_cursor();
        }

        if k.isset(Flag::Zero) && k.lastmessage > Msg::Hush {
            if k.ofile().cursor_row == i64_of(k.editwinrows) - 1 && k.lines() > 1 {
                k.edit_scroll(FORWARD);
                k.term.wnoutrefresh(Win::Mid);
            }
            k.term.wredrawln(Win::Foot, 0, 1);
            k.term.wnoutrefresh(Win::Foot);
            k.place_the_cursor();
        } else if k.isset(Flag::Zero) && k.lastmessage > Msg::Vacuum {
            k.term.wredrawln(Win::Mid, k.editwinrows - 1, 1);
        }

        k.final_status = 0;
        k.focusing = true;

        k.put_cursor_at_end_of_answer();

        k.process_a_keystroke();
    }
}

impl Zu {
    pub fn make_new_node(&mut self, prevnode: Option<Lp>) -> Lp {
        let lineno = match prevnode {
            Some(p) => self.nodes[p].lineno + 1,
            None => 1,
        };
        let node = LineNode {
            data: Vec::new(),
            lineno,
            next: None,
            prev: prevnode,
            has_anchor: false,
            alive: true,
        };
        if let Some(i) = self.free_nodes.pop() {
            self.nodes[i] = node;
            i
        } else {
            self.nodes.push(node);
            self.nodes.len() - 1
        }
    }

    pub fn splice_node(&mut self, afterthis: Lp, newnode: Lp) {
        let after_next = self.nodes[afterthis].next;
        self.nodes[newnode].next = after_next;
        self.nodes[newnode].prev = Some(afterthis);
        if let Some(n) = after_next {
            self.nodes[n].prev = Some(newnode);
        }
        self.nodes[afterthis].next = Some(newnode);
        if afterthis == self.ofile().filebot {
            self.ofile_mut().filebot = newnode;
        }
    }

    pub fn delete_node(&mut self, line: Lp) {
        if !self.files.is_empty() {
            if line == self.ofile().edittop {
                let p = self.nodes[line].prev;
                if let Some(p) = p {
                    self.ofile_mut().edittop = p;
                }
            }
            if self.ofile().spillage_line == Some(line) {
                self.ofile_mut().spillage_line = None;
            }
        }
        let n = &mut self.nodes[line];
        n.data.zeroize();
        n.data = Vec::new();
        n.next = None;
        n.prev = None;
        n.alive = false;
        self.free_nodes.push(line);
    }

    pub fn unlink_node(&mut self, line: Lp) {
        let (prev, next) = (self.nodes[line].prev, self.nodes[line].next);
        if let Some(p) = prev {
            self.nodes[p].next = next;
        }
        if let Some(n) = next {
            self.nodes[n].prev = prev;
        }
        if line == self.ofile().filebot
            && let Some(p) = prev
        {
            self.ofile_mut().filebot = p;
        }
        self.delete_node(line);
    }

    pub fn free_lines(&mut self, src: Option<Lp>) {
        let Some(mut src) = src else { return };
        while let Some(n) = self.nodes[src].next {
            let prev = src;
            src = n;
            self.delete_node(prev);
        }
        self.delete_node(src);
    }

    pub fn copy_node(&mut self, src: Lp) -> Lp {
        let data = self.nodes[src].data.clone();
        let lineno = self.nodes[src].lineno;
        let anchor = self.nodes[src].has_anchor;
        let dst = self.make_new_node(None);
        let n = &mut self.nodes[dst];
        n.data = data;
        n.lineno = lineno;
        n.has_anchor = anchor;
        dst
    }

    pub fn copy_buffer(&mut self, src: Lp) -> Lp {
        let head = self.copy_node(src);
        self.nodes[head].prev = None;
        let mut item = head;
        let mut s = self.nodes[src].next;
        while let Some(sn) = s {
            let c = self.copy_node(sn);
            self.nodes[item].next = Some(c);
            self.nodes[c].prev = Some(item);
            item = c;
            s = self.nodes[sn].next;
        }
        self.nodes[item].next = None;
        head
    }

    pub fn renumber_from(&mut self, line: Lp) {
        let mut number = match self.nodes[line].prev {
            None => 0,
            Some(p) => self.nodes[p].lineno,
        };
        let mut l = Some(line);
        while let Some(x) = l {
            number += 1;
            self.nodes[x].lineno = number;
            l = self.nodes[x].next;
        }
    }

    pub fn last_of(&self, mut line: Lp) -> Lp {
        while let Some(n) = self.nodes[line].next {
            line = n;
        }
        line
    }
}

impl Zu {
    pub fn in_restricted_mode(&mut self) -> bool {
        if self.isset(Flag::Restricted) {
            self.statusline(
                Msg::Ahem,
                "Im eingeschränkten Modus ist diese Funktion deaktiviert",
            );
            self.term.beep();
            true
        } else {
            false
        }
    }
}

impl Zu {
    pub fn print_view_warning(&mut self) {
        self.statusline(Msg::Ahem, "Taste unzulässig im Nur-Lese-Modus");
    }
}

impl Zu {
    pub fn inject(&mut self, burst: &mut [u8]) {
        let thisline = self.current();
        let count = burst.len();
        let (original_row, old_amount) = if self.isset(Flag::SoftWrap) {
            let row = if self.ofile().cursor_row == i64_of(self.editwinrows) - 1 {
                self.chunk_for(self.xplustabs(), thisline)
            } else {
                0
            };
            (row, self.extra_chunks_in(thisline))
        } else {
            (0, 0)
        };
        for b in burst.iter_mut() {
            if *b == 0 {
                *b = b'\n';
            }
        }
        let (cont_ok, tail_lineno, tail_x) = self
            .current_undo()
            .map_or((false, 0, 0), |u| (true, u.tail_lineno, u.tail_x));
        if self.ofile().last_action != UndoType::Add
            || !cont_ok
            || tail_lineno != self.lineno(thisline)
            || tail_x != self.current_x()
        {
            self.add_undo(UndoType::Add, None);
        }
        let cx = self.current_x();
        {
            let d = &mut self.nodes[thisline].data;
            let tail: Vec<u8> = d.split_off(cx.min(d.len()));
            d.extend_from_slice(burst);
            d.extend_from_slice(&tail);
        }
        if thisline == self.edittop() && self.ofile().firstcolumn > 0 {
            self.ensure_firstcolumn_is_aligned();
            self.refresh_needed = true;
        }
        {
            let of = self.ofile_mut();
            if of.mark == Some(thisline) && of.current_x < of.mark_x {
                of.mark_x += count;
            }
            of.current_x += count;
        }
        let n = mbstrlen(burst);
        self.ofile_mut().totsize = self.ofile().totsize.wrapping_add(n);
        self.set_modified();
        if thisline == self.filebot() && !self.isset(Flag::NoNewlines) {
            self.new_magicline();
            if self.margin > 0 && self.ofile().cursor_row < i64_of(self.editwinrows) - 1 {
                let n = self.next(thisline).unwrap();
                self.update_line(n, 0);
            }
        }
        self.update_undo(UndoType::Add);
        if self.isset(Flag::BreakLongLines) {
            self.do_wrap();
        }
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
        if self.united_sidescroll && pww > self.ofile().brink + self.editwincols - CUSHION - 1 {
            self.refresh_needed = true;
        }
        if self.isset(Flag::SoftWrap) {
            let c = self.current();
            if self.extra_chunks_in(c) != old_amount
                || (self.ofile().cursor_row == i64_of(self.editwinrows) - 1
                    && self.chunk_for(pww, c) > original_row)
            {
                self.refresh_needed = true;
                self.focusing = false;
            }
        }
        if !self.refresh_needed {
            let (c, x) = (self.current(), self.current_x());
            self.update_line(c, x);
        }
    }
}

const fn wanted_to_move(func: Func) -> bool {
    matches!(
        func,
        Func::Left
            | Func::Right
            | Func::Up
            | Func::Down
            | Func::Home
            | Func::End
            | Func::PrevWord
            | Func::NextWord
            | Func::ParaBegin
            | Func::ParaEnd
            | Func::PrevBlock
            | Func::NextBlock
            | Func::PageUp
            | Func::PageDown
            | Func::FirstLine
            | Func::LastLine
    )
}

#[must_use]
pub const fn changes_something(f: Func) -> bool {
    matches!(
        f,
        Func::Savefile
            | Func::Writeout
            | Func::Enter
            | Func::Tab
            | Func::Delete
            | Func::Backspace
            | Func::CutText
            | Func::PasteText
            | Func::ChopPrevWord
            | Func::ChopNextWord
            | Func::ZapText
            | Func::CutTillEof
            | Func::Execute
            | Func::Indent
            | Func::Unindent
            | Func::Justify
            | Func::FullJustify
            | Func::Comment
            | Func::Complete
            | Func::Replace
            | Func::Verbatim
            | Func::ChangePassword
    )
}
