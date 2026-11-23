//! The pack's `vendors.json`, read into `game/Trader/Content.rs`'s [`Vendors`].
//!
//! The file is ASURA's own (`ASURA/config/World/Vanilla/vendors.json`, written by its
//! `tools/vendors-from-preforged.py`), copied unchanged. ASURA reads it with serde derives and
//! `deny_unknown_fields` (`ASURA/crates/asura-world/src/vendor/content.rs:23-193`, `:380-390`); this
//! reads it with this tree's JSON, keeping the refusal of an unknown key, as `Maps/NpcPack.rs` does.
//! Every check is [`Vendors::check`]'s.
//!
//! One difference of arrangement, the owner's decision (2026-11-23): JSON is today's source of this
//! content, and a Lua script is the next one. The reader is therefore the only thing that knows the
//! file; the handlers see [`Vendors`] alone.
//!
//! It lives here, beside `Pack.rs`, because it reads the disk at start-up, and
//! `worldserver/tests/content_is_held.rs` refuses a file read inside a connection-serving module.

use std::path::Path;

use serde_json::{Map, Value};

use crate::trader::content::{
    Collector, FILE, ItemRow, Merchant, Offer, Rules, Storage, Vendors,
};

/// The names every file of this tree may carry as commentary (`common/src/Configuration/Json.rs`);
/// ASURA's file adds `fields`, its own table of what each section means.
const COMMENTARY: [&str; 5] = ["doc", "note", "immutable", "information", "fields"];

/// One object with the keys it may hold: anything else refuses it (`deny_unknown_fields`).
fn object<'a>(
    value: &'a Value,
    at: &str,
    known: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    let Some(map) = value.as_object() else {
        return Err(format!("{at}: is not an object"));
    };
    for key in map.keys() {
        if !known.contains(&key.as_str()) && !COMMENTARY.contains(&key.as_str()) {
            return Err(format!("{at}: unknown field {key:?}"));
        }
    }
    Ok(map)
}

fn field<'a>(map: &'a Map<String, Value>, at: &str, key: &str) -> Result<&'a Value, String> {
    match map.get(key) {
        None | Some(Value::Null) => Err(format!("{at}: missing field {key:?}")),
        Some(value) => Ok(value),
    }
}

fn unsigned(map: &Map<String, Value>, at: &str, key: &str, max: u64) -> Result<u64, String> {
    field(map, at, key)?
        .as_u64()
        .filter(|number| *number <= max)
        .ok_or_else(|| format!("{at}: field {key:?} is not a number from 0 to {max}"))
}

fn signed32(map: &Map<String, Value>, at: &str, key: &str) -> Result<i32, String> {
    field(map, at, key)?
        .as_i64()
        .and_then(|number| i32::try_from(number).ok())
        .ok_or_else(|| format!("{at}: field {key:?} is not an i32"))
}

fn text(map: &Map<String, Value>, at: &str, key: &str) -> Result<String, String> {
    field(map, at, key)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{at}: field {key:?} is not text"))
}

fn numbers(map: &Map<String, Value>, at: &str, key: &str, max: u64) -> Result<Vec<u64>, String> {
    field(map, at, key)?
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_u64().filter(|number| *number <= max))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| format!("{at}: field {key:?} is not a list of numbers from 0 to {max}"))
}

fn rows<'a>(map: &'a Map<String, Value>, at: &str, key: &str) -> Result<&'a [Value], String> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(format!("{at}: field {key:?} is not a list")),
    }
}

/// A rule: `{ "value": n, "doc": ..., "immutable": ... }` (ASURA's `Value<T>`).
fn rule(map: &Map<String, Value>, key: &str) -> Result<f64, String> {
    let at = format!("rules.{key}");
    let inner = object(field(map, "rules", key)?, &at, &["value"])?;
    field(inner, &at, "value")?
        .as_f64()
        .ok_or_else(|| format!("{at}: field \"value\" is not a number"))
}

fn rule_u32(map: &Map<String, Value>, key: &str) -> Result<u32, String> {
    let number = rule(map, key)?;
    if number < 0.0 || number > f64::from(u32::MAX) || number.fract() != 0.0 {
        return Err(format!("rules.{key}: \"value\" is not a whole number of a u32"));
    }
    Ok(number as u32)
}

/// `content.rs:23-54`.
fn item(value: &Value, at: &str) -> Result<ItemRow, String> {
    let map = object(
        value,
        at,
        &[
            "id",
            "file_id",
            "item_type",
            "dye_tint",
            "dye_colors",
            "materials",
            "unk1",
            "flags",
            "value",
            "model_id",
            "name",
            "modifiers",
            "definition",
        ],
    )?;
    let u8_max = u64::from(u8::MAX);
    let u16_max = u64::from(u16::MAX);
    let u32_max = u64::from(u32::MAX);
    Ok(ItemRow {
        id: text(map, at, "id")?,
        file_id: unsigned(map, at, "file_id", u32_max)? as u32,
        item_type: unsigned(map, at, "item_type", u8_max)? as u8,
        dye_tint: unsigned(map, at, "dye_tint", u8_max)? as u8,
        dye_colors: unsigned(map, at, "dye_colors", u16_max)? as u16,
        materials: unsigned(map, at, "materials", u16_max)? as u16,
        unk1: unsigned(map, at, "unk1", u8_max)? as u8,
        flags: unsigned(map, at, "flags", u32_max)? as u32,
        value: unsigned(map, at, "value", u32_max)? as u32,
        model_id: unsigned(map, at, "model_id", u32_max)? as u32,
        name: numbers(map, at, "name", u16_max)?
            .into_iter()
            .map(|unit| unit as u16)
            .collect(),
        modifiers: numbers(map, at, "modifiers", u32_max)?
            .into_iter()
            .map(|word| word as u32)
            .collect(),
        definition: match map.get("definition") {
            None | Some(Value::Null) => None,
            Some(Value::String(key)) => Some(key.clone()),
            Some(_) => return Err(format!("{at}: field \"definition\" is not text")),
        },
    })
}

/// Parse and check a vendors document (`content.rs:216-222`, `from_json`).
pub fn from_json(text_in: &str) -> Result<Vendors, String> {
    let document: Value = serde_json::from_str(text_in).map_err(|err| err.to_string())?;
    let top = object(
        &document,
        "vendors",
        &[
            "rules",
            "unsellable_models",
            "items",
            "merchants",
            "collectors",
            "storage",
        ],
    )?;
    let rules_map = object(
        field(top, "vendors", "rules")?,
        "rules",
        &[
            "service_range",
            "buy_scale",
            "carried_gold_max",
            "stored_gold_max",
            "stack_max",
        ],
    )?;
    let rules = Rules {
        service_range: rule(rules_map, "service_range")? as f32,
        buy_scale: rule(rules_map, "buy_scale")? as f32,
        carried_gold_max: rule_u32(rules_map, "carried_gold_max")?,
        stored_gold_max: rule_u32(rules_map, "stored_gold_max")?,
        stack_max: rule_u32(rules_map, "stack_max")?,
    };
    let unsellable_models = match top.get("unsellable_models") {
        None | Some(Value::Null) => Vec::new(),
        Some(_) => numbers(top, "vendors", "unsellable_models", u64::from(u32::MAX))?
            .into_iter()
            .map(|model| model as u32)
            .collect(),
    };
    let mut items = Vec::new();
    for (index, value) in rows(top, "vendors", "items")?.iter().enumerate() {
        items.push(item(value, &format!("items[{index}]"))?);
    }
    let mut merchants = Vec::new();
    for (index, value) in rows(top, "vendors", "merchants")?.iter().enumerate() {
        let at = format!("merchants[{index}]");
        let map = object(value, &at, &["map", "npc", "stock"])?;
        let stock = field(map, &at, "stock")?
            .as_array()
            .and_then(|keys| {
                keys.iter()
                    .map(|key| key.as_str().map(str::to_string))
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| format!("{at}: field \"stock\" is not a list of item keys"))?;
        merchants.push(Merchant {
            map: signed32(map, &at, "map")?,
            npc: text(map, &at, "npc")?,
            stock,
        });
    }
    let mut collectors = Vec::new();
    for (index, value) in rows(top, "vendors", "collectors")?.iter().enumerate() {
        let at = format!("collectors[{index}]");
        let map = object(value, &at, &["map", "npc", "trophy", "cost", "heading", "offers"])?;
        let mut offers = Vec::new();
        for (offer_index, offer) in rows(map, &at, "offers")?.iter().enumerate() {
            let offer_at = format!("{at}.offers[{offer_index}]");
            let offer_map = object(offer, &offer_at, &["reward", "profession"])?;
            offers.push(Offer {
                reward: text(offer_map, &offer_at, "reward")?,
                profession: unsigned(offer_map, &offer_at, "profession", u64::from(u8::MAX))?
                    as u8,
            });
        }
        collectors.push(Collector {
            map: signed32(map, &at, "map")?,
            npc: text(map, &at, "npc")?,
            trophy: text(map, &at, "trophy")?,
            cost: unsigned(map, &at, "cost", u64::from(u32::MAX))? as u32,
            heading: numbers(map, &at, "heading", u64::from(u16::MAX))?
                .into_iter()
                .map(|unit| unit as u16)
                .collect(),
            offers,
        });
    }
    let mut storage = Vec::new();
    for (index, value) in rows(top, "vendors", "storage")?.iter().enumerate() {
        let at = format!("storage[{index}]");
        let map = object(value, &at, &["map", "npc"])?;
        storage.push(Storage {
            map: match map.get("map") {
                None | Some(Value::Null) => None,
                Some(_) => Some(signed32(map, &at, "map")?),
            },
            npc: text(map, &at, "npc")?,
        });
    }
    let vendors = Vendors {
        rules,
        unsellable_models,
        items,
        merchants,
        collectors,
        storage,
    };
    vendors.check()?;
    Ok(vendors)
}

/// Read `<pack>/vendors.json` (`content.rs:380-390`). A pack without the file has no windows, as in
/// ASURA; a file that is there and does not read refuses the start (`AGENTS.md`, rule 7), where ASURA
/// logs it once and opens no window.
pub fn load(pack: &Path) -> Result<Vendors, String> {
    let path = pack.join(FILE);
    let text_in = match std::fs::read_to_string(&path) {
        Ok(text_in) => text_in,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vendors::default()),
        Err(err) => return Err(format!("{} is not readable: {err}", path.display())),
    };
    from_json(&text_in).map_err(|err| format!("{} is not a vendors file: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> Vendors {
        load(Path::new("../config/World/Vanilla")).expect("the pack's vendors file loads")
    }

    // `content.rs:427-444`, on this tree's copy of the file (7 merchants and 9 collectors today; the
    // ASURA test's 6 and 8 predate the file's last rows).
    #[test]
    fn the_vanilla_pack_holds_together() {
        let vendors = pack();
        assert_eq!(vendors.merchants.len(), 7);
        assert_eq!(vendors.collectors.len(), 9);
        let sanura = vendors
            .merchants
            .iter()
            .find(|merchant| merchant.npc == "Sanura")
            .expect("Sanura");
        assert_eq!(sanura.map, 148);
        assert_eq!(sanura.stock.len(), 7);
        assert!(
            vendors
                .items
                .iter()
                .all(|item| item.file_id & 0x8000_0000 != 0)
        );
    }

    #[test]
    fn an_unknown_key_is_refused() {
        let text_in = std::fs::read_to_string("../config/World/Vanilla/vendors.json").expect("the file");
        let mut document: Value = serde_json::from_str(&text_in).expect("json");
        document["merchants"][0]["colour"] = Value::from("red");
        let err = from_json(&document.to_string()).expect_err("refused");
        assert!(err.contains("colour"), "said: {err}");
    }
}
