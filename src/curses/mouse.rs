use crate::curses::terminfo::{PadInfo, TermInfo, tparm, tputs};

const fn mouse_mask(button: u32, m: u32) -> u32 {
    m << ((button - 1) * 5)
}

const MAX_BUTTONS: u32 = 5;
const EV_MAX: usize = 8;
const INVALID_EVENT: i16 = -1;
const NORMAL_EVENT: i16 = 0;
const DEFAULT_MAXCLICK: i32 = 166;

const fn mask_press(b: u32) -> u32 {
    mouse_mask(b, 0o2)
}
const fn mask_release(b: u32) -> u32 {
    mouse_mask(b, 0o1)
}
const fn mask_click(b: u32) -> u32 {
    mouse_mask(b, 0o4)
}
const fn mask_double_click(b: u32) -> u32 {
    mouse_mask(b, 0o10)
}
const fn mask_triple_click(b: u32) -> u32 {
    mouse_mask(b, 0o20)
}
const fn all_buttons(m: u32) -> u32 {
    mouse_mask(1, m) | mouse_mask(2, m) | mouse_mask(3, m) | mouse_mask(4, m) | mouse_mask(5, m)
}

const BUTTON_RELEASED: u32 = all_buttons(0o1);
const BUTTON_PRESSED: u32 = all_buttons(0o2);
const BUTTON_CLICKED: u32 = all_buttons(0o4);
const BUTTON_DOUBLE_CLICKED: u32 = all_buttons(0o10);
const BUTTON_TRIPLE_CLICKED: u32 = all_buttons(0o20);

pub const BUTTON1_RELEASED: u32 = mask_release(1);
pub const BUTTON1_CLICKED: u32 = mask_click(1);
pub const BUTTON4_PRESSED: u32 = mask_press(4);
pub const BUTTON5_PRESSED: u32 = mask_press(5);
const BUTTON_CTRL: u32 = mouse_mask(6, 0o1);
const BUTTON_SHIFT: u32 = mouse_mask(6, 0o2);
const BUTTON_ALT: u32 = mouse_mask(6, 0o4);
const REPORT_MOUSE_POSITION: u32 = mouse_mask(6, 0o10);
pub const ALL_MOUSE_EVENTS: u32 = REPORT_MOUSE_POSITION - 1;

#[derive(Clone, Copy, Debug, Default)]
pub struct MEvent {
    id: i16,
    pub x: i32,
    pub y: i32,
    pub bstate: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MouseType {
    None,
    Xterm,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MouseFormat {
    X10,
    Sgr1006,
}

struct SgrData {
    nerror: i32,
    params: [i32; 9],
    last: u8,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "mirrors the boolean mouse state kept by ncurses"
)]
pub struct Mouse {
    kmous: Vec<u8>,
    pad: PadInfo,
    xterm_named: bool,
    xm_string: Option<Vec<u8>>,
    xm_number: i32,
    initialized: bool,
    kind: MouseType,
    format: MouseFormat,
    xtermcap: Vec<u8>,
    bound: bool,
    mask: u32,
    mask2: u32,
    bstate: u32,
    events: [MEvent; EV_MAX],
    eventp: usize,
    pub maxclick: i32,
    wants_xterm_kmous: bool,
}

const fn next(ep: usize) -> usize {
    if ep >= EV_MAX - 1 { 0 } else { ep + 1 }
}

const fn prev(ep: usize) -> usize {
    if ep == 0 { EV_MAX - 1 } else { ep - 1 }
}

fn is_final(c: u8) -> bool {
    (0x40..=0x7E).contains(&c)
}

impl Mouse {
    pub fn new() -> Self {
        Self {
            kmous: Vec::new(),
            pad: PadInfo::default(),
            xterm_named: false,
            xm_string: None,
            xm_number: -1,
            initialized: false,
            kind: MouseType::None,
            format: MouseFormat::X10,
            xtermcap: Vec::new(),
            bound: false,
            mask: 0,
            mask2: 0,
            bstate: 0,
            events: [MEvent::default(); EV_MAX],
            eventp: 0,
            maxclick: DEFAULT_MAXCLICK,
            wants_xterm_kmous: false,
        }
    }

    pub fn configure(&mut self, ti: &TermInfo, pad: PadInfo) {
        self.pad = pad;
        self.kmous = ti.get("kmous").map_or_default(<[u8]>::to_vec);
        self.xterm_named = ti.names().contains("xterm");
        self.xm_string = ti.get("XM").map(<[u8]>::to_vec);
        self.xm_number = ti.extended_number("XM").unwrap_or(-1);
    }

    pub fn take_xterm_kmous_request(&mut self) -> bool {
        std::mem::take(&mut self.wants_xterm_kmous)
    }

    fn init_xterm_mouse(&mut self) {
        self.kind = MouseType::Xterm;
        self.format = MouseFormat::X10;
        if let Some(xm) = self.xm_string.clone() {
            self.xtermcap.clone_from(&xm);
            if let Some(found) = xm.windows(2).position(|w| w == b"[?") {
                let mut code = found + 2;
                while code < xm.len() && xm[code].is_ascii_digit() {
                    let mut next = code;
                    while next < xm.len() && xm[next].is_ascii_digit() {
                        next += 1;
                    }
                    if b"1006".starts_with(&xm[code..next]) {
                        self.format = MouseFormat::Sgr1006;
                    }
                    if next < xm.len() && xm[next] == b';' {
                        while next < xm.len() && xm[next] == b';' {
                            next += 1;
                        }
                        code = next;
                    } else {
                        break;
                    }
                }
            }
        } else if self.xm_number == 1006 {
            self.xtermcap = b"\x1B[?1006;1000%?%p1%{1}%=%th%el%;".to_vec();
            self.format = MouseFormat::Sgr1006;
        } else {
            self.xtermcap = b"\x1B[?1000%?%p1%{1}%=%th%el%;".to_vec();
        }
    }

    fn initialize_mousetype(&mut self) {
        if !self.kmous.is_empty() {
            self.init_xterm_mouse();
        } else if self.xterm_named {
            self.wants_xterm_kmous = true;
            self.init_xterm_mouse();
        }
    }

    fn init(&mut self) -> bool {
        if !self.initialized {
            self.initialized = true;
            self.eventp = 0;
            for ev in &mut self.events {
                ev.id = INVALID_EVENT;
            }
            self.initialize_mousetype();
        }
        self.initialized
    }

    fn enable_xterm_mouse(&self, enable: bool, out: &mut Vec<u8>) {
        tputs(
            out,
            &tparm(&self.xtermcap, &[i32::from(enable)]),
            false,
            &self.pad,
        );
    }

    fn activate(&mut self, on: bool, out: &mut Vec<u8>) {
        if !on && !self.initialized {
            return;
        }
        if !self.init() {
            return;
        }
        if on {
            self.bstate = 0;
            match self.kind {
                MouseType::Xterm => self.enable_xterm_mouse(true, out),
                MouseType::None => return,
            }
            self.bound = true;
        } else if self.kind == MouseType::Xterm {
            self.enable_xterm_mouse(false, out);
        }
    }

    pub fn mousemask(&mut self, newmask: u32, out: &mut Vec<u8>) -> u32 {
        let mut result = 0;
        if newmask != 0 || self.initialized {
            self.init();
            if self.kind != MouseType::None {
                result = newmask
                    & (REPORT_MOUSE_POSITION
                        | BUTTON_ALT
                        | BUTTON_CTRL
                        | BUTTON_SHIFT
                        | BUTTON_PRESSED
                        | BUTTON_RELEASED
                        | BUTTON_CLICKED
                        | BUTTON_DOUBLE_CLICKED
                        | BUTTON_TRIPLE_CLICKED);
                self.activate(result != 0, out);
                self.mask = result;
                self.mask2 = result;
                for b in 1..=MAX_BUTTONS {
                    if self.mask2 & mask_triple_click(b) != 0 {
                        self.mask2 |= mask_double_click(b);
                    }
                    if self.mask2 & mask_double_click(b) != 0 {
                        self.mask2 |= mask_click(b);
                    }
                    if self.mask2 & mask_click(b) != 0 {
                        self.mask2 |= mask_press(b) | mask_release(b);
                    }
                }
            }
        }
        result
    }

    pub const fn mouseinterval(&mut self, maxclick: i32) -> i32 {
        let old = self.maxclick;
        if maxclick >= 0 {
            self.maxclick = maxclick;
        }
        old
    }

    pub fn wrap(&mut self, out: &mut Vec<u8>) {
        if self.kind == MouseType::Xterm && self.mask != 0 {
            self.activate(false, out);
        }
    }

    pub fn resume(&mut self, out: &mut Vec<u8>) {
        if self.kind == MouseType::Xterm && self.mask != 0 {
            self.activate(true, out);
        }
    }

    const fn press_position(&mut self, ep: usize, n: u32) {
        self.events[ep].bstate = if self.bstate & mask_press(n) != 0 {
            REPORT_MOUSE_POSITION
        } else {
            mask_press(n)
        };
        self.bstate |= mask_press(n);
    }

    const fn handle_wheel(&mut self, ep: usize, button: i32, wheel: bool) -> bool {
        let mut result = true;
        match button & 3 {
            0 => {
                if wheel {
                    self.events[ep].bstate = mask_press(4);
                } else {
                    self.press_position(ep, 1);
                }
            }
            1 => {
                if wheel {
                    self.events[ep].bstate = mask_press(5);
                } else {
                    self.press_position(ep, 2);
                }
            }
            2 => self.press_position(ep, 3),
            _ => {
                self.events[ep].bstate = REPORT_MOUSE_POSITION;
                result = false;
            }
        }
        result
    }

    fn decode_x10_bstate(&mut self, ep: usize, intro: u32) -> bool {
        let wheel = (intro & 0x60) == 0x60;
        self.events[ep].bstate = 0;
        let button: i32 = if intro >= 96 {
            if intro >= 160 {
                intro.cast_signed() - 152
            } else {
                intro.cast_signed() - 92
            }
        } else {
            (intro & 3).cast_signed()
        };
        if button > MAX_BUTTONS.cast_signed() {
            self.events[ep].bstate = REPORT_MOUSE_POSITION;
        } else if !self.handle_wheel(ep, intro.cast_signed(), wheel) {
            if self.bstate & BUTTON_PRESSED != 0 {
                let mut state = BUTTON_RELEASED;
                for b in 1..=MAX_BUTTONS {
                    if self.bstate & mask_press(b) == 0 {
                        state &= !mask_release(b);
                    }
                }
                self.events[ep].bstate = state;
                self.bstate = 0;
            } else {
                self.events[ep].bstate = REPORT_MOUSE_POSITION;
            }
        }
        if intro & 4 != 0 {
            self.events[ep].bstate |= BUTTON_SHIFT;
        }
        if intro & 8 != 0 {
            self.events[ep].bstate |= BUTTON_ALT;
        }
        if intro & 16 != 0 {
            self.events[ep].bstate |= BUTTON_CTRL;
        }
        self.events[ep].bstate & REPORT_MOUSE_POSITION != 0
    }

    fn decode_xterm_x10(&mut self, ep: usize, read: &mut dyn FnMut() -> Option<u8>) -> bool {
        let mut kbuf = [0u8; 3];
        for slot in &mut kbuf {
            match read() {
                Some(b) => *slot = b,
                None => break,
            }
        }
        self.events[ep].id = NORMAL_EVENT;
        let result = self.decode_x10_bstate(ep, u32::from(kbuf[0]));
        self.events[ep].x = (i32::from(kbuf[1]) - i32::from(b' ')) - 1;
        self.events[ep].y = (i32::from(kbuf[2]) - i32::from(b' ')) - 1;
        result
    }

    fn read_sgr(read: &mut dyn FnMut() -> Option<u8>) -> (SgrData, bool) {
        let mut data = SgrData {
            nerror: 0,
            params: [0; 9],
            last: 0,
        };
        let mut grabbed = 0usize;
        let mut now: i32 = -1;
        let mut marker = true;
        while let Some(ch) = read() {
            if grabbed + 3 >= 80 {
                data.nerror += 1;
                break;
            }
            match ch {
                b'0'..=b'9' => {
                    if marker {
                        now += 1;
                    }
                    marker = false;
                    if let Some(p) = usize::try_from(now)
                        .ok()
                        .and_then(|n| data.params.get_mut(n))
                    {
                        *p = p.wrapping_mul(10).wrapping_add(i32::from(ch - b'0'));
                    }
                }
                b';' => {
                    if marker {
                        now += 1;
                    }
                    marker = true;
                }
                _ => {
                    if !(32..=126).contains(&ch) {
                        data.nerror += 1;
                        continue;
                    } else if is_final(ch) {
                        data.last = ch;
                    } else {
                        data.nerror += 1;
                    }
                }
            }
            grabbed += 1;
            if is_final(ch) {
                break;
            }
        }
        let ok = grabbed > 0 && data.nerror == 0;
        (data, ok)
    }

    fn decode_xterm_sgr1006(&mut self, ep: usize, read: &mut dyn FnMut() -> Option<u8>) -> bool {
        let (data, ok) = Self::read_sgr(read);
        if !ok {
            return false;
        }
        let b = data.params[0];
        let mut b3 = 1 + (b & 3);
        let wheel = (b & 64) == 64;
        if b >= 132 {
            b3 = MAX_BUTTONS.cast_signed() + 1;
        } else if b >= 128 {
            b3 = b - 120;
        } else if b >= 64 {
            b3 = b - 60;
        }
        self.events[ep].id = NORMAL_EVENT;
        if data.last == b'M' {
            self.handle_wheel(ep, b, wheel);
        } else if b3 > MAX_BUTTONS.cast_signed() {
            self.events[ep].bstate = REPORT_MOUSE_POSITION;
        } else {
            let pressed = mouse_mask(b3.cast_unsigned(), 0o2);
            let release = mouse_mask(b3.cast_unsigned(), 0o1);
            if self.bstate & pressed != 0 {
                self.events[ep].bstate = release;
                self.bstate &= !pressed;
            } else {
                self.events[ep].bstate = REPORT_MOUSE_POSITION;
            }
        }
        if b & 4 != 0 {
            self.events[ep].bstate |= BUTTON_SHIFT;
        }
        if b & 8 != 0 {
            self.events[ep].bstate |= BUTTON_ALT;
        }
        if b & 16 != 0 {
            self.events[ep].bstate |= BUTTON_CTRL;
        }
        let result = self.events[ep].bstate & REPORT_MOUSE_POSITION != 0;
        self.events[ep].x = if data.params[1] != 0 {
            data.params[1] - 1
        } else {
            0
        };
        self.events[ep].y = if data.params[2] != 0 {
            data.params[2] - 1
        } else {
            0
        };
        result
    }

    pub fn inline_event(&mut self, read: &mut dyn FnMut() -> Option<u8>) -> bool {
        if !self.bound {
            return false;
        }
        let mut result = false;
        let ep = self.eventp;
        if self.kind == MouseType::Xterm {
            result = match self.format {
                MouseFormat::X10 => self.decode_xterm_x10(ep, read),
                MouseFormat::Sgr1006 => self.decode_xterm_sgr1006(ep, read),
            };
            self.eventp = next(ep);
            if !result {
                let state = self.events[ep].bstate;
                if state & BUTTON_PRESSED != 0 {
                    for b in 4..=MAX_BUTTONS {
                        if state & mask_press(b) != 0 {
                            result = true;
                            break;
                        }
                    }
                }
            }
        }
        result
    }

    const fn valid(&self, ep: usize) -> bool {
        self.events[ep].id != INVALID_EVENT
    }

    const fn invalidate(&mut self, ep: usize) {
        self.events[ep].id = INVALID_EVENT;
    }

    pub fn parse(&mut self, runcount: usize) -> bool {
        if !self.bound {
            return true;
        }
        let mut eventp = self.eventp;
        let mut first_valid: Option<usize> = None;
        let mut first_invalid: Option<usize> = None;

        let mut ep = eventp;
        for _ in runcount..EV_MAX {
            self.invalidate(ep);
            ep = next(ep);
        }

        let mut end_loop = false;
        while !end_loop {
            let nx = next(ep);
            if nx == eventp {
                end_loop = true;
            } else {
                let (e, n) = (self.events[ep], self.events[nx]);
                if self.valid(ep)
                    && self.valid(nx)
                    && e.x == n.x
                    && e.y == n.y
                    && e.bstate & BUTTON_PRESSED != 0
                    && n.bstate & BUTTON_PRESSED == 0
                {
                    let mut changed = true;
                    for b in 1..=MAX_BUTTONS {
                        if (e.bstate & mask_press(b) == 0) != (n.bstate & mask_release(b) == 0) {
                            changed = false;
                            break;
                        }
                    }
                    if changed {
                        let mut merge = false;
                        for b in 1..=MAX_BUTTONS {
                            if self.mask2 & mask_click(b) != 0 && e.bstate & mask_press(b) != 0 {
                                self.events[nx].bstate &= !mask_release(b);
                                self.events[nx].bstate |= mask_click(b);
                                merge = true;
                            }
                        }
                        if merge {
                            self.invalidate(ep);
                        }
                    }
                }
            }
            if !self.valid(ep) {
                if first_valid.is_some() && first_invalid.is_none() {
                    first_invalid = Some(ep);
                }
            } else if first_valid.is_none() {
                first_valid = Some(ep);
            } else if let Some(fi) = first_invalid {
                self.events[fi] = self.events[ep];
                self.invalidate(ep);
                first_invalid = Some(next(fi));
            }
            ep = nx;
        }
        if let Some(fi) = first_invalid {
            eventp = fi;
        }

        first_invalid = None;
        let mut end_loop = first_valid.is_none();
        let mut ep = first_valid.unwrap_or(0);
        while !end_loop {
            let nx = next(ep);
            if nx == eventp {
                end_loop = true;
            } else if !self.valid(nx) {
                break;
            } else {
                let (e, n) = (self.events[ep], self.events[nx]);
                if e.bstate & BUTTON_CLICKED != 0 && n.bstate & BUTTON_CLICKED != 0 {
                    let mut merge = false;
                    for b in 1..=MAX_BUTTONS {
                        if self.mask2 & mask_double_click(b) != 0
                            && e.bstate & mask_click(b) != 0
                            && n.bstate & mask_click(b) != 0
                        {
                            self.events[nx].bstate &= !mask_click(b);
                            self.events[nx].bstate |= mask_double_click(b);
                            merge = true;
                        }
                    }
                    if merge {
                        self.invalidate(ep);
                    }
                }
                let (e, n) = (self.events[ep], self.events[nx]);
                if e.bstate & BUTTON_DOUBLE_CLICKED != 0 && n.bstate & BUTTON_CLICKED != 0 {
                    let mut merge = false;
                    for b in 1..=MAX_BUTTONS {
                        if self.mask2 & mask_triple_click(b) != 0
                            && e.bstate & mask_double_click(b) != 0
                            && n.bstate & mask_click(b) != 0
                        {
                            self.events[nx].bstate &= !mask_click(b);
                            self.events[nx].bstate |= mask_triple_click(b);
                            merge = true;
                        }
                    }
                    if merge {
                        self.invalidate(ep);
                    }
                }
            }
            if self.events[ep].bstate & self.mask2 == 0 {
                self.invalidate(ep);
            }
            if !self.valid(ep) {
                if Some(ep) == first_valid {
                    first_valid = Some(nx);
                } else if first_invalid.is_none() {
                    first_invalid = Some(ep);
                }
            } else if let Some(fi) = first_invalid {
                self.events[fi] = self.events[ep];
                self.invalidate(ep);
                first_invalid = Some(next(fi));
            }
            ep = nx;
        }
        let first_invalid = first_invalid.unwrap_or(eventp);
        self.eventp = first_invalid;
        let last = prev(first_invalid);
        self.valid(last) && self.events[last].bstate & self.mask != 0
    }

    pub fn getmouse(&mut self) -> Option<MEvent> {
        if self.kind == MouseType::None {
            return None;
        }
        let mut p = prev(self.eventp);
        while self.valid(p) && self.events[p].bstate & self.mask2 == 0 {
            self.invalidate(p);
            p = prev(p);
        }
        if self.valid(p) {
            let event = self.events[p];
            self.invalidate(p);
            self.eventp = p;
            Some(event)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(bytes: &[u8]) -> impl FnMut() -> Option<u8> + '_ {
        let mut i = 0;
        move || {
            let b = bytes.get(i).copied();
            i += 1;
            b
        }
    }

    fn xterm_sgr() -> Mouse {
        let mut m = Mouse::new();
        m.kmous = b"\x1B[<".to_vec();
        m.xm_string = Some(b"\x1B[?1006;1000%?%p1%{1}%=%th%el%;".to_vec());
        m
    }

    #[test]
    fn click_from_press_and_release() {
        let mut m = xterm_sgr();
        let mut out = Vec::new();
        assert_eq!(m.mousemask(ALL_MOUSE_EVENTS, &mut out), ALL_MOUSE_EVENTS);
        assert_eq!(out, b"\x1B[?1006;1000h");
        assert!(!m.inline_event(&mut feed(b"0;5;3M")));
        assert!(!m.inline_event(&mut feed(b"0;5;3m")));
        assert!(m.parse(2));
        let ev = m.getmouse().unwrap();
        assert_eq!((ev.x, ev.y), (4, 2));
        assert_eq!(ev.bstate, BUTTON1_CLICKED);
        assert!(m.getmouse().is_none());
    }

    #[test]
    fn wheel_reports_at_once_and_masked_off_nothing() {
        let mut m = xterm_sgr();
        let mut out = Vec::new();
        assert!(!m.inline_event(&mut feed(b"64;1;1M")));
        assert!(m.parse(1));
        assert!(m.getmouse().is_none());
        m.mousemask(ALL_MOUSE_EVENTS, &mut out);
        assert!(m.inline_event(&mut feed(b"65;7;2M")));
        assert_eq!(m.getmouse().unwrap().bstate, BUTTON5_PRESSED);
        out.clear();
        m.mousemask(0, &mut out);
        assert_eq!(out, b"\x1B[?1006;1000l");
        assert!(!m.inline_event(&mut feed(b"0;5;3M")));
        assert!(!m.parse(1));
        assert!(m.getmouse().is_none());
    }

    #[test]
    fn x10_without_xm() {
        let mut m = Mouse::new();
        m.kmous = b"\x1B[M".to_vec();
        let mut out = Vec::new();
        m.mousemask(ALL_MOUSE_EVENTS, &mut out);
        assert_eq!(out, b"\x1B[?1000h");
        assert!(!m.inline_event(&mut feed(b" !!")));
        assert!(!m.inline_event(&mut feed(b"#!!")));
        assert!(m.parse(2));
        let ev = m.getmouse().unwrap();
        assert_eq!((ev.x, ev.y, ev.bstate), (0, 0, BUTTON1_CLICKED));
    }

    #[test]
    fn no_mouse_without_kmous() {
        let mut m = Mouse::new();
        let mut out = Vec::new();
        assert_eq!(m.mousemask(ALL_MOUSE_EVENTS, &mut out), 0);
        assert_eq!(out, [] as [u8; 0]);
        assert!(m.getmouse().is_none());
        let mut m = Mouse::new();
        m.xterm_named = true;
        assert_eq!(m.mousemask(ALL_MOUSE_EVENTS, &mut out), ALL_MOUSE_EVENTS);
        assert!(m.take_xterm_kmous_request());
        assert_eq!(out, b"\x1B[?1000h");
    }
}
