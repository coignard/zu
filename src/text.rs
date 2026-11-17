use crate::chars::{
    char_length, is_blank_char, mbstrchr, mbstrlen, step_left, step_right, white_string,
};
use crate::definitions::{
    CURSOR_WAS_AT_HEAD, FORWARD, Flag, GENERAL_COMMENT_CHARACTER, Group, HAD_ANCHOR_AT_START,
    INCLUDED_LAST_LINE, Lp, MARK_WAS_SET, MMAIN, Msg, Undo, UndoType, Update, WAS_BACKSPACE_AT_EOF,
    WAS_WHOLE_LINE,
};
use crate::global::Zu;
use crate::utils::{i64_of, usize_of};

impl Zu {
    pub fn do_mark(&mut self) {
        if self.ofile().mark.is_none() {
            let of = self.ofile_mut();
            of.mark = Some(of.current);
            of.mark_x = of.current_x;
            of.softmark = false;
            self.statusbar("Markierung gesetzt");
        } else {
            self.ofile_mut().mark = None;
            self.statusbar("Markierung entfernt");
            self.refresh_needed = true;
        }
    }

    pub fn do_tab(&mut self) {
        let of = self.ofile();
        if of.mark.is_some() && of.mark != Some(of.current) {
            self.do_indent();
        } else if self.isset(Flag::TabsToSpaces) {
            let ts = self.tabsize.cast_unsigned();
            let length = ts - (self.xplustabs() % ts);
            let mut spaces = vec![b' '; length];
            self.inject(&mut spaces);
        } else {
            let mut t = vec![b'\t'];
            self.inject(&mut t);
        }
    }

    pub fn indent_a_line(&mut self, line: Lp, indentation: &[u8]) {
        let indent_len = indentation.len();
        if indent_len == 0 {
            return;
        }
        {
            let d = &mut self.nodes[line].data;
            let mut nd = Vec::with_capacity(d.len() + indent_len);
            nd.extend_from_slice(indentation);
            nd.extend_from_slice(d);
            *d = nd;
        }
        self.ofile_mut().totsize = self.ofile().totsize.wrapping_add(indent_len);
        let of = self.ofile_mut();
        if of.mark == Some(line) && of.mark_x > 0 {
            of.mark_x += indent_len;
        }
        if of.current == line && of.current_x > 0 {
            of.current_x += indent_len;
            let pww = self.xplustabs();
            self.ofile_mut().placewewant = pww;
        }
    }

    pub fn do_indent(&mut self) {
        let (mut top, bot) = self.get_range();
        let stop = self.next(bot);
        while Some(top) != stop && self.datalen(top) == 0 {
            match self.next(top) {
                Some(following) => top = following,
                None => return,
            }
        }
        if Some(top) == stop {
            return;
        }
        let indentation: Vec<u8> = if self.isset(Flag::TabsToSpaces) {
            vec![b' '; self.tabsize.cast_unsigned()]
        } else {
            vec![b'\t']
        };
        self.add_undo(UndoType::Indent, None);
        let mut line = Some(top);
        while line != stop {
            let l = line.unwrap();
            let real_indent: Vec<u8> = if self.datalen(l) == 0 {
                Vec::new()
            } else {
                indentation.clone()
            };
            self.indent_a_line(l, &real_indent);
            let ln = self.lineno(l);
            self.update_multiline_undo(ln, &real_indent);
            line = self.next(l);
        }
        self.set_modified();
        self.ensure_firstcolumn_is_aligned();
        self.refresh_needed = true;
        self.shift_held = true;
    }

    pub fn length_of_white(&self, text: &[u8]) -> usize {
        let mut white_count = 0;
        let ts = self.tabsize.cast_unsigned();
        let mut i = 0;
        loop {
            let c = text.get(i).copied().unwrap_or(0);
            if c == b'\t' {
                return white_count + 1;
            }
            if c != b' ' {
                return white_count;
            }
            white_count += 1;
            if white_count == ts {
                return ts;
            }
            i += 1;
        }
    }

    pub fn compensate_leftward(&mut self, line: Lp, leftshift: usize) {
        let of = self.ofile_mut();
        if of.mark == Some(line) {
            of.mark_x = of.mark_x.saturating_sub(leftshift);
        }
        if of.current == line {
            of.current_x = of.current_x.saturating_sub(leftshift);
            let pww = self.xplustabs();
            self.ofile_mut().placewewant = pww;
        }
    }

    pub fn unindent_a_line(&mut self, line: Lp, indent_len: usize) {
        if indent_len == 0 {
            return;
        }
        self.nodes[line].data.drain(..indent_len);
        self.ofile_mut().totsize = self.ofile().totsize.wrapping_sub(indent_len);
        self.compensate_leftward(line, indent_len);
    }

    pub fn do_unindent(&mut self) {
        let (mut top, bot) = self.get_range();
        let stop = self.next(bot);
        while Some(top) != stop && self.length_of_white(self.data(top)) == 0 {
            match self.next(top) {
                Some(following) => top = following,
                None => return,
            }
        }
        if Some(top) == stop {
            return;
        }
        self.add_undo(UndoType::Unindent, None);
        let mut line = Some(top);
        while line != stop {
            let l = line.unwrap();
            let indent_len = self.length_of_white(self.data(l));
            let indentation: Vec<u8> = self.nodes[l].data[..indent_len].to_vec();
            self.unindent_a_line(l, indent_len);
            let ln = self.lineno(l);
            self.update_multiline_undo(ln, &indentation);
            line = self.next(l);
        }
        self.set_modified();
        self.ensure_firstcolumn_is_aligned();
        self.refresh_needed = true;
        self.shift_held = true;
    }

    fn handle_indent_action(&mut self, ui: usize, undoing: bool, add_indent: bool) {
        let (head_lineno, head_x, group) = {
            let u = &self.ofile().undo[ui];
            (u.head_lineno, u.head_x, u.grouping.last().cloned())
        };
        let Some(group) = group else { return };
        let mut line = Some(self.line_from_number(group.top_line));
        if !undoing {
            self.goto_line_posx(head_lineno, head_x);
        }
        while let Some(l) = line {
            if self.lineno(l) > group.bottom_line {
                break;
            }
            let idx = usize_of(self.lineno(l) - group.top_line);
            let blanks = group.indentations.get(idx).cloned().unwrap_or_default();
            if undoing ^ add_indent {
                self.indent_a_line(l, &blanks);
            } else {
                self.unindent_a_line(l, blanks.len());
            }
            line = self.next(l);
        }
        if undoing {
            self.goto_line_posx(head_lineno, head_x);
        }
        self.refresh_needed = true;
    }

    pub fn comment_line(&mut self, action: UndoType, line: Lp, comment_seq: &[u8]) -> bool {
        let comment_seq_len = comment_seq.len();
        let bar = comment_seq.iter().position(|&c| c == b'|');
        let pre_len = bar.unwrap_or(comment_seq_len);
        let post_seq: &[u8] = match bar {
            Some(b) => &comment_seq[b + 1..],
            None => &[],
        };
        let post_len = post_seq.len();
        let line_len = self.datalen(line);
        if !self.isset(Flag::NoNewlines) && line == self.filebot() {
            return false;
        }
        if action == UndoType::Comment {
            {
                let d = &mut self.nodes[line].data;
                let mut nd = Vec::with_capacity(line_len + pre_len + post_len);
                nd.extend_from_slice(&comment_seq[..pre_len]);
                nd.extend_from_slice(d);
                nd.extend_from_slice(post_seq);
                *d = nd;
            }
            self.ofile_mut().totsize = self.ofile().totsize.wrapping_add(pre_len + post_len);
            let of = self.ofile_mut();
            if of.mark == Some(line) && of.mark_x > 0 {
                of.mark_x += pre_len;
            }
            if of.current == line && of.current_x > 0 {
                of.current_x += pre_len;
                let pww = self.xplustabs();
                self.ofile_mut().placewewant = pww;
            }
            return true;
        }
        let d = &self.nodes[line].data;
        if d.len() >= pre_len + post_len
            && d.starts_with(&comment_seq[..pre_len])
            && (post_len == 0 || d.ends_with(post_seq))
        {
            if action == UndoType::Preflight {
                return true;
            }
            let d = &mut self.nodes[line].data;
            d.drain(..pre_len);
            let nl = d.len() - post_len;
            d.truncate(nl);
            self.ofile_mut().totsize = self.ofile().totsize.wrapping_sub(pre_len + post_len);
            self.compensate_leftward(line, pre_len);
            return true;
        }
        false
    }

    pub fn do_comment(&mut self) {
        let comment_seq: Vec<u8> = GENERAL_COMMENT_CHARACTER.to_vec();
        let mut action = UndoType::Uncomment;
        let (top, bot) = self.get_range();
        let mut all_empty = true;
        if top == bot && bot == self.filebot() && !self.isset(Flag::NoNewlines) {
            self.statusline(
                Msg::Ahem,
                "Nach dem Ende der Datei kann nicht kommentiert werden",
            );
            return;
        }
        let stop = self.next(bot);
        let mut line = Some(top);
        while line != stop {
            let l = line.unwrap();
            let empty = white_string(self.data(l));
            if !empty && !self.comment_line(UndoType::Preflight, l, &comment_seq) {
                action = UndoType::Comment;
                break;
            }
            all_empty = all_empty && empty;
            line = self.next(l);
        }
        if all_empty {
            action = UndoType::Comment;
        }
        self.add_undo(action, None);
        if let Some(u) = self.current_undo_mut() {
            u.strdata.clone_from(&comment_seq);
        }
        let mut line = Some(top);
        while line != stop {
            let l = line.unwrap();
            if self.comment_line(action, l, &comment_seq) {
                let ln = self.lineno(l);
                self.update_multiline_undo(ln, b"");
            }
            line = self.next(l);
        }
        self.set_modified();
        self.ensure_firstcolumn_is_aligned();
        self.refresh_needed = true;
        self.shift_held = true;
    }

    fn handle_comment_action(&mut self, ui: usize, undoing: bool, add_comment: bool) {
        let (head_lineno, head_x, groups, seq) = {
            let u = &self.ofile().undo[ui];
            (
                u.head_lineno,
                u.head_x,
                u.grouping.clone(),
                u.strdata.clone(),
            )
        };
        if !undoing {
            self.goto_line_posx(head_lineno, head_x);
        }
        for group in groups.iter().rev() {
            let mut line = Some(self.line_from_number(group.top_line));
            while let Some(l) = line {
                if self.lineno(l) > group.bottom_line {
                    break;
                }
                let act = if undoing ^ add_comment {
                    UndoType::Comment
                } else {
                    UndoType::Uncomment
                };
                self.comment_line(act, l, &seq);
                line = self.next(l);
            }
        }
        if undoing {
            self.goto_line_posx(head_lineno, head_x);
        }
        self.refresh_needed = true;
    }

    fn undo_cut(&mut self, ui: usize) {
        let (head_lineno, head_x, xflags, cb) = {
            let u = &self.ofile().undo[ui];
            (u.head_lineno, u.head_x, u.xflags, u.cutbuffer)
        };
        self.goto_line_posx(
            head_lineno,
            if xflags & WAS_WHOLE_LINE != 0 {
                0
            } else {
                head_x
            },
        );
        if xflags & HAD_ANCHOR_AT_START == 0 {
            let c = self.current();
            self.nodes[c].has_anchor = false;
        }
        if let Some(cb) = cb {
            self.copy_from_buffer(cb);
        }
        if xflags & INCLUDED_LAST_LINE != 0
            && !self.isset(Flag::NoNewlines)
            && self.filebot() != self.current()
        {
            let bp = self.prev(self.filebot()).unwrap();
            if self.datalen(bp) == 0 {
                self.remove_magicline();
            }
        }
        if xflags & CURSOR_WAS_AT_HEAD != 0 {
            self.goto_line_posx(head_lineno, head_x);
        }
    }

    fn redo_cut(&mut self, ui: usize) {
        let oldcutbuffer = self.cutbuffer;
        self.cutbuffer = None;
        let (head_lineno, head_x, xflags, tail_lineno, tail_x, utype) = {
            let u = &self.ofile().undo[ui];
            (
                u.head_lineno,
                u.head_x,
                u.xflags,
                u.tail_lineno,
                u.tail_x,
                u.utype,
            )
        };
        let m = self.line_from_number(head_lineno);
        {
            let of = self.ofile_mut();
            of.mark = Some(m);
            of.mark_x = if xflags & WAS_WHOLE_LINE != 0 {
                0
            } else {
                head_x
            };
        }
        self.goto_line_posx(tail_lineno, tail_x);
        self.do_snip(true, false, utype == UndoType::Zap);
        let cb = self.cutbuffer;
        self.free_lines(cb);
        self.cutbuffer = oldcutbuffer;
    }

    pub fn do_undo(&mut self) {
        let cur_undo = self.ofile().cur_undo;
        if cur_undo == 0 {
            self.statusline(Msg::Ahem, "Nichts rückgängig zu machen");
            return;
        }
        let mut ui = cur_undo - 1;
        let utype = self.ofile().undo[ui].utype;
        let line = if utype <= UndoType::Replace {
            let tl = self.ofile().undo[ui].tail_lineno;
            Some(self.line_from_number(tl))
        } else {
            None
        };
        let (head_lineno, head_x, tail_lineno, tail_x, xflags) = {
            let u = &self.ofile().undo[ui];
            (u.head_lineno, u.head_x, u.tail_lineno, u.tail_x, u.xflags)
        };
        let mut undidmsg: Option<String> = None;
        match utype {
            UndoType::Add => {
                undidmsg = Some("Hinzufügung".into());
                if xflags & INCLUDED_LAST_LINE != 0 && !self.isset(Flag::NoNewlines) {
                    self.remove_magicline();
                }
                let l = line.unwrap();
                let slen = self.ofile().undo[ui].strdata.len();
                let d = &mut self.nodes[l].data;
                let end = (head_x + slen).min(d.len());
                let start = head_x.min(end);
                d.drain(start..end);
                self.goto_line_posx(head_lineno, head_x);
            }
            UndoType::Enter => {
                undidmsg = Some("Zeilenvorschub".into());
                let l = line.unwrap();
                let original_x = if head_x == 0 { tail_x } else { head_x };
                let regain_from_x = if head_x == 0 { 0 } else { tail_x };
                let sd = self.ofile().undo[ui].strdata.clone();
                if regain_from_x < sd.len() {
                    self.nodes[l].data.extend_from_slice(&sd[regain_from_x..]);
                }
                if let Some(n) = self.next(l) {
                    let a = self.nodes[n].has_anchor;
                    self.nodes[l].has_anchor |= a;
                    self.unlink_node(n);
                }
                self.renumber_from(l);
                self.ofile_mut().current = l;
                self.goto_line_posx(head_lineno, original_x);
            }
            UndoType::Back | UndoType::Del => {
                undidmsg = Some("Löschung".into());
                let l = line.unwrap();
                let sd = self.ofile().undo[ui].strdata.clone();
                let d = &mut self.nodes[l].data;
                let hx = head_x.min(d.len());
                let tail: Vec<u8> = d.split_off(hx);
                d.extend_from_slice(&sd);
                d.extend_from_slice(&tail);
                self.goto_line_posx(tail_lineno, tail_x);
            }
            UndoType::Join => {
                undidmsg = Some("Zeile verbinden".into());
                if xflags & WAS_BACKSPACE_AT_EOF != 0 && !self.isset(Flag::NoNewlines) {
                    let bl = self.lineno(self.filebot());
                    self.goto_line_posx(bl, 0);
                    self.focusing = false;
                } else {
                    let l = line.unwrap();
                    self.nodes[l].data.truncate(tail_x);
                    let intruder = self.make_new_node(Some(l));
                    self.nodes[intruder]
                        .data
                        .clone_from(&self.files[self.cur].undo[ui].strdata);
                    self.splice_node(l, intruder);
                    self.renumber_from(intruder);
                    self.goto_line_posx(head_lineno, head_x);
                }
            }
            UndoType::Replace => {
                undidmsg = Some("Ersetzung".into());
                let l = line.unwrap();
                let cur = self.cur;
                std::mem::swap(
                    &mut self.files[cur].undo[ui].strdata,
                    &mut self.nodes[l].data,
                );
                self.goto_line_posx(head_lineno, head_x);
            }
            UndoType::SplitBegin => {
                undidmsg = Some("Hinzufügung".into());
            }
            UndoType::SplitEnd => {
                self.ofile_mut().cur_undo -= 1;
                while self.ofile().cur_undo > 0
                    && self.ofile().undo[self.ofile().cur_undo - 1].utype != UndoType::SplitBegin
                {
                    self.do_undo();
                }
                ui = self.ofile().cur_undo.saturating_sub(1);
            }
            UndoType::Zap => {
                undidmsg = Some("löschen".into());
                self.undo_cut(ui);
            }
            UndoType::CutToEof | UndoType::Cut => {
                undidmsg = Some("ausschneiden".into());
                self.undo_cut(ui);
            }
            UndoType::Paste => {
                undidmsg = Some("einfügen".into());
                self.redo_cut(ui);
                if xflags & INCLUDED_LAST_LINE != 0
                    && !self.isset(Flag::NoNewlines)
                    && self.filebot() != self.current()
                {
                    self.remove_magicline();
                }
            }
            UndoType::Insert => {
                undidmsg = Some("Einfügung".into());
                let oldcutbuffer = self.cutbuffer;
                self.cutbuffer = None;
                self.goto_line_posx(head_lineno, head_x);
                let m = self.line_from_number(tail_lineno);
                let of = self.ofile_mut();
                of.mark = Some(m);
                of.mark_x = tail_x;
                self.cut_marked_region();
                let cb = self.cutbuffer;
                let cur = self.cur;
                self.files[cur].undo[ui].cutbuffer = cb;
                self.cutbuffer = oldcutbuffer;
                if xflags & INCLUDED_LAST_LINE != 0
                    && !self.isset(Flag::NoNewlines)
                    && self.filebot() != self.current()
                {
                    self.remove_magicline();
                }
            }
            UndoType::CoupleBegin => {
                undidmsg =
                    Some(String::from_utf8_lossy(&self.ofile().undo[ui].strdata).into_owned());
                self.goto_line_posx(head_lineno, head_x);
                self.ofile_mut().cursor_row = tail_lineno;
                self.adjust_viewport(Update::Stationary);
            }
            UndoType::CoupleEnd => {
                let row = self.ofile().cursor_row;
                let cur = self.cur;
                self.files[cur].undo[ui].head_lineno = row;
                self.ofile_mut().cur_undo -= 1;
                self.do_undo();
                self.do_undo();
                self.do_undo();
                return;
            }
            UndoType::Indent => {
                self.handle_indent_action(ui, true, true);
                undidmsg = Some("Einrücken".into());
            }
            UndoType::Unindent => {
                self.handle_indent_action(ui, true, false);
                undidmsg = Some("Ausrücken".into());
            }
            UndoType::Comment => {
                self.handle_comment_action(ui, true, true);
                undidmsg = Some("Kommentieren".into());
            }
            UndoType::Uncomment => {
                self.handle_comment_action(ui, true, false);
                undidmsg = Some("Nicht kommentieren".into());
            }
            _ => {}
        }
        if let Some(m) = undidmsg
            && !self.isset(Flag::Zero)
            && self.pletion_line.is_none()
        {
            self.statusline(Msg::Hush, &fmt!("%s wurde rückgängig gemacht", &m));
        }
        let wassize = self.ofile().undo[ui].wassize;
        {
            let of = self.ofile_mut();
            of.cur_undo = ui;
            of.last_action = UndoType::Other;
            of.mark = None;
        }
        let pww = self.xplustabs();
        let of = self.ofile_mut();
        of.placewewant = pww;
        of.totsize = wassize;
        self.check_saved_state();
    }

    fn check_saved_state(&mut self) {
        let of = self.ofile();
        let at_saved = match (of.cur_undo, of.last_saved) {
            (0, None) => true,
            (n, Some(s)) if n > 0 => of.undo[n - 1].serial == s,
            _ => false,
        };
        if at_saved {
            self.ofile_mut().modified = false;
            self.titlebar(None);
        } else {
            self.set_modified();
        }
    }

    pub fn do_redo(&mut self) {
        let of = self.ofile();
        if of.undo.is_empty() || of.cur_undo == of.undo.len() {
            self.statusline(Msg::Ahem, "Nichts zu wiederholen");
            return;
        }
        let mut ui = of.cur_undo;
        let mut suppress_modification = false;
        let utype = self.ofile().undo[ui].utype;
        let line = if utype <= UndoType::Replace {
            let tl = self.ofile().undo[ui].tail_lineno;
            Some(self.line_from_number(tl))
        } else {
            None
        };
        let (head_lineno, head_x, tail_lineno, tail_x, xflags) = {
            let u = &self.ofile().undo[ui];
            (u.head_lineno, u.head_x, u.tail_lineno, u.tail_x, u.xflags)
        };
        let mut redidmsg: Option<String> = None;
        match utype {
            UndoType::Add => {
                redidmsg = Some("Hinzufügung".into());
                if xflags & INCLUDED_LAST_LINE != 0 && !self.isset(Flag::NoNewlines) {
                    self.new_magicline();
                }
                let l = line.unwrap();
                let sd = self.ofile().undo[ui].strdata.clone();
                let d = &mut self.nodes[l].data;
                let hx = head_x.min(d.len());
                let tail: Vec<u8> = d.split_off(hx);
                d.extend_from_slice(&sd);
                d.extend_from_slice(&tail);
                self.goto_line_posx(tail_lineno, tail_x);
            }
            UndoType::Enter => {
                redidmsg = Some("Zeilenvorschub".into());
                let l = line.unwrap();
                self.nodes[l].data.truncate(head_x);
                let intruder = self.make_new_node(Some(l));
                self.nodes[intruder]
                    .data
                    .clone_from(&self.files[self.cur].undo[ui].strdata);
                self.splice_node(l, intruder);
                self.renumber_from(intruder);
                self.goto_line_posx(head_lineno + 1, tail_x);
            }
            UndoType::Back | UndoType::Del => {
                redidmsg = Some("Löschung".into());
                let l = line.unwrap();
                let slen = self.ofile().undo[ui].strdata.len();
                let d = &mut self.nodes[l].data;
                let end = (head_x + slen).min(d.len());
                let start = head_x.min(end);
                d.drain(start..end);
                self.goto_line_posx(head_lineno, head_x);
            }
            UndoType::Join => {
                redidmsg = Some("Zeile verbinden".into());
                if xflags & WAS_BACKSPACE_AT_EOF == 0 || self.isset(Flag::NoNewlines) {
                    let l = line.unwrap();
                    let sd = self.ofile().undo[ui].strdata.clone();
                    self.nodes[l].data.extend_from_slice(&sd);
                    if let Some(n) = self.next(l) {
                        self.unlink_node(n);
                    }
                    self.renumber_from(l);
                    self.ofile_mut().current = l;
                }
                self.goto_line_posx(tail_lineno, tail_x);
            }
            UndoType::Replace => {
                redidmsg = Some("Ersetzung".into());
                let l = line.unwrap();
                let cur = self.cur;
                std::mem::swap(
                    &mut self.files[cur].undo[ui].strdata,
                    &mut self.nodes[l].data,
                );
                self.goto_line_posx(head_lineno, head_x);
            }
            UndoType::SplitBegin => {
                self.ofile_mut().cur_undo = ui + 1;
                while self.ofile().cur_undo < self.ofile().undo.len()
                    && self.ofile().undo[self.ofile().cur_undo - 1].utype != UndoType::SplitEnd
                {
                    self.do_redo();
                }
                ui = self.ofile().cur_undo - 1;
                let (hl, hx) = {
                    let u = &self.ofile().undo[ui];
                    (u.head_lineno, u.head_x)
                };
                self.goto_line_posx(hl, hx);
                self.ensure_firstcolumn_is_aligned();
            }
            UndoType::SplitEnd => {
                redidmsg = Some("Hinzufügung".into());
            }
            UndoType::Zap => {
                redidmsg = Some("löschen".into());
                self.redo_cut(ui);
            }
            UndoType::CutToEof | UndoType::Cut => {
                redidmsg = Some("ausschneiden".into());
                self.redo_cut(ui);
            }
            UndoType::Paste => {
                redidmsg = Some("einfügen".into());
                self.undo_cut(ui);
            }
            UndoType::Insert => {
                redidmsg = Some("Einfügung".into());
                self.goto_line_posx(head_lineno, head_x);
                let cb = self.ofile().undo[ui].cutbuffer;
                if let Some(cb) = cb {
                    self.copy_from_buffer(cb);
                } else {
                    suppress_modification = true;
                }
                self.free_lines(cb);
                let cur = self.cur;
                self.files[cur].undo[ui].cutbuffer = None;
            }
            UndoType::CoupleBegin => {
                self.ofile_mut().cur_undo = ui + 1;
                self.do_redo();
                self.do_redo();
                self.do_redo();
                return;
            }
            UndoType::CoupleEnd => {
                redidmsg =
                    Some(String::from_utf8_lossy(&self.ofile().undo[ui].strdata).into_owned());
                self.goto_line_posx(tail_lineno, tail_x);
                self.ofile_mut().cursor_row = head_lineno;
                self.adjust_viewport(Update::Stationary);
            }
            UndoType::Indent => {
                self.handle_indent_action(ui, false, true);
                redidmsg = Some("Einrücken".into());
            }
            UndoType::Unindent => {
                self.handle_indent_action(ui, false, false);
                redidmsg = Some("Ausrücken".into());
            }
            UndoType::Comment => {
                self.handle_comment_action(ui, false, true);
                redidmsg = Some("Kommentieren".into());
            }
            UndoType::Uncomment => {
                self.handle_comment_action(ui, false, false);
                redidmsg = Some("Nicht kommentieren".into());
            }
            _ => {}
        }
        if let Some(m) = redidmsg
            && !self.isset(Flag::Zero)
        {
            self.statusline(Msg::Hush, &fmt!("%s wurde wiederholt", &m));
        }
        let newsize = self.ofile().undo[ui].newsize;
        {
            let of = self.ofile_mut();
            of.cur_undo = ui + 1;
            of.last_action = UndoType::Other;
            of.mark = None;
        }
        let pww = self.xplustabs();
        let of = self.ofile_mut();
        of.placewewant = pww;
        of.totsize = newsize;
        let of = self.ofile();
        let at_saved = match (of.cur_undo, of.last_saved) {
            (0, None) => true,
            (n, Some(s)) if n > 0 => of.undo[n - 1].serial == s,
            _ => false,
        };
        if at_saved {
            self.ofile_mut().modified = false;
            self.titlebar(None);
        } else if !suppress_modification {
            self.set_modified();
        }
    }

    pub fn do_enter(&mut self) {
        let cur = self.current();
        let newnode = self.make_new_node(Some(cur));
        let mut extra = 0usize;
        let mut sampleline = cur;
        let mut allblanks = false;
        if self.isset(Flag::AutoIndent) {
            if self.isset(Flag::BreakLongLines)
                && let Some(n) = self.next(cur)
                && self.inpar(n)
                && !self.begpar(n, 0)
            {
                sampleline = n;
            }
            extra = indent_length(self.data(sampleline));
            let cx = self.current_x();
            if extra > cx {
                extra = cx;
            } else if extra == cx {
                allblanks = indent_length(self.data(cur)) == extra;
            }
        }
        let cx = self.current_x();
        let mut newdata: Vec<u8> = Vec::with_capacity(self.datalen(cur) - cx + extra);
        if self.isset(Flag::AutoIndent) {
            newdata.extend_from_slice(&self.nodes[sampleline].data[..extra]);
        } else {
            newdata.extend(std::iter::repeat_n(0u8, extra));
        }
        newdata.extend_from_slice(&self.nodes[cur].data[cx..]);
        self.nodes[newnode].data = newdata;
        {
            let of = self.ofile_mut();
            if of.mark == Some(cur) && of.mark_x > of.current_x {
                of.mark = Some(newnode);
                of.mark_x += extra;
                of.mark_x -= of.current_x;
            }
        }
        if self.isset(Flag::AutoIndent) {
            if allblanks {
                self.ofile_mut().current_x = 0;
            }
            let of = self.ofile_mut();
            if allblanks && of.mark == Some(cur) {
                of.mark_x = 0;
            }
        }
        let cx = self.current_x();
        self.nodes[cur].data.truncate(cx);
        self.add_undo(UndoType::Enter, None);
        self.splice_node(cur, newnode);
        self.renumber_from(newnode);
        {
            let of = self.ofile_mut();
            of.current = newnode;
            of.current_x = extra;
        }
        let pww = self.xplustabs();
        let of = self.ofile_mut();
        of.placewewant = pww;
        of.totsize = of.totsize.wrapping_add(1);
        self.set_modified();
        if self.isset(Flag::AutoIndent) && !allblanks {
            self.ofile_mut().totsize = self.ofile().totsize.wrapping_add(extra);
        }
        self.update_undo(UndoType::Enter);
        self.refresh_needed = true;
        self.focusing = false;
    }

    pub fn discard_until_current(&mut self) {
        let cur = self.ofile().cur_undo;
        let cutbuffers: Vec<Option<Lp>> = self.ofile().undo[cur..]
            .iter()
            .map(|u| u.cutbuffer)
            .collect();
        for cb in cutbuffers {
            self.free_lines(cb);
        }
        let of = self.ofile_mut();
        of.undo.truncate(cur);
        of.last_action = UndoType::Other;
    }

    pub fn discard_all_undo(&mut self) {
        self.ofile_mut().cur_undo = 0;
        self.discard_until_current();
    }

    pub fn discard_topmost_undo(&mut self) {
        let len = self.ofile().undo.len();
        if len > 0 {
            self.ofile_mut().cur_undo = len - 1;
            self.discard_until_current();
        }
    }

    pub fn add_undo(&mut self, action: UndoType, message: Option<&str>) {
        let thisline = self.current();
        let cx = self.current_x();
        let totsize = self.ofile().totsize;
        self.undo_serial += 1;
        let mut u = Undo {
            serial: self.undo_serial,
            utype: action,
            xflags: 0,
            head_lineno: self.lineno(thisline),
            head_x: cx,
            strdata: Vec::new(),
            wassize: totsize,
            newsize: totsize,
            grouping: Vec::new(),
            cutbuffer: None,
            tail_lineno: self.lineno(thisline),
            tail_x: cx,
        };
        self.discard_until_current();
        let mut last_action = action;

        let ui = if u.utype == UndoType::SplitBegin && !self.ofile().undo.is_empty() {
            let top = self.ofile().undo.len() - 1;
            last_action = self.ofile().undo[top].utype;
            u.wassize = self.ofile().undo[top].wassize;
            self.ofile_mut().undo.insert(top, u);
            top
        } else {
            self.ofile_mut().undo.push(u);
            self.ofile().undo.len() - 1
        };
        let len = self.ofile().undo.len();
        self.ofile_mut().cur_undo = len;
        let filebot = self.filebot();
        let mark = self.ofile().mark;
        let mark_x = self.ofile().mark_x;
        let mark_before = if mark.is_some() {
            self.mark_is_before_cursor()
        } else {
            false
        };
        let cur_anchor = self.nodes[thisline].has_anchor;
        let mark_anchor = mark.is_some_and(|m| self.nodes[m].has_anchor);
        let cutbuffer = self.cutbuffer;
        let cursor_row = self.ofile().cursor_row;
        let cur_data_len = self.datalen(thisline);
        let next_line = self.next(thisline);
        let next_is_bot = next_line == Some(filebot);
        let next_data: Option<Vec<u8>> = next_line.map(|n| self.nodes[n].data.clone());
        let next_lineno = next_line.map(|n| self.lineno(n));
        let charlen_at_cursor = if cx < cur_data_len {
            char_length(&self.nodes[thisline].data[cx..])
        } else {
            0
        };
        let char_at_cursor: Vec<u8> = if cx < cur_data_len {
            self.nodes[thisline].data[cx..cx + charlen_at_cursor].to_vec()
        } else {
            Vec::new()
        };
        let whole_line: Vec<u8> = self.nodes[thisline].data.clone();
        let cfc = self.isset(Flag::CutFromCursor);
        let filebot_lineno = self.lineno(filebot);
        let paste_copy = if action == UndoType::Paste
            && let Some(cb) = cutbuffer
        {
            Some(self.copy_buffer(cb))
        } else {
            None
        };
        let cur = self.cur;
        let u = &mut self.files[cur].undo[ui];
        match u.utype {
            UndoType::Add | UndoType::Insert => {
                if thisline == filebot {
                    u.xflags |= INCLUDED_LAST_LINE;
                }
            }
            UndoType::Back | UndoType::Del => {
                if u.utype == UndoType::Back && next_is_bot && cur_data_len > 0 {
                    u.xflags |= WAS_BACKSPACE_AT_EOF;
                }
                if cx < cur_data_len {
                    u.strdata = char_at_cursor;
                    if u.utype == UndoType::Back {
                        u.tail_x += charlen_at_cursor;
                    }
                } else {
                    last_action = UndoType::Join;
                    if let Some(nd) = next_data {
                        if u.utype == UndoType::Back {
                            u.head_lineno = next_lineno.unwrap();
                            u.head_x = 0;
                        }
                        u.strdata = nd;
                    }
                    u.utype = UndoType::Join;
                }
            }
            UndoType::Replace => {
                u.strdata = whole_line;
            }
            UndoType::CutToEof => {
                u.xflags |= INCLUDED_LAST_LINE | CURSOR_WAS_AT_HEAD;
                if cur_anchor {
                    u.xflags |= HAD_ANCHOR_AT_START;
                }
            }
            UndoType::Zap | UndoType::Cut => {
                if let Some(m) = mark {
                    let mln = self.nodes[m].lineno;
                    if mark_before {
                        u.head_lineno = mln;
                        u.head_x = mark_x;
                        u.xflags |= MARK_WAS_SET;
                    } else {
                        u.tail_lineno = mln;
                        u.tail_x = mark_x;
                        u.xflags |= MARK_WAS_SET | CURSOR_WAS_AT_HEAD;
                    }
                    if u.tail_lineno == filebot_lineno {
                        u.xflags |= INCLUDED_LAST_LINE;
                    }
                } else if !cfc {
                    u.xflags |= WAS_WHOLE_LINE | CURSOR_WAS_AT_HEAD;
                    u.tail_x = 0;
                } else {
                    u.xflags |= CURSOR_WAS_AT_HEAD;
                }
                if (mark.is_some() && mark_before && mark_anchor)
                    || ((mark.is_none() || !mark_before) && cur_anchor)
                {
                    u.xflags |= HAD_ANCHOR_AT_START;
                }
            }
            UndoType::Paste => {
                u.cutbuffer = paste_copy;
                if thisline == filebot {
                    u.xflags |= INCLUDED_LAST_LINE;
                }
            }
            UndoType::CoupleBegin => {
                u.tail_lineno = cursor_row;
                u.strdata = message.unwrap_or("").as_bytes().to_vec();
            }
            UndoType::CoupleEnd => {
                u.strdata = message.unwrap_or("").as_bytes().to_vec();
            }
            UndoType::Enter
            | UndoType::Join
            | UndoType::SplitBegin
            | UndoType::SplitEnd
            | UndoType::Indent
            | UndoType::Unindent
            | UndoType::Comment
            | UndoType::Uncomment
            | UndoType::Preflight
            | UndoType::Copy
            | UndoType::Other => {}
        }
        self.ofile_mut().last_action = last_action;
    }

    pub fn update_multiline_undo(&mut self, lineno: i64, indentation: &[u8]) {
        let totsize = self.ofile().totsize;
        let Some(u) = self.current_undo_mut() else {
            return;
        };
        let extend = u
            .grouping
            .last()
            .is_some_and(|g| g.bottom_line + 1 == lineno);
        if extend {
            let g = u.grouping.last_mut().unwrap();
            g.bottom_line = lineno;
            g.indentations.push(indentation.to_vec());
        } else {
            u.grouping.push(Group {
                top_line: lineno,
                bottom_line: lineno,
                indentations: vec![indentation.to_vec()],
            });
        }
        u.newsize = totsize;
    }

    pub fn update_undo(&mut self, action: UndoType) {
        let _ = action;
        let cur = self.current();
        let cx = self.current_x();
        let totsize = self.ofile().totsize;
        let lineno = self.lineno(cur);
        let data = self.nodes[cur].data.clone();
        let cutbuffer = self.cutbuffer;
        let cfc = self.isset(Flag::CutFromCursor);
        let nn = self.isset(Flag::NoNewlines);
        let at_bot = cur == self.filebot();
        let ui = match self.ofile().undo.len() {
            0 => return,
            n => n - 1,
        };
        let utype = self.ofile().undo[ui].utype;
        let mut need_new_undo: Option<UndoType> = None;
        let cut_copy = if matches!(utype, UndoType::Cut | UndoType::CutToEof)
            && let Some(cb) = cutbuffer
        {
            let old = self.ofile().undo[ui].cutbuffer;
            self.free_lines(old);
            Some(self.copy_buffer(cb))
        } else {
            None
        };
        let cutinfo = |k: &Self, cb: Lp| -> (usize, usize) {
            let mut bottom = cb;
            let mut count = 0usize;
            while let Some(n) = k.next(bottom) {
                bottom = n;
                count += 1;
            }
            (count, k.datalen(bottom))
        };
        let zap_info = if utype == UndoType::Zap {
            cutbuffer.map(|cb| cutinfo(self, cb))
        } else {
            cut_copy.map(|cb| cutinfo(self, cb))
        };
        let curidx = self.cur;
        let u = &mut self.files[curidx].undo[ui];
        u.newsize = totsize;
        match u.utype {
            UndoType::Add => {
                let newlen = cx.saturating_sub(u.head_x);
                let end = (u.head_x + newlen).min(data.len());
                u.strdata = data[u.head_x.min(end)..end].to_vec();
                u.tail_x = cx;
            }
            UndoType::Enter => {
                u.strdata = data;
                u.tail_x = cx;
            }
            UndoType::Back | UndoType::Del => {
                let charlen = if cx < data.len() {
                    char_length(&data[cx..])
                } else {
                    1
                };
                let piece: Vec<u8> = if cx < data.len() {
                    data[cx..(cx + charlen).min(data.len())].to_vec()
                } else {
                    Vec::new()
                };
                if cx == u.head_x {
                    u.strdata.extend_from_slice(&piece);
                    u.tail_x = cx;
                } else if cx + charlen == u.head_x {
                    let mut nd = piece;
                    nd.extend_from_slice(&u.strdata);
                    u.strdata = nd;
                    u.head_x = cx;
                } else {
                    need_new_undo = Some(u.utype);
                }
            }
            UndoType::Zap | UndoType::CutToEof | UndoType::Cut => {
                if u.utype == UndoType::Zap {
                    u.cutbuffer = cutbuffer;
                } else if cut_copy.is_some() {
                    u.cutbuffer = cut_copy;
                } else {
                    return;
                }
                if u.xflags & MARK_WAS_SET == 0
                    && let Some((count, botlen)) = zap_info
                {
                    u.tail_lineno = u.head_lineno + i64_of(count);
                    if cfc || u.utype == UndoType::CutToEof {
                        u.tail_x = botlen;
                        if count == 0 {
                            u.tail_x += u.head_x;
                        }
                    } else if at_bot && nn {
                        u.tail_x = botlen;
                    }
                }
            }
            UndoType::CoupleEnd | UndoType::Paste | UndoType::Insert => {
                u.tail_lineno = lineno;
                u.tail_x = cx;
            }
            UndoType::Replace
            | UndoType::SplitBegin
            | UndoType::SplitEnd
            | UndoType::CoupleBegin
            | UndoType::Join
            | UndoType::Indent
            | UndoType::Unindent
            | UndoType::Comment
            | UndoType::Uncomment
            | UndoType::Preflight
            | UndoType::Copy
            | UndoType::Other => {}
        }
        if let Some(t) = need_new_undo {
            self.add_undo(t, None);
        }
    }

    pub fn do_wrap(&mut self) {
        let line = self.current();
        let line_len = self.datalen(line);
        let quot_len = self.quote_length(self.data(line));
        let lead_len = quot_len + indent_length(&self.nodes[line].data[quot_len..]);
        let cursor_x = self.current_x();
        let data = self.nodes[line].data.clone();
        let goal = self.wrap_at.cast_signed() - (self.wideness(&data, lead_len)).cast_signed();
        let wrap_loc = self.break_line(&data[lead_len..], goal, false);
        if wrap_loc < 0 || lead_len + wrap_loc.cast_unsigned() == line_len {
            return;
        }
        let wrap_loc = lead_len + step_right(&data[lead_len..], wrap_loc.cast_unsigned());
        if wrap_loc >= line_len {
            return;
        }
        self.add_undo(UndoType::SplitBegin, None);
        let autowhite = self.isset(Flag::AutoIndent);
        if quot_len > 0 {
            self.flags.unset(Flag::AutoIndent);
        }
        let mut rest_length = line_len - wrap_loc;
        let spill = self.ofile().spillage_line;
        if spill.is_some()
            && spill == self.next(line)
            && rest_length + self.breadth(self.data(spill.unwrap())) <= self.wrap_at
        {
            self.ofile_mut().current_x = line_len;
            let rem = &self.nodes[line].data[wrap_loc..];
            let last = step_left(rem, rest_length);
            if !is_blank_char(&rem[last..]) {
                self.add_undo(UndoType::Add, None);
                self.nodes[line].data.push(b' ');
                rest_length += 1;
                self.ofile_mut().totsize = self.ofile().totsize.wrapping_add(1);
                self.ofile_mut().current_x += 1;
                self.update_undo(UndoType::Add);
            }
            let _ = rest_length;
            self.expunge(UndoType::Del);
            let cx = self.current_x();
            let d = &self.nodes[line].data;
            if d.len() >= cx + lead_len && d[..lead_len] == d[cx..cx + lead_len] {
                for _ in 0..lead_len {
                    self.expunge(UndoType::Del);
                }
            }
            loop {
                let cx = self.current_x();
                let d = &self.nodes[line].data;
                if cx < d.len() && is_blank_char(&d[cx..]) {
                    self.expunge(UndoType::Del);
                } else {
                    break;
                }
            }
        }
        self.ofile_mut().current_x = wrap_loc;
        if self.isset(Flag::TrimBlanks) {
            let d = &self.nodes[line].data;
            let mut rear_x = step_left(d, wrap_loc);
            let typed_x = step_left(d, cursor_x);
            loop {
                let d = &self.nodes[line].data;
                if (rear_x != typed_x || cursor_x >= wrap_loc)
                    && rear_x < d.len()
                    && is_blank_char(&d[rear_x..])
                {
                    self.ofile_mut().current_x = rear_x;
                    self.expunge(UndoType::Del);
                    let d = &self.nodes[line].data;
                    rear_x = step_left(d, rear_x);
                } else {
                    break;
                }
            }
        }
        self.do_enter();
        if self.edittop() == line && self.ofile().firstcolumn > 0 && cursor_x >= wrap_loc {
            let mut top = self.edittop();
            let mut fc = self.ofile().firstcolumn;
            self.go_forward_chunks(1, &mut top, &mut fc);
            let of = self.ofile_mut();
            of.edittop = top;
            of.firstcolumn = fc;
        }
        if quot_len > 0 {
            let nl = self.next(line).unwrap();
            let lead: Vec<u8> = self.nodes[line].data[..lead_len].to_vec();
            let d = &mut self.nodes[nl].data;
            let mut nd = lead;
            nd.extend_from_slice(d);
            *d = nd;
            self.ofile_mut().current_x += lead_len;
            self.ofile_mut().totsize = self.ofile().totsize.wrapping_add(lead_len);
            self.update_undo(UndoType::Enter);
            if autowhite {
                self.flags.set(Flag::AutoIndent);
            }
        }
        let c = self.current();
        self.ofile_mut().spillage_line = Some(c);
        if cursor_x < wrap_loc {
            let p = self.prev(c).unwrap();
            let of = self.ofile_mut();
            of.current = p;
            of.current_x = cursor_x;
        } else {
            self.ofile_mut().current_x += cursor_x - wrap_loc;
        }
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
        self.add_undo(UndoType::SplitEnd, None);
        self.refresh_needed = true;
    }

    pub fn break_line(&self, textstart: &[u8], goal: isize, snap_at_nl: bool) -> isize {
        let mut lastblank: Option<usize> = None;
        let mut pointer = 0usize;
        let mut column = 0usize;
        while pointer < textstart.len() && is_blank_char(&textstart[pointer..]) {
            pointer += self.advance_over(&textstart[pointer..], &mut column);
        }
        while pointer < textstart.len() && (column.cast_signed()) <= goal {
            if is_blank_char(&textstart[pointer..]) && (!self.inhelp || column > 17 || goal < 40) {
                lastblank = Some(pointer);
            } else if snap_at_nl && textstart[pointer] == b'\n' {
                lastblank = Some(pointer);
                break;
            }
            pointer += self.advance_over(&textstart[pointer..], &mut column);
        }
        if (column.cast_signed()) <= goal {
            return pointer.cast_signed();
        }
        if snap_at_nl && lastblank.is_none() {
            return (step_left(textstart, pointer)).cast_signed();
        }
        while lastblank.is_none() {
            if pointer >= textstart.len() {
                return -1;
            }
            if is_blank_char(&textstart[pointer..]) {
                lastblank = Some(pointer);
            } else {
                pointer += char_length(&textstart[pointer..]);
            }
        }
        let mut lb = lastblank.unwrap();
        pointer = lb + char_length(&textstart[lb..]);
        while pointer < textstart.len() && is_blank_char(&textstart[pointer..]) {
            lb = pointer;
            pointer += char_length(&textstart[pointer..]);
        }
        lb.cast_signed()
    }

    pub fn quote_length(&self, line: &[u8]) -> usize {
        let Some(re) = &self.quotereg else { return 0 };
        let mut m: [libc::regmatch_t; 1] = unsafe { std::mem::zeroed() };
        if !crate::regex::exec(re, line, 0, line.len(), &mut m, 0) || m[0].rm_so < 0 {
            return 0;
        }
        usize_of(crate::regex::offset(m[0].rm_eo))
    }

    pub fn begpar(&self, line: Lp, depth: i32) -> bool {
        let Some(prev) = self.prev(line) else {
            return true;
        };
        if depth > 222 {
            return false;
        }
        let d = self.data(line);
        let quot_len = self.quote_length(d);
        let indent_len = indent_length(&d[quot_len..]);
        if quot_len + indent_len >= d.len() {
            return false;
        }
        if self.isset(Flag::BookStyle) && !self.isset(Flag::AutoIndent) && is_blank_char(d) {
            return true;
        }
        let pd = self.data(prev);
        if quot_len != self.quote_length(pd) || d[..quot_len] != pd[..quot_len.min(pd.len())] {
            return true;
        }
        let prev_dent_len = indent_length(&pd[quot_len..]);
        if quot_len + prev_dent_len >= pd.len() {
            return true;
        }
        if self.wideness(pd, quot_len + prev_dent_len) == self.wideness(d, quot_len + indent_len) {
            return false;
        }
        !self.begpar(prev, depth + 1)
    }

    pub fn inpar(&self, line: Lp) -> bool {
        let d = self.data(line);
        let quot_len = self.quote_length(d);
        let indent_len = indent_length(&d[quot_len..]);
        quot_len + indent_len < d.len()
    }

    pub fn find_paragraph(&self, firstline: &mut Lp) -> Option<usize> {
        let mut line = *firstline;
        while !self.inpar(line) && self.next(line).is_some() {
            line = self.next(line).unwrap();
        }
        *firstline = line;
        self.do_para_end(&mut line);
        if !self.inpar(line) {
            return None;
        }
        Some(usize_of(self.lineno(line) - self.lineno(*firstline) + 1))
    }

    pub fn concat_paragraph(&mut self, line: Lp, mut count: usize) {
        while count > 1 {
            let Some(next_line) = self.next(line) else {
                break;
            };
            let nd = self.nodes[next_line].data.clone();
            let next_quot_len = self.quote_length(&nd);
            let next_lead_len = next_quot_len + indent_length(&nd[next_quot_len..]);
            {
                let d = &mut self.nodes[line].data;
                if !d.is_empty() && *d.last().unwrap() != b' ' {
                    d.push(b' ');
                }
                d.extend_from_slice(&nd[next_lead_len..]);
            }
            let a = self.nodes[next_line].has_anchor;
            self.nodes[line].has_anchor |= a;
            self.unlink_node(next_line);
            count -= 1;
        }
    }

    pub fn squeeze(&mut self, line: Lp, skip: usize) {
        let data = self.nodes[line].data.clone();
        let mut out: Vec<u8> = data[..skip.min(data.len())].to_vec();
        let mut from = skip.min(data.len());
        while from < data.len() {
            if is_blank_char(&data[from..]) {
                from += char_length(&data[from..]);
                out.push(b' ');
                while from < data.len() && is_blank_char(&data[from..]) {
                    from += char_length(&data[from..]);
                }
            } else if mbstrchr(&self.punct, &data[from..]).is_some() {
                let l = char_length(&data[from..]);
                out.extend_from_slice(&data[from..from + l]);
                from += l;
                if from < data.len() && mbstrchr(&self.brackets, &data[from..]).is_some() {
                    let l = char_length(&data[from..]);
                    out.extend_from_slice(&data[from..from + l]);
                    from += l;
                }
                if from < data.len() && is_blank_char(&data[from..]) {
                    from += char_length(&data[from..]);
                    out.push(b' ');
                }
                if from < data.len() && is_blank_char(&data[from..]) {
                    from += char_length(&data[from..]);
                    out.push(b' ');
                }
                while from < data.len() && is_blank_char(&data[from..]) {
                    from += char_length(&data[from..]);
                }
            } else {
                let l = char_length(&data[from..]);
                out.extend_from_slice(&data[from..(from + l).min(data.len())]);
                from += l;
            }
        }
        while out.len() > skip && *out.last().unwrap() == b' ' {
            out.pop();
        }
        self.nodes[line].data = out;
    }

    pub fn rewrap_paragraph(&mut self, line: &mut Lp, lead_string: &[u8], lead_len: usize) {
        while self.breadth(self.data(*line)) > self.wrap_at {
            let data = self.nodes[*line].data.clone();
            let line_len = data.len();
            let goal = self.wrap_at.cast_signed() - (self.wideness(&data, lead_len)).cast_signed();
            let break_pos = self.break_line(&data[lead_len..], goal, false);
            if break_pos < 0 || lead_len + break_pos.cast_unsigned() == line_len {
                break;
            }
            let mut break_pos = break_pos.cast_unsigned() + lead_len + 1;
            let n = self.make_new_node(Some(*line));
            self.splice_node(*line, n);
            let mut nd: Vec<u8> = lead_string[..lead_len].to_vec();
            nd.extend_from_slice(&data[break_pos.min(line_len)..]);
            self.nodes[n].data = nd;
            if self.isset(Flag::TrimBlanks) {
                while break_pos > 0 && data[break_pos - 1] == b' ' {
                    break_pos -= 1;
                }
            }
            self.nodes[*line].data.truncate(break_pos);
            *line = n;
        }
        if let Some(n) = self.next(*line) {
            *line = n;
        }
    }

    pub fn justify_paragraph(&mut self, line: &mut Lp, count: usize) {
        let sampleline = if count == 1 {
            *line
        } else {
            self.next(*line).unwrap()
        };
        let sd = self.nodes[sampleline].data.clone();
        let quot_len = self.quote_length(&sd);
        let lead_len = quot_len + indent_length(&sd[quot_len..]);
        let lead_string: Vec<u8> = sd[..lead_len].to_vec();
        self.concat_paragraph(*line, count);
        let d = self.nodes[*line].data.clone();
        let skip = quot_len + indent_length(&d[quot_len.min(d.len())..]);
        self.squeeze(*line, skip);
        self.rewrap_paragraph(line, &lead_string, lead_len);
    }

    pub fn justify_text(&mut self, whole_buffer: bool) {
        let was_cutbuffer = self.cutbuffer;
        let mut before_eol = false;
        let mut primary_lead: Vec<u8> = Vec::new();
        let mut secondary_lead: Vec<u8> = Vec::new();
        let was_the_linenumber = self.lineno(self.current());
        let marked_backward = self.ofile().mark.is_some() && !self.mark_is_before_cursor();
        self.add_undo(UndoType::CoupleBegin, Some("Ausrichtung"));
        let startline;
        let endline;
        let mut start_x;
        let mut end_x;
        let linecount;
        if self.ofile().mark.is_some() {
            let (sl, sx, el, ex) = self.get_region();
            startline = sl;
            start_x = sx;
            endline = el;
            end_x = ex;
            if startline == endline && start_x == end_x {
                self.statusline(Msg::Ahem, "Auswahl ist leer");
                self.discard_topmost_undo();
                return;
            }
            let sd = self.nodes[startline].data.clone();
            let quot_len = self.quote_length(&sd);
            let fore_len = quot_len + indent_length(&sd[quot_len..]);
            if start_x <= fore_len {
                start_x = 0;
            }
            while start_x > 0 && is_blank_char(&sd[start_x - 1..]) {
                start_x = step_left(&sd, start_x);
            }
            let ed = self.nodes[endline].data.clone();
            let quot_len = self.quote_length(&ed);
            let fore_len = quot_len + indent_length(&ed[quot_len..]);
            if 0 < end_x && end_x < fore_len {
                end_x = fore_len;
            }
            while end_x > 0 && end_x < ed.len() && is_blank_char(&ed[end_x..]) {
                end_x = step_right(&ed, end_x);
            }
            let mut sampleline = startline;
            while self.prev(sampleline).is_some()
                && self.inpar(sampleline)
                && !self.begpar(sampleline, 0)
            {
                sampleline = self.prev(sampleline).unwrap();
            }
            while self.next(sampleline).is_some() && !self.inpar(sampleline) {
                sampleline = self.next(sampleline).unwrap();
            }
            let smd = self.nodes[sampleline].data.clone();
            let quot_len = self.quote_length(&smd);
            let primary_len = quot_len + indent_length(&smd[quot_len..]);
            primary_lead = smd[..primary_len].to_vec();
            if self.next(sampleline).is_some() && startline != endline {
                sampleline = self.next(sampleline).unwrap();
            }
            let smd = self.nodes[sampleline].data.clone();
            let other_quot_len = self.quote_length(&smd);
            let other_white_len = indent_length(&smd[other_quot_len..]);
            secondary_lead = if sd.len() >= quot_len {
                sd[..quot_len].to_vec()
            } else {
                primary_lead[..quot_len].to_vec()
            };
            secondary_lead
                .extend_from_slice(&smd[other_quot_len..other_quot_len + other_white_len]);
            {
                let of = self.ofile_mut();
                of.mark = Some(startline);
                of.mark_x = start_x;
                of.current = endline;
                of.current_x = end_x;
            }
            linecount =
                usize_of(self.lineno(endline) - self.lineno(startline)) + usize::from(end_x > 0);
            before_eol = end_x < self.datalen(endline);
        } else {
            if whole_buffer {
                let top = self.filetop();
                self.ofile_mut().current = top;
            } else if self.inpar(self.current()) && !self.begpar(self.current(), 0) {
                let mut c = self.current();
                self.do_para_begin(&mut c);
                self.ofile_mut().current = c;
            }
            let mut c = self.current();
            match self.find_paragraph(&mut c) {
                None => {
                    let len = self.datalen(self.filebot());
                    self.ofile_mut().current = c;
                    self.ofile_mut().current_x = len;
                    self.discard_topmost_undo();
                    self.refresh_needed = true;
                    return;
                }
                Some(lc) => {
                    linecount = lc;
                    self.ofile_mut().current = c;
                    self.ofile_mut().current_x = 0;
                }
            }
            startline = self.current();
            start_x = 0;
            let mut el;
            if whole_buffer {
                el = self.filebot();
            } else {
                el = startline;
                for _ in 1..linecount {
                    el = self.next(el).unwrap();
                }
            }
            if let Some(n) = self.next(el) {
                el = n;
                end_x = 0;
            } else {
                end_x = self.datalen(el);
            }
            endline = el;
        }
        self.add_undo(UndoType::Cut, None);
        self.cutbuffer = None;
        self.extract_segment(startline, start_x, endline, end_x);
        self.update_undo(UndoType::Cut);
        let Some(cb) = self.cutbuffer else {
            self.cutbuffer = was_cutbuffer;
            return;
        };
        if self.ofile().mark.is_some() {
            let mut line = cb;
            let d = self.nodes[line].data.clone();
            let quot_len = self.quote_length(&d);
            let fore_len = quot_len + indent_length(&d[quot_len..]);
            let text: Vec<u8> = d[fore_len..].to_vec();
            let mut nd = primary_lead.clone();
            nd.extend_from_slice(&text);
            self.nodes[line].data = nd;
            self.concat_paragraph(cb, linecount);
            let pl = primary_lead.len();
            self.squeeze(cb, pl);
            let sl = secondary_lead.len();
            self.rewrap_paragraph(&mut line, &secondary_lead, sl);
            if start_x > 0 {
                let n = self.make_new_node(None);
                self.nodes[n].data = Vec::new();
                self.nodes[n].next = Some(cb);
                self.nodes[cb].prev = Some(n);
                self.cutbuffer = Some(n);
            }
            if end_x > 0 && before_eol {
                let last = self.last_of(cb);
                let n = self.make_new_node(Some(last));
                self.nodes[n].data = primary_lead;
                self.nodes[last].next = Some(n);
            }
            self.focusing = false;
        } else {
            let mut jusline = cb;
            self.justify_paragraph(&mut jusline, linecount);
            if whole_buffer {
                loop {
                    let mut j = jusline;
                    match self.find_paragraph(&mut j) {
                        None => break,
                        Some(lc) => {
                            jusline = j;
                            self.justify_paragraph(&mut jusline, lc);
                            if self.next(jusline).is_none() {
                                break;
                            }
                        }
                    }
                }
            }
        }
        let cb = self.cutbuffer.unwrap();
        if whole_buffer && self.ofile().mark.is_none() && !self.nodes[cb].has_anchor {
            let c = self.current();
            self.nodes[c].has_anchor = false;
        }
        self.add_undo(UndoType::Paste, None);
        self.ingraft_buffer(cb);
        self.update_undo(UndoType::Paste);
        if marked_backward {
            let of = self.ofile_mut();
            let bottom = of.current;
            let bottom_x = of.current_x;
            of.current = of.mark.unwrap();
            of.current_x = of.mark_x;
            of.mark = Some(bottom);
            of.mark_x = bottom_x;
        } else if whole_buffer && self.ofile().mark.is_none() {
            self.goto_line_posx(was_the_linenumber, 0);
        }
        self.add_undo(UndoType::CoupleEnd, Some("Ausrichtung"));
        if self.ofile().mark.is_some() {
            self.statusline(Msg::Remark, "Auswahl wurde ausgerichtet");
        } else if whole_buffer {
            self.statusline(Msg::Remark, "Datei wurde ausgerichtet");
        } else {
            self.statusbar("Absatz wurde ausgerichtet");
        }
        self.cutbuffer = was_cutbuffer;
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
        self.set_modified();
        self.refresh_needed = true;
        self.shift_held = true;
    }

    pub fn do_justify(&mut self) {
        self.justify_text(false);
    }

    pub fn do_full_justify(&mut self) {
        self.justify_text(true);
        self.ran_a_tool = true;
    }

    pub fn count_lines_words_and_characters(&mut self) {
        let was_current = self.current();
        let was_x = self.current_x();
        let (topline, top_x, botline, bot_x, chars);
        if self.ofile().mark.is_some() {
            let (t, tx, b, bx) = self.get_region();
            topline = t;
            top_x = tx;
            botline = b;
            bot_x = bx;
            let mut c = if topline == botline {
                0
            } else {
                self.number_of_characters_in(self.next(topline).unwrap(), botline) + 1
            };
            c += mbstrlen(&self.nodes[topline].data[top_x..])
                - mbstrlen(&self.nodes[botline].data[bot_x..]);
            chars = c;
        } else {
            topline = self.filetop();
            top_x = 0;
            botline = self.filebot();
            bot_x = self.datalen(botline);
            chars = self.ofile().totsize;
        }
        let mut lines = self.lineno(botline) - self.lineno(topline);
        lines += i64::from(!(bot_x == 0 || (topline == botline && top_x == bot_x)));
        {
            let of = self.ofile_mut();
            of.current = topline;
            of.current_x = top_x;
        }
        let mut words = 0usize;
        while self.lineno(self.current()) < self.lineno(botline)
            || (self.current() == botline && self.current_x() < bot_x)
        {
            if self.do_next_word(false) {
                words += 1;
            }
        }
        {
            let of = self.ofile_mut();
            of.current = was_current;
            of.current_x = was_x;
        }
        let sel = if self.ofile().mark.is_some() {
            "In Auswahl:  "
        } else {
            ""
        };
        let msg = fmt!(
            "%s%zd %s,  %zu %s,  %zu %s",
            sel,
            lines,
            crate::utils::plural("Zeile", "Zeilen", lines.cast_unsigned()),
            words,
            crate::utils::plural("Wort", "Wörter", words as u64),
            chars,
            "Zeichen"
        );
        self.statusline(Msg::Info, &msg);
    }

    pub fn do_verbatim_input(&mut self) {
        if self.isset(Flag::Zero)
            && self.ofile().cursor_row == i64_of(self.editwinrows) - 1
            && self.lines() > 1
        {
            self.edit_scroll(FORWARD);
            self.edit_refresh();
        }
        self.statusline(Msg::Info, "Direkte Eingabe");
        self.place_the_cursor();
        let (mut bytes, count) = self.get_verbatim_kbinput(crate::curses::Win::Mid);
        if count > 0 {
            if self.isset(Flag::ConstantShow) || self.isset(Flag::MiniBar) {
                self.lastmessage = Msg::Vacuum;
            }
            if count < 999 {
                self.inject(&mut bytes);
            }
            if self.isset(Flag::Zero) && self.currmenu == MMAIN {
                self.term
                    .wredrawln(crate::curses::Win::Mid, self.editwinrows - 1, 1);
            } else {
                self.wipe_statusbar();
            }
        } else {
            self.statusline(Msg::Ahem, "Ungültiger Code");
        }
    }

    fn copy_completion(&self, text: &[u8]) -> Vec<u8> {
        let mut length = 0;
        while length < text.len() && self.is_word_char(&text[length..], false) {
            length = step_right(text, length);
        }
        text[..length.min(text.len())].to_vec()
    }

    pub fn complete_a_word(&mut self) {
        let was_set_wrapping = self.isset(Flag::BreakLongLines);
        if self.pletion_line.is_none() {
            self.list_of_completions.clear();
            self.ofile_mut().last_action = UndoType::Other;
            self.scouring = self.cur;
            self.pletion_line = Some(self.filetop());
            self.pletion_x = 0;
            self.wipe_statusbar();
        } else {
            self.do_undo();
        }
        let cur = self.current();
        let cx = self.current_x();
        let mut start_of_shard = cx;
        while start_of_shard > 0 {
            let oneleft = step_left(self.data(cur), start_of_shard);
            if !self.is_word_char(&self.nodes[cur].data[oneleft..], false) {
                break;
            }
            start_of_shard = oneleft;
        }
        if start_of_shard == cx {
            self.statusline(Msg::Ahem, "Kein Wort-Fragment");
            self.pletion_line = None;
            return;
        }
        let shard: Vec<u8> = self.nodes[cur].data[start_of_shard..cx].to_vec();
        let shard_length = shard.len();
        while let Some(pl) = self.pletion_line {
            let pdata = self.nodes[pl].data.clone();
            let threshold = pdata.len().cast_signed() - shard_length.cast_signed();
            let mut i = self.pletion_x;
            while (i.cast_signed()) < threshold {
                if pdata[i] != shard[0] {
                    i += 1;
                    continue;
                }
                let mut j = 1;
                while j < shard_length && pdata[i + j] == shard[j] {
                    j += 1;
                }
                if j < shard_length {
                    i += 1;
                    continue;
                }
                if !self.is_word_char(&pdata[i + j..], false) {
                    i += 1;
                    continue;
                }
                if i > 0 && self.is_word_char(&pdata[step_left(&pdata, i)..], false) {
                    i += 1;
                    continue;
                }
                if pl == cur && self.scouring == self.cur && i == cx - shard_length {
                    i += 1;
                    continue;
                }
                let completion = self.copy_completion(&pdata[i..]);
                if self
                    .list_of_completions
                    .iter()
                    .any(|c| c.word == completion)
                {
                    i += 1;
                    continue;
                }
                self.list_of_completions
                    .push(crate::definitions::Completion {
                        word: completion.clone(),
                    });
                self.flags.unset(Flag::BreakLongLines);
                let mut rest: Vec<u8> = completion[shard_length..].to_vec();
                self.inject(&mut rest);
                if was_set_wrapping {
                    self.flags.set(Flag::BreakLongLines);
                    self.do_wrap();
                }
                self.pletion_x = i + 1;
                return;
            }
            self.pletion_line = self.next(pl);
            self.pletion_x = 0;
            if self.pletion_line.is_none() && self.next_buffer_index(self.scouring) != self.cur {
                self.scouring = self.next_buffer_index(self.scouring);
                self.pletion_line = Some(self.files[self.scouring].filetop);
            }
        }
        if self.list_of_completions.is_empty() {
            self.statusline(Msg::Ahem, "Keine Treffer");
        } else {
            self.edit_refresh();
            self.statusline(Msg::Ahem, "Keine weiteren Treffer");
        }
    }
}

impl Zu {
    pub fn current_undo(&self) -> Option<&Undo> {
        let of = self.ofile();
        if of.cur_undo == 0 {
            None
        } else {
            Some(&of.undo[of.cur_undo - 1])
        }
    }

    pub fn current_undo_mut(&mut self) -> Option<&mut Undo> {
        let of = self.ofile_mut();
        if of.cur_undo == 0 {
            None
        } else {
            let i = of.cur_undo - 1;
            Some(&mut of.undo[i])
        }
    }
}

pub fn indent_length(line: &[u8]) -> usize {
    let mut i = 0;
    while i < line.len() && is_blank_char(&line[i..]) {
        i += char_length(&line[i..]);
    }
    i.min(line.len())
}
