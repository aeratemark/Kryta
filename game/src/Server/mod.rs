//! `game/Server/` - the reference's `src/server/game/Server/` (53 files).
//!
//! The socket layer of the game library: the object a gameplay connection becomes, the session
//! behind it, the manager that owns the listener, and the protocol's tables and messages. It is not
//! a program - `worldserver` starts it - which is why one process owns one listener and still keeps
//! the network as a subsystem (`AGENTS.md`).
//!
//! Here now: `Protocol/`, with the first message this client sends on a gameplay connection. The
//! world socket, session and manager arrive with the front door that hands them a connection
//! (`docs/decisions/0004-gameplay-connections-arrive-at-the-front-door.md`).

#[path = "Protocol/mod.rs"]
pub mod protocol;
