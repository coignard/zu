use crate::crypto::{KeyMaterial, PlainOrigin};
use zeroize::Zeroize;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BRANDING: &str = concat!("zu ", env!("CARGO_PKG_VERSION"));

pub const BACKWARD: bool = false;
pub const FORWARD: bool = true;

pub const YESORNO: bool = false;
pub const YESORALLORNO: bool = true;

pub const YES: i32 = 1;
pub const ALL: i32 = 2;
pub const NO: i32 = 0;
pub const CANCEL: i32 = -1;

pub const BLIND: bool = false;
pub const VISIBLE: bool = true;

pub const JUSTFIND: i32 = 0;
pub const REPLACING: i32 = 1;
pub const INREGION: i32 = 2;

pub const NONOTES: bool = false;

pub const PRUNE_DUPLICATE: bool = true;

pub const WIDTH_OF_TAB: isize = 8;
pub const COLUMNS_FROM_EOL: isize = 8;
pub const CUSHION: usize = 3;
pub const GENERAL_COMMENT_CHARACTER: &[u8] = b"#";
pub const MAX_SEARCH_HISTORY: i64 = 100;
pub const HIGHEST_POSITIVE: usize = usize::MAX >> 1;

pub const ERR: i32 = -1;

pub const ESC_CODE: i32 = 0x1B;
pub const DEL_CODE: i32 = 0x7F;

pub const KEY_DOWN: i32 = 0x102;
pub const KEY_UP: i32 = 0x103;
pub const KEY_LEFT: i32 = 0x104;
pub const KEY_RIGHT: i32 = 0x105;
pub const KEY_HOME: i32 = 0x106;
pub const KEY_BACKSPACE: i32 = 0x107;
pub const KEY_F0: i32 = 0x108;
pub const KEY_DC: i32 = 0x14A;
pub const KEY_IC: i32 = 0x14B;
pub const KEY_NPAGE: i32 = 0x152;
pub const KEY_PPAGE: i32 = 0x153;
pub const KEY_ENTER: i32 = 0x157;
pub const KEY_CANCEL: i32 = 0x163;
pub const KEY_END: i32 = 0x168;
pub const KEY_MOUSE: i32 = 0x199;
#[allow(non_snake_case)]
pub const fn KEY_F(n: i32) -> i32 {
    KEY_F0 + n
}

pub const CONTROL_LEFT: i32 = 0x401;
pub const CONTROL_RIGHT: i32 = 0x402;
pub const CONTROL_UP: i32 = 0x403;
pub const CONTROL_DOWN: i32 = 0x404;
pub const CONTROL_HOME: i32 = 0x405;
pub const CONTROL_END: i32 = 0x406;
pub const CONTROL_DELETE: i32 = 0x40D;
pub const SHIFT_CONTROL_LEFT: i32 = 0x411;
pub const SHIFT_CONTROL_RIGHT: i32 = 0x412;
pub const SHIFT_CONTROL_UP: i32 = 0x413;
pub const SHIFT_CONTROL_DOWN: i32 = 0x414;
pub const SHIFT_CONTROL_HOME: i32 = 0x415;
pub const SHIFT_CONTROL_END: i32 = 0x416;
pub const CONTROL_SHIFT_DELETE: i32 = 0x41D;
pub const ALT_LEFT: i32 = 0x421;
pub const ALT_RIGHT: i32 = 0x422;
pub const ALT_UP: i32 = 0x423;
pub const ALT_DOWN: i32 = 0x424;
pub const ALT_HOME: i32 = 0x425;
pub const ALT_END: i32 = 0x426;
pub const ALT_PAGEUP: i32 = 0x427;
pub const ALT_PAGEDOWN: i32 = 0x428;
pub const ALT_INSERT: i32 = 0x42C;
pub const ALT_DELETE: i32 = 0x42D;
pub const SHIFT_ALT_LEFT: i32 = 0x431;
pub const SHIFT_ALT_RIGHT: i32 = 0x432;
pub const SHIFT_ALT_UP: i32 = 0x433;
pub const SHIFT_ALT_DOWN: i32 = 0x434;
pub const SHIFT_UP: i32 = 0x453;
pub const SHIFT_DOWN: i32 = 0x454;
pub const SHIFT_HOME: i32 = 0x455;
pub const SHIFT_END: i32 = 0x456;
pub const SHIFT_PAGEUP: i32 = 0x457;
pub const SHIFT_PAGEDOWN: i32 = 0x458;
pub const SHIFT_DELETE: i32 = 0x45D;
pub const SHIFT_TAB: i32 = 0x45F;
pub const SHIFT_LEFT: i32 = 0x451;
pub const SHIFT_RIGHT: i32 = 0x452;
pub const KEY_SIC: i32 = 0x15F;
pub const IGNORED_KEY: i32 = 0x2FD;
pub const UNBOUND_KEY: i32 = 0x2FF;

pub const START_OF_PASTE: i32 = 0x4B5;
pub const END_OF_PASTE: i32 = 0x4BE;

pub const MORE_PLANTS: i32 = 0x4EA;
pub const MISSING_BRACE: i32 = 0x4EB;
pub const PLANTED_A_COMMAND: i32 = 0x4EC;
pub const NO_SUCH_FUNCTION: i32 = 0x4EF;

pub const KEY_CENTER: i32 = 0x4F0;
pub const THE_WINDOW_RESIZED: i32 = 0x4F7;
pub const FOREIGN_SEQUENCE: i32 = 0x4FC;
pub const KEY_FRESH: i32 = 0x4FE;

pub const WAS_BACKSPACE_AT_EOF: u32 = 1 << 1;
pub const WAS_WHOLE_LINE: u32 = 1 << 2;
pub const INCLUDED_LAST_LINE: u32 = 1 << 3;
pub const MARK_WAS_SET: u32 = 1 << 4;
pub const CURSOR_WAS_AT_HEAD: u32 = 1 << 5;
pub const HAD_ANCHOR_AT_START: u32 = 1 << 6;

pub const MMAIN: u32 = 1 << 0;
pub const MWHEREIS: u32 = 1 << 1;
pub const MREPLACE: u32 = 1 << 2;
pub const MREPLACEWITH: u32 = 1 << 3;
pub const MGOTOLINE: u32 = 1 << 4;
pub const MWRITEFILE: u32 = 1 << 5;
pub const MINSERTFILE: u32 = 1 << 6;
pub const MEXECUTE: u32 = 1 << 7;
pub const MHELP: u32 = 1 << 8;
pub const MSPELL: u32 = 1 << 9;
pub const MBROWSER: u32 = 1 << 10;
pub const MWHEREISFILE: u32 = 1 << 11;
pub const MGOTODIR: u32 = 1 << 12;
pub const MYESNO: u32 = 1 << 13;
pub const MLINTER: u32 = 1 << 14;
pub const MFINDINHELP: u32 = 1 << 15;
pub const MPASSWORD: u32 = 1 << 16;
pub const MMOST: u32 = MMAIN
    | MWHEREIS
    | MREPLACE
    | MREPLACEWITH
    | MGOTOLINE
    | MWRITEFILE
    | MINSERTFILE
    | MEXECUTE
    | MWHEREISFILE
    | MGOTODIR
    | MFINDINHELP
    | MSPELL
    | MLINTER;
pub const MSOME: u32 = MMOST | MBROWSER;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Unspecified,
    Nix,
    Dos,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Msg {
    Vacuum,
    Hush,
    Remark,
    Info,
    Notice,
    Ahem,
    Mild,
    Alert,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Writing {
    Overwrite,
    Append,
    Prepend,
    Special,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Update {
    Centering,
    Flowing,
    Stationary,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum UndoType {
    Add,
    Enter,
    Back,
    Del,
    Join,
    Replace,
    SplitBegin,
    SplitEnd,
    Indent,
    Unindent,
    Comment,
    Uncomment,
    Preflight,
    Zap,
    Cut,
    CutToEof,
    Copy,
    Paste,
    Insert,
    CoupleBegin,
    CoupleEnd,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Elem {
    TitleBar = 0,
    LineNumber,
    GuideStripe,
    ScrollBar,
    SelectedText,
    Spotlighted,
    MiniInfobar,
    PromptBar,
    StatusBar,
    ErrorMessage,
    KeyCombo,
    FunctionTag,
}
pub const NUMBER_OF_ELEMENTS: usize = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
#[allow(dead_code)]
pub enum Flag {
    DontUse = 0,
    Zero,
    NoHelp,
    ConstantShow,
    LineNumbers,
    WhitespaceDisplay,
    NoSyntax,
    SoftWrap,
    BreakLongLines,
    TabsToSpaces,
    AutoIndent,
    SmartHome,
    CutFromCursor,
    UseMouse,
    CaseSensitive,
    UseRegexp,
    BackwardsSearch,
    ColonParsing,
    NoreadMode,
    Locking,
    Restricted,
    ViewMode,
    UseMagic,
    NewBuffer,
    NoConvert,
    NoNewlines,
    HistoryLog,
    PositionLog,
    MakeBackup,
    InsecureBackup,
    SaveOnExit,
    MakeItUnix,
    Preserve,
    RebindDelete,
    RawSequences,
    ModernBindings,
    LetThemZap,
    AfterEnds,
    WordBounds,
    AtBlanks,
    TrimBlanks,
    BookStyle,
    NoWrap,
    JumpyScrolling,
    SoloSidescroll,
    QuickBlank,
    ShowCursor,
    BoldText,
    EmptyLine,
    Indicator,
    StateFlags,
    MiniBar,
}

pub const TOGGLE_FLAGS: [Flag; 13] = [
    Flag::Zero,
    Flag::NoHelp,
    Flag::ConstantShow,
    Flag::LineNumbers,
    Flag::WhitespaceDisplay,
    Flag::NoSyntax,
    Flag::SoftWrap,
    Flag::BreakLongLines,
    Flag::TabsToSpaces,
    Flag::AutoIndent,
    Flag::SmartHome,
    Flag::CutFromCursor,
    Flag::UseMouse,
];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags(pub u64);

impl Flags {
    #[inline]
    pub const fn set(&mut self, f: Flag) {
        self.0 |= 1u64 << (f as u8);
    }
    #[inline]
    pub const fn unset(&mut self, f: Flag) {
        self.0 &= !(1u64 << (f as u8));
    }
    #[inline]
    pub const fn isset(self, f: Flag) -> bool {
        (self.0 >> (f as u8)) & 1 != 0
    }
    #[inline]
    pub const fn toggle(&mut self, f: Flag) {
        self.0 ^= 1u64 << (f as u8);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Func {
    Cancel,
    Help,
    Exit,
    DiscardBuffer,
    Writeout,
    Savefile,
    Insertfile,
    SearchForward,
    SearchBackward,
    FindPrevious,
    FindNext,
    Replace,
    CutText,
    CopyText,
    PasteText,
    Execute,
    CutTillEof,
    ZapText,
    Mark,
    Location,
    GotoLineColumn,
    Justify,
    FullJustify,
    ParaBegin,
    ParaEnd,
    Comment,
    Complete,
    Indent,
    Unindent,
    ChopPrevWord,
    ChopNextWord,
    FindBracket,
    WordCount,
    RecordMacro,
    RunMacro,
    Anchor,
    PrevAnchor,
    NextAnchor,
    Undo,
    Redo,
    Suspend,
    CursesVersion,
    Left,
    Right,
    Up,
    Down,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollDown,
    PrevWord,
    NextWord,
    Home,
    End,
    PrevBlock,
    NextBlock,
    TopRow,
    BottomRow,
    Center,
    Cycle,
    PageUp,
    PageDown,
    FirstLine,
    LastLine,
    PrevBuffer,
    NextBuffer,
    Verbatim,
    Tab,
    Enter,
    Delete,
    Backspace,
    FullRefresh,
    CaseSens,
    Regexp,
    Backwards,
    FlipReplace,
    FlipGoto,
    OlderItem,
    NewerItem,
    DosFormat,
    AppendIt,
    PrependIt,
    BackItUp,
    FlipExecute,
    FlipConvert,
    FlipNewbuffer,
    ToFiles,
    GotoDir,
    FirstFile,
    LastFile,
    Toggle,
    Implant,
    Nothing,
    SuggestCtrlTZ,
    SuckUpPaste,
    ChangePassword,
}

#[derive(Clone, Debug)]
pub struct Shortcut {
    pub keystr: String,
    pub keycode: i32,
    pub menus: u32,
    pub func: Func,
    pub toggle: Option<Flag>,
    pub expansion: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct FuncItem {
    pub func: Func,
    pub tag: &'static str,
    pub phrase: &'static str,
    pub blank_after: bool,
    pub menus: u32,
}

pub type Lp = usize;

pub fn tail_bytes(path: &[u8]) -> &[u8] {
    path.iter()
        .rposition(|&b| b == b'/')
        .map_or(path, |i| &path[i + 1..])
}

#[derive(Debug, Default)]
pub struct LineNode {
    pub data: Vec<u8>,
    pub lineno: i64,
    pub next: Option<Lp>,
    pub prev: Option<Lp>,
    pub has_anchor: bool,
    pub alive: bool,
}

impl Drop for LineNode {
    fn drop(&mut self) {
        self.data.zeroize();
    }
}

#[derive(Debug, Clone)]
pub struct Group {
    pub top_line: i64,
    pub bottom_line: i64,
    pub indentations: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub struct Undo {
    pub serial: u64,
    pub utype: UndoType,
    pub xflags: u32,
    pub head_lineno: i64,
    pub head_x: usize,
    pub strdata: Vec<u8>,
    pub wassize: usize,
    pub newsize: usize,
    pub grouping: Vec<Group>,
    pub cutbuffer: Option<Lp>,
    pub tail_lineno: i64,
    pub tail_x: usize,
}

impl Drop for Undo {
    fn drop(&mut self) {
        self.strdata.zeroize();
        for g in &mut self.grouping {
            for s in &mut g.indentations {
                s.zeroize();
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatInfo {
    pub dev: u64,
    pub ino: u64,
    pub mtime: i64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}

pub struct OpenFile {
    pub filename: Vec<u8>,
    pub filetop: Lp,
    pub filebot: Lp,
    pub edittop: Lp,
    pub current: Lp,
    pub totsize: usize,
    pub firstcolumn: usize,
    pub current_x: usize,
    pub placewewant: usize,
    pub brink: usize,
    pub cursor_row: i64,
    pub statinfo: Option<StatInfo>,
    pub spillage_line: Option<Lp>,
    pub mark: Option<Lp>,
    pub mark_x: usize,
    pub softmark: bool,
    pub fmt: Format,
    pub lock_filename: Option<Vec<u8>>,
    pub undo: Vec<Undo>,
    pub cur_undo: usize,
    pub last_saved: Option<u64>,
    pub last_action: UndoType,
    pub modified: bool,
    pub errormessage: Option<String>,
    pub keymat: Option<KeyMaterial>,
    pub plain_origin: Option<PlainOrigin>,
}

impl Drop for OpenFile {
    fn drop(&mut self) {
        self.filename.zeroize();
    }
}

#[derive(Debug, Clone, Default)]
pub struct History {
    pub items: Vec<Vec<u8>>,
    pub pos: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            items: vec![Vec::new()],
            pos: 0,
        }
    }
    pub const fn bottom(&self) -> usize {
        self.items.len() - 1
    }
    pub const fn reset(&mut self) {
        self.pos = self.bottom();
    }
    pub const fn at_bottom(&self) -> bool {
        self.pos == self.bottom()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HistKind {
    Search,
    Replace,
    Execute,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub filename: Vec<u8>,
    pub linenumber: i64,
    pub columnnumber: i64,
    pub anchors: Vec<u8>,
}

pub struct Completion {
    pub word: Vec<u8>,
}
