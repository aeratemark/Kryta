//! The item family: a drag (`0x8072`), an equip (`0x8030`), an unequip (`0x804F`), a destroy
//! (`0x8069`), a drop (`0x802C`) and a pickup (`0x803F`).
//!
//! The decisions are the rules' (`game/Characters/Inventory.rs`), the rows are the character's
//! (`game/Characters/ItemOps.rs`), and this file is the translation: read the client's message, ask the
//! rules for one operation, write the changes in one transaction, and answer with the frames the live
//! server sends.
//!
//! Reference: `ASURA/crates/asura-world/src/item.rs` - `drop_item` (`:483`), `interact_item` (`:593`),
//! `pick_up` (`:664`), `move_item` (`:704`), `equip_item` (`:749`), `unequip_item` (`:784`),
//! `destroy_item` (`:830`) and `grant_definition` (`:867`) - read from the local ASURA tree on
//! 2026-11-22. The message shapes are its handlers' (`:1041-1278`).

use common::logging::LogLevel;
use shared::cryptography::AuthCrypt;
use shared::network::socket::SessionIo;

use crate::characters::item_ops;
use crate::characters::{
    Change, CharacterSheet, DatabaseCharacters, EQUIPPED_ITEMS_MODEL, HeldItem, Op, Wearer,
};
use crate::creation::Bag;
use crate::server::packets::{agent_packets, chat_packets, item_packets, misc_packets};
use crate::server::world_session::WorldSession;

/// The item stream's id: the reference hardcodes `1` in every message that carries one.
use item_packets::ITEM_STREAM;

impl WorldSession {
    /// `GAME_CMSG_ITEM_MOVE` (`0x8072`): a drag in the inventory.
    ///
    /// `ASURA/crates/asura-world/src/item.rs:704-740`, `move_item`, whole: a move into the equipped bag
    /// is an equip into exactly that slot, with every equip rule. A refusal is answered with where the
    /// item **really is** - the client moved it in its own model before asking - and, when what the
    /// character wears changed, this client also gets the bulk `0x006E` it has always been sent for a
    /// drag, while the rest of the instance gets the single visual slot.
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
        let rows = match item_ops_rows(&characters, &uuid, peer, self) {
            Some(rows) => rows,
            None => return,
        };

        let op = Op::Move {
            item: item_id,
            bag: bag_id,
            slot,
        };
        match crate::characters::item_ops::commit(
            &characters,
            &uuid,
            &rows,
            &bags,
            self.world.item_catalogue(),
            &[op],
            self.wearer_for(sheet),
        ) {
            Ok(Ok(applied)) => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} to bag {bag_id} slot {slot}: {:?}",
                        applied.changes
                    ),
                );
                let (mut own, others) = placement_frames(&applied.changes, self.agent_id(), false);
                if applied
                    .changes
                    .iter()
                    .any(|change| matches!(change, Change::Worn { .. }))
                {
                    own.push(equipment_frame(self.agent_id(), &applied.rows, &bags));
                }
                self.deliver(io, peer, crypt, own, others, "the item's new place")
                    .await;
            }
            Ok(Err(refusal)) => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} to bag {bag_id} slot {slot} refused: {refusal:?}."
                    ),
                );
                let frames = snap_back(&rows, item_id);
                if !frames.is_empty() {
                    self.send(io, peer, crypt, &frames, "the item's real place")
                        .await;
                }
            }
            Err(error) => self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: the item move could not be written: {error}"
                ),
            ),
        }
    }

    /// `GAME_CMSG_EQUIP_ITEM` (`0x8030`): wear an item.
    ///
    /// `ASURA/crates/asura-world/src/item.rs:749-774`, `equip_item`: the rules wear it in the slot its
    /// kind belongs to, take whatever was worn there off to the first free carrying slot, and push the
    /// other hand's piece off for a two-handed weapon. The answer is the move plus the single visual
    /// slot (`0x006F`), which the rest of the instance is sent too.
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
        let bags = self.bags();
        let rows = match item_ops_rows(&characters, &uuid, peer, self) {
            Some(rows) => rows,
            None => return,
        };
        match crate::characters::item_ops::commit(
            &characters,
            &uuid,
            &rows,
            &bags,
            self.world.item_catalogue(),
            &[Op::Equip { item: item_id }],
            self.wearer_for(sheet),
        ) {
            Ok(Ok(applied)) => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} worn: {:?}",
                        applied.changes
                    ),
                );
                let (own, others) = placement_frames(&applied.changes, self.agent_id(), true);
                self.deliver(io, peer, crypt, own, others, "the item worn")
                    .await;
            }
            Ok(Err(refusal)) => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: equipping item {item_id} refused: {refusal:?}."
                    ),
                );
                let frames = snap_back(&rows, item_id);
                if !frames.is_empty() {
                    self.send(io, peer, crypt, &frames, "the item's real place")
                        .await;
                }
            }
            Err(error) => self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: the equip could not be written: {error}"),
            ),
        }
    }

    /// `GAME_CMSG_UNEQUIP_ITEM` (`0x804F`): take a worn piece off to a bag slot the client names.
    ///
    /// `ASURA/crates/asura-world/src/item.rs:784-822`, `unequip_item`: the message is
    /// `{equipped_slot: u8, bag: u16, slot: u8}`; the answer is the move and the piece's single visual
    /// slot cleared. This tree has no armour-to-energy model, so the character's pools stay as the load
    /// set them, exactly as ASURA's comment says.
    pub(crate) async fn answer_unequip_item(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let Some(body) = message.get(2..6) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an unequip of {} bytes cannot be read.",
                    message.len()
                ),
            );
            return;
        };
        let equipped_slot = body[0];
        let dest_bag = u16::from_le_bytes([body[1], body[2]]);
        let dest_slot = body[3];
        let Some(characters) = self.characters() else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an unequip was asked for, and this world has no character database."
                ),
            );
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let bags = self.bags();
        let rows = match item_ops_rows(&characters, &uuid, peer, self) {
            Some(rows) => rows,
            None => return,
        };
        let worn = worn_item(&rows, &bags, equipped_slot);
        let op = Op::Unequip {
            equipped: equipped_slot,
            bag: dest_bag,
            slot: dest_slot,
        };
        match crate::characters::item_ops::commit(
            &characters,
            &uuid,
            &rows,
            &bags,
            self.world.item_catalogue(),
            &[op],
            self.wearer_for(sheet),
        ) {
            Ok(Ok(applied)) => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: equipped slot {equipped_slot} taken off: {:?}",
                        applied.changes
                    ),
                );
                let (own, others) = placement_frames(&applied.changes, self.agent_id(), true);
                self.deliver(io, peer, crypt, own, others, "the piece taken off")
                    .await;
            }
            Ok(Err(refusal)) => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: unequip of slot {equipped_slot} to bag {dest_bag} slot {dest_slot} refused: {refusal:?}."
                    ),
                );
                let frames = worn
                    .and_then(|id| item_ops::snap_back(&rows, id))
                    .map(|(bag, slot)| {
                        vec![item_packets::item_change_location(
                            ITEM_STREAM,
                            worn.unwrap_or(0),
                            bag,
                            slot,
                        )]
                    })
                    .unwrap_or_default();
                if !frames.is_empty() {
                    self.send(io, peer, crypt, &frames, "the piece's real place")
                        .await;
                }
            }
            Err(error) => self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: the unequip could not be written: {error}"
                ),
            ),
        }
    }

    /// `GAME_CMSG_ITEM_DESTROY` (`0x8069`): drag an item to the trash.
    ///
    /// `ASURA/crates/asura-world/src/item.rs:830-858`, `destroy_item`: the row is deleted and the answer
    /// is `ITEM_REMOVE` with the same id; without it the client's removal is only in its model and the
    /// item returns on the next load. An item the character does not hold in a slot is answered with
    /// nothing.
    pub(crate) async fn answer_destroy_item(
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
                    "Gameplay connection from {peer}: a destroy of {} bytes cannot be read.",
                    message.len()
                ),
            );
            return;
        };
        if item_id == 0 {
            return;
        }
        let Some(characters) = self.characters() else {
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let bags = self.bags();
        let rows = match item_ops_rows(&characters, &uuid, peer, self) {
            Some(rows) => rows,
            None => return,
        };
        match crate::characters::item_ops::commit(
            &characters,
            &uuid,
            &rows,
            &bags,
            self.world.item_catalogue(),
            &[Op::Destroy { item: item_id }],
            self.wearer_for(sheet),
        ) {
            Ok(Ok(applied)) => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} destroyed: {:?}",
                        applied.changes
                    ),
                );
                let (own, others) = placement_frames(&applied.changes, self.agent_id(), true);
                self.deliver(io, peer, crypt, own, others, "the item destroyed")
                    .await;
            }
            Ok(Err(refusal)) => self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: a destroy of item {item_id} refused: {refusal:?}."
                ),
            ),
            Err(error) => self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: the destroy could not be written: {error}"
                ),
            ),
        }
    }
}

impl WorldSession {
    /// `/item <name or id> [quantity]`: give the character one live-verified catalogue definition in
    /// the first free carrying slot.
    ///
    /// `ASURA/crates/asura-world/src/item.rs:867-966`, `grant_definition`, whole: the argument is a
    /// name or id and an optional quantity; only definitions with `live: true` are accepted, because
    /// the flag means the client's own item probe has shown that wire identity and an unverified file
    /// id can miss its UI texture. The row is a full definition, inserted through the rules
    /// (`Op::Insert`), and the answer is the measured pair `CREATE_NAMED_ITEM` and
    /// `ITEM_CHANGE_LOCATION`, plus `0x015A` when the item was made for a profession.
    ///
    /// ASURA answers this command on the server log only - it is its test command, not the
    /// reference's - and this port keeps that: the item appearing in the bag is the answer.
    pub(crate) async fn answer_grant_item(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
        argument: &str,
    ) {
        let Some((query, quantity)) = grant_request(argument) else {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: /item needs a name or id and a positive quantity."
                ),
            );
            return;
        };
        let Some(catalogue) = self.world.item_catalogue() else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: /item: this world read no item catalogue."
                ),
            );
            return;
        };
        let wanted = grant_key(query);
        let matches: Vec<_> = catalogue
            .definitions()
            .iter()
            .filter(|definition| {
                definition.live
                    && (grant_key(&definition.id) == wanted
                        || grant_key(&definition.description) == wanted
                        || grant_key(&definition.id).contains(&wanted)
                        || grant_key(&definition.description).contains(&wanted))
            })
            .collect();
        let definition = match matches.as_slice() {
            [definition] => *definition,
            [] => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: /item: {query:?} is not a live-verified definition; the client's item probe has not shown its wire identity."
                    ),
                );
                return;
            }
            many => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: /item: {query:?} matches {} live definitions; use an id: {}",
                        many.len(),
                        many.iter()
                            .map(|definition| definition.id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
                return;
            }
        };
        let Some(characters) = self.characters() else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: /item needs the character database, and this world opened none."
                ),
            );
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let bags = self.bags();
        let rows = match item_ops_rows(&characters, &uuid, peer, self) {
            Some(rows) => rows,
            None => return,
        };
        let item_id = match characters.allocate_item_id() {
            Ok(id) => id,
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer}: /item: {error}"),
                );
                return;
            }
        };
        let row = item_ops::instantiate(definition, item_id, quantity);
        let entry = match item_ops::entry_of(&row, Some(catalogue)) {
            Ok(entry) => entry,
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!(
                        "Gameplay connection from {peer}: /item: {} is not a valid row: {error}",
                        definition.id
                    ),
                );
                return;
            }
        };
        match item_ops::commit_ops(
            &characters,
            &uuid,
            &rows,
            &bags,
            Some(catalogue),
            &[Op::Insert { entry }],
            self.wearer_for(sheet),
            std::slice::from_ref(&row),
        ) {
            Ok(Ok(applied)) => {
                let seat = applied.changes.iter().find_map(|change| match *change {
                    Change::Inserted { at, .. } => Some(at),
                    _ => None,
                });
                let Some(stored) = applied.rows.iter().find(|row| row.item_id == item_id) else {
                    self.say(
                        LogLevel::Warn,
                        format!(
                            "Gameplay connection from {peer}: /item: wrote item {item_id} but could not read it back."
                        ),
                    );
                    return;
                };
                let mut frames = vec![item_packets::create_named_item(
                    &crate::creation::named_item(&item_ops::item_of_row(stored)),
                )];
                if stored.profession != 0 {
                    frames.push(item_packets::item_set_profession(
                        stored.item_id,
                        stored.profession,
                    ));
                }
                let Some((bag, slot)) = seat else {
                    self.say(
                        LogLevel::Warn,
                        format!(
                            "Gameplay connection from {peer}: /item: item {item_id} was written but the rules placed it nowhere."
                        ),
                    );
                    return;
                };
                frames.push(item_packets::item_change_location(
                    ITEM_STREAM,
                    stored.item_id,
                    bag,
                    slot,
                ));
                self.send(io, peer, crypt, &frames, "the item granted")
                    .await;
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: /item gave {} x{quantity} in bag {bag} slot {slot} (item {item_id}; base {}, upgrades {:x?}).",
                        definition.id,
                        definition.base.as_deref().unwrap_or("-"),
                        definition.upgrades
                    ),
                );
            }
            Ok(Err(refusal)) => {
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: /item: {} x{quantity} refused: {refusal:?}.",
                        definition.id
                    ),
                );
            }
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer}: /item could not be written: {error}"),
                );
            }
        }
    }
}

impl WorldSession {
    /// `GAME_CMSG_DROP_ITEM` (`0x802C`): `count` of an item, off the stack and onto the ground.
    ///
    /// `ASURA/crates/asura-world/src/item.rs:483-578`, `drop_item`, whole:
    ///
    /// * **a whole stack** - `ITEM_REMOVE {stream, item_id}`, then an item agent whose model is that
    ///   same id. The row is gone; the client kept the item's record and the agent names it.
    /// * **part of a stack** - `ITEM_UPDATE_QUANTITY {item_id, remaining}`, then `ITEM_UPDATE`
    ///   (`0x0162`) for a ground record with a fresh id and the dropped quantity, then the item agent
    ///   for that id. The stack's row stays.
    ///
    /// Both end with the drop line on `Channel_Global`. Everyone else in the instance is sent the
    /// ground record and its agent, so the item is on their ground too. A pickup is
    /// [`Self::answer_interact_item`].
    pub(crate) async fn answer_drop_item(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let item_id = message
            .get(2..6)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        let count = message.get(6).copied();
        let (Some(item_id), Some(count)) = (item_id, count) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a drop of {} bytes cannot be read.",
                    message.len()
                ),
            );
            return;
        };
        if item_id == 0 || count == 0 {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: a drop of {count} of item {item_id} names something that is not an item; ignored."
                ),
            );
            return;
        }
        // An item is never droppable outside an explorable. The client itself does not offer the
        // drop there (`docs/game-rules.md` E51), and ASURA's server therefore has no check; this tree
        // refuses one a host does send. A refused drop is invisible: no message acknowledges one
        // (`E47`), so this is a log line and silence.
        let kind = self
            .maps
            .iter()
            .find(|map| map.id == self.map_id())
            .and_then(|map| map.kind);
        if !drop_allowed(kind) {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: a drop in map {} is refused - an item is never droppable outside an explorable (docs/game-rules.md E51).",
                    self.map_id()
                ),
            );
            return;
        }
        let Some(characters) = self.characters() else {
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let bags = self.bags();
        let rows = match item_ops_rows(&characters, &uuid, peer, self) {
            Some(rows) => rows,
            None => return,
        };
        let Some(item) = rows.iter().find(|row| row.item_id == item_id).cloned() else {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: a drop names item {item_id}, which the character does not have; ignored."
                ),
            );
            return;
        };
        let Some((position, plane)) = self.with_agent(|agent| (agent.position, agent.plane)) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a drop was asked for before the character had a body; ignored."
                ),
            );
            return;
        };
        let op = Op::Take {
            item: item_id,
            count: u32::from(count),
        };
        match item_ops::commit(
            &characters,
            &uuid,
            &rows,
            &bags,
            self.world.item_catalogue(),
            &[op],
            self.wearer_for(sheet),
        ) {
            Ok(Ok(applied)) => {
                let remaining = applied
                    .changes
                    .iter()
                    .find_map(|change| match *change {
                        Change::Quantity { quantity, .. } => Some(quantity),
                        _ => None,
                    })
                    .unwrap_or(0);
                let agent_id = self
                    .world
                    .allocate_ground_agent(self.map_id(), self.district());
                let at = (position.0 + DROP_OFFSET, position.1 + DROP_OFFSET);
                let mut own = Vec::new();
                let ground = if remaining == 0 {
                    own.push(item_packets::item_remove(ITEM_STREAM, item.item_id));
                    crate::districts::GroundItem {
                        agent_id,
                        item: HeldItem {
                            bag_id: 0,
                            slot: 0,
                            ..item.clone()
                        },
                        source_item_id: None,
                        position: at,
                        plane,
                        direction: (1.0, 0.0),
                    }
                } else {
                    let ground_id = match characters.allocate_item_id() {
                        Ok(id) => id,
                        Err(error) => {
                            self.world.release_ground_agent(
                                self.map_id(),
                                self.district(),
                                agent_id,
                            );
                            self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: the drop's item id could not be allocated: {error}"
                                ),
                            );
                            return;
                        }
                    };
                    let ground_item = HeldItem {
                        item_id: ground_id,
                        bag_id: 0,
                        slot: 0,
                        quantity: u32::from(count),
                        ..item.clone()
                    };
                    own.push(item_packets::item_update_quantity(
                        item.item_id,
                        remaining,
                    ));
                    own.push(item_packets::item_update(
                        &crate::creation::named_item(&item_ops::item_of_row(&ground_item)),
                    ));
                    crate::districts::GroundItem {
                        agent_id,
                        item: ground_item,
                        source_item_id: Some(item.item_id),
                        position: at,
                        plane,
                        direction: (1.0, 0.0),
                    }
                };
                self.world
                    .put_ground_item(self.map_id(), self.district(), ground.clone());
                own.push(agent_packets::item_agent(
                    agent_id,
                    ground.item.item_id,
                    at,
                    plane,
                    (1.0, 0.0),
                ));
                own.extend(item_line(DROP_LINE, &ground.item, self.player_id()));
                let others = crate::districts::ground::ground_frames(&ground);
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: {count} of item {item_id} dropped at ({:.1}, {:.1}) as item {} agent {agent_id}; {remaining} left in the bag.",
                        at.0, at.1, ground.item.item_id
                    ),
                );
                self.deliver(io, peer, crypt, own, others, "the drop")
                    .await;
            }
            Ok(Err(refusal)) => self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: a drop of {count} of item {item_id} refused: {refusal:?}."
                ),
            ),
            Err(error) => self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: the drop could not be written: {error}"),
            ),
        }
    }

    /// `GAME_CMSG_INTERACT_ITEM` (`0x803F`, `{agent_id: u32, u8}`): pick something up.
    ///
    /// `ASURA/crates/asura-world/src/item.rs:593-694`, `interact_item`, whole. A ground item is the
    /// one kind this world answers; every other target is ignored, as the reference's own unfinished
    /// handler does. The live server answers with the action hold and its target, the pickup line,
    /// `ITEM_PICKED_UP`, and then one of two bag updates:
    ///
    /// * **part of a stack** - `ITEM_UPDATE_QUANTITY` says the stack it came from grew back; no new
    ///   row. Only when that stack is still this character's and has room under its limit.
    /// * **otherwise** - `ITEM_MOVE_TO_LOCATION` puts the item, under its own id, in the first free
    ///   carrying slot, and the row is written.
    ///
    /// Both end with `WORLD_REMOVE_AGENT` for the ground item, and the action hold cleared. The rest
    /// of the instance is sent the `WORLD_REMOVE_AGENT` too. With no room, the item stays on the
    /// ground.
    pub(crate) async fn answer_interact_item(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let Some(agent_id) = message
            .get(2..6)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an interaction of {} bytes cannot be read.",
                    message.len()
                ),
            );
            return;
        };
        // Claimed before any await, so two players cannot both pick it up
        // (`ASURA/crates/asura-world/src/item.rs:601-605`).
        let Some(ground) = self
            .world
            .take_ground_item(self.map_id(), self.district(), agent_id)
        else {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: interact names agent {agent_id}, which is not a ground item; ignored."
                ),
            );
            return;
        };
        let Some(characters) = self.characters() else {
            self.world
                .put_ground_item(self.map_id(), self.district(), ground);
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let bags = self.bags();
        let rows = match item_ops_rows(&characters, &uuid, peer, self) {
            Some(rows) => rows,
            None => {
                self.world
                    .put_ground_item(self.map_id(), self.district(), ground);
                return;
            }
        };
        let catalogue = self.world.item_catalogue();
        let result = pick_up(
            &characters,
            &uuid,
            &rows,
            &bags,
            catalogue,
            &ground,
            self.wearer_for(sheet),
        );
        match result {
            Ok(Ok(applied)) => {
                let mut own = vec![
                    agent_packets::agent_property_update_int(
                        agent_packets::AGENT_PROPERTY_ACTION,
                        self.agent_id(),
                        1,
                    ),
                    agent_packets::agent_property_update_int(
                        agent_packets::AGENT_PROPERTY_TARGET,
                        self.agent_id(),
                        agent_id,
                    ),
                ];
                own.extend(item_line(PICKUP_LINE, &ground.item, self.player_id()));
                own.push(item_packets::item_picked_up(
                    ground.item.item_id,
                    u16::try_from(self.agent_id()).unwrap_or(0),
                ));
                for change in &applied.changes {
                    match *change {
                        Change::Quantity { item, quantity } => {
                            own.push(item_packets::item_update_quantity(item, quantity));
                        }
                        Change::Inserted { item, at } => {
                            own.push(item_packets::item_move_to_location(
                                ITEM_STREAM,
                                item,
                                at.0,
                                at.1,
                            ));
                        }
                        _ => {}
                    }
                }
                own.push(agent_packets::world_remove_agent(agent_id).1);
                let others = vec![agent_packets::world_remove_agent(agent_id).1];
                // The live server clears the hold about 0.8 s later; the client accepts it at once
                // and the item is already back, so this does not wait on the tick
                // (`ASURA/crates/asura-world/src/item.rs:638-644`).
                own.push(agent_packets::agent_property_update_int(
                    agent_packets::AGENT_PROPERTY_ACTION,
                    self.agent_id(),
                    0,
                ));
                self.world
                    .release_ground_agent(self.map_id(), self.district(), agent_id);
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: picked up item {} from agent {agent_id}.",
                        ground.item.item_id
                    ),
                );
                self.deliver(io, peer, crypt, own, others, "the pickup")
                    .await;
            }
            Ok(Err(refusal)) => {
                self.world
                    .put_ground_item(self.map_id(), self.district(), ground.clone());
                self.say(
                    LogLevel::Debug,
                    format!(
                        "Gameplay connection from {peer}: a pickup of agent {agent_id} refused ({refusal:?}); it stays on the ground."
                    ),
                );
            }
            Err(error) => {
                self.world
                    .put_ground_item(self.map_id(), self.district(), ground);
                self.say(
                    LogLevel::Warn,
                    format!(
                        "Gameplay connection from {peer}: the pickup could not be written: {error}"
                    ),
                );
            }
        }
    }
}

/// The bag half of a pickup: back onto the stack it came from when that works, else a row of its own
/// (`ASURA/crates/asura-world/src/item.rs:663-694`, `pick_up`).
fn pick_up(
    characters: &DatabaseCharacters,
    uuid: &str,
    rows: &[HeldItem],
    bags: &[Bag],
    catalogue: Option<&crate::data_stores::item_catalogue::Catalogue>,
    ground: &crate::districts::GroundItem,
    wearer: Wearer,
) -> Result<Result<item_ops::Applied, crate::characters::Refusal>, String> {
    if let Some(source) = ground.source_item_id
        && rows.iter().any(|row| row.item_id == source)
    {
        let merge = Op::Merge {
            into: source,
            count: ground.item.quantity,
        };
        if let Ok(applied) =
            item_ops::commit(characters, uuid, rows, bags, catalogue, &[merge], wearer)?
        {
            return Ok(Ok(applied));
        }
    }
    let entry = item_ops::entry_of(&ground.item, catalogue)?;
    item_ops::commit_ops(
        characters,
        uuid,
        rows,
        bags,
        catalogue,
        &[Op::Insert { entry }],
        wearer,
        std::slice::from_ref(&ground.item),
    )
}

/// How far from the player the reference puts an item it drops: `Vec2fAdd(position, (30, 30))`
/// (`ASURA/crates/asura-world/src/item.rs:1036`, `DROP_OFFSET`).
const DROP_OFFSET: f32 = 30.0;

/// The four coded-string words the live server puts before an item's own name units on the drop line
/// (`items-live-20261108-173913.jsonl` seq 44: `F2 07 B4 AC 04 A7 4B 31`).
const DROP_LINE: [u16; 4] = [0x07F2, 0xACB4, 0xA704, 0x314B];

/// And on the pickup line (`items-live-20261108-173913.jsonl` seq 58: `FC 07 59 C5 E7 A2 E3 05`).
const PICKUP_LINE: [u16; 4] = [0x07FC, 0x59C5, 0xE7A2, 0xE305];

/// The chat line the live server commits on a drop or a pickup: a fixed four-word message prefix and
/// the item's own coded name, then the server tag on `Channel_Global`
/// (`ASURA/crates/asura-world/src/item.rs:194-205`, `item_line`).
fn item_line(prefix: [u16; 4], item: &HeldItem, player: u32) -> Vec<Vec<u8>> {
    let mut units = prefix.to_vec();
    units.extend(item.name.iter().copied());
    units.truncate(chat_packets::CHAT_MESSAGE_CORE_UNITS);
    vec![
        chat_packets::chat_message_core(&units),
        chat_packets::chat_message_server(
            u16::try_from(player).unwrap_or(0),
            chat_packets::CHANNEL_GLOBAL,
        ),
    ]
}

/// Whether an item may be dropped where this character stands: only in an explorable.
///
/// The client never offers a drop outside one (`docs/game-rules.md` E51: "the client itself refuses
/// to offer a drop outside an explorable map ... destroying a bag is allowed in outposts where
/// dropping is not"). ASURA's server has no such check because no client sends the message there;
/// this server refuses it as well, and the refusal is silent because no message acknowledges a drop
/// (`E47`).
fn drop_allowed(kind: Option<u8>) -> bool {
    kind == Some(crate::world::map_kind::EXPLORABLE)
}

/// The character's rows, or a log line and `None` when they cannot be read.
fn item_ops_rows(
    characters: &DatabaseCharacters,
    uuid: &str,
    peer: &str,
    session: &WorldSession,
) -> Option<Vec<HeldItem>> {
    match item_ops::reload(characters, uuid) {
        Ok(rows) => Some(rows),
        Err(error) => {
            session.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: the character's items cannot be read: {error}"
                ),
            );
            None
        }
    }
}

/// The frames one operation's changes amount to: this client's own list, and what the rest of the
/// instance is told.
///
/// `ASURA/crates/asura-world/src/item.rs:404-432`, `placement_frames`: a move is `0x014B`, a stack
/// change `0x0139`, a removal `0x014D`; a worn change is `0x006F` for the single visual slot, sent to
/// this client too unless the bulk `0x006E` is its answer (a drag).
fn placement_frames(
    changes: &[Change],
    agent: u32,
    own_visual: bool,
) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut own = Vec::new();
    let mut others = Vec::new();
    for change in changes {
        match *change {
            Change::Moved { item, to, .. } => own.push(item_packets::item_change_location(
                ITEM_STREAM,
                item,
                to.0,
                to.1,
            )),
            Change::Quantity { item, quantity } => {
                own.push(item_packets::item_update_quantity(item, quantity));
            }
            Change::Removed { item } => own.push(item_packets::item_remove(ITEM_STREAM, item)),
            Change::Worn { slot, item } => {
                let frame = item_packets::update_agent_visual_equipment_slot(
                    agent,
                    item_packets::visual_equipment_slot(slot),
                    item.unwrap_or(0),
                );
                if own_visual {
                    own.push(frame.clone());
                }
                others.push(frame);
            }
            // What a new item is announced with depends on why it appeared; its caller sends it.
            Change::Inserted { .. } => {}
        }
    }
    (own, others)
}

/// `UPDATE_AGENT_VISUAL_EQUIPMENT` for this character: the nine ids the agent is drawn wearing, read
/// out of the rows the operation left (`ASURA/crates/asura-world/src/item.rs:1031-1033`).
fn equipment_frame(agent: u32, rows: &[HeldItem], bags: &[Bag]) -> Vec<u8> {
    misc_packets::update_agent_visual_equipment(agent, item_ops::equipped_item_ids(rows, bags))
}

/// The frames that put a client's own copy of the item back where the rules say it is
/// (`ASURA/crates/asura-world/src/item.rs:450-460`, `snap_back`), for a drag or an equip it asked for.
fn snap_back(rows: &[HeldItem], item_id: u32) -> Vec<Vec<u8>> {
    item_ops::snap_back(rows, item_id)
        .map(|(bag, slot)| {
            vec![item_packets::item_change_location(
                ITEM_STREAM,
                item_id,
                bag,
                slot,
            )]
        })
        .unwrap_or_default()
}

/// The item worn in an equipped-bag slot, for the snap-back an unequip that was refused answers with.
fn worn_item(rows: &[HeldItem], bags: &[Bag], slot: u8) -> Option<u32> {
    let bag = bags.iter().find(|bag| bag.model == EQUIPPED_ITEMS_MODEL)?;
    rows.iter()
        .find(|row| row.bag_id == bag.id && row.slot == slot)
        .map(|row| row.item_id)
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

/// The comparison spelling for `/item`: case-folded, with underscores and hyphens treated as spaces,
/// so `skale_fin`, `skale-fin` and `Skale Fin` are one query
/// (`ASURA/crates/asura-world/src/item.rs:1250-1260`).
fn grant_key(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '_' | '-' => ' ',
            other => other.to_ascii_lowercase(),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse `/item`'s argument: the last token is a quantity only when it is all digits. A name may
/// contain spaces, so `skale fin 5` is five Skale Fins
/// (`ASURA/crates/asura-world/src/item.rs:1264-1277`).
fn grant_request(argument: &str) -> Option<(&str, u32)> {
    let argument = argument.trim();
    if argument.is_empty() {
        return None;
    }
    if let Some((name, quantity)) = argument.rsplit_once(' ') {
        if !quantity.is_empty() && quantity.chars().all(|ch| ch.is_ascii_digit()) {
            let quantity = quantity.parse::<u32>().ok()?;
            let name = name.trim();
            return (!name.is_empty() && quantity > 0).then_some((name, quantity));
        }
    }
    Some((argument, 1))
}

/// The frames and the two destinations an operation's answer goes to.
impl WorldSession {
    /// Send what the rest of the instance is told first, then this client's own answer - the order
    /// `ASURA/crates/asura-world/src/item.rs:435-446` (`deliver`) uses.
    async fn deliver(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        own: Vec<Vec<u8>>,
        others: Vec<Vec<u8>>,
        what: &str,
    ) {
        if !others.is_empty()
            && let Ok(peers) = self.peers.lock()
        {
            peers.broadcast(self.player_id(), self.map_id(), self.district(), &others);
        }
        if !own.is_empty() {
            self.send(io, peer, crypt, &own, what).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{grant_key, grant_request}; // `ASURA/crates/asura-world/src/item.rs:1284-1290`.
    #[test]
    fn an_item_name_may_contain_spaces_and_an_optional_quantity() {
        assert_eq!(grant_request("skale fin"), Some(("skale fin", 1)));
        assert_eq!(grant_request("skale fin 5"), Some(("skale fin", 5)));
        assert_eq!(grant_request("  worn belt  "), Some(("worn belt", 1)));
        assert_eq!(grant_request(""), None);
        assert_eq!(grant_request("skale fin 0"), None);
    }

    // `ASURA/crates/asura-world/src/item.rs:1293-1297`.
    #[test]
    fn an_item_query_ignores_case_and_separator_spelling() {
        assert_eq!(grant_key("Skale Fin"), "skale fin");
        assert_eq!(grant_key("skale_fin"), "skale fin");
        assert_eq!(grant_key("skale-fin"), "skale fin");
    }

    /// The client never offers a drop outside an explorable (`docs/game-rules.md` E51); the server
    /// refuses one all the same.
    #[test]
    fn a_drop_is_only_allowed_in_an_explorable() {
        use crate::world::map_kind;
        assert!(super::drop_allowed(Some(map_kind::EXPLORABLE)));
        assert!(!super::drop_allowed(Some(map_kind::OUTPOST)));
        assert!(!super::drop_allowed(Some(map_kind::CREATION)));
        assert!(!super::drop_allowed(None));
    }
}
