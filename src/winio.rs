use crate::rcfile::strtosc;
use crate::utils::{i32_of, i64_of, isize_of, u8_of, usize_of};
use std::time::Duration;

use zeroize::Zeroize;

use crate::chars::{
    char_length, is_blank_char, is_cntrl_char, is_doublewidth, is_zerowidth, mbstrlen, mbtowide,
    step_left, wcwidth,
};
use crate::curses::mouse::{BUTTON1_CLICKED, BUTTON1_RELEASED, BUTTON4_PRESSED, BUTTON5_PRESSED};
use crate::curses::terminfo::KeyMatch;
use crate::curses::{A_REVERSE, Attr, Wait, Win};
use crate::definitions::{
    ALT_DELETE, ALT_DOWN, ALT_END, ALT_HOME, ALT_INSERT, ALT_LEFT, ALT_PAGEDOWN, ALT_PAGEUP,
    ALT_RIGHT, ALT_UP, BACKWARD, BRANDING, CONTROL_DELETE, CONTROL_DOWN, CONTROL_END, CONTROL_HOME,
    CONTROL_LEFT, CONTROL_RIGHT, CONTROL_SHIFT_DELETE, CONTROL_UP, DEL_CODE, END_OF_PASTE, ERR,
    ESC_CODE, Elem, FOREIGN_SEQUENCE, FORWARD, Flag, Format, Func, HIGHEST_POSITIVE, IGNORED_KEY,
    KEY_BACKSPACE, KEY_CENTER, KEY_DC, KEY_DOWN, KEY_END, KEY_ENTER, KEY_F, KEY_F0, KEY_FRESH,
    KEY_HOME, KEY_IC, KEY_LEFT, KEY_MOUSE, KEY_NPAGE, KEY_PPAGE, KEY_RIGHT, KEY_UP, Lp, MBROWSER,
    MGOTODIR, MISSING_BRACE, MMAIN, MORE_PLANTS, MREPLACEWITH, MSPELL, MWHEREISFILE, MYESNO, Msg,
    NO_SUCH_FUNCTION, PLANTED_A_COMMAND, SHIFT_ALT_DOWN, SHIFT_ALT_LEFT, SHIFT_ALT_RIGHT,
    SHIFT_ALT_UP, SHIFT_CONTROL_DOWN, SHIFT_CONTROL_END, SHIFT_CONTROL_HOME, SHIFT_CONTROL_LEFT,
    SHIFT_CONTROL_RIGHT, SHIFT_CONTROL_UP, SHIFT_DELETE, SHIFT_DOWN, SHIFT_END, SHIFT_HOME,
    SHIFT_LEFT, SHIFT_PAGEDOWN, SHIFT_PAGEUP, SHIFT_RIGHT, SHIFT_TAB, SHIFT_UP, START_OF_PASTE,
    THE_WINDOW_RESIZED, Update,
};
use crate::global::Zu;
use crate::utils::digits;

#[allow(dead_code)]
const PAINT_LIMIT: usize = 2000;
const PROCEED: i32 = -44;
const INVALID_DIGIT: i64 = -77;

impl Zu {
    pub fn record_macro(&mut self) {
        self.recording = !self.recording;
        if self.recording {
            self.previous_macro = std::mem::take(&mut self.macro_buffer);
            self.statusline(Msg::Remark, "Makro wird aufgezeichnet …");
        } else if self.milestone == 0 {
            self.macro_buffer = std::mem::take(&mut self.previous_macro);
            self.statusline(Msg::Remark, "Abgebrochen");
        } else {
            self.previous_macro.clear();
            self.macro_buffer.truncate(self.milestone);
            self.statusline(Msg::Remark, "Aufzeichnung gestoppt");
        }
        if self.isset(Flag::StateFlags) {
            self.titlebar(None);
        }
    }

    pub fn run_macro(&mut self) {
        if self.recording {
            self.statusline(
                Msg::Ahem,
                "Makro kann während der Aufzeichnung nicht ausgeführt werden",
            );
            self.macro_buffer.truncate(self.milestone);
            return;
        }
        if self.macro_buffer.is_empty() {
            self.statusline(Msg::Ahem, "Makro ist leer");
            return;
        }
        let codes = self.macro_buffer.clone();
        for &c in codes.iter().rev() {
            self.put_back(c);
        }
        self.mute_modifiers = true;
    }

    fn push_code(&mut self, code: i32) {
        if self.recording {
            self.macro_buffer.push(code);
        }
        self.key_buffer.push_back(code);
    }

    fn absorb_bytes(&mut self) -> usize {
        let mut fresh: Vec<u8> = Vec::with_capacity(64);
        let count = self.term.read_available(&mut |byte| fresh.push(byte));
        self.pending_input.extend_from_slice(&fresh);
        fresh.zeroize();
        count
    }

    fn input_is_incomplete(&self) -> bool {
        if !self.keypad_on || self.keytable.is_empty() {
            return false;
        }
        let raw = &self.pending_input;
        let mut i = 0;
        while i < raw.len() {
            match self.keytable.lookup(&raw[i..]) {
                KeyMatch::Key(len, _) => i += len,
                KeyMatch::Partial => return true,
                KeyMatch::None => i += 1,
            }
        }
        false
    }

    fn translate_input(&mut self) {
        let mut raw = std::mem::take(&mut self.pending_input);
        let mut i = 0;
        while i < raw.len() {
            match self.kgetch(&mut raw, &mut i, false) {
                Some(KEY_MOUSE) => self.mouse_loop(&mut raw, &mut i),
                Some(code) => self.push_code(code),
                None => break,
            }
        }
        raw.zeroize();
    }

    fn kgetch(&mut self, raw: &mut Vec<u8>, i: &mut usize, wait: bool) -> Option<i32> {
        loop {
            if *i >= raw.len() && !(wait && self.await_more_input(raw, None)) {
                return None;
            }
            let Some(focused) = self
                .term
                .reports_focus()
                .then(|| crate::curses::focus_report(&raw[*i..]))
                .flatten()
            else {
                break;
            };
            self.term.set_focus(focused);
            *i += 3;
        }
        if self.keypad_on && !self.keytable.is_empty() {
            loop {
                match self.keytable.lookup(&raw[*i..]) {
                    KeyMatch::Key(len, code) => {
                        *i += len;
                        return Some(code);
                    }
                    KeyMatch::Partial
                        if wait && self.await_more_input(raw, Some(Duration::from_millis(50))) => {}
                    _ => break,
                }
            }
        }
        let byte = raw[*i];
        *i += 1;
        Some(i32::from(byte))
    }

    fn await_more_input(&mut self, raw: &mut Vec<u8>, patience: Option<Duration>) -> bool {
        loop {
            match self
                .term
                .wait_for_input(patience.unwrap_or(Duration::from_secs(1)))
            {
                Wait::Ready => {
                    let before = raw.len();
                    self.term.read_available(&mut |byte| raw.push(byte));
                    return raw.len() > before;
                }
                Wait::Timeout if patience.is_none() => {}
                _ => return false,
            }
        }
    }

    fn mouse_loop(&mut self, raw: &mut Vec<u8>, i: &mut usize) {
        let mut runcount = 0;
        let mut ch = KEY_MOUSE;
        loop {
            if ch == KEY_MOUSE {
                runcount += 1;
                let mut mouse =
                    std::mem::replace(&mut self.term.mouse, crate::curses::mouse::Mouse::new());
                let complete = mouse.inline_event(&mut || {
                    if *i >= raw.len() && !self.await_more_input(raw, None) {
                        return None;
                    }
                    *i += 1;
                    Some(raw[*i - 1])
                });
                self.term.mouse = mouse;
                if complete {
                    break;
                }
            }
            if self.term.mouse.maxclick < 0 || ch != KEY_MOUSE {
                break;
            }
            let interval =
                Duration::from_millis((i64::from(self.term.mouse.maxclick)).cast_unsigned());
            let activity = *i < raw.len() || self.await_more_input(raw, Some(interval));
            if !activity && self.term.mouse.parse(runcount) {
                break;
            }
            if let Some(code) = self.kgetch(raw, i, true) {
                ch = code;
            } else {
                ch = ERR;
                break;
            }
        }
        self.push_code(KEY_MOUSE);
        if ch != KEY_MOUSE && ch != ERR {
            self.push_code(ch);
        }
    }

    pub fn read_keys_from(&mut self, frame: Win) {
        self.term.refresh_before_input(frame);
        self.term.keypad_wanted = self.keypad_on && !self.term.keypad_xmit.is_empty();
        self.term.apply_keypad();
        if self.reveal_cursor
            && (!self.spotlighted || self.isset(Flag::ShowCursor) || self.currmenu == MSPELL)
            && (self.lines() > 1 || self.lastmessage <= Msg::Hush)
        {
            self.term.curs_set(true);
        }
        self.term.settle();
        let mut timed = self.currmenu == MMAIN
            && (((self.isset(Flag::MiniBar) || self.isset(Flag::Zero) || self.lines() == 1)
                && self.lastmessage > Msg::Hush
                && self.lastmessage < Msg::Alert
                && self.lastmessage != Msg::Info)
                || self.spotlighted);
        let mut patience = Duration::from_millis(if !timed {
            250
        } else if self.isset(Flag::QuickBlank) {
            800
        } else {
            1500
        });
        loop {
            self.check_signals();
            if self.the_window_resized {
                self.regenerate_screen();
                self.clear_key_buffer();
                self.key_buffer.push_back(THE_WINDOW_RESIZED);
                self.term.curs_set(false);
                if self.currmenu == MMAIN {
                    self.refresh_needed |= self.spotlighted;
                    self.spotlighted = false;
                }
                return;
            }
            if !self.pending_input.is_empty() {
                break;
            }
            match self.term.wait_for_input(patience) {
                Wait::Ready => {
                    if self.absorb_bytes() > 0 {
                        break;
                    }
                }
                Wait::Timeout => {
                    if timed {
                        timed = false;
                        patience = Duration::from_millis(250);
                        if self.spotlighted || self.isset(Flag::Zero) || self.lines() == 1 {
                            if self.isset(Flag::Zero) && self.lastmessage > Msg::Vacuum {
                                self.term.wredrawln(Win::Mid, self.editwinrows - 1, 1);
                            }
                            self.lastmessage = Msg::Vacuum;
                            self.spotlighted = false;
                            let (c, x) = (self.current(), self.current_x());
                            self.update_line(c, x);
                            self.term.wnoutrefresh(Win::Mid);
                            self.term.curs_set(true);
                        }
                        if self.isset(Flag::MiniBar) && !self.isset(Flag::Zero) && self.lines() > 1
                        {
                            self.minibar();
                        }
                        self.as_an_at = true;
                        self.place_the_cursor();
                        self.term.refresh_before_input(frame);
                        self.term.settle();
                    }
                }
                Wait::Interrupted | Wait::Hangup => {}
            }
        }
        self.term.curs_set(false);
        if self.currmenu == MMAIN {
            self.refresh_needed |= self.spotlighted;
            self.spotlighted = false;
        }
        self.milestone = self
            .macro_buffer
            .len()
            .saturating_sub(self.key_buffer.len());
        if !self.keypad_on && self.pending_input == [u8_of(i64::from(ESC_CODE))] {
            let _ = self.term.wait_for_input(Duration::from_millis(20));
        }
        while self.absorb_bytes() > 0 {}
        if self.input_is_incomplete()
            && self.term.wait_for_input(Duration::from_millis(50)) == Wait::Ready
        {
            while self.absorb_bytes() > 0 {}
        }
        self.translate_input();
    }

    pub fn pop_code(&mut self) -> Option<i32> {
        let code = {
            let slot = self.key_buffer.front_mut()?;
            std::mem::replace(slot, 0)
        };
        self.key_buffer.pop_front();
        Some(code)
    }

    pub fn clear_key_buffer(&mut self) {
        for slot in &mut self.key_buffer {
            *slot = 0;
        }
        self.key_buffer.clear();
    }

    pub fn waiting_keycodes(&self) -> usize {
        self.key_buffer.len()
    }

    pub fn put_back(&mut self, keycode: i32) {
        self.key_buffer.push_front(keycode);
    }

    pub fn implant(&mut self, string: &[u8]) {
        self.plants = string.to_vec();
        self.plants_pos = 0;
        self.put_back(MORE_PLANTS);
        self.mute_modifiers = true;
    }

    fn get_code_from_plantation(&mut self) -> i32 {
        let p: Vec<u8> = self.plants[self.plants_pos..].to_vec();
        if p.first() == Some(&b'{') {
            let closing = match p[1..].iter().position(|&c| c == b'}') {
                Some(i) => i + 1,
                None => return MISSING_BRACE,
            };
            if p.get(1) == Some(&b'{') || p.get(1) == Some(&b'}') {
                if p.get(2) != Some(&b'}') {
                    return MISSING_BRACE;
                }
                let ch = i32::from(p[1]);
                self.plants_pos += 3;
                if self.plants_pos < self.plants.len() {
                    self.put_back(MORE_PLANTS);
                }
                return ch;
            }
            self.commandname = p[1..closing].to_vec();
            let name = String::from_utf8_lossy(&self.commandname).into_owned();
            self.planted_shortcut = strtosc(&name);
            if self.planted_shortcut.is_none() {
                return NO_SUCH_FUNCTION;
            }
            self.plants_pos += closing + 1;
            if self.plants_pos < self.plants.len() {
                self.put_back(MORE_PLANTS);
            }
            PLANTED_A_COMMAND
        } else {
            let opening = p.iter().position(|&c| c == b'{');
            let firstbyte = p.first().copied().unwrap_or(0);
            let length = opening.map_or(p.len(), |o| {
                self.put_back(MORE_PLANTS);
                o
            });
            let piece: Vec<u8> = p[..length].to_vec();
            for i in (1..length).rev() {
                self.put_back(i32::from(piece[i]));
            }
            self.plants_pos += length;
            if firstbyte != 0 {
                i32::from(firstbyte)
            } else {
                ERR
            }
        }
    }

    pub fn get_input(&mut self, frame: Option<Win>) -> i32 {
        if !self.key_buffer.is_empty() {
            self.spotlighted = false;
        } else if let Some(f) = frame {
            self.read_keys_from(f);
        }
        match self.pop_code() {
            Some(MORE_PLANTS) => self.get_code_from_plantation(),
            Some(c) => c,
            None => ERR,
        }
    }

    const fn arrow_from_abcd(letter: i32) -> i32 {
        if letter < 'C' as i32 {
            if letter == 'A' as i32 {
                KEY_UP
            } else {
                KEY_DOWN
            }
        } else if letter == 'D' as i32 {
            KEY_LEFT
        } else {
            KEY_RIGHT
        }
    }

    fn convert_ss3_sequence(&mut self, seq: &[i32; 8], length: usize, consumed: &mut usize) -> i32 {
        let c = |ch: char| ch as i32;
        let s0 = seq[0];
        if s0 == c('1') {
            if length > 3 && seq[1] == c(';') {
                *consumed = 4;
                if seq[2] == c('2') {
                    if (c('A')..=c('D')).contains(&seq[3]) {
                        self.shift_held = true;
                        return Self::arrow_from_abcd(seq[3]);
                    }
                } else if seq[2] == c('5') {
                    match u8_of(i64::from(seq[3])) {
                        b'A' => return CONTROL_UP,
                        b'B' => return CONTROL_DOWN,
                        b'C' => return CONTROL_RIGHT,
                        b'D' => return CONTROL_LEFT,
                        _ => {}
                    }
                }
            }
        } else if (c('2')..=c('8')).contains(&s0) {
            if length > 1 {
                *consumed = 2;
                if s0 == c('4') || s0 > c('5') {
                    return FOREIGN_SEQUENCE;
                }
                match u8_of(i64::from(seq[1])) {
                    b'A' => return CONTROL_UP,
                    b'B' => return CONTROL_DOWN,
                    b'C' => return CONTROL_RIGHT,
                    b'D' => return CONTROL_LEFT,
                    _ => {}
                }
                return seq[1] - 0x40;
            }
        } else if (c('A')..=c('D')).contains(&s0) {
            return Self::arrow_from_abcd(s0);
        } else {
            match u8_of(i64::from(s0)) {
                b'F' | b'q' => return KEY_END,
                b'H' | b'w' => return KEY_HOME,
                b'M' => return KEY_ENTER,
                b'P'..=b'Y' => return KEY_F(s0 - c('O')),
                b'a' => return CONTROL_UP,
                b'b' => return CONTROL_DOWN,
                b'c' => return CONTROL_RIGHT,
                b'd' => return CONTROL_LEFT,
                b'j' => return c('*'),
                b'k' => return c('+'),
                b'l' => return c(','),
                b'm' => return c('-'),
                b'n' => return KEY_DC,
                b'o' => return c('/'),
                b'p' => return KEY_IC,
                b'r' => return KEY_DOWN,
                b's' => return KEY_NPAGE,
                b't' => return KEY_LEFT,
                b'v' => return KEY_RIGHT,
                b'x' => return KEY_UP,
                b'y' => return KEY_PPAGE,
                _ => {}
            }
        }
        FOREIGN_SEQUENCE
    }

    fn convert_csi_sequence(&mut self, seq: &[i32; 8], length: usize, consumed: &mut usize) -> i32 {
        let c = |ch: char| ch as i32;
        let s0 = seq[0];
        if s0 < c('9') && length > 1 {
            *consumed = 2;
        }
        if !(0..0x80).contains(&s0) {
            return FOREIGN_SEQUENCE;
        }
        match u8_of(i64::from(s0)) {
            b'1' => {
                if length > 1 && seq[1] == c('~') {
                    return KEY_HOME;
                } else if length > 2 && seq[2] == c('~') {
                    *consumed = 3;
                    match u8_of(i64::from(seq[1])) {
                        b'1'..=b'5' => return KEY_F(seq[1] - c('0')),
                        b'7'..=b'9' => return KEY_F(seq[1] - c('1')),
                        _ => {}
                    }
                } else if length > 3 && seq[1] == c(';') {
                    *consumed = 4;
                    let letter = if (0..0x80).contains(&seq[3]) {
                        u8_of(i64::from(seq[3]))
                    } else {
                        0
                    };
                    match u8_of(i64::from(seq[2])) {
                        b'2' => match letter {
                            b'A'..=b'D' => {
                                self.shift_held = true;
                                return Self::arrow_from_abcd(seq[3]);
                            }
                            b'F' => return SHIFT_END,
                            b'H' => return SHIFT_HOME,
                            _ => {}
                        },
                        b'9' | b'3' => match letter {
                            b'A' => return ALT_UP,
                            b'B' => return ALT_DOWN,
                            b'C' => return ALT_RIGHT,
                            b'D' => return ALT_LEFT,
                            b'F' => return ALT_END,
                            b'H' => return ALT_HOME,
                            _ => {}
                        },
                        b'4' => match letter {
                            b'A' => return SHIFT_PAGEUP,
                            b'B' => return SHIFT_PAGEDOWN,
                            b'C' => return SHIFT_END,
                            b'D' => return SHIFT_HOME,
                            _ => {}
                        },
                        b'5' => match letter {
                            b'A' => return CONTROL_UP,
                            b'B' => return CONTROL_DOWN,
                            b'C' => return CONTROL_RIGHT,
                            b'D' => return CONTROL_LEFT,
                            b'E' => return KEY_CENTER,
                            b'F' => return CONTROL_END,
                            b'H' => return CONTROL_HOME,
                            _ => {}
                        },
                        b'6' => match letter {
                            b'A' => return SHIFT_CONTROL_UP,
                            b'B' => return SHIFT_CONTROL_DOWN,
                            b'C' => return SHIFT_CONTROL_RIGHT,
                            b'D' => return SHIFT_CONTROL_LEFT,
                            b'F' => return SHIFT_CONTROL_END,
                            b'H' => return SHIFT_CONTROL_HOME,
                            _ => {}
                        },
                        _ => {}
                    }
                } else if length > 4 && seq[2] == c(';') && seq[4] == c('~') {
                    *consumed = 5;
                }
            }
            b'2' => {
                if length > 2 && seq[2] == c('~') {
                    *consumed = 3;
                    match u8_of(i64::from(seq[1])) {
                        b'0' => return KEY_F(9),
                        b'1' => return KEY_F(10),
                        b'3' => return KEY_F(11),
                        b'4' => return KEY_F(12),
                        b'5' => return KEY_F(13),
                        b'6' => return KEY_F(14),
                        b'8' => return KEY_F(15),
                        b'9' => return KEY_F(16),
                        _ => {}
                    }
                } else if length > 1 && seq[1] == c('~') {
                    return KEY_IC;
                } else if length > 3 && seq[1] == c(';') && seq[3] == c('~') {
                    *consumed = 4;
                    if seq[2] == c('3') {
                        return ALT_INSERT;
                    }
                } else if length > 4 && seq[2] == c(';') && seq[4] == c('~') {
                    *consumed = 5;
                } else if length > 3 && seq[1] == c('0') && seq[3] == c('~') {
                    *consumed = 4;
                    return if seq[2] == c('0') {
                        START_OF_PASTE
                    } else {
                        END_OF_PASTE
                    };
                } else {
                    *consumed = length;
                    return FOREIGN_SEQUENCE;
                }
            }
            b'3' => {
                if length > 1 && seq[1] == c('~') {
                    return KEY_DC;
                }
                if length > 3 && seq[1] == c(';') && seq[3] == c('~') {
                    *consumed = 4;
                    match u8_of(i64::from(seq[2])) {
                        b'2' => return SHIFT_DELETE,
                        b'3' => return ALT_DELETE,
                        b'5' => return CONTROL_DELETE,
                        b'6' => return CONTROL_SHIFT_DELETE,
                        _ => {}
                    }
                }
                if length > 1 && seq[1] == c('$') {
                    return SHIFT_DELETE;
                }
                if length > 1 && seq[1] == c('^') {
                    return CONTROL_DELETE;
                }
                if length > 1 && seq[1] == c('@') {
                    return CONTROL_SHIFT_DELETE;
                }
                if length > 2 && seq[2] == c('~') {
                    *consumed = 3;
                }
            }
            b'4' => {
                if length > 1 && seq[1] == c('~') {
                    return KEY_END;
                }
            }
            b'5' => {
                if length > 1 && seq[1] == c('~') {
                    return KEY_PPAGE;
                } else if length > 3 && seq[1] == c(';') && seq[3] == c('~') {
                    *consumed = 4;
                    if seq[2] == c('2') {
                        return SHIFT_ALT_UP;
                    }
                    if seq[2] == c('3') {
                        return ALT_PAGEUP;
                    }
                }
            }
            b'6' => {
                if length > 1 && seq[1] == c('~') {
                    return KEY_NPAGE;
                } else if length > 3 && seq[1] == c(';') && seq[3] == c('~') {
                    *consumed = 4;
                    if seq[2] == c('2') {
                        return SHIFT_ALT_DOWN;
                    }
                    if seq[2] == c('3') {
                        return ALT_PAGEDOWN;
                    }
                }
            }
            b'7' => {
                if length > 1 {
                    match u8_of(i64::from(seq[1])) {
                        b'~' => return KEY_HOME,
                        b'$' => return SHIFT_HOME,
                        b'^' => return CONTROL_HOME,
                        b'@' => return SHIFT_CONTROL_HOME,
                        _ => {}
                    }
                }
            }
            b'8' => {
                if length > 1 {
                    match u8_of(i64::from(seq[1])) {
                        b'~' => return KEY_END,
                        b'$' => return SHIFT_END,
                        b'^' => return CONTROL_END,
                        b'@' => return SHIFT_CONTROL_END,
                        _ => {}
                    }
                }
            }
            b'9' => return KEY_DC,
            b'@' | b'L' => return KEY_IC,
            b'A'..=b'D' => return Self::arrow_from_abcd(s0),
            b'F' | b'Y' => return KEY_END,
            b'G' | b'U' => return KEY_NPAGE,
            b'H' => return KEY_HOME,
            b'I' | b'V' => return KEY_PPAGE,
            b'M'..=b'T' => return KEY_F(s0 - c('L')),
            b'W' => return KEY_F(11),
            b'X' => return KEY_F(12),
            b'Z' => return SHIFT_TAB,
            b'a'..=b'd' => {
                self.shift_held = true;
                return Self::arrow_from_abcd(s0 - 0x20);
            }
            b'[' if length > 1 => {
                *consumed = 2;
                if c('@') < seq[1] && seq[1] < c('F') {
                    return KEY_F(seq[1] - c('@'));
                }
            }
            _ => {}
        }
        FOREIGN_SEQUENCE
    }

    fn parse_escape_sequence(&mut self, starter: i32) -> i32 {
        let length = self.key_buffer.len();
        let mut seq = [0i32; 8];
        for (slot, code) in seq.iter_mut().zip(self.key_buffer.iter()) {
            *slot = *code;
        }
        let mut consumed = 1;
        let mut keycode = 0;
        if starter == 'O' as i32 {
            keycode = self.convert_ss3_sequence(&seq, length, &mut consumed);
        } else if starter == '[' as i32 {
            keycode = self.convert_csi_sequence(&seq, length, &mut consumed);
        }
        for _ in 0..consumed.min(length) {
            self.pop_code();
        }
        keycode
    }

    const fn assemble_byte_code(&mut self, keycode: i32) -> i32 {
        self.digit_count += 1;
        if self.digit_count == 1 {
            self.byte_code = (keycode - '0' as i32) * 100;
            return PROCEED;
        }
        if self.digit_count == 2 {
            if self.byte_code < 200 || keycode <= '5' as i32 {
                self.byte_code += (keycode - '0' as i32) * 10;
                return PROCEED;
            }
            self.digit_count = 0;
            return keycode;
        }
        self.digit_count = 0;
        if self.byte_code < 250 || keycode <= '5' as i32 {
            self.byte_code + keycode - '0' as i32
        } else {
            keycode
        }
    }

    pub fn parse_kbinput(&mut self, frame: Option<Win>) -> i32 {
        self.meta_key = false;
        self.shift_held = false;
        let mut keycode = self.get_input(frame);
        if keycode == ESC_CODE {
            self.first_escape_was_alone = self.last_escape_was_alone;
            self.last_escape_was_alone = self.key_buffer.is_empty();
            if self.digit_count > 0 {
                self.digit_count = 0;
                self.escapes = 1;
            } else {
                self.escapes += 1;
                if self.escapes > 2 {
                    self.escapes = i32::from(!self.last_escape_was_alone);
                }
            }
            return ERR;
        } else if keycode == ERR {
            return ERR;
        }
        let next = self.key_buffer.front().copied();
        let is = |ch: char| ch as i32;
        if self.escapes == 0 {
            if keycode < 0xFF && keycode != is('\t') && keycode != DEL_CODE {
                return keycode;
            }
        } else if self.escapes == 1 {
            self.escapes = 0;
            if !(0x20..=0x7E).contains(&keycode) {
                if keycode == is('\t') {
                    return SHIFT_TAB;
                } else if keycode == KEY_BACKSPACE || keycode == 8 || keycode == DEL_CODE {
                    return CONTROL_SHIFT_DELETE;
                } else if keycode < 0x20 && !self.last_escape_was_alone {
                    self.meta_key = true;
                    return keycode;
                }
            } else if next.is_none()
                || next == Some(ESC_CODE)
                || (keycode != is('O') && keycode != is('['))
            {
                if (is('A')..=is('Z')).contains(&keycode) && !self.shifted_metas {
                    keycode |= 0x20;
                }
                self.meta_key = true;
                return keycode;
            } else {
                keycode = self.parse_escape_sequence(keycode);
            }
        } else {
            self.escapes = 0;
            let arrow_follows = matches!(
                next,
                Some(n) if (is('A')..=is('D')).contains(&n) || (is('a')..=is('d')).contains(&n)
            );
            if keycode == is('[') && arrow_follows {
                match u8_of(i64::from(self.get_input(None))) {
                    b'A' => return KEY_HOME,
                    b'B' => return KEY_END,
                    b'C' => return CONTROL_RIGHT,
                    b'D' => return CONTROL_LEFT,
                    b'a' => {
                        self.shift_held = true;
                        return KEY_PPAGE;
                    }
                    b'b' => {
                        self.shift_held = true;
                        return KEY_NPAGE;
                    }
                    b'c' => {
                        self.shift_held = true;
                        return KEY_HOME;
                    }
                    _ => {
                        self.shift_held = true;
                        return KEY_END;
                    }
                }
            } else if next.is_some()
                && next != Some(ESC_CODE)
                && (keycode == is('[') || keycode == is('O'))
            {
                keycode = self.parse_escape_sequence(keycode);
                self.meta_key = true;
            } else if is('0') <= keycode
                && (keycode <= is('2') || (keycode <= is('9') && self.digit_count > 0))
            {
                let byte = self.assemble_byte_code(keycode);
                if byte == PROCEED {
                    self.escapes = 2;
                    return ERR;
                } else if byte > 0x7F {
                    if byte < 0xC0 {
                        self.put_back(byte);
                        return 0xC2;
                    }
                    self.put_back(byte - 0x40);
                    return 0xC3;
                } else if byte == is('\t') || byte == DEL_CODE {
                    keycode = byte;
                } else {
                    return byte;
                }
            } else if self.digit_count == 0 {
                if self.first_escape_was_alone && !self.last_escape_was_alone {
                    if (is('A')..=is('Z')).contains(&keycode) && !self.shifted_metas {
                        keycode |= 0x20;
                    }
                    self.meta_key = true;
                } else {
                    keycode = convert_to_control(keycode);
                }
            }
        }
        if (KEY_F0 + 24) < keycode && keycode < (KEY_F0 + 64) {
            return FOREIGN_SEQUENCE;
        }
        #[cfg(target_os = "linux")]
        if self.on_a_vt && !self.mute_modifiers {
            let mut modifiers: u8 = 6;
            if unsafe { libc::ioctl(0, libc::TIOCLINUX, &raw mut modifiers) } >= 0 {
                if modifiers & 0x01 != 0 {
                    if keycode == is('\t') {
                        return SHIFT_TAB;
                    }
                    if keycode == KEY_DC && modifiers == 0x01 {
                        return SHIFT_DELETE;
                    }
                    if keycode == KEY_DC && modifiers == 0x05 {
                        return CONTROL_SHIFT_DELETE;
                    }
                    if !self.meta_key {
                        self.shift_held = true;
                    }
                }
                if modifiers == 0x08 {
                    match keycode {
                        KEY_UP => return ALT_UP,
                        KEY_DOWN => return ALT_DOWN,
                        KEY_HOME => return ALT_HOME,
                        KEY_END => return ALT_END,
                        KEY_PPAGE => return ALT_PAGEUP,
                        KEY_NPAGE => return ALT_PAGEDOWN,
                        KEY_DC => return ALT_DELETE,
                        KEY_IC => return ALT_INSERT,
                        _ => {}
                    }
                }
                if modifiers & 0x04 != 0 {
                    match keycode {
                        KEY_UP => return CONTROL_UP,
                        KEY_DOWN => return CONTROL_DOWN,
                        KEY_LEFT => return CONTROL_LEFT,
                        KEY_RIGHT => return CONTROL_RIGHT,
                        KEY_HOME => return CONTROL_HOME,
                        KEY_END => return CONTROL_END,
                        KEY_DC => return CONTROL_DELETE,
                        _ => {}
                    }
                }
                if modifiers & 0x09 == 0x09 {
                    match keycode {
                        KEY_UP => return KEY_PPAGE,
                        KEY_DOWN => return KEY_NPAGE,
                        KEY_LEFT => return KEY_HOME,
                        KEY_RIGHT => return KEY_END,
                        _ => {}
                    }
                }
            }
        }
        match keycode {
            SHIFT_LEFT => {
                self.shift_held = true;
                KEY_LEFT
            }
            SHIFT_RIGHT => {
                self.shift_held = true;
                KEY_RIGHT
            }
            IGNORED_KEY | KEY_FRESH => ERR,
            SHIFT_HOME | SHIFT_ALT_LEFT => {
                self.shift_held = true;
                KEY_HOME
            }
            SHIFT_END | SHIFT_ALT_RIGHT => {
                self.shift_held = true;
                KEY_END
            }
            SHIFT_PAGEUP | SHIFT_ALT_UP => {
                self.shift_held = true;
                KEY_PPAGE
            }
            SHIFT_PAGEDOWN | SHIFT_ALT_DOWN => {
                self.shift_held = true;
                KEY_NPAGE
            }
            SHIFT_UP => {
                self.shift_held = true;
                KEY_UP
            }
            SHIFT_DOWN => {
                self.shift_held = true;
                KEY_DOWN
            }
            SHIFT_CONTROL_LEFT => {
                self.shift_held = true;
                CONTROL_LEFT
            }
            SHIFT_CONTROL_RIGHT => {
                self.shift_held = true;
                CONTROL_RIGHT
            }
            SHIFT_CONTROL_UP => {
                self.shift_held = true;
                CONTROL_UP
            }
            SHIFT_CONTROL_DOWN => {
                self.shift_held = true;
                CONTROL_DOWN
            }
            SHIFT_CONTROL_HOME => {
                self.shift_held = true;
                CONTROL_HOME
            }
            SHIFT_CONTROL_END => {
                self.shift_held = true;
                CONTROL_END
            }
            DEL_CODE | KEY_BACKSPACE => {
                if self.isset(Flag::RebindDelete) {
                    KEY_DC
                } else {
                    KEY_BACKSPACE
                }
            }
            KEY_DC => {
                if self.isset(Flag::RebindDelete) {
                    KEY_BACKSPACE
                } else {
                    KEY_DC
                }
            }
            _ => keycode,
        }
    }

    pub fn get_kbinput(&mut self, frame: Win, showcursor: bool) -> i32 {
        self.reveal_cursor = showcursor;
        let mut kbinput = ERR;
        while kbinput == ERR {
            kbinput = self.parse_kbinput(Some(frame));
        }
        if frame == Win::Mid {
            self.blank_it_when_expired();
        }
        kbinput
    }

    fn assemble_unicode(&mut self, symbol: i32, unicode: &mut i64, digits: &mut i32) -> i64 {
        let mut outcome = i64::from(PROCEED);
        if ('0' as i32) <= symbol && symbol <= ('9' as i32) {
            *unicode = (*unicode << 4) + i64::from(symbol - '0' as i32);
        } else if ('a' as i32) <= (symbol | 0x20) && (symbol | 0x20) <= ('f' as i32) {
            *unicode = (*unicode << 4) + i64::from((symbol | 0x20) - 'a' as i32 + 10);
        } else if symbol == '\r' as i32 || symbol == ' ' as i32 {
            outcome = *unicode;
        } else {
            outcome = INVALID_DIGIT;
        }
        *digits += 1;
        if *digits == 6 && outcome == i64::from(PROCEED) {
            outcome = if *unicode < 0x0011_0000 {
                *unicode
            } else {
                INVALID_DIGIT
            };
        }
        if outcome == i64::from(PROCEED) && self.currmenu == MMAIN {
            let partial = format!(
                "{:>6}",
                format!(
                    "{:0width$X}",
                    *unicode,
                    width = usize_of(i64::from(*digits))
                )
            );
            self.statusline(Msg::Info, &fmt!("Unicode-Eingabe: %s", &partial));
        }
        if outcome != i64::from(PROCEED) {
            *unicode = 0;
            *digits = 0;
        }
        outcome
    }

    fn parse_verbatim_kbinput(&mut self, frame: Win) -> (Vec<i32>, usize) {
        self.reveal_cursor = true;
        let mut keycode = self.get_input(Some(frame));
        if keycode == THE_WINDOW_RESIZED {
            return (Vec::new(), 999);
        }
        if !(0..=0xFF).contains(&keycode) {
            return (Vec::new(), 0);
        }
        if (u8_of(i64::from(keycode))).is_ascii_hexdigit() {
            let mut unicode: i64 = 0;
            let mut digits = 0;
            let mut outcome = self.assemble_unicode(keycode, &mut unicode, &mut digits);
            self.reveal_cursor = false;
            while outcome == i64::from(PROCEED) {
                keycode = self.get_input(Some(frame));
                outcome = self.assemble_unicode(keycode, &mut unicode, &mut digits);
            }
            if keycode == THE_WINDOW_RESIZED {
                return (Vec::new(), 999);
            }
            if outcome == INVALID_DIGIT {
                let front = |k: &Self| k.key_buffer.front().copied().unwrap_or(-1);
                if keycode == ESC_CODE && !self.key_buffer.is_empty() {
                    self.get_input(None);
                    while (0x20..0x40).contains(&front(self)) {
                        self.get_input(None);
                    }
                    if (0x40..0x7F).contains(&front(self)) {
                        self.get_input(None);
                    }
                } else if (0xC0..=0xFF).contains(&keycode) {
                    while (0x80..0xC0).contains(&front(self)) {
                        self.get_input(None);
                    }
                }
                return (Vec::new(), 0);
            }
            let Some(ch) = char::from_u32(i32_of(outcome).cast_unsigned()) else {
                return (Vec::new(), 0);
            };
            let mut buf = [0u8; 4];
            let enc: Vec<i32> = ch.encode_utf8(&mut buf).bytes().map(i32::from).collect();
            let n = enc.len();
            return (enc, n);
        }
        let mut yield_ = vec![keycode];
        if keycode == ESC_CODE && !self.key_buffer.is_empty() {
            let second = self.get_input(None);
            yield_.push(second);
            return (yield_, 2);
        }
        (yield_, 1)
    }

    pub fn get_verbatim_kbinput(&mut self, frame: Win) -> (Vec<u8>, usize) {
        if self.isset(Flag::Preserve) {
            crate::curses::set_flow_control(false);
        }
        self.term.set_bracketed_paste(false);
        let had_keypad = self.keypad_on;
        self.keypad_on = false;
        let (mut input, mut count) = self.parse_verbatim_kbinput(frame);
        self.keypad_on = had_keypad;
        if !input.is_empty() && count > 0 && count < 999 {
            if input[0] >= 0x80 && count == 1 {
                self.put_back(input[0]);
                count = 999;
            } else if (input[0] == '\n' as i32 && self.as_an_at)
                || (input[0] == 0 && !self.as_an_at)
            {
                count = 0;
            }
        }
        self.term.set_bracketed_paste(true);
        if self.isset(Flag::Preserve) {
            crate::curses::set_flow_control(true);
        }
        let bytes: Vec<u8> = if count < 999 {
            input
                .iter()
                .take(count)
                .map(|&c| u8_of(i64::from(c)))
                .collect()
        } else {
            Vec::new()
        };
        input.zeroize();
        (bytes, count)
    }

    pub fn get_mouseinput(&mut self) -> (i32, i32, i32) {
        let Some(ev) = self.term.mouse.getmouse() else {
            return (-1, 0, 0);
        };
        let in_middle = self.wenclose(Win::Mid, ev.y, ev.x);
        let in_footer = self.wenclose(Win::Foot, ev.y, ev.x);
        let mut mouse_x = ev.x
            - if in_middle {
                i32_of(i64_of(self.margin))
            } else {
                0
            };
        let mut mouse_y = ev.y;
        if ev.bstate & (BUTTON1_RELEASED | BUTTON1_CLICKED) != 0 {
            if in_middle
                && self.sidebar > 0
                && ev.x == i32_of(i64_of(self.cols())) - 1
                && self.currmenu == MMAIN
            {
                mouse_y -= i32_of(i64_of(self.term.win_y0(Win::Mid)));
                mouse_y += i32::from(mouse_y != 0);
                let target =
                    self.lineno(self.filebot()) * i64::from(mouse_y) / i64_of(self.editwinrows) + 1;
                let col = self.ofile().placewewant.cast_signed() + 1;
                self.goto_line_and_column(isize_of(target), col, true);
                self.refresh_needed = true;
                (2, mouse_y, mouse_x)
            } else if in_footer && !self.isset(Flag::NoHelp) && self.currmenu != MYESNO {
                if mouse_y == i32_of(i64_of(self.lines())) - 3 {
                    return (0, mouse_y, mouse_x);
                }
                mouse_y -= i32_of(i64_of(self.term.win_y0(Win::Foot)));
                let number = self.shown_entries_for(self.currmenu);
                let width = i32_of(i64_of(if number < 5 {
                    self.cols() / 2
                } else {
                    self.cols() / number.div_ceil(2)
                }));
                if width == 0 {
                    return (2, mouse_y, mouse_x);
                }
                let mut index = (mouse_x / width) * 2 + mouse_y;
                if usize_of(i64::from(index)) > number
                    && (mouse_x % width) < (i32_of(i64_of(self.cols())) % width)
                {
                    index -= 2;
                }
                if usize_of(i64::from(index)) > number || index < 1 {
                    return (2, mouse_y, mouse_x);
                }
                let mut idx = index;
                let menu = self.currmenu;
                let mut chosen: Option<(i32, bool)> = None;
                for f in &self.allfuncs {
                    if f.menus & menu == 0 {
                        continue;
                    }
                    let Some(sc) = self.first_sc_for(menu, f.func) else {
                        continue;
                    };
                    idx -= 1;
                    if idx == 0 {
                        chosen = Some((sc.keycode, (0x20..=0x7E).contains(&sc.keycode)));
                        break;
                    }
                }
                if let Some((code, is_meta)) = chosen {
                    self.put_back(code);
                    if is_meta {
                        self.put_back(ESC_CODE);
                    }
                }
                (1, mouse_y, mouse_x)
            } else {
                (0, mouse_y, mouse_x)
            }
        } else if ev.bstate & (BUTTON4_PRESSED | BUTTON5_PRESSED) != 0 {
            if in_footer {
                mouse_y -= i32_of(i64_of(self.term.win_y0(Win::Foot)));
            }
            if in_middle || (in_footer && mouse_y == 0) {
                let code = if ev.bstate & BUTTON4_PRESSED != 0 {
                    ALT_UP
                } else {
                    ALT_DOWN
                };
                self.put_back(code);
                self.put_back(code);
                (1, mouse_y, mouse_x)
            } else {
                (2, mouse_y, mouse_x)
            }
        } else {
            let _ = &mut mouse_x;
            (2, mouse_y, mouse_x)
        }
    }

    fn wenclose(&self, w: Win, y: i32, x: i32) -> bool {
        let y0 = i32_of(i64_of(self.term.win_y0(w)));
        let rows = i32_of(i64_of(self.term.win_rows(w)));
        rows > 0 && y >= y0 && y < y0 + rows && x >= 0 && (usize_of(i64::from(x))) < self.cols()
    }

    pub fn wmouse_trafo(&self, w: Win, y: &mut i32, x: i32) -> bool {
        let y0 = i32_of(i64_of(self.term.win_y0(w)));
        let rows = i32_of(i64_of(self.term.win_rows(w)));
        if *y >= y0 && *y < y0 + rows && x >= 0 && (usize_of(i64::from(x))) < self.cols() {
            *y -= y0;
            true
        } else {
            false
        }
    }

    pub fn blank_titlebar(&mut self) {
        self.term.fill_row(Win::Top, 0);
    }

    pub fn blank_edit(&mut self) {
        for row in 0..self.editwinrows {
            self.term.blank_row(Win::Mid, row);
        }
    }

    pub fn blank_statusbar(&mut self) {
        self.term.blank_row(Win::Foot, 0);
    }

    pub fn wipe_statusbar(&mut self) {
        self.lastmessage = Msg::Vacuum;
        if (self.isset(Flag::Zero) || self.isset(Flag::MiniBar) || self.lines() == 1)
            && self.currmenu == MMAIN
        {
            return;
        }
        self.term.blank_row(Win::Foot, 0);
        self.term.wnoutrefresh(Win::Foot);
    }

    pub fn blank_bottombars(&mut self) {
        if !self.isset(Flag::NoHelp) && self.lines() > 5 {
            self.term.blank_row(Win::Foot, 1);
            self.term.blank_row(Win::Foot, 2);
        }
    }

    pub fn blank_it_when_expired(&mut self) {
        if self.countdown == 0 {
            return;
        }
        self.countdown -= 1;
        if self.countdown == 0 {
            self.wipe_statusbar();
        }
        if self.currmenu == MMAIN && (self.isset(Flag::Zero) || self.lines() == 1) {
            self.term.wredrawln(Win::Mid, self.editwinrows - 1, 1);
            self.term.wnoutrefresh(Win::Mid);
        }
    }

    pub const fn set_blankdelay_to_one(&mut self) {
        self.countdown = 1;
    }

    pub fn display_string(
        &mut self,
        text: &[u8],
        mut column: usize,
        span: usize,
        isdata: bool,
        isprompt: bool,
    ) -> Vec<u8> {
        let start_x = self.actual_x(text, column);
        let start_col = self.wideness(text, start_x);
        let mut converted: Vec<u8> = Vec::with_capacity(span * 2 + 16);
        let beyond = column + span;
        let mut i = start_x;
        if span > HIGHEST_POSITIVE {
            self.statusline(Msg::Alert, "Spanne untergelaufen, bitte als Fehler melden");
            return converted;
        }
        if (start_col < column || (start_col > 0 && isdata && !self.isset(Flag::SoftWrap)))
            && i < text.len()
            && text[i] != b'\t'
        {
            if is_cntrl_char(&text[i..]) {
                if start_col < column {
                    let r = self.control_mbrep(&text[i..], isdata);
                    converted.push(r);
                    column += 1;
                    i += char_length(&text[i..]);
                }
            } else if is_doublewidth(&text[i..]) {
                if start_col == column {
                    converted.push(b' ');
                    column += 1;
                }
                converted.push(b']');
                column += 1;
                i += char_length(&text[i..]);
            }
        }
        let wsd = self.isset(Flag::WhitespaceDisplay);
        let sw = self.isset(Flag::SoftWrap);
        let ts = self.tabsize.cast_unsigned();
        while i < text.len() && (column < beyond || is_zerowidth(&text[i..])) {
            let c = text[i];
            if c > 0x20 && c < 0x7F {
                converted.push(c);
                column += 1;
                i += 1;
                continue;
            }
            if c == b' ' {
                if wsd {
                    converted.extend_from_slice(
                        &self.whitespace[self.whitelen[0]..self.whitelen[0] + self.whitelen[1]],
                    );
                } else {
                    converted.push(b' ');
                }
                column += 1;
                i += 1;
                continue;
            }
            if c == b'\t' {
                if wsd
                    && (!converted.is_empty()
                        || !isdata
                        || !sw
                        || column.is_multiple_of(ts)
                        || column == start_col)
                {
                    converted.extend_from_slice(&self.whitespace[..self.whitelen[0]]);
                } else {
                    converted.push(b' ');
                }
                column += 1;
                while !column.is_multiple_of(ts) && column < beyond {
                    converted.push(b' ');
                    column += 1;
                }
                i += 1;
                continue;
            }
            if is_cntrl_char(&text[i..]) {
                converted.push(b'^');
                let r = self.control_mbrep(&text[i..], isdata);
                converted.push(r);
                i += char_length(&text[i..]);
                column += 2;
                continue;
            }
            match mbtowide(&text[i..]) {
                None => {
                    converted.extend_from_slice(&[0xEF, 0xBF, 0xBD]);
                    i += 1;
                    column += 1;
                }
                Some((ch, len)) => {
                    let w = wcwidth(ch);
                    if self.on_a_vt && w == 0 {
                        i += len;
                        continue;
                    }
                    converted.extend_from_slice(&text[i..i + len]);
                    i += len;
                    column += if w < 0 { 1 } else { usize_of(i64::from(w)) };
                }
            }
        }
        if column > beyond || (i < text.len() && (isprompt || (isdata && !sw))) {
            let mut index = converted.len();
            loop {
                index = step_left(&converted, index);
                if !is_zerowidth(&converted[index..]) || index == 0 {
                    break;
                }
            }
            if is_doublewidth(&converted[index..]) {
                converted.truncate(index);
                converted.push(b'[');
            } else {
                converted.truncate(index);
            }
            self.has_more = true;
        } else {
            self.has_more = false;
        }
        self.is_shorter = column < beyond;
        self.from_x = start_x;
        self.till_x = i;
        converted
    }

    fn show_states_at(&mut self, w: Win) {
        let s = format!(
            "{}{}{}{}{}",
            if self.isset(Flag::AutoIndent) {
                "I"
            } else {
                " "
            },
            if self.ofile().mark.is_some() {
                "M"
            } else {
                " "
            },
            if self.isset(Flag::BreakLongLines) {
                "L"
            } else {
                " "
            },
            if self.recording { "R" } else { " " },
            if self.isset(Flag::SoftWrap) { "S" } else { " " }
        );
        self.term.waddstr(w, &s);
    }

    pub fn titlebar(&mut self, path: Option<&[u8]>) {
        if !self.term.win_exists(Win::Top) {
            return;
        }
        let cols = self.cols();
        let mut pluglen = 0usize;
        let mut offset = 0usize;
        let mut upperleft: Vec<u8> = Vec::new();
        let mut prefix: &str = "";
        let mut state: &str = "";
        let mut path: Vec<u8> = path.map_or_default(<[u8]>::to_vec);
        let given_path = !path.is_empty();
        let title_attr = self.interface_color_pair[Elem::TitleBar as usize];
        self.term.wattron(Win::Top, title_attr);
        self.blank_titlebar();
        self.as_an_at = false;
        if !self.inhelp && given_path {
            prefix = "Verz.:";
        } else if !self.inhelp {
            if self.more_than_one {
                upperleft =
                    format!("[{}/{}]", buffer_number(self.cur), self.files.len()).into_bytes();
            } else {
                upperleft = BRANDING.as_bytes().to_vec();
            }
            if self.ofile().filename.is_empty() {
                path = b"Neuer Puffer".to_vec();
            } else {
                path = self.ofile().filename.clone();
            }
            if self.isset(Flag::ViewMode) {
                state = "Ansehen";
            } else if self.isset(Flag::StateFlags) {
                state = "+.xxxxx";
            } else if self.ofile().modified {
                state = "Verändert";
            } else if self.isset(Flag::Restricted) {
                state = "Eingeschränkt";
            } else {
                pluglen = self.breadth("Verändert".as_bytes()) + 1;
            }
        }
        let mut verlen = self.breadth(&upperleft) + 3;
        let mut prefixlen = self.breadth(prefix.as_bytes());
        if prefixlen > 0 {
            prefixlen += 1;
        }
        let mut pathlen = self.breadth(&path);
        let mut statelen = self.breadth(state.as_bytes()) + 2;
        if statelen > 2 {
            pathlen += 1;
        }
        if verlen + prefixlen + pathlen + pluglen + statelen <= cols {
            self.term.mvwaddbytes(Win::Top, 0, 2, &upperleft);
        } else {
            verlen = 2;
            if verlen + prefixlen + pathlen + pluglen + statelen > cols {
                pluglen = 0;
            }
            if verlen + prefixlen + pathlen + pluglen + statelen > cols {
                verlen = 0;
                statelen -= 2;
            }
        }
        if verlen > 0 {
            offset = verlen
                + (cols
                    .saturating_sub(verlen + pluglen + statelen)
                    .saturating_sub(prefixlen + pathlen))
                    / 2;
        }
        if verlen + prefixlen + pathlen + pluglen + statelen <= cols {
            self.term.mvwaddstr(Win::Top, 0, offset, prefix);
            if prefixlen > 0 {
                self.term.waddstr(Win::Top, " ");
            }
        } else {
            self.term.wmove(Win::Top, 0, offset);
        }
        if pathlen + pluglen + statelen <= cols {
            let caption = self.display_string(&path, 0, pathlen, false, false);
            self.term.waddbytes(Win::Top, &caption);
        } else if 5 + statelen <= cols {
            self.term.waddstr(Win::Top, "...");
            let caption = self.display_string(
                &path,
                3 + pathlen + statelen - cols,
                cols - statelen,
                false,
                false,
            );
            self.term.waddbytes(Win::Top, &caption);
        }
        if !state.is_empty() && self.isset(Flag::StateFlags) && !self.isset(Flag::ViewMode) {
            if self.ofile().modified && cols > 1 {
                self.term.waddstr(Win::Top, " *");
            }
            if statelen < cols {
                self.term.wmove(Win::Top, 0, cols + 2 - statelen);
                self.show_states_at(Win::Top);
            }
        } else if statelen > 0 && statelen <= cols {
            self.term.mvwaddstr(Win::Top, 0, cols - statelen, state);
        } else if statelen > 0 {
            let n = self.actual_x(state.as_bytes(), cols);
            self.term.mvwaddstr(
                Win::Top,
                0,
                0,
                &String::from_utf8_lossy(&state.as_bytes()[..n]),
            );
        }
        self.term.wattroff(Win::Top, title_attr);
        self.term.wrefresh(Win::Top);
    }

    pub fn minibar(&mut self) {
        let cols = self.cols();
        let mut padding = 2usize;
        let mut tallywidth = 0usize;
        let attr = self.interface_color_pair[Elem::MiniInfobar as usize];
        self.term.wattron(Win::Foot, attr);
        self.term.fill_row(Win::Foot, 0);
        let thename: Vec<u8> = if self.ofile().filename.is_empty() {
            b"(unbenannt)".to_vec()
        } else {
            self.ofile().filename.clone()
        };
        let namewidth = self.breadth(&thename);
        self.as_an_at = false;
        if namewidth + 19 > cols {
            padding = 0;
        }
        if cols > 4 {
            self.term.wmove(Win::Foot, 0, padding);
            let stagename = if namewidth > cols - 2 {
                self.term.waddstr(Win::Foot, "...");
                self.display_string(&thename, namewidth - cols + 5, cols - 5, false, false)
            } else {
                self.display_string(&thename, 0, cols - 2, false, false)
            };
            self.term.waddbytes(Win::Foot, &stagename);
            let m = if self.ofile().modified { " *" } else { "  " };
            self.term.waddstr(Win::Foot, m);
        }
        let location = format!("{},{}", self.lineno(self.current()), self.xplustabs() + 1);
        let placewidth = location.len();
        if self.report_size && cols > 35 {
            let count = self.lineno(self.filebot()) - i64::from(self.datalen(self.filebot()) == 0);
            let plain = if self.ofile().plain_origin.is_some() {
                ", unverschlüsselt"
            } else {
                ""
            };
            let number_of_lines =
                if self.ofile().fmt == Format::Nix || self.ofile().fmt == Format::Unspecified {
                    fmt!(
                        crate::utils::plural(
                            " (%zu Zeile%s)",
                            " (%zu Zeilen%s)",
                            count.cast_unsigned()
                        ),
                        count,
                        plain
                    )
                } else {
                    fmt!(
                        crate::utils::plural(
                            " (%zu Zeile, %s%s)",
                            " (%zu Zeilen, %s%s)",
                            count.cast_unsigned()
                        ),
                        count,
                        "DOS",
                        plain
                    )
                };
            tallywidth = self.breadth(number_of_lines.as_bytes());
            if namewidth + tallywidth + 11 < cols {
                self.term.waddstr(Win::Foot, &number_of_lines);
            } else {
                tallywidth = 0;
            }
            self.report_size = false;
        } else if self.files.len() > 1 && cols > 35 {
            let ranking = format!(" [{}/{}]", buffer_number(self.cur), self.files.len());
            if namewidth + placewidth + self.breadth(ranking.as_bytes()) + 32 < cols {
                self.term.waddstr(Win::Foot, &ranking);
            }
        }
        if self.isset(Flag::ConstantShow) && namewidth + tallywidth + placewidth + 32 < cols {
            self.term
                .mvwaddstr(Win::Foot, 0, cols - 27 - placewidth, &location);
        }
        let mut successor = false;
        if self.isset(Flag::ConstantShow) && namewidth + tallywidth + 28 < cols {
            let cur = self.current();
            let x = self.current_x();
            let d = self.nodes[cur].data.clone();
            let hexadecimal = if x >= d.len() {
                if self.next(cur).is_some() {
                    "U+000A".to_string()
                } else {
                    "  ----".to_string()
                }
            } else if d[x] == b'\n' {
                "  0x00".to_string()
            } else if d[x] < 0x80 {
                format!("U+{:04X}", d[x])
            } else if let Some((ch, _)) = mbtowide(&d[x..]) {
                format!("U+{:04X}", ch as u32)
            } else {
                format!("  0x{:02X}", d[x])
            };
            self.term.mvwaddstr(Win::Foot, 0, cols - 23, &hexadecimal);
            if x < d.len() {
                let mut s = x + char_length(&d[x..]);
                if s < d.len()
                    && is_zerowidth(&d[s..])
                    && let Some((ch, _)) = mbtowide(&d[s..])
                {
                    self.term.waddstr(Win::Foot, &format!("|{:04X}", ch as u32));
                    successor = true;
                    s += char_length(&d[s..]);
                    if s < d.len()
                        && is_zerowidth(&d[s..])
                        && let Some((ch, _)) = mbtowide(&d[s..])
                    {
                        self.term.waddstr(Win::Foot, &format!("|{:04X}", ch as u32));
                    }
                }
            }
        }
        if self.isset(Flag::StateFlags)
            && !successor
            && namewidth + tallywidth + 14 + 2 * padding < cols
        {
            self.term.wmove(Win::Foot, 0, cols - 11 - padding);
            self.show_states_at(Win::Foot);
        }
        if self.nodes[self.current()].has_anchor && namewidth + 7 < cols {
            self.term
                .mvwaddstr(Win::Foot, 0, cols - 5 - padding, "\u{2020}");
        }
        if namewidth + 6 < cols {
            let pct = 100 * self.lineno(self.current()) / self.lineno(self.filebot()).max(1);
            self.term
                .mvwaddstr(Win::Foot, 0, cols - 4 - padding, &format!("{pct:>3}%"));
        }
        self.term.wattroff(Win::Foot, attr);
        self.term.wrefresh(Win::Foot);
    }

    pub fn statusline(&mut self, importance: Msg, msg: &str) {
        self.statusline_bytes(importance, msg.as_bytes());
    }

    pub fn statusline_bytes(&mut self, importance: Msg, msg: &[u8]) {
        let showed_whitespace = self.isset(Flag::WhitespaceDisplay);
        if importance >= Msg::Ahem {
            self.clear_key_buffer();
        }
        if importance < self.lastmessage && self.lastmessage > Msg::Notice {
            return;
        }
        let compound: Vec<u8> = msg.to_vec();
        if self.term.isendwin() {
            warn!("\n{}\n", String::from_utf8_lossy(&compound));
            return;
        }
        if !self.we_are_running
            && importance == Msg::Alert
            && !self.files.is_empty()
            && self.ofile().fmt == Format::Unspecified
            && self.ofile().errormessage.is_none()
            && self.files.len() > 1
        {
            self.ofile_mut().errormessage = Some(String::from_utf8_lossy(&compound).into_owned());
        }
        if self.lines() == 1 && importance < Msg::Info {
            self.term.wnoutrefresh(Win::Mid);
        }
        let cols = self.cols();
        if self.lastmessage == Msg::Alert {
            if self.statusline_start_col > 4 {
                self.term
                    .wmove(Win::Foot, 0, cols + 2 - self.statusline_start_col);
                let a = self.interface_color_pair[Elem::ErrorMessage as usize];
                self.term.wattron(Win::Foot, a);
                self.term.waddstr(Win::Foot, "...");
                self.term.wattroff(Win::Foot, a);
                self.term.wnoutrefresh(Win::Foot);
                self.statusline_start_col = 0;
                self.term.doupdate();
                self.term.napms(100);
                self.term.beep();
            }
            return;
        }
        let colorpair = match importance.cmp(&Msg::Notice) {
            std::cmp::Ordering::Greater => {
                if importance == Msg::Alert {
                    self.term.beep();
                }
                self.interface_color_pair[Elem::ErrorMessage as usize]
            }
            std::cmp::Ordering::Equal => self.interface_color_pair[Elem::SelectedText as usize],
            std::cmp::Ordering::Less => self.interface_color_pair[Elem::StatusBar as usize],
        };
        self.lastmessage = importance;
        self.blank_statusbar();
        self.flags.unset(Flag::WhitespaceDisplay);
        let message = self.display_string(&compound, 0, cols, false, false);
        if showed_whitespace {
            self.flags.set(Flag::WhitespaceDisplay);
        }
        let start_col = cols.saturating_sub(self.breadth(&message)) / 2;
        self.statusline_start_col = start_col;
        let bracketed = start_col > 1;
        self.term.wmove(
            Win::Foot,
            0,
            if bracketed { start_col - 2 } else { start_col },
        );
        self.term.wattron(Win::Foot, colorpair);
        if bracketed {
            self.term.waddstr(Win::Foot, "[ ");
        }
        self.term.waddbytes(Win::Foot, &message);
        if bracketed {
            self.term.waddstr(Win::Foot, " ]");
        }
        self.term.wattroff(Win::Foot, colorpair);
        self.term.wrefresh(Win::Foot);
        self.countdown = if self.isset(Flag::QuickBlank) { 1 } else { 20 };
    }

    pub fn statusbar(&mut self, msg: &str) {
        self.statusline(Msg::Hush, msg);
    }

    pub fn warn_and_briefly_pause(&mut self, msg: &str) {
        self.blank_bottombars();
        self.statusline(Msg::Alert, msg);
        self.lastmessage = Msg::Vacuum;
        self.term.napms(1500);
    }

    pub fn post_one_key(&mut self, keystroke: &str, tag: &str, width: usize) {
        let kc = self.interface_color_pair[Elem::KeyCombo as usize];
        self.term.wattron(Win::Foot, kc);
        let n = self.actual_x(keystroke.as_bytes(), width);
        self.term.waddbytes(Win::Foot, &keystroke.as_bytes()[..n]);
        self.term.wattroff(Win::Foot, kc);
        let used = self.breadth(keystroke.as_bytes());
        if width < used + 2 {
            return;
        }
        let width = width - used;
        self.term.waddch(Win::Foot, ' ');
        let ft = self.interface_color_pair[Elem::FunctionTag as usize];
        self.term.wattron(Win::Foot, ft);
        let n = self.actual_x(tag.as_bytes(), width - 1);
        self.term.waddbytes(Win::Foot, &tag.as_bytes()[..n]);
        self.term.wattroff(Win::Foot, ft);
    }

    pub fn bottombars(&mut self, menu: u32) {
        self.currmenu = menu;
        let minimum = if self.isset(Flag::Zero) {
            3
        } else if self.isset(Flag::MiniBar) {
            4
        } else {
            5
        };
        if self.isset(Flag::NoHelp) || self.lines() < minimum {
            return;
        }
        let number = self.shown_entries_for(menu);
        if number == 0 {
            return;
        }
        let cols = self.cols();
        let itemwidth = cols / number.div_ceil(2);
        if itemwidth == 0 {
            return;
        }
        self.blank_bottombars();
        let mut index = 0usize;
        let items: Vec<(Func, &'static str, u32)> = self
            .allfuncs
            .iter()
            .map(|f| (f.func, f.tag, f.menus))
            .collect();
        for (func, tag, menus) in items {
            if index >= number {
                break;
            }
            if menus & menu == 0 {
                continue;
            }
            let keystr = match self.first_sc_for(menu, func) {
                Some(s) => s.keystr.clone(),
                None => continue,
            };
            let mut thiswidth = itemwidth;
            self.term
                .wmove(Win::Foot, 1 + index % 2, (index / 2) * itemwidth);
            if number % 2 == 1 && index + 2 == number {
                thiswidth += itemwidth;
            }
            if index + 2 >= number {
                thiswidth += cols % itemwidth;
            }
            self.post_one_key(&keystr, tag, thiswidth);
            index += 1;
        }
        self.term.wrefresh(Win::Foot);
    }

    pub fn place_the_cursor(&mut self) {
        let mut row: i64 = 0;
        let mut column = self.xplustabs();
        if self.isset(Flag::SoftWrap) {
            let top = self.edittop();
            row -= i64_of(self.chunk_for(self.ofile().firstcolumn, top));
            let mut line = Some(top);
            while let Some(l) = line {
                if l == self.current() {
                    break;
                }
                row += 1 + i64_of(self.extra_chunks_in(l));
                line = self.next(l);
            }
            let mut leftedge = 0;
            row += i64_of(self.get_chunk_and_edge(column, self.current(), Some(&mut leftedge)));
            column -= leftedge;
        } else {
            row = self.lineno(self.current()) - self.lineno(self.edittop());
            column -= self.get_page_start(column);
        }
        if row < 0 {
        } else if (usize_of(row)) < self.editwinrows {
            self.term
                .wmove(Win::Mid, usize_of(row), self.margin + column);
        } else {
            self.statusline(
                Msg::Alert,
                "Cursor falsch platziert, bitte als Fehler melden",
            );
        }
        self.ofile_mut().cursor_row = row;
    }

    fn draw_row(&mut self, row: usize, converted: &[u8], line: Lp, from_col: usize) {
        let cols = self.cols();
        if self.margin > 0 {
            let ln = self.interface_color_pair[Elem::LineNumber as usize];
            self.term.wattron(Win::Mid, ln);
            if self.isset(Flag::SoftWrap) && from_col != 0 {
                self.term
                    .mvwaddstr(Win::Mid, row, 0, &" ".repeat(self.margin - 1));
            } else {
                self.term.mvwaddstr(
                    Win::Mid,
                    row,
                    0,
                    &format!("{:>w$}", self.lineno(line), w = self.margin - 1),
                );
            }
            self.term.wattroff(Win::Mid, ln);
            if self.nodes[line].has_anchor && (from_col == 0 || !self.isset(Flag::SoftWrap)) {
                self.term.waddstr(Win::Mid, "\u{2020}");
            } else {
                self.term.waddstr(Win::Mid, " ");
            }
        }
        self.term.mvwaddbytes(Win::Mid, row, self.margin, converted);
        if self.is_shorter || self.isset(Flag::SoftWrap) {
            self.term.wclrtoeol(Win::Mid);
        }
        if self.sidebar > 0 && row < self.bardata.len() {
            let a = self.bardata[row];
            self.term.wmove(Win::Mid, row, cols - 1);
            self.term.waddch_attr(Win::Mid, ' ', a);
        }
        if self.stripe_column > 0
            && (self.stripe_column.cast_unsigned()) > from_col
            && !self.inhelp
            && (self.sequel_column == 0 || self.stripe_column.cast_unsigned() <= self.sequel_column)
            && self.stripe_column.cast_unsigned() <= from_col + self.editwincols
        {
            let mut target_column = self.stripe_column.cast_unsigned() - from_col - 1;
            let target_x = self.actual_x(converted, target_column);
            let striped: Vec<u8>;
            if target_x < converted.len() {
                let l = char_length(&converted[target_x..]);
                striped = converted[target_x..target_x + l].to_vec();
                target_column = self.wideness(converted, target_x);
            } else {
                striped = vec![b' '];
            }
            let gs = self.interface_color_pair[Elem::GuideStripe as usize];
            self.term.wattron(Win::Mid, gs);
            self.term
                .mvwaddbytes(Win::Mid, row, self.margin + target_column, &striped);
            self.term.wattroff(Win::Mid, gs);
        }
        if let Some(mark) = self.ofile().mark {
            let ln = self.lineno(line);
            let (ml, cl) = (self.lineno(mark), self.lineno(self.current()));
            if (ln >= ml && ln <= cl) || (ln <= ml && ln >= cl) {
                let (top, mut top_x, bot, mut bot_x) = self.get_region();
                if self.lineno(top) < ln || top_x < self.from_x {
                    top_x = self.from_x;
                }
                if self.lineno(bot) > ln || bot_x > self.till_x {
                    bot_x = self.till_x;
                }
                if top_x < self.till_x && bot_x > self.from_x {
                    let start_col = self
                        .wideness(self.data(line), top_x)
                        .saturating_sub(from_col);
                    let tx = self.actual_x(converted, start_col);
                    let thetext = &converted[tx..];
                    let paintlen = if bot_x < self.till_x {
                        let end_col = self.wideness(self.data(line), bot_x) - from_col;
                        self.actual_x(thetext, end_col - start_col)
                    } else {
                        thetext.len()
                    };
                    let piece = thetext[..paintlen.min(thetext.len())].to_vec();
                    let st = self.interface_color_pair[Elem::SelectedText as usize];
                    self.term.wattron(Win::Mid, st);
                    self.term
                        .mvwaddbytes(Win::Mid, row, self.margin + start_col, &piece);
                    self.term.wattroff(Win::Mid, st);
                }
            }
        }
    }

    pub fn update_line(&mut self, line: Lp, index: usize) -> usize {
        if self.isset(Flag::SoftWrap) {
            return self.update_softwrapped_line(line);
        }
        self.sequel_column = 0;
        let from_col = if self.united_sidescroll {
            self.ofile().brink
        } else {
            self.get_page_start(self.wideness(self.data(line), index))
        };
        let row = self.lineno(line) - self.lineno(self.edittop());
        if row < 0 || usize_of(row) >= self.editwinrows {
            return 1;
        }
        let row = usize_of(row);
        let data = self.nodes[line].data.clone();
        let converted = self.display_string(&data, from_col, self.editwincols, true, false);
        self.draw_row(row, &converted, line, from_col);
        let ha = self.hilite_attribute;
        if from_col > 0 && !converted.is_empty() {
            self.term.wattron(Win::Mid, ha);
            self.term.mvwaddch(Win::Mid, row, self.margin, '<');
            self.term.wattroff(Win::Mid, ha);
        }
        if self.has_more {
            self.term.wattron(Win::Mid, ha);
            self.term
                .mvwaddch(Win::Mid, row, self.cols() - 1 - self.sidebar, '>');
            self.term.wattroff(Win::Mid, ha);
        }
        if self.spotlighted && line == self.current() {
            let (f, t) = (self.light_from_col, self.light_to_col);
            self.spotlight(f, t);
        }
        1
    }

    pub fn update_softwrapped_line(&mut self, line: Lp) -> usize {
        let top = self.edittop();
        let (mut row, mut from_col) = if line == top {
            (0, self.ofile().firstcolumn)
        } else {
            (-i64_of(self.chunk_for(self.ofile().firstcolumn, top)), 0)
        };
        let mut someline = Some(top);
        while let Some(s) = someline {
            if s == line {
                break;
            }
            row += 1 + i64_of(self.extra_chunks_in(s));
            someline = self.next(s);
        }
        if row < 0 || usize_of(row) >= self.editwinrows {
            return 0;
        }
        let starting_row = usize_of(row);
        let mut row = starting_row;
        let mut kickoff = true;
        let mut end_of_line = false;
        let data = self.nodes[line].data.clone();
        while !end_of_line && row < self.editwinrows {
            let to_col =
                self.get_softwrap_breakpoint(&data, from_col, &mut kickoff, &mut end_of_line);
            self.sequel_column = if end_of_line { 0 } else { to_col };
            let converted = self.display_string(&data, from_col, to_col - from_col, true, false);
            self.draw_row(row, &converted, line, from_col);
            row += 1;
            from_col = to_col;
        }
        if self.spotlighted && line == self.current() {
            let (f, t) = (self.light_from_col, self.light_to_col);
            self.spotlight_softwrapped(f, t);
        }
        row - starting_row
    }

    pub fn line_needs_update(&mut self, old_column: usize, new_column: usize) -> bool {
        if self.get_page_start(old_column) == self.get_page_start(new_column) {
            return self.ofile().mark.is_some();
        }
        if self.united_sidescroll {
            self.refresh_needed = true;
        }
        !self.refresh_needed
    }

    pub fn go_back_chunks(&self, nrows: i32, line: &mut Lp, leftedge: &mut usize) -> i32 {
        let mut i = nrows;
        if self.isset(Flag::SoftWrap) {
            while i > 0 {
                let chunk = i32_of(i64_of(self.chunk_for(*leftedge, *line)));
                *leftedge = 0;
                if chunk >= i {
                    return self.go_forward_chunks(chunk - i, line, leftedge);
                }
                if *line == self.filetop() {
                    break;
                }
                i -= chunk;
                *line = self.prev(*line).unwrap();
                *leftedge = HIGHEST_POSITIVE;
                i -= 1;
            }
            if *leftedge == HIGHEST_POSITIVE {
                *leftedge = self.leftedge_for(*leftedge, *line);
            }
        } else {
            while i > 0 && self.prev(*line).is_some() {
                *line = self.prev(*line).unwrap();
                i -= 1;
            }
        }
        i
    }

    pub fn go_forward_chunks(&self, nrows: i32, line: &mut Lp, leftedge: &mut usize) -> i32 {
        let mut i = nrows;
        if self.isset(Flag::SoftWrap) {
            let mut current_leftedge = *leftedge;
            let mut kickoff = true;
            let mut sw = SoftwrapState::default();
            while i > 0 {
                let mut end_of_line = false;
                current_leftedge = self.softwrap_breakpoint(
                    &self.nodes[*line].data,
                    current_leftedge,
                    &mut kickoff,
                    &mut end_of_line,
                    &mut sw,
                );
                if !end_of_line {
                    i -= 1;
                    continue;
                }
                if *line == self.filebot() {
                    break;
                }
                *line = self.next(*line).unwrap();
                current_leftedge = 0;
                kickoff = true;
                i -= 1;
            }
            if i < nrows {
                *leftedge = current_leftedge;
            }
        } else {
            while i > 0 && self.next(*line).is_some() {
                *line = self.next(*line).unwrap();
                i -= 1;
            }
        }
        i
    }

    pub fn less_than_a_screenful(&self, was_lineno: usize, was_leftedge: usize) -> bool {
        if self.isset(Flag::SoftWrap) {
            let mut line = self.current();
            let mut leftedge = self.leftedge_for(self.xplustabs(), self.current());
            let rows_left = self.go_back_chunks(
                i32_of(i64_of(self.editwinrows)) - 1,
                &mut line,
                &mut leftedge,
            );
            rows_left > 0
                || (usize_of(self.lineno(line))) < was_lineno
                || (usize_of(self.lineno(line)) == was_lineno && leftedge <= was_leftedge)
        } else {
            (usize_of(self.lineno(self.current()))).saturating_sub(was_lineno) < self.editwinrows
        }
    }

    pub fn draw_scrollbar(&mut self) {
        let fromline = usize_of(self.lineno(self.edittop()) - 1);
        let totallines = usize_of(self.lineno(self.filebot()).max(1));
        let coveredlines = if self.isset(Flag::SoftWrap) {
            let mut line = self.edittop();
            let mut extras = i64_of(self.extra_chunks_in(line))
                - i64_of(self.chunk_for(self.ofile().firstcolumn, line));
            while (self.lineno(line) + extras) < i64_of(fromline + self.editwinrows)
                && self.next(line).is_some()
            {
                line = self.next(line).unwrap();
                extras += i64_of(self.extra_chunks_in(line));
            }
            usize_of(self.lineno(line)).saturating_sub(fromline)
        } else {
            self.editwinrows
        };
        let lowest = (fromline * self.editwinrows) / totallines;
        let highest = if self.editwinrows > totallines && !self.isset(Flag::SoftWrap) {
            self.editwinrows
        } else {
            lowest + (self.editwinrows * coveredlines) / totallines
        };
        let base = self.interface_color_pair[Elem::ScrollBar as usize];
        self.bardata
            .resize(self.lines().max(self.editwinrows), base);
        let cols = self.cols();
        for row in 0..self.editwinrows {
            let a = if row < lowest || row > highest {
                base
            } else {
                base.or(Attr::bits(A_REVERSE))
            };
            self.bardata[row] = a;
            self.term.wmove(Win::Mid, row, cols - 1);
            self.term.waddch_attr(Win::Mid, ' ', a);
        }
    }

    pub fn edit_scroll(&mut self, direction: bool) {
        let mut top = self.edittop();
        let mut fc = self.ofile().firstcolumn;
        if direction == BACKWARD {
            self.go_back_chunks(1, &mut top, &mut fc);
        } else {
            self.go_forward_chunks(1, &mut top, &mut fc);
        }
        {
            let of = self.ofile_mut();
            of.edittop = top;
            of.firstcolumn = fc;
        }
        self.term
            .wscrl(Win::Mid, if direction == BACKWARD { -1 } else { 1 });
        let mut nrows: i32 = 1;
        let pww = self.ofile().placewewant;
        if self.line_needs_update(pww, 0) && (usize_of(i64::from(nrows))) < self.editwinrows {
            nrows += 1;
        }
        let mut line = self.edittop();
        let mut leftedge = self.ofile().firstcolumn;
        if direction == FORWARD {
            self.go_forward_chunks(
                i32_of(i64_of(self.editwinrows)) - nrows,
                &mut line,
                &mut leftedge,
            );
        }
        if self.sidebar > 0 {
            self.draw_scrollbar();
        }
        if self.isset(Flag::SoftWrap) {
            nrows += i32_of(i64_of(self.chunk_for(leftedge, line)));
            if line == self.edittop() {
                nrows -= i32_of(i64_of(self.chunk_for(self.ofile().firstcolumn, line)));
            }
        }
        let mut l = Some(line);
        while nrows > 0 {
            let Some(line) = l else { break };
            let idx = if line == self.current() {
                self.current_x()
            } else {
                0
            };
            nrows -= i32_of(i64_of(self.update_line(line, idx)));
            l = self.next(line);
        }
    }

    pub fn get_softwrap_breakpoint(
        &mut self,
        linedata: &[u8],
        leftedge: usize,
        kickoff: &mut bool,
        end_of_line: &mut bool,
    ) -> usize {
        let mut sw = self.sw_state;
        let r = self.softwrap_breakpoint(linedata, leftedge, kickoff, end_of_line, &mut sw);
        self.sw_state = sw;
        r
    }

    pub fn softwrap_breakpoint(
        &self,
        linedata: &[u8],
        leftedge: usize,
        kickoff: &mut bool,
        end_of_line: &mut bool,
        sw: &mut SoftwrapState,
    ) -> usize {
        let rightside = leftedge + self.editwincols;
        let mut breaking_col = rightside;
        let mut last_blank_col = 0usize;
        let mut farthest_blank: Option<usize> = None;
        if *kickoff {
            sw.pos = 0;
            sw.column = 0;
            *kickoff = false;
        }
        let mut text = sw.pos.min(linedata.len());
        let mut column = sw.column;
        while text < linedata.len() && column < leftedge {
            text += self.advance_over(&linedata[text..], &mut column);
        }
        while text < linedata.len() && column <= rightside {
            if self.isset(Flag::AtBlanks) && is_blank_char(&linedata[text..]) && column < rightside
            {
                farthest_blank = Some(text);
                last_blank_col = column;
            }
            breaking_col = if linedata[text] == b'\t' {
                rightside
            } else {
                column
            };
            text += self.advance_over(&linedata[text..], &mut column);
        }
        if column <= rightside {
            *end_of_line = column < rightside;
            sw.pos = text;
            sw.column = column;
            return column;
        }
        if let Some(fb) = farthest_blank {
            let mut aftertheblank = last_blank_col;
            let onestep = self.advance_over(&linedata[fb..], &mut aftertheblank);
            if aftertheblank <= rightside {
                sw.pos = fb + onestep;
                sw.column = aftertheblank;
                return aftertheblank;
            }
            if linedata[fb] == b'\t' {
                breaking_col = rightside;
            }
        }
        sw.pos = text;
        sw.column = column;
        if self.editwincols > 1 {
            breaking_col
        } else {
            column - 1
        }
    }

    pub fn get_chunk_and_edge(
        &self,
        column: usize,
        line: Lp,
        leftedge: Option<&mut usize>,
    ) -> usize {
        let mut current_chunk = 0usize;
        let mut end_of_line = false;
        let mut kickoff = true;
        let mut start_col = 0usize;
        let mut sw = SoftwrapState::default();
        let data = &self.nodes[line].data;
        loop {
            let end_col =
                self.softwrap_breakpoint(data, start_col, &mut kickoff, &mut end_of_line, &mut sw);
            if end_of_line || (start_col <= column && column < end_col) {
                if let Some(le) = leftedge {
                    *le = start_col;
                }
                return current_chunk;
            }
            start_col = end_col;
            current_chunk += 1;
        }
    }

    pub fn extra_chunks_in(&self, line: Lp) -> usize {
        self.get_chunk_and_edge(usize::MAX, line, None)
    }

    pub fn chunk_for(&self, column: usize, line: Lp) -> usize {
        self.get_chunk_and_edge(column, line, None)
    }

    pub fn leftedge_for(&self, column: usize, line: Lp) -> usize {
        let mut leftedge = 0;
        self.get_chunk_and_edge(column, line, Some(&mut leftedge));
        leftedge
    }

    pub fn ensure_firstcolumn_is_aligned(&mut self) {
        if self.isset(Flag::SoftWrap) {
            let fc = self.leftedge_for(self.ofile().firstcolumn, self.edittop());
            self.ofile_mut().firstcolumn = fc;
        } else {
            self.ofile_mut().firstcolumn = 0;
        }
        self.focusing = false;
    }

    pub fn actual_last_column(&self, leftedge: usize, mut column: usize) -> usize {
        if self.isset(Flag::SoftWrap) {
            let mut kickoff = true;
            let mut last_chunk = false;
            let mut sw = SoftwrapState::default();
            let mut end_col = self.softwrap_breakpoint(
                &self.nodes[self.current()].data,
                leftedge,
                &mut kickoff,
                &mut last_chunk,
                &mut sw,
            ) - leftedge;
            if !last_chunk {
                end_col -= 1;
            }
            if column > end_col {
                column = end_col;
            }
        }
        leftedge + column
    }

    pub fn current_is_above_screen(&self) -> bool {
        if self.isset(Flag::SoftWrap) {
            self.lineno(self.current()) < self.lineno(self.edittop())
                || (self.lineno(self.current()) == self.lineno(self.edittop())
                    && self.xplustabs() < self.ofile().firstcolumn)
        } else {
            self.lineno(self.current()) < self.lineno(self.edittop())
        }
    }

    fn shim(&self) -> usize {
        usize::from(
            self.isset(Flag::Zero) && (self.currmenu == MREPLACEWITH || self.currmenu == MYESNO),
        )
    }

    pub fn current_is_below_screen(&self) -> bool {
        if self.isset(Flag::SoftWrap) {
            let mut line = self.edittop();
            let mut leftedge = self.ofile().firstcolumn;
            self.go_forward_chunks(
                i32_of(i64_of(self.editwinrows)) - 1 - i32_of(i64_of(self.shim())),
                &mut line,
                &mut leftedge,
            ) == 0
                && (self.lineno(line) < self.lineno(self.current())
                    || (self.lineno(line) == self.lineno(self.current())
                        && leftedge < self.leftedge_for(self.xplustabs(), self.current())))
        } else {
            self.lineno(self.current())
                >= self.lineno(self.edittop()) + i64_of(self.editwinrows) - i64_of(self.shim())
        }
    }

    pub fn current_is_offscreen(&self) -> bool {
        self.current_is_above_screen() || self.current_is_below_screen()
    }

    pub fn edit_redraw(&mut self, old_current: Lp, manner: Update) {
        let was_pww = self.ofile().placewewant;
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
        if self.current_is_offscreen() {
            self.adjust_viewport(if self.isset(Flag::JumpyScrolling) {
                Update::Centering
            } else {
                manner
            });
            self.refresh_needed = true;
            return;
        } else if self.united_sidescroll && self.ofile().brink != self.get_page_start(pww) {
            self.refresh_needed = true;
            return;
        }
        if self.ofile().mark.is_some() {
            let mut line = old_current;
            let mut guard = 0;
            while line != self.current() && guard < 1_000_000 {
                self.update_line(line, 0);
                line = if self.lineno(line) > self.lineno(self.current()) {
                    match self.prev(line) {
                        Some(p) => p,
                        None => break,
                    }
                } else {
                    match self.next(line) {
                        Some(n) => n,
                        None => break,
                    }
                };
                guard += 1;
            }
        } else if old_current != self.current() && self.get_page_start(was_pww) > 0 {
            self.update_line(old_current, 0);
        }
        if self.line_needs_update(was_pww, pww)
            || (old_current != self.current() && self.get_page_start(pww) > 0)
        {
            let (c, x) = (self.current(), self.current_x());
            self.update_line(c, x);
        }
    }

    pub fn edit_refresh(&mut self) {
        if self.current_is_offscreen() {
            self.adjust_viewport(if self.focusing || self.isset(Flag::JumpyScrolling) {
                Update::Centering
            } else {
                Update::Flowing
            });
        }
        if self.united_sidescroll {
            let b = self.get_page_start(self.xplustabs());
            self.ofile_mut().brink = b;
        }
        if self.sidebar > 0 {
            self.draw_scrollbar();
        }
        let mut row = 0usize;
        let mut line = Some(self.edittop());
        while row < self.editwinrows {
            let Some(l) = line else { break };
            let idx = if l == self.current() {
                self.current_x()
            } else {
                0
            };
            row += self.update_line(l, idx);
            line = self.next(l);
        }
        let cols = self.cols();
        while row < self.editwinrows {
            self.term.blank_row(Win::Mid, row);
            if self.sidebar > 0 && row < self.bardata.len() {
                let a = self.bardata[row];
                self.term.wmove(Win::Mid, row, cols - 1);
                self.term.waddch_attr(Win::Mid, ' ', a);
            }
            row += 1;
        }
        self.place_the_cursor();
        self.term.wnoutrefresh(Win::Mid);
        self.refresh_needed = false;
    }

    pub fn adjust_viewport(&mut self, manner: Update) {
        let goal: i32 = match manner {
            Update::Stationary => i32_of(self.ofile().cursor_row),
            Update::Centering => i32_of(i64_of(self.editwinrows / 2)),
            Update::Flowing => {
                if self.current_is_above_screen() {
                    0
                } else {
                    i32_of(i64_of(self.editwinrows)) - 1 - i32_of(i64_of(self.shim()))
                }
            }
        };
        let cur = self.current();
        self.ofile_mut().edittop = cur;
        if self.isset(Flag::SoftWrap) {
            let fc = self.leftedge_for(self.xplustabs(), cur);
            self.ofile_mut().firstcolumn = fc;
        }
        let mut top = cur;
        let mut fc = self.ofile().firstcolumn;
        self.go_back_chunks(goal.max(0), &mut top, &mut fc);
        let of = self.ofile_mut();
        of.edittop = top;
        of.firstcolumn = fc;
    }

    pub fn full_refresh(&mut self) {
        self.term.redrawwin();
        self.term.doupdate();
    }

    pub fn do_credits(&mut self) {
        let with_interface = !self.isset(Flag::Zero);
        let with_help = !self.isset(Flag::NoHelp);
        let version = crate::NANO_VERSION;
        let credits: [Option<&str>; 54] = [
            None,
            None,
            Some(version),
            Some(""),
            None,
            Some("Chris Allegretta"),
            Some("Benno Schulenberg"),
            Some("David Lawrence Ramsey"),
            Some("Jordi Mallach"),
            Some("David Benbennick"),
            Some("Rocco Corsi"),
            Some("Mike Frysinger"),
            Some("Adam Rogoyski"),
            Some("Rob Siemborski"),
            Some("Mark Majeres"),
            Some("Ken Tyler"),
            Some("Sven Guckes"),
            Some("Bill Soudan"),
            Some("Christian Weisgerber"),
            Some("Erik Andersen"),
            Some("Big Gaute"),
            Some("Joshua Jensen"),
            Some("Ryan Krebs"),
            Some("Albert Chin"),
            Some(""),
            None,
            Some("Monique, Brielle & Joseph"),
            Some("Plattsburgh State University"),
            Some("Benet Laboratories"),
            Some("Amy Allegretta"),
            Some("Linda Young"),
            Some("Jeremy Robichaud"),
            Some("Richard Kolb II"),
            None,
            Some("Linus Torvalds"),
            None,
            None,
            Some("Thomas Dickey"),
            Some("Pavel Curtis"),
            Some("Zeyd Ben-Halim"),
            Some("Eric S. Raymond"),
            None,
            Some(""),
            Some(""),
            None,
            Some(""),
            Some(""),
            Some("(C) 2026"),
            Some("Free Software Foundation, Inc."),
            Some(""),
            Some(""),
            Some("https://nano-editor.org/"),
            Some(""),
            Some(""),
        ];
        let xlcredits = [
            "Der Texteditor Nano",
            "Version",
            "Entwickelt von:",
            "Speziellen Dank an:",
            "The Free Software Foundation",
            "die zahlreichen Übersetzer und das TP",
            "Für ncurses:",
            "und alle Nichtgenannten …",
            "Danke für die Benutzung von nano!",
        ];
        let credit_len = 52;
        if with_interface || with_help {
            self.flags.set(Flag::Zero);
            self.flags.set(Flag::NoHelp);
            self.window_init();
        }
        self.blank_edit();
        self.term.wrefresh(Win::Mid);
        self.term.napms(600);
        let mut xlpos = 0;
        let rows = self.editwinrows;
        let pressed = |k: &mut Self| -> bool {
            k.term.wrefresh(Win::Mid);
            if k.term.wait_for_input(Duration::from_millis(0)) == Wait::Ready {
                k.term.read_available(&mut |_| {});
                return true;
            }
            false
        };
        for crpos in 0..credit_len + rows / 2 {
            if let Some(credit) = credits.get(crpos) {
                let text: &str = credit.map_or_else(
                    || {
                        let t = xlcredits[xlpos];
                        xlpos += 1;
                        t
                    },
                    |t| t,
                );
                let x = self.cols().saturating_sub(self.breadth(text.as_bytes())) / 2;
                self.term.mvwaddstr(Win::Mid, rows - 1, x, text);
                self.term.wrefresh(Win::Mid);
            }
            if pressed(self) {
                break;
            }
            self.term.napms(600);
            self.term.wscrl(Win::Mid, 1);
            self.term.wrefresh(Win::Mid);
            if pressed(self) {
                break;
            }
            self.term.napms(600);
            self.term.wscrl(Win::Mid, 1);
            self.term.wrefresh(Win::Mid);
        }
        if with_interface {
            self.flags.unset(Flag::Zero);
        }
        if with_help {
            self.flags.unset(Flag::NoHelp);
        }
        self.window_init();
        self.draw_all_subwindows();
    }

    pub fn draw_all_subwindows(&mut self) {
        if self.currmenu & !(MBROWSER | MGOTODIR | MWHEREISFILE) != 0 {
            let t = self.title.clone();
            self.titlebar(t.as_deref());
        }
        if self.inhelp {
            self.close_buffer();
            self.wrap_help_text_into_buffer();
        } else if self.currmenu & (MBROWSER | MGOTODIR | MWHEREISFILE) != 0 {
            self.browser_refresh();
        } else {
            self.edit_refresh();
        }
        let m = self.currmenu;
        self.bottombars(m);
    }

    pub fn report_cursor_position(&mut self) {
        let cur = self.current();
        let fullwidth = self.breadth(self.data(cur)) + 1;
        let column = self.xplustabs() + 1;
        let mut sum = 0usize;
        let mut line = Some(self.filetop());
        while let Some(l) = line {
            if l == cur {
                break;
            }
            sum += mbstrlen(self.data(l)) + 1;
            line = self.next(l);
        }
        sum += mbstrlen(&self.nodes[cur].data[..self.current_x()]);
        let totsize = self.ofile().totsize;
        let linepct = 100 * self.lineno(cur) / self.lineno(self.filebot()).max(1);
        let colpct = 100 * column / fullwidth;
        let charpct = (100 * sum).checked_div(totsize).unwrap_or(0);
        let msg = fmt!(
            "Zeile %*zd/%zd (%2d%%), Spalte %2zu/%2zu (%3d%%), Zeichen %*zu/%zu (%2d%%)",
            i64_of(digits(self.lineno(self.filebot()))),
            self.lineno(cur),
            self.lineno(self.filebot()),
            linepct,
            column,
            fullwidth,
            colpct,
            i64_of(digits(i64_of(totsize))),
            sum,
            totsize,
            charpct
        );
        self.statusline(Msg::Info, &msg);
    }

    pub fn spotlight(&mut self, from_col: usize, mut to_col: usize) {
        let right_edge = self.get_page_start(from_col) + self.editwincols;
        let overshoots = to_col > right_edge;
        self.place_the_cursor();
        if overshoots {
            to_col = right_edge;
        }
        let word: Vec<u8> = if to_col == from_col {
            to_col += 1;
            vec![b' ']
        } else {
            let d = self.nodes[self.current()].data.clone();
            self.display_string(&d, from_col, to_col - from_col, false, overshoots)
        };
        let sp = self.interface_color_pair[Elem::Spotlighted as usize];
        self.term.wattron(Win::Mid, sp);
        let n = self.actual_x(&word, to_col);
        self.term.waddbytes(Win::Mid, &word[..n]);
        if overshoots {
            let row = usize_of(self.ofile().cursor_row.max(0));
            self.term
                .mvwaddch(Win::Mid, row, self.cols() - 1 - self.sidebar, '>');
        }
        self.term.wattroff(Win::Mid, sp);
    }

    pub fn spotlight_softwrapped(&mut self, mut from_col: usize, to_col: usize) {
        let mut leftedge = self.leftedge_for(from_col, self.current());
        let mut end_of_line = false;
        let mut kickoff = true;
        self.place_the_cursor();
        let mut row = usize_of(self.ofile().cursor_row.max(0));
        let d = self.nodes[self.current()].data.clone();
        while row < self.editwinrows {
            let mut break_col =
                self.get_softwrap_breakpoint(&d, leftedge, &mut kickoff, &mut end_of_line);
            if break_col >= to_col {
                end_of_line = true;
                break_col = to_col;
            }
            let word: Vec<u8> = if break_col == from_col {
                break_col += 1;
                vec![b' ']
            } else {
                self.display_string(&d, from_col, break_col - from_col, false, false)
            };
            let sp = self.interface_color_pair[Elem::Spotlighted as usize];
            self.term.wattron(Win::Mid, sp);
            let n = self.actual_x(&word, break_col);
            self.term.waddbytes(Win::Mid, &word[..n]);
            self.term.wattroff(Win::Mid, sp);
            if end_of_line {
                break;
            }
            row += 1;
            self.term.wmove(Win::Mid, row, self.margin);
            leftedge = break_col;
            from_col = break_col;
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct SoftwrapState {
    pub pos: usize,
    pub column: usize,
}

pub const fn convert_to_control(kbinput: i32) -> i32 {
    if ('@' as i32) <= kbinput && kbinput <= ('_' as i32) {
        return kbinput - '@' as i32;
    }
    if ('`' as i32) <= kbinput && kbinput <= ('~' as i32) {
        return kbinput - '`' as i32;
    }
    if ('3' as i32) <= kbinput && kbinput <= ('7' as i32) {
        return kbinput - 24;
    }
    if kbinput == '?' as i32 || kbinput == '8' as i32 {
        return DEL_CODE;
    }
    if kbinput == ' ' as i32 || kbinput == '2' as i32 {
        return 0;
    }
    if kbinput == '/' as i32 {
        return 31;
    }
    kbinput
}

pub const fn buffer_number(idx: usize) -> usize {
    idx + 1
}
