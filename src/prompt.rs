use crate::changes_something;
use crate::utils::{i32_of, i64_of, u8_of, usize_of};
use std::time::Duration;
use zeroize::Zeroizing;

use crate::chars::{char_length, is_zerowidth, step_left, step_right};
use crate::curses::{Wait, Win};
use crate::definitions::{
    ALL, BLIND, CANCEL, END_OF_PASTE, Elem, Flag, Func, HIGHEST_POSITIVE, HistKind, KEY_BACKSPACE,
    KEY_ENTER, KEY_MOUSE, MEXECUTE, MGOTODIR, MINSERTFILE, MMAIN, MMOST, MPASSWORD, MWRITEFILE,
    MYESNO, Msg, NO, START_OF_PASTE, THE_WINDOW_RESIZED, VISIBLE, YES,
};
use crate::global::Zu;
use crate::search::RefreshFn;

const UNDECIDED: i32 = -2;

impl Zu {
    const fn do_statusbar_home(&mut self) {
        self.typing_x = 0;
    }

    const fn do_statusbar_end(&mut self) {
        self.typing_x = self.answer.len();
    }

    fn do_statusbar_prev_word(&mut self) {
        let mut seen_a_word = false;
        let mut step_forward = false;
        while self.typing_x != 0 {
            self.typing_x = step_left(&self.answer, self.typing_x);
            let d = &self.answer[self.typing_x..];
            if self.is_word_char(d, false) {
                seen_a_word = true;
            } else if is_zerowidth(d) {
            } else if seen_a_word {
                step_forward = true;
                break;
            }
        }
        if step_forward {
            self.typing_x = step_right(&self.answer, self.typing_x);
        }
    }

    fn do_statusbar_next_word(&mut self) {
        let mut seen_space =
            !self.is_word_char(&self.answer[self.typing_x.min(self.answer.len())..], false);
        let mut seen_word = !seen_space;
        while self.typing_x < self.answer.len() {
            self.typing_x = step_right(&self.answer, self.typing_x);
            let d = &self.answer[self.typing_x.min(self.answer.len())..];
            if self.isset(Flag::AfterEnds) {
                if self.is_word_char(d, false) {
                    seen_word = true;
                } else if is_zerowidth(d) {
                } else if seen_word {
                    break;
                }
            } else if is_zerowidth(d) {
            } else if !self.is_word_char(d, false) {
                seen_space = true;
            } else if seen_space {
                break;
            }
        }
    }

    fn do_statusbar_left(&mut self) {
        if self.typing_x > 0 {
            self.typing_x = step_left(&self.answer, self.typing_x);
            while self.typing_x > 0 && is_zerowidth(&self.answer[self.typing_x..]) {
                self.typing_x = step_left(&self.answer, self.typing_x);
            }
        }
    }

    fn do_statusbar_right(&mut self) {
        if self.typing_x < self.answer.len() {
            self.typing_x = step_right(&self.answer, self.typing_x);
            while self.typing_x < self.answer.len() && is_zerowidth(&self.answer[self.typing_x..]) {
                self.typing_x = step_right(&self.answer, self.typing_x);
            }
        }
    }

    fn do_statusbar_backspace(&mut self) {
        if self.typing_x > 0 {
            let was_x = self.typing_x;
            self.typing_x = step_left(&self.answer, self.typing_x);
            self.answer.drain(self.typing_x..was_x);
        }
    }

    fn do_statusbar_delete(&mut self) {
        if self.typing_x < self.answer.len() {
            let charlen = char_length(&self.answer[self.typing_x..]);
            let end = (self.typing_x + charlen).min(self.answer.len());
            self.answer.drain(self.typing_x..end);
            if self.typing_x < self.answer.len() && is_zerowidth(&self.answer[self.typing_x..]) {
                self.do_statusbar_delete();
            }
        }
    }

    fn lop_the_answer(&mut self) {
        if self.typing_x >= self.answer.len() {
            self.typing_x = 0;
        }
        self.answer.truncate(self.typing_x);
    }

    fn copy_the_answer(&mut self) {
        if !self.answer.is_empty() {
            let cb = self.cutbuffer;
            self.free_lines(cb);
            let n = self.make_new_node(None);
            self.nodes[n].data = self.answer.clone();
            self.cutbuffer = Some(n);
            self.cutbottom = Some(n);
            self.typing_x = 0;
        }
    }

    fn paste_into_answer(&mut self) {
        let Some(cb) = self.cutbuffer else { return };
        let piece = self.nodes[cb].data.clone();
        let tail = self.answer.split_off(self.typing_x.min(self.answer.len()));
        self.answer.extend_from_slice(&piece);
        self.answer.extend_from_slice(&tail);
        self.typing_x += piece.len();
    }

    fn process_prompt_click(&mut self) -> i32 {
        let (retval, mut y, x) = self.get_mouseinput();
        if retval == 0 && self.wmouse_trafo(Win::Foot, &mut y, x) {
            let start_col = self.breadth(&self.prompt) + 2;
            if usize_of(i64::from(x)) >= start_col {
                let col = self.get_statusbar_page_start(
                    start_col,
                    start_col + self.wideness(&self.answer, self.typing_x),
                ) + usize_of(i64::from(x))
                    - start_col;
                self.typing_x = self.actual_x(&self.answer, col);
            } else {
                self.typing_x = 0;
            }
        }
        retval
    }

    fn inject_into_answer(&mut self, burst: &mut [u8]) {
        for b in burst.iter_mut() {
            if *b == 0 {
                *b = b'\n';
            }
        }
        let tail = self.answer.split_off(self.typing_x.min(self.answer.len()));
        self.answer.extend_from_slice(burst);
        self.answer.extend_from_slice(&tail);
        self.typing_x += burst.len();
    }

    fn do_statusbar_verbatim_input(&mut self) {
        let (mut bytes, count) = self.get_verbatim_kbinput(Win::Foot);
        if count > 0 && count < 999 {
            self.inject_into_answer(&mut bytes);
        } else if count == 0 {
            self.term.beep();
        }
    }

    fn absorb_character(&mut self, input: i32, function: Option<Func>) {
        if function.is_none() {
            if (input < 0x20 && input != '\t' as i32) || self.meta_key || input > 0xFF {
                self.term.beep();
            } else if !self.isset(Flag::Restricted)
                || self.currmenu != MWRITEFILE
                || self.ofile().filename.is_empty()
            {
                self.prompt_puddle.push(u8_of(i64::from(input)));
            }
        }
        if !self.prompt_puddle.is_empty() && (function.is_some() || self.waiting_keycodes() == 0) {
            let mut p = std::mem::take(&mut self.prompt_puddle);
            self.inject_into_answer(&mut p);
        }
    }

    fn handle_editing(&mut self, function: Func) -> bool {
        let restricted_write = self.isset(Flag::Restricted)
            && self.currmenu == MWRITEFILE
            && !self.ofile().filename.is_empty();
        match function {
            Func::Left => self.do_statusbar_left(),
            Func::Right => self.do_statusbar_right(),
            Func::PrevWord => self.do_statusbar_prev_word(),
            Func::NextWord => self.do_statusbar_next_word(),
            Func::Home => self.do_statusbar_home(),
            Func::End => self.do_statusbar_end(),
            Func::Verbatim | Func::Delete | Func::Backspace | Func::CutText | Func::PasteText
                if restricted_write => {}
            Func::Verbatim => self.do_statusbar_verbatim_input(),
            Func::Delete => self.do_statusbar_delete(),
            Func::Backspace => self.do_statusbar_backspace(),
            Func::CutText => self.lop_the_answer(),
            Func::CopyText => self.copy_the_answer(),
            Func::PasteText => {
                if self.cutbuffer.is_some() {
                    self.paste_into_answer();
                }
            }
            _ => return false,
        }
        true
    }

    pub const fn get_statusbar_page_start(&self, base: usize, column: usize) -> usize {
        let cols = self.cols();
        if column == base || column < cols - 1 {
            0
        } else if cols > base + 2 {
            column - base - 1 - (column - base - 1) % (cols - base - 2)
        } else {
            column - 2
        }
    }

    pub const fn put_cursor_at_end_of_answer(&mut self) {
        self.typing_x = HIGHEST_POSITIVE;
    }

    fn draw_the_promptbar(&mut self) {
        let cols = self.cols();
        let base = self.breadth(&self.prompt) + 2;
        let column = base + self.wideness(&self.answer, self.typing_x);
        let the_page = self.get_statusbar_page_start(base, column);
        let end_page = self
            .get_statusbar_page_start(base, base + self.breadth(&self.answer).saturating_sub(1));
        let pb = self.interface_color_pair[Elem::PromptBar as usize];
        self.term.wattron(Win::Foot, pb);
        self.term.fill_row(Win::Foot, 0);
        let p = self.prompt.clone();
        self.term.mvwaddbytes(Win::Foot, 0, 0, &p);
        self.term.waddch(Win::Foot, ':');
        self.term
            .waddch(Win::Foot, if the_page == 0 { ' ' } else { '<' });
        let a = self.answer.clone();
        let expanded = self.display_string(&a, the_page, cols.saturating_sub(base), false, true);
        self.term.waddbytes(Win::Foot, &expanded);
        if the_page < end_page && base + self.breadth(&self.answer) - the_page > cols {
            self.term.mvwaddch(Win::Foot, 0, cols - 1, '>');
        }
        self.term.wattroff(Win::Foot, pb);
        self.term.wmove(Win::Foot, 0, column - the_page);
        self.term.wnoutrefresh(Win::Foot);
    }

    const fn history_mut(&mut self, kind: HistKind) -> &mut crate::definitions::History {
        match kind {
            HistKind::Search => &mut self.search_history,
            HistKind::Replace => &mut self.replace_history,
            HistKind::Execute => &mut self.execute_history,
        }
    }

    fn acquire_an_answer(
        &mut self,
        actual: &mut i32,
        listed: &mut bool,
        history: Option<HistKind>,
        refresh: RefreshFn,
    ) -> Option<Func> {
        let mut stored_string: Option<Vec<u8>> = None;
        let mut previous_was_tab = false;
        let mut fragment_length = 0usize;
        let mut bracketed_paste = false;
        let mut function: Option<Func>;
        let mut input;
        if self.typing_x > self.answer.len() {
            self.typing_x = self.answer.len();
        }
        loop {
            self.draw_the_promptbar();
            input = self.get_kbinput(Win::Foot, VISIBLE);
            if input == THE_WINDOW_RESIZED {
                *actual = THE_WINDOW_RESIZED;
                return None;
            }
            if input == START_OF_PASTE || input == END_OF_PASTE {
                bracketed_paste = input == START_OF_PASTE;
            }
            if input == KEY_MOUSE {
                if self.process_prompt_click() == 1 {
                    input = self.get_kbinput(Win::Foot, BLIND);
                } else {
                    continue;
                }
            }
            let shortcut = self.get_shortcut(input);
            function = shortcut.as_ref().map(|s| s.func);
            if input == '\t' as i32 && bracketed_paste {
                function = None;
            }
            self.absorb_character(input, function);
            if bracketed_paste {
                if function.is_some() && function != Some(Func::Nothing) {
                    self.term.beep();
                }
                continue;
            }
            if function == Some(Func::Cancel) || function == Some(Func::Enter) {
                break;
            }
            match function {
                Some(Func::Tab) => {
                    if let Some(h) = history {
                        if !previous_was_tab {
                            fragment_length = self.answer.len();
                        }
                        if fragment_length > 0 {
                            let a = self.answer.clone();
                            self.answer = self.get_history_completion(h, a, fragment_length);
                            self.typing_x = self.answer.len();
                        }
                    } else if self.currmenu & (MINSERTFILE | MWRITEFILE | MGOTODIR) != 0
                        && !self.isset(Flag::Restricted)
                    {
                        let a = std::mem::take(&mut self.answer);
                        let mut place = self.typing_x;
                        self.answer = self.input_tab(a, &mut place, refresh, listed);
                        self.typing_x = place;
                    }
                }
                Some(Func::OlderItem) if history.is_some() => {
                    let h = history.unwrap();
                    if stored_string.is_none() {
                        self.history_mut(h).reset();
                    }
                    if self.history_mut(h).at_bottom() {
                        stored_string = Some(self.answer.clone());
                    }
                    let hist = self.history_mut(h);
                    if hist.pos > 0 {
                        hist.pos -= 1;
                        let item = hist.items[hist.pos].clone();
                        self.answer = item;
                        self.typing_x = self.answer.len();
                    }
                }
                Some(Func::NewerItem) if history.is_some() => {
                    let h = history.unwrap();
                    let hist = self.history_mut(h);
                    if !hist.at_bottom() {
                        hist.pos += 1;
                        let item = hist.items[hist.pos].clone();
                        self.answer = item;
                        self.typing_x = self.answer.len();
                    }
                    if self.history_mut(h).at_bottom()
                        && stored_string.is_some()
                        && self.answer.is_empty()
                    {
                        self.answer = stored_string.clone().unwrap();
                        self.typing_x = self.answer.len();
                    }
                }
                Some(Func::Help) => self.do_help(),
                Some(Func::FullRefresh) => self.full_refresh(),
                Some(Func::Toggle)
                    if shortcut.as_ref().and_then(|s| s.toggle) == Some(Flag::NoHelp) =>
                {
                    self.flags.toggle(Flag::NoHelp);
                    self.window_init();
                    self.focusing = false;
                    self.refresh_window(refresh);
                    let m = self.currmenu;
                    self.bottombars(m);
                }
                Some(Func::Nothing) | None => {}
                Some(Func::Implant) => {
                    if let Some(exp) = shortcut.as_ref().and_then(|s| s.expansion.clone()) {
                        self.implant(&exp);
                    }
                }
                Some(f) => {
                    if !self.handle_editing(f) {
                        if !self.isset(Flag::ViewMode) || !changes_something(f) {
                            if self.currmenu == MEXECUTE {
                                self.foretext = self.answer.clone();
                            }
                            self.execute_function(f);
                            break;
                        }
                        self.term.beep();
                    }
                }
            }
            previous_was_tab = function == Some(Func::Tab);
        }
        if self.currmenu == MEXECUTE && function == Some(Func::Enter) {
            self.foretext.clear();
        }
        if stored_string.is_some()
            && let Some(h) = history
        {
            self.history_mut(h).reset();
        }
        *actual = input;
        function
    }

    pub fn do_prompt(
        &mut self,
        menu: u32,
        provided: &[u8],
        history: Option<HistKind>,
        refresh: RefreshFn,
        msg: &[u8],
    ) -> i32 {
        let mut listed = false;
        let mut retval: i32;
        let was_typing_x = self.typing_x;
        let saved_prompt = std::mem::take(&mut self.prompt);
        self.bottombars(menu);
        if self.answer != provided {
            self.answer = provided.to_vec();
        }
        let function;
        loop {
            let cols = self.cols();
            let mut p = msg.to_vec();
            let n = self.actual_x(&p, cols.saturating_sub(5));
            p.truncate(n);
            self.prompt = p;
            self.lastmessage = Msg::Vacuum;
            retval = 0;
            let f = self.acquire_an_answer(&mut retval, &mut listed, history, refresh);
            if retval == THE_WINDOW_RESIZED {
                continue;
            }
            function = f;
            break;
        }
        self.prompt = saved_prompt;
        if matches!(
            function,
            Some(
                Func::Cancel
                    | Func::Enter
                    | Func::FirstFile
                    | Func::LastFile
                    | Func::FirstLine
                    | Func::LastLine
            )
        ) {
            self.typing_x = was_typing_x;
        }
        if function == Some(Func::Cancel) {
            retval = -1;
        } else if function == Some(Func::Enter) {
            retval = if self.answer.is_empty() { -2 } else { 0 };
        }
        if self.lastmessage == Msg::Vacuum {
            self.wipe_statusbar();
        }
        if listed {
            self.refresh_window(refresh);
        }
        retval
    }

    pub fn ask_user(&mut self, withall: bool, question: &str) -> i32 {
        let mut choice = UNDECIDED;
        let mut width = 16usize;
        let yesstr = "JjYy";
        let nostr = "Nn";
        let allstr = "Aa";
        while choice == UNDECIDED {
            if !self.isset(Flag::NoHelp) {
                let cols = self.cols();
                if cols < 32 {
                    width = cols / 2;
                }
                self.blank_bottombars();
                self.term.wmove(Win::Foot, 1, 0);
                self.post_one_key(" J", "Ja", width);
                self.term.wmove(Win::Foot, 2, 0);
                self.post_one_key(" N", "Nein", width);
                if withall {
                    self.term.wmove(Win::Foot, 1, width);
                    self.post_one_key(" A", "Alle", width);
                }
                let cancel_key = self
                    .first_sc_for(MYESNO, Func::Cancel)
                    .map_or_else(|| "^C".into(), |s| s.keystr.clone());
                self.term.wmove(Win::Foot, 2, width);
                self.post_one_key(&cancel_key, "Abbrechen", width);
            }
            let pb = self.interface_color_pair[Elem::PromptBar as usize];
            self.term.wattron(Win::Foot, pb);
            self.term.fill_row(Win::Foot, 0);
            let cols = self.cols();
            let n = self.actual_x(question.as_bytes(), cols - 1);
            self.term
                .mvwaddbytes(Win::Foot, 0, 0, &question.as_bytes()[..n]);
            self.term.wattroff(Win::Foot, pb);
            self.term.wnoutrefresh(Win::Foot);
            self.currmenu = MYESNO;
            let mut kbinput = self.get_kbinput(Win::Foot, !withall);
            if kbinput == THE_WINDOW_RESIZED {
                continue;
            }
            if kbinput == START_OF_PASTE {
                kbinput = self.get_kbinput(Win::Foot, BLIND);
                while self.get_kbinput(Win::Foot, BLIND) != END_OF_PASTE {}
            }
            if (0..0x80).contains(&kbinput) && !self.meta_key {
                let c = u8_of(i64::from(kbinput)) as char;
                if yesstr.contains(c) {
                    choice = YES;
                } else if nostr.contains(c) {
                    choice = NO;
                } else if withall && allstr.contains(c) {
                    choice = ALL;
                }
            }
            if choice != UNDECIDED {
                break;
            }
            let shortcut = self.get_shortcut(kbinput);
            let function = shortcut.as_ref().map(|s| s.func);
            if function == Some(Func::Cancel) {
                choice = CANCEL;
            } else if function == Some(Func::FullRefresh) {
                self.full_refresh();
            } else if function == Some(Func::Toggle)
                && shortcut.as_ref().and_then(|s| s.toggle) == Some(Flag::NoHelp)
            {
                self.flags.toggle(Flag::NoHelp);
                self.window_init();
                self.titlebar(None);
                self.focusing = false;
                self.edit_refresh();
                self.focusing = true;
            } else if kbinput == 0x0E
                || (kbinput == 0x11 && !self.isset(Flag::ModernBindings))
                || (kbinput == 0x18 && self.isset(Flag::ModernBindings))
            {
                choice = NO;
                if kbinput != 0x0E {
                    self.final_status = 2;
                }
            } else if kbinput == 0x19 {
                choice = YES;
            } else if kbinput == 0x01 && withall {
                choice = ALL;
            } else if kbinput == KEY_MOUSE {
                let (r, mut y, x) = self.get_mouseinput();
                if r == 0
                    && self.wmouse_trafo(Win::Foot, &mut y, x)
                    && (usize_of(i64::from(x))) < width * 2
                    && y > 0
                {
                    let xx = x / i32_of(i64_of(width));
                    let yy = y - 1;
                    choice = -2 * xx * yy + xx - yy + 1;
                    if choice == ALL && !withall {
                        choice = UNDECIDED;
                    }
                }
            } else {
                self.term.beep();
            }
        }
        choice
    }

    fn show_password_keys(&mut self) {
        self.bottombars(MPASSWORD);
        if !self.isset(Flag::NoHelp) && self.lines() > 5 {
            self.blank_bottombars();
            let cols = self.cols();
            let width = if cols < 32 { cols / 2 } else { 16 };
            self.term.wmove(Win::Foot, 1, 0);
            self.post_one_key("Enter", "Bestätigen", width);
            self.term.wmove(Win::Foot, 2, 0);
            self.post_one_key("^C", "Abbrechen", width);
            self.term.wmove(Win::Foot, 1, width);
            self.post_one_key("^U", "Eingabe löschen", width + 8);
            self.term.wmove(Win::Foot, 2, width);
            self.post_one_key("Bsp", "Zeichen löschen", width + 8);
        }
    }

    fn show_password_prompt(&mut self, question: &str) {
        let cols = self.cols();
        let pb = self.interface_color_pair[Elem::PromptBar as usize];
        self.term.wattron(Win::Foot, pb);
        self.term.fill_row(Win::Foot, 0);
        let n = self.actual_x(question.as_bytes(), cols.saturating_sub(5));
        self.term
            .mvwaddbytes(Win::Foot, 0, 0, &question.as_bytes()[..n]);
        self.term.waddstr(Win::Foot, ": ");
        let base = self.breadth(&question.as_bytes()[..n]) + 2;
        self.term.wattroff(Win::Foot, pb);
        self.term.wmove(Win::Foot, 0, base.min(cols - 1));
        self.term.wnoutrefresh(Win::Foot);
        self.term.curs_set(true);
        self.term.settle();
    }

    fn typed_ahead(&mut self, recorded: bool) -> Zeroizing<Vec<u8>> {
        let mut bytes = Zeroizing::new(Vec::with_capacity(
            self.key_buffer.len() * 6 + self.pending_input.len(),
        ));
        if recorded {
            let keep = self
                .macro_buffer
                .len()
                .saturating_sub(self.key_buffer.len());
            self.macro_buffer[keep..].zeroize();
            self.macro_buffer.truncate(keep);
        }
        while let Some(code) = self.pop_code() {
            match code {
                THE_WINDOW_RESIZED => {}
                KEY_ENTER => bytes.push(b'\r'),
                KEY_BACKSPACE => bytes.push(0x7F),
                START_OF_PASTE => bytes.extend_from_slice(b"\x1B[200~"),
                END_OF_PASTE => bytes.extend_from_slice(b"\x1B[201~"),
                _ => bytes.push(u8::try_from(code).unwrap_or(0)),
            }
        }
        bytes.extend_from_slice(&self.pending_input);
        self.pending_input.zeroize();
        bytes
    }

    fn absorb_secret(
        &mut self,
        reader: &mut SecretReader,
        bytes: &[u8],
        password: &mut Vec<u8>,
    ) -> Typed {
        let capacity = password.capacity();
        let (typed, used) = reader.absorb(bytes, password);
        if password.capacity() != capacity {
            crate::crypto::lock_region(password.as_ptr(), password.capacity());
        }
        if std::mem::take(&mut reader.rejected) {
            self.term.beep();
        }
        if let Some(focused) = reader.focus.take()
            && self.term.reports_focus()
        {
            self.term.set_focus(focused);
        }
        if typed == Typed::Confirmed || (typed == Typed::Cancelled && !reader.pasting) {
            self.pending_input.extend_from_slice(&bytes[used..]);
        }
        typed
    }

    pub fn ask_for_password(&mut self, question: &str) -> Option<Zeroizing<Vec<u8>>> {
        let mut password: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::with_capacity(256));
        crate::crypto::lock_region(password.as_ptr(), password.capacity());
        let saved_menu = self.currmenu;
        let was_recording = self.recording;
        self.recording = false;
        let mut early = self.typed_ahead(was_recording);
        self.show_password_keys();
        let before = self.term.begin_secret_input();
        let mut reader = SecretReader::default();
        let mut chunk = [0u8; 256];
        let result = loop {
            let typed = if early.is_empty() {
                self.check_signals();
                if self.the_window_resized {
                    self.regenerate_screen();
                    self.show_password_keys();
                }
                self.term.keep_secret_input();
                self.show_password_prompt(question);
                if self.term.wait_for_input(Duration::from_millis(250)) != Wait::Ready {
                    continue;
                }
                let n = self.term.read_secret(&mut chunk);
                let typed = self.absorb_secret(&mut reader, &chunk[..n], &mut password);
                chunk.zeroize();
                typed
            } else {
                self.show_password_prompt(question);
                let typed = self.absorb_secret(&mut reader, &early, &mut password);
                early.zeroize();
                typed
            };
            match typed {
                Typed::Confirmed => {
                    break Some(std::mem::replace(&mut password, Zeroizing::new(Vec::new())));
                }
                Typed::Cancelled => break None,
                Typed::Pending => {}
            }
        };
        self.term
            .end_secret_input(before, result.is_none() && reader.pasting);
        self.term.curs_set(false);
        password.zeroize();
        drop(password);
        self.recording = was_recording;
        self.milestone = self.macro_buffer.len();
        self.wipe_statusbar();
        self.currmenu = saved_menu;
        if saved_menu != MMAIN && saved_menu != MMOST {
            self.bottombars(saved_menu);
        } else {
            self.blank_bottombars();
            self.currmenu = MMOST;
        }
        result
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Typed {
    Pending,
    Confirmed,
    Cancelled,
}

#[derive(Default)]
struct SecretReader {
    pasting: bool,
    escape: Vec<u8>,
    skip: usize,
    rejected: bool,
    focus: Option<bool>,
}

const fn utf8_tail(lead: u8) -> usize {
    match lead {
        0xC0..=0xDF => 1,
        0xE0..=0xEF => 2,
        0xF0..=0xF7 => 3,
        _ => 0,
    }
}

impl SecretReader {
    fn absorb(&mut self, bytes: &[u8], password: &mut Vec<u8>) -> (Typed, usize) {
        for (i, &byte) in bytes.iter().enumerate() {
            let typed = self.take(byte, password);
            if typed == Typed::Confirmed && byte == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                return (typed, i + 2);
            }
            if typed != Typed::Pending {
                return (typed, i + 1);
            }
        }
        (Typed::Pending, bytes.len())
    }

    fn take(&mut self, byte: u8, password: &mut Vec<u8>) -> Typed {
        if self.skip > 0 {
            self.skip -= 1;
            return Typed::Pending;
        }
        if !self.escape.is_empty() && !self.continue_escape(byte) {
            return Typed::Pending;
        }
        match byte {
            0x1B => self.escape.push(byte),
            b'\n' | b'\r' if !self.pasting => return Typed::Confirmed,
            0x03 => return Typed::Cancelled,
            0x08 | 0x7F if !self.pasting => {
                let mut end = password.len().saturating_sub(1);
                while end > 0 && password[end] & 0xC0 == 0x80 {
                    end -= 1;
                }
                password[end..].zeroize();
                password.truncate(end);
            }
            0x15 if !self.pasting => password.zeroize(),
            0x00..=0x1F | 0x7F => self.rejected = true,
            _ => {
                if password.len() == password.capacity() {
                    let mut grown = Vec::with_capacity(password.capacity().max(16) * 2);
                    grown.extend_from_slice(password);
                    password.zeroize();
                    *password = grown;
                }
                password.push(byte);
            }
        }
        Typed::Pending
    }

    fn continue_escape(&mut self, byte: u8) -> bool {
        if self.escape.len() == 1 {
            if byte == b'[' || byte == b'O' {
                self.escape.push(byte);
                return false;
            }
            self.rejected = true;
            match byte {
                0x1B => {}
                0x20..=0x7E | 0x80..=0xBF => self.escape.clear(),
                0xC0..=0xFF => {
                    self.escape.clear();
                    self.skip = utf8_tail(byte);
                }
                _ => {
                    self.escape.clear();
                    return true;
                }
            }
            return false;
        }
        if self.escape[1] == b'O' {
            self.escape.clear();
            self.rejected = true;
            return !(0x40..=0x7E).contains(&byte);
        }
        match byte {
            0x20..=0x3F => {
                if self.escape.len() < 16 {
                    self.escape.push(byte);
                }
                false
            }
            0x40..=0x7E => {
                if self.escape.len() < 16 {
                    self.escape.push(byte);
                }
                match self.escape.as_slice() {
                    b"\x1B[200~" => self.pasting = true,
                    b"\x1B[201~" => self.pasting = false,
                    b"\x1B[I" => self.focus = Some(true),
                    b"\x1B[O" => self.focus = Some(false),
                    b"\x1B[M" => {
                        self.skip = 3;
                        self.rejected = true;
                    }
                    _ => self.rejected = true,
                }
                self.escape.clear();
                false
            }
            _ => {
                self.escape.clear();
                self.rejected = true;
                true
            }
        }
    }
}

use zeroize::Zeroize;

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(chunks: &[&[u8]]) -> (Vec<Typed>, Vec<u8>, bool) {
        let mut reader = SecretReader::default();
        let mut password = Vec::new();
        let results = chunks
            .iter()
            .map(|chunk| reader.absorb(chunk, &mut password).0)
            .collect();
        (results, password, reader.rejected)
    }

    #[test]
    fn a_line_is_a_password() {
        assert_eq!(
            typed(&[b"Kohlrabi-Gewitter-17\n"]),
            (
                vec![Typed::Confirmed],
                b"Kohlrabi-Gewitter-17".to_vec(),
                false
            )
        );
        assert_eq!(
            typed(&[b"Kohlrabi-", b"Gewitter-17\n"]),
            (
                vec![Typed::Pending, Typed::Confirmed],
                b"Kohlrabi-Gewitter-17".to_vec(),
                false
            )
        );
        assert_eq!(
            typed(&["Gänseblümchen\n".as_bytes()]).1,
            "Gänseblümchen".as_bytes()
        );
        assert_eq!(typed(&[b"\n"]), (vec![Typed::Confirmed], Vec::new(), false));
        assert_eq!(typed(&[b"geheim\x03"]).0, vec![Typed::Cancelled]);
    }

    #[test]
    fn pasted_line_breaks_do_not_confirm() {
        let (results, password, rejected) = typed(&[
            b"\x1B[200~Kohlrabi\n",
            b"\n",
            b"Gewitter\x01",
            b"17\x1B[201~\n",
        ]);
        assert_eq!(
            results,
            vec![
                Typed::Pending,
                Typed::Pending,
                Typed::Pending,
                Typed::Confirmed
            ]
        );
        assert_eq!(password, b"KohlrabiGewitter17");
        assert!(rejected);
        assert_eq!(
            typed(&[b"\x1B[200~geheim\x1B[201~\n"]),
            (vec![Typed::Confirmed], b"geheim".to_vec(), false)
        );
        assert_eq!(
            typed(&[b"\x1B[200~geheim\n", b"\x03"]).0,
            vec![Typed::Pending, Typed::Cancelled]
        );
    }

    #[test]
    fn arbitrary_input_never_reaches_the_password_as_control_bytes() {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let alphabet = b"\x1B[O~0123456789;<M\n\x03\x01\x7Fa\xC3\xA4\xF0";
        for _ in 0..if cfg!(miri) { 40 } else { 4000 } {
            let mut reader = SecretReader::default();
            let mut password = Vec::new();
            for _ in 0..=next() % 6 {
                let len = usize::try_from(next() % 24).unwrap();
                let chunk: Vec<u8> = (0..len)
                    .map(|_| {
                        let pick = usize::try_from(next()).unwrap();
                        if pick % 3 == 0 {
                            next().to_le_bytes()[0]
                        } else {
                            alphabet[pick % alphabet.len()]
                        }
                    })
                    .collect();
                let (typed, used) = reader.absorb(&chunk, &mut password);
                assert!(password.iter().all(|&b| !matches!(b, 0x00..=0x1F | 0x7F)));
                assert!(used <= chunk.len());
                match typed {
                    Typed::Confirmed => assert!(matches!(chunk[used - 1], b'\n' | b'\r')),
                    Typed::Cancelled => assert!(chunk.contains(&0x03)),
                    Typed::Pending => continue,
                }
                break;
            }
        }
    }

    #[test]
    fn keys_and_sequences_are_left_out() {
        let (results, password, rejected) =
            typed(&[b"\x1BOA\x1B[A\x1B[1;5Ca\x01bc\x1Bd\x1B\xC3\xA4e\x1B[M !!f\x1B[<0;5;5Mg\n"]);
        assert_eq!(results, vec![Typed::Confirmed]);
        assert_eq!(password, b"abcefg");
        assert!(rejected);
        let long = [b"\x1B[".as_slice(), &[b'1'; 40], b"~z\n"].concat();
        assert_eq!(typed(&[&long]).1, b"z");
        assert_eq!(typed(&[b"\x1B", b"[200~p\n", b"\x1B[201~\n"]).1, b"p");
        assert_eq!(
            typed(&[b"x\x1B\n"]),
            (vec![Typed::Confirmed], b"x".to_vec(), true)
        );
        assert_eq!(typed(&[b"\x1B[12\n"]).0, vec![Typed::Confirmed]);
        assert_eq!(typed(&[b"\x1B\x1B\x1BOPq\n"]).1, b"q");
    }

    #[test]
    fn focus_reports_are_noted_and_left_out() {
        let mut reader = SecretReader::default();
        let mut password = Vec::new();
        assert_eq!(
            reader.absorb(b"\x1B[Oab\x1B[Ic\n", &mut password),
            (Typed::Confirmed, 10)
        );
        assert_eq!(password, b"abc");
        assert_eq!(reader.focus, Some(true));
        assert!(!reader.rejected);
    }

    #[test]
    fn typed_ahead_lines_leave_the_rest() {
        let mut reader = SecretReader::default();
        let mut password = Vec::new();
        assert_eq!(
            reader.absorb(b"erstes\rzweites\r", &mut password),
            (Typed::Confirmed, 7)
        );
        assert_eq!(password, b"erstes");
        for (input, used) in [(b"a\r\nb".as_slice(), 3), (b"a\nb", 2), (b"a\x03b", 2)] {
            let mut reader = SecretReader::default();
            assert_eq!(reader.absorb(input, &mut Vec::new()).1, used);
        }
        assert_eq!(
            typed(&[b"\x1B[200~a\rb\x1B[201~\r"]),
            (vec![Typed::Confirmed], b"ab".to_vec(), true)
        );
    }

    #[test]
    fn erase_and_kill_work_on_typed_ahead_keys() {
        assert_eq!(typed(&[b"abX\x7F\r"]).1, b"ab");
        assert_eq!(typed(&["aä\x08\r".as_bytes()]).1, b"a");
        assert_eq!(typed(&[b"\x7F\x7Fz\r"]).1, b"z");
        assert_eq!(typed(&[b"falsch\x15richtig\r"]).1, b"richtig");
        assert_eq!(
            typed(&[b"\x1B[200~a\x7Fb\x15c\x1B[201~\r"]),
            (vec![Typed::Confirmed], b"abc".to_vec(), true)
        );
    }

    #[test]
    fn long_passwords_survive_growing() {
        let long: Vec<u8> = (0..1000)
            .map(|i| b'a' + u8::try_from(i % 26).unwrap())
            .collect();
        let mut reader = SecretReader::default();
        let mut password = Vec::with_capacity(4);
        assert_eq!(reader.absorb(&long, &mut password), (Typed::Pending, 1000));
        assert_eq!(password, long);
    }
}
