//! Where a login's character list comes from: the character repository, re-exported.
//!
//! The repository moved to the game library (`game/src/Characters/Repository.rs`) when the
//! administration tool began reading the same rows: one implementation of the data, two programs and
//! a tool asking it (`AGENTS.md`, rule 11's move - copy, re-export, delete). The login's use is
//! unchanged: its answer carries one `0x0007` per character of the account the portal identified.

pub use game::characters::DatabaseCharacters;
