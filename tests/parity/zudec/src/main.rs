use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use std::io::Write;
use std::process::ExitCode;

const HEADER_LEN: usize = 54;

fn field(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn decrypt(data: &[u8], password: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < HEADER_LEN + 16 || data[..2] != [47, 1] {
        return Err("keine zu-Datei".into());
    }
    let memory = field(data, 2).ok_or("Vorspann zu kurz")?;
    let passes = field(data, 6).ok_or("Vorspann zu kurz")?;
    let lanes = field(data, 10).ok_or("Vorspann zu kurz")?;
    let params = Params::new(memory, passes, lanes, Some(32)).map_err(|e| e.to_string())?;
    let mut key = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password, &data[14..30], &mut key)
        .map_err(|e| e.to_string())?;
    let cipher = XChaCha20Poly1305::new(&key.into());
    let nonce: [u8; 24] = data[30..HEADER_LEN].try_into().map_err(|_| "Nonce fehlt")?;
    cipher
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &data[HEADER_LEN..],
                aad: &data[..HEADER_LEN],
            },
        )
        .map_err(|_| "Entschlüsselung fehlgeschlagen".into())
}

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.len() != 3 {
        eprintln!("Aufruf: zudec DATEI PASSWORT");
        return ExitCode::from(2);
    }
    let data = match std::fs::read(&args[1]) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match decrypt(&data, args[2].as_encoded_bytes()) {
        Ok(plain) => {
            if std::io::stdout().write_all(&plain).is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
