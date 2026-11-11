//! What a character-creation screen is made of: the inventory a client is given, and the block it is
//! answered with.
//!
//! Reference: the creation instance is the one map type that is a conversation rather than a place -
//! `MapType_CharacterCreation` - and its four handlers are `GameSrv_HandleCharCreation*`. The data is
//! the reference's own: its six bags, the items it creates before anything else, and what a new
//! character wears per campaign and profession (`GetDefaultEquipments`, `GmDefaultArmors.c`).
//!
//! The working implementation this port follows is `ASURA/crates/asura-world/src/creation.rs` and
//! `item.rs`. The two data files are imported from its rows by `tools/import-creation-inventory.py`,
//! so the numbers here are the reference's and not this tree's invention.

use common::configuration::json::Document;
use common::logging::{Log, LogLevel};

use crate::server::packets::{agent_packets, character_packets, instance_packets, item_packets};

/// A bag of the client's inventory model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bag {
    /// The bag's own id, which the item messages name.
    pub id: u16,
    /// The model the client draws.
    pub model: u8,
    /// The bag's kind: 1 bag, 2 equipped, 3 unclaimed, 4 storage, 5 material storage.
    pub kind: u8,
    /// How many slots it has.
    pub slots: u8,
    /// The item that *is* this bag, or zero when it is not an item.
    pub assoc_item: u32,
}

/// One item, in the field order the client is told about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The item's runtime id. The reference allocates one per item; this tree renumbers each set, so
    /// two characters cannot own the same id.
    pub id: u32,
    /// The bag it sits in, or `None` for an item that is nowhere yet.
    pub bag_id: Option<u16>,
    /// Its slot in that bag.
    pub slot: Option<u8>,
    /// The `Gw.dat` file id the client draws.
    pub file_id: u32,
    /// The item's kind code; its slot in the equipped bag comes from it.
    pub item_type: u8,
    /// Which dye slot mask applies.
    pub dye_tint: u8,
    /// The four packed dye colours.
    pub dye_colors: u16,
    /// Crafting materials, as the reference packs them.
    pub materials: u16,
    /// The reference's own unverified byte.
    pub unk1: u8,
    /// Item flags.
    pub flags: u32,
    /// The item's value in gold.
    pub value: u32,
    /// The model id the client draws.
    pub model_id: u32,
    /// How many are in the stack.
    pub quantity: u32,
    /// The profession the item was made for, or zero for none.
    pub profession: u8,
    /// The name, as UTF-16 code units.
    pub name: Vec<u16>,
    /// The modifier list, as dwords.
    pub modifiers: Vec<u32>,
}

/// One (campaign, profession) pair and what a character of it starts wearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipmentSet {
    /// Which campaign: `0` is Prophecies, and the client's own numbering.
    pub campaign: u8,
    /// Which profession, the client's own numbering.
    pub profession: u8,
    /// The items, in the reference's table order.
    pub items: Vec<Item>,
}

/// The bags and items a creation screen is given, and the sets it may choose from.
#[derive(Debug, Clone, Default)]
pub struct Tables {
    bags: Vec<Bag>,
    /// The items the reference creates before a profession is chosen - the backpack's own item and
    /// the ones that hang off the other bags.
    items: Vec<Item>,
    sets: Vec<EquipmentSet>,
}

/// What one creation screen's inventory is: the bags, and every item in them.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    /// The bags, in the reference's order.
    pub bags: Vec<Bag>,
    /// Every item, with the id it will be named by.
    pub items: Vec<Item>,
}

/// `EquippedItemSlot_Weapon` and friends, in the client's order.
const SLOT_WEAPON: u8 = 0;
const SLOT_OFF_HAND: u8 = 1;
const SLOT_BODY: u8 = 2;
const SLOT_LEGS: u8 = 3;
const SLOT_HEAD: u8 = 4;
const SLOT_BOOTS: u8 = 5;
const SLOT_GLOVES: u8 = 6;
const SLOT_COSTUME_BODY: u8 = 7;
const SLOT_COSTUME_HEAD: u8 = 8;

/// The slot an item type is worn in, or `None` for something that is not worn.
///
/// Evidence: `ASURA/crates/asura-protocol/src/game/mod.rs`, `equipped_slot`, which is the reference's
/// `EquippedItemSlot` list. A caller that gets `None` refuses rather than inventing a slot.
pub fn equipped_slot(item_type: u8) -> Option<u8> {
    let slot = match item_type {
        // `ItemType_Leadhand`, `Axe`, `Bow`, `Hammer`, `Wand`, `Staff`, `Sword`, `Daggers`, `Scythe`,
        // `Spear`.
        1 | 2 | 5 | 15 | 22 | 26 | 27 | 32 | 35 | 36 => SLOT_WEAPON,
        // `ItemType_Focus`, `Shield`.
        12 | 24 => SLOT_OFF_HAND,
        7 => SLOT_BODY,
        19 => SLOT_LEGS,
        16 => SLOT_HEAD,
        4 => SLOT_BOOTS,
        13 => SLOT_GLOVES,
        44 => SLOT_COSTUME_BODY,
        45 => SLOT_COSTUME_HEAD,
        _ => return None,
    };
    Some(slot)
}

/// The bag a character wears things in: `BagType_Equipped`.
const BAG_EQUIPPED: u8 = 2;

impl Tables {
    /// Read the two record lists: the inventory and the profession sets.
    ///
    /// A file that is missing is reported and leaves that half empty, so a world can still be started
    /// with what it has - and the log says what it does not have.
    pub fn load(
        inventory_path: impl AsRef<std::path::Path>,
        equipment_path: impl AsRef<std::path::Path>,
        log: &Log,
    ) -> Result<Self, String> {
        let mut tables = Tables::default();

        let inventory_path = inventory_path.as_ref();
        if inventory_path.exists() {
            let document = Document::read(inventory_path).map_err(|error| error.to_string())?;
            for record in document.records("bags")? {
                tables.bags.push(Bag {
                    id: number(&record, "bag_id")? as u16,
                    model: number(&record, "model")? as u8,
                    kind: number(&record, "type")? as u8,
                    slots: number(&record, "slots")? as u8,
                    assoc_item: number(&record, "assoc_item")? as u32,
                });
            }
            for record in document.records("items")? {
                tables.items.push(item(&record, None, None)?);
            }
        } else {
            log.out(
                LogLevel::Warn,
                "server.worldserver",
                format!(
                    "No creation inventory at {}: a creation screen will be sent no items.",
                    inventory_path.display()
                ),
            );
        }

        let equipment_path = equipment_path.as_ref();
        if equipment_path.exists() {
            let document = Document::read(equipment_path).map_err(|error| error.to_string())?;
            for record in document.records("sets")? {
                let campaign = number(&record, "campaign")? as u8;
                let profession = number(&record, "profession")? as u8;
                let mut items = Vec::new();
                for row in record.rows("items")? {
                    items.push(item(&row, None, None)?);
                }
                tables.sets.push(EquipmentSet {
                    campaign,
                    profession,
                    items,
                });
            }
        } else {
            log.out(
                LogLevel::Warn,
                "server.worldserver",
                format!(
                    "No equipment table at {}: a creation screen will show no clothing.",
                    equipment_path.display()
                ),
            );
        }

        Ok(tables)
    }

    /// How many bags, items and sets were read, for the program's own log.
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.bags.len(), self.items.len(), self.sets.len())
    }

    /// The set for a choice, or the campaign's first set when the choice is not in the table.
    pub fn set(&self, campaign: u8, profession: u8) -> Option<&EquipmentSet> {
        self.sets
            .iter()
            .find(|set| set.campaign == campaign && set.profession == profession)
    }

    /// The inventory a creation screen is given for a choice: the reference's bags, the items it
    /// creates before anything else, and the chosen profession's equipment in the equipped bag.
    ///
    /// The ids are this tree's own, allocated from one per screen: the reference renumbers too
    /// (`GameSrv_AllocateItem`), because an id is a runtime allocation and two characters cannot own
    /// the same one.
    pub fn inventory(&self, campaign: u8, profession: u8) -> Inventory {
        let equipped_bag = self
            .bags
            .iter()
            .find(|bag| bag.kind == BAG_EQUIPPED)
            .map(|bag| bag.id);

        let mut items: Vec<Item> = Vec::with_capacity(self.items.len() + 16);
        let mut next_id = 1u32;

        // The items that are not the equipped set: the backpack's own item and the ones that hang
        // off the other bags. A bag names its own item by seed id, so the map is kept.
        let mut by_seed: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for seed in &self.items {
            if equipped_bag.is_some() && seed.bag_id == equipped_bag {
                continue;
            }
            by_seed.insert(seed.id, next_id);
            let mut placed = seed.clone();
            placed.id = next_id;
            items.push(placed);
            next_id += 1;
        }

        if let Some(set) = self.set(campaign, profession) {
            for spec in &set.items {
                let mut placed = spec.clone();
                placed.id = next_id;
                placed.bag_id = equipped_bag;
                placed.slot = equipped_slot(spec.item_type);
                items.push(placed);
                next_id += 1;
            }
        }

        let bags = self
            .bags
            .iter()
            .map(|bag| Bag {
                id: bag.id,
                model: bag.model,
                kind: bag.kind,
                slots: bag.slots,
                assoc_item: by_seed.get(&bag.assoc_item).copied().unwrap_or(0),
            })
            .collect();

        Inventory { bags, items }
    }
}

impl Inventory {
    /// The items in one bag, in the order the file states them.
    pub fn items_in(&self, bag_id: u16) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|item| item.bag_id == Some(bag_id))
            .collect()
    }

    /// The equipped bag's items: what the screen shows, and what has to come off before another set
    /// goes into the same slots.
    pub fn equipped_items(&self) -> Vec<&Item> {
        let Some(equipped) = self.bags.iter().find(|bag| bag.kind == BAG_EQUIPPED) else {
            return Vec::new();
        };
        self.items_in(equipped.id)
    }
}

/// A creation screen's player-data block, in the reference's order.
///
/// Reference: `GameSrv_HandleCharCreationRequestPlayer` - the player-data start, the inventory stream
/// (bags and items, each item placed in its slot), the four empty weapon sets and the gold, the four
/// faction caps, the attribute points, the profession, the skillbar, the skill and attribute records,
/// the creation path's own energy and health, and the player-data end.
pub fn player_data_block(tables: &Tables, campaign: u8, profession: u8) -> Vec<Vec<u8>> {
    let agent = crate::server::world_session::CREATION_AGENT;
    let inventory = tables.inventory(campaign, profession);

    let mut frames = vec![instance_packets::instance_player_data_start()];
    frames.push(item_packets::item_stream_create(
        item_packets::ITEM_STREAM,
        0,
    ));
    frames.extend(inventory_block(&inventory));
    frames.push(item_packets::item_set_active_weapon_set(
        item_packets::ITEM_STREAM,
        0,
    ));
    for slot in 0..4u8 {
        frames.push(item_packets::item_weapon_set(
            item_packets::ITEM_STREAM,
            slot,
            0,
            0,
        ));
    }
    frames.push(item_packets::update_gold_storage(
        item_packets::ITEM_STREAM,
        0,
    ));

    for header in [
        character_packets::FACTION_MAX_KURZICK,
        character_packets::FACTION_MAX_LUXON,
        character_packets::FACTION_MAX_BALTHAZAR,
        character_packets::FACTION_MAX_IMPERIAL,
    ] {
        frames.push(character_packets::faction_max(header, 10_000));
    }

    frames.push(agent_packets::agent_update_attribute_points(agent, 50, 50));
    frames.push(character_packets::player_update_profession(
        agent, profession, 0, 0,
    ));
    frames.push(character_packets::player_update_unlocked_professions(
        agent,
        character_packets::CREATION_UNLOCKED_PROFESSIONS,
    ));
    frames.push(character_packets::skillbar_update(
        agent,
        &[0u32; 8],
        &[0u32; 8],
        1,
    ));
    frames.push(character_packets::update_unlocked_skills(&[]));
    frames.push(character_packets::character_update_factions(&[0u32; 13]));
    frames.push(agent_packets::agent_update_attributes(agent, &[0u32; 42]));

    // The creation path's own numbers: `GameSrv_SendPlayerHealthEnergy` sends energy 20, health 100
    // and a zero for the third property. A map load's numbers differ, and mixing them is a mismatch.
    frames.push(agent_packets::agent_property_update_int(
        agent_packets::AGENT_PROPERTY_ENERGY,
        agent,
        20,
    ));
    frames.push(agent_packets::agent_property_update_int(
        agent_packets::AGENT_PROPERTY_HEALTH,
        agent,
        100,
    ));
    frames.push(agent_packets::agent_property_update_int(
        agent_packets::AGENT_PROPERTY_VALUE64,
        agent,
        0,
    ));

    frames.push(instance_packets::instance_player_data_done());
    frames
}

/// The inventory stream: per bag its own item, the bag, and then every item in it with its slot.
pub fn inventory_block(inventory: &Inventory) -> Vec<Vec<u8>> {
    let mut frames = Vec::with_capacity(inventory.bags.len() * 2 + inventory.items.len() * 3);    for bag in &inventory.bags {
        // A bag that is also an item is created before the bag, because the bag names it.
        if bag.assoc_item != 0
            && let Some(item) = inventory
                .items
                .iter()
                .find(|item| item.id == bag.assoc_item)
        {
            frames.push(item_packets::create_named_item(&named_item(item)));
        }

        frames.push(item_packets::inventory_create_bag(
            item_packets::ITEM_STREAM,
            bag.kind,
            bag.model,
            bag.id,
            bag.slots,
            bag.assoc_item,
        ));

        for item in inventory.items_in(bag.id) {
            frames.push(item_packets::create_named_item(&named_item(item)));
            if item.profession != 0 {
                frames.push(item_packets::item_set_profession(item.id, item.profession));
            }
            frames.push(item_packets::item_move_to_location(
                item_packets::ITEM_STREAM,
                item.id,
                bag.id,
                item.slot.unwrap_or(0),
            ));
        }
    }
    frames
}

/// The outpost load's item stream: the stream opens, the active set, the inventory, the four weapon
/// sets, the gold, and the four faction caps.
///
/// Reference: `GameSrv_HandleRequestItems`-shaped load traffic, measured from a working server in
/// `recordings/gwps/world/load-stream.log` - `0x0144`, `0x0148`, the bag and item frames, `0x0147`
/// four times, `0x0141`, then `0x00EA` to `0x00ED`. A map load carries no player-data end (`0x018A`):
/// the reference sends the creation screen's wrapper and a map's stream do not share it.
pub fn load_item_stream(tables: &Tables, campaign: u8, profession: u8) -> Vec<Vec<u8>> {
    let inventory = tables.inventory(campaign, profession);
    let mut frames = vec![
        item_packets::item_stream_create(item_packets::ITEM_STREAM, 0),
        item_packets::item_set_active_weapon_set(item_packets::ITEM_STREAM, 0),
    ];
    frames.extend(inventory_block(&inventory));
    for slot in 0..4u8 {
        frames.push(item_packets::item_weapon_set(
            item_packets::ITEM_STREAM,
            slot,
            0,
            0,
        ));
    }
    frames.push(item_packets::update_gold_storage(item_packets::ITEM_STREAM, 0));
    for header in [
        character_packets::FACTION_MAX_KURZICK,
        character_packets::FACTION_MAX_LUXON,
        character_packets::FACTION_MAX_BALTHAZAR,
        character_packets::FACTION_MAX_IMPERIAL,
    ] {
        frames.push(character_packets::faction_max(header, 10_000));
    }
    frames
}

/// An item as `CREATE_NAMED_ITEM` carries it.
pub fn named_item(item: &Item) -> item_packets::NamedItem {
    item_packets::NamedItem {
        id: item.id,
        file_id: item.file_id,
        item_type: item.item_type,
        dye_tint: item.dye_tint,
        dye_colors: item.dye_colors,
        materials: item.materials,
        unk1: item.unk1,
        flags: item.flags,
        value: item.value,
        model_id: item.model_id,
        quantity: item.quantity,
        name: item.name.clone(),
        modifiers: item.modifiers.clone(),
    }
}

/// A row's number, by name.
fn number(record: &common::configuration::json::Record<'_>, field: &str) -> Result<u64, String> {
    record
        .required(field)?
        .parse()
        .map_err(|_| format!("{}: '{}' is not a number", record.at(), field))
}

/// One item row, with the bag and slot it is placed in when the row states them.
fn item(
    record: &common::configuration::json::Record<'_>,
    bag_id: Option<u16>,
    slot: Option<u8>,
) -> Result<Item, String> {
    Ok(Item {
        id: record
            .text("id")?
            .map(|text| text.parse().unwrap_or(0))
            .unwrap_or(0),
        bag_id: record
            .text("bag_id")?
            .and_then(|text| text.parse().ok())
            .or(bag_id),
        slot: record
            .text("slot")?
            .and_then(|text| text.parse().ok())
            .or(slot),
        file_id: number(record, "file_id")? as u32,
        item_type: number(record, "item_type")? as u8,
        dye_tint: number(record, "dye_tint")? as u8,
        dye_colors: number(record, "dye_colors")? as u16,
        materials: number(record, "materials")? as u16,
        unk1: number(record, "unk1")? as u8,
        flags: number(record, "flags")? as u32,
        value: number(record, "value")? as u32,
        model_id: number(record, "model_id")? as u32,
        quantity: number(record, "quantity")? as u32,
        profession: record
            .text("profession")?
            .map(|text| text.parse().unwrap_or(0))
            .unwrap_or(0),
        name: record
            .numbers("name")?
            .unwrap_or_default()
            .into_iter()
            .map(|unit| unit as u16)
            .collect(),
        modifiers: record.numbers("modifiers")?.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> Tables {
        let log = Log::new();
        Tables::load("../data/inventory.json", "../data/equipment.json", &log)
            .expect("the creation data reads")
    }

    #[test]
    fn the_data_is_the_references() {
        let tables = tables();
        let (bags, items, sets) = tables.counts();
        assert_eq!(bags, 6, "the reference creates six bags");
        assert_eq!(items, 9, "and nine items before a profession is chosen");
        assert_eq!(sets, 32, "thirty-two (campaign, profession) sets");
        let total: usize = tables.sets.iter().map(|set| set.items.len()).sum();
        assert_eq!(total, 179, "and a hundred and seventy-nine items between them");
    }

    #[test]
    fn a_choice_is_given_its_own_equipment_in_the_equipped_bag() {
        let tables = tables();
        let inventory = tables.inventory(0, 1);
        let equipped = inventory.equipped_items();
        assert!(!equipped.is_empty(), "a Warrior is given something to wear");
        assert!(
            equipped.iter().all(|item| equipped_slot(item.item_type).is_some()),
            "every equipped item is worn somewhere"
        );
        // Every id in the screen's inventory is its own: a client keys its items by them.
        let mut ids: Vec<u32> = inventory.items.iter().map(|item| item.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), inventory.items.len(), "no id is handed out twice");
    }

    #[test]
    fn the_block_opens_with_the_stream_and_closes_with_the_end() {
        let tables = tables();
        let block = player_data_block(&tables, 0, 1);
        let headers: Vec<u16> = block
            .iter()
            .map(|frame| u16::from_le_bytes([frame[0], frame[1]]))
            .collect();
        assert_eq!(headers[0], 0x0186, "the player-data start");
        assert_eq!(headers[1], 0x0144, "the item stream opens");
        assert!(headers.contains(&0x013F), "a bag is created");
        assert!(headers.contains(&0x0161), "an item is created");
        assert!(headers.contains(&0x013E), "and placed");
        assert_eq!(
            *headers.last().unwrap(),
            0x018A,
            "the player-data end closes the block"
        );
    }

    #[test]
    fn every_frame_carries_its_own_header() {
        let tables = tables();
        for frame in player_data_block(&tables, 0, 1) {
            assert!(frame.len() >= 2);
        }
    }
}
