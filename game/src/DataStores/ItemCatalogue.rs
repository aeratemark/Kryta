//! Static item definitions, and the owned items that name them.
//!
//! Ported from `ASURA/crates/asura-world/src/item_catalogue.rs` (636 lines), read from the local ASURA
//! tree on 2026-11-22. Carried: the constants `:24-41`, `ItemDefinition` `:43-98`, `Upgrade` `:100-118`,
//! the definition's three rules `:128-153`, `Catalogue` and its lookups `:164-291`, `validate`
//! `:293-365`, `load` `:367-384`, `name_units` `:411-416`, and the 11 tests `:418-636`. Each item below
//! carries its ASURA line.
//!
//! Three differences of arrangement, none of behaviour:
//!
//! - ASURA derives its readers with `serde(deny_unknown_fields)`; this tree reads pack files through
//!   `serde_json::Value` (`game/Characters/Experience.rs` does the same), so [`fields`] states each
//!   object's keys and refuses any other one, and an absent `Option` or `#[serde(default)]` field reads
//!   as serde would read it.
//! - ASURA's `current()` (`:391-409`) reads the pack lazily on first use and `path()` (`:386-389`) asks
//!   its spawn module for the pack. Here a request path reads no file
//!   (`worldserver/tests/content_is_held.rs`), so the world reads the catalogue once at start-up
//!   (`World::load_item_catalogue`) and a missing `items.json` stops the program (`AGENTS.md`, rule 7).
//! - ASURA's `for_item` (`:278-291`) takes a `t_item` row. This tree's item rows carry no
//!   `definition_id` column yet, so [`Catalogue::for_item`] takes the four fields it reads, and a row
//!   passes `None` for the id until the column arrives with the `item.rs` re-diff.
//!
//! The pack files are `config/World/Vanilla/items.json` and `upgrades.json`, copied from ASURA's pack.
//!
//! ASURA's own module comment follows, unchanged in meaning:
//!
//! An owned item is a row in `t_item` (`asura_db::inventory::Item`): it has an id, a bag and slot,
//! a quantity, dye, flags and the modifier words the wire carries. A definition is content: the
//! model, file id, item type, default name, inherent modifiers, stack behaviour and the equipment
//! positions the item may occupy. Definitions live in the world pack
//! (`config/World/Vanilla/items.json`, ASURA `docs/38-layer-decisions.md` D20-D24), never in a table;
//! `t_item.definition_id` is a reference to this catalogue and nothing else.
//!
//! The boundary is deliberately one-way. A definition never holds an item id, a bag, a slot, a
//! quantity, a dye, an identification flag or an instance's actual modifier list; those belong to
//! the row. A row keeps every wire field it already had, so a database written before this module
//! existed still rebuilds `CREATE_NAMED_ITEM` byte for byte with `definition_id` null.
//!
//! [`ItemDefinition::modifiers`] is the default list a new instance starts with. The row's `modifiers`
//! is the list the client is actually sent, including upgrades and runes, and
//! [`crate::data_stores::item_modifier`] interprets that list. Do not recompute a row's wire fields
//! from a definition: the row is the evidence of what this instance is.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Map, Value};

/// The file inside a world pack that holds the item definitions.
// ASURA/crates/asura-world/src/item_catalogue.rs:25
pub const FILE: &str = "items.json";
/// The file inside a world pack that holds the upgrades (insignias, runes, weapon mods). Optional: a
/// pack without it has no upgrade definitions, and then no item in it may list a fitted upgrade.
// ASURA/crates/asura-world/src/item_catalogue.rs:28
pub const UPGRADES_FILE: &str = "upgrades.json";

/// The first word of a name the client *composed* for display (`0xa3f(...)`, `0xa40(...)`, and the
/// `0xa30`/`0xa31`/`0xa33` base-plus-upgrade templates inside them). The live server never sends one:
/// across 584 live `CREATE_NAMED_ITEM` records no name starts with these. Sending one for an item with
/// upgrades made the client add the upgrades' names a second time ("Knight's Knight's Templar Helm of
/// Vitae of Vitae"), so a definition with upgrades must carry its bare base name.
// ASURA/crates/asura-world/src/item_catalogue.rs:35
pub const COMPOSED_NAME_WORDS: [u16; 8] = [0xA30, 0xA31, 0xA33, 0xA3D, 0xA3F, 0xA40, 0xA42, 0xA43];

/// The most name units and modifier words `CREATE_NAMED_ITEM` can carry (both counted fields are
/// bounded by the wire: 64 units and a one-byte count whose practical limit is 64 dwords).
// ASURA/crates/asura-world/src/item_catalogue.rs:39
pub const MAX_NAME_UNITS: usize = 64;
/// See [`MAX_NAME_UNITS`].
// ASURA/crates/asura-world/src/item_catalogue.rs:41
pub const MAX_MODIFIERS: usize = 64;

/// One static item definition.
#[derive(Debug, Clone, PartialEq, Eq)]
// ASURA/crates/asura-world/src/item_catalogue.rs:46
pub struct ItemDefinition {
    /// The stable key an owned item writes into `t_item.definition_id`.
    pub id: String,
    /// True when the client's own item probe has shown this exact wire identity. Only these
    /// definitions are safe to hand the client: an unverified file id can be missing its UI texture.
    pub live: bool,
    /// What this definition is, for the person opening the file.
    pub doc: String,
    /// The `Gw.dat` entry the model comes from, as `CREATE_NAMED_ITEM.file_id`.
    pub file_id: u32,
    /// The model the client draws, as `CREATE_NAMED_ITEM.model_id`.
    pub model_id: u32,
    /// The client's `ItemType` (`GmItem.h`).
    pub item_type: u8,
    /// The default name, as the UTF-16 code units the wire carries. Empty for an item whose name
    /// the client resolves from the model.
    pub name: Vec<u16>,
    /// A readable description for authors. Never sent: the client's item name is `name`.
    pub description: String,
    /// The item's merchant value, as `CREATE_NAMED_ITEM.value`.
    pub value: u32,
    /// Crafting materials the item carries or needs, as `CREATE_NAMED_ITEM.materials`.
    pub materials: u16,
    /// The reference's own unverified byte, copied from the row that defines it.
    pub unk1: u8,
    /// The interaction flags a new instance starts with. Bit 0 is the client's
    /// `GetIsIdentified`; without it the inventory draws the hourglass icon.
    pub flags: u32,
    /// The profession an item was made for, or 0 for `Profession_None`.
    pub profession: u8,
    /// The item's inherent modifier words, as the client's own dwords.
    pub modifiers: Vec<u32>,
    /// Whether more than one can sit in a stack.
    pub stackable: bool,
    /// The largest stack this definition allows. `null` means no source has measured it; a caller
    /// that would merge into it refuses rather than inventing a limit.
    pub max_stack: Option<u32>,
    /// The equipped-items bag slots this item may occupy (`EquippedItemSlot`): 0 weapon, 1 off
    /// hand, 2 body, 3 legs, 4 head, 5 boots, 6 gloves, 7 costume body, 8 costume head.
    pub equip_positions: Vec<u8>,
    /// Named restrictions. Empty means nothing beyond the normal item rules applies.
    pub restrictions: Vec<String>,
    /// The base this definition is a copy of: two definitions with the same `base` are the same item
    /// with different upgrades or rolls ("Wyvern Boots" plain, and "Knight's Wyvern Boots of Superior
    /// Absorption"). `None` for a hand-written definition that does not say.
    pub base: Option<String>,
    /// The upgrades fitted to this copy, by id (`upgrades.json`), in the order its modifier words list
    /// them. The words themselves stay in `modifiers`: an upgrade's effect value is rolled per item.
    pub upgrades: Vec<u16>,
}

/// One upgrade: an insignia, a rune or a weapon mod. Its identity is the `0x2408XXXX` word it puts in
/// an item's modifier list; its words' values live in the item, because they are rolled.
#[derive(Debug, Clone, PartialEq, Eq)]
// ASURA/crates/asura-world/src/item_catalogue.rs:104
pub struct Upgrade {
    /// The id in the upgrade's `0x2408XXXX` word.
    pub id: u16,
    /// `prefix` (an insignia, a weapon prefix), `suffix` (a rune, a weapon suffix) or `unknown` (the
    /// client does not name it: an inscription, a built-in mod).
    pub role: String,
    /// The fragment the client composes into the item's display name. Never sent by the server.
    pub name: Vec<u16>,
    /// The top 16 bits of each word it contributes, in order: the id word, then a pair word and an
    /// effect word per effect.
    pub shape: Vec<u16>,
    /// The probed items it was read from, for the reader.
    pub seen_on: Vec<String>,
}

// ASURA/crates/asura-world/src/item_catalogue.rs:128
impl ItemDefinition {
    /// The largest stack, or `None` when the definition did not measure one. A non-stackable item
    /// is a stack of one by definition, so it answers `Some(1)` even when `max_stack` is null.
    // ASURA/crates/asura-world/src/item_catalogue.rs:131
    pub fn stack_limit(&self) -> Option<u32> {
        if self.stackable {
            self.max_stack
        } else {
            Some(1)
        }
    }

    /// Whether this definition may occupy an equipped-items bag slot.
    // ASURA/crates/asura-world/src/item_catalogue.rs:140
    pub fn can_equip_in(&self, slot: u8) -> bool {
        self.equip_positions.contains(&slot)
    }

    /// Whether this definition can be put in a stack of `quantity`. One copy is always legal;
    /// anything more needs a measured maximum, so an unmeasured stackable definition refuses a
    /// merge instead of choosing a limit.
    // ASURA/crates/asura-world/src/item_catalogue.rs:147
    pub fn accepts_quantity(&self, quantity: u32) -> bool {
        match self.stack_limit() {
            Some(max) => quantity > 0 && quantity <= max,
            None => quantity == 1,
        }
    }

    /// The default name as the wire writes it.
    // ASURA/crates/asura-world/src/item_catalogue.rs:413
    pub fn name_units(&self) -> Vec<u16> {
        self.name.clone()
    }
}

/// A loaded catalogue, indexed by key and by the wire fields a row carries.
#[derive(Debug, Clone, Default)]
// ASURA/crates/asura-world/src/item_catalogue.rs:166
pub struct Catalogue {
    definitions: Vec<ItemDefinition>,
    by_id: BTreeMap<String, usize>,
    by_wire: BTreeMap<(u32, u8, u32), usize>,
    upgrades: BTreeMap<u16, Upgrade>,
}

// ASURA/crates/asura-world/src/item_catalogue.rs:173
impl Catalogue {
    /// Parse and validate a catalogue file. A malformed or ambiguous definition is an error, not a
    /// definition silently dropped: the world refuses to start rather than serving half a pack.
    // ASURA/crates/asura-world/src/item_catalogue.rs:176
    pub fn from_json(text: &str) -> Result<Catalogue, String> {
        let definitions = definition_file(text).map_err(|err| format!("items.json: {err}"))?;
        if definitions.is_empty() {
            return Err("items.json: definitions is empty".to_string());
        }

        let mut by_id = BTreeMap::new();
        let mut by_wire = BTreeMap::new();
        for (index, definition) in definitions.iter().enumerate() {
            validate(definition)?;
            if by_id.insert(definition.id.clone(), index).is_some() {
                return Err(format!(
                    "items.json: duplicate definition id {:?}",
                    definition.id
                ));
            }
            by_wire
                .entry((
                    definition.file_id,
                    definition.item_type,
                    definition.model_id,
                ))
                .or_insert(index);
        }

        Ok(Catalogue {
            definitions,
            by_id,
            by_wire,
            upgrades: BTreeMap::new(),
        })
    }

    /// Add the pack's upgrades and check every definition against them: each upgrade a definition
    /// lists must be defined here.
    // ASURA/crates/asura-world/src/item_catalogue.rs:213
    pub fn with_upgrades_json(mut self, text: &str) -> Result<Catalogue, String> {
        let upgrades = upgrade_file(text).map_err(|err| format!("{UPGRADES_FILE}: {err}"))?;
        for upgrade in upgrades {
            if upgrade.shape.first() != Some(&0x2408) {
                return Err(format!(
                    "{UPGRADES_FILE}: upgrade {:#x} does not start with its 0x2408 id word",
                    upgrade.id
                ));
            }
            if !matches!(upgrade.role.as_str(), "prefix" | "suffix" | "unknown") {
                return Err(format!(
                    "{UPGRADES_FILE}: upgrade {:#x} has role {:?}",
                    upgrade.id, upgrade.role
                ));
            }
            let id = upgrade.id;
            if self.upgrades.insert(id, upgrade).is_some() {
                return Err(format!("{UPGRADES_FILE}: upgrade {id:#x} is defined twice"));
            }
        }
        for definition in &self.definitions {
            if let Some(missing) = definition
                .upgrades
                .iter()
                .find(|id| !self.upgrades.contains_key(id))
            {
                return Err(format!(
                    "{FILE}: {} lists upgrade {missing:#x}, which {UPGRADES_FILE} does not define",
                    definition.id
                ));
            }
        }
        Ok(self)
    }

    /// The upgrade with this id.
    // ASURA/crates/asura-world/src/item_catalogue.rs:250
    pub fn upgrade(&self, id: u16) -> Option<&Upgrade> {
        self.upgrades.get(&id)
    }

    /// Every upgrade, by id.
    // ASURA/crates/asura-world/src/item_catalogue.rs:255
    pub fn upgrades(&self) -> impl Iterator<Item = &Upgrade> {
        self.upgrades.values()
    }

    /// Every definition, in file order.
    // ASURA/crates/asura-world/src/item_catalogue.rs:260
    pub fn definitions(&self) -> &[ItemDefinition] {
        &self.definitions
    }

    /// The definition with this stable key.
    // ASURA/crates/asura-world/src/item_catalogue.rs:265
    pub fn definition(&self, id: &str) -> Option<&ItemDefinition> {
        self.by_id.get(id).map(|index| &self.definitions[*index])
    }

    /// The first definition with these wire fields, for a row written before `definition_id`
    /// existed. A row with an explicit id must use [`Catalogue::definition`] instead: the id is
    /// the authoritative link even when two definitions share a model.
    // ASURA/crates/asura-world/src/item_catalogue.rs:272
    pub fn by_wire(&self, file_id: u32, item_type: u8, model_id: u32) -> Option<&ItemDefinition> {
        self.by_wire
            .get(&(file_id, item_type, model_id))
            .map(|index| &self.definitions[*index])
    }

    /// The definition an owned row names, falling back to the row's wire identity for a row that
    /// predates the column. `None` means this pack does not define the item; the row is still
    /// valid and still rebuilds its message.
    ///
    /// ASURA passes the `t_item` row and reads these four of its fields; the out-of-range fallback to
    /// `MAX` is ASURA's own.
    // ASURA/crates/asura-world/src/item_catalogue.rs:281
    pub fn for_item(
        &self,
        definition_id: Option<&str>,
        file_id: i64,
        item_type: i32,
        model_id: i64,
    ) -> Option<&ItemDefinition> {
        match definition_id {
            Some(id) => self.definition(id),
            None => self.by_wire(
                u32::try_from(file_id).unwrap_or(u32::MAX),
                u8::try_from(item_type).unwrap_or(u8::MAX),
                u32::try_from(model_id).unwrap_or(u32::MAX),
            ),
        }
    }
}

// ASURA/crates/asura-world/src/item_catalogue.rs:293
fn validate(definition: &ItemDefinition) -> Result<(), String> {
    if definition.id.trim().is_empty() {
        return Err("items.json: a definition has an empty id".to_string());
    }
    if definition.doc.trim().is_empty() {
        return Err(format!("items.json: {} has no doc", definition.id));
    }
    if definition.description.trim().is_empty() {
        return Err(format!("items.json: {} has no description", definition.id));
    }
    if definition.name.len() > MAX_NAME_UNITS {
        return Err(format!(
            "items.json: {} has {} name units; the wire allows {MAX_NAME_UNITS}",
            definition.id,
            definition.name.len()
        ));
    }
    if definition.modifiers.len() > MAX_MODIFIERS {
        return Err(format!(
            "items.json: {} has {} modifier words; the wire allows {MAX_MODIFIERS}",
            definition.id,
            definition.modifiers.len()
        ));
    }
    let positions: BTreeSet<u8> = definition.equip_positions.iter().copied().collect();
    if positions.len() != definition.equip_positions.len() {
        return Err(format!(
            "items.json: {} lists an equipment position twice",
            definition.id
        ));
    }
    if let Some(slot) = positions.iter().find(|slot| **slot > 8) {
        return Err(format!(
            "items.json: {} names equipped slot {slot}; the client has 0..8",
            definition.id
        ));
    }
    // The upgrades a definition lists are exactly the ones its words carry, in their order: the list
    // is an index into `modifiers`, never a second source of truth.
    let fitted = crate::data_stores::item_modifier::fitted_upgrades(&definition.modifiers);
    if fitted != definition.upgrades {
        return Err(format!(
            "items.json: {} lists upgrades {:x?}, and its modifier words carry {:x?}",
            definition.id, definition.upgrades, fitted
        ));
    }
    if !definition.upgrades.is_empty()
        && definition
            .name
            .first()
            .is_some_and(|word| COMPOSED_NAME_WORDS.contains(word))
    {
        return Err(format!(
            "items.json: {} has upgrades and a composed display name ({:#x}...); it must carry its bare \
             base name, or the client names its upgrades twice",
            definition.id, definition.name[0]
        ));
    }
    if definition.stackable {
        if definition.max_stack == Some(0) {
            return Err(format!(
                "items.json: {} is stackable with a maximum of zero",
                definition.id
            ));
        }
    } else if definition.max_stack.is_some_and(|max| max != 1) {
        return Err(format!(
            "items.json: {} is not stackable and its max_stack is not 1",
            definition.id
        ));
    }
    Ok(())
}

/// Read `<pack>/items.json`, and `<pack>/upgrades.json` when the pack has one.
// ASURA/crates/asura-world/src/item_catalogue.rs:368
pub fn load(pack: &Path) -> Result<Catalogue, String> {
    let path = pack.join(FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("{} is not readable: {err}", path.display()))?;
    let catalogue = Catalogue::from_json(&text)
        .map_err(|err| format!("{} is not an item catalogue: {err}", path.display()))?;
    let upgrades = pack.join(UPGRADES_FILE);
    let text = match std::fs::read_to_string(&upgrades) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // No upgrades file: an item may still load, but not one that lists an upgrade.
            return catalogue.with_upgrades_json(r#"{"upgrades":[]}"#);
        }
        Err(err) => return Err(format!("{} is not readable: {err}", upgrades.display())),
    };
    catalogue.with_upgrades_json(&text)
}

// The readers. ASURA's are `serde` derives with `deny_unknown_fields` (`:44-46`, `:102-103`,
// `:120-126`, `:156-162`); these state the same keys and the same defaults.

/// The keys of `items.json`'s top level (`DefinitionFile`, `:158-162`).
const DEFINITION_FILE_KEYS: [&str; 2] = ["note", "definitions"];
/// The keys of one definition (`ItemDefinition`, `:46-98`).
const DEFINITION_KEYS: [&str; 20] = [
    "id",
    "live",
    "doc",
    "file_id",
    "model_id",
    "item_type",
    "name",
    "description",
    "value",
    "materials",
    "unk1",
    "flags",
    "profession",
    "modifiers",
    "stackable",
    "max_stack",
    "equip_positions",
    "restrictions",
    "base",
    "upgrades",
];
/// The keys of `upgrades.json`'s top level (`UpgradeFile`, `:122-126`).
const UPGRADE_FILE_KEYS: [&str; 2] = ["note", "upgrades"];
/// The keys of one upgrade (`Upgrade`, `:104-118`).
const UPGRADE_KEYS: [&str; 5] = ["id", "role", "name", "shape", "seen_on"];

/// An object whose keys are all in `allowed`, as `deny_unknown_fields` holds it.
fn fields<'a>(
    value: &'a Value,
    allowed: &[&str],
    what: &str,
) -> Result<&'a Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{what} is not an object"))?;
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("{what} has unknown field `{key}`"));
    }
    Ok(object)
}

/// A field that must be present.
fn required<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    what: &str,
) -> Result<&'a Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{what} is missing field `{key}`"))
}

fn string(value: &Value, what: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{what} is not a string"))
}

fn boolean(value: &Value, what: &str) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| format!("{what} is not a boolean"))
}

/// A whole number that fits `T`, as serde reads an unsigned field.
fn whole<T: TryFrom<u64>>(value: &Value, what: &str) -> Result<T, String> {
    let number = value
        .as_u64()
        .ok_or_else(|| format!("{what} is not a whole number"))?;
    T::try_from(number).map_err(|_| format!("{what} is out of range: {number}"))
}

fn list<T>(
    value: &Value,
    what: &str,
    item: impl Fn(&Value, &str) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{what} is not a list"))?
        .iter()
        .map(|entry| item(entry, what))
        .collect()
}

/// A `#[serde(default)]` field: absent reads as the default.
fn or_default<T: Default>(
    object: &Map<String, Value>,
    key: &str,
    read: impl Fn(&Value) -> Result<T, String>,
) -> Result<T, String> {
    object.get(key).map_or(Ok(T::default()), read)
}

/// An `Option` field: absent or `null` reads as `None`.
fn optional<T>(
    object: &Map<String, Value>,
    key: &str,
    read: impl Fn(&Value) -> Result<T, String>,
) -> Result<Option<T>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => read(value).map(Some),
    }
}

fn definition_file(text: &str) -> Result<Vec<ItemDefinition>, String> {
    let document: Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
    let file = fields(&document, &DEFINITION_FILE_KEYS, "the file")?;
    optional(file, "note", |value| string(value, "note"))?;
    list(
        required(file, "definitions", "the file")?,
        "definitions",
        |value, _| definition(value),
    )
}

fn definition(value: &Value) -> Result<ItemDefinition, String> {
    let object = fields(value, &DEFINITION_KEYS, "a definition")?;
    let what = |key: &str| format!("a definition's `{key}`");
    let get = |key: &str| required(object, key, "a definition");
    Ok(ItemDefinition {
        id: string(get("id")?, &what("id"))?,
        live: or_default(object, "live", |value| boolean(value, &what("live")))?,
        doc: string(get("doc")?, &what("doc"))?,
        file_id: whole(get("file_id")?, &what("file_id"))?,
        model_id: whole(get("model_id")?, &what("model_id"))?,
        item_type: whole(get("item_type")?, &what("item_type"))?,
        name: list(get("name")?, &what("name"), whole)?,
        description: string(get("description")?, &what("description"))?,
        value: whole(get("value")?, &what("value"))?,
        materials: whole(get("materials")?, &what("materials"))?,
        unk1: whole(get("unk1")?, &what("unk1"))?,
        flags: whole(get("flags")?, &what("flags"))?,
        profession: whole(get("profession")?, &what("profession"))?,
        modifiers: list(get("modifiers")?, &what("modifiers"), whole)?,
        stackable: boolean(get("stackable")?, &what("stackable"))?,
        max_stack: optional(object, "max_stack", |value| {
            whole(value, &what("max_stack"))
        })?,
        equip_positions: list(get("equip_positions")?, &what("equip_positions"), whole)?,
        restrictions: list(get("restrictions")?, &what("restrictions"), string)?,
        base: optional(object, "base", |value| string(value, &what("base")))?,
        upgrades: or_default(object, "upgrades", |value| {
            list(value, &what("upgrades"), whole)
        })?,
    })
}

fn upgrade_file(text: &str) -> Result<Vec<Upgrade>, String> {
    let document: Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
    let file = fields(&document, &UPGRADE_FILE_KEYS, "the file")?;
    optional(file, "note", |value| string(value, "note"))?;
    list(
        required(file, "upgrades", "the file")?,
        "upgrades",
        |value, _| upgrade(value),
    )
}

fn upgrade(value: &Value) -> Result<Upgrade, String> {
    let object = fields(value, &UPGRADE_KEYS, "an upgrade")?;
    let what = |key: &str| format!("an upgrade's `{key}`");
    let get = |key: &str| required(object, key, "an upgrade");
    Ok(Upgrade {
        id: whole(get("id")?, &what("id"))?,
        role: string(get("role")?, &what("role"))?,
        name: list(get("name")?, &what("name"), whole)?,
        shape: list(get("shape")?, &what("shape"), whole)?,
        seen_on: or_default(object, "seen_on", |value| {
            list(value, &what("seen_on"), string)
        })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed pack, read the way the world reads it.
    const VANILLA: &str = include_str!("../../../config/World/Vanilla/items.json");
    const VANILLA_UPGRADES: &str = include_str!("../../../config/World/Vanilla/upgrades.json");
    /// The creation screen's own items: this tree's counterpart of ASURA's
    /// `asura_db::inventory::seed::ITEMS`, the reference's default set the seed rows were copied from.
    const CREATION_INVENTORY: &str =
        include_str!("../../../config/World/Vanilla/creation/inventory.json");

    // ASURA/crates/asura-world/src/item_catalogue.rs:426
    fn vanilla() -> Catalogue {
        Catalogue::from_json(VANILLA).expect("the committed catalogue parses")
    }

    /// The creation items, as `(id, file_id, item_type, model_id, profession)`.
    fn creation_items() -> Vec<(u64, i64, i32, i64, i32)> {
        let document: Value = serde_json::from_str(CREATION_INVENTORY).expect("inventory.json");
        document["items"]
            .as_array()
            .expect("items")
            .iter()
            .map(|item| {
                let number = |key: &str| item[key].as_i64().expect(key);
                (
                    item["id"].as_u64().expect("id"),
                    number("file_id"),
                    number("item_type") as i32,
                    number("model_id"),
                    number("profession") as i32,
                )
            })
            .collect()
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:431
    fn the_committed_catalogue_loads() {
        let catalogue = vanilla();
        assert!(
            catalogue.definitions().len() >= 58,
            "the committed catalogue must keep at least the 58 reference-seed and curated definitions"
        );
        let backpack = catalogue.definition("backpack").expect("backpack");
        assert_eq!(backpack.item_type, 3);
        assert_eq!(backpack.model_id, 32);
        assert_eq!(backpack.name, vec![0x21A8, 0xD157, 0xB58F, 0x166F]);
        assert_eq!(backpack.modifiers, vec![0x2448_1400]);
        assert_eq!(backpack.flags, 0x2000_1000);
        assert_eq!(backpack.stack_limit(), Some(1));
    }

    /// ASURA checks each seed row's named definition against the row's wire fields. The creation items
    /// here name no definition (no `definition_id` column yet), so each is resolved by its wire fields
    /// and must find one, with the same profession.
    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:447
    fn every_reference_seed_item_names_a_definition() {
        let catalogue = vanilla();
        for (id, file_id, item_type, model_id, profession) in creation_items() {
            let definition = catalogue
                .for_item(None, file_id, item_type, model_id)
                .unwrap_or_else(|| {
                    panic!(
                        "creation item {id} ({file_id}, {item_type}, {model_id}) has no definition"
                    )
                });
            assert_eq!(
                i64::from(definition.file_id),
                file_id,
                "{} file id",
                definition.id
            );
            assert_eq!(
                i32::from(definition.item_type),
                item_type,
                "{} item type",
                definition.id
            );
            assert_eq!(
                i64::from(definition.model_id),
                model_id,
                "{} model id",
                definition.id
            );
            assert_eq!(
                i32::from(definition.profession),
                profession,
                "{} profession",
                definition.id
            );
        }
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:487
    fn a_row_with_an_id_resolves_even_when_two_definitions_share_wire_fields() {
        let catalogue = vanilla();
        let items = creation_items();
        let wire = |id: u64| {
            let (_, file_id, item_type, model_id, _) = *items
                .iter()
                .find(|item| item.0 == id)
                .expect("creation item");
            catalogue
                .for_item(None, file_id, item_type, model_id)
                .map(|definition| definition.id.as_str())
        };
        assert_eq!(wire(1), Some("backpack"));
        assert_eq!(
            catalogue
                .for_item(Some("backpack"), 0, 0, 0)
                .map(|definition| definition.id.as_str()),
            Some("backpack"),
            "an explicit id wins over the wire fields"
        );

        // The two cupcake rows are one definition: identity is the definition, not the item id.
        assert_eq!(wire(2), wire(9));
        assert_eq!(wire(2), Some("birthday_cupcake"));
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:505
    fn stackability_and_equipment_positions_are_definition_data() {
        let catalogue = vanilla();
        let cupcake = catalogue
            .definition("birthday_cupcake")
            .expect("Birthday Cupcake");
        assert!(cupcake.stackable);
        assert_eq!(
            cupcake.stack_limit(),
            Some(250),
            "Guild Wars stacks hold up to 250"
        );
        assert!(cupcake.accepts_quantity(100));
        assert!(!cupcake.accepts_quantity(251));
        assert!(!cupcake.accepts_quantity(0));

        let hammer = catalogue
            .definition("warrior_starter_hammer")
            .expect("hammer");
        assert!(!hammer.stackable);
        assert_eq!(hammer.stack_limit(), Some(1));
        assert!(hammer.can_equip_in(0));
        assert!(!hammer.can_equip_in(1), "a hammer is not an off hand");
        assert!(
            !catalogue
                .definition("backpack")
                .expect("backpack")
                .can_equip_in(0),
            "a bag's own item is not worn"
        );
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:537
    fn a_duplicate_id_or_an_unknown_key_is_refused() {
        let duplicate = r#"{"definitions":[
            {"id":"a","doc":"d","file_id":1,"model_id":1,"item_type":1,"name":[],
             "description":"d","value":0,"materials":0,"unk1":0,"flags":0,"profession":0,
             "modifiers":[],"stackable":false,"max_stack":1,"equip_positions":[],
             "restrictions":[]},
            {"id":"a","doc":"d","file_id":2,"model_id":2,"item_type":1,"name":[],
             "description":"d","value":0,"materials":0,"unk1":0,"flags":0,"profession":0,
             "modifiers":[],"stackable":false,"max_stack":1,"equip_positions":[],
             "restrictions":[]}
        ]}"#;
        let err = Catalogue::from_json(duplicate).expect_err("duplicate refused");
        assert!(err.contains("duplicate definition id"), "said: {err}");

        let unknown = r#"{"definitions":[{"id":"a","doc":"d","file_id":1,"model_id":1,
            "item_type":1,"name":[],"description":"d","value":0,"materials":0,"unk1":0,
            "flags":0,"profession":0,"modifiers":[],"stackable":false,"max_stack":1,
            "equip_positions":[],"restrictions":[],"invented":true}]}"#;
        assert!(Catalogue::from_json(unknown).is_err());
    }

    /// One definition with the given name, words and listed upgrades.
    // ASURA/crates/asura-world/src/item_catalogue.rs:559
    fn one(name: &str, words: &str, upgrades: &str) -> String {
        format!(
            r#"{{"definitions":[{{"id":"a","doc":"d","file_id":1,"model_id":1,"item_type":7,
            "name":{name},"description":"d","value":0,"materials":0,"unk1":0,"flags":0,
            "profession":0,"modifiers":{words},"stackable":false,"max_stack":1,
            "equip_positions":[2],"restrictions":[],"upgrades":{upgrades}}}]}}"#
        )
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:569
    fn the_committed_upgrades_load_and_every_listed_upgrade_is_defined() {
        let catalogue = vanilla()
            .with_upgrades_json(VANILLA_UPGRADES)
            .expect("the committed upgrades load");
        assert!(catalogue.upgrades().count() > 100);
        // Vitae (0x212) is a rune: a suffix, Health +10.
        let vitae = catalogue.upgrade(0x212).expect("Vitae");
        assert_eq!(vitae.role, "suffix");
        assert_eq!(vitae.shape, vec![0x2408, 0x2530, 0x2348]);
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:581
    fn an_upgraded_item_is_its_base_name_plus_its_upgrades() {
        let catalogue = vanilla()
            .with_upgrades_json(VANILLA_UPGRADES)
            .expect("the committed upgrades load");
        // Knight's Templar Helm of Vitae: the base name alone, and the two upgrades it carries.
        let helm = catalogue.definition("live_49_23244_16").expect("the helm");
        assert_eq!(helm.name, vec![0x8102, 0x0A19]);
        assert_eq!(helm.upgrades, vec![0x212, 0x1F9]);
        assert_eq!(helm.base.as_deref(), Some("t16_8102_a19"));
        for definition in catalogue.definitions() {
            if !definition.upgrades.is_empty() {
                assert!(
                    !COMPOSED_NAME_WORDS.contains(&definition.name[0]),
                    "{} carries a composed name",
                    definition.id
                );
            }
        }
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:602
    fn a_composed_name_on_an_upgraded_item_is_refused() {
        // The doubled-name bug, as data: 0xa3f(0x8102 0x95b) with Knight's fitted.
        let composed = one(
            "[2623,266,33026,2395,1,0]",
            "[604504569,2747813888]",
            "[505]",
        );
        let err = Catalogue::from_json(&composed).expect_err("refused");
        assert!(err.contains("composed display name"), "said: {err}");
        let bare = one("[33026,2395]", "[604504569,2747813888]", "[505]");
        assert!(Catalogue::from_json(&bare).is_ok());
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:616
    fn listed_upgrades_must_match_the_words_and_be_defined() {
        // Words carry Knight's (0x1f9); the list says nothing.
        let unlisted = one("[33026,2395]", "[604504569]", "[]");
        assert!(
            Catalogue::from_json(&unlisted)
                .expect_err("refused")
                .contains("lists upgrades")
        );
        // Listed and carried, but the upgrades file does not define it.
        let undefined = one("[33026,2395]", "[604504569]", "[505]");
        let err = Catalogue::from_json(&undefined)
            .expect("the items parse")
            .with_upgrades_json(r#"{"upgrades":[]}"#)
            .expect_err("refused");
        assert!(err.contains("does not define"), "said: {err}");
    }

    #[test]
    // ASURA/crates/asura-world/src/item_catalogue.rs:632
    fn a_missing_file_is_an_error_not_an_empty_catalogue() {
        let err = load(Path::new("no/such/pack")).expect_err("refused");
        assert!(err.contains("items.json"), "said: {err}");
    }

    /// This tree's reader, not ASURA's: the fields serde would default read as their defaults, and a
    /// field serde requires is required.
    #[test]
    fn the_reader_keeps_serdes_defaults_and_requirements() {
        let minimal = r#"{"definitions":[{"id":"a","doc":"d","file_id":1,"model_id":1,
            "item_type":1,"name":[],"description":"d","value":0,"materials":0,"unk1":0,
            "flags":0,"profession":0,"modifiers":[],"stackable":false,
            "equip_positions":[],"restrictions":[]}]}"#;
        let catalogue = Catalogue::from_json(minimal).expect("absent optional fields are defaults");
        let definition = catalogue.definition("a").expect("a");
        assert!(!definition.live);
        assert_eq!(definition.max_stack, None);
        assert_eq!(definition.base, None);
        assert!(definition.upgrades.is_empty());
        let missing = minimal.replace(r#""doc":"d","#, "");
        assert!(Catalogue::from_json(&missing).is_err(), "doc is required");
        let wide = minimal.replace(r#""item_type":1"#, r#""item_type":256"#);
        assert!(Catalogue::from_json(&wide).is_err(), "item_type is a byte");
        assert!(
            Catalogue::from_json(r#"{"definitions":[],"extra":1}"#).is_err(),
            "the top level is closed too"
        );
    }
}
