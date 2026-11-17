#![no_main]

use libfuzzer_sys::fuzz_target;
use zu_fuzz::curses::terminfo::{PadInfo, tparm, tputs};

fuzz_target!(|data: &[u8]| {
    let split = data.len().min(36);
    let (head, cap) = data.split_at(split);
    let params: Vec<i32> = head
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let expanded = tparm(cap, &params);
    for (always, baudrate) in [(false, 38400), (true, i32::MAX)] {
        let pad = PadInfo {
            xon_xoff: false,
            padding_baud_rate: -1,
            no_pad_char: false,
            pad_char: 0,
            baudrate,
            no_padding: false,
        };
        let mut out = Vec::new();
        tputs(&mut out, cap, always, &pad);
        let mut out = Vec::new();
        tputs(&mut out, &expanded, always, &pad);
    }
});
