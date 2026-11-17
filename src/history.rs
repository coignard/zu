use crate::utils::{i64_of, isize_of};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

use crate::chars::revstrstr;
use crate::definitions::{Flag, HistKind, MAX_SEARCH_HISTORY, Position};
use crate::global::Zu;
use crate::utils::{bytes_to_path, concatenate, io_errstr, recode_lf_to_nul, recode_nul_to_lf};

const POSITION_HISTORY: &[u8] = b"filepos_history";

impl Zu {
    const fn hist(&mut self, kind: HistKind) -> &mut crate::definitions::History {
        match kind {
            HistKind::Search => &mut self.search_history,
            HistKind::Replace => &mut self.replace_history,
            HistKind::Execute => &mut self.execute_history,
        }
    }

    pub fn update_history(&mut self, kind: HistKind, text: &[u8], avoid_duplicates: bool) {
        let h = self.hist(kind);
        if avoid_duplicates {
            let bottom = h.bottom();
            if let Some(i) = (0..bottom).rev().find(|&i| h.items[i] == text) {
                h.items.remove(i);
            }
        }
        while i64_of(h.items.len()) > MAX_SEARCH_HISTORY {
            h.items.remove(0);
        }
        let bottom = h.bottom();
        h.items[bottom] = text.to_vec();
        h.items.push(Vec::new());
        h.reset();
        self.history_changed = true;
    }

    pub fn get_history_completion(
        &mut self,
        kind: HistKind,
        string: Vec<u8>,
        len: usize,
    ) -> Vec<u8> {
        let h = self.hist(kind);
        let frag = &string[..len.min(string.len())];
        let start = h.pos;
        let mut i = start;
        while i > 0 {
            i -= 1;
            if h.items[i].starts_with(frag) && h.items[i] != string {
                h.pos = i;
                return h.items[i].clone();
            }
        }
        let mut i = h.bottom();
        while i > start {
            i -= 1;
            if i <= start {
                break;
            }
            if h.items[i].starts_with(frag) && h.items[i] != string {
                h.pos = i;
                return h.items[i].clone();
            }
        }
        string
    }

    pub fn have_statedir(&mut self) -> bool {
        self.get_homedir();
        if let Some(h) = self.homedir.clone() {
            let sd = concatenate(&h, b"/.zu/");
            if fs::metadata(bytes_to_path(&sd)).is_ok_and(|m| m.is_dir()) {
                self.registername = Some(concatenate(&sd, POSITION_HISTORY));
                self.statedir = Some(sd);
                return true;
            }
        }
        let xdg = std::env::var_os("XDG_DATA_HOME").map(|v| {
            use std::os::unix::ffi::OsStrExt;
            v.as_bytes().to_vec()
        });
        if self.homedir.is_none() && xdg.is_none() {
            return false;
        }
        let sd = match &xdg {
            Some(x) => concatenate(x, b"/zu/"),
            None => concatenate(self.homedir.as_ref().unwrap(), b"/.local/share/zu/"),
        };
        match fs::metadata(bytes_to_path(&sd)) {
            Err(_) => {
                if xdg.is_none() {
                    let h = self.homedir.clone().unwrap();
                    let _ = fs::DirBuilder::new()
                        .mode(0o700)
                        .create(bytes_to_path(&concatenate(&h, b"/.local")));
                    let _ = fs::DirBuilder::new()
                        .mode(0o700)
                        .create(bytes_to_path(&concatenate(&h, b"/.local/share")));
                }
                if let Err(e) = fs::DirBuilder::new().mode(0o700).create(bytes_to_path(&sd)) {
                    let m = fmtb!(
                        "Verzeichnis %s kann nicht angelegt werden: %s\nEs wird für die Cursorpositionen gebraucht.\n",
                        &sd,
                        &io_errstr(&e),
                    );
                    self.jot_error(&String::from_utf8_lossy(&m));
                    return false;
                }
            }
            Ok(md) if !md.is_dir() => {
                let m = fmtb!(
                    "Pfad %s ist kein Verzeichnis, obwohl er eines sein muss.\nCursorpositionen werden nicht gespeichert.\n",
                    &sd,
                );
                self.jot_error(&String::from_utf8_lossy(&m));
                return false;
            }
            _ => {}
        }
        self.registername = Some(concatenate(&sd, POSITION_HISTORY));
        self.statedir = Some(sd);
        true
    }

    fn stringify_anchors(&self) -> Vec<u8> {
        let mut s = Vec::new();
        let mut line = Some(self.filetop());
        while let Some(l) = line {
            if self.nodes[l].has_anchor {
                s.extend_from_slice(format!("{} ", self.lineno(l)).as_bytes());
            }
            line = self.next(l);
        }
        s
    }

    fn restore_anchors(&mut self, string: &[u8]) {
        let mut line = self.filetop();
        for num in String::from_utf8_lossy(string).split(' ') {
            let number: i64 = match num.parse() {
                Ok(n) => n,
                Err(_) => continue,
            };
            while self.lineno(line) < number {
                match self.next(line) {
                    Some(n) => line = n,
                    None => return,
                }
            }
            self.nodes[line].has_anchor = true;
        }
    }

    pub fn load_positions_register(&mut self) {
        let Some(rn) = self.registername.clone() else {
            return;
        };
        let content = match fs::read(bytes_to_path(&rn)) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                let m = fmtb!("Fehler beim Lesen von %s: %s", &rn, &io_errstr(&e));
                self.jot_error(&String::from_utf8_lossy(&m));
                self.flags.unset(Flag::PositionLog);
                return;
            }
        };
        let mut count = 0;
        for phrase in content.split(|&b| b == b'\n') {
            if phrase.len() < 2 || count >= 200 {
                continue;
            }
            count += 1;
            let slash = phrase.iter().position(|&b| b == b'/').unwrap_or(0);
            let anchors = phrase[..slash].to_vec();
            let mut stanza = phrase[slash..].to_vec();
            recode_nul_to_lf(&mut stanza);
            let Some(columnptr) = revstrstr(&stanza, b" ", stanza.len().saturating_sub(3)) else {
                continue;
            };
            let Some(lineptr) = revstrstr(&stanza, b" ", columnptr.saturating_sub(2)) else {
                continue;
            };
            let filename = stanza[..lineptr].to_vec();
            let linenumber = String::from_utf8_lossy(&stanza[lineptr + 1..columnptr])
                .trim()
                .parse()
                .unwrap_or(1);
            let columnnumber = String::from_utf8_lossy(&stanza[columnptr + 1..])
                .trim()
                .parse()
                .unwrap_or(1);
            self.positions_register.push(Position {
                filename,
                linenumber,
                columnnumber,
                anchors,
            });
        }
        if let Some(si) = crate::files::stat_info(&rn) {
            self.latest_timestamp = si.mtime;
        }
    }

    fn save_positions_register(&mut self) {
        let Some(rn) = self.registername.clone() else {
            return;
        };
        let mut out: Vec<u8> = Vec::new();
        for item in self.positions_register.iter().take(200) {
            out.extend_from_slice(&item.anchors);
            let mut line = item.filename.clone();
            line.extend_from_slice(
                format!(" {} {}", item.linenumber, item.columnnumber).as_bytes(),
            );
            recode_lf_to_nul(&mut line);
            out.extend_from_slice(&line);
            out.push(b'\n');
        }
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(bytes_to_path(&rn))
        {
            Ok(f) => f,
            Err(e) => {
                let m = fmtb!("Fehler beim Schreiben von %s: %s", &rn, &io_errstr(&e));
                self.jot_error(&String::from_utf8_lossy(&m));
                return;
            }
        };
        if let Err(e) = file.set_permissions(fs::Permissions::from_mode(0o600)) {
            let m = fmtb!(
                "Zugriffsrechte auf %s können nicht begrenzt werden: %s",
                &rn,
                &io_errstr(&e)
            );
            self.jot_error(&String::from_utf8_lossy(&m));
        }
        if let Err(e) = file.write_all(&out).and_then(|()| file.flush()) {
            let m = fmtb!("Fehler beim Schreiben von %s: %s", &rn, &io_errstr(&e));
            self.jot_error(&String::from_utf8_lossy(&m));
        }
        drop(file);
        if let Some(si) = crate::files::stat_info(&rn) {
            self.latest_timestamp = si.mtime;
        }
    }

    fn reload_positions_if_needed(&mut self) {
        let Some(rn) = self.registername.clone() else {
            return;
        };
        match crate::files::stat_info(&rn) {
            Some(si) if si.mtime != self.latest_timestamp => {
                self.positions_register.clear();
                self.load_positions_register();
            }
            _ => {}
        }
    }

    pub fn update_positions_register(&mut self) {
        let fname = self.ofile().filename.clone();
        let Some(fullpath) = self.get_full_path(&fname) else {
            return;
        };
        self.reload_positions_if_needed();
        let mut item = match self
            .positions_register
            .iter()
            .position(|p| p.filename == fullpath)
        {
            Some(i) => self.positions_register.remove(i),
            None => Position {
                filename: fullpath,
                linenumber: 1,
                columnnumber: 1,
                anchors: Vec::new(),
            },
        };
        item.linenumber = self.lineno(self.current());
        item.columnnumber = i64_of(self.xplustabs()) + 1;
        item.anchors = self.stringify_anchors();
        self.positions_register.insert(0, item);
        self.save_positions_register();
    }

    pub fn restore_cursor_position_if_any(&mut self) {
        let fname = self.ofile().filename.clone();
        let Some(fullpath) = self.get_full_path(&fname) else {
            return;
        };
        self.reload_positions_if_needed();
        let item = self
            .positions_register
            .iter()
            .find(|p| p.filename == fullpath)
            .cloned();
        if let Some(item) = item {
            if !item.anchors.is_empty() {
                self.restore_anchors(&item.anchors);
            }
            self.goto_line_and_column(isize_of(item.linenumber), isize_of(item.columnnumber), true);
        }
    }
}
