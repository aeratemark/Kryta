//! What each NPC offers: which window it opens, the full item row of everything a window lists, and
//! the limits a trade runs on.
//!
//! Ported 1:1 from `ASURA/crates/asura-world/src/vendor/content.rs` (478 lines, read from the local
//! ASURA tree on 2026-11-23): `ItemRow` (`:23-99`), `Merchant`, `Offer`, `Collector`, `Storage`,
//! `Rules`, `Vendors` (`:101-193`), `Service` (`:195-204`), `base_name` (`:206-214`), `check`
//! (`:259-336`), `item`, `service` and `limits` (`:338-378`), with their tests.
//!
//! **The source is the owner's decision (2026-11-23), not ASURA's.** ASURA reads these types from the
//! pack's `vendors.json`. Here they are the contract a pack's content fills: today the same
//! `vendors.json`, read by `game/Maps/VendorPack.rs`; when the Lua runtime lands with ASURA's
//! `npc/lua.rs`, a script that names an NPC's offer fills the same [`Vendors`]. What the client sees
//! does not change with the source: every frame is the handler's (`game/Handlers/NpcHandler.rs`),
//! ported from ASURA's `vendor/mod.rs`.

use crate::characters::HeldItem;
use crate::server::packets::item_packets::NamedItem;

/// The file inside a pack (`content.rs:16-17`).
pub const FILE: &str = "vendors.json";

/// The most ids one `0x0084` lists; a collector spends one on its cost row (`content.rs:19-21`).
pub const WINDOW_ROWS: usize = 16;

/// One item row: every field `CREATE_NAMED_ITEM` carries but the id and the quantity
/// (`content.rs:23-54`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    /// The key merchants and collectors name it by.
    pub id: String,
    /// The `Gw.dat` file id, bit 31 included, as the live server sends it.
    pub file_id: u32,
    /// `item_type`.
    pub item_type: u8,
    /// `dye_tint`.
    pub dye_tint: u8,
    /// `dye_colors`.
    pub dye_colors: u16,
    /// `materials`.
    pub materials: u16,
    /// `unk1`.
    pub unk1: u8,
    /// `flags`.
    pub flags: u32,
    /// What a merchant pays for one, and half of what one costs.
    pub value: u32,
    /// `model_id`.
    pub model_id: u32,
    /// The coded name.
    pub name: Vec<u16>,
    /// The modifier dwords.
    pub modifiers: Vec<u32>,
    /// The `items.json` definition this row is, written into an owned copy's `definition_id`.
    pub definition: Option<String>,
}

impl ItemRow {
    /// The row as one `CREATE_NAMED_ITEM`, under `id` and `quantity` (`content.rs:57-74`).
    pub fn named(&self, id: u32, quantity: u32) -> NamedItem {
        NamedItem {
            id,
            file_id: self.file_id,
            item_type: self.item_type,
            dye_tint: self.dye_tint,
            dye_colors: self.dye_colors,
            materials: self.materials,
            unk1: self.unk1,
            flags: self.flags,
            value: self.value,
            model_id: self.model_id,
            quantity,
            name: self.name.clone(),
            modifiers: self.modifiers.clone(),
        }
    }

    /// The row as an owned item: the row a purchase or an exchange writes, not yet placed
    /// (`content.rs:76-98`; this tree's row is `HeldItem`, whose unplaced form is bag 0).
    pub fn owned(&self, item_id: u32, quantity: u32, profession: u8) -> HeldItem {
        HeldItem {
            template_id: 0,
            file_id: self.file_id,
            item_type: self.item_type,
            dye_tint: self.dye_tint,
            dye_colors: self.dye_colors,
            materials: self.materials,
            unk1: self.unk1,
            flags: self.flags,
            value: self.value,
            model_id: self.model_id,
            item_id,
            bag_id: 0,
            slot: 0,
            quantity,
            profession,
            name: self.name.clone(),
            modifiers: self.modifiers.clone(),
            definition_id: self.definition.clone().unwrap_or_default(),
        }
    }
}

/// A merchant: where, who, and what it sells (`content.rs:101-111`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merchant {
    /// Its map.
    pub map: i32,
    /// Its name, without a bracketed title.
    pub npc: String,
    /// The item keys it sells, in window order.
    pub stock: Vec<String>,
}

/// One collector offer (`content.rs:113-121`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The reward's item key.
    pub reward: String,
    /// The primary profession it is for; 0 for any.
    pub profession: u8,
}

/// A collector: where, who, its trophy and its offers (`content.rs:123-139`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collector {
    /// Its map.
    pub map: i32,
    /// Its name, without a bracketed title.
    pub npc: String,
    /// The trophy's item key.
    pub trophy: String,
    /// How many trophies one offer costs.
    pub cost: u32,
    /// The coded line it says over its offers (`0x00C5`).
    pub heading: Vec<u16>,
    /// Its offers, in window order.
    pub offers: Vec<Offer>,
}

/// An NPC that opens Xunlai storage (`content.rs:141-149`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Storage {
    /// Its map; `None` for every map.
    pub map: Option<i32>,
    /// Its name, without a bracketed title.
    pub npc: String,
}

/// The limits a trade runs on (`content.rs:151-165`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    /// How far a player may stand from the NPC whose window is open.
    pub service_range: f32,
    /// A merchant's price scale.
    pub buy_scale: f32,
    /// The most gold a character carries.
    pub carried_gold_max: u32,
    /// The most gold storage holds.
    pub stored_gold_max: u32,
    /// The largest stack a purchase may make.
    pub stack_max: u32,
}

/// Everything the pack's NPCs offer (`content.rs:167-193`).
#[derive(Debug, Clone, PartialEq)]
pub struct Vendors {
    /// The limits.
    pub rules: Rules,
    /// Models a merchant does not buy.
    pub unsellable_models: Vec<u32>,
    /// Every item row.
    pub items: Vec<ItemRow>,
    /// The merchants.
    pub merchants: Vec<Merchant>,
    /// The collectors.
    pub collectors: Vec<Collector>,
    /// The Xunlai storage NPCs.
    pub storage: Vec<Storage>,
}

/// What an NPC does when talked to, if the pack gives it a window (`content.rs:195-204`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service<'a> {
    /// A merchant's window.
    Merchant(&'a Merchant),
    /// A collector's window.
    Collector(&'a Collector),
    /// The Xunlai storage window.
    Storage,
}

/// A name as the pack compares it: without a bracketed title, and without surrounding spaces
/// (`content.rs:206-214`).
pub fn base_name(name: &str) -> &str {
    let trimmed = name.trim();
    match trimmed.rfind(" [") {
        Some(at) if trimmed.ends_with(']') => trimmed[..at].trim_end(),
        _ => trimmed,
    }
}

impl Default for Vendors {
    /// No windows at all: the answer for a pack without content (`content.rs:224-257`).
    fn default() -> Self {
        Vendors {
            rules: Rules {
                service_range: 0.0,
                buy_scale: 1.0,
                carried_gold_max: 0,
                stored_gold_max: 0,
                stack_max: 1,
            },
            unsellable_models: Vec::new(),
            items: Vec::new(),
            merchants: Vec::new(),
            collectors: Vec::new(),
            storage: Vec::new(),
        }
    }
}

impl Vendors {
    /// Refuse content whose rows do not hold together, naming the row (`content.rs:259-336`).
    pub fn check(&self) -> Result<(), String> {
        let mut keys = std::collections::BTreeSet::new();
        for item in &self.items {
            if !keys.insert(item.id.as_str()) {
                return Err(format!("item {} is defined twice", item.id));
            }
            if item.file_id == 0 {
                return Err(format!(
                    "item {} has file id 0, which the client asserts on",
                    item.id
                ));
            }
            if item.name.len() > 64 || item.modifiers.len() > 64 {
                return Err(format!(
                    "item {} has more name units or modifiers than 0x0161 carries",
                    item.id
                ));
            }
        }
        let known = |key: &str, whose: &str| {
            if keys.contains(key) {
                Ok(())
            } else {
                Err(format!(
                    "{whose} names item {key}, which the file does not define"
                ))
            }
        };
        for merchant in &self.merchants {
            if merchant.stock.is_empty() || WINDOW_ROWS < merchant.stock.len() {
                return Err(format!(
                    "merchant {} on map {} sells {} items; a window lists 1 to {WINDOW_ROWS}",
                    merchant.npc,
                    merchant.map,
                    merchant.stock.len()
                ));
            }
            for key in &merchant.stock {
                known(key, &format!("merchant {}", merchant.npc))?;
            }
        }
        for collector in &self.collectors {
            if collector.offers.is_empty() || WINDOW_ROWS <= collector.offers.len() {
                return Err(format!(
                    "collector {} on map {} has {} offers; a window lists 1 to {}",
                    collector.npc,
                    collector.map,
                    collector.offers.len(),
                    WINDOW_ROWS - 1
                ));
            }
            if collector.cost == 0 || collector.cost > self.rules.stack_max {
                return Err(format!(
                    "collector {} costs {} trophies",
                    collector.npc, collector.cost
                ));
            }
            if collector.heading.len() > 122 {
                return Err(format!(
                    "collector {}'s heading is longer than 0x00C5 carries",
                    collector.npc
                ));
            }
            known(&collector.trophy, &format!("collector {}", collector.npc))?;
            for offer in &collector.offers {
                known(&offer.reward, &format!("collector {}", collector.npc))?;
            }
        }
        let range = self.rules.service_range;
        if range.is_nan() || range <= 0.0 {
            return Err("rules.service_range must be above 0".to_string());
        }
        let scale = self.rules.buy_scale;
        if !scale.is_finite() || scale <= 0.0 {
            return Err("rules.buy_scale must be a finite number above 0".to_string());
        }
        Ok(())
    }

    /// An item row by key (`content.rs:338-341`).
    pub fn item(&self, key: &str) -> Option<&ItemRow> {
        self.items.iter().find(|item| item.id == key)
    }

    /// What the NPC called `name` on `map_id` does, if anything: a merchant or a collector of this map
    /// first, then a storage NPC, which may be named for every map (`content.rs:343-367`).
    pub fn service(&self, map_id: i32, name: &str) -> Option<Service<'_>> {
        let name = base_name(name);
        if let Some(merchant) = self
            .merchants
            .iter()
            .find(|merchant| merchant.map == map_id && base_name(&merchant.npc) == name)
        {
            return Some(Service::Merchant(merchant));
        }
        if let Some(collector) = self
            .collectors
            .iter()
            .find(|collector| collector.map == map_id && base_name(&collector.npc) == name)
        {
            return Some(Service::Collector(collector));
        }
        self.storage
            .iter()
            .any(|storage| {
                storage.map.is_none_or(|map| map == map_id) && base_name(&storage.npc) == name
            })
            .then_some(Service::Storage)
    }

    /// The limits the pure rules take (`content.rs:369-377`).
    pub fn limits(&self) -> super::rules::Limits {
        super::rules::Limits {
            buy_scale: self.rules.buy_scale,
            carried_max: self.rules.carried_gold_max,
            stored_max: self.rules.stored_gold_max,
            stack_max: self.rules.stack_max,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `content.rs:446-467`, without the pack: the lookup rules on a built value.
    fn sample() -> Vendors {
        let row = |id: &str| ItemRow {
            id: id.to_string(),
            file_id: 0x8000_0001,
            item_type: 30,
            dye_tint: 0,
            dye_colors: 0,
            materials: 0,
            unk1: 0,
            flags: 0,
            value: 2,
            model_id: 429,
            name: vec![1],
            modifiers: Vec::new(),
            definition: None,
        };
        Vendors {
            rules: Rules {
                service_range: 300.0,
                buy_scale: 1.0,
                carried_gold_max: 100_000,
                stored_gold_max: 1_000_000,
                stack_max: 250,
            },
            unsellable_models: vec![1101],
            items: vec![row("flute"), row("skale_fin"), row("belt_pouch_33")],
            merchants: vec![Merchant {
                map: 148,
                npc: "Sanura".to_string(),
                stock: vec!["flute".to_string()],
            }],
            collectors: vec![Collector {
                map: 146,
                npc: "Brownlow".to_string(),
                trophy: "skale_fin".to_string(),
                cost: 5,
                heading: vec![1],
                offers: vec![Offer {
                    reward: "belt_pouch_33".to_string(),
                    profession: 0,
                }],
            }],
            storage: vec![Storage {
                map: None,
                npc: "Xunlai Agent".to_string(),
            }],
        }
    }

    #[test]
    fn a_name_is_found_with_or_without_its_title() {
        let vendors = sample();
        assert!(vendors.check().is_ok());
        assert!(matches!(
            vendors.service(148, "Sanura [Merchant]"),
            Some(Service::Merchant(_))
        ));
        assert!(matches!(
            vendors.service(148, "Sanura"),
            Some(Service::Merchant(_))
        ));
        assert!(vendors.service(163, "Sanura").is_none());
        assert!(matches!(
            vendors.service(146, "Brownlow [Collector]"),
            Some(Service::Collector(_))
        ));
        assert!(matches!(
            vendors.service(500, "Xunlai Agent [Storage]"),
            Some(Service::Storage)
        ));
        assert_eq!(base_name("  Gram  "), "Gram");
    }

    // `content.rs:469-476`.
    #[test]
    fn a_merchant_selling_an_unknown_item_is_refused() {
        let mut vendors = sample();
        vendors.merchants[0].stock[0] = "no_such_item".to_string();
        let err = vendors.check().expect_err("refused");
        assert!(err.contains("no_such_item"), "said: {err}");
    }
}
