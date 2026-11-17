#![expect(
    clippy::wrong_self_convention,
    reason = "keeps the GNU nano names to_prev_anchor() and to_next_anchor()"
)]

use crate::utils::is_separate_word;
use crate::utils::{i32_of, i64_of, isize_of, usize_of};
use std::time::{Duration, Instant};

use zeroize::Zeroize;

use crate::chars::{
    char_length, mbrevstrpbrk, mbstrchr, mbstrlen, mbstrpbrk, step_left, step_right,
};
use crate::curses::Win;
use crate::definitions::{
    ALL, CANCEL, CUSHION, ERR, ESC_CODE, Flag, Func, INREGION, JUSTFIND, Lp, MFINDINHELP,
    MGOTOLINE, MREPLACE, MREPLACEWITH, MWHEREIS, Msg, NO, PRUNE_DUPLICATE, REPLACING, UndoType,
    Update, YES, YESORALLORNO,
};
use crate::global::Zu;
use crate::regex;

impl Zu {
    pub fn regexp_init(&mut self, pattern: &[u8]) -> bool {
        let flags = regex::REG_EXTENDED
            | if self.isset(Flag::CaseSensitive) {
                0
            } else {
                regex::REG_ICASE
            };
        match regex::compile(pattern, flags) {
            Ok(re) => {
                self.search_regexp = Some(re);
                true
            }
            Err(e) => {
                let p = String::from_utf8_lossy(pattern).into_owned();
                self.statusline(
                    Msg::Ahem,
                    &fmt!("Ungültiger regulärer Ausdruck „%s“: %s", &p, &e),
                );
                false
            }
        }
    }

    pub fn tidy_up_after_search(&mut self) {
        self.search_regexp = None;
        if self.ofile().mark.is_some() {
            self.refresh_needed = true;
        }
    }

    pub fn search_init(&mut self, mut replacing: bool, mut retain_answer: bool) {
        let cols = self.cols();
        let thedefault: Vec<u8> = if self.last_search.is_empty() {
            Vec::new()
        } else {
            let ls = self.last_search.clone();
            let disp = self.display_string(&ls, 0, cols / 3, false, false);
            let mut v = b" [".to_vec();
            v.extend_from_slice(&disp);
            if self.breadth(&ls) > cols / 3 {
                v.extend_from_slice(b"...");
            }
            v.push(b']');
            v
        };
        loop {
            let menu = if self.inhelp {
                MFINDINHELP
            } else if replacing {
                MREPLACE
            } else {
                MWHEREIS
            };
            let provided = if retain_answer {
                self.answer.clone()
            } else {
                Vec::new()
            };
            let mut msg = String::from("Suche");
            if self.isset(Flag::CaseSensitive) {
                msg.push_str(" [Groß/klein beachten]");
            }
            if self.isset(Flag::UseRegexp) {
                msg.push_str(" [reg. Ausdr.]");
            }
            if self.isset(Flag::BackwardsSearch) {
                msg.push_str(" [Rückwärts]");
            }
            if replacing {
                if self.ofile().mark.is_some() {
                    msg.push_str(" (zu ersetzen) in Auswahl");
                } else {
                    msg.push_str(" (zu ersetzen)");
                }
            }
            let mut msgb = msg.into_bytes();
            msgb.extend_from_slice(&thedefault);
            let response = self.do_prompt(
                menu,
                &provided,
                Some(crate::definitions::HistKind::Search),
                RefreshFn::Edit,
                &msgb,
            );
            if response == -1 || (response == -2 && self.last_search.is_empty()) {
                self.statusbar("Abgebrochen");
                break;
            }
            if response == 0 || response == -2 {
                if !self.answer.is_empty() {
                    self.last_search = self.answer.clone();
                    let a = self.answer.clone();
                    self.update_history(crate::definitions::HistKind::Search, &a, PRUNE_DUPLICATE);
                }
                if self.isset(Flag::UseRegexp) {
                    let ls = self.last_search.clone();
                    if !self.regexp_init(&ls) {
                        break;
                    }
                }
                if replacing {
                    self.ask_for_and_do_replacements();
                } else {
                    self.go_looking();
                }
                break;
            }
            retain_answer = true;
            let function = self.func_from_key(response);
            match function {
                Some(Func::CaseSens) => self.flags.toggle(Flag::CaseSensitive),
                Some(Func::Backwards) => self.flags.toggle(Flag::BackwardsSearch),
                Some(Func::Regexp) => self.flags.toggle(Flag::UseRegexp),
                Some(Func::FlipReplace) => {
                    if self.isset(Flag::ViewMode) {
                        self.print_view_warning();
                        self.term.napms(600);
                    } else {
                        replacing = !replacing;
                    }
                }
                Some(Func::FlipGoto) => {
                    let a = self.answer.clone();
                    self.ask_for_line_and_column(&a);
                    break;
                }
                _ => break,
            }
        }
        if !self.inhelp {
            self.tidy_up_after_search();
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "same parameters as findnextstr() in GNU nano"
    )]
    pub fn findnextstr(
        &mut self,
        needle: &[u8],
        whole_word_only: bool,
        modus: i32,
        match_len: Option<&mut usize>,
        mut skipone: bool,
        begin: Option<Lp>,
        begin_x: usize,
    ) -> i32 {
        let mut found_len = needle.len();
        let mut feedback: i32 = 0;
        let mut line = self.current();
        let mut from = self.current_x();
        let mut found: Option<usize>;
        let mut lastkbcheck = Instant::now();
        if begin.is_none() {
            self.came_full_circle = false;
        }
        loop {
            let data = self.nodes[line].data.clone();
            found = None;
            if skipone {
                skipone = false;
                if self.isset(Flag::BackwardsSearch) && from > 0 {
                    from = step_left(&data, from);
                    found = self.strstrwrapper(&data, needle, from);
                } else if !self.isset(Flag::BackwardsSearch) && from < data.len() {
                    from += char_length(&data[from..]);
                    found = self.strstrwrapper(&data, needle, from);
                }
            } else {
                found = self.strstrwrapper(&data, needle, from.min(data.len()));
            }
            if let Some(f) = found {
                if self.isset(Flag::UseRegexp) {
                    found_len = usize_of(crate::regex::offset(
                        self.regmatches[0].rm_eo - self.regmatches[0].rm_so,
                    ));
                }
                if whole_word_only && !is_separate_word(f, found_len, &data) {
                    from = f + char_length(&data[f..]);
                    continue;
                }
                if self.next(line).is_some() || !data.is_empty() {
                    break;
                }
            }
            if self.the_window_resized {
                self.regenerate_screen();
                self.statusbar("Suchen …");
                feedback = 1;
            }
            if self.came_full_circle {
                return 0;
            }
            let nl = if self.isset(Flag::BackwardsSearch) {
                self.prev(line)
            } else {
                self.next(line)
            };
            line = if let Some(l) = nl {
                l
            } else {
                if whole_word_only || modus == INREGION {
                    return 0;
                }
                let l = if self.isset(Flag::BackwardsSearch) {
                    self.filebot()
                } else {
                    self.filetop()
                };
                if modus == JUSTFIND {
                    self.statusline(Msg::Remark, "Suche umbrechen");
                    feedback = -2;
                }
                l
            };
            if Some(line) == begin {
                self.came_full_circle = true;
            }
            from = if self.isset(Flag::BackwardsSearch) {
                self.datalen(line)
            } else {
                0
            };
            if lastkbcheck.elapsed() > Duration::from_secs(1) {
                lastkbcheck = Instant::now();
                let mut pending: Vec<i32> = Vec::new();
                self.term
                    .read_available(&mut |byte| pending.push(i32::from(byte)));
                let mut index = 0;
                while index < pending.len() {
                    let mut input = pending[index];
                    index += 1;
                    if input == ESC_CODE {
                        if index >= pending.len() {
                            self.term.napms(20);
                            self.term
                                .read_available(&mut |byte| pending.push(i32::from(byte)));
                        }
                        input = pending.get(index).copied().unwrap_or(ERR);
                        index += 1;
                        self.meta_key = true;
                    } else {
                        self.meta_key = false;
                    }
                    if self.func_from_key(input) == Some(Func::Cancel) {
                        self.check_signals();
                        if self.the_window_resized {
                            self.regenerate_screen();
                        }
                        self.statusbar("Abgebrochen");
                        self.clear_key_buffer();
                        pending.zeroize();
                        return -2;
                    }
                }
                pending.zeroize();
                feedback += 1;
                if feedback > 0 {
                    self.statusbar("Suchen …");
                }
            }
        }
        let found_x = found.unwrap();
        if self.came_full_circle
            && ((!self.isset(Flag::BackwardsSearch)
                && (found_x > begin_x || (modus == REPLACING && found_x == begin_x)))
                || (self.isset(Flag::BackwardsSearch) && found_x < begin_x))
        {
            return 0;
        }
        {
            let of = self.ofile_mut();
            of.current = line;
            of.current_x = found_x;
        }
        if let Some(ml) = match_len {
            *ml = found_len;
        }
        if modus == JUSTFIND && (self.ofile().mark.is_none() || self.ofile().softmark) {
            self.spotlighted = true;
            self.light_from_col = self.xplustabs();
            self.light_to_col = self.wideness(self.data(line), found_x + found_len);
            if self.united_sidescroll && self.light_to_col < self.editwincols - CUSHION {
                self.ofile_mut().brink = 0;
            } else if self.united_sidescroll {
                let b = self.get_page_start(self.light_to_col);
                self.ofile_mut().brink = b;
            }
            self.refresh_needed = true;
        }
        if feedback > 0 {
            self.wipe_statusbar();
        }
        1
    }

    pub fn do_search_forward(&mut self) {
        self.flags.unset(Flag::BackwardsSearch);
        self.search_init(false, false);
    }

    pub fn do_search_backward(&mut self) {
        self.flags.set(Flag::BackwardsSearch);
        self.search_init(false, false);
    }

    pub fn do_research(&mut self) {
        if self.last_search.is_empty() && self.search_history.items.len() > 1 {
            let i = self.search_history.items.len() - 2;
            self.last_search = self.search_history.items[i].clone();
        }
        if self.last_search.is_empty() {
            self.statusline(Msg::Ahem, "Kein aktuelles Suchmuster");
            return;
        }
        if self.isset(Flag::UseRegexp) {
            let ls = self.last_search.clone();
            if !self.regexp_init(&ls) {
                return;
            }
        }
        self.currmenu = MWHEREIS;
        if self.lines() > 1 {
            self.wipe_statusbar();
        }
        self.go_looking();
        if !self.inhelp {
            self.tidy_up_after_search();
        }
    }

    pub fn do_findprevious(&mut self) {
        self.flags.set(Flag::BackwardsSearch);
        self.do_research();
    }

    pub fn do_findnext(&mut self) {
        self.flags.unset(Flag::BackwardsSearch);
        self.do_research();
    }

    pub fn not_found_msg(&mut self, s: &[u8]) {
        let cols = self.cols();
        let disp = self.display_string(s, 0, cols / 2 + 1, false, false);
        let numchars = self.actual_x(&disp, self.wideness(&disp, cols / 2));
        let tail = if numchars >= disp.len() { "" } else { "..." };
        let msg = fmtb!("„%.*s%s“ nicht gefunden", numchars, &disp, tail);
        self.statusline_bytes(Msg::Ahem, &msg);
    }

    pub fn go_looking(&mut self) {
        let was_current = self.current();
        let was_x = self.current_x();
        self.came_full_circle = false;
        let needle = self.last_search.clone();
        self.didfind = self.findnextstr(
            &needle,
            false,
            JUSTFIND,
            None,
            true,
            Some(was_current),
            was_x,
        );
        if self.didfind == 1 && self.current() == was_current && self.current_x() == was_x {
            self.statusline(Msg::Remark, "Das ist das einzige Auftreten");
        } else if self.didfind == 0 {
            self.not_found_msg(&needle);
        }
        self.edit_redraw(was_current, Update::Centering);
    }

    fn replace_regexp(&self) -> Vec<u8> {
        let given = &self.answer;
        let nsub = self.search_regexp.as_ref().map_or(0, |r| r.nsub);
        let data = self.cur_data();
        let mut out = Vec::new();
        let mut i = 0;
        while i < given.len() {
            let digit = given.get(i + 1).map_or(-1, |&d| i32::from(d) - '0' as i32);
            if given[i] == b'\\' && digit > 0 && digit < 10 && (usize_of(i64::from(digit))) <= nsub
            {
                let m = self.regmatches[usize_of(i64::from(digit))];
                if m.rm_so >= 0 && m.rm_eo >= m.rm_so {
                    let (so, eo) = (
                        usize_of(crate::regex::offset(m.rm_so)),
                        usize_of(crate::regex::offset(m.rm_eo)),
                    );
                    if eo <= data.len() {
                        out.extend_from_slice(&data[so..eo]);
                    }
                }
                i += 2;
            } else {
                out.push(given[i]);
                i += 1;
            }
        }
        out
    }

    fn replace_line(&self, needle: &[u8]) -> (Vec<u8>, usize) {
        let data = self.cur_data();
        let x = self.current_x();
        let (match_len, replacement) = if self.isset(Flag::UseRegexp) {
            (
                usize_of(crate::regex::offset(
                    self.regmatches[0].rm_eo - self.regmatches[0].rm_so,
                )),
                self.replace_regexp(),
            )
        } else {
            (needle.len(), self.answer.clone())
        };
        let mut copy = Vec::with_capacity(data.len() + replacement.len());
        copy.extend_from_slice(&data[..x.min(data.len())]);
        copy.extend_from_slice(&replacement);
        copy.extend_from_slice(&data[(x + match_len).min(data.len())..]);
        (copy, match_len)
    }

    pub fn do_replace_loop(
        &mut self,
        needle: &[u8],
        whole_word_only: bool,
        real_current: Lp,
        real_current_x: &mut usize,
    ) -> i64 {
        let mut skipone = self.isset(Flag::BackwardsSearch);
        let mut replaceall = false;
        let mut modus = REPLACING;
        let mut numreplaced: i64 = -1;
        let mut match_len = 0usize;
        let was_mark = self.ofile().mark;
        let right_side_up = was_mark.is_some() && self.mark_is_before_cursor();
        let (mut top, mut top_x, mut bot, mut bot_x) =
            (self.filetop(), 0usize, self.filebot(), 0usize);
        if was_mark.is_some() {
            let r = self.get_region();
            top = r.0;
            top_x = r.1;
            bot = r.2;
            bot_x = r.3;
            self.ofile_mut().mark = None;
            modus = INREGION;
            if self.isset(Flag::BackwardsSearch) {
                let of = self.ofile_mut();
                of.current = bot;
                of.current_x = bot_x;
            } else {
                let of = self.ofile_mut();
                of.current = top;
                of.current_x = top_x;
            }
        }
        self.came_full_circle = false;
        loop {
            let mut choice = NO;
            let result = self.findnextstr(
                needle,
                whole_word_only,
                modus,
                Some(&mut match_len),
                skipone,
                Some(real_current),
                *real_current_x,
            );
            if result < 1 {
                if result < 0 {
                    numreplaced = -2;
                }
                break;
            }
            if was_mark.is_some() {
                let c = self.current();
                let cx = self.current_x();
                if self.lineno(c) > self.lineno(bot)
                    || self.lineno(c) < self.lineno(top)
                    || (c == bot && cx + match_len > bot_x)
                    || (c == top && cx < top_x)
                {
                    break;
                }
            }
            if numreplaced == -1 {
                numreplaced = 0;
            }
            if !replaceall {
                self.spotlighted = true;
                self.light_from_col = self.xplustabs();
                let c = self.current();
                self.light_to_col = self.wideness(self.data(c), self.current_x() + match_len);
                if self.united_sidescroll && self.light_to_col < self.editwincols - CUSHION {
                    self.ofile_mut().brink = 0;
                } else if self.united_sidescroll {
                    let b = self.get_page_start(self.light_to_col);
                    self.ofile_mut().brink = b;
                }
                self.edit_refresh();
                choice = self.ask_user(YESORALLORNO, "Fundstelle ersetzen?");
                self.spotlighted = false;
                if choice == CANCEL {
                    break;
                }
                replaceall = choice == ALL;
                skipone = choice == 0 || self.isset(Flag::BackwardsSearch);
            }
            if choice == YES || replaceall {
                let (altered, mlen) = self.replace_line(needle);
                let c = self.current();
                let length_change = i64_of(altered.len()) - i64_of(self.datalen(c));
                self.add_undo(UndoType::Replace, None);
                if was_mark.is_some() && !right_side_up {
                    let of = self.ofile_mut();
                    if Some(of.current) == was_mark && of.mark_x > of.current_x {
                        if of.mark_x < of.current_x + mlen {
                            of.mark_x = of.current_x;
                        } else {
                            of.mark_x = usize_of(i64_of(of.mark_x) + length_change);
                        }
                        bot_x = of.mark_x;
                    }
                }
                if (was_mark.is_none() || right_side_up)
                    && self.current() == real_current
                    && self.current_x() < *real_current_x
                {
                    if *real_current_x < self.current_x() + mlen {
                        *real_current_x = self.current_x() + mlen;
                    }
                    *real_current_x = usize_of(i64_of(*real_current_x) + length_change);
                    bot_x = *real_current_x;
                }
                if mlen == 0 || (needle.first() == Some(&b'^') && self.isset(Flag::UseRegexp)) {
                    skipone = true;
                }
                if !self.isset(Flag::BackwardsSearch) {
                    let of = self.ofile_mut();
                    of.current_x = usize_of(i64_of(of.current_x) + i64_of(mlen) + length_change);
                }
                let old_chars = i64_of(mbstrlen(self.data(c)));
                let new_chars = i64_of(mbstrlen(&altered));
                let of = self.ofile_mut();
                of.totsize = usize_of(i64_of(of.totsize) + new_chars - old_chars);
                self.nodes[c].data = altered;
                self.set_modified();
                self.as_an_at = true;
                numreplaced += 1;
            }
        }
        if numreplaced == -1 {
            self.not_found_msg(needle);
        }
        self.ofile_mut().mark = was_mark;
        numreplaced
    }

    pub fn do_replace(&mut self) {
        if self.isset(Flag::ViewMode) {
            self.print_view_warning();
        } else {
            self.flags.unset(Flag::BackwardsSearch);
            self.search_init(true, false);
        }
    }

    pub fn ask_for_and_do_replacements(&mut self) {
        let was_edittop = self.edittop();
        let was_firstcolumn = self.ofile().firstcolumn;
        let beginline = self.current();
        let mut begin_x = self.current_x();
        let replacee = self.last_search.clone();
        let response = self.do_prompt(
            MREPLACEWITH,
            b"",
            Some(crate::definitions::HistKind::Replace),
            RefreshFn::Edit,
            b"Ersetzen durch",
        );
        self.last_search.clone_from(&replacee);
        if response == 0 {
            let a = self.answer.clone();
            self.update_history(crate::definitions::HistKind::Replace, &a, PRUNE_DUPLICATE);
        }
        if response == -1 {
            self.statusbar("Abgebrochen");
            return;
        } else if response > 0 {
            return;
        }
        let numreplaced = self.do_replace_loop(&replacee, false, beginline, &mut begin_x);
        {
            let of = self.ofile_mut();
            of.edittop = was_edittop;
            of.firstcolumn = was_firstcolumn;
            of.current = beginline;
            of.current_x = begin_x;
        }
        self.refresh_needed = true;
        if numreplaced >= 0 {
            let m = fmt!(
                crate::utils::plural(
                    "%zd Ersetzung vorgenommen",
                    "%zd Ersetzungen vorgenommen",
                    numreplaced.cast_unsigned()
                ),
                numreplaced
            );
            self.statusline(Msg::Remark, &m);
        }
    }

    pub fn goto_line_posx(&mut self, linenumber: i64, pos_x: usize) {
        let target = if linenumber < self.lineno(self.filebot()) {
            self.line_from_number(linenumber)
        } else {
            self.filebot()
        };
        let of = self.ofile_mut();
        of.current = target;
        of.current_x = pos_x;
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
        self.refresh_needed = true;
    }

    pub fn do_gotolinecolumn(&mut self) {
        self.ask_for_line_and_column(b"");
    }

    pub fn ask_for_line_and_column(&mut self, provided: &[u8]) {
        let mut line = isize_of(self.lineno(self.current()));
        let mut column = self.ofile().placewewant.cast_signed() + 1;
        let response = self.do_prompt(
            MGOTOLINE,
            provided,
            None,
            RefreshFn::Edit,
            "Zeilennummer, Spaltennummer eingeben".as_bytes(),
        );
        if self.func_from_key(response) == Some(Func::FlipGoto) {
            self.flags.unset(Flag::BackwardsSearch);
            self.search_init(false, true);
            return;
        }
        if response < 0 {
            self.statusbar("Abgebrochen");
            return;
        } else if response > 0 {
            return;
        }
        let a = self.answer.clone();
        let doublesign = usize::from(
            a.len() >= 2 && ((a[0] == b'+' && a[1] == b'+') || (a[0] == b'-' && a[1] == b'-')),
        );
        if !crate::utils::parse_line_column(&a[doublesign..], &mut line, &mut column) {
            self.statusline(Msg::Ahem, "Ungültige Zeilen- oder Spaltennummer");
            return;
        }
        if doublesign == 1 {
            line += isize_of(self.lineno(self.current()));
            if line < 1 {
                line = 1;
            }
        }
        self.goto_line_and_column(line, column, false);
        self.adjust_viewport(if a.first() == Some(&b',') {
            Update::Stationary
        } else {
            Update::Centering
        });
        self.refresh_needed = true;
    }

    pub fn goto_line_and_column(&mut self, mut line: isize, mut column: isize, hugfloor: bool) {
        if line < 0 {
            line = isize_of(self.lineno(self.filebot())) + line + 1;
        } else if line == 0 {
            line = isize_of(self.lineno(self.current()));
        }
        if line < 1 {
            line = 1;
        }
        let mut cur = self.filetop();
        while line > 1 && cur != self.filebot() {
            cur = self.next(cur).unwrap();
            line -= 1;
        }
        self.ofile_mut().current = cur;
        if column < 0 {
            column = self.breadth(self.data(cur)).cast_signed() + column + 2;
        } else if column == 0 {
            column = self.ofile().placewewant.cast_signed() + 1;
        }
        if column < 1 {
            column = 1;
        }
        let x = self.actual_x(self.data(cur), column.cast_unsigned() - 1);
        {
            let of = self.ofile_mut();
            of.current_x = x;
            of.placewewant = column.cast_unsigned() - 1;
        }
        if self.isset(Flag::SoftWrap)
            && self.ofile().placewewant / self.editwincols
                > self.breadth(self.data(cur)) / self.editwincols
        {
            let b = self.breadth(self.data(cur));
            self.ofile_mut().placewewant = b;
        }
        if !hugfloor {
            return;
        }

        let rows_from_tail: i64 = if self.isset(Flag::SoftWrap) {
            let mut currentline = cur;
            let mut leftedge = self.leftedge_for(self.xplustabs(), cur);
            let half = i32_of(i64_of(self.editwinrows / 2));
            i64::from(half - self.go_forward_chunks(half, &mut currentline, &mut leftedge))
        } else {
            self.lineno(self.filebot()) - self.lineno(cur)
        };
        if rows_from_tail < i64_of(self.editwinrows / 2) && !self.isset(Flag::JumpyScrolling) {
            self.ofile_mut().cursor_row = i64_of(self.editwinrows) - 1 - rows_from_tail;
            self.adjust_viewport(Update::Stationary);
        } else {
            self.adjust_viewport(Update::Centering);
        }
    }

    fn find_a_bracket(&mut self, reverse: bool, bracket_pair: &[u8]) -> bool {
        let mut line = self.current();
        let found: usize;
        if reverse {
            let mut pointer = if self.current_x() == 0 {
                line = match self.prev(line) {
                    Some(p) => p,
                    None => return false,
                };
                self.datalen(line)
            } else {
                step_left(self.data(line), self.current_x())
            };
            loop {
                if let Some(f) = mbrevstrpbrk(self.data(line), bracket_pair, pointer) {
                    found = f;
                    break;
                }
                line = match self.prev(line) {
                    Some(p) => p,
                    None => return false,
                };
                pointer = self.datalen(line);
            }
        } else {
            let mut pointer = step_right(self.data(line), self.current_x());
            loop {
                let d = self.data(line);
                if pointer <= d.len()
                    && let Some(f) = mbstrpbrk(&d[pointer..], bracket_pair)
                {
                    found = f + pointer;
                    break;
                }
                line = match self.next(line) {
                    Some(n) => n,
                    None => return false,
                };
                pointer = 0;
            }
        }
        let of = self.ofile_mut();
        of.current = line;
        of.current_x = found;
        true
    }

    pub fn do_find_bracket(&mut self) {
        let was_current = self.current();
        let was_x = self.current_x();
        let mb = self.matchbrackets.clone();
        let Some(ch) = mbstrchr(
            &mb,
            &self.nodes[was_current].data[was_x.min(self.datalen(was_current))..],
        ) else {
            self.statusline(Msg::Ahem, "Keine Klammer");
            return;
        };
        let mut charcount = mbstrlen(&mb) / 2;
        let mut halfway = 0usize;
        for _ in 0..charcount {
            halfway += char_length(&mb[halfway..]);
        }
        let reverse = ch >= halfway;
        let mut wanted_ch = ch;
        while charcount > 0 {
            if reverse {
                wanted_ch = step_left(&mb, wanted_ch);
            } else {
                wanted_ch += char_length(&mb[wanted_ch..]);
            }
            charcount -= 1;
        }
        let ch_len = char_length(&mb[ch..]);
        let wanted_ch_len = char_length(&mb[wanted_ch..]);
        let mut bracket_pair: Vec<u8> = mb[ch..ch + ch_len].to_vec();
        bracket_pair.extend_from_slice(&mb[wanted_ch..wanted_ch + wanted_ch_len]);
        let this_bracket: Vec<u8> = mb[ch..ch + ch_len].to_vec();
        let mut balance: i64 = 1;
        while self.find_a_bracket(reverse, &bracket_pair) {
            let c = self.current();
            let x = self.current_x();
            let d = &self.nodes[c].data;
            balance += if d.len() >= x + ch_len && d[x..x + ch_len] == this_bracket[..] {
                1
            } else {
                -1
            };
            if balance == 0 {
                self.edit_redraw(was_current, Update::Flowing);
                return;
            }
        }
        self.statusline(Msg::Ahem, "Keine passende Klammer");
        let of = self.ofile_mut();
        of.current = was_current;
        of.current_x = was_x;
    }

    pub fn put_or_lift_anchor(&mut self) {
        let c = self.current();
        self.nodes[c].has_anchor = !self.nodes[c].has_anchor;
        if c == self.filetop() {
            self.refresh_needed = true;
        } else {
            let x = self.current_x();
            self.update_line(c, x);
        }
        if !self.isset(Flag::LineNumbers) && (!self.isset(Flag::MiniBar) || self.isset(Flag::Zero))
        {
            if self.nodes[c].has_anchor {
                self.statusline(Msg::Remark, "Anker gesetzt");
            } else {
                self.statusline(Msg::Hush, "Anker entfernt");
            }
        }
    }

    fn go_to_and_confirm(&mut self, line: Lp) {
        let was_current = self.current();
        if line != was_current {
            let of = self.ofile_mut();
            of.current = line;
            of.current_x = 0;
            self.edit_redraw(was_current, Update::Centering);
            if !self.isset(Flag::LineNumbers) {
                self.statusbar("Zum Anker gesprungen");
            }
        } else if self.nodes[was_current].has_anchor {
            self.statusline(Msg::Remark, "Das ist der einzige Anker");
        } else {
            self.statusline(Msg::Ahem, "Es gibt keine Anker");
        }
    }

    pub fn to_prev_anchor(&mut self) {
        let cur = self.current();
        let mut line = cur;
        loop {
            line = self.prev(line).unwrap_or_else(|| self.filebot());
            if self.nodes[line].has_anchor || line == cur {
                break;
            }
        }
        self.go_to_and_confirm(line);
    }

    pub fn to_next_anchor(&mut self) {
        let cur = self.current();
        let mut line = cur;
        loop {
            line = self.next(line).unwrap_or_else(|| self.filetop());
            if self.nodes[line].has_anchor || line == cur {
                break;
            }
        }
        self.go_to_and_confirm(line);
    }

    pub fn refresh_window(&mut self, which: RefreshFn) {
        match which {
            RefreshFn::Edit => self.edit_refresh(),
            RefreshFn::Browser => self.browser_refresh(),
        }
        let _ = Win::Mid;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefreshFn {
    Edit,
    Browser,
}
