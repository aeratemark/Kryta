//! The item family: `GAME_CMSG_ITEM_MOVE` (`0x8072`) and `GAME_CMSG_EQUIP_ITEM` (`0x8030`).
//!
//! Reference: `src/server/game/Handlers/ItemHandler.cpp`, whose `HandleItemOpcode`-family members read
//! the message and call player operations (`CanEquipItem`, `EquipItem`); `ItemHandler.cpp:155-265` is
//! the equip pair. The behaviours themselves are ASURA's, ported whole:
//! `ASURA/crates/asura-world/src/item.rs:255-312` (`handle_item_move`) and `:314-436`
//! (`handle_equip_item`).
//!
//! What is here is the translation only. The decision - where an item would be worn - is
//! `crate::characters::equip_place` (`Characters/Equipping.rs`), and whether a placement is one the
//! character may have is `DatabaseCharacters::move_item`. **No frame changed when this moved out of
//! `WorldSession.rs`**: the same messages, the same order, the same sizes, the same log lines.

use common::logging::LogLevel;
use shared::cryptography::AuthCrypt;
use shared::network::socket::SessionIo;

use crate::characters::{DatabaseCharacters, MoveOutcome};
use crate::server::packets::{item_packets, misc_packets};
use crate::server::world_session::{CharacterSheet, WorldSession};

impl WorldSession {
    /// `GAME_CMSG_ITEM_MOVE` (`0x8072`): a drag in the inventory.
    ///
    /// Ported from `ASURA/crates/asura-world/src/item.rs:255-312`, whole: the row moves when the
    /// client's placement is one it may have, a refused move is answered with where the item **really
    /// is** - the client moved it in its own model before asking - and the agent's worn items are sent
    /// again when the equipped set changed, because that is the one thing the client cannot work out
    /// for itself.
    pub(crate) async fn answer_item_move(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let Some((item_id, bag_id, slot)) = item_move(message) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an item move of {} bytes cannot be read.",
                    message.len()
                ),
            );
            return;
        };
        let Some(characters) = self.characters() else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an item move was asked for, and this world has no character database."
                ),
            );
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let bags = self.bags();
        let before = characters.equipped_item_ids(&uuid, &bags).unwrap_or([0u32; 9]);

        let mut frames = Vec::new();
        match characters.move_item(&uuid, item_id, bag_id, slot, &bags) {
            Ok(MoveOutcome::At { bag_id, slot }) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} moved to bag {bag_id} slot {slot}."
                    ),
                );
                frames.push(item_packets::item_move_to_location(
                    item_packets::ITEM_STREAM,
                    item_id,
                    bag_id,
                    slot,
                ));
            }
            Ok(MoveOutcome::Unchanged) => {}
            Ok(MoveOutcome::Refused { place, reason }) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} to bag {bag_id} slot {slot} refused: {reason:?}."
                    ),
                );
                // The client moved it in its own model before asking, so put it back.
                if let Some((bag, slot)) = place {
                    frames.push(item_packets::item_move_to_location(
                        item_packets::ITEM_STREAM,
                        item_id,
                        bag,
                        slot,
                    ));
                }
            }
            Err(error) => self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: the item move could not be written: {error}"),
            ),
        }
        if characters.equipped_item_ids(&uuid, &bags).unwrap_or(before) != before {
            frames.push(equipment_frame(
                self.player_id(),
                &uuid,
                &bags,
                &characters,
            ));
        }
        if !frames.is_empty() {
            self.send(io, peer, crypt, &frames, "the item's new place").await;
        }
    }

    /// `GAME_CMSG_EQUIP_ITEM` (`0x8030`): wear an item, evicting whatever the equipped slot held.
    ///
    /// Ported from `ASURA/crates/asura-world/src/item.rs:314-436`, whole: the item must be the
    /// character's, its kind must be one that is worn, the equipped bag must exist, whatever is in that
    /// slot comes off to the first free carrying slot first (and with nowhere to put it, nothing is
    /// worn), and the agent's worn items are sent again when the set changed.
    pub(crate) async fn answer_equip_item(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let Some(item_id) = message
            .get(2..6)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an equip of {} bytes cannot be read.",
                    message.len()
                ),
            );
            return;
        };
        let Some(characters) = self.characters() else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an equip was asked for, and this world has no character database."
                ),
            );
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let held: Vec<crate::characters::HeldItem> = match characters.items_of(&uuid) {
            Ok(held) => held,
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("The character's items cannot be read: {error}"),
                );
                return;
            }
        };
        // The noun answers where it would be worn, or why it cannot be - the handler says which.
        let place = crate::characters::equip_place(
            &held,
            self.item_templates(),
            self.equipped_bag().as_ref(),
            item_id,
        );
        let (bag_id, slot) = match place {
            Ok(place) => place,
            Err(refusal) => {
                let (level, why) = match refusal {
                    crate::characters::NotWorn::NotTheCharacters => (
                        LogLevel::Info,
                        format!("Item {item_id} is not the character's, so it cannot be worn."),
                    ),
                    crate::characters::NotWorn::NoTemplate => (
                        LogLevel::Warn,
                        format!("No item template describes item {item_id}; it cannot be worn."),
                    ),
                    crate::characters::NotWorn::KindNotWorn { item_type } => (
                        LogLevel::Info,
                        format!("Item {item_id} is kind {item_type}, which is not worn."),
                    ),
                    crate::characters::NotWorn::NoEquippedBag => (
                        LogLevel::Warn,
                        "The setting names no equipped bag, so nothing can be worn.".to_string(),
                    ),
                };
                self.say(level, why);
                return;
            }
        };
        let bags = self.bags();
        let before = characters.equipped_item_ids(&uuid, &bags).unwrap_or([0u32; 9]);
        let mut frames = Vec::new();

        // Whatever is worn in that slot has to come off first, and there has to be somewhere to put it.
        let occupant = held
            .iter()
            .find(|row| row.bag_id == bag_id && row.slot == slot && row.item_id != item_id)
            .cloned();
        if let Some(occupant) = occupant {
            let Some((to_bag, to_slot)) = characters
                .first_free_carrying_slot(&uuid, &bags)
                .unwrap_or_default()
            else {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: no free carrying slot to take item {} off, so item {item_id} cannot be worn.",
                        occupant.item_id
                    ),
                );
                return;
            };
            match characters.move_item(&uuid, occupant.item_id, to_bag, to_slot, &bags) {
                Ok(MoveOutcome::At { bag_id, slot }) => {
                    self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: item {} taken off to bag {bag_id} slot {slot}.",
                            occupant.item_id
                        ),
                    );
                    frames.push(item_packets::item_move_to_location(
                        item_packets::ITEM_STREAM,
                        occupant.item_id,
                        bag_id,
                        slot,
                    ));
                }
                outcome => {
                    self.say(
                        LogLevel::Warn,
                        format!(
                            "Gameplay connection from {peer}: could not take item {} off to make room for {item_id}: {outcome:?}",
                            occupant.item_id
                        ),
                    );
                    return;
                }
            }
        }

        match characters.move_item(&uuid, item_id, bag_id, slot, &bags) {
            Ok(MoveOutcome::At { bag_id, slot }) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} worn in equipped slot {slot} of bag {bag_id}."
                    ),
                );
                frames.push(item_packets::item_move_to_location(
                    item_packets::ITEM_STREAM,
                    item_id,
                    bag_id,
                    slot,
                ));
            }
            Ok(MoveOutcome::Unchanged) => {}
            Ok(MoveOutcome::Refused { place, reason }) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: equipping item {item_id} refused: {reason:?}."
                    ),
                );
                if let Some((bag, slot)) = place {
                    frames.push(item_packets::item_move_to_location(
                        item_packets::ITEM_STREAM,
                        item_id,
                        bag,
                        slot,
                    ));
                }
            }
            Err(error) => self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: the equip could not be written: {error}"),
            ),
        }

        if characters.equipped_item_ids(&uuid, &bags).unwrap_or(before) != before {
            frames.push(equipment_frame(
                self.player_id(),
                &uuid,
                &bags,
                &characters,
            ));
        }
        if !frames.is_empty() {
            self.send(io, peer, crypt, &frames, "the item's new place").await;
        }
    }
}

/// `UPDATE_AGENT_VISUAL_EQUIPMENT` for this character: the nine ids the agent is drawn wearing.
///
/// `ASURA/crates/asura-world/src/item.rs:438-440`, `equipment_frame`. A character whose rows cannot be
/// read is drawn wearing nothing, which is what the load does with the same failure.
fn equipment_frame(
    agent: u32,
    uuid: &str,
    bags: &[crate::creation::Bag],
    characters: &DatabaseCharacters,
) -> Vec<u8> {
    let equipped = characters.equipped_item_ids(uuid, bags).unwrap_or([0u32; 9]);
    misc_packets::update_agent_visual_equipment(agent, equipped)
}

/// The item, the bag and the slot a drag names: the header, a dword and two bytes.
///
/// `0x8072`'s own row: `MsgHeader`, the item as a dword, the bag as a word and the slot as a byte.
fn item_move(message: &[u8]) -> Option<(u32, u16, u8)> {
    if message.len() < 9 {
        return None;
    }
    let item = message.get(2..6)?;
    let bag = message.get(6..8)?;
    let slot = message.get(8)?;
    Some((
        u32::from_le_bytes([item[0], item[1], item[2], item[3]]),
        u16::from_le_bytes([bag[0], bag[1]]),
        *slot,
    ))
}
