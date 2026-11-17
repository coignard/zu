use crate::utils::{u8_of, usize_of};
use std::path::{Path, PathBuf};

use crate::definitions::{
    ALT_DELETE, ALT_DOWN, ALT_END, ALT_HOME, ALT_INSERT, ALT_LEFT, ALT_PAGEDOWN, ALT_PAGEUP,
    ALT_RIGHT, ALT_UP, CONTROL_DELETE, CONTROL_DOWN, CONTROL_END, CONTROL_HOME, CONTROL_LEFT,
    CONTROL_RIGHT, CONTROL_SHIFT_DELETE, CONTROL_UP, END_OF_PASTE, IGNORED_KEY, KEY_BACKSPACE,
    KEY_CANCEL, KEY_DC, KEY_DOWN, KEY_END, KEY_ENTER, KEY_F, KEY_HOME, KEY_IC, KEY_LEFT, KEY_MOUSE,
    KEY_NPAGE, KEY_PPAGE, KEY_RIGHT, KEY_SIC, KEY_UP, SHIFT_ALT_DOWN, SHIFT_ALT_LEFT,
    SHIFT_ALT_RIGHT, SHIFT_ALT_UP, SHIFT_CONTROL_DOWN, SHIFT_CONTROL_END, SHIFT_CONTROL_HOME,
    SHIFT_CONTROL_LEFT, SHIFT_CONTROL_RIGHT, SHIFT_CONTROL_UP, SHIFT_DELETE, SHIFT_DOWN, SHIFT_END,
    SHIFT_HOME, SHIFT_LEFT, SHIFT_PAGEDOWN, SHIFT_PAGEUP, SHIFT_RIGHT, SHIFT_TAB, SHIFT_UP,
    START_OF_PASTE, UNBOUND_KEY,
};

const BOOLNAMES: [&str; 44] = [
    "bw", "am", "xsb", "xhp", "xenl", "eo", "gn", "hc", "km", "hs", "in", "da", "db", "mir",
    "msgr", "os", "eslok", "xt", "hz", "ul", "xon", "nxon", "mc5i", "chts", "nrrmc", "npc",
    "ndscr", "ccc", "bce", "hls", "xhpa", "crxm", "daisy", "xvpa", "sam", "cpix", "lpix", "OTbs",
    "OTns", "OTnc", "OTMT", "OTNL", "OTpt", "OTxr",
];

const NUMCOUNT: usize = 39;
const MAX_ENTRY_SIZE1: usize = 4096;
const MAX_ENTRY_SIZE2: usize = 32768;

const STRNAMES: [&str; 414] = [
    "cbt", "bel", "cr", "csr", "tbc", "clear", "el", "ed", "hpa", "cmdch", "cup", "cud1", "home",
    "civis", "cub1", "mrcup", "cnorm", "cuf1", "ll", "cuu1", "cvvis", "dch1", "dl1", "dsl", "hd",
    "smacs", "blink", "bold", "smcup", "smdc", "dim", "smir", "invis", "prot", "rev", "smso",
    "smul", "ech", "rmacs", "sgr0", "rmcup", "rmdc", "rmir", "rmso", "rmul", "flash", "ff", "fsl",
    "is1", "is2", "is3", "if", "ich1", "il1", "ip", "kbs", "ktbc", "kclr", "kctab", "kdch1",
    "kdl1", "kcud1", "krmir", "kel", "ked", "kf0", "kf1", "kf10", "kf2", "kf3", "kf4", "kf5",
    "kf6", "kf7", "kf8", "kf9", "khome", "kich1", "kil1", "kcub1", "kll", "knp", "kpp", "kcuf1",
    "kind", "kri", "khts", "kcuu1", "rmkx", "smkx", "lf0", "lf1", "lf10", "lf2", "lf3", "lf4",
    "lf5", "lf6", "lf7", "lf8", "lf9", "rmm", "smm", "nel", "pad", "dch", "dl", "cud", "ich",
    "indn", "il", "cub", "cuf", "rin", "cuu", "pfkey", "pfloc", "pfx", "mc0", "mc4", "mc5", "rep",
    "rs1", "rs2", "rs3", "rf", "rc", "vpa", "sc", "ind", "ri", "sgr", "hts", "wind", "ht", "tsl",
    "uc", "hu", "iprog", "ka1", "ka3", "kb2", "kc1", "kc3", "mc5p", "rmp", "acsc", "pln", "kcbt",
    "smxon", "rmxon", "smam", "rmam", "xonc", "xoffc", "enacs", "smln", "rmln", "kbeg", "kcan",
    "kclo", "kcmd", "kcpy", "kcrt", "kend", "kent", "kext", "kfnd", "khlp", "kmrk", "kmsg", "kmov",
    "knxt", "kopn", "kopt", "kprv", "kprt", "krdo", "kref", "krfr", "krpl", "krst", "kres", "ksav",
    "kspd", "kund", "kBEG", "kCAN", "kCMD", "kCPY", "kCRT", "kDC", "kDL", "kslt", "kEND", "kEOL",
    "kEXT", "kFND", "kHLP", "kHOM", "kIC", "kLFT", "kMSG", "kMOV", "kNXT", "kOPT", "kPRV", "kPRT",
    "kRDO", "kRPL", "kRIT", "kRES", "kSAV", "kSPD", "kUND", "rfi", "kf11", "kf12", "kf13", "kf14",
    "kf15", "kf16", "kf17", "kf18", "kf19", "kf20", "kf21", "kf22", "kf23", "kf24", "kf25", "kf26",
    "kf27", "kf28", "kf29", "kf30", "kf31", "kf32", "kf33", "kf34", "kf35", "kf36", "kf37", "kf38",
    "kf39", "kf40", "kf41", "kf42", "kf43", "kf44", "kf45", "kf46", "kf47", "kf48", "kf49", "kf50",
    "kf51", "kf52", "kf53", "kf54", "kf55", "kf56", "kf57", "kf58", "kf59", "kf60", "kf61", "kf62",
    "kf63", "el1", "mgc", "smgl", "smgr", "fln", "sclk", "dclk", "rmclk", "cwin", "wingo", "hup",
    "dial", "qdial", "tone", "pulse", "hook", "pause", "wait", "u0", "u1", "u2", "u3", "u4", "u5",
    "u6", "u7", "u8", "u9", "op", "oc", "initc", "initp", "scp", "setf", "setb", "cpi", "lpi",
    "chr", "cvr", "defc", "swidm", "sdrfq", "sitm", "slm", "smicm", "snlq", "snrmq", "sshm",
    "ssubm", "ssupm", "sum", "rwidm", "ritm", "rlm", "rmicm", "rshm", "rsubm", "rsupm", "rum",
    "mhpa", "mcud1", "mcub1", "mcuf1", "mvpa", "mcuu1", "porder", "mcud", "mcub", "mcuf", "mcuu",
    "scs", "smgb", "smgbp", "smglp", "smgrp", "smgt", "smgtp", "sbim", "scsd", "rbim", "rcsd",
    "subcs", "supcs", "docr", "zerom", "csnm", "kmous", "minfo", "reqmp", "getm", "setaf", "setab",
    "pfxl", "devt", "csin", "s0ds", "s1ds", "s2ds", "s3ds", "smglr", "smgtb", "birep", "binel",
    "bicr", "colornm", "defbi", "endbi", "setcolor", "slines", "dispc", "smpch", "rmpch", "smsc",
    "rmsc", "pctrm", "scesc", "scesa", "ehhlm", "elhlm", "elohlm", "erhlm", "ethlm", "evhlm",
    "sgr1", "slength", "OTi2", "OTrs", "OTnl", "OTbc", "OTko", "OTma", "OTG2", "OTG3", "OTG1",
    "OTG4", "OTGR", "OTGL", "OTGU", "OTGD", "OTGH", "OTGV", "OTGC", "meml", "memu", "box1",
];

pub struct TermInfo {
    names: String,
    booleans: Vec<bool>,
    numbers: Vec<i32>,
    strings: Vec<Option<Vec<u8>>>,
    extended_numbers: Vec<(String, i32)>,
    extended: Vec<(String, Vec<u8>)>,
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(t) = std::env::var_os("TERMINFO") {
        dirs.push(PathBuf::from(t));
    }
    if let Some(h) = std::env::var_os("HOME") {
        dirs.push(Path::new(&h).join(".terminfo"));
    }
    let defaults = [
        "/etc/terminfo",
        "/lib/terminfo",
        "/usr/share/terminfo",
        "/usr/lib/terminfo",
        "/usr/share/lib/terminfo",
    ];
    if let Some(list) = std::env::var_os("TERMINFO_DIRS") {
        for part in list.to_string_lossy().split(':') {
            if part.is_empty() {
                dirs.extend(defaults.iter().map(PathBuf::from));
            } else {
                dirs.push(PathBuf::from(part));
            }
        }
    }
    dirs.extend(defaults.iter().map(PathBuf::from));
    dirs
}

fn le16(d: &[u8], at: usize) -> Option<i32> {
    Some(i32::from(i16::from_le_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
    ])))
}

fn cstr(table: &[u8], at: usize) -> Option<Vec<u8>> {
    let rest = table.get(at..)?;
    let end = rest.iter().position(|&b| b == 0)?;
    Some(rest[..end].to_vec())
}

impl TermInfo {
    pub fn load(term: &str) -> Option<Self> {
        if term.is_empty() || term.contains('/') || term.starts_with('.') {
            return None;
        }
        let first = term.chars().next()?;
        for dir in search_dirs() {
            for sub in [first.to_string(), format!("{:02x}", first as u32)] {
                let path = dir.join(&sub).join(term);
                if let Ok(data) = std::fs::read(&path)
                    && let Some(ti) = Self::parse(&data)
                {
                    return Some(ti);
                }
            }
        }
        None
    }

    pub fn parse(d: &[u8]) -> Option<Self> {
        let magic = le16(d, 0)?;
        let (numsize, max_entry_size) = match magic {
            0o432 => (2, MAX_ENTRY_SIZE1),
            0o1036 => (4, MAX_ENTRY_SIZE2),
            _ => return None,
        };
        let count = |at: usize, limit: usize| {
            le16(d, at).and_then(|n| usize::try_from(n).ok().filter(|&n| n <= limit))
        };
        let names_size = count(2, usize::MAX)?;
        let bool_count = count(4, BOOLNAMES.len())?;
        let num_count = count(6, NUMCOUNT)?;
        let str_count = count(8, STRNAMES.len())?;
        let strtab_size = count(10, usize::MAX)?;
        if str_count * 2 >= max_entry_size {
            return None;
        }
        let names = d.get(12..12 + names_size)?;
        let names =
            String::from_utf8_lossy(names.split(|&b| b == 0).next().unwrap_or(&[])).into_owned();
        let booleans = d
            .get(12 + names_size..12 + names_size + bool_count)?
            .iter()
            .map(|&b| b != 0)
            .collect();
        let mut pos = 12 + names_size + bool_count;
        if pos % 2 == 1 {
            pos += 1;
        }
        let mut numbers = Vec::with_capacity(num_count);
        for i in 0..num_count {
            let at = pos + i * numsize;
            let value = if numsize == 2 {
                le16(d, at)?
            } else {
                i32::from_le_bytes([
                    *d.get(at)?,
                    *d.get(at + 1)?,
                    *d.get(at + 2)?,
                    *d.get(at + 3)?,
                ])
            };
            numbers.push(if value < 0 { -1 } else { value });
        }
        pos += num_count * numsize;
        let offsets_at = pos;
        pos += str_count * 2;
        let table = d.get(pos..pos + strtab_size)?;
        let mut strings = vec![None; STRNAMES.len()];
        for (i, slot) in strings.iter_mut().enumerate().take(str_count) {
            let off = le16(d, offsets_at + 2 * i)?;
            if off >= 0 {
                *slot = cstr(table, usize_of(i64::from(off)));
            }
        }
        pos += strtab_size;
        if pos % 2 == 1 {
            pos += 1;
        }
        let mut extended = Vec::new();
        let mut extended_numbers = Vec::new();
        if d.len() >= pos + 10 {
            let header: Vec<usize> = (0..5)
                .map(|i| le16(d, pos + 2 * i).and_then(|n| usize::try_from(n).ok()))
                .collect::<Option<_>>()?;
            let (ext_bools, ext_nums, ext_strs, ext_usage, ext_table_size) =
                (header[0], header[1], header[2], header[3], header[4]);
            if ext_bools + ext_nums + ext_strs >= max_entry_size / 2
                || ext_usage >= max_entry_size
                || ext_table_size >= max_entry_size
            {
                return None;
            }
            let mut p = pos + 10 + ext_bools;
            if p % 2 == 1 {
                p += 1;
            }
            let nums_at = p;
            p += ext_nums * numsize;
            let str_offsets_at = p;
            p += ext_strs * 2;
            let name_offsets_at = p;
            let name_count = ext_bools + ext_nums + ext_strs;
            p += name_count * 2;
            let ext_table = d.get(p..p + ext_table_size)?;
            let mut values = Vec::with_capacity(ext_strs);
            let mut names_base = 0usize;
            for i in 0..ext_strs {
                let off = le16(d, str_offsets_at + 2 * i)?;
                if off >= 0 {
                    let v = cstr(ext_table, usize_of(i64::from(off)))?;
                    names_base = names_base.max(usize_of(i64::from(off)) + v.len() + 1);
                    values.push(Some(v));
                } else {
                    values.push(None);
                }
            }
            let names_table = ext_table.get(names_base..)?;
            for (i, value) in values.into_iter().enumerate() {
                let off = le16(d, name_offsets_at + 2 * (ext_bools + ext_nums + i))?;
                if off < 0 {
                    continue;
                }
                let name = String::from_utf8_lossy(&cstr(names_table, usize_of(i64::from(off)))?)
                    .into_owned();
                if let Some(v) = value {
                    extended.push((name, v));
                }
            }
            for i in 0..ext_nums {
                let at = nums_at + i * numsize;
                let value = if numsize == 2 {
                    le16(d, at)?
                } else {
                    i32::from_le_bytes([
                        *d.get(at)?,
                        *d.get(at + 1)?,
                        *d.get(at + 2)?,
                        *d.get(at + 3)?,
                    ])
                };
                let off = le16(d, name_offsets_at + 2 * (ext_bools + i))?;
                if off < 0 || value < 0 {
                    continue;
                }
                let name = String::from_utf8_lossy(&cstr(names_table, usize_of(i64::from(off)))?)
                    .into_owned();
                extended_numbers.push((name, value));
            }
        }
        Some(Self {
            names,
            booleans,
            numbers,
            strings,
            extended_numbers,
            extended,
        })
    }

    pub fn flag(&self, name: &str) -> bool {
        BOOLNAMES
            .iter()
            .position(|&n| n == name)
            .and_then(|i| self.booleans.get(i).copied())
            .unwrap_or(false)
    }

    pub fn padding_baud_rate(&self) -> i32 {
        self.numbers.get(5).copied().unwrap_or(-1)
    }

    pub fn lines(&self) -> i32 {
        self.numbers.get(2).copied().unwrap_or(-1)
    }

    pub fn cols(&self) -> i32 {
        self.numbers.first().copied().unwrap_or(-1)
    }

    pub fn colors(&self) -> i32 {
        self.numbers.get(13).copied().unwrap_or(-1)
    }

    pub fn has_colors(&self) -> bool {
        let pairs = self.numbers.get(14).copied().unwrap_or(-1);
        let has = |n: &str| self.get(n).is_some();
        self.colors() >= 0
            && pairs >= 0
            && ((has("setaf") && has("setab")) || (has("setf") && has("setb")) || has("scp"))
    }

    pub fn names(&self) -> &str {
        &self.names
    }

    pub fn extended_number(&self, name: &str) -> Option<i32> {
        self.extended_numbers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        if let Some(i) = STRNAMES.iter().position(|&n| n == name) {
            return self.strings[i].as_deref();
        }
        self.extended
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_slice())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadInfo {
    pub xon_xoff: bool,
    pub padding_baud_rate: i32,
    pub no_pad_char: bool,
    pub pad_char: u8,
    pub baudrate: i32,
    pub no_padding: bool,
}

impl PadInfo {
    pub fn new(ti: &TermInfo, baudrate: i32, no_padding: bool) -> Self {
        Self {
            xon_xoff: ti.flag("xon"),
            padding_baud_rate: ti.padding_baud_rate(),
            no_pad_char: ti.flag("npc"),
            pad_char: ti.get("pad").and_then(|p| p.first().copied()).unwrap_or(0),
            baudrate,
            no_padding,
        }
    }

    fn delay_output(&self, out: &mut Vec<u8>, ms: i32) {
        if self.no_pad_char {
            crate::crypto::raw_write(1, out);
            zeroize::Zeroize::zeroize(out);
            std::thread::sleep(std::time::Duration::from_millis(
                u64::try_from(ms).unwrap_or(0),
            ));
        } else {
            let nullcount = i64::from(ms) * i64::from(self.baudrate) / (BAUDBYTE * 1000);
            let nullcount = usize::try_from(nullcount.clamp(0, MAX_NULLS)).unwrap_or(0);
            out.extend(std::iter::repeat_n(self.pad_char, nullcount));
        }
    }
}

const BAUDBYTE: i64 = 9;
const MAX_NULLS: i64 = 1 << 20;

pub fn tputs(out: &mut Vec<u8>, cap: &[u8], always_delay: bool, pad: &PadInfo) {
    let normal_delay = !pad.xon_xoff
        && pad.padding_baud_rate != 0
        && !pad.no_padding
        && pad.baudrate >= pad.padding_baud_rate;
    let at = |i: usize| cap.get(i).copied().unwrap_or(0);
    let mut i = 0;
    while at(i) != 0 {
        if at(i) == b'$' {
            i += 1;
            if at(i) == b'<' {
                i += 1;
                if (!at(i).is_ascii_digit() && at(i) != b'.') || !cap[i..].contains(&b'>') {
                    out.extend_from_slice(b"$<");
                    continue;
                }
                let mut number: i32 = 0;
                while at(i).is_ascii_digit() {
                    number = number
                        .saturating_mul(10)
                        .saturating_add(i32::from(at(i) - b'0'));
                    i += 1;
                }
                number = number.saturating_mul(10);
                if at(i) == b'.' {
                    i += 1;
                    if at(i).is_ascii_digit() {
                        number = number.saturating_add(i32::from(at(i) - b'0'));
                        i += 1;
                    }
                    while at(i).is_ascii_digit() {
                        i += 1;
                    }
                }
                let mut mandatory = false;
                while at(i) == b'*' || at(i) == b'/' {
                    mandatory |= at(i) == b'/';
                    i += 1;
                }
                if number > 0 && (always_delay || normal_delay || mandatory) {
                    pad.delay_output(out, number / 10);
                }
            } else {
                out.push(b'$');
                if at(i) != 0 {
                    out.push(at(i));
                }
            }
        } else {
            out.push(at(i));
        }
        if at(i) == 0 {
            break;
        }
        i += 1;
    }
}

const STACKSIZE: usize = 20;
const NUM_PARM: usize = 9;
const MAX_FORMAT_VALUE: usize = 10000;

fn parse_format(cap: &[u8], mut s: usize) -> (usize, Vec<u8>) {
    let at = |i: usize| cap.get(i).copied().unwrap_or(0);
    let mut format = vec![b'%'];
    let mut allowminus = false;
    let mut dot = false;
    let mut err = false;
    let mut value = 0usize;
    while at(s) != 0 {
        match at(s) {
            b'c' | b'd' | b'o' | b'x' | b'X' | b's' => {
                format.push(at(s));
                break;
            }
            b'.' => {
                format.push(b'.');
                s += 1;
                err |= dot;
                dot = true;
                value = 0;
            }
            b'#' | b' ' => {
                format.push(at(s));
                s += 1;
            }
            b':' => {
                s += 1;
                allowminus = true;
            }
            b'-' if allowminus => {
                format.push(b'-');
                s += 1;
            }
            c if c.is_ascii_digit() => {
                value = value
                    .saturating_mul(10)
                    .saturating_add(usize::from(c - b'0'));
                err |= value > MAX_FORMAT_VALUE;
                format.push(c);
                s += 1;
            }
            _ => break,
        }
    }
    if err {
        format = vec![b'%', at(s)];
    }
    (s, format)
}

fn printf_one(format: &[u8], conversion: u8, value: i32) -> Vec<u8> {
    let mut k = 1;
    let (mut left, mut space, mut alternate, mut zero) = (false, false, false, false);
    while let Some(&f) = format.get(k).filter(|f| b"-+ #0".contains(f)) {
        match f {
            b'-' => left = true,
            b' ' => space = true,
            b'#' => alternate = true,
            b'0' => zero = true,
            _ => {}
        }
        k += 1;
    }
    let number = |k: &mut usize| {
        let mut n = 0usize;
        while let Some(&d) = format.get(*k).filter(|d| d.is_ascii_digit()) {
            n = n.saturating_mul(10).saturating_add(usize::from(d - b'0'));
            *k += 1;
        }
        n
    };
    let width = number(&mut k);
    let precision = if format.get(k) == Some(&b'.') {
        k += 1;
        Some(number(&mut k))
    } else {
        None
    };
    if format.get(k) != Some(&conversion) || k + 1 != format.len() {
        return format.to_vec();
    }
    let mut body = if conversion == b's' {
        Vec::new()
    } else {
        let mut digits = match conversion {
            b'd' => i64::from(value).unsigned_abs().to_string(),
            b'o' => format!("{:o}", value.cast_unsigned()),
            b'x' => format!("{:x}", value.cast_unsigned()),
            _ => format!("{:X}", value.cast_unsigned()),
        }
        .into_bytes();
        if precision == Some(0) && value == 0 {
            digits.clear();
        }
        while digits.len() < precision.unwrap_or(1) {
            digits.insert(0, b'0');
        }
        if conversion == b'o' && alternate && digits.first() != Some(&b'0') {
            digits.insert(0, b'0');
        }
        let mut sign = Vec::new();
        if conversion == b'd' {
            if value < 0 {
                sign.push(b'-');
            } else if space {
                sign.push(b' ');
            }
        } else if alternate && value != 0 && conversion != b'o' {
            sign.extend_from_slice(if conversion == b'x' { b"0x" } else { b"0X" });
        }
        if zero && !left && precision.is_none() {
            while sign.len() + digits.len() < width {
                digits.insert(0, b'0');
            }
        }
        sign.extend_from_slice(&digits);
        sign
    };
    while body.len() < width {
        if left {
            body.push(b' ');
        } else {
            body.insert(0, b' ');
        }
    }
    body
}

pub fn tparm(cap: &[u8], params: &[i32]) -> Vec<u8> {
    let at = |i: usize| cap.get(i).copied().unwrap_or(0);
    let mut out = Vec::new();
    let mut stack: Vec<i32> = Vec::new();
    let mut dynamic = [0i32; 26];
    let mut statics = [0i32; 26];
    let mut param = [0i32; NUM_PARM];
    for (slot, value) in param.iter_mut().zip(params.iter()) {
        *slot = *value;
    }
    let mut incremented_two = false;
    let pop_num = |stack: &mut Vec<i32>| stack.pop().unwrap_or(0);
    let push_num = |stack: &mut Vec<i32>, value: i32| {
        if stack.len() < STACKSIZE {
            stack.push(value);
        }
    };
    let mut i = 0;
    while i < cap.len() {
        let c = cap[i];
        i += 1;
        if c != b'%' {
            out.push(c);
            continue;
        }
        let (next, format) = parse_format(cap, i);
        i = next;
        let op = at(i);
        if op == 0 {
            break;
        }
        i += 1;
        match op {
            b'%' => out.push(b'%'),
            b'd' | b'o' | b'x' | b'X' => {
                let value = pop_num(&mut stack);
                out.extend_from_slice(&printf_one(&format, op, value));
            }
            b'c' => {
                let value = pop_num(&mut stack);
                out.push(if value == 0 {
                    0o200
                } else {
                    u8_of(i64::from(value))
                });
            }
            b'l' => {
                stack.pop();
                push_num(&mut stack, 0);
            }
            b's' => {
                stack.pop();
                out.extend_from_slice(&printf_one(&format, op, 0));
            }
            b'p' => {
                let d = at(i);
                i += 1;
                if (b'1'..=b'9').contains(&d) {
                    push_num(&mut stack, param[usize::from(d - b'1')]);
                }
            }
            b'P' | b'g' => {
                let v = at(i);
                i += 1;
                let slot = match v {
                    b'a'..=b'z' => Some(&mut dynamic[usize::from(v - b'a')]),
                    b'A'..=b'Z' => Some(&mut statics[usize::from(v - b'A')]),
                    _ => None,
                };
                if let Some(slot) = slot {
                    if op == b'P' {
                        *slot = pop_num(&mut stack);
                    } else {
                        push_num(&mut stack, *slot);
                    }
                }
            }
            b'\'' => {
                push_num(&mut stack, i32::from(at(i)));
                i += 2;
            }
            b'{' => {
                let mut value: i32 = 0;
                while at(i).is_ascii_digit() {
                    value = value.wrapping_mul(10).wrapping_add(i32::from(at(i) - b'0'));
                    i += 1;
                }
                push_num(&mut stack, value);
                i += 1;
            }
            b'+' | b'-' | b'*' | b'/' | b'm' | b'&' | b'|' | b'^' | b'=' | b'<' | b'>' | b'A'
            | b'O' => {
                let rhs = pop_num(&mut stack);
                let lhs = pop_num(&mut stack);
                let result = match op {
                    b'+' => lhs.wrapping_add(rhs),
                    b'-' => lhs.wrapping_sub(rhs),
                    b'*' => lhs.wrapping_mul(rhs),
                    b'/' => lhs.checked_div(rhs).unwrap_or(0),
                    b'm' => lhs.checked_rem(rhs).unwrap_or(0),
                    b'&' => lhs & rhs,
                    b'|' => lhs | rhs,
                    b'^' => lhs ^ rhs,
                    b'=' => i32::from(lhs == rhs),
                    b'<' => i32::from(lhs < rhs),
                    b'>' => i32::from(lhs > rhs),
                    b'A' => i32::from(lhs != 0 && rhs != 0),
                    _ => i32::from(lhs != 0 || rhs != 0),
                };
                push_num(&mut stack, result);
            }
            b'!' => {
                let value = pop_num(&mut stack);
                push_num(&mut stack, i32::from(value == 0));
            }
            b'~' => {
                let value = pop_num(&mut stack);
                push_num(&mut stack, !value);
            }
            b'i' => {
                if !incremented_two {
                    incremented_two = true;
                    param[0] = param[0].wrapping_add(1);
                    param[1] = param[1].wrapping_add(1);
                }
            }
            b't' => {
                if pop_num(&mut stack) == 0 {
                    let mut level = 0;
                    while i < cap.len() {
                        if cap[i] == b'%' && i + 1 < cap.len() {
                            let o = cap[i + 1];
                            i += 2;
                            match o {
                                b'?' => level += 1,
                                b';' if level > 0 => level -= 1,
                                b';' => break,
                                b'e' if level == 0 => break,
                                _ => {}
                            }
                        } else {
                            i += 1;
                        }
                    }
                }
            }
            b'e' => {
                let mut level = 0;
                while i < cap.len() {
                    if cap[i] == b'%' && i + 1 < cap.len() {
                        let o = cap[i + 1];
                        i += 2;
                        match o {
                            b'?' => level += 1,
                            b';' if level > 0 => level -= 1,
                            b';' => break,
                            _ => {}
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[derive(Default)]
pub struct KeyTable {
    entries: Vec<(Vec<u8>, i32)>,
}

pub enum KeyMatch {
    Key(usize, i32),
    Partial,
    None,
}

fn meaning(name: &str) -> i32 {
    if let Some(n) = name.strip_prefix("kf")
        && let Ok(n) = n.parse::<i32>()
        && (0..=63).contains(&n)
    {
        return KEY_F(n);
    }
    match name {
        "kcuu1" => KEY_UP,
        "kcud1" => KEY_DOWN,
        "kcub1" => KEY_LEFT,
        "kcuf1" => KEY_RIGHT,
        "khome" | "ka1" => KEY_HOME,
        "kend" | "kc1" => KEY_END,
        "kpp" | "ka3" => KEY_PPAGE,
        "knp" | "kc3" => KEY_NPAGE,
        "kich1" => KEY_IC,
        "kdch1" => KEY_DC,
        "kbs" => KEY_BACKSPACE,
        "kent" => KEY_ENTER,
        "kcbt" => SHIFT_TAB,
        "kDC" => SHIFT_DELETE,
        "kIC" => KEY_SIC,
        "kLFT" => SHIFT_LEFT,
        "kRIT" => SHIFT_RIGHT,
        "kri" | "kUP" => SHIFT_UP,
        "kind" | "kDN" => SHIFT_DOWN,
        "kHOM" => SHIFT_HOME,
        "kEND" => SHIFT_END,
        "kPRV" => SHIFT_PAGEUP,
        "kNXT" => SHIFT_PAGEDOWN,
        "kel" | "kEND5" => CONTROL_END,
        "kcan" | "kCAN" => KEY_CANCEL,
        "kspd" | "kSPD" => 0x1A,
        "kb2" | "kbeg" | "kBEG" => IGNORED_KEY,
        "kmous" => KEY_MOUSE,
        "kLFT5" => CONTROL_LEFT,
        "kRIT5" => CONTROL_RIGHT,
        "kUP5" => CONTROL_UP,
        "kDN5" => CONTROL_DOWN,
        "kHOM5" => CONTROL_HOME,
        "kDC5" => CONTROL_DELETE,
        "kDC6" => CONTROL_SHIFT_DELETE,
        "kLFT6" => SHIFT_CONTROL_LEFT,
        "kRIT6" => SHIFT_CONTROL_RIGHT,
        "kUP6" => SHIFT_CONTROL_UP,
        "kDN6" => SHIFT_CONTROL_DOWN,
        "kHOM6" => SHIFT_CONTROL_HOME,
        "kEND6" => SHIFT_CONTROL_END,
        "kLFT3" => ALT_LEFT,
        "kRIT3" => ALT_RIGHT,
        "kUP3" => ALT_UP,
        "kDN3" => ALT_DOWN,
        "kHOM3" => ALT_HOME,
        "kEND3" => ALT_END,
        "kPRV3" => ALT_PAGEUP,
        "kNXT3" => ALT_PAGEDOWN,
        "kIC3" => ALT_INSERT,
        "kDC3" => ALT_DELETE,
        "kLFT4" => SHIFT_ALT_LEFT,
        "kRIT4" => SHIFT_ALT_RIGHT,
        "kUP4" => SHIFT_ALT_UP,
        "kDN4" => SHIFT_ALT_DOWN,
        "kpDIV" => '/' as i32,
        "kpMUL" => '*' as i32,
        "kpSUB" => '-' as i32,
        "kpADD" => '+' as i32,
        _ => UNBOUND_KEY,
    }
}

const NANO_CHAIN: [&str; 30] = [
    "kLFT5", "kRIT5", "kUP5", "kDN5", "kHOM5", "kEND5", "kDC5", "kDC6", "kUP", "kDN", "kLFT6",
    "kRIT6", "kUP6", "kDN6", "kHOM6", "kEND6", "kLFT3", "kRIT3", "kUP3", "kDN3", "kHOM3", "kEND3",
    "kPRV3", "kNXT3", "kIC3", "kDC3", "kLFT4", "kRIT4", "kUP4", "kDN4",
];

impl KeyTable {
    pub fn build(ti: &TermInfo) -> Self {
        let mut entries: Vec<(Vec<u8>, String)> = Vec::new();
        for (i, name) in STRNAMES.iter().enumerate() {
            if name.starts_with('k')
                && let Some(Some(v)) = ti.strings.get(i)
            {
                if v.is_empty() {
                    continue;
                }
                if let Some(e) = entries.iter_mut().find(|(s, _)| s == v) {
                    e.1 = name.to_string();
                } else {
                    entries.push((v.clone(), name.to_string()));
                }
            }
        }
        for (name, v) in &ti.extended {
            if name.starts_with('k') && !v.is_empty() && !entries.iter().any(|(s, _)| s == v) {
                entries.push((v.clone(), name.clone()));
            }
        }
        let mut table: Vec<(Vec<u8>, i32)> = entries
            .iter()
            .map(|(s, n)| (s.clone(), meaning(n)))
            .collect();
        for name in NANO_CHAIN.iter().rev() {
            if let Some(v) = ti.get(name)
                && let Some(e) = table.iter_mut().find(|(s, _)| s.as_slice() == v)
            {
                e.1 = meaning(name);
            }
        }
        for (seq, code) in [
            (b"\x1B[200~".to_vec(), START_OF_PASTE),
            (b"\x1B[201~".to_vec(), END_OF_PASTE),
        ] {
            if !table.iter().any(|(s, _)| *s == seq) {
                table.push((seq, code));
            }
        }
        Self { entries: table }
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn add(&mut self, seq: &[u8], code: i32) {
        if seq.is_empty() {
            return;
        }
        if let Some(e) = self.entries.iter_mut().find(|(s, _)| s.as_slice() == seq) {
            e.1 = code;
        } else {
            self.entries.push((seq.to_vec(), code));
        }
    }

    pub fn lookup(&self, input: &[u8]) -> KeyMatch {
        let mut partial = false;
        let mut best: Option<(usize, i32)> = None;
        for (seq, code) in &self.entries {
            if input.len() >= seq.len() {
                if input.starts_with(seq) && best.is_none_or(|(l, _)| seq.len() < l) {
                    best = Some((seq.len(), *code));
                }
            } else if seq.starts_with(input) {
                partial = true;
            }
        }
        match best {
            Some((len, code)) => KeyMatch::Key(len, code),
            None if partial => KeyMatch::Partial,
            None => KeyMatch::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn xterm_entry_is_read() {
        let Some(ti) = TermInfo::load("xterm-256color") else {
            return;
        };
        assert_eq!(ti.get("kcuu1"), Some(&b"\x1BOA"[..]));
        assert_eq!(ti.get("smkx"), Some(&b"\x1B[?1h\x1B="[..]));
        assert_eq!(ti.get("kUP5"), Some(&b"\x1B[1;5A"[..]));
        assert_eq!(ti.colors(), 256);
        assert!(ti.has_colors());
        let table = KeyTable::build(&ti);
        assert!(matches!(
            table.lookup(b"\x1B[1;5A"),
            KeyMatch::Key(6, CONTROL_UP)
        ));
        assert!(matches!(
            table.lookup(b"\x1BOE"),
            KeyMatch::Key(3, IGNORED_KEY)
        ));
        assert!(matches!(
            table.lookup(b"\x1B[2;2~"),
            KeyMatch::Key(6, KEY_SIC)
        ));
        assert!(matches!(
            table.lookup(b"\x1B[3;4~"),
            KeyMatch::Key(6, UNBOUND_KEY)
        ));
        assert!(matches!(table.lookup(b"\x1B[1;"), KeyMatch::Partial));
        assert!(matches!(table.lookup(b"\x1Bx"), KeyMatch::None));
        assert!(matches!(
            table.lookup(b"\x1B[<"),
            KeyMatch::Key(3, KEY_MOUSE)
        ));
        assert!(ti.names().starts_with("xterm-256color|"));
        assert_eq!(
            ti.get("XM"),
            Some(&b"\x1B[?1006;1000%?%p1%{1}%=%th%el%;"[..])
        );
    }

    fn padded(cap: &[u8], always_delay: bool, pad: &PadInfo) -> Vec<u8> {
        let mut out = Vec::new();
        tputs(&mut out, cap, always_delay, pad);
        out
    }

    #[test]
    fn padding_like_tputs() {
        let xon = PadInfo {
            xon_xoff: true,
            padding_baud_rate: -1,
            baudrate: 38400,
            ..PadInfo::default()
        };
        let plain = PadInfo {
            xon_xoff: false,
            ..xon
        };
        assert_eq!(padded(b"\x1B[H$<5>", false, &xon), b"\x1B[H");
        assert_eq!(
            padded(b"\x1B[H$<5>", false, &plain),
            [&b"\x1B[H"[..], &[0; 21]].concat()
        );
        let quiet = PadInfo {
            no_padding: true,
            ..plain
        };
        assert_eq!(padded(b"a$<5>b", false, &quiet), b"ab");
        assert_eq!(
            padded(b"a$<5/>b", false, &quiet),
            [&b"a"[..], &[0; 21], b"b"].concat()
        );
        let flash = b"\x1B[?5h$<100/>\x1B[?5l";
        assert_eq!(
            padded(flash, true, &xon),
            [&b"\x1B[?5h"[..], &[0; 426], b"\x1B[?5l"].concat()
        );
        assert_eq!(
            padded(b"a$<2.57*/>b", false, &xon).len(),
            2 + 2 * 38400 / 9000
        );
        assert_eq!(padded(b"$<x>$y$", false, &xon), b"$<x>$y$");
        assert_eq!(padded(b"$<5", false, &xon), b"$<5");
        assert_eq!(padded(b"a$<0/>b", false, &xon), b"ab");
        let star = PadInfo {
            pad_char: b'*',
            ..xon
        };
        assert_eq!(padded(b"$<1/>", false, &star), b"****");
        assert_eq!(
            padded(b"$<99999999999999/>", false, &PadInfo::default()),
            b""
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn boolean_capabilities_are_read() {
        let Some(ti) = TermInfo::load("xterm-256color") else {
            return;
        };
        assert!(ti.flag("am"));
        assert!(ti.flag("npc"));
        assert!(!ti.flag("xon"));
        assert!(!ti.flag("gibt-es-nicht"));
        assert_eq!(ti.padding_baud_rate(), -1);
        let pad = PadInfo::new(&ti, 38400, false);
        assert!(pad.no_pad_char && !pad.xon_xoff);
        if let Some(vt) = TermInfo::load("vt100") {
            assert!(vt.flag("xon"));
            assert!(!vt.flag("npc"));
        }
    }

    struct Noise(u64);

    impl Noise {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            usize::try_from(self.next() % u64::try_from(n.max(1)).unwrap()).unwrap()
        }

        fn byte(&mut self) -> u8 {
            self.next().to_le_bytes()[0]
        }
    }

    #[test]
    fn corrupt_entries_never_panic() {
        let original = search_dirs()
            .into_iter()
            .flat_map(|dir| [dir.join("x"), dir.join("78")])
            .find_map(|dir| std::fs::read(dir.join("xterm-256color")).ok());
        let Some(original) = original else {
            return;
        };
        let mut noise = Noise(0x2545_F491_4F6C_DD1D);
        for round in 0..if cfg!(miri) { 30 } else { 3000 } {
            let mut data = original.clone();
            for _ in 0..=noise.below(8) {
                let at = if round % 3 == 0 {
                    noise.below(12)
                } else {
                    noise.below(data.len())
                };
                data[at] = noise.byte();
            }
            if round % 5 == 0 {
                let keep = noise.below(data.len() + 1);
                data.truncate(keep);
            }
            if let Some(ti) = TermInfo::parse(&data) {
                let table = KeyTable::build(&ti);
                let _ = table.lookup(b"\x1B[1;5A");
                let _ = (ti.colors(), ti.has_colors(), ti.flag("npc"), ti.get("XM"));
                let _ = PadInfo::new(&ti, 38400, false);
            }
        }
    }

    #[test]
    fn arbitrary_capabilities_never_panic() {
        let alphabet = b"%pPgdcsoxX{}'+-*/m&|^=<>!~AOi?te;:#.0123456789$<>* \x1B[";
        let pads = [
            PadInfo::default(),
            PadInfo {
                no_pad_char: false,
                baudrate: i32::MAX,
                padding_baud_rate: -1,
                ..PadInfo::default()
            },
        ];
        let mut noise = Noise(0x9E37_79B9_7F4A_7C15);
        for _ in 0..if cfg!(miri) { 200 } else { 20000 } {
            let len = noise.below(24);
            let cap: Vec<u8> = (0..len)
                .map(|_| alphabet[noise.below(alphabet.len())])
                .collect();
            let params: Vec<i32> = (0..noise.below(10))
                .map(|_| i32::from_le_bytes(noise.next().to_le_bytes()[..4].try_into().unwrap()))
                .collect();
            let _ = tparm(&cap, &params);
            for pad in &pads {
                let mut out = Vec::new();
                tputs(&mut out, &cap, true, pad);
                assert!(out.len() <= cap.len() + 3 * usize::try_from(MAX_NULLS).unwrap());
            }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn formats_match_glibc_printf() {
        let values = [0, 1, -1, 7, 255, -123, 3054, i32::MAX, i32::MIN];
        for flags in ["", "-", " ", "#", "- ", "-#", " #", "-# "] {
            for width in ["", "1", "5", "05", "010", "12"] {
                for precision in ["", ".", ".0", ".3", ".12"] {
                    for conversion in ["d", "o", "x", "X", "s"] {
                        let spec = format!("{flags}{width}{precision}{conversion}");
                        let cap = format!("%p1%:{spec}");
                        let fmt = std::ffi::CString::new(format!("%{spec}")).unwrap();
                        for value in values {
                            let mut buf = [0u8; 64];
                            let n = unsafe {
                                if conversion == "s" {
                                    libc::snprintf(
                                        buf.as_mut_ptr().cast(),
                                        buf.len(),
                                        fmt.as_ptr(),
                                        c"".as_ptr(),
                                    )
                                } else {
                                    libc::snprintf(
                                        buf.as_mut_ptr().cast(),
                                        buf.len(),
                                        fmt.as_ptr(),
                                        value,
                                    )
                                }
                            };
                            let expected = &buf[..usize::try_from(n).unwrap()];
                            assert_eq!(
                                tparm(cap.as_bytes(), &[value]),
                                expected,
                                "{cap} mit {value}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(tparm(b"%p1%5#d", &[42]), b"%5#d");
        assert_eq!(tparm(b"%p1%10001d|", &[42]), b"42|");
        assert_eq!(tparm(b"%p1%.10001d|", &[42]), b"42|");
        assert_eq!(tparm(b"%p1%9.9.9d|", &[42]), b"42|");
        assert_eq!(tparm(b"%{1}%{2}%5+%d", &[]), b"3");
        assert_eq!(tparm(b"%i%i%p1%d,%p2%d", &[1, 1]), b"2,2");
        assert_eq!(tparm(b"%px%p1%d", &[4]), b"4");
        assert_eq!(tparm(b"%'a'%d|%'a%d|%{65x%c", &[]), b"97|d|A");
        let deep = "%{1}".repeat(30) + &"%+".repeat(29) + "%d";
        assert_eq!(tparm(deep.as_bytes(), &[]), b"20");
    }

    #[test]
    fn parameters_are_substituted() {
        let xm = b"\x1B[?1006;1000%?%p1%{1}%=%th%el%;";
        assert_eq!(tparm(xm, &[1]), b"\x1B[?1006;1000h");
        assert_eq!(tparm(xm, &[0]), b"\x1B[?1006;1000l");
        assert_eq!(tparm(b"\x1B[%i%p1%d;%p2%dH", &[4, 9]), b"\x1B[5;10H");
        assert_eq!(tparm(b"%p1%02d|%p1%x|%'a'%c|%{65}%c", &[7]), b"07|7|a|A");
        assert_eq!(
            tparm(b"%?%p1%{2}%>%tgross%e%p1%{1}%=%teins%eklein%;", &[3]),
            b"gross"
        );
        assert_eq!(
            tparm(b"%?%p1%{2}%>%tgross%e%p1%{1}%=%teins%eklein%;", &[1]),
            b"eins"
        );
        assert_eq!(
            tparm(b"%?%p1%{2}%>%tgross%e%p1%{1}%=%teins%eklein%;", &[0]),
            b"klein"
        );
    }
}
