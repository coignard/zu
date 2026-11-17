#![no_main]

use libfuzzer_sys::fuzz_target;
use zu_fuzz::curses::mouse::{ALL_MOUSE_EVENTS, Mouse};
use zu_fuzz::curses::terminfo::{KeyTable, PadInfo, TermInfo};

fuzz_target!(|data: &[u8]| {
    let Some(ti) = TermInfo::parse(data) else {
        return;
    };
    let table = KeyTable::build(&ti);
    for probe in [
        &b"\x1b[A"[..],
        b"\x1bOP",
        b"\x1b[1;5A",
        b"\x1b[<0;1;1M",
        b"\x1b[M !!",
        &data[..data.len().min(8)],
    ] {
        let _ = table.lookup(probe);
    }
    let _ = (
        ti.colors(),
        ti.has_colors(),
        ti.lines(),
        ti.cols(),
        ti.flag("xon"),
        ti.flag("npc"),
        ti.padding_baud_rate(),
        ti.names(),
        ti.extended_number("RGB"),
    );
    let pad = PadInfo::new(&ti, 38400, false);
    let mut mouse = Mouse::new();
    mouse.configure(
        &ti,
        PadInfo {
            no_pad_char: false,
            ..pad
        },
    );
    let mut out = Vec::new();
    mouse.mousemask(ALL_MOUSE_EVENTS, &mut out);
    mouse.mousemask(0, &mut out);
});
