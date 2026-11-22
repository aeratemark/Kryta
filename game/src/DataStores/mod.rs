//! `game/DataStores/` - the reference's `src/server/game/DataStores/` (5 files).
//!
//! What is here now: the item templates - what an item is, once - seeded from the creation record
//! lists at start-up and read back as rows (`Items.rs`, `data/sql/base/db_world/0004-item-templates.sql`).
//! The reference's own `ItemTemplate` store is the same arrangement: one pass at
//! `SetInitialWorldSettings`, then rows in memory.
//!
//! The item catalogue - every static item definition and upgrade the pack states
//! (`config/World/Vanilla/items.json`, `upgrades.json`), read once at start-up - and the reading of an
//! item's modifier words are ASURA's `item_catalogue.rs` and `item_modifier.rs`, ported whole
//! (`ItemCatalogue.rs`, `ItemModifier.rs`; `docs/port/09-parity-audit.md`, section 7h). The templates
//! above are this tree's older arrangement, kept until the `item.rs` re-diff moves the rows onto the
//! catalogue.
//!
//! Not ported yet: the reference's other stores (creature templates, quests, the DBC data).

#[path = "Items.rs"]
pub mod items;

#[path = "ItemCatalogue.rs"]
pub mod item_catalogue;

#[path = "ItemModifier.rs"]
pub mod item_modifier;

#[path = "PvpItems.rs"]
pub mod pvp_items;

pub use items::{DatabaseItems, ItemDefinition, definitions_as_templates};
