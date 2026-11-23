//! `game/Handlers/` - the reference's `src/server/game/Handlers/` (36 files).
//!
//! One file per message family, and in each one the translation from a client's message to the nouns
//! that own the state: the reference writes these as `WorldSession::HandleXxxOpcode` members, so they
//! are `impl WorldSession` here too, in their own file, calling into `Characters/`, `Agents/`,
//! `Maps/` and the rest. The rule they follow is the guide's (`docs/port/10-separation-guide.md`,
//! section 2): a handler reads the message, asks a noun for the decision, and answers - it does not
//! hold the decision itself.
//!
//! Ported so far: the item messages (`ItemHandler`, whose decisions are
//! `characters::equip_place` and `DatabaseCharacters::move_item`), the four movement messages
//! (`MovementHandler`, answered by `AgentState`, `pathfind` and `Maps::Portals`), the travel family
//! (`TravelHandler`, answered by the travel row, `Portals` and `place_for_travel`), a spoken line
//! (`ChatHandler`, answered by `game/Chat/`), the guild requests (`GuildHandler`, answered by
//! `game/Guilds/`) and the compass relays (`CompassHandler`, answered by `game/Parties/` and the
//! copy's agent tables).

#[path = "CharacterHandler.rs"]
pub mod character;

#[path = "CompassHandler.rs"]
pub mod compass;

#[path = "ItemHandler.rs"]
pub mod item;

#[path = "NpcHandler.rs"]
pub mod npc;

#[path = "MovementHandler.rs"]
pub mod movement;

#[path = "TravelHandler.rs"]
pub mod travel;

#[path = "ChatHandler.rs"]
pub mod chat;

#[path = "GuildHandler.rs"]
pub mod guild;

#[path = "SkillHandler.rs"]
pub mod skill;
