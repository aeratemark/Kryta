//! The wire: the byte buffer, the message table, and the walk that frames a message with it.
//!
//! Reference: `src/server/shared/Packets/` holds the reference's `ByteBuffer`, and the rest of its
//! wire lives in the game library: `game/Server/Protocol/` for the opcode table and
//! `game/Server/Packets/` for one file pair per family.
//!
//! Here the table and the walker are **shared**, and that is a deviation taken on purpose. Both
//! connections need them, because a login message carries a counted field exactly as a gameplay
//! message does, and the auth program does not link the game library. The reference keeps its opcode
//! table in the game library because only its world reads one; this client's login connection is a
//! second framed stream, and one walker for two connections is one rule instead of two that can drift.
//! `game`'s `Protocol/` re-exports them, so a reader looking there finds them where the reference puts
//! them.
//!
//! The table is ported row for row from `ASURA/crates/asura-protocol/src/packets.rs`; the walker from
//! its `client/frame.rs`. See `docs/port/02-wire.md`, "The table, and the walk that splits a stream".

#[path = "ByteBuffer.rs"]
pub mod byte_buffer;
#[path = "Frame.rs"]
pub mod frame;
#[path = "Tables.rs"]
pub mod tables;

pub use byte_buffer::{ByteBuffer, ByteBufferError};
pub use frame::{Frame, FrameError, HEADER_LEN};
