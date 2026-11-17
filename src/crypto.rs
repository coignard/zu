use std::alloc::{GlobalAlloc, Layout, System};
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering, compiler_fence};

use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use ctutils::CtEq;
use zeroize::{Zeroize, Zeroizing};

pub const MAGIC: u8 = 47;
pub const FORMAT_VERSION: u8 = 1;
pub const HEADER_LEN: usize = 1 + 1 + 12 + SALT_LEN + NONCE_LEN;
pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;

pub const MIN_MEMORY_MIB: u32 = 64;
pub const MAX_MEMORY_MIB: u32 = 4096;
pub const MIN_PASSES: u32 = 3;
pub const MAX_PASSES: u32 = 100;
pub const LANES: u32 = 4;
const MAX_LANES: u32 = 64;
pub const MIN_PASSWORD_CHARS: usize = 15;

const COMMON_PASSWORDS: &str = include_str!("../assets/common-passwords.txt");

pub fn password_is_common(password: &[u8]) -> bool {
    let lowered = Zeroizing::new(String::from_utf8_lossy(password).to_lowercase());
    COMMON_PASSWORDS
        .lines()
        .collect::<Vec<_>>()
        .binary_search(&lowered.as_str())
        .is_ok()
}

pub fn same_password(first: &[u8], second: &[u8]) -> bool {
    first.ct_eq(second).to_bool()
}

#[derive(Debug)]
pub enum CryptoError {
    NotZu,
    Corrupt,
    BadPassword,
    Random,
    Kdf(&'static str),
}

impl CryptoError {
    pub fn message(&self) -> String {
        match self {
            Self::NotZu => "Keine Zu-Datei".to_string(),
            Self::Corrupt => "Dateivorspann beschädigt".to_string(),
            Self::BadPassword => "Falsches Passwort oder beschädigte Datei".to_string(),
            Self::Random => "Keine Zufallsquelle".to_string(),
            Self::Kdf(why) => format!("Schlüsselableitung fehlgeschlagen ({why})"),
        }
    }
}

const fn kdf_error(e: argon2::Error) -> CryptoError {
    use argon2::Error as E;
    CryptoError::Kdf(match e {
        E::MemoryTooLittle | E::MemoryTooMuch => "ungültiger Speicherbedarf",
        E::TimeTooSmall => "ungültige Durchlaufzahl",
        E::ThreadsTooFew | E::ThreadsTooMany => "ungültige Spurenzahl",
        E::PwdTooLong => "Passwort zu lang",
        E::SaltTooShort | E::SaltTooLong => "ungültiges Salz",
        _ => "interner Fehler",
    })
}

pub const fn is_zu_file(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == MAGIC && data[1] == FORMAT_VERSION
}

pub fn fill_random(buffer: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(buffer).map_err(|_| CryptoError::Random)
}

pub fn derive_key(
    password: &[u8],
    salt: &[u8; SALT_LEN],
    kdf: KdfParams,
) -> Result<SecretKey, CryptoError> {
    let params =
        Params::new(kdf.mem_kib, kdf.passes, kdf.lanes, Some(KEY_LEN)).map_err(kdf_error)?;
    let blocks = params.block_count();
    let mut memory: Vec<Block> = Vec::new();
    memory
        .try_reserve_exact(blocks)
        .map_err(|_| CryptoError::Kdf("nicht genug Speicher"))?;
    memory.resize(blocks, Block::default());
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key: SecretKey = Zeroizing::new(vec![0u8; KEY_LEN].into_boxed_slice());
    lock_region(key.as_ptr(), key.len());
    let result =
        argon.hash_password_into_with_memory(password, salt, &mut key[..], &mut memory[..]);
    memory.zeroize();
    result.map_err(kdf_error)?;
    Ok(key)
}

pub fn new_key_material(password: &[u8], kdf: KdfParams) -> Result<KeyMaterial, CryptoError> {
    let mut salt = [0u8; SALT_LEN];
    fill_random(&mut salt)?;
    let key = derive_key(password, &salt, kdf)?;
    Ok(KeyMaterial { key, salt, kdf })
}

fn build_header(km: &KeyMaterial, nonce: &[u8; NONCE_LEN]) -> Vec<u8> {
    let mut h = Vec::with_capacity(HEADER_LEN);
    h.push(MAGIC);
    h.push(FORMAT_VERSION);
    h.extend_from_slice(&km.kdf.mem_kib.to_le_bytes());
    h.extend_from_slice(&km.kdf.passes.to_le_bytes());
    h.extend_from_slice(&km.kdf.lanes.to_le_bytes());
    h.extend_from_slice(&km.salt);
    h.extend_from_slice(nonce);
    h
}

fn cipher_for(key: &[u8]) -> Result<XChaCha20Poly1305, CryptoError> {
    let key: &Key = key.try_into().map_err(|_| CryptoError::Corrupt)?;
    Ok(XChaCha20Poly1305::new(key))
}

pub fn encrypt(km: &KeyMaterial, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce)?;
    let header = build_header(km, &nonce);
    let ciphertext = cipher_for(&km.key)?
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad: &header,
            },
        )
        .map_err(|_| CryptoError::Corrupt)?;
    let mut out = header;
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn le_u32(data: &[u8], at: usize) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&data[at..at + 4]);
    u32::from_le_bytes(bytes)
}

pub fn parse_header(
    data: &[u8],
) -> Result<(KdfParams, [u8; SALT_LEN], [u8; NONCE_LEN]), CryptoError> {
    if !is_zu_file(data) {
        return Err(CryptoError::NotZu);
    }
    if data.len() < HEADER_LEN + TAG_LEN {
        return Err(CryptoError::Corrupt);
    }
    let kdf = KdfParams {
        mem_kib: le_u32(data, 2),
        passes: le_u32(data, 6),
        lanes: le_u32(data, 10),
    };
    if !(MIN_MEMORY_MIB * 1024..=MAX_MEMORY_MIB * 1024).contains(&kdf.mem_kib)
        || !(MIN_PASSES..=MAX_PASSES).contains(&kdf.passes)
        || !(1..=MAX_LANES).contains(&kdf.lanes)
    {
        return Err(CryptoError::Corrupt);
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&data[14..14 + SALT_LEN]);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&data[14 + SALT_LEN..HEADER_LEN]);
    Ok((kdf, salt, nonce))
}

pub fn decrypt_with_key(data: &[u8], key: &SecretKey) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let (_, _, nonce) = parse_header(data)?;
    let plaintext = cipher_for(key)?
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &data[HEADER_LEN..],
                aad: &data[..HEADER_LEN],
            },
        )
        .map_err(|_| CryptoError::BadPassword)?;
    Ok(Zeroizing::new(plaintext))
}

pub fn decrypt(
    data: &[u8],
    password: &[u8],
) -> Result<(Zeroizing<Vec<u8>>, KeyMaterial), CryptoError> {
    let (kdf, salt, _) = parse_header(data)?;
    let key = derive_key(password, &salt, kdf)?;
    let plaintext = decrypt_with_key(data, &key)?;
    Ok((plaintext, KeyMaterial { key, salt, kdf }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub mem_kib: u32,
    pub passes: u32,
    pub lanes: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            mem_kib: MIN_MEMORY_MIB * 1024,
            passes: MIN_PASSES,
            lanes: LANES,
        }
    }
}

pub type SecretKey = Zeroizing<Box<[u8]>>;

pub struct KeyMaterial {
    pub key: SecretKey,
    pub salt: [u8; SALT_LEN],
    pub kdf: KdfParams,
}

impl Clone for KeyMaterial {
    fn clone(&self) -> Self {
        let key: SecretKey = self.key.clone();
        lock_region(key.as_ptr(), key.len());
        Self {
            key,
            salt: self.salt,
            kdf: self.kdf,
        }
    }
}

#[derive(Clone)]
pub struct PlainOrigin {
    pub shown: Vec<u8>,
    pub path: Vec<u8>,
    pub dev: u64,
    pub ino: u64,
}

impl Drop for PlainOrigin {
    fn drop(&mut self) {
        self.shown.zeroize();
        self.path.zeroize();
    }
}

pub struct ZeroizingAllocator;

#[inline(never)]
unsafe fn wipe(p: *mut u8, size: usize) {
    let bytes = unsafe { std::slice::from_raw_parts_mut(p.cast::<MaybeUninit<u8>>(), size) };
    let (head, words, tail) = unsafe { bytes.align_to_mut::<MaybeUninit<u64>>() };
    for word in words {
        unsafe { ptr::write_volatile(word.as_mut_ptr(), 0) };
    }
    for byte in head.iter_mut().chain(tail.iter_mut()) {
        unsafe { ptr::write_volatile(byte.as_mut_ptr(), 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

unsafe impl GlobalAlloc for ZeroizingAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc_zeroed(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe {
            wipe(p, layout.size());
            System.dealloc(p, layout);
        }
    }

    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe {
            let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
            let fresh = System.alloc(new_layout);
            if !fresh.is_null() {
                ptr::copy_nonoverlapping(p, fresh, layout.size().min(new_size));
                wipe(p, layout.size());
                System.dealloc(p, layout);
            }
            fresh
        }
    }
}

pub fn lock_region(p: *const u8, len: usize) {
    if len > 0 && !MEMORY_LOCKED.load(Ordering::SeqCst) {
        unsafe {
            libc::mlock(p.cast::<libc::c_void>(), len);
        }
    }
}

pub fn raw_write(fd: i32, mut data: &[u8]) {
    while !data.is_empty() {
        let n = unsafe { libc::write(fd, data.as_ptr().cast::<libc::c_void>(), data.len()) };
        match n.cmp(&0) {
            std::cmp::Ordering::Greater => data = &data[n.cast_unsigned()..],
            std::cmp::Ordering::Equal => return,
            std::cmp::Ordering::Less => match std::io::Error::last_os_error().kind() {
                std::io::ErrorKind::Interrupted => {}
                std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                _ => return,
            },
        }
    }
}

pub static MEMORY_LOCKED: AtomicBool = AtomicBool::new(false);

pub fn harden_process() {
    unsafe {
        let no_core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &raw const no_core);

        #[cfg(target_os = "linux")]
        {
            libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
        }

        #[cfg(target_os = "macos")]
        {
            libc::ptrace(libc::PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0);
        }

        let mut lim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_MEMLOCK, &raw mut lim) == 0 {
            if lim.rlim_cur != lim.rlim_max {
                lim.rlim_cur = lim.rlim_max;
                libc::setrlimit(libc::RLIMIT_MEMLOCK, &raw const lim);
            }
            if (lim.rlim_max == libc::RLIM_INFINITY || memlock_limit_is_waived())
                && libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) == 0
            {
                MEMORY_LOCKED.store(true, Ordering::SeqCst);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn memlock_limit_is_waived() -> bool {
    const CAP_IPC_LOCK: u32 = 14;
    const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
    #[repr(C)]
    struct Header {
        version: u32,
        pid: libc::c_int,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let mut header = Header {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let mut data = [Data {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    let r = unsafe { libc::syscall(libc::SYS_capget, &raw mut header, data.as_mut_ptr()) };
    if r != 0 || data[0].effective & (1 << CAP_IPC_LOCK) == 0 {
        return false;
    }
    std::fs::read_to_string("/proc/self/uid_map").is_ok_and(|map| {
        let fields: Vec<&str> = map.split_whitespace().collect();
        fields == ["0", "0", "4294967295"]
    })
}

#[cfg(not(target_os = "linux"))]
const fn memlock_limit_is_waived() -> bool {
    false
}

#[cfg(target_os = "macos")]
#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn EnableSecureEventInput() -> i32;
    fn DisableSecureEventInput() -> i32;
}

#[cfg(target_os = "macos")]
pub fn secure_keyboard(on: bool) -> bool {
    let status = unsafe {
        if on {
            EnableSecureEventInput()
        } else {
            DisableSecureEventInput()
        }
    };
    status == 0
}

#[cfg(not(target_os = "macos"))]
pub const fn secure_keyboard(_on: bool) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const KDF: KdfParams = KdfParams {
        mem_kib: MIN_MEMORY_MIB * 1024,
        passes: MIN_PASSES,
        lanes: LANES,
    };

    const FORMAT_KEY: &str = "035e7f3e26fe74615bd228227495028a502e9a337ce1c5b023164f29ce05b930";

    const SODIUM_VECTOR: &str = concat!(
        "2f010000010003000000040000005a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a0001",
        "02030405060708090a0b0c0d0e0f1011121314151617771856cd27a2045fff3a",
        "8385a6009c09cf3eda7160fc3e84665965f852",
    );

    fn fixed_key_material() -> KeyMaterial {
        KeyMaterial {
            key: Zeroizing::new(vec![0x42; KEY_LEN].into_boxed_slice()),
            salt: [0x5A; SALT_LEN],
            kdf: KDF,
        }
    }

    const FORMAT_VECTOR: &str = concat!(
        "2f010000010003000000040000005a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5aa5a5",
        "a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5c98583d3d84c6908a5b2",
        "7414e6466f20ba80fa192547a9546d1edba2d27e76d2e8140551c22e25deb308",
    );

    #[test]
    #[cfg_attr(miri, ignore)]
    fn common_passwords_are_refused() {
        let list: Vec<&str> = COMMON_PASSWORDS.lines().collect();
        assert!(list.len() >= 3000);
        assert!(list.windows(2).all(|w| w[0] < w[1]));
        assert!(list.iter().all(|p| p.chars().count() >= MIN_PASSWORD_CHARS));
        assert!(password_is_common(b"1q2w3e4r5t6y7u8i"));
        assert!(password_is_common(b"1Q2W3E4R5T6Y7U8I"));
        assert!(!password_is_common(b"Kohlrabi-Gewitter-17"));
        assert!(!password_is_common("Gänseblümchenwiese".as_bytes()));
    }

    #[test]
    fn passwords_are_compared_completely() {
        let password = b"Kohlrabi-Gewitter-17";
        assert!(same_password(password, password));
        assert!(!same_password(password, b"Kohlrabi-Gewitter-18"));
        assert!(!same_password(password, &password[..19]));
        assert!(!same_password(&password[..19], password));
        assert!(!same_password(b"", b"K"));
        assert!(same_password(b"", b""));
    }

    #[test]
    fn defaults_follow_rfc_9106() {
        let kdf = KdfParams::default();
        assert_eq!((kdf.mem_kib, kdf.passes, kdf.lanes), (64 * 1024, 3, 4));
        assert_eq!((SALT_LEN, KEY_LEN), (16, 32));
    }

    #[test]
    fn kdf_parameters_out_of_range_are_rejected() {
        let km = fixed_key_material();
        let file = encrypt(&km, b"x").unwrap();
        assert!(parse_header(&file).is_ok());
        for (at, value) in [
            (2, MAX_MEMORY_MIB * 1024 + 1),
            (2, MIN_MEMORY_MIB * 1024 - 1),
            (2, 8),
            (6, MAX_PASSES + 1),
            (6, MIN_PASSES - 1),
            (6, 1),
            (10, 0),
            (10, MAX_LANES + 1),
        ] {
            let mut bad = file.clone();
            bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(
                matches!(parse_header(&bad), Err(CryptoError::Corrupt)),
                "{at}: {value}"
            );
        }
    }

    #[test]
    fn fixed_test_vector_decrypts_with_its_key() {
        let file = hex(FORMAT_VECTOR);
        let key: SecretKey = Zeroizing::new(hex(FORMAT_KEY).into_boxed_slice());
        assert_eq!(
            &decrypt_with_key(&file, &key).unwrap()[..],
            "zu 0.1.0: Klartext äöü\n".as_bytes()
        );
    }

    #[test]
    fn libsodium_file_decrypts() {
        let key: SecretKey = Zeroizing::new(hex(FORMAT_KEY).into_boxed_slice());
        assert_eq!(
            &decrypt_with_key(&hex(SODIUM_VECTOR), &key).unwrap()[..],
            b"aus libsodium"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn fixed_test_vector_key_is_derived() {
        let key = derive_key(
            "Gänseblümchen".as_bytes(),
            &[0x5A; SALT_LEN],
            KdfParams::default(),
        );
        assert_eq!(&key.unwrap()[..], &hex(FORMAT_KEY)[..]);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn fixed_test_vector_decrypts() {
        let file = hex(FORMAT_VECTOR);
        let (kdf, salt, nonce) = parse_header(&file).unwrap();
        assert_eq!(kdf, KdfParams::default());
        assert_eq!(salt, [0x5A; SALT_LEN]);
        assert_eq!(nonce, [0xA5; NONCE_LEN]);
        let (plain, km) = decrypt(&file, "Gänseblümchen".as_bytes()).unwrap();
        assert_eq!(&plain[..], "zu 0.1.0: Klartext äöü\n".as_bytes());
        assert_eq!(km.salt, salt);
        assert!(matches!(
            decrypt(&file, b"Gaenseblumchen"),
            Err(CryptoError::BadPassword)
        ));
    }

    #[test]
    fn arbitrary_input_never_panics() {
        let km = fixed_key_material();
        let file = encrypt(&km, b"Inhalt").unwrap();
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for round in 0..if cfg!(miri) { 40 } else { 4000 } {
            let mut data = if round % 2 == 0 {
                file.clone()
            } else {
                let len = usize::try_from(next() % 160).unwrap();
                (0..len).map(|_| next().to_le_bytes()[0]).collect()
            };
            if !data.is_empty() {
                for _ in 0..=next() % 4 {
                    let at = usize::try_from(next()).unwrap() % data.len();
                    data[at] = next().to_le_bytes()[0];
                }
                data.truncate(usize::try_from(next()).unwrap() % (data.len() + 1));
            }
            let _ = parse_header(&data);
            let _ = decrypt_with_key(&data, &km.key);
        }
    }

    #[test]
    fn wipe_clears_exactly_the_range_at_every_alignment() {
        let mut buffer = [0xAAu8; 96];
        for start in 0..17 {
            for len in 0..40 {
                buffer.fill(0xAA);
                unsafe { wipe(buffer.as_mut_ptr().add(start), len) };
                for (at, &byte) in buffer.iter().enumerate() {
                    let inside = (start..start + len).contains(&at);
                    assert_eq!(byte, if inside { 0 } else { 0xAA }, "{start}+{len} at {at}");
                }
            }
        }
    }

    #[test]
    fn allocator_moves_and_frees_blocks() {
        let mut grow: Vec<u8> = Vec::new();
        for n in 0..300u16 {
            grow.extend_from_slice(&n.to_le_bytes());
        }
        let copy = grow.clone();
        grow.shrink_to_fit();
        assert_eq!(grow, copy);
        let layout = Layout::from_size_align(64, 32).unwrap();
        unsafe {
            let p = ZeroizingAllocator.alloc(layout);
            assert!(!p.is_null());
            p.write_bytes(0x5A, 64);
            let q = ZeroizingAllocator.realloc(p, layout, 200);
            assert!(!q.is_null());
            assert!(std::slice::from_raw_parts(q, 64).iter().all(|&b| b == 0x5A));
            ZeroizingAllocator.dealloc(q, Layout::from_size_align(200, 32).unwrap());
        }
    }

    fn hex(text: &str) -> Vec<u8> {
        let digits: Vec<u8> = text.bytes().filter(u8::is_ascii_hexdigit).collect();
        digits
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn round_trip_with_key() {
        let km = fixed_key_material();
        let file = encrypt(&km, "Klartext äöü\n".as_bytes()).unwrap();
        assert!(is_zu_file(&file));
        assert!(!file.windows(8).any(|w| w == b"Klartext"));
        assert_eq!(
            &decrypt_with_key(&file, &km.key).unwrap()[..],
            "Klartext äöü\n".as_bytes()
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn round_trip() {
        let km = new_key_material(b"geheimnis", KDF).unwrap();
        let file = encrypt(&km, "Klartext äöü\n".as_bytes()).unwrap();
        assert!(is_zu_file(&file));
        assert!(!file.windows(8).any(|w| w == b"Klartext"));
        let (pt, km2) = decrypt(&file, b"geheimnis").unwrap();
        assert_eq!(&pt[..], "Klartext äöü\n".as_bytes());
        assert_eq!(km2.salt, km.salt);
        assert_eq!(&decrypt_with_key(&file, &km.key).unwrap()[..], &pt[..]);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn empty_plaintext() {
        let km = new_key_material(b"xxxxxxxx", KDF).unwrap();
        let file = encrypt(&km, b"").unwrap();
        assert_eq!(file.len(), HEADER_LEN + 16);
        assert!(decrypt(&file, b"xxxxxxxx").unwrap().0.is_empty());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn wrong_password_is_rejected() {
        let km = new_key_material(b"richtig", KDF).unwrap();
        let file = encrypt(&km, b"Inhalt").unwrap();
        assert!(matches!(
            decrypt(&file, b"falsch"),
            Err(CryptoError::BadPassword)
        ));
    }

    #[test]
    fn every_modification_is_detected() {
        let km = fixed_key_material();
        let file = encrypt(&km, b"Inhalt der Datei").unwrap();
        for i in 0..file.len() {
            let mut bad = file.clone();
            bad[i] ^= 0x01;
            assert!(
                decrypt_with_key(&bad, &km.key).is_err(),
                "Byte {i} nicht gesichert"
            );
        }
        assert!(decrypt_with_key(&file[..file.len() - 1], &km.key).is_err());
    }

    #[test]
    fn every_save_has_its_own_nonce() {
        let km = fixed_key_material();
        let a = encrypt(&km, b"gleich").unwrap();
        let b = encrypt(&km, b"gleich").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn header_is_checked() {
        assert!(matches!(
            parse_header(b"Klartextdatei"),
            Err(CryptoError::NotZu)
        ));
        assert!(matches!(
            parse_header(b"\x2F\x01kurz"),
            Err(CryptoError::Corrupt)
        ));
        assert!(matches!(
            parse_header("// Klartext, der mit einem Schrägstrich beginnt".as_bytes()),
            Err(CryptoError::NotZu)
        ));
        assert!(!is_zu_file(b"/\n"));
        assert!(!is_zu_file(b"/"));
        let km = fixed_key_material();
        let mut file = encrypt(&km, b"x").unwrap();
        assert_eq!(&file[..2], &[47, FORMAT_VERSION]);
        assert_eq!(file.len(), 54 + 1 + 16);
        file[1] = 9;
        assert!(matches!(parse_header(&file), Err(CryptoError::NotZu)));
        file[1] = FORMAT_VERSION;
        file[2..6].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(parse_header(&file), Err(CryptoError::Corrupt)));
    }
}
