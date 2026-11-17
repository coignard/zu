#![expect(
    clippy::wrong_self_convention,
    reason = "keeps the GNU nano names to_first_file() and to_last_file()"
)]

use crate::files::diralphasort;
use crate::utils::usize_of;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;

use crate::chars::mbstrcasestr;
use crate::curses::Win;
use crate::definitions::{
    BACKWARD, BLIND, END_OF_PASTE, Elem, FORWARD, Flag, Func, HistKind, KEY_ENTER, KEY_MOUSE,
    MBROWSER, MGOTODIR, MWHEREISFILE, Msg, PRUNE_DUPLICATE, START_OF_PASTE, THE_WINDOW_RESIZED,
};
use crate::global::Zu;
use crate::search::RefreshFn;
use crate::utils::{bytes_to_path, errno, io_errstr, path_to_bytes, strerror, tail};

impl Zu {
    fn read_the_list(&mut self, path: &[u8], entries: Vec<Vec<u8>>) {
        let mut widest = 0usize;
        for e in &entries {
            widest = widest.max(self.breadth(e));
        }
        self.gauge = widest + 10;
        if self.gauge < 15 {
            self.gauge = 15;
        }
        if self.gauge > self.cols() {
            self.gauge = self.cols();
        }
        let mut list: Vec<Vec<u8>> = Vec::new();
        for e in entries {
            if e == b"." {
                continue;
            }
            let mut full = path.to_vec();
            full.extend_from_slice(&e);
            list.push(full);
        }
        diralphasort(&mut list);
        self.filelist = list;
        self.piles = (self.cols() + 2) / (self.gauge + 2);
        self.usable_rows =
            self.editwinrows - usize::from(self.isset(Flag::Zero) && self.lines() > 1);
    }

    fn reselect(&mut self, name: &[u8]) {
        let len = self.filelist.len();
        match self.filelist.iter().position(|f| f == name) {
            Some(i) => self.selected = i,
            None => {
                if self.selected > len {
                    self.selected = len.saturating_sub(1);
                } else {
                    self.selected = self.selected.saturating_sub(1);
                }
            }
        }
    }

    pub fn browser_refresh(&mut self) {
        let mut row = 0usize;
        let mut col = 0usize;
        let mut the_row = 0usize;
        let mut the_column = 0usize;
        let pp = self.present_path.clone();
        self.titlebar(Some(&pp));
        self.blank_edit();
        let cols = self.cols();
        let gauge = self.gauge;
        let page = self.usable_rows * self.piles.max(1);
        let start = self.selected - self.selected % page.max(1);
        let mut index = start;
        while index < self.filelist.len() && row < self.usable_rows {
            let full = self.filelist[index].clone();
            let thename = tail(&full).to_vec();
            let namelen = self.breadth(&thename);
            let mut infomaxlen = 7usize;
            let dots = cols >= 15 && namelen >= gauge.saturating_sub(infomaxlen);
            let disp = self.display_string(
                &thename,
                if dots {
                    namelen + infomaxlen + 4 - gauge
                } else {
                    0
                },
                gauge,
                false,
                false,
            );
            let st = self.interface_color_pair[Elem::SelectedText as usize];
            if index == self.selected {
                self.term.wattron(Win::Mid, st);
                self.term.wmove(Win::Mid, row, col);
                self.term.waddstr(Win::Mid, &" ".repeat(gauge));
                the_row = row;
                the_column = col;
            }
            if dots {
                self.term.mvwaddstr(Win::Mid, row, col, "...");
            }
            self.term
                .mvwaddbytes(Win::Mid, row, if dots { col + 3 } else { col }, &disp);
            col += gauge;
            let info: Vec<u8> = match fs::symlink_metadata(bytes_to_path(&full)) {
                Err(_) => b"--".to_vec(),
                Ok(md) if md.file_type().is_symlink() => match fs::metadata(bytes_to_path(&full)) {
                    Ok(t) if t.is_dir() => b"(Verz.)".to_vec(),
                    _ => b"--".to_vec(),
                },
                Ok(md) if md.is_dir() => {
                    if thename == b".." {
                        infomaxlen = 12;
                        "(höher)".as_bytes().to_vec()
                    } else {
                        b"(Verz.)".to_vec()
                    }
                }
                Ok(md) => {
                    let size = md.size();
                    let (result, modifier) = if size < (1 << 10) {
                        (size, ' ')
                    } else if size < (1 << 20) {
                        (size >> 10, 'K')
                    } else if size < (1 << 30) {
                        (size >> 20, 'M')
                    } else {
                        (size >> 30, 'G')
                    };
                    if result < (1 << 10) {
                        format!("{result:>4} {modifier}B").into_bytes()
                    } else {
                        b"riesig".to_vec()
                    }
                }
            };
            let mut infolen = self.breadth(&info);
            let mut info = info;
            if infolen > infomaxlen {
                let n = self.actual_x(&info, infomaxlen);
                info.truncate(n);
                infolen = infomaxlen;
            }
            self.term.mvwaddbytes(Win::Mid, row, col - infolen, &info);
            if index == self.selected {
                self.term.wattroff(Win::Mid, st);
            }
            col += 2;
            if col > cols.saturating_sub(gauge) {
                row += 1;
                col = 0;
            }
            index += 1;
        }
        if self.isset(Flag::ShowCursor) {
            self.term.wmove(Win::Mid, the_row, the_column);
            self.term.curs_set(true);
        }
        self.term.wnoutrefresh(Win::Mid);
    }

    fn findfile(&mut self, needle: &[u8], forwards: bool) {
        let began_at = self.selected;
        let len = self.filelist.len();
        loop {
            if forwards {
                if self.selected + 1 >= len {
                    self.selected = 0;
                    self.statusbar("Suche umbrechen");
                } else {
                    self.selected += 1;
                }
            } else if self.selected == 0 {
                self.selected = len - 1;
                self.statusbar("Suche umbrechen");
            } else {
                self.selected -= 1;
            }
            if mbstrcasestr(tail(&self.filelist[self.selected]), needle).is_some() {
                if self.selected == began_at {
                    self.statusbar("Das ist das einzige Auftreten");
                }
                return;
            }
            if self.selected == began_at {
                self.not_found_msg(needle);
                return;
            }
        }
    }

    fn search_filename(&mut self, forwards: bool) {
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
        let mut msg = b"Suche".to_vec();
        if !forwards {
            msg.extend_from_slice(" [Rückwärts]".as_bytes());
        }
        msg.extend_from_slice(&thedefault);
        let response = self.do_prompt(
            MWHEREISFILE,
            b"",
            Some(HistKind::Search),
            RefreshFn::Browser,
            &msg,
        );
        if response == -1 || (response == -2 && self.last_search.is_empty()) {
            self.statusbar("Abgebrochen");
            return;
        }
        if !self.answer.is_empty() {
            self.last_search = self.answer.clone();
            let a = self.answer.clone();
            self.update_history(HistKind::Search, &a, PRUNE_DUPLICATE);
        }
        if response == 0 || response == -2 {
            let ls = self.last_search.clone();
            self.findfile(&ls, forwards);
        }
    }

    fn research_filename(&mut self, forwards: bool) {
        if self.last_search.is_empty() && self.search_history.items.len() > 1 {
            let i = self.search_history.items.len() - 2;
            self.last_search = self.search_history.items[i].clone();
        }
        if self.last_search.is_empty() {
            self.statusbar("Kein aktuelles Suchmuster");
        } else {
            self.wipe_statusbar();
            let ls = self.last_search.clone();
            self.findfile(&ls, forwards);
        }
    }

    pub const fn to_first_file(&mut self) {
        self.selected = 0;
    }

    pub const fn to_last_file(&mut self) {
        self.selected = self.filelist.len().saturating_sub(1);
    }

    fn strip_last_component(path: &[u8]) -> Vec<u8> {
        path.iter()
            .rposition(|&c| c == b'/')
            .map_or_else(|| path.to_vec(), |i| path[..i].to_vec())
    }

    fn browse(&mut self, mut path: Vec<u8>) -> Option<Vec<u8>> {
        let mut present_name: Option<Vec<u8>> = None;
        let mut chosen: Option<Vec<u8>> = None;
        'read_directory_contents: loop {
            let full = self.get_full_path(&path);
            let entries: Option<Vec<Vec<u8>>> = full.as_ref().and_then(|p| {
                fs::read_dir(bytes_to_path(p)).ok().map(|rd| {
                    let mut v: Vec<Vec<u8>> = rd
                        .flatten()
                        .map(|e| e.file_name().as_bytes().to_vec())
                        .collect();
                    v.push(b"..".to_vec());
                    v
                })
            });
            if let (Some(p), Some(e)) = (full, entries) {
                path.clone_from(&p);
                self.read_the_list(&p, e);
            } else {
                let m = fmt!(
                    "Verzeichnis kann nicht geöffnet werden: %s",
                    &strerror(errno())
                );
                self.statusline(Msg::Alert, &m);
                if self.filelist.is_empty() {
                    self.lastmessage = Msg::Vacuum;
                    self.term.napms(1200);
                    return None;
                }
                path = self.present_path.clone();
                present_name = Some(self.filelist[self.selected].clone());
            }
            self.resized_for_browser = false;
            if let Some(pn) = present_name.take() {
                self.reselect(&pn);
            } else {
                self.selected = 0;
            }
            let mut old_selected = usize::MAX;
            self.present_path.clone_from(&path);
            self.titlebar(Some(&path));
            if self.filelist.is_empty() {
                self.statusline(Msg::Alert, "Keine Einträge");
                self.term.napms(1200);
                break;
            }
            loop {
                self.lastmessage = Msg::Vacuum;
                self.bottombars(MBROWSER);
                if old_selected != self.selected || self.isset(Flag::ShowCursor) {
                    self.browser_refresh();
                }
                old_selected = self.selected;
                let mut kbinput = self.get_kbinput(Win::Mid, self.isset(Flag::ShowCursor));
                if kbinput == KEY_MOUSE {
                    let (r, mut y, x) = self.get_mouseinput();
                    if r == 0 && self.wmouse_trafo(Win::Mid, &mut y, x) {
                        let page = self.usable_rows * self.piles;
                        self.selected = self.selected - self.selected % page.max(1)
                            + (usize_of(i64::from(y)) * self.piles)
                            + (usize_of(i64::from(x)) / (self.gauge + 2));
                        if usize_of(i64::from(x)) > self.piles * (self.gauge + 2) {
                            self.selected = self.selected.saturating_sub(1);
                        }
                        if self.selected > self.filelist.len() - 1 {
                            self.selected = self.filelist.len() - 1;
                        }
                        if old_selected == self.selected {
                            kbinput = KEY_ENTER;
                        }
                    }
                    if kbinput == KEY_MOUSE {
                        continue;
                    }
                }
                let function = self.interpret(kbinput);
                let len = self.filelist.len();
                let piles = self.piles.max(1);
                let page = self.usable_rows * piles;
                match function {
                    Some(Func::Help) => self.do_help(),
                    Some(Func::FullRefresh) => {
                        kbinput = THE_WINDOW_RESIZED;
                    }
                    Some(Func::Toggle)
                        if self.get_shortcut(kbinput).and_then(|s| s.toggle)
                            == Some(Flag::NoHelp) =>
                    {
                        self.flags.toggle(Flag::NoHelp);
                        self.window_init();
                        kbinput = THE_WINDOW_RESIZED;
                    }
                    Some(Func::SearchBackward) => self.search_filename(BACKWARD),
                    Some(Func::SearchForward) => self.search_filename(FORWARD),
                    Some(Func::FindPrevious) => self.research_filename(BACKWARD),
                    Some(Func::FindNext) => self.research_filename(FORWARD),
                    Some(Func::Left) => {
                        if self.selected > 0 {
                            self.selected -= 1;
                        }
                    }
                    Some(Func::Right) => {
                        if self.selected < len - 1 {
                            self.selected += 1;
                        }
                    }
                    Some(Func::PrevWord) => self.selected -= self.selected % piles,
                    Some(Func::NextWord) => {
                        self.selected += piles - 1 - (self.selected % piles);
                        if self.selected >= len {
                            self.selected = len - 1;
                        }
                    }
                    Some(Func::Up) => {
                        if self.selected >= piles {
                            self.selected -= piles;
                        }
                    }
                    Some(Func::Down) => {
                        if self.selected + piles < len {
                            self.selected += piles;
                        }
                    }
                    Some(Func::PrevBlock) => {
                        self.selected =
                            (self.selected / page.max(1)) * page + self.selected % piles;
                    }
                    Some(Func::NextBlock) => {
                        self.selected =
                            (self.selected / page.max(1)) * page + self.selected % piles + page
                                - piles;
                        if self.selected >= len {
                            self.selected = (len / piles) * piles + self.selected % piles;
                        }
                        if self.selected >= len {
                            self.selected -= piles;
                        }
                    }
                    Some(Func::PageUp) => {
                        if self.selected < piles {
                            self.selected = 0;
                        } else if self.selected < page {
                            self.selected %= piles;
                        } else {
                            self.selected -= page;
                        }
                    }
                    Some(Func::PageDown) => {
                        if self.selected + piles >= len - 1 {
                            self.selected = len - 1;
                        } else if self.selected + page >= len {
                            self.selected = (self.selected + page - len) % piles + len - piles;
                        } else {
                            self.selected += page;
                        }
                    }
                    Some(Func::FirstFile) => self.to_first_file(),
                    Some(Func::LastFile) => self.to_last_file(),
                    Some(Func::GotoDir) => {
                        if self.do_prompt(
                            MGOTODIR,
                            b"",
                            None,
                            RefreshFn::Browser,
                            b"Gehe zu Verzeichnis",
                        ) < 0
                        {
                            self.statusbar("Abgebrochen");
                        } else {
                            let a = self.answer.clone();
                            let mut p = self.expand_leading_tilde(&a);
                            if p.first() != Some(&b'/') {
                                let mut np = self.present_path.clone();
                                np.extend_from_slice(&a);
                                p = np;
                            }
                            if self.operating_dir.is_some()
                                && self.outside_of_confinement(&p, false)
                            {
                                let od = self.operating_dir.clone().unwrap();
                                let m =
                                    fmtb!("Außerhalb von %s kann nicht geschrieben werden", &od);
                                self.statusline_bytes(Msg::Alert, &m);
                            } else {
                                while p.len() > 1 && p.last() == Some(&b'/') {
                                    p.pop();
                                }
                                if let Some(j) = self.filelist.iter().position(|f| *f == p) {
                                    self.selected = j;
                                }
                                path = p;
                                continue 'read_directory_contents;
                            }
                        }
                    }
                    Some(Func::Enter) => {
                        let sel = self.filelist[self.selected].clone();
                        if sel == b"/.." {
                            self.statusline(
                                Msg::Alert,
                                "Ins übergeordnete Verzeichnis kann nicht gewechselt werden",
                            );
                            continue;
                        }
                        if self.operating_dir.is_some() && self.outside_of_confinement(&sel, false)
                        {
                            let od = self.operating_dir.clone().unwrap();
                            let m = fmtb!("Außerhalb von %s kann nicht geschrieben werden", &od);
                            self.statusline_bytes(Msg::Alert, &m);
                            continue;
                        }
                        let md = match fs::metadata(bytes_to_path(&sel)) {
                            Ok(m) => m,
                            Err(e) => {
                                let m = fmtb!("Fehler beim Lesen von %s: %s", &sel, &io_errstr(&e));
                                self.statusline_bytes(Msg::Alert, &m);
                                continue;
                            }
                        };
                        if !md.is_dir() {
                            chosen = Some(sel);
                            break 'read_directory_contents;
                        }
                        if tail(&sel) == b".." {
                            present_name = Some(Self::strip_last_component(&sel));
                        }
                        path = sel;
                        continue 'read_directory_contents;
                    }
                    Some(Func::Implant) => {
                        if let Some(exp) = self
                            .first_sc_for(MBROWSER, Func::Implant)
                            .and_then(|s| s.expansion.clone())
                        {
                            self.implant(&exp);
                        }
                    }
                    _ => {
                        if kbinput == START_OF_PASTE {
                            while self.get_kbinput(Win::Mid, BLIND) != END_OF_PASTE {}
                            self.statusline(Msg::Ahem, "Einfügen wird ignoriert");
                        } else if kbinput == THE_WINDOW_RESIZED {
                        } else if function == Some(Func::Exit) {
                            break 'read_directory_contents;
                        } else {
                            self.unbound_key(kbinput);
                        }
                    }
                }
                if kbinput == THE_WINDOW_RESIZED || self.resized_for_browser {
                    present_name = Some(self.filelist[self.selected].clone());
                    continue 'read_directory_contents;
                }
            }
        }
        self.titlebar(None);
        self.edit_refresh();
        self.filelist.clear();
        chosen
    }

    pub fn browse_in(&mut self, inpath: &[u8]) -> Option<Vec<u8>> {
        let mut path = self.expand_leading_tilde(inpath);
        let is_dir = |p: &[u8]| fs::metadata(bytes_to_path(p)).is_ok_and(|m| m.is_dir());
        if !is_dir(&path) {
            path = Self::strip_last_component(&path);
            if !is_dir(&path) {
                if let Ok(p) = fs::canonicalize(".") {
                    path = path_to_bytes(&p);
                } else {
                    self.statusline(
                        Msg::Alert,
                        "Das Arbeitsverzeichnis ist nicht mehr vorhanden",
                    );
                    self.term.napms(1200);
                    return None;
                }
            }
        }
        if self.operating_dir.is_some() && self.outside_of_confinement(&path, false) {
            path = self.operating_dir.clone().unwrap();
        }
        self.browse(path)
    }
}
