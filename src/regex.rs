use std::ffi::CString;

pub const REG_EXTENDED: i32 = libc::REG_EXTENDED;
pub const REG_ICASE: i32 = libc::REG_ICASE;

fn count_subexpressions(pattern: &[u8]) -> usize {
    let mut count = 0;
    let mut i = 0;
    let mut in_bracket = false;
    while i < pattern.len() {
        let c = pattern[i];
        if in_bracket {
            if c == b']' {
                in_bracket = false;
            } else if c == b'['
                && i + 1 < pattern.len()
                && (pattern[i + 1] == b':' || pattern[i + 1] == b'.' || pattern[i + 1] == b'=')
            {
                let close = pattern[i + 2..]
                    .windows(2)
                    .position(|w| w[0] == pattern[i + 1] && w[1] == b']');
                if let Some(p) = close {
                    i += 2 + p + 1;
                }
            }
        } else if c == b'\\' {
            i += 1;
        } else if c == b'[' {
            in_bracket = true;
            if i + 1 < pattern.len() && pattern[i + 1] == b'^' {
                i += 1;
            }
            if i + 1 < pattern.len() && pattern[i + 1] == b']' {
                i += 1;
            }
        } else if c == b'(' {
            count += 1;
        }
        i += 1;
    }
    count
}

pub fn compile(pattern: &[u8], flags: i32) -> Result<Regex, String> {
    let Ok(cpat) = CString::new(pattern) else {
        return Err("Ungültiges Zeichen im Ausdruck".to_string());
    };
    unsafe {
        let mut re: libc::regex_t = std::mem::zeroed();
        let rc = libc::regcomp(&raw mut re, cpat.as_ptr(), flags);
        if rc != 0 {
            let mut buf = vec![0u8; 256];
            let n = libc::regerror(
                rc,
                &raw const re,
                buf.as_mut_ptr().cast::<libc::c_char>(),
                buf.len(),
            );
            let n = n.min(buf.len()).saturating_sub(1);
            buf.truncate(n);
            libc::regfree(&raw mut re);
            return Err(translate_regerror(&String::from_utf8_lossy(&buf)));
        }
        Ok(Regex {
            re,
            nsub: count_subexpressions(pattern),
        })
    }
}

fn translate_regerror(msg: &str) -> String {
    let text = crate::regex::regex_text(msg);
    if text == msg && !msg.is_empty() {
        return "Ungültiger regulärer Ausdruck".to_string();
    }
    text
}

#[cfg(target_vendor = "apple")]
pub const fn offset(position: libc::regoff_t) -> i64 {
    position
}

#[cfg(not(target_vendor = "apple"))]
pub fn offset(position: libc::regoff_t) -> i64 {
    i64::from(position)
}

pub fn exec(
    re: &Regex,
    text: &[u8],
    start: usize,
    end: usize,
    matches: &mut [libc::regmatch_t],
    eflags: i32,
) -> bool {
    let Ok(ctext) = CString::new(text) else {
        return false;
    };
    let end = end.min(text.len());
    if start > end {
        return false;
    }
    unsafe {
        let nmatch = matches.len();
        if nmatch > 0 {
            matches[0].rm_so = libc::regoff_t::try_from(start).unwrap_or(libc::regoff_t::MAX);
            matches[0].rm_eo = libc::regoff_t::try_from(end).unwrap_or(libc::regoff_t::MAX);
        }
        let rc = libc::regexec(
            &raw const re.re,
            ctext.as_ptr(),
            nmatch,
            matches.as_mut_ptr(),
            eflags | libc::REG_STARTEND,
        );
        rc == 0
    }
}

pub struct Regex {
    pub re: libc::regex_t,
    pub nsub: usize,
}

impl Drop for Regex {
    fn drop(&mut self) {
        unsafe { libc::regfree(&raw mut self.re) }
    }
}

const REGEX_TEXTS: &[(&str, &str)] = &[
    ("Success", "Erfolg"),
    ("No match", "Kein Treffer"),
    (
        "Invalid regular expression",
        "Ungültiger regulärer Ausdruck",
    ),
    (
        "Invalid collation character",
        "Ungültiges Vergleichszeichen",
    ),
    (
        "Invalid character class name",
        "Ungültiger Zeichenklassenname",
    ),
    ("Trailing backslash", "Angehängter Rückschrägstrich"),
    ("Invalid back reference", "Ungültige Rückreferenz"),
    (
        "Unmatched [, [^, [:, [., or [=",
        "Kein Gegenstück für [, [^, [:, [. oder [=",
    ),
    ("Unmatched ( or \\(", "Kein Gegenstück für ( oder \\("),
    ("Unmatched \\{", "Kein Gegenstück für \\{"),
    ("Invalid content of \\{\\}", "Ungültiger Inhalt von \\{\\}"),
    ("Invalid range end", "Ungültiges Bereichsende"),
    ("Memory exhausted", "Speicher ausgeschöpft"),
    (
        "Invalid preceding regular expression",
        "Ungültiger vorangestellter regulärer Ausdruck",
    ),
    (
        "Premature end of regular expression",
        "Vorzeitiges Ende des regulären Ausdrucks",
    ),
    (
        "Regular expression too big",
        "Regulärer Ausdruck ist zu groß",
    ),
    ("Unmatched ) or \\)", "Kein Gegenstück für ) oder \\)"),
    (
        "No previous regular expression",
        "Kein vorangehender regulärer Ausdruck",
    ),
];

pub fn regex_text(english: &str) -> String {
    match REGEX_TEXTS.iter().find(|(en, _)| *en == english) {
        Some((_, text)) => text.to_string(),
        None => english.to_string(),
    }
}
