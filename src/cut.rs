use crate::chars::{char_length, is_zerowidth, step_left};
use crate::definitions::{BACKWARD, CUSHION, FORWARD, Flag, Lp, Msg, UndoType, Update};
use crate::global::Zu;
use crate::utils::usize_of;

impl Zu {
    pub fn expunge(&mut self, action: UndoType) {
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
        let cur = self.current();
        let x = self.current_x();
        if x < self.datalen(cur) {
            let charlen = char_length(&self.nodes[cur].data[x..]);
            let old_amount = if self.isset(Flag::SoftWrap) {
                self.extra_chunks_in(cur)
            } else {
                0
            };
            let head_lineno = self.current_undo().map(|u| u.head_lineno);
            if action != self.ofile().last_action || Some(self.lineno(cur)) != head_lineno {
                self.add_undo(action, None);
            } else {
                self.update_undo(action);
            }
            {
                let d = &mut self.nodes[cur].data;
                let end = (x + charlen).min(d.len());
                d.drain(x..end);
            }
            if (self.isset(Flag::SoftWrap) && self.extra_chunks_in(cur) != old_amount)
                || (self.united_sidescroll
                    && self.ofile().placewewant < self.ofile().brink + CUSHION)
            {
                self.refresh_needed = true;
            }
            let of = self.ofile_mut();
            if of.mark == Some(cur) && of.mark_x > of.current_x {
                of.mark_x -= charlen;
            }
        } else if cur != self.filebot() {
            let joining = self.next(cur).unwrap();
            if joining == self.filebot() && x != 0 && !self.isset(Flag::NoNewlines) {
                if action == UndoType::Back {
                    self.add_undo(UndoType::Back, None);
                }
                return;
            }
            self.add_undo(action, None);
            {
                let of = self.ofile_mut();
                if of.mark == Some(joining) {
                    of.mark = Some(cur);
                    of.mark_x += of.current_x;
                }
            }
            let anchor = self.nodes[joining].has_anchor;
            self.nodes[cur].has_anchor |= anchor;
            let jdata = std::mem::take(&mut self.nodes[joining].data);
            self.nodes[cur].data.extend_from_slice(&jdata);
            drop(jdata);
            self.unlink_node(joining);
            self.renumber_from(cur);
            self.refresh_needed = true;
        } else {
            return;
        }
        if !self.refresh_needed {
            let (c, x) = (self.current(), self.current_x());
            self.update_line(c, x);
        }
        self.ofile_mut().totsize = self.ofile().totsize.wrapping_sub(1);
        let tot = self.ofile().totsize;
        if let Some(u) = self.current_undo_mut() {
            u.newsize = tot;
        }
        self.set_modified();
    }

    pub fn do_delete(&mut self) {
        if self.ofile().mark.is_some() && self.isset(Flag::LetThemZap) {
            self.zap_text();
        } else {
            self.expunge(UndoType::Del);
            loop {
                let cur = self.current();
                let x = self.current_x();
                if x < self.datalen(cur) && is_zerowidth(&self.nodes[cur].data[x..]) {
                    self.expunge(UndoType::Del);
                } else {
                    break;
                }
            }
        }
    }

    pub fn do_backspace(&mut self) {
        if self.ofile().mark.is_some() && self.isset(Flag::LetThemZap) {
            self.zap_text();
        } else if self.current_x() > 0 {
            let cur = self.current();
            let x = step_left(self.data(cur), self.current_x());
            self.ofile_mut().current_x = x;
            self.expunge(UndoType::Back);
        } else if self.current() != self.filetop() {
            self.do_left();
            self.expunge(UndoType::Back);
        }
    }

    pub fn is_cuttable(&mut self, test_cliff: bool) -> bool {
        let from = if test_cliff { self.current_x() } else { 0 };
        let cur = self.current();
        let of = self.ofile();
        let at_end = from >= self.datalen(cur);
        if (self.next(cur).is_none() && at_end && of.mark.is_none())
            || (of.mark == Some(cur) && of.mark_x == of.current_x)
            || (from > 0
                && !self.isset(Flag::NoNewlines)
                && at_end
                && self.next(cur) == Some(of.filebot))
        {
            self.statusbar("Nichts ausgeschnitten");
            self.ofile_mut().mark = None;
            false
        } else {
            true
        }
    }

    pub fn chop_word(&mut self, forward: bool) {
        let was_current = self.current();
        let was_x = self.current_x();
        let is_cutbuffer = self.cutbuffer;
        self.cutbuffer = None;
        if forward {
            let ae = self.isset(Flag::AfterEnds);
            self.do_next_word(ae);
            if self.current() != was_current && was_x < self.datalen(was_current) {
                let len = self.datalen(was_current);
                let of = self.ofile_mut();
                of.current = was_current;
                of.current_x = len;
            }
        } else {
            self.do_prev_word();
            if self.current() != was_current {
                if was_x > 0 {
                    let of = self.ofile_mut();
                    of.current = was_current;
                    of.current_x = 0;
                } else {
                    let len = self.datalen(self.current());
                    self.ofile_mut().current_x = len;
                }
            }
        }
        {
            let of = self.ofile_mut();
            of.mark = Some(of.current);
            of.mark_x = of.current_x;
            of.current = was_current;
            of.current_x = was_x;
        }
        self.add_undo(UndoType::Cut, None);
        self.do_snip(true, false, false);
        self.update_undo(UndoType::Cut);
        let cb = self.cutbuffer;
        self.free_lines(cb);
        self.cutbuffer = is_cutbuffer;
    }

    pub fn chop_previous_word(&mut self) {
        if self.prev(self.current()).is_none() && self.current_x() == 0 {
            self.statusbar("Nichts ausgeschnitten");
        } else {
            self.chop_word(BACKWARD);
        }
    }

    pub fn chop_next_word(&mut self) {
        self.ofile_mut().mark = None;
        if self.is_cuttable(true) {
            self.chop_word(FORWARD);
        }
    }

    pub fn extract_segment(&mut self, top: Lp, top_x: usize, bot: Lp, bot_x: usize) {
        let edittop = self.edittop();
        let edittop_inside =
            self.lineno(edittop) >= self.lineno(top) && self.lineno(edittop) <= self.lineno(bot);
        let mark = self.ofile().mark;
        let same_line = mark == Some(top);
        let post_marked = mark.is_some_and(|m| {
            self.lineno(m) > self.lineno(top) || (same_line && self.ofile().mark_x > top_x)
        });
        let mut had_anchor = self.nodes[top].has_anchor;
        if top == bot && top_x == bot_x {
            return;
        }
        if top != bot {
            let mut line = self.next(top);
            let stop = self.next(bot);
            while line != stop {
                let l = line.unwrap();
                had_anchor |= self.nodes[l].has_anchor;
                line = self.next(l);
            }
        }
        let taken;
        let last;
        if top == bot {
            taken = self.make_new_node(None);
            let piece: Vec<u8> = self.nodes[top].data[top_x..bot_x].to_vec();
            self.nodes[taken].data = piece;
            self.nodes[top].data.drain(top_x..bot_x);
            last = taken;
        } else if top_x == 0 && bot_x == 0 {
            taken = top;
            last = self.make_new_node(None);
            self.nodes[last].data = Vec::new();
            self.nodes[last].has_anchor = self.nodes[bot].has_anchor;
            let bot_prev = self.prev(bot).unwrap();
            self.nodes[last].prev = Some(bot_prev);
            self.nodes[bot_prev].next = Some(last);
            self.nodes[last].next = None;
            let top_prev = self.prev(top);
            self.nodes[bot].prev = top_prev;
            match top_prev {
                Some(p) => self.nodes[p].next = Some(bot),
                None => self.ofile_mut().filetop = bot,
            }
            self.ofile_mut().current = bot;
        } else {
            taken = self.make_new_node(None);
            let tail_of_top: Vec<u8> = self.nodes[top].data[top_x..].to_vec();
            self.nodes[taken].data = tail_of_top;
            let top_next = self.next(top).unwrap();
            self.nodes[taken].next = Some(top_next);
            self.nodes[top_next].prev = Some(taken);
            let bot_next = self.next(bot);
            self.nodes[top].next = bot_next;
            if let Some(bn) = bot_next {
                self.nodes[bn].prev = Some(top);
            }
            let rest_of_bot: Vec<u8> = self.nodes[bot].data[bot_x..].to_vec();
            self.nodes[top].data.truncate(top_x);
            self.nodes[top].data.extend_from_slice(&rest_of_bot);
            last = bot;
            self.nodes[last].data.truncate(bot_x);
            self.nodes[last].next = None;
            self.ofile_mut().current = top;
        }
        let n = self.number_of_characters_in(taken, last);
        self.ofile_mut().totsize = self.ofile().totsize.wrapping_sub(n);
        if self.cutbuffer.is_none() {
            self.cutbuffer = Some(taken);
            self.cutbottom = Some(last);
            self.inherited_anchor = self.nodes[taken].has_anchor;
        } else {
            let cb = self.cutbottom.unwrap();
            let taken_data = std::mem::take(&mut self.nodes[taken].data);
            self.nodes[cb].data.extend_from_slice(&taken_data);
            drop(taken_data);
            let ta = self.nodes[taken].has_anchor;
            self.nodes[cb].has_anchor = ta && !self.inherited_anchor;
            self.inherited_anchor |= ta;
            let tn = self.next(taken);
            self.nodes[cb].next = tn;
            self.delete_node(taken);
            if let Some(tn) = tn {
                self.nodes[tn].prev = Some(cb);
                self.cutbottom = Some(last);
            }
        }
        self.ofile_mut().current_x = top_x;
        let cur = self.current();
        self.nodes[cur].has_anchor = had_anchor;
        if post_marked || same_line {
            self.ofile_mut().mark = Some(cur);
        }
        if post_marked {
            self.ofile_mut().mark_x = top_x;
        }
        if self.filebot() == bot {
            self.ofile_mut().filebot = cur;
        }
        self.renumber_from(cur);
        if edittop_inside {
            self.adjust_viewport(Update::Stationary);
            self.refresh_needed = true;
        }
        if !self.isset(Flag::NoNewlines) && !self.data(self.filebot()).is_empty() {
            self.new_magicline();
        }
    }

    pub fn ingraft_buffer(&mut self, topline: Lp) {
        let line = self.current();
        let mut length = self.datalen(line);
        let mut extralen = self.datalen(topline);
        let xpos = self.current_x();
        let tailtext: Vec<u8> = self.nodes[line].data[xpos..].to_vec();
        let mark_follows = self.ofile().mark == Some(line) && !self.mark_is_before_cursor();
        let botline = self.last_of(topline);
        let n = self.number_of_characters_in(topline, botline);
        self.ofile_mut().totsize = self.ofile().totsize.wrapping_add(n);
        if topline != botline {
            length = xpos;
        }
        if extralen > 0 {
            let top_data = self.nodes[topline].data.clone();
            let d = &mut self.nodes[line].data;
            let tail: Vec<u8> = d.split_off(xpos);
            d.extend_from_slice(&top_data);
            d.extend_from_slice(&tail);
        }
        if topline == botline {
            self.ofile_mut().current_x += extralen;
        } else {
            if self.next(line).is_none() {
                self.ofile_mut().filebot = botline;
            }
            self.nodes[line].data.truncate(xpos + extralen);
            let line_next = self.next(line);
            self.nodes[botline].next = line_next;
            if let Some(ln) = line_next {
                self.nodes[ln].prev = Some(botline);
            }
            let top_next = self.next(topline).unwrap();
            self.nodes[line].next = Some(top_next);
            self.nodes[top_next].prev = Some(line);
            length = self.datalen(botline);
            extralen = tailtext.len();
            self.nodes[botline].data.extend_from_slice(&tailtext);
            let of = self.ofile_mut();
            of.current = botline;
            of.current_x = length;
        }
        if mark_follows && topline != botline {
            let of = self.ofile_mut();
            of.mark = Some(botline);
            of.mark_x = of.mark_x.wrapping_add(length.wrapping_sub(xpos));
        } else if mark_follows {
            self.ofile_mut().mark_x += extralen;
        }
        self.delete_node(topline);
        self.renumber_from(line);
        if !self.isset(Flag::NoNewlines) && !self.data(self.filebot()).is_empty() {
            self.new_magicline();
        }
    }

    pub fn copy_from_buffer(&mut self, somebuffer: Lp) {
        let the_copy = self.copy_buffer(somebuffer);
        self.ingraft_buffer(the_copy);
    }

    pub fn cut_marked_region(&mut self) {
        let (top, top_x, bot, bot_x) = self.get_region();
        self.extract_segment(top, top_x, bot, bot_x);
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
    }

    pub fn do_snip(&mut self, marked: bool, until_eof: bool, append: bool) {
        let line = self.current();
        self.keep_cutbuffer &= self.ofile().last_action != UndoType::Copy;
        if (marked || until_eof || !self.keep_cutbuffer) && !append {
            let cb = self.cutbuffer;
            self.free_lines(cb);
            self.cutbuffer = None;
        }
        if until_eof {
            let bot = self.filebot();
            let x = self.current_x();
            let len = self.datalen(bot);
            self.extract_segment(line, x, bot, len);
        } else if self.ofile().mark.is_some() {
            self.cut_marked_region();
            self.ofile_mut().mark = None;
        } else if self.isset(Flag::CutFromCursor) {
            let x = self.current_x();
            if x < self.datalen(line) {
                let len = self.datalen(line);
                self.extract_segment(line, x, line, len);
            } else if line != self.filebot() {
                let n = self.next(line).unwrap();
                self.extract_segment(line, x, n, 0);
                let pww = self.xplustabs();
                self.ofile_mut().placewewant = pww;
            }
        } else {
            if line == self.filebot() {
                let len = self.datalen(line);
                self.extract_segment(line, 0, line, len);
            } else {
                let n = self.next(line).unwrap();
                self.extract_segment(line, 0, n, 0);
            }
            self.ofile_mut().placewewant = 0;
        }
        self.keep_cutbuffer = !marked && !until_eof;
        self.set_modified();
        self.refresh_needed = true;
    }

    pub fn cut_text(&mut self) {
        let test = self.isset(Flag::CutFromCursor) && self.ofile().mark.is_none();
        if !self.is_cuttable(test) {
            return;
        }
        if self.ofile().last_action != UndoType::Cut || !self.keep_cutbuffer {
            self.keep_cutbuffer = false;
            self.add_undo(UndoType::Cut, None);
        }
        let marked = self.ofile().mark.is_some();
        self.do_snip(marked, false, false);
        self.update_undo(UndoType::Cut);
        self.wipe_statusbar();
    }

    pub fn cut_till_eof(&mut self) {
        self.ran_a_tool = true;
        let cur = self.current();
        let x = self.current_x();
        if x >= self.datalen(cur)
            && (self.next(cur).is_none()
                || (!self.isset(Flag::NoNewlines)
                    && x > 0
                    && self.next(cur) == Some(self.filebot())))
        {
            self.statusbar("Nichts ausgeschnitten");
            return;
        }
        self.add_undo(UndoType::CutToEof, None);
        self.do_snip(false, true, false);
        self.update_undo(UndoType::CutToEof);
        self.wipe_statusbar();
    }

    pub fn zap_text(&mut self) {
        let was_cutbuffer = self.cutbuffer;
        let test = self.isset(Flag::CutFromCursor) && self.ofile().mark.is_none();
        if !self.is_cuttable(test) {
            return;
        }
        if self.ofile().last_action != UndoType::Zap || !self.keep_cutbuffer {
            self.add_undo(UndoType::Zap, None);
        }
        self.cutbuffer = self.current_undo().and_then(|u| u.cutbuffer);
        if let Some(cb) = self.cutbuffer {
            self.cutbottom = Some(self.last_of(cb));
        }
        let marked = self.ofile().mark.is_some();
        self.do_snip(marked, false, true);
        self.update_undo(UndoType::Zap);
        self.wipe_statusbar();
        self.cutbuffer = was_cutbuffer;
    }

    pub fn copy_marked_region(&mut self) {
        let (topline, top_x, botline, bot_x) = self.get_region();
        self.ofile_mut().last_action = UndoType::Other;
        self.keep_cutbuffer = false;
        self.ofile_mut().mark = None;
        self.refresh_needed = true;
        if topline == botline && top_x == bot_x {
            self.statusbar("Nichts kopiert");
            return;
        }
        let head = self.make_new_node(None);
        let mut item = head;
        let mut line = topline;
        loop {
            let d = &self.nodes[line].data;
            let from = if line == topline { top_x } else { 0 };
            let to = if line == botline { bot_x } else { d.len() };
            let piece = d[from..to].to_vec();
            self.nodes[item].data = piece;
            self.nodes[item].lineno = self.nodes[line].lineno;
            self.nodes[item].has_anchor = self.nodes[line].has_anchor;
            if line == botline {
                break;
            }
            let n = self.make_new_node(Some(item));
            self.nodes[item].next = Some(n);
            item = n;
            line = self.next(line).unwrap();
        }
        self.cutbuffer = Some(head);
        self.cutbottom = Some(item);
    }

    pub fn copy_text(&mut self) {
        let cur = self.current();
        let at_eol = self.current_x() >= self.datalen(cur);
        let mut sans_newline = self.isset(Flag::NoNewlines) && self.next(cur).is_none();
        let from_x = if self.isset(Flag::CutFromCursor) {
            self.current_x()
        } else {
            0
        };
        let was_current = cur;
        if self.ofile().mark.is_some() || self.ofile().last_action != UndoType::Copy {
            self.keep_cutbuffer = false;
        }
        if !self.keep_cutbuffer {
            let cb = self.cutbuffer;
            self.free_lines(cb);
            self.cutbuffer = None;
        }
        self.wipe_statusbar();
        if self.ofile().mark.is_some() {
            self.copy_marked_region();
            return;
        }
        if self.next(cur).is_none()
            && at_eol
            && (self.isset(Flag::CutFromCursor)
                || self.current_x() == 0
                || self.cutbuffer.is_some())
        {
            self.statusbar("Nichts kopiert");
            return;
        }
        let addition = self.make_new_node(None);
        let piece: Vec<u8> = self.nodes[cur].data[from_x..].to_vec();
        self.nodes[addition].data = piece;
        if self.isset(Flag::CutFromCursor) {
            sans_newline = !at_eol;
        }
        if self.cutbuffer.is_none() && sans_newline {
            self.cutbuffer = Some(addition);
            self.cutbottom = Some(addition);
        } else if self.cutbuffer.is_none() {
            self.cutbuffer = Some(addition);
            let cb = self.make_new_node(Some(addition));
            self.nodes[cb].data = Vec::new();
            self.nodes[addition].next = Some(cb);
            self.cutbottom = Some(cb);
        } else if sans_newline {
            let cb = self.cutbottom.unwrap();
            let p = self.prev(cb).unwrap();
            self.nodes[addition].prev = Some(p);
            self.nodes[p].next = Some(addition);
            self.delete_node(cb);
            self.cutbottom = Some(addition);
        } else if self.isset(Flag::CutFromCursor) {
            let cb = self.cutbottom.unwrap();
            self.nodes[addition].prev = Some(cb);
            self.nodes[cb].next = Some(addition);
            self.cutbottom = Some(addition);
        } else {
            let cb = self.cutbottom.unwrap();
            let p = self.prev(cb).unwrap();
            self.nodes[addition].prev = Some(p);
            self.nodes[p].next = Some(addition);
            self.nodes[addition].next = Some(cb);
            self.nodes[cb].prev = Some(addition);
        }
        if (!self.isset(Flag::CutFromCursor) || at_eol) && self.next(cur).is_some() {
            let n = self.next(cur).unwrap();
            let of = self.ofile_mut();
            of.current = n;
            of.current_x = 0;
        } else {
            let len = self.datalen(cur);
            self.ofile_mut().current_x = len;
        }
        self.edit_redraw(was_current, Update::Flowing);
        self.ofile_mut().last_action = UndoType::Copy;
        self.keep_cutbuffer = true;
    }

    pub fn paste_text(&mut self) {
        let was_current = self.current();
        let had_anchor = self.nodes[was_current].has_anchor;
        let was_lineno = self.lineno(was_current);
        let mut was_leftedge = 0;
        let Some(cb) = self.cutbuffer else {
            self.statusline(Msg::Ahem, "Zwischenablage ist leer");
            return;
        };
        self.add_undo(UndoType::Paste, None);
        if self.isset(Flag::SoftWrap) {
            was_leftedge = self.leftedge_for(self.xplustabs(), was_current);
        }
        self.copy_from_buffer(cb);
        let mut line = Some(was_current);
        let stop = self.next(self.current());
        while line != stop {
            let l = line.unwrap();
            self.nodes[l].has_anchor = false;
            line = self.next(l);
        }
        self.nodes[was_current].has_anchor = had_anchor;
        self.update_undo(UndoType::Paste);
        if self.current() == was_current && self.isset(Flag::BreakLongLines) {
            self.do_wrap();
        }
        if self.less_than_a_screenful(usize_of(was_lineno), was_leftedge) {
            self.focusing = false;
        }
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
        self.set_modified();
        self.wipe_statusbar();
        self.refresh_needed = true;
    }
}
