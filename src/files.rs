use crate::utils::{u8_of, usize_of};
use std::ffi::CStr;
use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::time::Duration;

use zeroize::{Zeroize, Zeroizing};

use crate::chars::{char_length, mbstrcasecmp, revstrstr};
use crate::crypto::{self, CryptoError};
use crate::crypto::{KeyMaterial, PlainOrigin};
use crate::curses::Win;
use crate::definitions::{
    CANCEL, Flag, Format, Func, HistKind, Lp, MEXECUTE, MGOTODIR, MINSERTFILE, MMOST, MWRITEFILE,
    Msg, NO, NONOTES, OpenFile, PRUNE_DUPLICATE, StatInfo, UndoType, VERSION, Writing, YES,
    YESORNO,
};
use crate::global::Zu;
use crate::search::RefreshFn;
use crate::utils::{
    bytes_to_path, errno, io_errstr, path_to_bytes, recode_nul_to_lf, strerror, tail,
};

const LOCKSIZE: usize = 1024;

enum LockResult {
    Skip,
    Failed,
    Name(Vec<u8>),
}

pub fn stat_info(path: &[u8]) -> Option<StatInfo> {
    let md = fs::metadata(bytes_to_path(path)).ok()?;
    Some(StatInfo {
        dev: md.dev(),
        ino: md.ino(),
        mtime: md.mtime(),
        mode: md.mode(),
        uid: md.uid(),
        gid: md.gid(),
    })
}

fn path_access(path: &std::path::Path, mode: libc::c_int) -> bool {
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .is_ok_and(|c| unsafe { libc::access(c.as_ptr(), mode) } == 0)
}

fn is_dir_path(path: &[u8]) -> bool {
    fs::metadata(bytes_to_path(path)).is_ok_and(|m| m.is_dir())
}

type Opened = (Zeroizing<Vec<u8>>, Option<KeyMaterial>);

impl Zu {
    pub fn make_new_buffer(&mut self) {
        let filetop = self.make_new_node(None);
        self.nodes[filetop].data = Vec::new();
        let of = OpenFile {
            filename: Vec::new(),
            filetop,
            filebot: filetop,
            edittop: filetop,
            current: filetop,
            totsize: 0,
            firstcolumn: 0,
            current_x: 0,
            placewewant: 0,
            brink: 0,
            cursor_row: 0,
            statinfo: None,
            spillage_line: None,
            mark: None,
            mark_x: 0,
            softmark: false,
            fmt: Format::Unspecified,
            lock_filename: None,
            undo: Vec::new(),
            cur_undo: 0,
            last_saved: None,
            last_action: UndoType::Other,
            modified: false,
            errormessage: None,
            keymat: None,
            plain_origin: None,
        };
        if self.files.is_empty() {
            self.files.push(of);
            self.cur = 0;
        } else {
            let idx = self.cur + 1;
            self.files.insert(idx, of);
            self.cur = idx;
            self.set_exit_tag(crate::global::CLOSE_TAG);
            self.more_than_one = !self.inhelp || self.more_than_one;
        }
    }

    pub fn crop_to_fit(&mut self, name: &[u8], room: usize) -> Vec<u8> {
        if self.breadth(name) <= room {
            return self.display_string(name, 0, room, false, false);
        }
        if room < 4 {
            return b"_".to_vec();
        }
        let b = self.breadth(name);
        let clipped = self.display_string(name, b - room + 3, room, false, false);
        let mut v = b"...".to_vec();
        v.extend_from_slice(&clipped);
        v
    }

    pub fn delete_lockfile(&mut self, lockfilename: &[u8]) -> bool {
        match fs::remove_file(bytes_to_path(lockfilename)) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Err(e) => {
                let m = fmtb!(
                    "Fehler beim Löschen der Sperrdatei %s: %s",
                    lockfilename,
                    &io_errstr(&e)
                );
                self.statusline_bytes(Msg::Mild, &m);
                false
            }
        }
    }

    pub fn write_lockfile(&mut self, lockfilename: &[u8], filename: &[u8], modified: bool) -> bool {
        let mypid = std::process::id();
        let (user, host) = unsafe {
            let pw = libc::getpwuid(libc::geteuid());
            if pw.is_null() {
                self.statusline(
                    Msg::Mild,
                    "Meine Identität für die Sperrdatei konnte nicht bestimmt werden",
                );
                return false;
            }
            let user = CStr::from_ptr((*pw).pw_name).to_bytes().to_vec();
            let mut buf = [0i8; 32];
            let r = libc::gethostname(buf.as_mut_ptr().cast::<libc::c_char>(), 31);
            if r < 0 && errno() != libc::ENAMETOOLONG {
                let m = fmt!(
                    "Rechnername konnte nicht bestimmt werden: %s",
                    &strerror(errno())
                );
                self.statusline(Msg::Mild, &m);
                return false;
            }
            buf[31] = 0;
            let host = CStr::from_ptr(buf.as_ptr().cast::<libc::c_char>())
                .to_bytes()
                .to_vec();
            (user, host)
        };
        if !self.delete_lockfile(lockfilename) {
            return false;
        }
        let mut lockdata = vec![0u8; LOCKSIZE];
        lockdata[0] = 0x62;
        lockdata[1] = 0x30;
        let prog = format!("zu {VERSION}");
        let pb = prog.as_bytes();
        lockdata[2..2 + pb.len().min(10)].copy_from_slice(&pb[..pb.len().min(10)]);
        lockdata[24] = u8_of(i64::from(mypid % 256));
        lockdata[25] = u8_of(i64::from((mypid / 256) % 256));
        lockdata[26] = u8_of(i64::from((mypid / (256 * 256)) % 256));
        lockdata[27] = (mypid / (256 * 256 * 256)) as u8;
        let n = user.len().min(16);
        lockdata[28..28 + n].copy_from_slice(&user[..n]);
        let n = host.len().min(32);
        lockdata[68..68 + n].copy_from_slice(&host[..n]);
        let n = filename.len().min(768);
        lockdata[108..108 + n].copy_from_slice(&filename[..n]);
        lockdata[1007] = if modified { 0x55 } else { 0 };
        let r = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o666)
            .open(bytes_to_path(lockfilename))
            .and_then(|mut f| f.write_all(&lockdata).and_then(|()| f.flush()));
        if let Err(e) = r {
            let m = fmtb!(
                "Fehler beim Schreiben der Sperrdatei %s: %s",
                lockfilename,
                &io_errstr(&e)
            );
            self.statusline_bytes(Msg::Mild, &m);
            return false;
        }
        true
    }

    fn do_lockfile(&mut self, filename: &[u8], ask_the_user: bool) -> LockResult {
        let (dir, base) = filename.iter().rposition(|&c| c == b'/').map_or_else(
            || (b".".to_vec(), filename.to_vec()),
            |i| (filename[..i.max(1)].to_vec(), filename[i + 1..].to_vec()),
        );
        let mut lockfilename = dir;
        lockfilename.push(b'/');
        lockfilename.push(b'.');
        lockfilename.extend_from_slice(&base);
        lockfilename.extend_from_slice(b".swp");
        let exists = fs::metadata(bytes_to_path(&lockfilename)).is_ok();
        if !ask_the_user && exists {
            self.blank_bottombars();
            self.statusline(
                Msg::Alert,
                "Die Datei wird auch von jemand anderem bearbeitet",
            );
            self.term.napms(1200);
        } else if exists {
            let mut lockbuf = Vec::new();
            if let Err(e) = fs::File::open(bytes_to_path(&lockfilename))
                .and_then(|f| f.take(LOCKSIZE as u64).read_to_end(&mut lockbuf))
            {
                let m = fmtb!(
                    "Fehler beim Öffnen der Sperrdatei %s: %s",
                    &lockfilename,
                    &io_errstr(&e)
                );
                self.statusline_bytes(Msg::Alert, &m);
                return LockResult::Failed;
            }
            if lockbuf.len() < 68 || lockbuf[0] != 0x62 || lockbuf[1] != 0x30 {
                let m = fmtb!("Fehlerhafte Sperrdatei wird ignoriert: %s", &lockfilename);
                self.statusline_bytes(Msg::Alert, &m);
                return LockResult::Failed;
            }
            let lockprog: Vec<u8> = lockbuf[2..12]
                .iter()
                .take_while(|&&c| c != 0)
                .copied()
                .collect();
            let lockpid = ((u64::from(lockbuf[27]) * 256 + u64::from(lockbuf[26])) * 256
                + u64::from(lockbuf[25]))
                * 256
                + u64::from(lockbuf[24]);
            let lockuser: Vec<u8> = lockbuf[28..44]
                .iter()
                .take_while(|&&c| c != 0)
                .copied()
                .collect();
            let pidstring = lockpid.to_string();
            self.as_an_at = false;
            let question = "Datei %s wird von %s bearbeitet (mit %s, Prozess-ID %s); fortsetzen?";
            let room = ((self.cols().cast_signed()
                - self.breadth(question.as_bytes()).cast_signed()
                - self.breadth(&lockuser).cast_signed()
                - self.breadth(&lockprog).cast_signed()
                - self.breadth(pidstring.as_bytes()).cast_signed()
                + 7)
            .max(0))
            .cast_unsigned();
            let postedname = self.crop_to_fit(filename, room);
            let promptstr = fmtb!(question, &postedname, &lockuser, &lockprog, &pidstring);
            let choice = self.ask_user(YESORNO, &String::from_utf8_lossy(&promptstr));
            if choice == CANCEL && !self.we_are_running {
                self.finish();
            }
            if choice != YES {
                self.wipe_statusbar();
                return LockResult::Skip;
            }
        }
        if self.write_lockfile(&lockfilename, filename, false) {
            LockResult::Name(lockfilename)
        } else {
            LockResult::Failed
        }
    }

    fn has_valid_path(&mut self, filename: &[u8]) -> bool {
        let parentdir: Vec<u8> = match filename.iter().rposition(|&c| c == b'/') {
            Some(0) => b"/".to_vec(),
            Some(i) => filename[..i].to_vec(),
            None => b".".to_vec(),
        };
        if parentdir == b"." && fs::canonicalize(".").is_err() && errno() == libc::ENOENT {
            self.statusline(
                Msg::Alert,
                "Das Arbeitsverzeichnis ist nicht mehr vorhanden",
            );
            return false;
        }
        match fs::metadata(bytes_to_path(&parentdir)) {
            Err(e) => {
                let m = if e.kind() == std::io::ErrorKind::NotFound {
                    fmtb!("Verzeichnis „%s“ existiert nicht", &parentdir)
                } else {
                    fmtb!("Pfad „%s“: %s", &parentdir, &io_errstr(&e))
                };
                self.statusline_bytes(Msg::Alert, &m);
                false
            }
            Ok(md) => {
                let p = bytes_to_path(&parentdir);
                if !md.is_dir() {
                    let m = fmtb!("Pfad „%s“ ist kein Verzeichnis", &parentdir);
                    self.statusline_bytes(Msg::Alert, &m);
                    false
                } else if !path_access(p, libc::X_OK) {
                    let m = fmtb!("Auf Pfad „%s“ kann nicht zugegriffen werden", &parentdir);
                    self.statusline_bytes(Msg::Alert, &m);
                    false
                } else {
                    if self.isset(Flag::Locking)
                        && !self.isset(Flag::ViewMode)
                        && !path_access(p, libc::W_OK)
                    {
                        let m = fmtb!("Verzeichnis „%s“ ist schreibgeschützt", &parentdir);
                        self.statusline_bytes(Msg::Mild, &m);
                    }
                    true
                }
            }
        }
    }

    pub fn open_buffer(&mut self, filename: &[u8], new_one: bool) -> bool {
        self.as_an_at = false;
        if self.operating_dir.is_some() && self.outside_of_confinement(filename, false) {
            let od = self.operating_dir.clone().unwrap();
            let m = fmtb!("Datei kann außerhalb von %s nicht gelesen werden", &od);
            self.statusline_bytes(Msg::Alert, &m);
            return false;
        }
        let realname = self.expand_leading_tilde(filename);
        if !filename.is_empty() {
            if let Ok(md) = fs::metadata(bytes_to_path(&realname)) {
                let ft = md.file_type();
                if ft.is_dir() {
                    let m = fmtb!("„%s“ ist ein Verzeichnis", &realname);
                    self.statusline_bytes(Msg::Alert, &m);
                    return false;
                }
                if ft.is_char_device() || ft.is_block_device() {
                    let m = fmtb!("„%s“ ist eine Gerätedatei", &realname);
                    self.statusline_bytes(Msg::Alert, &m);
                    return false;
                }
                if new_one && (md.mode() & 0o222) == 0 && unsafe { libc::geteuid() } == 0 {
                    let m = fmtb!("„%s“ ist als schreibgeschützt deklariert", &realname);
                    self.statusline_bytes(Msg::Alert, &m);
                }
            } else if filename.last() == Some(&b'/') {
                let m = fmtb!("„%s“ ist ein ungültiger Dateiname", &realname);
                self.statusline_bytes(Msg::Alert, &m);
                return false;
            }
        }
        if new_one {
            self.make_new_buffer();
            if self.has_valid_path(&realname)
                && self.isset(Flag::Locking)
                && !self.isset(Flag::ViewMode)
                && !filename.is_empty()
            {
                match self.do_lockfile(&realname, true) {
                    LockResult::Skip => {
                        self.close_buffer();
                        return false;
                    }
                    LockResult::Name(n) => self.ofile_mut().lock_filename = Some(n),
                    LockResult::Failed => {}
                }
            }
        }
        let mut descriptor: i32 = 0;
        if !filename.is_empty() && !self.isset(Flag::NoreadMode) {
            match self.open_file(&realname, new_one) {
                Ok(Some((bytes, keymat))) => {
                    descriptor = 1;
                    let plain = keymat.is_none() && !bytes.is_empty();
                    if new_one
                        && plain
                        && let (Ok(full), Ok(md)) = (
                            fs::canonicalize(bytes_to_path(&realname)),
                            fs::metadata(bytes_to_path(&realname)),
                        )
                    {
                        self.ofile_mut().plain_origin = Some(PlainOrigin {
                            shown: realname.clone(),
                            path: path_to_bytes(&full),
                            dev: md.dev(),
                            ino: md.ino(),
                        });
                    }
                    if new_one || self.ofile().keymat.is_none() {
                        self.ofile_mut().keymat = keymat;
                    }
                    self.reading_plaintext = plain;
                    self.read_file(&bytes, &realname, !new_one);
                    self.reading_plaintext = false;
                    drop(bytes);
                    if self.ofile().statinfo.is_none() {
                        self.ofile_mut().statinfo = stat_info(&realname);
                    }
                }
                Ok(None) => descriptor = 0,
                Err(()) => descriptor = -1,
            }
        }
        if descriptor >= 0 && new_one {
            let top = self.filetop();
            let of = self.ofile_mut();
            of.filename.clone_from(&realname);
            of.current = top;
            of.current_x = 0;
            of.placewewant = 0;
        }
        true
    }

    pub fn set_modified(&mut self) {
        if self.ofile().modified {
            return;
        }
        self.ofile_mut().modified = true;
        self.titlebar(None);
        if let Some(lf) = self.ofile().lock_filename.clone() {
            let fname = self.ofile().filename.clone();
            self.write_lockfile(&lf, &fname, true);
        }
    }

    pub fn prepare_for_display(&mut self) {
        if !self.inhelp {
            self.titlebar(None);
        }
        self.refresh_needed = true;
    }

    pub fn mention_name_and_linecount(&mut self) {
        let count = self.lineno(self.filebot()) - i64::from(self.datalen(self.filebot()) == 0);
        if self.isset(Flag::MiniBar) {
            self.report_size = true;
            return;
        } else if self.isset(Flag::Zero) {
            return;
        }
        let name: Vec<u8> = if self.ofile().filename.is_empty() {
            b"Neuer Puffer".to_vec()
        } else {
            tail(&self.ofile().filename).to_vec()
        };
        let m = if self.ofile().fmt == Format::Dos {
            fmtb!(
                crate::utils::plural(
                    "%s -- %zu Zeile (%s)",
                    "%s -- %zu Zeilen (%s)",
                    count.cast_unsigned()
                ),
                &name,
                count,
                "DOS"
            )
        } else {
            fmtb!(
                crate::utils::plural("%s -- %zu Zeile", "%s -- %zu Zeilen", count.cast_unsigned()),
                &name,
                count
            )
        };
        self.statusline_bytes(Msg::Hush, &m);
    }

    fn redecorate_after_switch(&mut self) {
        if self.files.len() == 1 {
            self.statusline(Msg::Ahem, "Keine weiteren offenen Dateipuffer");
            return;
        }
        self.ensure_firstcolumn_is_aligned();
        self.prepare_for_display();
        self.currmenu = MMOST;
        self.shift_held = true;
        if let Some(e) = self.ofile_mut().errormessage.take() {
            self.statusline(Msg::Alert, &e);
        } else {
            self.mention_name_and_linecount();
        }
    }

    pub fn switch_to_prev_buffer(&mut self) {
        self.cur = self.prev_buffer_index(self.cur);
        self.redecorate_after_switch();
    }

    pub fn switch_to_next_buffer(&mut self) {
        self.cur = self.next_buffer_index(self.cur);
        self.redecorate_after_switch();
    }

    pub fn close_buffer(&mut self) {
        let orphan = self.cur;
        let top = self.files[orphan].filetop;
        self.free_lines(Some(top));
        self.discard_all_undo();
        let cbs: Vec<Option<Lp>> = self.files[orphan]
            .undo
            .iter()
            .map(|u| u.cutbuffer)
            .collect();
        for cb in cbs {
            self.free_lines(cb);
        }
        self.files.remove(orphan);
        if self.files.is_empty() {
            self.cur = 0;
        } else {
            self.cur = self.prev_buffer_index(orphan.min(self.files.len()));
            if orphan == 0 {
                self.cur = self.files.len() - 1;
            }
        }
        if self.files.len() == 1 {
            self.set_exit_tag(crate::global::EXIT_TAG);
        }
    }

    pub fn read_file(&mut self, bytes: &[u8], filename: &[u8], undoable: bool) {
        let was_lineno = self.lineno(self.current());
        let mut was_leftedge = 0usize;
        let mut num_lines = 0usize;
        let mut format = Format::Nix;
        if undoable {
            self.add_undo(UndoType::Insert, None);
        }
        if self.isset(Flag::SoftWrap) {
            was_leftedge = self.leftedge_for(self.xplustabs(), self.current());
        }
        let topline = self.make_new_node(None);
        let mut bottomline = topline;
        let mut buf: Vec<u8> = Vec::new();
        for &b in bytes {
            if b == b'\n' {
                if buf.last() == Some(&b'\r') && !self.isset(Flag::NoConvert) {
                    if num_lines == 0 {
                        format = Format::Dos;
                    }
                    buf.pop();
                }
                recode_nul_to_lf(&mut buf);
                self.nodes[bottomline].data = std::mem::take(&mut buf);
                let n = self.make_new_node(Some(bottomline));
                self.nodes[bottomline].next = Some(n);
                bottomline = n;
                num_lines += 1;
            } else {
                buf.push(b);
            }
        }
        let writable = if !undoable
            && !self.isset(Flag::ViewMode)
            && !filename.is_empty()
            && filename != b"stdin"
            && filename != b"pipe"
        {
            let c = std::ffi::CString::new(filename).unwrap_or_default();
            unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
        } else {
            true
        };
        if buf.is_empty() {
            self.nodes[bottomline].data = Vec::new();
        } else {
            recode_nul_to_lf(&mut buf);
            self.nodes[bottomline].data = std::mem::take(&mut buf);
            num_lines += 1;
        }
        buf.zeroize();
        self.ingraft_buffer(topline);
        let pww = self.xplustabs();
        self.ofile_mut().placewewant = pww;
        let plain = self.reading_plaintext;
        let (importance, note) = if plain {
            (Msg::Notice, " (unverschlüsselt)")
        } else {
            (Msg::Remark, "")
        };
        if !writable {
            let f = if plain {
                "Datei „%s“ ist schreibgeschützt (unverschlüsselt)"
            } else {
                "Datei „%s“ ist schreibgeschützt"
            };
            let m = fmtb!(f, filename);
            self.statusline_bytes(Msg::Alert, &m);
        } else if (self.isset(Flag::Zero) || self.isset(Flag::MiniBar))
            && !(self.we_are_running && undoable)
        {
            if plain && self.isset(Flag::Zero) {
                self.statusline(Msg::Notice, "Datei ist unverschlüsselt");
            }
        } else if format == Format::Dos {
            let m = fmt!(
                crate::utils::plural(
                    "%zu Zeile gelesen (aus DOS-Format konvertiert)",
                    "%zu Zeilen gelesen (aus DOS-Format konvertiert)",
                    num_lines as u64
                ),
                num_lines
            ) + note;
            self.statusline(importance, &m);
        } else {
            let m = fmt!(
                crate::utils::plural("%zu Zeile gelesen", "%zu Zeilen gelesen", num_lines as u64),
                num_lines
            ) + note;
            self.statusline(importance, &m);
        }
        self.report_size = true;
        if undoable && self.less_than_a_screenful(usize_of(was_lineno), was_leftedge) {
            self.focusing = false;
        }
        if undoable {
            self.update_undo(UndoType::Insert);
        }
        if self.isset(Flag::MakeItUnix) {
            self.ofile_mut().fmt = Format::Nix;
        } else if self.ofile().fmt == Format::Unspecified {
            self.ofile_mut().fmt = format;
        }
    }

    pub fn decrypt_with_prompt(
        &mut self,
        data: &[u8],
        name: &[u8],
    ) -> Option<(Zeroizing<Vec<u8>>, KeyMaterial)> {
        let shown = tail(name).to_vec();
        if let Some(km) = self.ofile().keymat.clone()
            && let Ok((kdf, salt, _)) = crypto::parse_header(data)
            && kdf == km.kdf
            && salt == km.salt
            && let Ok(pt) = crypto::decrypt_with_key(data, &km.key)
        {
            return Some((pt, km));
        }
        let mut hint = "";
        loop {
            let q = fmt!(
                "%sPasswort für %s",
                hint,
                &String::from_utf8_lossy(&shown).into_owned()
            );
            let password = self.ask_for_password(&q)?;
            if password.is_empty() {
                hint = "Leeres Passwort! ";
                self.term.beep();
                continue;
            }
            self.statusbar("Entschlüsseln …");
            self.term.doupdate();
            match crypto::decrypt(data, &password) {
                Ok(r) => {
                    self.wipe_statusbar();
                    return Some(r);
                }
                Err(CryptoError::BadPassword) => {
                    hint = "Falsches Passwort! ";
                    self.term.beep();
                    self.lastmessage = Msg::Vacuum;
                }
                Err(e) => {
                    self.statusline(Msg::Alert, &e.message());
                    return None;
                }
            }
        }
    }

    pub fn open_file(&mut self, filename: &[u8], new_one: bool) -> Result<Option<Opened>, ()> {
        let mut full_filename = self
            .get_full_path(filename)
            .unwrap_or_else(|| filename.to_vec());
        if fs::metadata(bytes_to_path(&full_filename)).is_err() {
            full_filename = filename.to_vec();
        }
        let Ok(md) = fs::metadata(bytes_to_path(&full_filename)) else {
            if new_one {
                self.statusline(Msg::Remark, "Neue Datei");
                return Ok(None);
            }
            let m = fmtb!("Datei „%s“ nicht gefunden", filename);
            self.statusline_bytes(Msg::Alert, &m);
            return Err(());
        };
        if md.file_type().is_fifo() {
            self.statusbar("Aus FIFO wird gelesen …");
        } else if !self.isset(Flag::Zero) || self.we_are_running {
            self.statusbar("Lesen …");
        }
        self.term.doupdate();
        let mut raw: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
        match fs::File::open(bytes_to_path(&full_filename))
            .and_then(|mut f| f.read_to_end(&mut raw))
        {
            Ok(_) => {}
            Err(e) => {
                let m = fmtb!("Fehler beim Lesen von %s: %s", filename, &io_errstr(&e));
                self.statusline_bytes(Msg::Alert, &m);
                return Err(());
            }
        }
        if crypto::is_zu_file(&raw) {
            if let Some((pt, km)) = self.decrypt_with_prompt(&raw, filename) {
                Ok(Some((pt, Some(km))))
            } else {
                self.statusbar("Abgebrochen");
                Err(())
            }
        } else {
            Ok(Some((raw, None)))
        }
    }

    pub fn execute_command(&mut self, command: &[u8]) {
        if command.first() == Some(&b'|') {
            self.statusline(Msg::Alert, "Weiterleiten ist deaktiviert");
            return;
        }
        let theshell = std::env::var_os("SHELL")
            .map_or_else(|| b"/bin/sh".to_vec(), |s| s.as_bytes().to_vec());
        let child = Command::new(bytes_to_path(&theshell))
            .arg("-c")
            .arg(std::ffi::OsStr::from_bytes(command))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                let m = fmt!("Fork konnte nicht durchgeführt werden: %s", &io_errstr(&e));
                self.statusline(Msg::Alert, &m);
                return;
            }
        };
        self.statusbar("Wird ausgeführt …");
        self.term.settle();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let out_handle = std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = stdout.read_to_end(&mut v);
            v
        });
        let err_handle = std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = stderr.read_to_end(&mut v);
            v
        });
        let mut cancelled = false;
        crate::CONTROL_C_WAS_PRESSED.store(false, std::sync::atomic::Ordering::SeqCst);
        crate::install_handler_for_ctrl_c();
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) => {}
                Err(_) => break None,
            }
            if crate::CONTROL_C_WAS_PRESSED.swap(false, std::sync::atomic::Ordering::SeqCst) {
                let _ = child.kill();
                cancelled = true;
            }
            if self.term.terminal_is_gone() {
                let _ = child.kill();
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        crate::restore_handler_for_ctrl_c();
        crate::CONTROL_C_WAS_PRESSED.store(false, std::sync::atomic::Ordering::SeqCst);
        let mut output = out_handle.join().unwrap_or_default();
        let errout = err_handle.join().unwrap_or_default();
        output.extend_from_slice(&errout);
        let _ = child.wait();
        self.read_file(&output, b"pipe", true);
        output.zeroize();
        let failed = status.is_none_or(|st| !st.success());
        if cancelled {
            self.statusline(Msg::Alert, "Abgebrochen");
        } else if failed {
            let reason: Vec<u8> = self.prev(self.current()).map_or_else(
                || b"---".to_vec(),
                |p| {
                    let d = self.data(p);
                    d.windows(2)
                        .position(|w| w == b": ")
                        .map_or_else(|| b"---".to_vec(), |i| d[i + 2..].to_vec())
                },
            );
            let m = fmtb!("Fehler: %s", &reason);
            self.statusline_bytes(Msg::Alert, &m);
        }
        if self.lastmessage == Msg::Alert {
            self.do_undo();
            self.discard_until_current();
        }
    }

    fn insert_a_file_or(&mut self, mut execute: bool) {
        let mut given: Vec<u8> = Vec::new();
        let was_multibuffer = self.isset(Flag::NewBuffer);
        self.as_an_at = false;
        self.ran_a_tool = false;
        if execute && !self.foretext.is_empty() {
            given.clone_from(&self.foretext);
        }
        loop {
            let msg: Vec<u8> = if execute {
                if self.isset(Flag::NewBuffer) {
                    "In neuem Puffer auszuführender Befehl".into()
                } else {
                    "Auszuführender Befehl".into()
                }
            } else {
                let from: Vec<u8> = self.operating_dir.clone().unwrap_or_else(|| b"./".to_vec());
                let f = if self.isset(Flag::NewBuffer) {
                    if self.isset(Flag::NoConvert) {
                        "In neuen Puffer ohne Umwandlung einzulesende Datei [von %s]"
                    } else {
                        "In neuen Puffer einzulesende Datei [von %s]"
                    }
                } else if self.isset(Flag::NoConvert) {
                    "Ohne Umwandlung einzufügende Datei [von %s]"
                } else {
                    "Einzufügende Datei [von %s]"
                };
                fmtb!(f, &from)
            };
            self.present_path = b"./".to_vec();
            let hist = if execute {
                Some(HistKind::Execute)
            } else {
                None
            };
            let menu = if execute { MEXECUTE } else { MINSERTFILE };
            let g = given.clone();
            let mut response = self.do_prompt(menu, &g, hist, RefreshFn::Edit, &msg);
            if response == -1 || (response == -2 && !self.isset(Flag::NewBuffer)) {
                self.statusbar("Abgebrochen");
                break;
            }
            let was_lineno = self.lineno(self.current());
            let was_x = self.current_x();
            let function = self.func_from_key(response);
            given.clone_from(&self.answer);
            if self.ran_a_tool {
                break;
            }
            match function {
                Some(Func::FlipNewbuffer) => {
                    if self.isset(Flag::ViewMode) {
                        self.term.beep();
                    } else {
                        self.flags.toggle(Flag::NewBuffer);
                    }
                    continue;
                }
                Some(Func::FlipConvert) => {
                    self.flags.toggle(Flag::NoConvert);
                    continue;
                }
                Some(Func::FlipExecute) => {
                    execute = !execute;
                    continue;
                }
                Some(Func::ToFiles) => {
                    let a = self.answer.clone();
                    match self.browse_in(&a) {
                        None => continue,
                        Some(chosen) => {
                            self.answer = chosen;
                            response = 0;
                        }
                    }
                }
                _ => {}
            }
            if response != 0 && (!self.isset(Flag::NewBuffer) || response != -2) {
                continue;
            }
            if execute {
                if self.isset(Flag::NewBuffer) {
                    self.open_buffer(b"", true);
                }
                if !self.answer.is_empty() {
                    let a = self.answer.clone();
                    self.execute_command(&a);
                    self.update_history(HistKind::Execute, &a, PRUNE_DUPLICATE);
                }
                if self.isset(Flag::NewBuffer) {
                    let top = self.filetop();
                    let of = self.ofile_mut();
                    of.current = top;
                    of.current_x = 0;
                    of.placewewant = 0;
                    self.set_modified();
                }
            } else {
                let a = self.answer.clone();
                let nb = self.isset(Flag::NewBuffer);
                self.open_buffer(&a, nb);
            }
            if self.isset(Flag::NewBuffer) {
                if self.isset(Flag::PositionLog) && !execute {
                    self.restore_cursor_position_if_any();
                }
                self.prepare_for_display();
            } else {
                if self.lineno(self.current()) != was_lineno || self.current_x() != was_x {
                    self.set_modified();
                }
                self.refresh_needed = true;
            }
            break;
        }
        if was_multibuffer {
            self.flags.set(Flag::NewBuffer);
        } else {
            self.flags.unset(Flag::NewBuffer);
        }
    }

    pub fn do_insertfile(&mut self) {
        if !self.in_restricted_mode() {
            self.insert_a_file_or(false);
        }
    }

    pub fn do_execute(&mut self) {
        if !self.in_restricted_mode() {
            self.insert_a_file_or(true);
        }
    }

    pub fn get_full_path(&mut self, origpath: &[u8]) -> Option<Vec<u8>> {
        let untilded = self.expand_leading_tilde(origpath);
        let mut target: Option<Vec<u8>> = fs::canonicalize(bytes_to_path(&untilded))
            .ok()
            .map(|p| path_to_bytes(&p));
        if target.is_none() {
            let mut u = untilded;
            let slash = u.iter().rposition(|&c| c == b'/').unwrap_or_else(|| {
                u.insert(0, b'/');
                u.insert(0, b'.');
                1
            });
            let dir: Vec<u8> = if slash == 0 {
                b"/".to_vec()
            } else {
                u[..slash].to_vec()
            };
            if let Ok(p) = fs::canonicalize(bytes_to_path(&dir)) {
                let mut t = path_to_bytes(&p);
                if t.last() != Some(&b'/') {
                    t.push(b'/');
                }
                t.extend_from_slice(&u[slash + 1..]);
                target = Some(t);
            }
        }
        if let Some(t) = target.as_mut()
            && t.len() > 1
            && is_dir_path(t)
            && t.last() != Some(&b'/')
        {
            t.push(b'/');
        }
        target
    }

    pub fn init_operating_dir(&mut self) {
        let od = self.operating_dir.clone().unwrap();
        let target = self.get_full_path(&od);
        match target {
            Some(t) if std::env::set_current_dir(bytes_to_path(&t)).is_ok() => {
                self.operating_dir = Some(t);
            }
            _ => {
                let m = fmtb!("Ungültiges Arbeitsverzeichnis: %s\n", &od);
                self.die_bytes(&m);
            }
        }
    }

    pub fn outside_of_confinement(&mut self, somepath: &[u8], tabbing: bool) -> bool {
        let Some(od) = self.operating_dir.clone() else {
            return false;
        };
        let Some(fullpath) = self.get_full_path(somepath) else {
            return tabbing;
        };
        let is_inside = fullpath.starts_with(&od);
        let begins_to_be = tabbing && od.starts_with(&fullpath);
        !is_inside && !begins_to_be
    }

    pub fn init_backup_dir(&mut self) {
        let bd = self.backup_dir.clone().unwrap();
        match self.get_full_path(&bd) {
            Some(t) if t.last() == Some(&b'/') => self.backup_dir = Some(t),
            _ => {
                let m = fmtb!("Ungültiges Sicherungsverzeichnis: %s\n", &bd);
                self.die_bytes(&m);
            }
        }
    }

    fn make_backup_of(&mut self, realname: &[u8], keymat: &KeyMaterial) -> bool {
        self.statusbar("Sicherung wird erstellt …");

        let backupname: Vec<u8> = if self.backup_dir.is_none() {
            let mut b = realname.to_vec();
            b.push(b'~');
            b
        } else {
            let thename: Vec<u8> = self.get_full_path(realname).map_or_else(
                || tail(realname).to_vec(),
                |mut t| {
                    for c in &mut t {
                        if *c == b'/' {
                            *c = b'!';
                        }
                    }
                    t
                },
            );
            let mut b = self.backup_dir.clone().unwrap();
            b.extend_from_slice(&thename);
            let b = get_next_filename(&b, b"~");
            if b.is_empty() {
                self.statusline(Msg::Alert, "Zu viele Sicherungsdateien vorhanden");
                return false;
            }
            b
        };
        let mut attempt_name = backupname;
        let mut second_attempt = false;
        let mut unreadable_original = false;
        loop {
            let result: std::io::Result<()> = (|| {
                match fs::remove_file(bytes_to_path(&attempt_name)) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        if !self.isset(Flag::InsecureBackup) {
                            return Err(e);
                        }
                    }
                }
                let mut oo = fs::OpenOptions::new();
                oo.write(true).create(true).mode(0o600);
                if self.isset(Flag::InsecureBackup) {
                    oo.truncate(true);
                } else {
                    oo.create_new(true);
                }
                let mut backup = oo.open(bytes_to_path(&attempt_name))?;
                if let Some(si) = self.ofile().statinfo {
                    use std::os::unix::io::AsRawFd;
                    if unsafe { libc::fchown(backup.as_raw_fd(), si.uid, si.gid) } < 0
                        && crate::utils::errno() != libc::EPERM
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    if unsafe {
                        libc::fchmod(backup.as_raw_fd(), (si.mode & 0o7777) as libc::mode_t)
                    } < 0
                        && crate::utils::errno() != libc::EPERM
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                let mut content: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
                let mut original = match fs::File::open(bytes_to_path(realname)) {
                    Ok(f) => f,
                    Err(e) => {
                        unreadable_original = true;
                        return Err(e);
                    }
                };
                if let Err(e) = original.read_to_end(&mut content) {
                    unreadable_original = true;
                    return Err(e);
                }
                if !content.is_empty() && !crypto::is_zu_file(&content) {
                    let sealed = crypto::encrypt(keymat, &content)
                        .map_err(|e| std::io::Error::other(e.message()))?;
                    backup.write_all(&sealed)?;
                } else {
                    backup.write_all(&content)?;
                }
                backup.sync_all()?;
                if let Ok(md) = original.metadata() {
                    let times = [
                        libc::timespec {
                            tv_sec: md.atime() as libc::time_t,
                            tv_nsec: md.atime_nsec() as _,
                        },
                        libc::timespec {
                            tv_sec: md.mtime() as libc::time_t,
                            tv_nsec: md.mtime_nsec() as _,
                        },
                    ];
                    unsafe {
                        use std::os::unix::io::AsRawFd;
                        libc::futimens(backup.as_raw_fd(), times.as_ptr());
                    }
                }
                Ok(())
            })();
            match result {
                Ok(()) => return true,
                Err(e) => {
                    let errstr = io_errstr(&e);
                    let errno_val = e.raw_os_error().unwrap_or(0);
                    self.get_homedir();
                    if unreadable_original {
                        self.warn_and_briefly_pause("Originaldatei kann nicht gelesen werden");
                    } else if !second_attempt && self.homedir.is_some() {
                        let _ = fs::remove_file(bytes_to_path(&attempt_name));
                        self.warn_and_briefly_pause(
                            "Reguläre Sicherung kann nicht erstellt werden",
                        );
                        self.warn_and_briefly_pause("Wird im Heimatverzeichnis erneut versucht");
                        self.currmenu = MMOST;
                        let mut b = self.homedir.clone().unwrap();
                        b.push(b'/');
                        b.extend_from_slice(tail(realname));
                        b.push(b'~');
                        attempt_name = get_next_filename(&b, b"");
                        second_attempt = true;
                        continue;
                    } else {
                        self.warn_and_briefly_pause("Sicherung kann nicht erstellt werden");
                    }
                    self.warn_and_briefly_pause(&errstr);
                    self.currmenu = MMOST;
                    if errno_val != libc::ENOSPC
                        && self.ask_user(
                            YESORNO,
                            "Sicherung kann nicht erstellt werden; tatsächliche Datei trotzdem speichern?",
                        ) == YES
                    {
                        return true;
                    }
                    let m = fmt!("Sicherung kann nicht erstellt werden: %s", &errstr);
                    self.statusline(Msg::Hush, &m);
                    return false;
                }
            }
        }
    }

    pub fn ask_for_new_password(&mut self, target: &[u8]) -> Option<KeyMaterial> {
        let mut hint = "";
        loop {
            let first = self.ask_for_password(&format!("{hint}Neues Passwort"))?;
            if first.is_empty() {
                hint = "Leeres Passwort! ";
                self.term.beep();
                continue;
            }
            if String::from_utf8_lossy(&first).chars().count() < crypto::MIN_PASSWORD_CHARS {
                hint = "Mindestens 15 Zeichen! ";
                self.term.beep();
                continue;
            }
            if crypto::password_is_common(&first) {
                hint = "Zu verbreitet! ";
                self.term.beep();
                continue;
            }
            if Self::password_names_the_file(&first, target) {
                hint = "Gleicht dem Dateinamen! ";
                self.term.beep();
                continue;
            }
            let second = self.ask_for_password("Passwort wiederholen")?;
            if !crypto::same_password(&first, &second) {
                hint = "Passwörter ungleich! ";
                self.term.beep();
                continue;
            }
            self.statusbar("Schlüssel wird abgeleitet …");
            self.term.doupdate();
            let kdf = self.kdf;
            match crypto::new_key_material(&first, kdf) {
                Ok(km) => {
                    self.wipe_statusbar();
                    return Some(km);
                }
                Err(e) => {
                    self.statusline(Msg::Alert, &e.message());
                    return None;
                }
            }
        }
    }

    fn password_names_the_file(password: &[u8], target: &[u8]) -> bool {
        let lowered = Zeroizing::new(String::from_utf8_lossy(password).to_lowercase());
        let name = String::from_utf8_lossy(tail(target)).to_lowercase();
        let stem = name
            .rsplit_once('.')
            .map_or(name.as_str(), |(stem, _)| stem);
        !name.is_empty() && (*lowered == name || *lowered == stem)
    }

    fn ensure_keymat(&mut self, target: &[u8]) -> Option<KeyMaterial> {
        if let Some(km) = self.ofile().keymat.clone() {
            return Some(km);
        }
        let km = self.ask_for_new_password(target)?;
        self.ofile_mut().keymat = Some(km.clone());
        Some(km)
    }

    pub fn change_password(&mut self) {
        if self.in_restricted_mode() {
            return;
        }
        let target = self.ofile().filename.clone();
        match self.ask_for_new_password(&target) {
            Some(km) => {
                self.ofile_mut().keymat = Some(km);
                self.statusline(Msg::Remark, "Passwort gilt ab dem nächsten Speichern");
                if !self.ofile().filename.is_empty() {
                    self.ofile_mut().modified = true;
                    self.titlebar(None);
                }
            }
            None => self.statusbar("Abgebrochen"),
        }
    }

    fn assemble_plaintext(&self, top: Lp, stop_after: Option<Lp>) -> (Zeroizing<Vec<u8>>, usize) {
        let mut out: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
        let mut lineswritten = 0usize;
        let mut line = top;
        loop {
            let d = self.data(line);
            out.extend(d.iter().map(|&b| if b == b'\n' { 0 } else { b }));
            let next = if Some(line) == stop_after {
                None
            } else {
                self.next(line)
            };
            match next {
                None => {
                    if !d.is_empty() {
                        lineswritten += 1;
                    }
                    break;
                }
                Some(n) => {
                    if self.ofile().fmt == Format::Dos {
                        out.push(b'\r');
                    }
                    out.push(b'\n');
                    lineswritten += 1;
                    line = n;
                }
            }
        }
        (out, lineswritten)
    }

    fn reserve_space(file: &fs::File, len: u64) -> std::io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::io::AsRawFd;
            if len > 0
                && unsafe {
                    libc::fallocate(
                        file.as_raw_fd(),
                        libc::FALLOC_FL_KEEP_SIZE,
                        0,
                        len.cast_signed(),
                    )
                } != 0
            {
                let e = std::io::Error::last_os_error();
                if matches!(
                    e.raw_os_error(),
                    Some(libc::ENOSPC | libc::EDQUOT | libc::EFBIG)
                ) {
                    return Err(e);
                }
            }
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::io::AsRawFd;
            let present = file.metadata()?.len();
            if len > present {
                let mut store = libc::fstore_t {
                    fst_flags: libc::F_ALLOCATEALL,
                    fst_posmode: libc::F_PEOFPOSMODE,
                    fst_offset: 0,
                    fst_length: (len - present).cast_signed(),
                    fst_bytesalloc: 0,
                };
                if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_PREALLOCATE, &raw mut store) }
                    == -1
                {
                    let e = std::io::Error::last_os_error();
                    if matches!(
                        e.raw_os_error(),
                        Some(libc::ENOSPC | libc::EDQUOT | libc::EFBIG)
                    ) {
                        return Err(e);
                    }
                }
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let _ = (file, len);
        Ok(())
    }

    fn overwrite_in_place(file: &fs::File, data: &[u8]) -> std::io::Result<()> {
        use std::os::unix::fs::FileExt;
        let old_len = file.metadata()?.len();
        let new_len = data.len() as u64;
        file.write_all_at(data, 0)?;
        if old_len > new_len {
            overwrite_with_zeros(file, new_len, old_len)?;
        }
        file.set_len(new_len)?;
        file.sync_all()
    }

    pub fn write_file(&mut self, name: &[u8], method: Writing, annotate: bool) -> bool {
        let top = self.filetop();
        self.write_lines_to_file(name, top, None, method, annotate)
    }

    fn write_lines_to_file(
        &mut self,
        name: &[u8],
        top: Lp,
        stop_after: Option<Lp>,
        method: Writing,
        annotate: bool,
    ) -> bool {
        let realname = self.expand_leading_tilde(name);
        let normal = method != Writing::Special;
        if normal && self.operating_dir.is_some() && self.outside_of_confinement(&realname, false) {
            let od = self.operating_dir.clone().unwrap();
            let m = fmtb!("Außerhalb von %s kann nicht geschrieben werden", &od);
            self.statusline_bytes(Msg::Alert, &m);
            return false;
        }
        let existing_md = if normal {
            fs::metadata(bytes_to_path(&realname)).ok()
        } else {
            None
        };
        let is_existing_file = existing_md.is_some();
        let is_fifo = existing_md
            .as_ref()
            .is_some_and(|m| m.file_type().is_fifo());
        let Some(keymat) = self.ensure_keymat(&realname) else {
            self.statusbar("Abgebrochen");
            return false;
        };
        if self.isset(Flag::MakeBackup)
            && is_existing_file
            && !is_fifo
            && !self.make_backup_of(&realname, &keymat)
        {
            return false;
        }
        let (mut plaintext, lineswritten) = self.assemble_plaintext(top, stop_after);
        let mut used_keymat = keymat.clone();
        if method == Writing::Append || method == Writing::Prepend {
            if is_fifo {
                let m = fmtb!("Fehler beim Schreiben von %s: %s", &realname, "FIFO");
                self.statusline_bytes(Msg::Alert, &m);
                return false;
            }
            let mut raw: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
            match fs::File::open(bytes_to_path(&realname)).and_then(|mut f| f.read_to_end(&mut raw))
            {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    let m = fmtb!("Fehler beim Lesen von %s: %s", &realname, &io_errstr(&e));
                    self.statusline_bytes(Msg::Alert, &m);
                    return false;
                }
            }
            let existing: Zeroizing<Vec<u8>> = if crypto::is_zu_file(&raw) {
                if let Some((pt, km)) = self.decrypt_with_prompt(&raw, &realname) {
                    used_keymat = km;
                    pt
                } else {
                    self.statusbar("Abgebrochen");
                    return false;
                }
            } else {
                raw.clone()
            };
            let mut combined: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
            if method == Writing::Append {
                combined.extend_from_slice(&existing);
                combined.extend_from_slice(&plaintext);
            } else {
                combined.extend_from_slice(&plaintext);
                combined.extend_from_slice(&existing);
            }
            plaintext = combined;
        }
        if is_fifo {
            self.statusbar("In FIFO wird geschrieben …");
        } else if normal {
            self.statusbar("Wird geschrieben …");
        }
        self.term.doupdate();
        let ciphertext = match crypto::encrypt(&used_keymat, &plaintext) {
            Ok(c) => c,
            Err(e) => {
                self.statusline(Msg::Alert, &e.message());
                return false;
            }
        };
        drop(plaintext);
        let mut touched_in_place = false;
        let atomic = if normal && !is_fifo {
            write_atomically(&realname, &ciphertext)
        } else {
            Ok(false)
        };
        let result = match atomic {
            Ok(true) => Ok(()),
            Err(e) => Err(e),
            Ok(false) => {
                let regular = existing_md.as_ref().is_none_or(|m| m.file_type().is_file());
                let mut oo = fs::OpenOptions::new();
                oo.write(true).create(true);
                if normal {
                    oo.mode(0o600);
                    if !regular {
                        oo.truncate(true);
                    }
                } else {
                    oo.create_new(true).mode(0o600);
                }
                oo.open(bytes_to_path(&realname)).and_then(|mut f| {
                    if normal && regular {
                        Self::reserve_space(&f, ciphertext.len() as u64)?;
                        touched_in_place = true;
                        Self::overwrite_in_place(&f, &ciphertext)
                    } else {
                        f.write_all(&ciphertext)?;
                        f.flush()?;
                        if !is_fifo {
                            f.sync_all()?;
                        }
                        Ok(())
                    }
                })
            }
        };
        if let Err(e) = result {
            let errno_val = e.raw_os_error().unwrap_or(0);
            let m = fmtb!(
                "Fehler beim Schreiben von %s: %s",
                &realname,
                &io_errstr(&e)
            );
            self.statusline_bytes(Msg::Alert, &m);
            if errno_val == libc::ENOSPC && normal && touched_in_place {
                self.term.napms(3200);
                self.lastmessage = Msg::Vacuum;
                self.statusline(Msg::Alert, "Die Datei auf dem Datenträger ist beschädigt!");
                self.term.napms(3200);
                self.lastmessage = Msg::Vacuum;
                self.statusline(
                    Msg::Alert,
                    "Vielleicht ^T^Z, Platz auf dem Datenträger schaffen, wiederaufnehmen, dann ^S^X",
                );
                self.ofile_mut().statinfo = stat_info(&realname);
            }
            return false;
        }
        if annotate && method == Writing::Overwrite {
            if self.ofile().filename != realname {
                if let Some(lf) = self.ofile_mut().lock_filename.take() {
                    self.delete_lockfile(&lf);
                }
                if self.isset(Flag::Locking)
                    && let LockResult::Name(n) = self.do_lockfile(&realname, false)
                {
                    self.ofile_mut().lock_filename = Some(n);
                }
                self.ofile_mut().filename.clone_from(&realname);
            }
            self.ofile_mut().statinfo = stat_info(&realname);
            let serial = self.current_undo().map(|u| u.serial);
            let of = self.ofile_mut();
            of.last_saved = serial;
            of.last_action = UndoType::Other;
            of.modified = false;
            self.titlebar(None);
        }
        if self.isset(Flag::MiniBar) && !self.isset(Flag::Zero) && self.lines() > 1 && annotate {
            self.report_size = true;
        } else if normal {
            let m = fmt!(
                crate::utils::plural(
                    "%zu Zeile geschrieben",
                    "%zu Zeilen geschrieben",
                    lineswritten as u64
                ),
                lineswritten
            );
            self.statusline(Msg::Remark, &m);
        }
        if annotate
            && method == Writing::Overwrite
            && let Some(origin) = self.ofile().plain_origin.clone()
        {
            if plaintext_still_there(&origin) {
                let m = fmtb!("Unverschlüsselte Datei „%s“ existiert noch", &origin.shown,);
                self.statusline_bytes(Msg::Mild, &m);
            } else {
                self.ofile_mut().plain_origin = None;
            }
        }
        true
    }

    pub fn write_region_to_file(&mut self, name: &[u8], method: Writing) -> bool {
        let (topline, top_x, botline, bot_x) = self.get_region();
        let head = self.make_new_node(None);
        let mut item = head;
        let mut line = topline;
        loop {
            let d = &self.nodes[line].data;
            let from = if line == topline { top_x } else { 0 };
            let to = if line == botline { bot_x } else { d.len() };
            self.nodes[item].data = d[from..to].to_vec();
            if line == botline {
                break;
            }
            let n = self.make_new_node(Some(item));
            self.nodes[item].next = Some(n);
            item = n;
            line = self.next(line).unwrap();
        }
        if method != Writing::Special && bot_x > 0 && !self.isset(Flag::NoNewlines) {
            let stopper = self.make_new_node(Some(item));
            self.nodes[item].next = Some(stopper);
            item = stopper;
        }
        let retval = self.write_lines_to_file(name, head, Some(item), method, NONOTES);
        self.free_lines(Some(head));
        retval
    }

    pub fn write_it_out(&mut self, exiting: bool, withprompt: bool) -> i32 {
        let mut confirm_namechange = !self.ofile().filename.is_empty();
        let mut method = Writing::Overwrite;
        self.as_an_at = false;
        let mut given: Vec<u8> = if self.ofile().mark.is_some() && !exiting {
            Vec::new()
        } else {
            self.ofile().filename.clone()
        };
        loop {
            let mut response = 0;
            let choice;
            let formatstr = if self.ofile().fmt == Format::Dos {
                " [DOS-Format]"
            } else {
                ""
            };
            let backupstr = if self.isset(Flag::MakeBackup) {
                " [Sicherungsdatei]"
            } else {
                ""
            };
            let msg = if self.ofile().mark.is_some() && !exiting && !self.isset(Flag::Restricted) {
                match method {
                    Writing::Prepend => "Auswahl vorne an Datei anfügen",
                    Writing::Append => "Auswahl an Datei anhängen",
                    _ => "Auswahl in Datei schreiben",
                }
            } else if method != Writing::Overwrite {
                if method == Writing::Prepend {
                    "Vorne an Datei anfügen"
                } else {
                    "An Datei anhängen"
                }
            } else {
                "In Datei schreiben"
            };
            self.present_path = b"./".to_vec();
            if (!withprompt || (self.isset(Flag::SaveOnExit) && exiting))
                && !self.ofile().filename.is_empty()
            {
                self.answer = self.ofile().filename.clone();
            } else {
                let full = format!("{msg}{formatstr}{backupstr}");
                let g = given.clone();
                response = self.do_prompt(MWRITEFILE, &g, None, RefreshFn::Edit, full.as_bytes());
            }
            if response < 0 {
                self.statusbar("Abgebrochen");
                return 0;
            }
            let function = self.func_from_key(response);
            if function == Some(Func::DiscardBuffer) {
                self.final_status = 2;
                return 2;
            }
            given.clone_from(&self.answer);
            match function {
                Some(Func::ToFiles) if !self.isset(Flag::Restricted) => {
                    let a = self.answer.clone();
                    match self.browse_in(&a) {
                        None => continue,
                        Some(c) => self.answer = c,
                    }
                }
                Some(Func::DosFormat) => {
                    let f = if self.ofile().fmt == Format::Dos {
                        Format::Nix
                    } else {
                        Format::Dos
                    };
                    self.ofile_mut().fmt = f;
                    continue;
                }
                Some(Func::BackItUp) if !self.isset(Flag::Restricted) => {
                    self.flags.toggle(Flag::MakeBackup);
                    continue;
                }
                Some(Func::PrependIt | Func::AppendIt) if !self.isset(Flag::Restricted) => {
                    if function == Some(Func::PrependIt) {
                        method = if method == Writing::Prepend {
                            Writing::Overwrite
                        } else {
                            Writing::Prepend
                        };
                    } else {
                        method = if method == Writing::Append {
                            Writing::Overwrite
                        } else {
                            Writing::Append
                        };
                    }
                    if self.answer == self.ofile().filename {
                        given.clear();
                    }
                    continue;
                }
                Some(Func::ChangePassword) => {
                    self.change_password();
                    continue;
                }
                Some(Func::Help) => continue,
                _ => {}
            }
            if exiting
                && !self.isset(Flag::SaveOnExit)
                && self.ofile().filename.is_empty()
                && self.answer == b"zzy"
                && !self.did_credits
            {
                if self.lines() > 5 && self.cols() > 31 {
                    self.do_credits();
                    self.did_credits = true;
                } else {
                    self.statusline(Msg::Ahem, "Zu klein");
                }
                return 0;
            }
            if method == Writing::Overwrite {
                let a = self.answer.clone();
                let full_answer = self.get_full_path(&a);
                let fname = self.ofile().filename.clone();
                let full_filename = self.get_full_path(&fname);
                let check: Vec<u8> = full_answer.clone().unwrap_or_else(|| a.clone());
                let fileinfo = stat_info(&check);
                let name_exists = fileinfo.is_some();
                let do_warning = if self.ofile().filename.is_empty() {
                    name_exists
                } else if full_answer.is_some() && full_filename.is_some() {
                    full_answer != full_filename
                } else {
                    a != fname
                };
                if do_warning {
                    if self.isset(Flag::Restricted) {
                        self.warn_and_briefly_pause(
                            "Datei existiert – kann nicht überschrieben werden",
                        );
                        continue;
                    }
                    if confirm_namechange && (exiting || self.ofile().mark.is_none()) {
                        if self.ask_user(YESORNO, "Datei unter einem ANDEREN NAMEN speichern? ")
                            != YES
                        {
                            continue;
                        }
                        confirm_namechange = false;
                    }
                    if name_exists {
                        let question = "Datei „%s“ existiert, ÜBERSCHREIBEN? ";
                        let room = ((self.cols().cast_signed()
                            - self.breadth(question.as_bytes()).cast_signed()
                            + 1)
                        .max(0))
                        .cast_unsigned();
                        let name = self.crop_to_fit(&a, room);
                        let message = fmtb!(question, &name);
                        choice = self.ask_user(YESORNO, &String::from_utf8_lossy(&message));
                        if choice != YES {
                            continue;
                        }
                    }
                } else if name_exists
                    && let (Some(si), Some(fi)) = (self.ofile().statinfo, fileinfo)
                    && (si.mtime < fi.mtime || si.dev != fi.dev || si.ino != fi.ino)
                {
                    self.warn_and_briefly_pause("Die Datei wurde auf dem Datenträger geändert");
                    choice = self.ask_user(
                        YESORNO,
                        "Datei wurde seit dem Öffnen geändert, dennoch speichern? ",
                    );
                    self.wipe_statusbar();
                    if self.isset(Flag::SaveOnExit) && withprompt {
                        if choice == YES {
                            return i32::from(self.write_file(&fname, Writing::Overwrite, NONOTES));
                        } else if choice == NO {
                            return 2;
                        }
                        return 0;
                    } else if choice == CANCEL && exiting {
                        continue;
                    } else if choice != YES {
                        return 1;
                    }
                }
            }
            break;
        }
        let a = self.answer.clone();
        if self.ofile().mark.is_some() && withprompt && !exiting && !self.isset(Flag::Restricted) {
            return i32::from(self.write_region_to_file(&a, method));
        }
        i32::from(self.write_file(&a, method, !exiting))
    }

    pub fn do_writeout(&mut self) {
        if self.write_it_out(false, true) == 2 {
            self.close_and_go();
        }
    }

    pub fn do_savefile(&mut self) {
        if self.write_it_out(false, false) == 2 {
            self.close_and_go();
        }
    }

    pub fn expand_leading_tilde(&mut self, path: &[u8]) -> Vec<u8> {
        if path.first() != Some(&b'~') || path.len() == 1 {
            return path.to_vec();
        }
        let mut i = 1;
        while i < path.len() && path[i] != b'/' {
            i += 1;
        }
        let tilded: Vec<u8> = if i == 1 {
            self.get_homedir();
            self.homedir.clone().unwrap_or_default()
        } else {
            let user = &path[1..i];
            let mut found: Option<Vec<u8>> = None;
            unsafe {
                libc::setpwent();
                loop {
                    let pw = libc::getpwent();
                    if pw.is_null() {
                        break;
                    }
                    if CStr::from_ptr((*pw).pw_name).to_bytes() == user {
                        found = Some(CStr::from_ptr((*pw).pw_dir).to_bytes().to_vec());
                        break;
                    }
                }
                libc::endpwent();
            }
            found.unwrap_or_else(|| path[..i].to_vec())
        };
        let mut v = tilded;
        v.extend_from_slice(&path[i..]);
        v
    }

    pub fn is_dir(&mut self, path: &[u8]) -> bool {
        let p = self.expand_leading_tilde(path);
        is_dir_path(&p)
    }

    fn username_completion(&mut self, morsel: &[u8], length: usize) -> Vec<Vec<u8>> {
        let mut matches = Vec::new();
        let frag = &morsel[1..length.min(morsel.len())];
        let mut entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        unsafe {
            libc::setpwent();
            loop {
                let pw = libc::getpwent();
                if pw.is_null() {
                    break;
                }
                entries.push((
                    CStr::from_ptr((*pw).pw_name).to_bytes().to_vec(),
                    CStr::from_ptr((*pw).pw_dir).to_bytes().to_vec(),
                ));
            }
            libc::endpwent();
        }
        for (name, dir) in entries {
            if name.starts_with(frag) {
                if self.operating_dir.is_some() && self.outside_of_confinement(&dir, true) {
                    continue;
                }
                let mut m = b"~".to_vec();
                m.extend_from_slice(&name);
                matches.push(m);
            }
        }
        matches
    }

    fn filename_completion(&mut self, morsel: &[u8]) -> Option<Vec<Vec<u8>>> {
        let (dirname, filename): (Vec<u8>, Vec<u8>) = match morsel.iter().rposition(|&c| c == b'/')
        {
            Some(slash) => {
                let f = morsel[slash + 1..].to_vec();
                let mut d = self.expand_leading_tilde(&morsel[..=slash]);
                if d.first() != Some(&b'/') {
                    let mut pp = self.present_path.clone();
                    pp.extend_from_slice(&morsel[..=slash]);
                    d = pp;
                }
                (d, f)
            }
            None => (self.present_path.clone(), morsel.to_vec()),
        };
        let Ok(rd) = fs::read_dir(bytes_to_path(&dirname)) else {
            self.term.beep();
            return None;
        };
        let mut matches = Vec::new();
        for entry in rd.flatten() {
            let name = entry.file_name().as_bytes().to_vec();
            if name.starts_with(&filename) && name != b"." && name != b".." {
                let mut fullname = dirname.clone();
                fullname.extend_from_slice(&name);
                if self.operating_dir.is_some() && self.outside_of_confinement(&fullname, true) {
                    continue;
                }
                if self.currmenu == MGOTODIR && !self.is_dir(&fullname) {
                    continue;
                }
                matches.push(name);
            }
        }
        Some(matches)
    }

    pub fn input_tab(
        &mut self,
        mut morsel: Vec<u8>,
        place: &mut usize,
        refresh: RefreshFn,
        listed: &mut bool,
    ) -> Vec<u8> {
        if *place < morsel.len() {
            self.term.beep();
            return morsel;
        }
        let mut matches: Option<Vec<Vec<u8>>> = None;
        if morsel.first() == Some(&b'~') && !morsel.contains(&b'/') {
            let m = self.username_completion(&morsel, *place);
            if !m.is_empty() {
                matches = Some(m);
            }
        }
        if matches.is_none() {
            matches = self.filename_completion(&morsel);
        }
        let num_matches = matches.as_ref().map_or(0, std::vec::Vec::len);
        if *listed && num_matches < 2 {
            self.refresh_window(refresh);
            *listed = false;
        }
        let mut matches = match matches {
            Some(m) if !m.is_empty() => m,
            _ => {
                self.term.beep();
                return morsel;
            }
        };
        let lastslash = revstrstr(&morsel, b"/", *place);
        let length_of_path = lastslash.map_or(0, |i| i + 1);
        let mut common_len = 0usize;
        loop {
            if common_len >= matches[0].len() {
                break;
            }
            let len1 = char_length(&matches[0][common_len..]);
            let c1 = &matches[0][common_len..common_len + len1];
            let mut all_same = true;
            for m in matches.iter().skip(1) {
                if m.len() < common_len + len1 || &m[common_len..common_len + len1] != c1 {
                    all_same = false;
                    break;
                }
            }
            if !all_same {
                break;
            }
            common_len += len1;
        }
        let mut shared: Vec<u8> = morsel[..length_of_path].to_vec();
        shared.extend_from_slice(&matches[0][..common_len]);
        common_len += length_of_path;
        let mut glued = self.present_path.clone();
        glued.extend_from_slice(&shared);
        if num_matches == 1 && (self.is_dir(&shared) || self.is_dir(&glued)) {
            shared.push(b'/');
            common_len += 1;
        }
        if common_len != *place {
            morsel = shared[..common_len].to_vec();
            *place = common_len;
        } else if num_matches == 1 {
            self.term.beep();
        }
        if num_matches > 1 {
            let lastrow =
                self.editwinrows - 1 - usize::from(self.isset(Flag::Zero) && self.lines() > 1);
            if !*listed {
                self.term.beep();
            }
            diralphasort(&mut matches);
            let mut longest_name = 0usize;
            for m in &matches {
                longest_name = longest_name.max(self.breadth(m));
            }
            let cols = self.cols();
            if longest_name > cols - 1 {
                longest_name = cols - 1;
            }
            let ncols = (cols + 1) / (longest_name + 2);
            let nrows = num_matches.div_ceil(ncols);
            let mut row = lastrow.saturating_sub(nrows);
            self.blank_edit();
            self.term.curs_set(false);
            for (i, m) in matches.iter().enumerate() {
                self.term
                    .wmove(Win::Mid, row, (longest_name + 2) * (i % ncols));
                if row == lastrow && (i + 1) % ncols == 0 && i + 1 < num_matches {
                    self.term.waddstr(Win::Mid, "(mehr)");
                    break;
                }
                let disp = self.display_string(m, 0, longest_name, false, false);
                self.term.waddbytes(Win::Mid, &disp);
                if (i + 1) % ncols == 0 {
                    row += 1;
                }
            }
            self.term.wnoutrefresh(Win::Mid);
            *listed = true;
        }
        morsel
    }
}

fn has_extended_attributes(path: &[u8]) -> bool {
    #[cfg(target_os = "linux")]
    {
        std::ffi::CString::new(path.to_vec()).map_or(true, |c| unsafe {
            libc::listxattr(c.as_ptr(), std::ptr::null_mut(), 0) != 0
        })
    }
    #[cfg(target_os = "macos")]
    {
        std::ffi::CString::new(path.to_vec()).map_or(true, |c| unsafe {
            libc::listxattr(c.as_ptr(), std::ptr::null_mut(), 0, 0) != 0
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = path;
        true
    }
}

fn overwrite_with_zeros(file: &fs::File, from: u64, to: u64) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;
    let zeros = vec![0u8; 64 * 1024];
    let mut pos = from;
    while pos < to {
        let n = (usize::try_from(to - pos).unwrap_or(usize::MAX)).min(zeros.len());
        file.write_all_at(&zeros[..n], pos)?;
        pos += n as u64;
    }
    file.sync_data()
}

fn plaintext_still_there(origin: &PlainOrigin) -> bool {
    let Ok(md) = fs::metadata(bytes_to_path(&origin.path)) else {
        return false;
    };
    if md.dev() != origin.dev || md.ino() != origin.ino || md.len() == 0 {
        return false;
    }
    let mut magic = [0u8; 4];
    fs::File::open(bytes_to_path(&origin.path))
        .and_then(|mut f| f.read(&mut magic))
        .is_ok_and(|n| !crypto::is_zu_file(&magic[..n]))
}

pub fn get_next_filename(name: &[u8], suffix: &[u8]) -> Vec<u8> {
    let mut base = name.to_vec();
    base.extend_from_slice(suffix);
    let mut i = 0u64;
    loop {
        let mut buf = base.clone();
        if i > 0 {
            buf.extend_from_slice(format!(".{i}").as_bytes());
        }
        if fs::symlink_metadata(bytes_to_path(&buf)).is_err() {
            return buf;
        }
        i += 1;
        if i == 100_000 {
            return Vec::new();
        }
    }
}

pub fn write_atomically(realname: &[u8], data: &[u8]) -> std::io::Result<bool> {
    use std::os::unix::io::AsRawFd;
    let target: Vec<u8> = if let Ok(p) = fs::canonicalize(bytes_to_path(realname)) {
        path_to_bytes(&p)
    } else {
        if fs::symlink_metadata(bytes_to_path(realname)).is_ok() {
            return Ok(false);
        }
        realname.to_vec()
    };
    let existing = fs::symlink_metadata(bytes_to_path(&target)).ok();
    let mut old_plaintext: Option<(fs::File, u64)> = None;
    if let Some(md) = &existing {
        if !md.file_type().is_file() || md.nlink() != 1 || has_extended_attributes(&target) {
            return Ok(false);
        }
        let Ok(c) = std::ffi::CString::new(target.clone()) else {
            return Ok(false);
        };
        if unsafe { libc::access(c.as_ptr(), libc::W_OK) } != 0 {
            return Ok(false);
        }
        if let Ok(mut old) = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(bytes_to_path(&target))
        {
            let mut magic = [0u8; 4];
            let got = old.read(&mut magic).unwrap_or(0);
            if md.len() > 0 && !crypto::is_zu_file(&magic[..got]) {
                old_plaintext = Some((old, md.len()));
            }
        }
    }
    let mut nonce = [0u8; 8];
    crypto::fill_random(&mut nonce).map_err(|e| std::io::Error::other(e.message()))?;
    let suffix = nonce.iter().fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    });
    let slash = target.iter().rposition(|&c| c == b'/').map_or(0, |i| i + 1);
    let mut tempname = target[..slash].to_vec();
    tempname.push(b'.');
    tempname.extend_from_slice(&target[slash..]);
    tempname.extend_from_slice(format!(".zu-{suffix}").as_bytes());
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(bytes_to_path(&tempname))
    {
        Ok(f) => f,
        Err(e) => {
            return match e.raw_os_error() {
                Some(
                    libc::EACCES | libc::EPERM | libc::EROFS | libc::EEXIST | libc::ENAMETOOLONG,
                ) => Ok(false),
                _ => Err(e),
            };
        }
    };
    let outcome: std::io::Result<bool> = (|| {
        if let Some(md) = &existing {
            let mine = file.metadata()?;
            if (mine.uid() != md.uid() || mine.gid() != md.gid())
                && unsafe { libc::fchown(file.as_raw_fd(), md.uid(), md.gid()) } != 0
            {
                return Ok(false);
            }
            file.set_permissions(fs::Permissions::from_mode(md.mode() & 0o7777))?;
        }
        file.write_all(data)?;
        file.sync_all()?;
        fs::rename(bytes_to_path(&tempname), bytes_to_path(&target))?;
        Ok(true)
    })();
    drop(file);
    match outcome {
        Ok(true) => {
            let dir: &[u8] = if slash > 0 { &target[..slash] } else { b"." };
            if let Ok(d) = fs::File::open(bytes_to_path(dir)) {
                let _ = d.sync_all();
            }
            if let Some((old, len)) = old_plaintext {
                let _ = overwrite_with_zeros(&old, 0, len);
            }
            Ok(true)
        }
        other => {
            let _ = fs::remove_file(bytes_to_path(&tempname));
            other
        }
    }
}

pub fn diralphasort(list: &mut [Vec<u8>]) {
    list.sort_by(|a, b| {
        let aisdir = is_dir_path(a);
        let bisdir = is_dir_path(b);
        if aisdir && !bisdir {
            return std::cmp::Ordering::Less;
        }
        if !aisdir && bisdir {
            return std::cmp::Ordering::Greater;
        }
        match mbstrcasecmp(a, b).cmp(&0) {
            std::cmp::Ordering::Equal => a.cmp(b),
            order => order,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("zu-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn password_may_not_repeat_the_file_name() {
        let target = b"/home/nutzer/Tagebuch-2026.zu";
        assert!(Zu::password_names_the_file(b"tagebuch-2026.zu", target));
        assert!(Zu::password_names_the_file(b"TAGEBUCH-2026", target));
        assert!(!Zu::password_names_the_file(
            b"Tagebuch-2026-geheim",
            target
        ));
        assert!(!Zu::password_names_the_file(b"", b""));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn in_place_overwrite_leaves_no_remainder() {
        let dir = scratch("inplace");
        let path = dir.join("f");
        fs::write(&path, b"KLARTEXT ".repeat(200)).unwrap();
        let f = fs::OpenOptions::new().write(true).open(&path).unwrap();
        Zu::reserve_space(&f, 10).unwrap();
        Zu::overwrite_in_place(&f, b"0123456789").unwrap();
        drop(f);
        assert_eq!(fs::read(&path).unwrap(), b"0123456789");
        let f = fs::OpenOptions::new().write(true).open(&path).unwrap();
        Zu::overwrite_in_place(&f, &[7u8; 5000]).unwrap();
        drop(f);
        assert_eq!(fs::read(&path).unwrap(), vec![7u8; 5000]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn zeros_overwrite_the_range() {
        let dir = scratch("zeros");
        let path = dir.join("f");
        fs::write(&path, b"abcdefghij").unwrap();
        let f = fs::OpenOptions::new().write(true).open(&path).unwrap();
        overwrite_with_zeros(&f, 2, 8).unwrap();
        drop(f);
        assert_eq!(fs::read(&path).unwrap(), b"ab\0\0\0\0\0\0ij");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn plaintext_origin_is_recognised() {
        let dir = scratch("origin");
        let path = dir.join("klar.txt");
        fs::write(&path, b"geheim").unwrap();
        let md = fs::metadata(&path).unwrap();
        let origin = PlainOrigin {
            shown: b"klar.txt".to_vec(),
            path: path_to_bytes(&path),
            dev: md.dev(),
            ino: md.ino(),
        };
        assert!(plaintext_still_there(&origin));
        fs::write(&path, b"\x2F\x01...").unwrap();
        assert!(!plaintext_still_there(&origin));
        fs::remove_file(&path).unwrap();
        assert!(!plaintext_still_there(&origin));
        fs::write(&path, b"anderer Klartext").unwrap();
        let fresh = fs::metadata(&path).unwrap();
        if fresh.ino() != md.ino() {
            assert!(!plaintext_still_there(&origin));
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
