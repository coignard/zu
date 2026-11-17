#![cfg(not(miri))]

use std::process::{Command, Output, Stdio};

fn zu(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zu"))
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::null())
        .output()
        .expect("zu should start")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn version_names_the_port_and_the_ciphers() {
    let out = zu(&["--version"]);
    assert!(out.status.success());
    let stdout = text(&out.stdout);
    assert!(stdout.starts_with(concat!(" zu, Version ", env!("CARGO_PKG_VERSION"), "\n")));
    assert!(stdout.contains("Argon2id"));
    assert!(stdout.contains("XChaCha20-Poly1305"));
}

#[test]
fn help_lists_the_key_derivation_options() {
    let out = zu(&["--help"]);
    assert!(out.status.success());
    let stdout = text(&out.stdout);
    assert!(stdout.contains("--argonmemory=<MiB>"));
    assert!(stdout.contains("--argonpasses=<Zahl>"));
    assert!(!stdout.contains("--syntax"));
}

#[test]
fn unknown_options_are_rejected_like_getopt() {
    let out = zu(&["--gibtsnicht"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("Unbekannte Option »--gibtsnicht«"));
    assert!(text(&out.stdout).contains("-h“ für eine Liste verfügbarer Optionen."));
}

#[test]
fn missing_arguments_are_rejected_like_getopt() {
    let out = zu(&["-r"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("Diese Option benötigt ein Argument -- »r«"));
}

#[test]
fn weak_key_derivation_parameters_are_refused() {
    for (args, message) in [
        (
            ["--argonmemory=8", "x"],
            "Argon2-Speichergröße „8“ ist ungültig (64 bis 4096 MiB)",
        ),
        (
            ["--argonmemory=4097", "x"],
            "Argon2-Speichergröße „4097“ ist ungültig (64 bis 4096 MiB)",
        ),
        (
            ["--argonpasses=2", "x"],
            "Argon2-Durchlaufzahl „2“ ist ungültig (3 bis 100)",
        ),
        (
            ["--argonpasses=101", "x"],
            "Argon2-Durchlaufzahl „101“ ist ungültig (3 bis 100)",
        ),
        (
            ["--argonmemory=abc", "x"],
            "Argon2-Speichergröße „abc“ ist ungültig (64 bis 4096 MiB)",
        ),
    ] {
        let out = zu(&args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        assert!(text(&out.stderr).contains(message), "{args:?}");
    }
}

#[test]
fn refuses_to_run_without_a_terminal() {
    let out = zu(&["datei.txt"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "");
}
