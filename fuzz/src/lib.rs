#![allow(dead_code, unused_imports, clippy::all)]

#[path = "../../src/crypto.rs"]
pub mod crypto;

#[path = "../../src/definitions.rs"]
pub mod definitions;

pub mod utils {
    pub const fn usize_of(n: i64) -> usize {
        usize::from_le_bytes(n.to_le_bytes())
    }

    pub const fn u8_of(n: i64) -> u8 {
        n.to_le_bytes()[0]
    }
}

#[path = "../../src/curses/terminfo.rs"]
pub mod terminfo;

#[path = "../../src/curses/mouse.rs"]
pub mod mouse;

pub mod curses {
    pub use crate::mouse;
    pub use crate::terminfo;
}

#[global_allocator]
static GLOBAL: crypto::ZeroizingAllocator = crypto::ZeroizingAllocator;
