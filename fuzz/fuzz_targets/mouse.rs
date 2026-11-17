#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use zu_fuzz::curses::mouse::{ALL_MOUSE_EVENTS, Mouse};
use zu_fuzz::curses::terminfo::{PadInfo, TermInfo};

static ENTRIES: OnceLock<Vec<TermInfo>> = OnceLock::new();

fuzz_target!(|data: &[u8]| {
    let entries = ENTRIES.get_or_init(|| {
        ["xterm-256color", "screen", "xterm"]
            .iter()
            .filter_map(|name| TermInfo::load(name))
            .collect()
    });
    let Some((&choice, rest)) = data.split_first() else {
        return;
    };
    let Some(ti) = entries.get(usize::from(choice) % entries.len().max(1)) else {
        return;
    };
    let mut mouse = Mouse::new();
    mouse.configure(ti, PadInfo::default());
    let mut out = Vec::new();
    mouse.mousemask(ALL_MOUSE_EVENTS, &mut out);
    mouse.mouseinterval(i32::from(choice >> 4) * 20);
    let mut bytes = rest.iter().copied();
    for _ in 0..16 {
        if !mouse.inline_event(&mut || bytes.next()) {
            continue;
        }
        if mouse.parse(usize::from(choice & 7)) {
            while mouse.getmouse().is_some() {}
        }
        if bytes.len() == 0 {
            break;
        }
    }
});
