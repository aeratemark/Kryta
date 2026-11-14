//! `game/Handlers/` - the reference's `src/server/game/Handlers/` (36 files).
//!
//! One file per message family, and in each one the translation from a client's message to the nouns
//! that own the state: the reference writes these as `WorldSession::HandleXxxOpcode` members, so they
//! are `impl WorldSession` here too, in their own file, calling into `Characters/`, `Agents/`,
//! `Maps/` and the rest. The rule they follow is the guide's (`docs/port/10-separation-guide.md` §2):
//! a handler reads the message, asks a noun for the decision, and answers - it does not hold the
//! decision itself.
//!
//! Ported so far: `ItemHandler`, the item messages. Their decisions belong to
//! `characters::equip_place` (where an item would be worn) and `DatabaseCharacters::move_item` (whether
//! a placement is one the character may have); the handler reads the rows, asks, and answers.

#[path = "ItemHandler.rs"]
pub mod item;
