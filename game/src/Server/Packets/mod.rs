//! The message families: one file per family, as the reference has it.
//!
//! Reference: `src/server/game/Server/Packets/`, whose `AllPackets.h` includes 17 family files.
//! One file holding every message in every direction is the arrangement this project exists to
//! avoid, so a family arrives as its own file and its own subject.

#[path = "InstancePackets.rs"]
pub mod instance_packets;

pub use instance_packets::{creation_load_block, char_creation_start, instance_load_head};
