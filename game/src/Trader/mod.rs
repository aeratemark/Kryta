//! `game/Trader/` - the reference's `src/server/game/AuctionHouse/` (4 files).
//!
//! Renamed: the reference calls this directory `AuctionHouse/`.
//!
//! What is here: the trade windows NPCs open, ASURA's `crates/asura-world/src/vendor/` ported 1:1.
//! `Content.rs` is what each NPC offers (`vendor/content.rs`), filled by the pack
//! (`game/Maps/VendorPack.rs`); `Rules.rs` is the pure trade rules (`vendor/rules.rs`). The handler is
//! `game/Handlers/NpcHandler.rs`; the messages are `game/Server/Packets/VendorPackets.rs`.

#[path = "Content.rs"]
pub mod content;
#[path = "Rules.rs"]
pub mod rules;
