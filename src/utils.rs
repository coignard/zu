use crate::chars::{
    is_alpha_char, mbrevstrcasestr, mbstrcasestr, mbstrlen, revstrstr, step_left, step_right,
};
use crate::definitions::{CUSHION, Flag, Lp, tail_bytes};
use crate::global::Zu;
use crate::regex;
use std::ffi::{CStr, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub fn tail(path: &[u8]) -> &[u8] {
    tail_bytes(path)
}

pub fn concatenate(path: &[u8], name: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(path.len() + name.len());
    v.extend_from_slice(path);
    v.extend_from_slice(name);
    v
}

pub const fn digits(n: i64) -> usize {
    if n < 100_000 {
        if n < 1000 {
            if n < 100 { 2 } else { 3 }
        } else if n < 10000 {
            4
        } else {
            5
        }
    } else if n < 10_000_000 {
        if n < 1_000_000 { 6 } else { 7 }
    } else if n < 100_000_000 {
        8
    } else {
        9
    }
}

pub fn parse_num(s: &[u8]) -> Option<isize> {
    let t = String::from_utf8_lossy(s);
    let t = t.trim_start();
    if t.is_empty() {
        return None;
    }
    t.parse::<isize>().ok()
}

pub fn parse_line_column(s: &[u8], line: &mut isize, column: &mut isize) -> bool {
    let mut s = s;
    while s.first() == Some(&b' ') {
        s = &s[1..];
    }
    let comma = s.iter().position(|&c| c == b',' || c == b'.' || c == b':');
    match comma {
        None => parse_num(s).is_some_and(|v| {
            *line = v;
            true
        }),
        Some(ci) => {
            let mut retval = parse_num(&s[ci + 1..]).is_some_and(|v| {
                *column = v;
                true
            });
            if ci == 0 {
                return retval;
            }
            retval = parse_num(&s[..ci]).is_some_and(|v| {
                *line = v;
                true
            }) && retval;
            retval
        }
    }
}

pub fn recode_nul_to_lf(s: &mut [u8]) {
    for b in s.iter_mut() {
        if *b == 0 {
            *b = b'\n';
        }
    }
}

pub fn recode_lf_to_nul(s: &mut [u8]) {
    for b in s.iter_mut() {
        if *b == b'\n' {
            *b = 0;
        }
    }
}

pub fn bytes_to_path(b: &[u8]) -> &Path {
    Path::new(OsStr::from_bytes(b))
}

pub fn path_to_bytes(p: &Path) -> Vec<u8> {
    p.as_os_str().as_bytes().to_vec()
}

pub fn strerror(errno: i32) -> String {
    crate::utils::errno_text(errno)
}

pub fn io_errstr(e: &std::io::Error) -> String {
    use std::io::ErrorKind;
    if let Some(n) = e.raw_os_error() {
        return strerror(n);
    }
    match e.kind() {
        ErrorKind::WriteZero => "Unvollständig geschrieben".to_string(),
        ErrorKind::UnexpectedEof => "Unerwartetes Dateiende".to_string(),
        ErrorKind::OutOfMemory => "Hauptspeicher erschöpft".to_string(),
        ErrorKind::InvalidData | ErrorKind::InvalidInput => "Ungültige Daten".to_string(),
        _ if e.get_ref().is_some() => e.to_string(),
        _ => "Ein-/Ausgabefehler".to_string(),
    }
}

pub fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

impl Zu {
    pub fn get_homedir(&mut self) {
        if self.homedir.is_some() {
            return;
        }
        let mut homenv: Option<Vec<u8>> = std::env::var_os("HOME").map(|v| v.as_bytes().to_vec());
        unsafe {
            if homenv.is_none() || libc::geteuid() == 0 {
                let pw = libc::getpwuid(libc::geteuid());
                if !pw.is_null() && !(*pw).pw_dir.is_null() {
                    homenv = Some(CStr::from_ptr((*pw).pw_dir).to_bytes().to_vec());
                }
            }
        }
        if let Some(h) = homenv
            && !h.is_empty()
        {
            self.homedir = Some(h);
        }
    }

    pub fn get_page_start(&self, column: usize) -> usize {
        if self.united_sidescroll {
            let brink = self.ofile().brink;
            if column < CUSHION {
                return 0;
            } else if column < brink + CUSHION {
                if self.flags.isset(Flag::JumpyScrolling) {
                    return column.saturating_sub(self.editwincols / 2);
                }
                return column - CUSHION;
            } else if column > brink + self.editwincols - CUSHION - 1 {
                return column
                    .wrapping_sub(self.editwincols)
                    .wrapping_add(if self.flags.isset(Flag::JumpyScrolling) {
                        self.editwincols / 2
                    } else {
                        CUSHION
                    })
                    .wrapping_add(1);
            }
            return brink;
        }
        if column == 0 || column + 2 < self.editwincols || self.flags.isset(Flag::SoftWrap) {
            0
        } else if self.editwincols > 8 {
            column - 6 - (column - 6) % (self.editwincols - 8)
        } else {
            column - (self.editwincols - 2)
        }
    }

    pub fn actual_x(&self, text: &[u8], column: usize) -> usize {
        let mut i = 0;
        let mut width = 0;
        while i < text.len() {
            let charlen = self.advance_over(&text[i..], &mut width);
            if width > column {
                break;
            }
            i += charlen;
        }
        i.min(text.len())
    }

    pub fn wideness(&self, text: &[u8], mut count: usize) -> usize {
        let mut width = 0;
        if count == 0 {
            return 0;
        }
        let mut i = 0;
        while i < text.len() {
            let charlen = self.advance_over(&text[i..], &mut width);
            if count <= charlen {
                break;
            }
            count -= charlen;
            i += charlen;
        }
        width
    }

    pub fn breadth(&self, text: &[u8]) -> usize {
        let mut span = 0;
        let mut i = 0;
        while i < text.len() {
            i += self.advance_over(&text[i..], &mut span);
        }
        span
    }

    pub fn xplustabs(&self) -> usize {
        let of = self.ofile();
        self.wideness(self.data(of.current), of.current_x)
    }

    pub fn new_magicline(&mut self) {
        let bot = self.ofile().filebot;
        let n = self.make_new_node(Some(bot));
        self.node_mut(n).data = Vec::new();
        self.node_mut(bot).next = Some(n);
        self.ofile_mut().filebot = n;
        self.ofile_mut().totsize = self.ofile().totsize.wrapping_add(1);
    }

    pub fn remove_magicline(&mut self) {
        let of = self.ofile();
        let bot = of.filebot;
        if self.data(bot).is_empty() && bot != of.filetop {
            let prev = self.node(bot).prev.unwrap();
            if self.ofile().current == bot {
                self.ofile_mut().current = prev;
            }
            self.ofile_mut().filebot = prev;
            self.delete_node(bot);
            self.node_mut(prev).next = None;
            self.ofile_mut().totsize = self.ofile().totsize.wrapping_sub(1);
        }
    }

    pub fn mark_is_before_cursor(&self) -> bool {
        let of = self.ofile();
        let mark = of.mark.unwrap();
        self.lineno(mark) < self.lineno(of.current)
            || (mark == of.current && of.mark_x <= of.current_x)
    }

    pub fn get_region(&self) -> (Lp, usize, Lp, usize) {
        let of = self.ofile();
        if self.mark_is_before_cursor() {
            (of.mark.unwrap(), of.mark_x, of.current, of.current_x)
        } else {
            (of.current, of.current_x, of.mark.unwrap(), of.mark_x)
        }
    }

    pub fn get_range(&mut self) -> (Lp, Lp) {
        let of = self.ofile();
        if of.mark.is_none() {
            return (of.current, of.current);
        }
        let (top, _top_x, mut bot, bot_x) = self.get_region();
        if bot_x == 0 && bot != top && !self.also_the_last {
            bot = self.node(bot).prev.unwrap();
        } else {
            self.also_the_last = true;
        }
        (top, bot)
    }

    pub fn line_from_number(&self, number: i64) -> Lp {
        let mut line = self.ofile().current;
        if self.lineno(line) > number {
            while self.lineno(line) != number {
                match self.node(line).prev {
                    Some(p) => line = p,
                    None => break,
                }
            }
        } else {
            while self.lineno(line) != number {
                match self.node(line).next {
                    Some(n) => line = n,
                    None => break,
                }
            }
        }
        line
    }

    pub fn number_of_characters_in(&self, begin: Lp, end: Lp) -> usize {
        let mut count = 0usize;
        let mut line = Some(begin);
        let stop = self.node(end).next;
        while line != stop {
            let Some(l) = line else { break };
            count += mbstrlen(self.data(l)) + 1;
            line = self.node(l).next;
        }
        count.saturating_sub(1)
    }
}

pub enum Arg<'a> {
    S(&'a str),
    B(&'a [u8]),
    I(i64),
    U(u64),
    C(char),
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(s: &'a str) -> Self {
        Arg::S(s)
    }
}
impl<'a> From<&'a String> for Arg<'a> {
    fn from(s: &'a String) -> Self {
        Arg::S(s.as_str())
    }
}
impl<'a> From<&'a [u8]> for Arg<'a> {
    fn from(s: &'a [u8]) -> Self {
        Arg::B(s)
    }
}
impl<'a> From<&'a Vec<u8>> for Arg<'a> {
    fn from(s: &'a Vec<u8>) -> Self {
        Arg::B(s.as_slice())
    }
}
impl From<i64> for Arg<'_> {
    fn from(v: i64) -> Self {
        Arg::I(v)
    }
}
impl From<i32> for Arg<'_> {
    fn from(v: i32) -> Self {
        Arg::I(i64::from(v))
    }
}
impl From<isize> for Arg<'_> {
    fn from(v: isize) -> Self {
        Arg::I(v as i64)
    }
}
impl From<usize> for Arg<'_> {
    fn from(v: usize) -> Self {
        Arg::U(v as u64)
    }
}
impl From<u64> for Arg<'_> {
    fn from(v: u64) -> Self {
        Arg::U(v)
    }
}
impl From<u32> for Arg<'_> {
    fn from(v: u32) -> Self {
        Arg::U(u64::from(v))
    }
}
impl From<char> for Arg<'_> {
    fn from(v: char) -> Self {
        Arg::C(v)
    }
}

const fn arg_int(a: &Arg) -> i64 {
    match a {
        Arg::I(v) => *v,
        Arg::U(v) => (*v).cast_signed(),
        Arg::C(c) => *c as i64,
        _ => 0,
    }
}

fn arg_bytes<'a>(a: &'a Arg<'a>, tmp: &'a mut Vec<u8>) -> &'a [u8] {
    match a {
        Arg::S(s) => s.as_bytes(),
        Arg::B(b) => b,
        Arg::I(v) => {
            tmp.extend_from_slice(v.to_string().as_bytes());
            tmp.as_slice()
        }
        Arg::U(v) => {
            tmp.extend_from_slice(v.to_string().as_bytes());
            tmp.as_slice()
        }
        Arg::C(c) => {
            let mut buf = [0u8; 4];
            tmp.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            tmp.as_slice()
        }
    }
}

pub fn sprintf_bytes(fmt: &str, args: &[Arg]) -> Vec<u8> {
    let f = fmt.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(fmt.len() + 32);
    let mut i = 0;
    let mut argno = 0;
    let next = |argno: &mut usize| -> Option<&Arg> {
        let a = args.get(*argno);
        *argno += 1;
        a
    };
    while i < f.len() {
        if f[i] != b'%' {
            out.push(f[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= f.len() {
            break;
        }
        if f[i] == b'%' {
            out.push(b'%');
            i += 1;
            continue;
        }
        let mut left = false;
        let mut zero = false;
        while i < f.len() && (f[i] == b'-' || f[i] == b'0' || f[i] == b' ' || f[i] == b'+') {
            if f[i] == b'-' {
                left = true;
            }
            if f[i] == b'0' {
                zero = true;
            }
            i += 1;
        }
        let mut width: Option<usize> = None;
        if i < f.len() && f[i] == b'*' {
            let w = next(&mut argno).map_or(0, arg_int);
            if w < 0 {
                left = true;
                width = Some(usize_of(-w));
            } else {
                width = Some(usize_of(w));
            }
            i += 1;
        } else {
            let mut w = 0usize;
            let mut any = false;
            while i < f.len() && f[i].is_ascii_digit() {
                w = w * 10 + (f[i] - b'0') as usize;
                i += 1;
                any = true;
            }
            if any {
                width = Some(w);
            }
        }
        let mut prec: Option<usize> = None;
        if i < f.len() && f[i] == b'.' {
            i += 1;
            if i < f.len() && f[i] == b'*' {
                let p = next(&mut argno).map_or(0, arg_int);
                prec = Some(if p < 0 { 0 } else { usize_of(p) });
                i += 1;
            } else {
                let mut p = 0usize;
                while i < f.len() && f[i].is_ascii_digit() {
                    p = p * 10 + (f[i] - b'0') as usize;
                    i += 1;
                }
                prec = Some(p);
            }
        }
        while i < f.len() && matches!(f[i], b'z' | b'l' | b'h' | b'j' | b't' | b'q' | b'L') {
            i += 1;
        }
        if i >= f.len() {
            break;
        }
        let conv = f[i];
        i += 1;
        let mut tmp = Vec::new();
        let body: Vec<u8> = match conv {
            b's' => {
                let a = next(&mut argno);
                let mut b: Vec<u8> = a.map_or_else(Vec::new, |a| arg_bytes(a, &mut tmp).to_vec());
                if let Some(p) = prec
                    && b.len() > p
                {
                    b.truncate(p);
                }
                b
            }
            b'c' => {
                let a = next(&mut argno);
                match a {
                    Some(Arg::C(c)) => {
                        let mut buf = [0u8; 4];
                        c.encode_utf8(&mut buf).as_bytes().to_vec()
                    }
                    Some(a) => {
                        let v = arg_int(a);
                        if (0..256).contains(&v) {
                            vec![u8_of(v)]
                        } else {
                            char::from_u32(i32_of(v).cast_unsigned())
                                .map_or_default(|c| c.to_string().into_bytes())
                        }
                    }
                    None => Vec::new(),
                }
            }
            b'd' | b'i' => {
                let v = next(&mut argno).map_or(0, arg_int);
                v.to_string().into_bytes()
            }
            b'u' => {
                let v = next(&mut argno).map_or(0, |a| match a {
                    Arg::I(v) => (*v).cast_unsigned(),
                    _ => arg_int(a).cast_unsigned(),
                });
                v.to_string().into_bytes()
            }
            b'x' => {
                let v = next(&mut argno).map_or(0, arg_int);
                format!("{:x}", v.cast_unsigned()).into_bytes()
            }
            b'X' => {
                let v = next(&mut argno).map_or(0, arg_int);
                format!("{:X}", v.cast_unsigned()).into_bytes()
            }
            _ => {
                out.push(b'%');
                out.push(conv);
                continue;
            }
        };
        let is_num = matches!(conv, b'd' | b'i' | b'u' | b'x' | b'X');
        let w = width.unwrap_or(0);
        let len = if is_num {
            body.len()
        } else {
            String::from_utf8_lossy(&body).chars().count()
        };
        if len >= w {
            out.extend_from_slice(&body);
        } else if left {
            out.extend_from_slice(&body);
            out.extend(std::iter::repeat_n(b' ', w - len));
        } else if zero && is_num {
            let (sign, digits) = if body.first() == Some(&b'-') {
                (&body[..1], &body[1..])
            } else {
                (&body[..0], &body[..])
            };
            out.extend_from_slice(sign);
            out.extend(std::iter::repeat_n(b'0', w - len));
            out.extend_from_slice(digits);
        } else {
            out.extend(std::iter::repeat_n(b' ', w - len));
            out.extend_from_slice(&body);
        }
    }
    out
}

pub fn sprintf(fmt: &str, args: &[Arg]) -> String {
    String::from_utf8_lossy(&sprintf_bytes(fmt, args)).into_owned()
}

pub const fn plural<'a>(singular: &'a str, pl: &'a str, n: u64) -> &'a str {
    if n == 1 { singular } else { pl }
}

#[macro_export]
macro_rules! fmt {
    ($f:expr) => { $crate::utils::sprintf($f, &[]) };
    ($f:expr, $($a:expr),+ $(,)?) => {
        $crate::utils::sprintf($f, &[$($crate::utils::Arg::from($a)),+])
    };
}

#[macro_export]
macro_rules! fmtb {
    ($f:expr) => { $crate::utils::sprintf_bytes($f, &[]) };
    ($f:expr, $($a:expr),+ $(,)?) => {
        $crate::utils::sprintf_bytes($f, &[$($crate::utils::Arg::from($a)),+])
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = write!(std::io::stderr(), $($arg)*);
    }};
}

const ERRNO_TEXTS: &[(i32, &str)] = &[
    (libc::EPERM, "Vorgang nicht zulässig"),
    (libc::ENOENT, "Datei oder Verzeichnis nicht gefunden"),
    (libc::ESRCH, "Kein passender Prozess gefunden"),
    (
        libc::EINTR,
        "Unterbrechung während des Betriebssystemaufrufs",
    ),
    (libc::EIO, "Eingabe-/Ausgabefehler"),
    (
        libc::ENXIO,
        "Kein passendes Gerät bzw. keine passende Adresse gefunden",
    ),
    (libc::E2BIG, "Die Argumentliste ist zu lang"),
    (libc::ENOEXEC, "Fehler im Format der Programmdatei"),
    (libc::EBADF, "Ungültiger Dateideskriptor"),
    (libc::ECHILD, "Keine Kind-Prozesse"),
    (libc::EAGAIN, "Die Ressource ist zur Zeit nicht verfügbar"),
    (libc::ENOMEM, "Nicht genügend Hauptspeicher verfügbar"),
    (libc::EACCES, "Keine Berechtigung"),
    (libc::EFAULT, "Ungültige Adresse"),
    (libc::ENOTBLK, "Es ist ein Block-Device notwendig"),
    (libc::EBUSY, "Das Gerät oder die Ressource ist belegt"),
    (libc::EEXIST, "Die Datei existiert bereits"),
    (libc::EXDEV, "Ungültiger Link über Gerätegrenzen hinweg"),
    (libc::ENODEV, "Kein passendes Gerät gefunden"),
    (libc::ENOTDIR, "Ist kein Verzeichnis"),
    (libc::EISDIR, "Ist ein Verzeichnis"),
    (libc::EINVAL, "Das Argument ist ungültig"),
    (libc::ENFILE, "Zu viele offene Dateien im System"),
    (libc::EMFILE, "Zu viele offene Dateien"),
    (
        libc::ENOTTY,
        "Unpassender IOCTL (I/O-Control) für das Gerät",
    ),
    (
        libc::ETXTBSY,
        "Das Programm kann nicht ausgeführt oder verändert werden (busy)",
    ),
    (libc::EFBIG, "Die Datei ist zu groß"),
    (
        libc::ENOSPC,
        "Auf dem Gerät ist kein Speicherplatz mehr verfügbar",
    ),
    (libc::ESPIPE, "Nicht erlaubter Seek"),
    (libc::EROFS, "Das Dateisystem ist nur lesbar"),
    (libc::EMLINK, "Zu viele Links"),
    (libc::EPIPE, "Datenübergabe unterbrochen (broken pipe)"),
    (
        libc::EDOM,
        "Das numerische Argument ist außerhalb des Definitionsbereiches",
    ),
    (
        libc::ERANGE,
        "Das numerische Ergebnis ist außerhalb des gültigen Bereiches",
    ),
    (
        libc::EDEADLK,
        "Verklemmung beim Zugriff auf eine Ressource vermieden",
    ),
    (libc::ENAMETOOLONG, "Der Dateiname ist zu lang"),
    (libc::ENOLCK, "Keine Sperren verfügbar"),
    (
        libc::ENOSYS,
        "Die angeforderte Funktion ist nicht implementiert",
    ),
    (libc::ENOTEMPTY, "Das Verzeichnis ist nicht leer"),
    (libc::ELOOP, "Zu viele Ebenen aus symbolischen Links"),
    (libc::ENOMSG, "Keine Nachricht des gewünschten Typs"),
    (libc::EIDRM, "Bezeichner wurde entfernt"),
    #[cfg(target_os = "linux")]
    (
        libc::ECHRNG,
        "Die Kanalnummer ist außerhalb des gültigen Bereiches",
    ),
    #[cfg(target_os = "linux")]
    (libc::EL2NSYNC, "Level 2 ist nicht synchronisiert"),
    #[cfg(target_os = "linux")]
    (libc::EL3HLT, "Level 3 angehalten"),
    #[cfg(target_os = "linux")]
    (libc::EL3RST, "Level 3 zurückgesetzt"),
    #[cfg(target_os = "linux")]
    (
        libc::ELNRNG,
        "Die Link-Nummer ist außerhalb des gültigen Bereiches",
    ),
    #[cfg(target_os = "linux")]
    (libc::EUNATCH, "Das Protokoll ist nicht verfügbar"),
    #[cfg(target_os = "linux")]
    (libc::ENOCSI, "Keine »CSI«-Struktur verfügbar"),
    #[cfg(target_os = "linux")]
    (libc::EL2HLT, "Level 2 angehalten"),
    #[cfg(target_os = "linux")]
    (libc::EBADE, "Ungültiger Austausch"),
    #[cfg(target_os = "linux")]
    (libc::EBADR, "Ungültiger Aufruf-Deskriptor"),
    #[cfg(target_os = "linux")]
    (libc::EXFULL, "Vermittlung ist überfüllt"),
    #[cfg(target_os = "linux")]
    (libc::ENOANO, "Keine Anode"),
    #[cfg(target_os = "linux")]
    (libc::EBADRQC, "Ungültiger Aufruf-Code"),
    #[cfg(target_os = "linux")]
    (libc::EBADSLT, "Ungültiger Slot"),
    #[cfg(target_os = "linux")]
    (libc::EBFONT, "Ungültiges Font-Dateiformat"),
    (libc::ENOSTR, "Das Gerät ist kein Stream"),
    (libc::ENODATA, "Keine Daten verfügbar"),
    (libc::ETIME, "Der virtuelle Zeitnehmer ist abgelaufen"),
    (libc::ENOSR, "Keine Stream-Ressourcen mehr verfügbar"),
    #[cfg(target_os = "linux")]
    (
        libc::ENONET,
        "Die Maschine ist nicht an das Netzwerk angeschlossen",
    ),
    #[cfg(target_os = "linux")]
    (libc::ENOPKG, "Das Zusatzpacket ist nicht installiert"),
    (libc::EREMOTE, "Das Objekt ist remote"),
    (libc::ENOLINK, "Der Link wurde beschädigt"),
    #[cfg(target_os = "linux")]
    (libc::EADV, "Konflikt mit Bekanntmachung"),
    #[cfg(target_os = "linux")]
    (libc::ESRMNT, "»Srmount«-Fehler"),
    #[cfg(target_os = "linux")]
    (libc::ECOMM, "Kommunikationsfehler beim Senden"),
    (libc::EPROTO, "Protokollfehler"),
    (libc::EMULTIHOP, "Ein »Multihop« wurde versucht"),
    #[cfg(target_os = "linux")]
    (libc::EDOTDOT, "RFS-spezifischer Fehler"),
    (libc::EBADMSG, "Ungültige Nachricht"),
    (
        libc::EOVERFLOW,
        "Der Wert ist zu groß für den definierten Datentyp",
    ),
    #[cfg(target_os = "linux")]
    (libc::ENOTUNIQ, "Der Name ist im Netzwerk nicht eindeutig"),
    #[cfg(target_os = "linux")]
    (
        libc::EBADFD,
        "Die Dateizugriffsnummer ist in schlechter Verfassung",
    ),
    #[cfg(target_os = "linux")]
    (
        libc::EREMCHG,
        "Die Adresse der Gegenstelle hat sich geändert",
    ),
    #[cfg(target_os = "linux")]
    (
        libc::ELIBACC,
        "Auf eine benötigte Shared Library kann nicht zugegriffen werden",
    ),
    #[cfg(target_os = "linux")]
    (
        libc::ELIBBAD,
        "Zugriff auf eine fehlerhafte oder defekte Shared Library",
    ),
    #[cfg(target_os = "linux")]
    (
        libc::ELIBSCN,
        "».lib«-Sektion in der »a.out«-Datei ist beschädigt",
    ),
    #[cfg(target_os = "linux")]
    (
        libc::ELIBMAX,
        "Versuch zu viele Shared Libraries einzubinden",
    ),
    #[cfg(target_os = "linux")]
    (
        libc::ELIBEXEC,
        "Eine Shared Library kann nicht direkt ausgeführt werden",
    ),
    (
        libc::EILSEQ,
        "Ungültiges oder unvollständiges Multi-Byte- oder Wide-Zeichen",
    ),
    #[cfg(target_os = "linux")]
    (
        libc::ERESTART,
        "Der unterbrochene Betriebssystemaufruf sollte neu gestartet werden",
    ),
    #[cfg(target_os = "linux")]
    (libc::ESTRPIPE, "Fehler in Stream-Pipe"),
    (libc::EUSERS, "Zu viele Benutzer"),
    (libc::ENOTSOCK, "Socket-Vorgang an einem nicht-Socket"),
    (libc::EDESTADDRREQ, "Es ist eine Zieladresse notwendig"),
    (libc::EMSGSIZE, "Die Nachricht ist zu lang"),
    (libc::EPROTOTYPE, "Das Protokoll passt nicht zu dem Socket"),
    (libc::ENOPROTOOPT, "Das Protokoll ist nicht verfügbar"),
    (
        libc::EPROTONOSUPPORT,
        "Das Protokoll wird nicht unterstützt",
    ),
    (libc::ESOCKTNOSUPPORT, "Socket-Typ wird nicht unterstützt"),
    (libc::ENOTSUP, "Vorgang wird nicht unterstützt"),
    (
        libc::EPFNOSUPPORT,
        "Die Protokollfamilie wird nicht unterstützt",
    ),
    (
        libc::EAFNOSUPPORT,
        "Die Adressfamilie wird von der Protokollfamilie nicht unterstützt",
    ),
    (libc::EADDRINUSE, "Die Adresse wird bereits verwendet"),
    (
        libc::EADDRNOTAVAIL,
        "Die angeforderte Adresse kann nicht zugewiesen werden",
    ),
    (libc::ENETDOWN, "Das Netzwerk ist nicht aktiv"),
    (libc::ENETUNREACH, "Das Netzwerk ist nicht erreichbar"),
    (
        libc::ENETRESET,
        "Das Netzwerk hat die Verbindung nach einem Reset verloren",
    ),
    (
        libc::ECONNABORTED,
        "Das Programm verursachte den Abbruch der Verbindung",
    ),
    (
        libc::ECONNRESET,
        "Die Verbindung wurde vom Kommunikationspartner zurückgesetzt",
    ),
    (libc::ENOBUFS, "Kein Hauptspeicher für den Puffer verfügbar"),
    (libc::EISCONN, "Der Socket ist bereits verbunden"),
    (libc::ENOTCONN, "Der Socket ist nicht verbunden"),
    (
        libc::ESHUTDOWN,
        "Ein Senden nach dem Beenden des Sockets ist nicht möglich",
    ),
    (libc::ETOOMANYREFS, "Zu viele Referenzen: can't splice"),
    (
        libc::ETIMEDOUT,
        "Die Wartezeit für die Verbindung ist abgelaufen",
    ),
    (libc::ECONNREFUSED, "Verbindungsaufbau abgelehnt"),
    (libc::EHOSTDOWN, "Der Rechner ist nicht aktiv"),
    (libc::EHOSTUNREACH, "Keine Route zum Zielrechner"),
    (libc::EALREADY, "Vorgang wird bereits ausgeführt"),
    (libc::EINPROGRESS, "Vorgang ist jetzt in Bearbeitung"),
    (libc::ESTALE, "Veraltete Dateizugriffsnummer (file handle)"),
    #[cfg(target_os = "linux")]
    (libc::EUCLEAN, "Die Struktur muss bereinigt werden"),
    #[cfg(target_os = "linux")]
    (libc::ENOTNAM, "Keine XENIX »named type« Datei"),
    #[cfg(target_os = "linux")]
    (libc::ENAVAIL, "Keine XENIX-Semaphoren verfügbar"),
    #[cfg(target_os = "linux")]
    (libc::EISNAM, "Ist eine »named type file«"),
    #[cfg(target_os = "linux")]
    (
        libc::EREMOTEIO,
        "Ein-/Ausgabefehler der Gegenstelle (remote)",
    ),
    (
        libc::EDQUOT,
        "Der zugewiesene Plattenplatz (Quota) ist überschritten",
    ),
    #[cfg(target_os = "linux")]
    (libc::ENOMEDIUM, "Kein Medium gefunden"),
    #[cfg(target_os = "linux")]
    (libc::EMEDIUMTYPE, "Falscher Medien-Typ"),
    (libc::ECANCELED, "Vorgang abgebrochen"),
    #[cfg(target_os = "linux")]
    (libc::ENOKEY, "Der notwendige Schlüssel ist nicht verfügbar"),
    #[cfg(target_os = "linux")]
    (libc::EKEYEXPIRED, "Der Schlüssel ist nicht mehr gültig"),
    #[cfg(target_os = "linux")]
    (libc::EKEYREVOKED, "Der Schlüssel wurde widerrufen"),
    #[cfg(target_os = "linux")]
    (
        libc::EKEYREJECTED,
        "Der SChlüssel wurde von Dienst zurückgewiesen",
    ),
    (libc::EOWNERDEAD, "Der Eigentümer-Prozess wurde beendet"),
    (
        libc::ENOTRECOVERABLE,
        "Der Status ist nicht wiederherstellbar",
    ),
    #[cfg(target_os = "linux")]
    (libc::ERFKILL, "Vorgang ist nicht möglich wegen RF-kill"),
    #[cfg(target_os = "linux")]
    (libc::EHWPOISON, "Memory-Seite hat einen Hardware-Fehler"),
    #[cfg(target_vendor = "apple")]
    (libc::EPROCLIM, "Zu viele Prozesse"),
    #[cfg(target_vendor = "apple")]
    (libc::EBADRPC, "Ungültige RPC-Struktur"),
    #[cfg(target_vendor = "apple")]
    (libc::ERPCMISMATCH, "Falsche RPC-Version"),
    #[cfg(target_vendor = "apple")]
    (libc::EPROGUNAVAIL, "RPC-Programm nicht verfügbar"),
    #[cfg(target_vendor = "apple")]
    (libc::EPROGMISMATCH, "Falsche Programmversion"),
    #[cfg(target_vendor = "apple")]
    (libc::EPROCUNAVAIL, "Ungültige Prozedur für das Programm"),
    #[cfg(target_vendor = "apple")]
    (
        libc::EFTYPE,
        "Unpassender Dateityp oder unpassendes Dateiformat",
    ),
    #[cfg(target_vendor = "apple")]
    (libc::EAUTH, "Authentifizierungsfehler"),
    #[cfg(target_vendor = "apple")]
    (libc::ENEEDAUTH, "Authentifizierung erforderlich"),
    #[cfg(target_vendor = "apple")]
    (libc::EPWROFF, "Gerät ausgeschaltet"),
    #[cfg(target_vendor = "apple")]
    (libc::EDEVERR, "Gerätefehler"),
    #[cfg(target_vendor = "apple")]
    (
        libc::EBADEXEC,
        "Ungültige ausführbare Datei oder Bibliothek",
    ),
    #[cfg(target_vendor = "apple")]
    (libc::EBADARCH, "Falscher Prozessortyp"),
    #[cfg(target_vendor = "apple")]
    (libc::ESHLIBVERS, "Falsche Bibliotheksversion"),
    #[cfg(target_vendor = "apple")]
    (libc::EBADMACHO, "Fehlerhafte Mach-O-Datei"),
    #[cfg(target_vendor = "apple")]
    (libc::ENOATTR, "Attribut nicht gefunden"),
    #[cfg(target_vendor = "apple")]
    (libc::EOPNOTSUPP, "Vom Socket nicht unterstützt"),
    #[cfg(target_vendor = "apple")]
    (libc::ENOPOLICY, "Richtlinie nicht gefunden"),
    #[cfg(target_vendor = "apple")]
    (libc::EQFULL, "Ausgabewarteschlange voll"),
    #[cfg(target_vendor = "apple")]
    (libc::ENOTCAPABLE, "Unzureichende Berechtigungen"),
];

pub fn errno_text(errno: i32) -> String {
    match ERRNO_TEXTS.iter().find(|(n, _)| *n == errno) {
        Some((_, text)) => text.to_string(),
        None => format!("Unbekannter Fehler {errno}"),
    }
}

impl Zu {}

impl Zu {
    pub fn strstrwrapper(&mut self, haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
        if self.isset(Flag::UseRegexp) {
            let re = self.search_regexp.take()?;

            let result = if self.isset(Flag::BackwardsSearch) {
                let far_end = haystack.len();
                let ceiling = start;
                let mut floor = 0usize;
                let mut next_rung = 0usize;
                if !regex::exec(&re, haystack, 0, far_end, &mut self.regmatches[..1], 0) {
                    self.search_regexp = Some(re);
                    return None;
                }
                let mut last_find = usize_of(crate::regex::offset(self.regmatches[0].rm_so));
                if last_find > ceiling {
                    self.search_regexp = Some(re);
                    return None;
                }
                while (usize_of(crate::regex::offset(self.regmatches[0].rm_so))) <= ceiling {
                    floor = next_rung;
                    last_find = usize_of(crate::regex::offset(self.regmatches[0].rm_so));
                    if last_find == ceiling {
                        break;
                    }
                    next_rung = step_right(haystack, last_find);
                    if !regex::exec(
                        &re,
                        haystack,
                        next_rung,
                        far_end,
                        &mut self.regmatches[..1],
                        0,
                    ) {
                        break;
                    }
                }
                if !regex::exec(&re, haystack, floor, far_end, &mut self.regmatches, 0) {
                    self.search_regexp = Some(re);
                    return None;
                }
                Some(usize_of(crate::regex::offset(self.regmatches[0].rm_so)))
            } else if regex::exec(
                &re,
                haystack,
                start,
                haystack.len(),
                &mut self.regmatches,
                0,
            ) {
                Some(usize_of(crate::regex::offset(self.regmatches[0].rm_so)))
            } else {
                None
            };
            self.search_regexp = Some(re);
            return result;
        }
        if self.isset(Flag::CaseSensitive) {
            if self.isset(Flag::BackwardsSearch) {
                return revstrstr(haystack, needle, start);
            }
            return haystack[start..]
                .windows(needle.len().max(1))
                .position(|w| w == needle)
                .map(|p| p + start)
                .or(if needle.is_empty() { Some(start) } else { None });
        }
        if self.isset(Flag::BackwardsSearch) {
            mbrevstrcasestr(haystack, needle, start)
        } else {
            mbstrcasestr(&haystack[start..], needle).map(|p| p + start)
        }
    }
}

#[cfg(target_pointer_width = "64")]
pub const fn usize_of(n: i64) -> usize {
    usize::from_le_bytes(n.to_le_bytes())
}

#[cfg(target_pointer_width = "32")]
pub const fn usize_of(n: i64) -> usize {
    let b = n.to_le_bytes();
    usize::from_le_bytes([b[0], b[1], b[2], b[3]])
}

#[cfg(target_pointer_width = "64")]
pub const fn i64_of(n: usize) -> i64 {
    i64::from_le_bytes(n.to_le_bytes())
}

#[cfg(target_pointer_width = "32")]
pub const fn i64_of(n: usize) -> i64 {
    i64::from(u32::from_le_bytes(n.to_le_bytes()))
}

#[cfg(target_pointer_width = "64")]
pub const fn isize_of(n: i64) -> isize {
    isize::from_le_bytes(n.to_le_bytes())
}

#[cfg(target_pointer_width = "32")]
pub const fn isize_of(n: i64) -> isize {
    let b = n.to_le_bytes();
    isize::from_le_bytes([b[0], b[1], b[2], b[3]])
}

pub const fn i32_of(n: i64) -> i32 {
    let b = n.to_le_bytes();
    i32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

pub const fn i16_of(n: i64) -> i16 {
    let b = n.to_le_bytes();
    i16::from_le_bytes([b[0], b[1]])
}

pub const fn u8_of(n: i64) -> u8 {
    n.to_le_bytes()[0]
}

pub fn is_separate_word(position: usize, length: usize, text: &[u8]) -> bool {
    let before = step_left(text, position);
    let after = position + length;
    (position == 0 || !is_alpha_char(&text[before..]))
        && (after >= text.len() || !is_alpha_char(&text[after..]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn conversions_like_as() {
        assert_eq!(usize_of(-1), usize::MAX);
        assert_eq!(usize_of(7), 7);
        assert_eq!(i64_of(usize::MAX), -1);
        assert_eq!(i64_of(42), 42);
        assert_eq!(isize_of(-5), -5);
        assert_eq!(i32_of(0x1_0000_0001), 1);
        assert_eq!(i32_of(-2), -2);
        assert_eq!(i32_of(0xFFFF_FFFF), -1);
        assert_eq!(i16_of(0x1_2345), 0x2345);
        assert_eq!(i16_of(-1), -1);
        assert_eq!(u8_of(-1), 255);
        assert_eq!(u8_of(0x1FF), 255);
        assert_eq!(u8_of(65), b'A');
    }

    use super::*;

    #[test]
    fn system_errors_are_german() {
        assert_eq!(strerror(libc::EACCES), "Keine Berechtigung");
        assert_eq!(
            strerror(libc::ENOSPC),
            "Auf dem Gerät ist kein Speicherplatz mehr verfügbar"
        );
        assert_eq!(
            strerror(libc::ENOENT),
            "Datei oder Verzeichnis nicht gefunden"
        );
        assert_eq!(strerror(9999), "Unbekannter Fehler 9999");
        #[cfg(target_os = "linux")]
        let last = 133;
        #[cfg(target_vendor = "apple")]
        let last = libc::ELAST;
        for n in (1..=last).filter(|n| cfg!(not(target_os = "linux")) || ![41, 58].contains(n)) {
            let t = strerror(n);
            assert!(
                !t.starts_with("Unbekannter Fehler") && !t.contains("Permission denied"),
                "errno {n}: {t}"
            );
        }
    }

    #[test]
    fn io_errors_without_errno_are_german() {
        let e = std::io::Error::from(std::io::ErrorKind::WriteZero);
        assert_eq!(io_errstr(&e), "Unvollständig geschrieben");
        let e = std::io::Error::from_raw_os_error(libc::EROFS);
        assert_eq!(io_errstr(&e), "Das Dateisystem ist nur lesbar");
        let e = std::io::Error::other("Eigene Meldung");
        assert_eq!(io_errstr(&e), "Eigene Meldung");
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn regex_errors_are_german() {
        assert_eq!(
            crate::regex::regex_text("Unmatched ( or \\("),
            "Kein Gegenstück für ( oder \\("
        );
        assert_eq!(
            crate::regex::compile(b"a\\(", 0).err().as_deref(),
            Some("Kein Gegenstück für ( oder \\(")
        );
        assert_eq!(
            crate::regex::compile(b"a[", 0).err().as_deref(),
            Some("Ungültiger regulärer Ausdruck")
        );
        assert_eq!(
            crate::regex::compile(b"[[:foo:]]", 0).err().as_deref(),
            Some("Ungültiger Zeichenklassenname")
        );
    }
}
