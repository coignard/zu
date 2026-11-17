mod ctype {
    unsafe extern "C" {
        pub safe fn wcwidth(c: libc::wchar_t) -> libc::c_int;
        pub safe fn iswalnum(c: libc::c_uint) -> libc::c_int;
        pub safe fn iswalpha(c: libc::c_uint) -> libc::c_int;
        pub safe fn iswblank(c: libc::c_uint) -> libc::c_int;
        pub safe fn iswpunct(c: libc::c_uint) -> libc::c_int;
        pub safe fn towlower(c: libc::c_uint) -> libc::c_uint;
    }
}

pub fn setup_locale() -> bool {
    let utf8 = || {
        let codeset = unsafe { std::ffi::CStr::from_ptr(libc::nl_langinfo(libc::CODESET)) };
        codeset.to_bytes() == b"UTF-8"
    };
    unsafe {
        libc::setlocale(libc::LC_ALL, c"".as_ptr());
        for fallback in [c"C.UTF-8", c"UTF-8", c"en_US.UTF-8"] {
            if utf8() {
                break;
            }
            libc::setlocale(libc::LC_CTYPE, fallback.as_ptr());
        }
        libc::setlocale(libc::LC_MESSAGES, c"C".as_ptr());
    }
    utf8()
}

use crate::definitions::HIGHEST_POSITIVE;
use crate::global::Zu;
use crate::utils::{u8_of, usize_of};

pub fn mbtowide(c: &[u8]) -> Option<(char, usize)> {
    if c.is_empty() {
        return None;
    }
    let v1 = c[0];
    if v1 < 0x80 {
        return Some((v1 as char, 1));
    }
    let v2 = *c.get(1)? ^ 0x80;
    if v2 > 0x3F || v1 < 0xC2 {
        return None;
    }
    if v1 < 0xE0 {
        let wc = (u32::from(v1 & 0x1F) << 6) | u32::from(v2);
        return char::from_u32(wc).map(|ch| (ch, 2));
    }
    let v3 = *c.get(2)? ^ 0x80;
    if v3 > 0x3F {
        return None;
    }
    if v1 < 0xF0 {
        if (v1 > 0xE0 || v2 >= 0x20) && (v1 != 0xED || v2 < 0x20) {
            let wc = (u32::from(v1 & 0x0F) << 12) | (u32::from(v2) << 6) | u32::from(v3);
            return char::from_u32(wc).map(|ch| (ch, 3));
        }
        return None;
    }
    let v4 = *c.get(3)? ^ 0x80;
    if v4 > 0x3F || v1 > 0xF4 {
        return None;
    }
    if (v1 > 0xF0 || v2 >= 0x10) && (v1 != 0xF4 || v2 < 0x10) {
        let wc = (u32::from(v1 & 0x07) << 18)
            | (u32::from(v2) << 12)
            | (u32::from(v3) << 6)
            | u32::from(v4);
        return char::from_u32(wc).map(|ch| (ch, 4));
    }
    None
}

pub fn wcwidth(ch: char) -> i32 {
    if (ch as u32) < 0x20 || ch as u32 == 0x7F {
        return -1;
    }
    ctype::wcwidth(ch as libc::wchar_t)
}

pub fn is_alpha_char(c: &[u8]) -> bool {
    match mbtowide(c) {
        Some((ch, _)) => ctype::iswalpha(ch as libc::c_uint) != 0,
        None => false,
    }
}

pub fn is_alnum_char(c: &[u8]) -> bool {
    match mbtowide(c) {
        Some((ch, _)) => ctype::iswalnum(ch as libc::c_uint) != 0,
        None => false,
    }
}

pub fn is_blank_char(c: &[u8]) -> bool {
    if c.is_empty() {
        return false;
    }
    if c[0] < 0x80 {
        return c[0] == b' ' || c[0] == b'\t';
    }
    match mbtowide(c) {
        Some((ch, _)) => ctype::iswblank(ch as libc::c_uint) != 0,
        None => false,
    }
}

pub const fn is_cntrl_char(c: &[u8]) -> bool {
    if c.is_empty() {
        return false;
    }
    (c[0] & 0xE0) == 0 || c[0] == 0x7F || (c[0] == 0xC2 && c.len() > 1 && c[1] < 0xA0)
}

pub fn is_punct_char(c: &[u8]) -> bool {
    match mbtowide(c) {
        Some((ch, _)) => ctype::iswpunct(ch as libc::c_uint) != 0,
        None => false,
    }
}

pub fn control_rep(c: u8) -> u8 {
    let sc = c.cast_signed();
    if c == 0x7F {
        b'?'
    } else if sc == -97 {
        b'='
    } else if sc < 0 {
        u8_of(i64::from(i32::from(sc) + 224))
    } else {
        c + 64
    }
}

pub fn char_length(p: &[u8]) -> usize {
    if p.is_empty() {
        return 1;
    }
    let c1 = p[0];
    if c1 > 0xC1 {
        let c2 = *p.get(1).unwrap_or(&0);
        if (c2 ^ 0x80) > 0x3F {
            return 1;
        }
        if c1 < 0xE0 {
            return 2;
        }
        if (*p.get(2).unwrap_or(&0) ^ 0x80) > 0x3F {
            return 1;
        }
        if c1 < 0xF0 {
            if (c1 > 0xE0 || c2 >= 0xA0) && (c1 != 0xED || c2 < 0xA0) {
                return 3;
            }
            return 1;
        }
        if (*p.get(3).unwrap_or(&0) ^ 0x80) > 0x3F {
            return 1;
        }
        if c1 > 0xF4 {
            return 1;
        }
        if (c1 > 0xF0 || c2 >= 0x90) && (c1 != 0xF4 || c2 < 0x90) {
            return 4;
        }
    }
    1
}

pub fn mbstrlen(mut p: &[u8]) -> usize {
    let mut count = 0;
    while !p.is_empty() {
        let l = char_length(p).min(p.len());
        p = &p[l..];
        count += 1;
    }
    count
}

pub fn is_doublewidth(ch: &[u8]) -> bool {
    if ch.is_empty() || ch[0] < 0xE1 {
        return false;
    }
    match mbtowide(ch) {
        Some((c, _)) => wcwidth(c) == 2,
        None => false,
    }
}

pub fn is_zerowidth(ch: &[u8]) -> bool {
    if ch.is_empty() || ch[0] < 0xCC {
        return false;
    }
    match mbtowide(ch) {
        Some((c, _)) => wcwidth(c) == 0,
        None => false,
    }
}

pub fn step_left(buf: &[u8], pos: usize) -> usize {
    let mut before;
    let mut charlen = 0usize;
    if pos > 3 {
        if ((buf[pos - 1]).cast_signed()) > -65 {
            before = pos - 1;
        } else if ((buf[pos - 2]).cast_signed()) > -65 {
            before = pos - 2;
        } else if ((buf[pos - 3]).cast_signed()) > -65 {
            before = pos - 3;
        } else if ((buf[pos - 4]).cast_signed()) > -65 {
            before = pos - 4;
        } else {
            before = pos - 1;
        }
    } else {
        before = 0;
    }
    while before < pos {
        charlen = char_length(&buf[before..]);
        before += charlen;
    }
    before - charlen
}

pub fn step_right(buf: &[u8], pos: usize) -> usize {
    pos + char_length(&buf[pos..])
}

fn to_lower(ch: char) -> u32 {
    ctype::towlower(ch as libc::c_uint)
}

pub fn mbstrncasecmp(mut s1: &[u8], mut s2: &[u8], mut n: usize) -> i32 {
    while !s1.is_empty() && !s2.is_empty() && n > 0 {
        let (a, b) = (s1[0], s2[0]);
        if a < 0x80 && b < 0x80 {
            let au = a & 0x5F;
            let bu = b & 0x5F;
            let a_is_letter = au.is_ascii_uppercase();
            let b_is_letter = bu.is_ascii_uppercase();
            if a_is_letter {
                if b_is_letter {
                    if au != bu {
                        return i32::from(au) - i32::from(bu);
                    }
                } else {
                    return i32::from(a | 0x20) - i32::from(b);
                }
            } else if b_is_letter {
                return i32::from(a) - i32::from(b | 0x20);
            } else if a != b {
                return i32::from(a) - i32::from(b);
            }
            s1 = &s1[1..];
            s2 = &s2[1..];
            n -= 1;
            continue;
        }
        let w1 = mbtowide(s1);
        let w2 = mbtowide(s2);
        let bad1 = w1.is_none();
        let bad2 = w2.is_none();
        if bad1 || bad2 {
            if a != b {
                return i32::from(a) - i32::from(b);
            }
            if bad1 != bad2 {
                return if bad1 { 1 } else { -1 };
            }
        } else {
            let d = i64::from(to_lower(w1.unwrap().0)) - i64::from(to_lower(w2.unwrap().0));
            if d != 0 {
                return d.signum() as i32;
            }
        }
        s1 = &s1[char_length(s1).min(s1.len())..];
        s2 = &s2[char_length(s2).min(s2.len())..];
        n -= 1;
    }
    if n > 0 {
        i32::from(s1.first().copied().unwrap_or(0)) - i32::from(s2.first().copied().unwrap_or(0))
    } else {
        0
    }
}

pub fn mbstrcasecmp(s1: &[u8], s2: &[u8]) -> i32 {
    mbstrncasecmp(s1, s2, HIGHEST_POSITIVE)
}

pub fn mbstrcasestr(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let needle_len = mbstrlen(needle);
    let mut i = 0;
    while i < haystack.len() {
        if mbstrncasecmp(&haystack[i..], needle, needle_len) == 0 {
            return Some(i);
        }
        i += char_length(&haystack[i..]);
    }
    if needle.is_empty() {
        Some(haystack.len())
    } else {
        None
    }
}

pub fn revstrstr(haystack: &[u8], needle: &[u8], pointer: usize) -> Option<usize> {
    let needle_len = needle.len();
    let tail_len = haystack.len() - pointer;
    let mut p = pointer.cast_signed();
    if tail_len < needle_len {
        p -= (needle_len - tail_len).cast_signed();
    }
    while p >= 0 {
        let pu = p.cast_unsigned();
        if haystack.len() >= pu + needle_len && &haystack[pu..pu + needle_len] == needle {
            return Some(pu);
        }
        p -= 1;
    }
    None
}

pub fn mbrevstrcasestr(haystack: &[u8], needle: &[u8], pointer: usize) -> Option<usize> {
    let needle_len = mbstrlen(needle);
    let tail_len = mbstrlen(&haystack[pointer..]);
    let mut p = pointer;
    if tail_len < needle_len {
        let mut back = needle_len - tail_len;
        while back > 0 {
            if p == 0 {
                return None;
            }
            p = step_left(haystack, p);
            back -= 1;
        }
    }
    loop {
        if mbstrncasecmp(&haystack[p..], needle, needle_len) == 0 {
            return Some(p);
        }
        if p == 0 {
            return None;
        }
        p = step_left(haystack, p);
    }
}

pub fn mbstrchr(string: &[u8], chr: &[u8]) -> Option<usize> {
    let (wc, bad_c) = match mbtowide(chr) {
        Some((c, _)) => (c as u32, false),
        None => (u32::from(chr.first().copied().unwrap_or(0)), true),
    };
    let mut i = 0;
    while i < string.len() {
        let (ws, bad_s, len) = match mbtowide(&string[i..]) {
            Some((c, l)) => (c as u32, false, l),
            None => (u32::from(string[i]), true, 1),
        };
        if ws == wc && bad_s == bad_c {
            return Some(i);
        }
        i += len;
    }
    None
}

pub fn mbstrpbrk(string: &[u8], accept: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < string.len() {
        if mbstrchr(accept, &string[i..]).is_some() {
            return Some(i);
        }
        i += char_length(&string[i..]);
    }
    None
}

pub fn mbrevstrpbrk(head: &[u8], accept: &[u8], mut pointer: usize) -> Option<usize> {
    if pointer >= head.len() {
        if pointer == 0 {
            return None;
        }
        pointer = step_left(head, pointer);
    }
    loop {
        if mbstrchr(accept, &head[pointer..]).is_some() {
            return Some(pointer);
        }
        if pointer == 0 {
            return None;
        }
        pointer = step_left(head, pointer);
    }
}

pub fn has_blank_char(string: &[u8]) -> bool {
    let mut i = 0;
    while i < string.len() && !is_blank_char(&string[i..]) {
        i += char_length(&string[i..]);
    }
    i < string.len()
}

pub fn white_string(string: &[u8]) -> bool {
    let mut i = 0;
    while i < string.len() && (is_blank_char(&string[i..]) || string[i] == b'\r') {
        i += char_length(&string[i..]);
    }
    i >= string.len()
}

impl Zu {
    pub fn is_word_char(&self, c: &[u8], allow_punct: bool) -> bool {
        if c.is_empty() {
            return false;
        }
        if is_alnum_char(c) {
            return true;
        }
        if allow_punct && is_punct_char(c) {
            return true;
        }
        if !self.word_chars.is_empty() {
            let symlen = char_length(c).min(c.len());
            let symbol = &c[..symlen];
            return self.word_chars.windows(symlen).any(|w| w == symbol);
        }
        false
    }

    pub fn control_mbrep(&self, c: &[u8], isdata: bool) -> u8 {
        if c[0] == b'\n' && (isdata || self.as_an_at) {
            return b'@';
        }
        if c[0] < 128 {
            control_rep(c[0])
        } else {
            control_rep(*c.get(1).unwrap_or(&0))
        }
    }

    pub fn advance_over(&self, string: &[u8], column: &mut usize) -> usize {
        if string.is_empty() {
            return 1;
        }
        if string[0] >= 0x80 {
            if string[0] == 0xC2 && string.len() > 1 && string[1] < 0xA0 {
                *column += 2;
                return 2;
            }
            match mbtowide(string) {
                None => {
                    *column += 1;
                    return 1;
                }
                Some((ch, len)) => {
                    let w = wcwidth(ch);
                    *column += if w < 0 { 1 } else { usize_of(i64::from(w)) };
                    return len;
                }
            }
        }
        if string[0] < 0x20 {
            if string[0] == b'\t' {
                let ts = self.tabsize.cast_unsigned();
                *column += ts - *column % ts;
            } else {
                *column += 2;
            }
        } else if string[0] == 0x7F {
            *column += 2;
        } else {
            *column += 1;
        }
        1
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(miri, ignore)]
    fn character_classes_from_glibc() {
        if !setup_locale() {
            return;
        }
        assert_eq!(wcwidth('日'), 2);
        assert_eq!(wcwidth('\u{00AD}'), 1);
        assert_eq!(wcwidth('\u{0301}'), 0);
        assert!(is_alnum_char("ä".as_bytes()));
        assert!(is_punct_char("€".as_bytes()));
        assert!(!is_punct_char("ß".as_bytes()));
        assert!(is_blank_char("\u{3000}".as_bytes()));
        assert_eq!(mbstrncasecmp("ÜBER".as_bytes(), "über".as_bytes(), 4), 0);
    }
}
