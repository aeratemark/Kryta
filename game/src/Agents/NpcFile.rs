//! The pack's NPC files, read - now at `game/Maps/NpcPack.rs`, re-exported here.
//!
//! The reader walks and reads the disk at start-up, and `worldserver/tests/content_is_held.rs`
//! refuses a file read inside a module that serves a connection. It therefore lives beside
//! `Maps/Pack.rs`, the world pack's own reader, and this path stays as the re-export while the
//! callers move (`AGENTS.md` rule 11).

pub use crate::maps::npc_pack::*;
