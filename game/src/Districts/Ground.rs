//! Items lying on the ground in a running copy.
//!
//! Ported from `ASURA/crates/asura-world/src/instance.rs:117-151` (`GroundItem`) and `:428-510`
//! (`allocate_agent_id`, `put_ground_item`, `ground_item`, `take_ground_item`, `ground_items`), read
//! from the local ASURA tree on 2026-11-22.
//!
//! A drop is not a bag row. The live server takes the whole stack's row out of the bag and keeps its
//! id for the ground agent, or keeps a partial stack and gives the ground part a fresh id
//! (`items-live-20261108-173913.jsonl` seq 41-43, `items-live-20261108-174014.jsonl` seq 133-136).
//! Both forms are carried here: `source_item_id` is `Some` exactly for the partial form, the stack a
//! pickup merges back into.
//!
//! The copy owns them, not the bag: a client that leaves and comes back finds them where it left
//! them, and every client in the map is told. A disposed copy takes its ground items with it.

use std::collections::{BTreeMap, HashMap};

use crate::characters::HeldItem;
use crate::server::packets::{agent_packets, item_packets};

/// One item lying on the ground: the record the client draws, where it lies, and the agent it is
/// (`ASURA/crates/asura-world/src/instance.rs:117-135`).
#[derive(Debug, Clone, PartialEq)]
pub struct GroundItem {
    /// The agent the client interacts with and `WORLD_REMOVE_AGENT` takes away.
    pub agent_id: u32,
    /// The item record the ground agent's `model_id` names. Its `bag_id` and `slot` are 0: it is in
    /// no bag.
    pub item: HeldItem,
    /// The bag row the dropped part came from, when that row stayed behind. `None` for a whole drop.
    pub source_item_id: Option<u32>,
    /// Where it lies.
    pub position: (f32, f32),
    /// The pathing plane, not a height.
    pub plane: u16,
    /// Which way it faces; the live server's ground items face `(1, 0)`.
    pub direction: (f32, f32),
}

/// Every copy's ground items, by agent id and keyed by map and district.
#[derive(Debug, Default)]
pub struct GroundDistricts {
    copies: HashMap<(u32, u32), BTreeMap<u32, GroundItem>>,
}

impl GroundDistricts {
    /// No copies yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Put one dropped item on the ground under its agent id
    /// (`ASURA/crates/asura-world/src/instance.rs:490-493`, `put_ground_item`).
    pub fn put(&mut self, map_id: u32, district: u32, item: GroundItem) {
        self.copies
            .entry((map_id, district))
            .or_default()
            .insert(item.agent_id, item);
    }

    /// Take a ground item off the map, so a pickup can claim it before it awaits
    /// (`ASURA/crates/asura-world/src/instance.rs:500-503`, `take_ground_item`).
    pub fn take(&mut self, map_id: u32, district: u32, agent_id: u32) -> Option<GroundItem> {
        self.copies
            .get_mut(&(map_id, district))
            .and_then(|items| items.remove(&agent_id))
    }

    /// Every ground item in one copy, by agent id, for a client that is entering the map
    /// (`ASURA/crates/asura-world/src/instance.rs:505-510`, `ground_items`).
    pub fn items(&self, map_id: u32, district: u32) -> Vec<GroundItem> {
        self.copies
            .get(&(map_id, district))
            .map(|items| items.values().cloned().collect())
            .unwrap_or_default()
    }

    /// The copy is gone: its ground items go with it. ASURA's items belong to the instance, and a
    /// disposed instance takes them (`ASURA/crates/asura-world/src/instance.rs:149-151`).
    pub fn close(&mut self, map_id: u32, district: u32) {
        self.copies.remove(&(map_id, district));
    }
}

/// The frames one ground item is announced with to a client entering the map: the record the client
/// draws (create for a whole drop, update for the part of a stack), then its agent.
///
/// `ASURA/crates/asura-world/src/item.rs:224-234`, `ground_frames`.
pub fn ground_frames(ground: &GroundItem) -> Vec<Vec<u8>> {
    let named = crate::creation::named_item(&crate::characters::item_ops::item_of_row(&ground.item));
    vec![
        if ground.source_item_id.is_some() {
            item_packets::item_update(&named)
        } else {
            item_packets::create_named_item(&named)
        },
        agent_packets::item_agent(
            ground.agent_id,
            ground.item.item_id,
            ground.position,
            ground.plane,
            ground.direction,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(item_id: u32, quantity: u32) -> HeldItem {
        HeldItem {
            template_id: 0,
            file_id: 1,
            item_type: 21,
            dye_tint: 0,
            dye_colors: 0,
            materials: 0,
            unk1: 0,
            flags: 0,
            value: 0,
            model_id: 1,
            item_id,
            bag_id: 0,
            slot: 0,
            quantity,
            profession: 0,
            name: Vec::new(),
            modifiers: Vec::new(),
            definition_id: String::new(),
        }
    }

    fn ground(agent_id: u32, item_id: u32, source: Option<u32>, quantity: u32) -> GroundItem {
        GroundItem {
            agent_id,
            item: held(item_id, quantity),
            source_item_id: source,
            position: (10.0, 20.0),
            plane: 0,
            direction: (1.0, 0.0),
        }
    }

    /// The copy owns them, by agent id, and a disposed copy takes them with it
    /// (`ASURA/crates/asura-world/src/instance.rs:149-151`, `:490-510`).
    #[test]
    fn a_copy_owns_its_ground_items_and_takes_them_with_it() {
        let mut ground_items = GroundDistricts::new();
        ground_items.put(148, 1, ground(5, 7, None, 1));
        ground_items.put(148, 1, ground(3, 8, Some(7), 2));
        assert_eq!(
            ground_items
                .items(148, 1)
                .iter()
                .map(|item| item.agent_id)
                .collect::<Vec<_>>(),
            vec![3, 5],
            "by agent id"
        );
        assert!(ground_items.items(148, 2).is_empty(), "another district");
        let taken = ground_items.take(148, 1, 3).expect("claimed");
        assert_eq!(taken.source_item_id, Some(7));
        assert!(ground_items.take(148, 1, 3).is_none(), "claimed once");
        ground_items.close(148, 1);
        assert!(ground_items.items(148, 1).is_empty(), "the copy took them");
    }

    /// A whole drop is a create and its agent; the part of a stack is an update and its agent
    /// (`ASURA/crates/asura-world/src/item.rs:224-234`, `ground_frames`).
    #[test]
    fn a_whole_drop_is_a_create_and_a_partial_is_an_update() {
        let whole = ground_frames(&ground(5, 7, None, 1));
        assert_eq!(&whole[0][..2], &[0x61, 0x01], "create named item");
        assert_eq!(&whole[1][..2], &[0x20, 0x00], "agent created");
        assert_eq!(&whole[1][2..6], &5u32.to_le_bytes(), "the agent");
        assert_eq!(&whole[1][6..10], &7u32.to_le_bytes(), "its model is the item");

        let part = ground_frames(&ground(5, 9, Some(7), 2));
        assert_eq!(&part[0][..2], &[0x62, 0x01], "item update");
        assert_eq!(&part[0][2..6], &9u32.to_le_bytes(), "the ground part's id");
        assert_eq!(&part[1][..2], &[0x20, 0x00], "agent created");
    }
}
