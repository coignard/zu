#![expect(
    clippy::wrong_self_convention,
    reason = "keeps the GNU nano names to_first_line(), to_next_word() and friends"
)]

use crate::chars::{is_zerowidth, step_left, step_right, white_string};
use crate::definitions::{BACKWARD, CUSHION, FORWARD, Flag, Lp, Msg, Update};
use crate::global::Zu;
use crate::text::indent_length;
use crate::utils::{i32_of, i64_of};

impl Zu {
    pub fn to_first_line(&mut self) {
        let top = self.filetop();
        let of = self.ofile_mut();
        of.current = top;
        of.current_x = 0;
        of.placewewant = 0;
        self.refresh_needed = true;
    }

    pub fn to_last_line(&mut self) {
        let bot = self.filebot();
        let len = if self.inhelp { 0 } else { self.datalen(bot) };
        {
            let of = self.ofile_mut();
            of.current = bot;
            of.current_x = len;
        }
        let pww = self.xplustabs();
        let rows = i64_of(self.editwinrows);
        let of = self.ofile_mut();
        of.placewewant = pww;
        of.cursor_row = rows - 1;
        self.refresh_needed = true;
        self.focusing = false;
    }

    pub fn get_edge_and_target(&self) -> (usize, usize) {
        if self.isset(Flag::SoftWrap) {
            let shim =
                self.editwincols * (1 + (self.tabsize.cast_unsigned() / self.editwincols.max(1)));
            let leftedge = self.leftedge_for(self.xplustabs(), self.current());
            let target = (self.ofile().placewewant + shim - leftedge) % self.editwincols.max(1);
            (leftedge, target)
        } else {
            (0, self.ofile().placewewant)
        }
    }

    pub fn proper_x(
        &self,
        line: Lp,
        leftedge: &mut usize,
        forward: bool,
        column: usize,
        shifted: Option<&mut bool>,
    ) -> usize {
        let data = self.data(line);
        let mut index = self.actual_x(data, column);
        if self.isset(Flag::SoftWrap) && self.byte_at(line, index) == b'\t' {
            let ts = self.tabsize.cast_unsigned();
            if (forward && self.wideness(data, index) < *leftedge)
                || (!forward
                    && column / ts == (leftedge.wrapping_sub(1)) / ts
                    && column / ts < (*leftedge + self.editwincols - 1) / ts)
            {
                index += 1;
                if let Some(s) = shifted {
                    *s = true;
                }
            }
        }
        if self.isset(Flag::SoftWrap) {
            *leftedge = self.leftedge_for(self.wideness(data, index), line);
        }
        index
    }

    pub fn set_proper_index_and_pww(&mut self, leftedge: &mut usize, target: usize, forward: bool) {
        let was_edge = *leftedge;
        let mut shifted = false;
        let cur = self.current();
        let col = self.actual_last_column(*leftedge, target);
        let x = self.proper_x(cur, leftedge, forward, col, Some(&mut shifted));
        self.ofile_mut().current_x = x;
        if shifted || *leftedge < was_edge {
            let col = self.actual_last_column(*leftedge, target);
            let x = self.proper_x(cur, leftedge, forward, col, Some(&mut shifted));
            self.ofile_mut().current_x = x;
        }
        self.ofile_mut().placewewant = *leftedge + target;
    }

    pub fn do_page_up(&mut self) {
        let mustmove = i32_of(i64_of(if self.editwinrows < 3 {
            1
        } else {
            self.editwinrows - 2
        }));
        let (mut leftedge, target_column);
        if self.isset(Flag::JumpyScrolling) {
            let top = self.edittop();
            let of = self.ofile_mut();
            of.current = top;
            leftedge = of.firstcolumn;
            of.cursor_row = 0;
            target_column = 0;
        } else {
            let (l, t) = self.get_edge_and_target();
            leftedge = l;
            target_column = t;
        }
        let mut line = self.current();
        if self.go_back_chunks(mustmove, &mut line, &mut leftedge) > 0 {
            self.to_first_line();
            return;
        }
        self.ofile_mut().current = line;
        self.set_proper_index_and_pww(&mut leftedge, target_column, false);
        self.adjust_viewport(Update::Stationary);
        self.refresh_needed = true;
    }

    pub fn do_page_down(&mut self) {
        let mustmove = i32_of(i64_of(if self.editwinrows < 3 {
            1
        } else {
            self.editwinrows - 2
        }));
        let (mut leftedge, target_column);
        if self.isset(Flag::JumpyScrolling) {
            let top = self.edittop();
            let of = self.ofile_mut();
            of.current = top;
            leftedge = of.firstcolumn;
            of.cursor_row = 0;
            target_column = 0;
        } else {
            let (l, t) = self.get_edge_and_target();
            leftedge = l;
            target_column = t;
        }
        let mut line = self.current();
        if self.go_forward_chunks(mustmove, &mut line, &mut leftedge) > 0 {
            self.to_last_line();
            return;
        }
        self.ofile_mut().current = line;
        self.set_proper_index_and_pww(&mut leftedge, target_column, true);
        self.adjust_viewport(Update::Stationary);
        self.refresh_needed = true;
    }

    pub fn to_top_row(&mut self) {
        let (_l, offset) = self.get_edge_and_target();
        let top = self.edittop();
        let mut leftedge = self.ofile().firstcolumn;
        self.ofile_mut().current = top;
        self.set_proper_index_and_pww(&mut leftedge, offset, false);
        self.refresh_needed = self.ofile().mark.is_some();
    }

    pub fn to_bottom_row(&mut self) {
        let (_l, offset) = self.get_edge_and_target();
        let mut line = self.edittop();
        let mut leftedge = self.ofile().firstcolumn;
        let rows = i32_of(i64_of(self.editwinrows));
        self.go_forward_chunks(rows - 1, &mut line, &mut leftedge);
        self.ofile_mut().current = line;
        self.set_proper_index_and_pww(&mut leftedge, offset, true);
        self.refresh_needed = self.ofile().mark.is_some();
    }

    pub fn do_cycle(&mut self) {
        if self.cycling_aim == 0 {
            self.adjust_viewport(Update::Centering);
        } else {
            let rows = i64_of(self.editwinrows);
            self.ofile_mut().cursor_row = if self.cycling_aim == 1 { 0 } else { rows - 1 };
            self.adjust_viewport(Update::Stationary);
        }
        self.cycling_aim = (self.cycling_aim + 1) % 3;
        self.draw_all_subwindows();
        self.full_refresh();
    }

    pub fn do_center(&mut self) {
        self.adjust_viewport(Update::Centering);
        self.draw_all_subwindows();
        self.full_refresh();
    }

    pub fn do_para_begin(&self, line: &mut Lp) {
        if let Some(p) = self.prev(*line) {
            *line = p;
        }
        while !self.begpar(*line, 0) {
            match self.prev(*line) {
                Some(p) => *line = p,
                None => break,
            }
        }
    }

    pub fn do_para_end(&self, line: &mut Lp) {
        while self.next(*line).is_some() && !self.inpar(*line) {
            *line = self.next(*line).unwrap();
        }
        while let Some(n) = self.next(*line) {
            if self.inpar(n) && !self.begpar(n, 0) {
                *line = n;
            } else {
                break;
            }
        }
    }

    pub fn to_para_begin(&mut self) {
        let was_current = self.current();
        let mut line = was_current;
        self.do_para_begin(&mut line);
        let of = self.ofile_mut();
        of.current = line;
        of.current_x = 0;
        self.edit_redraw(was_current, Update::Centering);
    }

    pub fn to_para_end(&mut self) {
        let was_current = self.current();
        let mut line = was_current;
        self.do_para_end(&mut line);
        if let Some(n) = self.next(line) {
            let of = self.ofile_mut();
            of.current = n;
            of.current_x = 0;
        } else {
            let len = self.datalen(line);
            let of = self.ofile_mut();
            of.current = line;
            of.current_x = len;
        }
        self.edit_redraw(was_current, Update::Centering);
    }

    pub fn to_prev_block(&mut self) {
        let was_current = self.current();
        let mut is_text = false;
        let mut seen_text = false;
        let mut cur = was_current;
        while let Some(p) = self.prev(cur) {
            if seen_text && !is_text {
                break;
            }
            cur = p;
            is_text = !white_string(self.data(cur));
            seen_text = seen_text || is_text;
        }
        if seen_text && self.next(cur).is_some() && white_string(self.data(cur)) {
            cur = self.next(cur).unwrap();
        }
        let of = self.ofile_mut();
        of.current = cur;
        of.current_x = 0;
        self.edit_redraw(was_current, Update::Centering);
    }

    pub fn to_next_block(&mut self) {
        let was_current = self.current();
        let mut cur = was_current;
        let mut is_white = white_string(self.data(cur));
        let mut seen_white = is_white;
        while let Some(n) = self.next(cur) {
            if seen_white && !is_white {
                break;
            }
            cur = n;
            is_white = white_string(self.data(cur));
            seen_white = seen_white || is_white;
        }
        let of = self.ofile_mut();
        of.current = cur;
        of.current_x = 0;
        self.edit_redraw(was_current, Update::Centering);
    }

    pub fn do_prev_word(&mut self) {
        let punctuation_as_letters = self.isset(Flag::WordBounds);
        let mut seen_a_word = false;
        let mut step_forward = false;
        loop {
            if self.current_x() == 0 {
                match self.prev(self.current()) {
                    None => break,
                    Some(p) => {
                        let len = self.datalen(p);
                        let of = self.ofile_mut();
                        of.current = p;
                        of.current_x = len;
                    }
                }
            }
            let cur = self.current();
            let x = step_left(self.data(cur), self.current_x());
            self.ofile_mut().current_x = x;
            let d = &self.nodes[cur].data[x..];
            if self.is_word_char(d, punctuation_as_letters) {
                seen_a_word = true;
                if x == 0 {
                    break;
                }
            } else if is_zerowidth(d) {
            } else if seen_a_word {
                step_forward = true;
                break;
            }
        }
        if step_forward {
            let cur = self.current();
            let x = step_right(self.data(cur), self.current_x());
            self.ofile_mut().current_x = x;
        }
    }

    pub fn do_next_word(&mut self, after_ends: bool) -> bool {
        let punctuation_as_letters = self.isset(Flag::WordBounds);
        let cur = self.current();
        let started_on_word = self.is_word_char(
            &self.nodes[cur].data[self.current_x()..],
            punctuation_as_letters,
        );
        let mut seen_space = !started_on_word;
        let mut seen_word = started_on_word;
        loop {
            let cur = self.current();
            let x = self.current_x();
            if x >= self.datalen(cur) {
                match self.next(cur) {
                    None => break,
                    Some(n) => {
                        let of = self.ofile_mut();
                        of.current = n;
                        of.current_x = 0;
                        seen_space = true;
                    }
                }
            } else {
                let nx = step_right(self.data(cur), x);
                self.ofile_mut().current_x = nx;
            }
            let cur = self.current();
            let x = self.current_x();
            let d = &self.nodes[cur].data[x.min(self.nodes[cur].data.len())..];
            if after_ends {
                if self.is_word_char(d, punctuation_as_letters) {
                    seen_word = true;
                } else if is_zerowidth(d) {
                } else if seen_word {
                    break;
                }
            } else if is_zerowidth(d) {
            } else if !self.is_word_char(d, punctuation_as_letters) {
                seen_space = true;
            } else if seen_space {
                break;
            }
        }
        started_on_word
    }

    pub fn to_prev_word(&mut self) {
        let was_current = self.current();
        self.do_prev_word();
        self.edit_redraw(was_current, Update::Flowing);
    }

    pub fn to_next_word(&mut self) {
        let was_current = self.current();
        let ae = self.isset(Flag::AfterEnds);
        self.do_next_word(ae);
        self.edit_redraw(was_current, Update::Flowing);
    }

    pub fn do_home(&mut self) {
        let was_current = self.current();
        let was_column = self.xplustabs();
        let mut moved_off_chunk = true;
        let mut moved = false;
        let mut leftedge = 0usize;
        let mut left_x = 0usize;
        let cur = was_current;
        if self.isset(Flag::SoftWrap) {
            leftedge = self.leftedge_for(was_column, cur);
            let le = leftedge;
            left_x = self.proper_x(cur, &mut leftedge, false, le, None);
        }
        if self.isset(Flag::SmartHome) {
            let indent_x = indent_length(self.data(cur));
            if indent_x < self.datalen(cur) {
                if self.current_x() == indent_x {
                    self.ofile_mut().current_x = 0;
                    moved = true;
                } else if left_x <= indent_x {
                    self.ofile_mut().current_x = indent_x;
                    moved = true;
                }
            }
        }
        if !moved && self.isset(Flag::SoftWrap) {
            if self.current_x() == left_x {
                self.ofile_mut().current_x = 0;
            } else {
                let of = self.ofile_mut();
                of.current_x = left_x;
                of.placewewant = leftedge;
                moved_off_chunk = false;
            }
        } else if !moved {
            self.ofile_mut().current_x = 0;
        }
        if moved_off_chunk {
            let pww = self.xplustabs();
            self.ofile_mut().placewewant = pww;
        }
        if self.isset(Flag::SoftWrap) && moved_off_chunk {
            self.edit_redraw(was_current, Update::Flowing);
        } else if self.line_needs_update(was_column, self.ofile().placewewant) {
            let (c, x) = (self.current(), self.current_x());
            self.update_line(c, x);
        }
    }

    pub fn do_end(&mut self) {
        let was_current = self.current();
        let was_column = self.xplustabs();
        let line_len = self.datalen(was_current);
        let mut moved_off_chunk = true;
        if self.isset(Flag::SoftWrap) {
            let mut kickoff = true;
            let mut last_chunk = false;
            let leftedge = self.leftedge_for(was_column, was_current);
            let data = self.nodes[was_current].data.clone();
            let mut rightedge =
                self.get_softwrap_breakpoint(&data, leftedge, &mut kickoff, &mut last_chunk);
            if !last_chunk {
                rightedge -= 1;
            }
            let right_x = self.actual_x(&data, rightedge);
            if self.current_x() == right_x {
                self.ofile_mut().current_x = line_len;
            } else {
                let of = self.ofile_mut();
                of.current_x = right_x;
                of.placewewant = rightedge;
                moved_off_chunk = false;
            }
        } else {
            self.ofile_mut().current_x = line_len;
        }
        if moved_off_chunk {
            let pww = self.xplustabs();
            self.ofile_mut().placewewant = pww;
        }
        if self.isset(Flag::SoftWrap) && moved_off_chunk {
            self.edit_redraw(was_current, Update::Flowing);
        } else if self.line_needs_update(was_column, self.ofile().placewewant) {
            let (c, x) = (self.current(), self.current_x());
            self.update_line(c, x);
        }
    }

    pub fn do_up(&mut self) {
        let was_current = self.current();
        let (mut leftedge, target_column) = self.get_edge_and_target();
        let mut line = was_current;
        if self.go_back_chunks(1, &mut line, &mut leftedge) > 0 {
            return;
        }
        self.ofile_mut().current = line;
        self.set_proper_index_and_pww(&mut leftedge, target_column, false);
        if self.ofile().cursor_row == 0
            && !self.isset(Flag::JumpyScrolling)
            && ((self.tabsize.cast_unsigned()) < self.editwincols || !self.isset(Flag::SoftWrap))
        {
            self.edit_scroll(BACKWARD);
        } else {
            self.edit_redraw(was_current, Update::Flowing);
        }
        self.ofile_mut().placewewant = leftedge + target_column;
    }

    pub fn do_down(&mut self) {
        let was_current = self.current();
        let (mut leftedge, target_column) = self.get_edge_and_target();
        let mut line = was_current;
        if self.go_forward_chunks(1, &mut line, &mut leftedge) > 0 {
            return;
        }
        self.ofile_mut().current = line;
        self.set_proper_index_and_pww(&mut leftedge, target_column, true);
        if self.ofile().cursor_row == i64_of(self.editwinrows) - 1
            && !self.isset(Flag::JumpyScrolling)
            && ((self.tabsize.cast_unsigned()) < self.editwincols || !self.isset(Flag::SoftWrap))
        {
            self.edit_scroll(FORWARD);
        } else {
            self.edit_redraw(was_current, Update::Flowing);
        }
        self.ofile_mut().placewewant = leftedge + target_column;
    }

    pub fn do_scroll_up(&mut self) {
        if self.prev(self.edittop()).is_none() && self.ofile().firstcolumn == 0 {
            return;
        }
        if self.ofile().cursor_row == i64_of(self.editwinrows) - 1 {
            self.do_up();
        }
        if self.editwinrows > 1 {
            self.edit_scroll(BACKWARD);
        }
    }

    pub fn do_scroll_down(&mut self) {
        if self.ofile().cursor_row == 0 {
            self.do_down();
        }
        let top = self.edittop();
        if self.editwinrows > 1
            && (self.next(top).is_some()
                || (self.isset(Flag::SoftWrap)
                    && self.extra_chunks_in(top) > self.chunk_for(self.ofile().firstcolumn, top)))
        {
            self.edit_scroll(FORWARD);
        }
    }

    pub fn do_left(&mut self) {
        let was_current = self.current();
        let cur = was_current;
        if self.current_x() > 0 {
            let data = &self.nodes[cur].data;
            let mut x = step_left(data, self.ofile().current_x);
            while x > 0 && is_zerowidth(&data[x..]) {
                x = step_left(data, x);
            }
            self.ofile_mut().current_x = x;
        } else if cur != self.filetop() {
            let p = self.prev(cur).unwrap();
            let len = self.datalen(p);
            let of = self.ofile_mut();
            of.current = p;
            of.current_x = len;
        }
        self.edit_redraw(was_current, Update::Flowing);
    }

    pub fn do_right(&mut self) {
        let was_current = self.current();
        let cur = was_current;
        let x = self.current_x();
        if x < self.datalen(cur) {
            let data = &self.nodes[cur].data;
            let mut nx = step_right(data, x);
            while nx < data.len() && is_zerowidth(&data[nx..]) {
                nx = step_right(data, nx);
            }
            self.ofile_mut().current_x = nx;
        } else if cur != self.filebot() {
            let n = self.next(cur).unwrap();
            let of = self.ofile_mut();
            of.current = n;
            of.current_x = 0;
        }
        self.edit_redraw(was_current, Update::Flowing);
    }

    pub fn do_scroll_left(&mut self) {
        if self.isset(Flag::SoftWrap) || self.isset(Flag::SoloSidescroll) {
            let opt = if self.isset(Flag::SoftWrap) {
                "--softwrap"
            } else {
                "--solo"
            };
            self.statusline(Msg::Ahem, &fmt!("Nicht möglich mit „%s“", opt));
            return;
        }
        let ts = self.tabsize.cast_unsigned();
        let brink = self.ofile().brink;
        let step = if brink < ts {
            brink
        } else if ts < 2 {
            2
        } else {
            ts
        };
        self.ofile_mut().brink -= step;
        let brink = self.ofile().brink;
        let cur = self.current();
        let frame_x = self.actual_x(self.data(cur), brink + self.editwincols - CUSHION - 1);
        if self.current_x() > frame_x {
            self.ofile_mut().current_x = frame_x;
            let pww = self.xplustabs();
            self.ofile_mut().placewewant = pww;
        }
        self.refresh_needed = true;
    }

    pub fn do_scroll_right(&mut self) {
        if self.isset(Flag::SoftWrap) || self.isset(Flag::SoloSidescroll) {
            let opt = if self.isset(Flag::SoftWrap) {
                "--softwrap"
            } else {
                "--solo"
            };
            self.statusline(Msg::Ahem, &fmt!("Nicht möglich mit „%s“", opt));
            return;
        }
        let sill = self.lineno(self.edittop()) + i64_of(self.editwinrows);
        let mut line = self.current();
        let ts = self.tabsize.cast_unsigned();
        self.ofile_mut().brink += if ts < 2 { 2 } else { ts };
        let brink = self.ofile().brink;
        while line != self.edittop() && self.breadth(self.data(line)) < brink + CUSHION {
            line = self.prev(line).unwrap();
        }
        while self.lineno(line) < sill
            && self.breadth(self.data(line)) < brink + CUSHION
            && self.next(line).is_some()
        {
            line = self.next(line).unwrap();
        }
        if self.lineno(line) < sill && self.breadth(self.data(line)) >= brink + CUSHION {
            self.ofile_mut().current = line;
        }
        let cur = self.current();
        let frame_x = self.actual_x(self.data(cur), brink + CUSHION);
        if self.current_x() < frame_x {
            self.ofile_mut().current_x = frame_x;
            let pww = self.xplustabs();
            self.ofile_mut().placewewant = pww;
        }
        self.refresh_needed = true;
    }
}
