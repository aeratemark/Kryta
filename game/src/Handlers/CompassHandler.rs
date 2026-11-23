//! The compass family: what one party member draws for the others.
//!
//! ASURA `crates/asura-world/src/compass.rs` (123 lines) is the authority for both relays, and
//! this file carries it whole: a stroke (`GAME_CMSG_DRAW_MAP`, `0x802B`) repeated to the drawer's
//! party as `GAME_SMSG_COMPASS_DRAWING` (`0x0091`), and a called target (`GAME_CMSG_TARGET_CALL`,
//! `0x8023`, kind 10) repeated to the whole party as `GAME_SMSG_AGENT_PINGED` (`0x0034`). The server
//! keeps no stroke and no target: a late arrival is shown neither.
//!
//! Neither message is in an ASURA capture. ASURA took them from preforged
//! (`gw_compass_wire.h`, 37130 CMSG 41 / SMSG 144; `gw_callout.h`, CMSG 33 / SMSG 52) on its own
//! packet table, and `ASURA docs/51-compass-and-chat.md` section 3 lists what a live recording still
//! has to confirm. The reference (AzerothCore) has no compass handler, so this family is this tree's
//! arrangement of a Tyria-only subject, exactly as `FileCheck`, `Portal` and `Pack` are; the bytes,
//! the orders and the refusals are ASURA's, line for line.
//!
//! The party is the one `game/Parties/mod.rs` keeps: [`crate::parties::Parties::present`] is the
//! members standing in the drawer's own copy, the drawer included, and a player the table does not
//! know is a party of one. Delivery is `Peers::send_to`, the same channel every party batch uses.

use common::logging::LogLevel;

use crate::server::packets::misc_packets;
use crate::server::world_session::{CharacterSheet, WorldSession};

impl WorldSession {
    /// The player's id as the word both relays carry, or `None` (logged) for an id that does not fit
    /// (`compass.rs:30-40`, `player_word`).
    fn player_word(&self, what: &str) -> Option<u16> {
        let player = self.player_id();
        match u16::try_from(player) {
            Ok(word) => Some(word),
            Err(_) => {
                self.say(
                    LogLevel::Warn,
                    format!(
                        "{what}: player id {player} does not fit the message's word; nothing sent"
                    ),
                );
                None
            }
        }
    }

    /// Send `frames` to the player's party in its copy; `to_self` says whether the player gets them
    /// too (`compass.rs:21-28`, `party_send`).
    fn party_send(&self, frames: &[Vec<u8>], to_self: bool) {
        let player = self.player_id();
        let present = match self.world.parties().lock() {
            Ok(parties) => parties.present(self.map_id(), self.district(), player),
            Err(_) => vec![player],
        };
        if let Ok(peers) = self.peers.lock() {
            for member in present {
                if to_self || member != player {
                    peers.send_to(member, frames);
                }
            }
        }
    }

    /// `GAME_CMSG_DRAW_MAP` (`0x802B`): one stroke segment drawn on the compass, repeated to the rest
    /// of the party unchanged (`compass.rs:42-74`, `handle_draw_map`).
    ///
    /// The drawer is not sent its own stroke: its client has already drawn what it sent. (Preforged
    /// echoes to the drawer as well; which one retail does is not measured.)
    pub(crate) fn answer_draw_map(&self, peer: &str, message: &[u8], sheet: &CharacterSheet) {
        let Some(body) = message.get(2..) else {
            return;
        };
        let Some(stroke) = misc_packets::read_draw_map(body) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: compass: {} sent a stroke of {} byte(s) that is not a stroke; nothing sent",
                    sheet.name,
                    body.len()
                ),
            );
            return;
        };
        let Some(drawer) = self.player_word("compass") else {
            return;
        };
        self.say(
            LogLevel::Debug,
            format!(
                "Gameplay connection from {peer}: compass: player {} draws {} point(s) of stroke {}",
                self.player_id(),
                stroke.points.len(),
                stroke.stroke
            ),
        );
        self.party_send(&[misc_packets::compass_drawing(drawer, &stroke)], false);
    }

    /// `GAME_CMSG_TARGET_CALL` (`0x8023`): the player calls something to its party
    /// (`compass.rs:76-123`, `handle_target_call`).
    ///
    /// A called target (kind 10) that is an agent of this copy - a player, an NPC or an item on the
    /// ground - goes to the whole party, the caller included, as `0x0034`. The other kinds announce
    /// the caller's own health, energy, experience or morale; this server models none of those
    /// values, so they are logged and nothing is sent.
    pub(crate) fn answer_target_call(&self, peer: &str, message: &[u8], sheet: &CharacterSheet) {
        let Some(body) = message.get(2..) else {
            return;
        };
        let Some((kind, argument)) = misc_packets::read_target_call(body) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: call: {} sent a call of {} byte(s) that is not a call; nothing sent",
                    sheet.name,
                    body.len()
                ),
            );
            return;
        };
        if kind != misc_packets::TARGET_CALL_TARGET {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: call: {} calls kind {kind} (argument {argument}); only a called target is modelled, nothing sent",
                    sheet.name
                ),
            );
            return;
        }
        let map_id = self.map_id();
        let district = self.district();
        if !self.world.has_agent(map_id, district, argument)
            && self.world.ground_item(map_id, district, argument).is_none()
        {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: call: {} calls agent {argument}, which is not in this instance; nothing sent",
                    sheet.name
                ),
            );
            return;
        }
        let Some(caller) = self.player_word("call") else {
            return;
        };
        self.say(
            LogLevel::Debug,
            format!(
                "Gameplay connection from {peer}: call: player {} calls agent {argument}",
                self.player_id()
            ),
        );
        self.party_send(&[misc_packets::agent_pinged(caller, argument)], true);
    }
}
