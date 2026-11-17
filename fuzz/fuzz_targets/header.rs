#![no_main]

use libfuzzer_sys::fuzz_target;
use zu_fuzz::crypto::{decrypt_with_key, is_zu_file, parse_header};

fuzz_target!(|data: &[u8]| {
    let key = zeroize::Zeroizing::new(vec![0x5A; 32].into_boxed_slice());
    if let Ok((kdf, _, _)) = parse_header(data) {
        assert!(is_zu_file(data));
        assert!((64 * 1024..=4096 * 1024).contains(&kdf.mem_kib));
        assert!((3..=100).contains(&kdf.passes));
        assert!((1..=64).contains(&kdf.lanes));
    }
    assert!(decrypt_with_key(data, &key).is_err());
});
