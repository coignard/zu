pub mod mouse;
pub mod terminfo;

use crate::curses::terminfo::{PadInfo, tputs};
use std::time::Duration;

use zeroize::Zeroize;

pub static IN_CURSES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static FOCUS_REPORTS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub const A_NORMAL: u8 = 0;
pub const A_REVERSE: u8 = 1;
pub const A_BOLD: u8 = 2;
pub const A_ITALIC: u8 = 4;
pub const A_UNDERLINE: u8 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Attr {
    pub fg: i16,
    pub bg: i16,
    pub bits: u8,
}

impl Attr {
    pub const NORMAL: Self = Self {
        fg: -1,
        bg: -1,
        bits: A_NORMAL,
    };
    pub const REVERSE: Self = Self {
        fg: -1,
        bg: -1,
        bits: A_REVERSE,
    };
    pub const BOLD: Self = Self {
        fg: -1,
        bg: -1,
        bits: A_BOLD,
    };

    pub const fn bits(bits: u8) -> Self {
        Self {
            fg: -1,
            bg: -1,
            bits,
        }
    }

    pub const fn colored(fg: i16, bg: i16, bits: u8) -> Self {
        Self { fg, bg, bits }
    }

    pub const fn or(self, other: Self) -> Self {
        Self {
            fg: if other.fg >= 0 { other.fg } else { self.fg },
            bg: if other.bg >= 0 { other.bg } else { self.bg },
            bits: self.bits | other.bits,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Cell {
    text: String,
    attr: Attr,
    cont: bool,
}

impl Cell {
    fn blank() -> Self {
        Self {
            text: String::from(" "),
            attr: Attr::NORMAL,
            cont: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Win {
    Top,
    Mid,
    Foot,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wait {
    Ready,
    Timeout,
    Interrupted,
    Hangup,
}

pub fn terminal_size() -> Option<(usize, usize)> {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        for fd in [1, 0, 2] {
            if libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_row > 0 && ws.ws_col > 0 {
                return Some((ws.ws_row as usize, ws.ws_col as usize));
            }
        }
    }
    None
}

const SPEEDS: &[(libc::speed_t, i32)] = &[
    (libc::B0, 0),
    (libc::B50, 50),
    (libc::B75, 75),
    (libc::B110, 110),
    (libc::B134, 134),
    (libc::B150, 150),
    (libc::B200, 200),
    (libc::B300, 300),
    (libc::B600, 600),
    (libc::B1200, 1200),
    (libc::B1800, 1800),
    (libc::B2400, 2400),
    (libc::B4800, 4800),
    (libc::B9600, 9600),
    (libc::B19200, 19200),
    #[cfg(target_vendor = "apple")]
    (libc::B28800, 28800),
    (libc::B38400, 38400),
    (libc::B57600, 57600),
    #[cfg(target_vendor = "apple")]
    (libc::B76800, 76800),
    (libc::B115200, 115_200),
    (libc::B230400, 230_400),
    #[cfg(target_os = "linux")]
    (libc::B460800, 460_800),
    #[cfg(target_os = "linux")]
    (libc::B500000, 500_000),
    #[cfg(target_os = "linux")]
    (libc::B576000, 576_000),
    #[cfg(target_os = "linux")]
    (libc::B921600, 921_600),
    #[cfg(target_os = "linux")]
    (libc::B1000000, 1_000_000),
    #[cfg(target_os = "linux")]
    (libc::B1152000, 1_152_000),
    #[cfg(target_os = "linux")]
    (libc::B1500000, 1_500_000),
    #[cfg(target_os = "linux")]
    (libc::B2000000, 2_000_000),
    #[cfg(target_os = "linux")]
    (libc::B2500000, 2_500_000),
    #[cfg(target_os = "linux")]
    (libc::B3000000, 3_000_000),
    #[cfg(target_os = "linux")]
    (libc::B3500000, 3_500_000),
    #[cfg(target_os = "linux")]
    (libc::B4000000, 4_000_000),
];

fn speed_to_baud(speed: libc::speed_t) -> i32 {
    SPEEDS
        .iter()
        .find(|&&(given, _)| given == speed)
        .map_or(-1, |&(_, actual)| actual)
}

fn baud_to_speed(baud: i64) -> libc::speed_t {
    SPEEDS
        .iter()
        .find(|&&(_, actual)| i64::from(actual) == baud)
        .map_or(1, |&(given, _)| given)
}

pub fn baudrate() -> i32 {
    let stdout_is_tty = unsafe { libc::isatty(1) } == 1;
    if !stdout_is_tty && std::env::var_os("BAUDRATE").is_some() {
        let wanted = getenv_num("BAUDRATE");
        let wanted = if wanted <= 0 { 9600 } else { wanted };
        return speed_to_baud(baud_to_speed(wanted));
    }
    let fd = if stdout_is_tty { 1 } else { 2 };
    let mut settings = std::mem::MaybeUninit::<libc::termios>::zeroed();
    let speed = if unsafe { libc::tcgetattr(fd, settings.as_mut_ptr()) } == 0 {
        unsafe { libc::cfgetospeed(settings.as_ptr()) }
    } else {
        libc::B0
    };
    speed_to_baud(speed)
}

struct Probe {
    count: i32,
    readable: bool,
    gone: bool,
}

#[cfg(not(target_vendor = "apple"))]
fn probe(fd: i32, input: bool, ms: i32) -> Probe {
    let mut pfd = libc::pollfd {
        fd,
        events: if input { libc::POLLIN } else { 0 },
        revents: 0,
    };
    let count = unsafe { libc::poll(&raw mut pfd, 1, ms) };
    Probe {
        count,
        readable: count > 0 && pfd.revents & libc::POLLIN != 0,
        gone: count > 0 && pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0,
    }
}

#[cfg(target_vendor = "apple")]
fn probe(fd: i32, input: bool, ms: i32) -> Probe {
    let quiet = Probe {
        count: 0,
        readable: false,
        gone: false,
    };
    if !input {
        return quiet;
    }
    if usize::try_from(fd).map_or(true, |f| f >= libc::FD_SETSIZE) {
        return Probe { count: -1, ..quiet };
    }
    let mut set = std::mem::MaybeUninit::<libc::fd_set>::zeroed();
    let mut wait = libc::timeval {
        tv_sec: libc::time_t::from(ms / 1000),
        tv_usec: (ms % 1000) * 1000,
    };
    let count = unsafe {
        libc::FD_ZERO(set.as_mut_ptr());
        libc::FD_SET(fd, set.as_mut_ptr());
        libc::select(
            fd + 1,
            set.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            if ms < 0 {
                std::ptr::null_mut()
            } else {
                &raw mut wait
            },
        )
    };
    Probe {
        count,
        readable: count > 0 && unsafe { libc::FD_ISSET(fd, set.as_ptr()) },
        gone: false,
    }
}

pub fn getenv_num(name: &str) -> i64 {
    std::env::var_os(name).map_or(-1, |value| parse_env_num(&value.to_string_lossy()))
}

pub fn parse_env_num(text: &str) -> i64 {
    let body = text.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let (negative, digits) = match body.as_bytes().first() {
        Some(b'-') => (true, &body[1..]),
        Some(b'+') => (false, &body[1..]),
        _ => (false, body),
    };
    let (radix, digits) =
        if digits.len() > 1 && (digits.starts_with("0x") || digits.starts_with("0X")) {
            (16, &digits[2..])
        } else if digits.len() > 1 && digits.starts_with('0') {
            (8, &digits[1..])
        } else {
            (10, digits)
        };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return -1;
    }
    match i64::from_str_radix(digits, radix) {
        Ok(n) if !negative && i32::try_from(n).is_ok() => n,
        _ => -1,
    }
}

fn emit(sequence: &str) {
    crate::crypto::raw_write(1, sequence.as_bytes());
}

const ENTER_SCREEN: &str = "\x1B[?1049h\x1B[?2004h\x1B[0m\x1B[H\x1B[2J";

pub fn leave_sequence(rows: usize) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(24 + rows * 10);
    if FOCUS_REPORTS.swap(false, std::sync::atomic::Ordering::SeqCst) {
        out.extend_from_slice(b"\x1B[?1004l");
    }
    out.extend_from_slice(b"\x1B[?2004l\x1B[0m");
    for row in 1..=rows.max(1) {
        out.extend_from_slice(format!("\x1B[{row};1H\x1B[2K").as_bytes());
    }
    out.extend_from_slice(b"\x1B[H\x1B[?1049l\x1B[?25h");
    out
}

#[derive(Clone, Debug, Default)]
pub struct WinState {
    pub y0: usize,
    pub rows: usize,
    pub cur_y: usize,
    pub cur_x: usize,
    pub attr: Attr,
    pub exists: bool,
    cells: Vec<Cell>,
    touched: Vec<bool>,
    moved: bool,
}

impl Default for Attr {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "mirrors the boolean screen state kept by ncurses"
)]
pub struct Term {
    pub rows: usize,
    pub cols: usize,
    cells: Vec<Cell>,
    prev: Vec<Cell>,
    cursor: (usize, usize),
    cursor_visible: bool,
    shown_cursor: Option<(usize, usize, bool)>,
    force_full: bool,
    win: [WinState; 3],
    stale_rows: Vec<bool>,
    pub mouse: crate::curses::mouse::Mouse,
    pub cursor_invisible: Option<Vec<u8>>,
    pub terminfo_lines: i32,
    pub terminfo_cols: i32,
    pub cursor_normal: Option<Vec<u8>>,
    in_curses: bool,
    pub read_errors: u32,
    pub hangup: bool,
    tty_fd: i32,
    cooked: Option<libc::termios>,
    pub keypad_xmit: Vec<u8>,
    pub keypad_local: Vec<u8>,
    pub pad: PadInfo,
    pub bell: Option<Vec<u8>>,
    pub flash: Option<Vec<u8>>,
    pub keypad_wanted: bool,
    keypad_active: bool,
    secure_wanted: bool,
    secure_on: bool,
    focused: bool,
}

impl Term {
    pub fn new() -> Self {
        let (rows, cols) = terminal_size().unwrap_or((24, 80));
        Self {
            rows,
            cols,
            cells: vec![Cell::blank(); rows * cols],
            prev: vec![Cell::blank(); rows * cols],
            cursor: (0, 0),
            cursor_visible: false,
            shown_cursor: None,
            force_full: true,
            win: [
                WinState::default(),
                WinState::default(),
                WinState::default(),
            ],
            stale_rows: vec![false; rows],
            mouse: crate::curses::mouse::Mouse::new(),
            cursor_invisible: Some(b"\x1B[?25l".to_vec()),
            terminfo_lines: -1,
            terminfo_cols: -1,
            cursor_normal: Some(b"\x1B[?25h".to_vec()),
            in_curses: false,
            read_errors: 0,
            hangup: false,
            tty_fd: -1,
            cooked: None,
            keypad_xmit: Vec::new(),
            keypad_local: Vec::new(),
            pad: PadInfo::default(),
            bell: Some(b"\x07".to_vec()),
            flash: None,
            keypad_wanted: false,
            keypad_active: false,
            secure_wanted: cfg!(target_os = "macos") && !over_ssh(|name| std::env::var_os(name)),
            secure_on: false,
            focused: true,
        }
    }

    fn raw(&mut self) {
        let fd = self.input_fd();
        unsafe {
            let mut settings: libc::termios = std::mem::zeroed();
            if fd < 0 || libc::tcgetattr(fd, &raw mut settings) != 0 {
                return;
            }
            if self.cooked.is_none() {
                self.cooked = Some(settings);
            }
            libc::cfmakeraw(&raw mut settings);
            settings.c_cc[libc::VMIN] = 1;
            settings.c_cc[libc::VTIME] = 0;
            libc::tcsetattr(fd, libc::TCSANOW, &raw const settings);
        }
    }

    fn cook(&mut self) {
        let fd = self.input_fd();
        if let Some(settings) = self.cooked.take()
            && fd >= 0
        {
            unsafe {
                libc::tcsetattr(fd, libc::TCSANOW, &raw const settings);
            }
        }
    }

    pub fn begin_secret_input(&mut self) -> Option<libc::termios> {
        let fd = self.input_fd();
        let mut current: libc::termios = unsafe { std::mem::zeroed() };
        if fd < 0 || unsafe { libc::tcgetattr(fd, &raw mut current) } != 0 {
            return None;
        }
        let secret = secret_settings(&current);
        unsafe {
            libc::tcsetattr(fd, libc::TCSANOW, &raw const secret);
        }
        let mut off = Vec::new();
        self.mouse.wrap(&mut off);
        crate::crypto::raw_write(1, &off);
        Some(current)
    }

    pub fn keep_secret_input(&mut self) {
        let fd = self.input_fd();
        let mut current: libc::termios = unsafe { std::mem::zeroed() };
        if fd < 0
            || unsafe { libc::tcgetattr(fd, &raw mut current) } != 0
            || current.c_lflag & libc::ICANON != 0
        {
            return;
        }
        let secret = secret_settings(&current);
        unsafe {
            libc::tcsetattr(fd, libc::TCSANOW, &raw const secret);
        }
        let mut off = Vec::new();
        self.mouse.wrap(&mut off);
        crate::crypto::raw_write(1, &off);
    }

    pub fn end_secret_input(&mut self, before: Option<libc::termios>, discard: bool) {
        let fd = self.input_fd();
        if fd < 0 {
            return;
        }
        if discard {
            unsafe {
                libc::tcflush(fd, libc::TCIFLUSH);
            }
        }
        if let Some(settings) = before {
            unsafe {
                libc::tcsetattr(fd, libc::TCSANOW, &raw const settings);
            }
            let mut on = Vec::new();
            self.mouse.resume(&mut on);
            crate::crypto::raw_write(1, &on);
        }
    }

    pub fn read_secret(&mut self, buf: &mut [u8]) -> usize {
        let fd = self.input_fd();
        if fd < 0 {
            self.read_errors += 1;
            return 0;
        }
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast::<libc::c_void>(), buf.len()) };
        match n.cmp(&0) {
            std::cmp::Ordering::Greater => {
                self.read_errors = 0;
                n.cast_unsigned()
            }
            std::cmp::Ordering::Equal => {
                self.hangup = true;
                0
            }
            std::cmp::Ordering::Less => {
                if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                    self.read_errors += 1;
                }
                0
            }
        }
    }

    pub fn apply_keypad(&mut self) {
        if self.in_curses && self.keypad_wanted != self.keypad_active {
            let seq = if self.keypad_wanted {
                &self.keypad_xmit
            } else {
                &self.keypad_local
            };
            self.putp(seq, false);
            self.keypad_active = self.keypad_wanted;
        }
    }

    pub fn initscr(&mut self) {
        self.raw();
        emit(ENTER_SCREEN);
        if let Some(civis) = &self.cursor_invisible {
            self.putp(civis, false);
        }
        self.in_curses = true;
        IN_CURSES.store(true, std::sync::atomic::Ordering::SeqCst);
        self.keypad_active = false;
        self.apply_keypad();
        self.watch_focus();
        self.refresh_size();
        self.force_full = true;
        self.shown_cursor = None;
    }

    fn watch_focus(&mut self) {
        if self.secure_wanted {
            emit("\x1B[?1004h");
            FOCUS_REPORTS.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        self.focused = true;
        self.apply_secure_input();
    }

    pub const fn reports_focus(&self) -> bool {
        self.secure_wanted && self.in_curses
    }

    pub fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        self.apply_secure_input();
    }

    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            clippy::missing_const_for_fn,
            reason = "calls EnableSecureEventInput on macOS"
        )
    )]
    fn apply_secure_input(&mut self) {
        let wanted = self.secure_wanted && self.in_curses && self.focused;
        if wanted != self.secure_on {
            let done = crate::crypto::secure_keyboard(wanted);
            self.secure_on = wanted && done;
        }
    }

    pub fn endwin(&mut self) {
        if !self.in_curses {
            return;
        }
        if self.secure_on {
            crate::crypto::secure_keyboard(false);
            self.secure_on = false;
        }
        let mut off = Vec::new();
        self.mouse.wrap(&mut off);
        crate::crypto::raw_write(1, &off);
        if self.keypad_active {
            self.putp(&self.keypad_local, false);
            self.keypad_active = false;
        }
        let rows = self.rows.max(terminal_size().map_or(0, |(r, _)| r));
        crate::crypto::raw_write(1, &leave_sequence(rows));
        self.cook();
        self.in_curses = false;
        IN_CURSES.store(false, std::sync::atomic::Ordering::SeqCst);
        self.shown_cursor = None;
    }

    pub const fn isendwin(&self) -> bool {
        !self.in_curses
    }

    pub fn wipe(&mut self) {
        for cell in self.cells.iter_mut().chain(self.prev.iter_mut()) {
            cell.text.zeroize();
        }
        for win in &mut self.win {
            for cell in &mut win.cells {
                cell.text.zeroize();
            }
            win.cells = Vec::new();
            win.touched = Vec::new();
            win.exists = false;
        }
        self.cells = Vec::new();
        self.prev = Vec::new();
        self.rows = 0;
        self.cols = 0;
    }

    pub fn reenter(&mut self) {
        if !self.in_curses {
            self.raw();
            emit(ENTER_SCREEN);
            if let Some(civis) = &self.cursor_invisible {
                self.putp(civis, false);
            }
            let mut on = Vec::new();
            self.mouse.resume(&mut on);
            crate::crypto::raw_write(1, &on);
            self.in_curses = true;
            IN_CURSES.store(true, std::sync::atomic::Ordering::SeqCst);
            self.keypad_active = false;
            self.apply_keypad();
            self.watch_focus();
        }
        self.refresh_size();
        self.force_full = true;
        self.shown_cursor = None;
    }

    pub fn set_bracketed_paste(&self, on: bool) {
        if self.in_curses {
            emit(if on { "\x1B[?2004h" } else { "\x1B[?2004l" });
        }
    }

    pub fn emit_bytes(&self, sequence: &[u8]) {
        if self.in_curses && !sequence.is_empty() {
            crate::crypto::raw_write(1, sequence);
        }
    }

    pub fn screen_size(&self) -> (usize, usize) {
        let mut lines = i64::from(self.terminfo_lines);
        let mut cols = i64::from(self.terminfo_cols);
        if unsafe { libc::isatty(1) } == 1 {
            let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
            loop {
                if unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &raw mut ws) } >= 0 {
                    lines = i64::from(ws.ws_row);
                    cols = i64::from(ws.ws_col);
                    break;
                }
                if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                    break;
                }
            }
        }
        let value = getenv_num("LINES");
        if value > 0 {
            lines = value;
        }
        let value = getenv_num("COLUMNS");
        if value > 0 {
            cols = value;
        }
        if lines <= 0 {
            lines = i64::from(self.terminfo_lines);
        }
        if cols <= 0 {
            cols = i64::from(self.terminfo_cols);
        }
        let lines = usize::try_from(lines).ok().filter(|&n| n > 0).unwrap_or(24);
        let cols = usize::try_from(cols).ok().filter(|&n| n > 0).unwrap_or(80);
        (lines, cols)
    }

    pub fn refresh_size(&mut self) {
        let (rows, cols) = self.screen_size();
        self.resize(rows, cols);
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        if rows != self.rows || cols != self.cols || self.cells.len() != rows * cols {
            self.rows = rows;
            self.cols = cols;
            self.cells = vec![Cell::blank(); rows * cols];
            self.prev = vec![Cell::blank(); rows * cols];
            for win in &mut self.win {
                win.exists = false;
                win.cells = Vec::new();
                win.touched = Vec::new();
            }
        }
        self.stale_rows = vec![false; self.rows];
        self.force_full = true;
    }

    pub fn newwin(&mut self, w: Win, rows: usize, y0: usize) {
        let cols = self.cols;
        self.win[w as usize] = WinState {
            y0,
            rows,
            cur_y: 0,
            cur_x: 0,
            attr: Attr::NORMAL,
            exists: true,
            cells: vec![Cell::blank(); rows * cols],
            touched: vec![true; rows],
            moved: false,
        };
    }

    pub fn delwin(&mut self, w: Win) {
        let win = &mut self.win[w as usize];
        win.exists = false;
        win.cells = Vec::new();
        win.touched = Vec::new();
    }

    pub const fn win_exists(&self, w: Win) -> bool {
        self.win[w as usize].exists
    }

    pub const fn win_rows(&self, w: Win) -> usize {
        self.win[w as usize].rows
    }

    pub const fn win_y0(&self, w: Win) -> usize {
        self.win[w as usize].y0
    }

    pub const fn wmove(&mut self, w: Win, y: usize, x: usize) {
        let ws = &mut self.win[w as usize];
        ws.cur_y = y;
        ws.cur_x = x;
        ws.moved = true;
    }

    pub const fn wattron(&mut self, w: Win, a: Attr) {
        let ws = &mut self.win[w as usize];
        ws.attr = ws.attr.or(a);
    }

    pub const fn wattroff(&mut self, w: Win, a: Attr) {
        let ws = &mut self.win[w as usize];
        if a.fg >= 0 || a.bg >= 0 {
            ws.attr.fg = -1;
            ws.attr.bg = -1;
        }
        ws.attr.bits &= !a.bits;
    }

    fn put_cell(line: &mut [Cell], col: usize, text: String, width: usize, attr: Attr) {
        let cols = line.len();
        if col >= cols {
            return;
        }
        if line[col].cont && col > 0 {
            line[col - 1] = Cell {
                text: " ".into(),
                attr: line[col - 1].attr,
                cont: false,
            };
        }
        if col + 1 < cols && line[col + 1].cont {
            line[col + 1] = Cell {
                text: " ".into(),
                attr: line[col + 1].attr,
                cont: false,
            };
        }
        if width == 2 && col + 2 < cols && line[col + 2].cont {
            line[col + 2] = Cell {
                text: " ".into(),
                attr: line[col + 2].attr,
                cont: false,
            };
        }
        line[col] = Cell {
            text,
            attr,
            cont: false,
        };
        if width == 2 && col + 1 < cols {
            line[col + 1] = Cell {
                text: String::new(),
                attr,
                cont: true,
            };
        }
    }

    fn win_line(&mut self, w: Win, y: usize) -> Option<&mut [Cell]> {
        let cols = self.cols;
        let ws = &mut self.win[w as usize];
        if !ws.exists || y >= ws.rows || ws.cells.len() < (y + 1) * cols {
            return None;
        }
        ws.touched[y] = true;
        Some(&mut ws.cells[y * cols..(y + 1) * cols])
    }

    pub fn waddstr(&mut self, w: Win, text: &str) {
        let (y, mut x, attr) = {
            let ws = &self.win[w as usize];
            (ws.cur_y, ws.cur_x, ws.attr)
        };
        let cols = self.cols;
        if self.win_line(w, y).is_none() {
            return;
        }
        let ws = &mut self.win[w as usize];
        for c in text.chars() {
            if c == '\n' || c == '\r' || c == '\t' || (c as u32) < 0x20 {
                continue;
            }
            let raw = crate::chars::wcwidth(c);
            if raw == 0 {
                let target = if x > 0 && x - 1 < cols {
                    let mut pos = y * cols + x - 1;
                    if ws.cells[pos].cont && x > 1 {
                        pos -= 1;
                    }
                    Some(pos)
                } else if x == 0 && y >= 1 {
                    ws.touched[y - 1] = true;
                    Some(y * cols - 1)
                } else {
                    None
                };
                if let Some(pos) = target {
                    ws.cells[pos].text.push(c);
                }
                continue;
            }
            let (c, width) = usize::try_from(raw).map_or((' ', 1), |width| (c, width));
            if x >= cols || (width == 2 && x + 1 >= cols) {
                x += width;
                continue;
            }
            Self::put_cell(
                &mut ws.cells[y * cols..(y + 1) * cols],
                x,
                c.to_string(),
                width,
                attr,
            );
            x += width;
        }
        ws.cur_x = x;
    }

    pub fn waddbytes(&mut self, w: Win, b: &[u8]) {
        let s = String::from_utf8_lossy(b);
        self.waddstr(w, &s);
    }

    pub fn waddch(&mut self, w: Win, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        let s = s.to_string();
        self.waddstr(w, &s);
    }

    pub fn waddch_attr(&mut self, w: Win, c: char, a: Attr) {
        let saved = self.win[w as usize].attr;
        self.win[w as usize].attr = a;
        self.waddch(w, c);
        self.win[w as usize].attr = saved;
    }

    pub fn mvwaddstr(&mut self, w: Win, y: usize, x: usize, s: &str) {
        self.wmove(w, y, x);
        self.waddstr(w, s);
    }

    pub fn mvwaddbytes(&mut self, w: Win, y: usize, x: usize, b: &[u8]) {
        self.wmove(w, y, x);
        self.waddbytes(w, b);
    }

    pub fn mvwaddch(&mut self, w: Win, y: usize, x: usize, c: char) {
        self.wmove(w, y, x);
        self.waddch(w, c);
    }

    pub fn fill_row(&mut self, w: Win, y: usize) {
        let attr = self.win[w as usize].attr;
        if let Some(line) = self.win_line(w, y) {
            for cell in line.iter_mut() {
                *cell = Cell {
                    text: " ".into(),
                    attr,
                    cont: false,
                };
            }
            self.win[w as usize].cur_y = y;
            self.win[w as usize].cur_x = 0;
        }
    }

    pub fn wclrtoeol(&mut self, w: Win) {
        let (y, start) = {
            let ws = &self.win[w as usize];
            (ws.cur_y, ws.cur_x)
        };
        if let Some(line) = self.win_line(w, y) {
            if start < line.len() && line[start].cont && start > 0 {
                line[start - 1] = Cell::blank();
            }
            for cell in line.iter_mut().skip(start) {
                *cell = Cell::blank();
            }
        }
    }

    pub fn blank_row(&mut self, w: Win, y: usize) {
        self.wmove(w, y, 0);
        self.wclrtoeol(w);
    }

    pub fn werase(&mut self, w: Win) {
        let rows = self.win[w as usize].rows;
        for y in 0..rows {
            self.blank_row(w, y);
        }
    }

    pub fn wscrl(&mut self, w: Win, n: i32) {
        let cols = self.cols;
        let ws = &mut self.win[w as usize];
        if !ws.exists || ws.rows == 0 || cols == 0 || ws.cells.len() < ws.rows * cols {
            return;
        }
        let rows = ws.rows;
        let shift = (n.unsigned_abs() as usize).min(rows);
        if n > 0 {
            ws.cells.rotate_left(shift * cols);
            for cell in &mut ws.cells[(rows - shift) * cols..] {
                *cell = Cell::blank();
            }
        } else if n < 0 {
            ws.cells.rotate_right(shift * cols);
            for cell in &mut ws.cells[..shift * cols] {
                *cell = Cell::blank();
            }
        }
        ws.touched.fill(true);
    }

    pub fn wredrawln(&mut self, w: Win, first: usize, count: usize) {
        let ws = &mut self.win[w as usize];
        if !ws.exists {
            return;
        }
        for y in first..(first + count).min(ws.rows) {
            ws.touched[y] = true;
            if ws.y0 + y < self.stale_rows.len() {
                self.stale_rows[ws.y0 + y] = true;
            }
        }
    }

    pub const fn curs_set(&mut self, on: bool) {
        self.cursor_visible = on;
    }

    pub fn sync_cursor(&mut self, w: Win) {
        let ws = &self.win[w as usize];
        let row = (ws.y0 + ws.cur_y).min(self.rows.saturating_sub(1));
        let col = ws.cur_x.min(self.cols.saturating_sub(1));
        self.cursor = (row, col);
    }

    pub fn wnoutrefresh(&mut self, w: Win) {
        let cols = self.cols;
        let ws = &mut self.win[w as usize];
        if ws.exists && ws.cells.len() >= ws.rows * cols {
            for y in 0..ws.rows {
                let row = ws.y0 + y;
                if !ws.touched[y] || row >= self.rows {
                    continue;
                }
                ws.touched[y] = false;
                self.cells[row * cols..(row + 1) * cols]
                    .clone_from_slice(&ws.cells[y * cols..(y + 1) * cols]);
            }
        }
        ws.moved = false;
        self.sync_cursor(w);
    }

    pub fn refresh_before_input(&mut self, w: Win) {
        let ws = &self.win[w as usize];
        if ws.exists && (ws.moved || ws.touched.iter().any(|&t| t)) {
            self.wnoutrefresh(w);
        }
    }

    pub fn wrefresh(&mut self, w: Win) {
        self.wnoutrefresh(w);
        self.doupdate();
    }

    pub const fn redrawwin(&mut self) {
        self.force_full = true;
    }

    fn apply_attr(out: &mut Vec<u8>, a: Attr) {
        out.extend_from_slice(b"\x1B[0");
        if a.bits & A_BOLD != 0 {
            out.extend_from_slice(b";1");
        }
        if a.bits & A_ITALIC != 0 {
            out.extend_from_slice(b";3");
        }
        if a.bits & A_UNDERLINE != 0 {
            out.extend_from_slice(b";4");
        }
        if a.bits & A_REVERSE != 0 {
            out.extend_from_slice(b";7");
        }
        for (color, base, bright, extended) in [(a.fg, 30, 90, 38), (a.bg, 40, 100, 48)] {
            if (0..8).contains(&color) {
                out.extend_from_slice(format!(";{}", base + color).as_bytes());
            } else if (8..16).contains(&color) {
                out.extend_from_slice(format!(";{}", bright + color - 8).as_bytes());
            } else if color >= 16 {
                out.extend_from_slice(format!(";{extended};5;{color}").as_bytes());
            }
        }
        out.push(b'm');
    }

    fn move_to(out: &mut Vec<u8>, row: usize, col: usize) {
        out.extend_from_slice(format!("\x1B[{};{}H", row + 1, col + 1).as_bytes());
    }

    pub fn doupdate(&mut self) {
        self.update(false);
    }

    pub fn settle(&mut self) {
        self.update(true);
    }

    fn update(&mut self, settle: bool) {
        if !self.in_curses {
            return;
        }
        let mut body: Vec<u8> = Vec::with_capacity(4096);
        let mut cur_attr: Option<Attr> = None;
        for row in 0..self.rows {
            let whole = self.force_full || self.stale_rows.get(row).copied().unwrap_or(false);
            let line = &self.cells[row * self.cols..(row + 1) * self.cols];
            let blank_from = line
                .iter()
                .rposition(|cell| !(cell.attr == Attr::NORMAL && !cell.cont && cell.text == " "))
                .map_or(0, |last| last + 1);
            let mut col = 0;
            while col < self.cols {
                let i = row * self.cols + col;
                if self.cells[i].cont {
                    col += 1;
                    continue;
                }
                if !whole && self.cells[i] == self.prev[i] {
                    col += 1;
                    continue;
                }
                if col >= blank_from {
                    Self::move_to(&mut body, row, col);
                    if cur_attr != Some(Attr::NORMAL) {
                        Self::apply_attr(&mut body, Attr::NORMAL);
                        cur_attr = Some(Attr::NORMAL);
                    }
                    body.extend_from_slice(b"\x1B[K");
                    break;
                }
                Self::move_to(&mut body, row, col);
                while col < blank_from {
                    let i = row * self.cols + col;
                    if self.cells[i].cont {
                        col += 1;
                        continue;
                    }
                    if !whole && self.cells[i] == self.prev[i] {
                        break;
                    }
                    let a = self.cells[i].attr;
                    if cur_attr != Some(a) {
                        Self::apply_attr(&mut body, a);
                        cur_attr = Some(a);
                    }
                    let is_wide = col + 1 < self.cols && self.cells[i + 1].cont;
                    body.extend_from_slice(self.cells[i].text.as_bytes());
                    col += if is_wide { 2 } else { 1 };
                }
            }
        }
        if cur_attr.is_some() {
            body.extend_from_slice(b"\x1B[0m");
        }
        let wanted = match self.shown_cursor {
            Some((row, col, visible)) if !settle => (row, col, visible && self.cursor_visible),
            _ => (self.cursor.0, self.cursor.1, self.cursor_visible),
        };
        let (r, c) = (wanted.0, wanted.1);
        let drew = !body.is_empty();
        if !drew && self.shown_cursor == Some(wanted) {
            return;
        }
        let was_visible = match self.shown_cursor {
            Some((_, _, visible)) => visible,
            None => true,
        };
        let mut out: Vec<u8> = Vec::with_capacity(body.len() + 32);
        let visible_now = if drew
            && was_visible
            && let Some(civis) = &self.cursor_invisible
        {
            tputs(&mut out, civis, false, &self.pad);
            false
        } else {
            was_visible
        };
        out.extend_from_slice(&body);
        body.zeroize();
        let moved = match self.shown_cursor {
            Some((pr, pc, _)) => (pr, pc) != (r, c),
            None => true,
        };
        if drew || moved {
            Self::move_to(&mut out, r, c);
        }
        if wanted.2 != visible_now || self.shown_cursor.is_none() {
            let switch = if wanted.2 {
                &self.cursor_normal
            } else {
                &self.cursor_invisible
            };
            if let Some(sequence) = switch {
                tputs(&mut out, sequence, false, &self.pad);
            }
        }
        if drew {
            self.prev.clone_from(&self.cells);
        }
        self.force_full = false;
        self.stale_rows.fill(false);
        self.shown_cursor = Some(wanted);
        crate::crypto::raw_write(1, &out);
        out.zeroize();
    }

    pub fn beep(&self) {
        if let Some(cap) = self.bell.as_ref().or(self.flash.as_ref()) {
            self.putp(cap, true);
        }
    }

    fn putp(&self, cap: &[u8], always_delay: bool) {
        let mut out = Vec::new();
        tputs(&mut out, cap, always_delay, &self.pad);
        crate::crypto::raw_write(1, &out);
    }

    pub fn napms(&mut self, ms: u64) {
        self.settle();
        std::thread::sleep(Duration::from_millis(ms));
    }

    pub fn input_fd(&mut self) -> i32 {
        if unsafe { libc::isatty(0) } == 1 {
            return 0;
        }
        if self.tty_fd < 0 {
            self.tty_fd = unsafe {
                libc::open(
                    c"/dev/tty".as_ptr().cast::<libc::c_char>(),
                    libc::O_RDWR | libc::O_CLOEXEC,
                )
            };
        }
        self.tty_fd
    }

    pub fn wait_for_input(&mut self, patience: Duration) -> Wait {
        if self.hangup {
            std::thread::sleep(patience.min(Duration::from_millis(20)));
            return Wait::Hangup;
        }
        let fd = self.input_fd();
        if fd < 0 {
            self.read_errors += 1;
            std::thread::sleep(Duration::from_millis(20));
            return Wait::Timeout;
        }
        let ms = i32::try_from(patience.as_millis()).unwrap_or(i32::MAX);
        let probe = probe(fd, true, ms);
        let ready = probe.count;
        if ready < 0 {
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                self.read_errors += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            return Wait::Interrupted;
        }
        if ready == 0 {
            return Wait::Timeout;
        }
        if probe.gone {
            self.hangup = true;
            return Wait::Hangup;
        }
        Wait::Ready
    }

    pub fn terminal_is_gone(&mut self) -> bool {
        if self.hangup {
            return true;
        }
        let fd = self.input_fd();
        if fd < 0 {
            return false;
        }
        if probe(fd, false, 0).gone {
            self.hangup = true;
        }
        self.hangup
    }

    pub fn read_available(&mut self, sink: &mut dyn FnMut(u8)) -> usize {
        let mut total = 0;
        let mut chunk = [0u8; 256];
        loop {
            let fd = self.input_fd();
            if fd < 0 || !probe(fd, true, 0).readable {
                break;
            }
            let n =
                unsafe { libc::read(fd, chunk.as_mut_ptr().cast::<libc::c_void>(), chunk.len()) };
            if n > 0 {
                self.read_errors = 0;
                for &byte in &chunk[..n.cast_unsigned()] {
                    sink(byte);
                }
                total += n.cast_unsigned();
                chunk.zeroize();
                continue;
            }
            if n == 0 {
                self.hangup = true;
            } else if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                self.read_errors += 1;
            }
            break;
        }
        chunk.zeroize();
        total
    }
}

fn over_ssh(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> bool {
    ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|name| var(name).is_some_and(|value| !value.is_empty()))
}

pub fn focus_report(input: &[u8]) -> Option<bool> {
    match input.get(..3)? {
        b"\x1B[I" => Some(true),
        b"\x1B[O" => Some(false),
        _ => None,
    }
}

const fn secret_settings(raw: &libc::termios) -> libc::termios {
    let mut settings = *raw;
    settings.c_lflag |= libc::ICANON;
    settings.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ISIG | libc::IEXTEN);
    settings.c_iflag |= libc::ICRNL | libc::IUTF8 | libc::IMAXBEL;
    settings.c_iflag &= !(libc::INLCR | libc::IGNCR);
    settings.c_cc[libc::VERASE] = libc::_POSIX_VDISABLE;
    settings.c_cc[libc::VKILL] = libc::_POSIX_VDISABLE;
    settings.c_cc[libc::VEOF] = libc::_POSIX_VDISABLE;
    settings.c_cc[libc::VEOL] = 0x03;
    settings.c_cc[libc::VEOL2] = libc::_POSIX_VDISABLE;
    settings
}

pub fn set_flow_control(on: bool) {
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &raw mut t) == 0 {
            if on {
                t.c_iflag |= libc::IXON;
            } else {
                t.c_iflag &= !libc::IXON;
            }
            libc::tcsetattr(0, libc::TCSANOW, &raw const t);
        }
    }
}

pub fn get_termios() -> Option<libc::termios> {
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &raw mut t) == 0 {
            Some(t)
        } else {
            None
        }
    }
}

pub fn set_termios(t: &libc::termios) {
    unsafe {
        libc::tcsetattr(0, libc::TCSANOW, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_like_strtol() {
        assert_eq!(parse_env_num("72"), 72);
        assert_eq!(parse_env_num("0x48"), 72);
        assert_eq!(parse_env_num("0110"), 72);
        assert_eq!(parse_env_num("  +72"), 72);
        assert_eq!(parse_env_num("0"), 0);
        assert_eq!(parse_env_num("72abc"), -1);
        assert_eq!(parse_env_num("72 "), -1);
        assert_eq!(parse_env_num("-5"), -1);
        assert_eq!(parse_env_num(""), -1);
        assert_eq!(parse_env_num("0x"), -1);
        assert_eq!(parse_env_num("08"), -1);
        assert_eq!(parse_env_num("99999999999"), -1);
    }

    #[test]
    fn focus_reports_and_ssh_are_recognised() {
        assert_eq!(focus_report(b"\x1B[I"), Some(true));
        assert_eq!(focus_report(b"\x1B[Oxyz"), Some(false));
        assert_eq!(focus_report(b"\x1B["), None);
        assert_eq!(focus_report(b"\x1B[A"), None);
        assert!(!over_ssh(|_| None));
        assert!(!over_ssh(|_| Some(std::ffi::OsString::new())));
        assert!(over_ssh(|name| {
            (name == "SSH_TTY").then(|| std::ffi::OsString::from("/dev/ttys004"))
        }));
        assert!(over_ssh(|name| {
            (name == "SSH_CONNECTION")
                .then(|| std::ffi::OsString::from("10.0.0.2 51234 10.0.0.1 22"))
        }));
    }

    #[test]
    fn password_prompts_look_like_password_prompts() {
        let mut raw: libc::termios = unsafe { std::mem::zeroed() };
        raw.c_iflag = libc::INLCR | libc::IGNCR;
        raw.c_lflag = libc::ECHO | libc::ECHONL | libc::ISIG | libc::IEXTEN;
        raw.c_oflag = 0;
        raw.c_cc[libc::VERASE] = 0x08;
        raw.c_cc[libc::VKILL] = 0x15;
        let secret = secret_settings(&raw);
        assert_ne!(secret.c_lflag & libc::ICANON, 0);
        assert_eq!(
            secret.c_lflag & (libc::ECHO | libc::ECHONL | libc::ISIG | libc::IEXTEN),
            0
        );
        assert_eq!(secret.c_iflag & (libc::INLCR | libc::IGNCR), 0);
        assert_ne!(secret.c_iflag & libc::ICRNL, 0);
        assert_ne!(secret.c_iflag & libc::IUTF8, 0);
        assert_eq!(secret.c_oflag, 0);
        assert_eq!(secret.c_cc[libc::VERASE], libc::_POSIX_VDISABLE);
        assert_eq!(secret.c_cc[libc::VKILL], libc::_POSIX_VDISABLE);
        assert_eq!(secret.c_cc[libc::VEOL], 0x03);
        assert_eq!(secret.c_cc[libc::VEOF], libc::_POSIX_VDISABLE);
    }
}
