//! The character's rows as the item rules, one operation, one transaction, then the rows as they now
//! are.
//!
//! Ported from `ASURA/crates/asura-world/src/item.rs:237-468` - `model_of` (`:264`), `entry_of`
//! (`:287`), `row_changes` (`:313`), `commit` (`:347`), `commit_ops` (`:360`), `reload` (`:463`) and
//! `snap_back` (`:450`) - read from the local ASURA tree on 2026-11-22. The rules themselves stay in
//! `game/Characters/Inventory.rs`; this module is the bridge between them and `item_instance`.
//!
//! Every client item operation is the same five steps (`ASURA/crates/asura-world/src/item.rs:236-247`,
//! `docs/50-item-framework.md` section 3):
//!
//! 1. build the rules model from the character's rows ([`model_of`]);
//! 2. `InvModel::apply` - the rules, atomic;
//! 3. one transaction for the whole operation ([`DatabaseCharacters::apply_item_changes`]);
//! 4. a fresh read, so the caller's rows are what the database holds ([`reload`]);
//! 5. frames for this client, and a broadcast for what others can see - the handler's half.
//!
//! A refusal changes nothing and, for an item the client already moved in its own model, puts it
//! back ([`snap_back`]).

use crate::characters::{
    BagKind, Change, DatabaseCharacters, Entry, HeldItem, InvModel, ItemKind, Op, Refusal,
    RowChange as RepositoryRow,
};
use crate::creation::Bag;
use crate::data_stores::item_catalogue::{Catalogue, ItemDefinition};

/// The stack limit Guild Wars uses for a stackable item whose definition the pack does not name
/// (`ASURA/crates/asura-world/src/item.rs:250`, `STACK_LIMIT`).
pub const STACK_LIMIT: u32 = 250;

/// One row as the rules see it: where it sits, what kind it is, its stack.
///
/// `ASURA/crates/asura-world/src/item.rs:287`, `entry_of`. The stack limit comes from the row's
/// definition; a row the pack does not define is a stack of [`STACK_LIMIT`] when it already holds more
/// than one, and a single item otherwise.
pub fn entry_of(row: &HeldItem, catalogue: Option<&Catalogue>) -> Result<Entry, String> {
    let definition = catalogue.and_then(|catalogue| {
        let id = (!row.definition_id.is_empty()).then_some(row.definition_id.as_str());
        catalogue.for_item(
            id,
            i64::from(row.file_id),
            i32::from(row.item_type),
            i64::from(row.model_id),
        )
    });
    let stack_max = definition
        .and_then(|definition| definition.stack_limit())
        .unwrap_or(if row.quantity > 1 { STACK_LIMIT } else { 1 });
    // A bag's own item is part of its bag and is in no slot: this tree writes it with bag 0, which
    // no bag has (the content's bags start at 1), the place ASURA's `NULL` bag_id and slot take
    // (`ASURA/crates/asura-world/src/inventory_model.rs:167-179`, `Entry::at`).
    let at = (row.bag_id != 0).then_some((row.bag_id, row.slot));
    Ok(Entry {
        id: row.item_id,
        at,
        kind: ItemKind::of(row.item_type),
        profession: row.profession,
        quantity: row.quantity,
        stack_max,
    })
}

/// The character's rows and bags as the rules model. A row that cannot be a wire value, or two rows in
/// one slot, is a broken inventory; it is reported, not coerced to zero
/// (`ASURA/crates/asura-world/src/item.rs:264`, `model_of`).
pub fn model_of(
    rows: &[HeldItem],
    bags: &[Bag],
    catalogue: Option<&Catalogue>,
) -> Result<InvModel, String> {
    let bags = bags.iter().map(|bag| crate::characters::BagInfo {
        id: bag.id,
        model: i32::from(bag.model),
        kind: BagKind::of(i32::from(bag.model)),
        slots: bag.slots,
    });
    let items = rows
        .iter()
        .map(|row| entry_of(row, catalogue))
        .collect::<Result<Vec<_>, String>>()?;
    InvModel::new(bags, items)
}

/// The row writes one operation's changes amount to.
///
/// `ASURA/crates/asura-world/src/item.rs:313`, `row_changes`. `new_rows` carries the full row of
/// anything an `Op::Insert` placed; the model only knows the fields its rules read.
pub fn row_changes(changes: &[Change], new_rows: &[HeldItem]) -> Vec<RepositoryRow> {
    changes
        .iter()
        .filter_map(|change| match *change {
            Change::Moved { item, to, .. } => Some(RepositoryRow::Place {
                item_id: item,
                bag_id: to.0,
                slot: to.1,
            }),
            Change::Quantity { item, quantity } => Some(RepositoryRow::Quantity {
                item_id: item,
                quantity,
            }),
            Change::Removed { item } => Some(RepositoryRow::Delete { item_id: item }),
            Change::Inserted { item, at } => new_rows
                .iter()
                .find(|row| row.item_id == item)
                .map(|row| RepositoryRow::Insert {
                    item: HeldItem {
                        bag_id: at.0,
                        slot: at.1,
                        ..row.clone()
                    },
                }),
            // What is worn is not a row: the equipped bag's placement is the row, and that change is
            // already a `Moved`.
            Change::Worn { .. } => None,
        })
        .collect()
}

/// What an operation did, as both the rules' changes and the rows now in force.
pub struct Applied {
    /// The changes, in the order the rules produced them.
    pub changes: Vec<Change>,
    /// The character's rows after the transaction, read back from the database.
    pub rows: Vec<HeldItem>,
}

/// Steps 1-4 for one operation. The outer `Result` is a failure of the server (the database); the
/// inner one is the rules' answer (`ASURA/crates/asura-world/src/item.rs:347`, `commit`).
pub fn commit(
    characters: &DatabaseCharacters,
    uuid: &str,
    rows: &[HeldItem],
    bags: &[Bag],
    catalogue: Option<&Catalogue>,
    ops: &[Op],
    wearer: crate::characters::Wearer,
) -> Result<Result<Applied, Refusal>, String> {
    commit_ops(characters, uuid, rows, bags, catalogue, ops, wearer, &[])
}

/// [`commit`] for several operations that stand or fall together, with row writes of the caller's
/// beside them in the same transaction (`ASURA/crates/asura-world/src/item.rs:360`, `commit_ops`). The
/// operations apply in order on one model; a refusal of any leaves everything as it was.
#[allow(clippy::too_many_arguments)]
pub fn commit_ops(
    characters: &DatabaseCharacters,
    uuid: &str,
    rows: &[HeldItem],
    bags: &[Bag],
    catalogue: Option<&Catalogue>,
    ops: &[Op],
    wearer: crate::characters::Wearer,
    new_rows: &[HeldItem],
) -> Result<Result<Applied, Refusal>, String> {
    let mut model = match model_of(rows, bags, catalogue) {
        Ok(model) => model,
        Err(error) => {
            characters.report_inventory_error(uuid, &error);
            return Ok(Err(Refusal::NoSuchItem));
        }
    };
    let mut changes = Vec::new();
    for op in ops {
        match model.apply(*op, wearer) {
            Ok(more) => changes.extend(more),
            Err(refusal) => return Ok(Err(refusal)),
        }
    }
    if !changes.is_empty() || !new_rows.is_empty() {
        let writes = row_changes(&changes, new_rows);
        let written = characters.apply_item_changes(uuid, &writes);
        // Read back either way: after a failed write the caller must see the database again, not the
        // model (`ASURA/crates/asura-world/src/item.rs:394-396`).
        let rows = reload(characters, uuid)?;
        written?;
        return Ok(Ok(Applied { changes, rows }));
    }
    Ok(Ok(Applied {
        changes,
        rows: rows.to_vec(),
    }))
}

/// Re-read the character's rows after a change, so the next message built from them is the new truth
/// (`ASURA/crates/asura-world/src/item.rs:463`, `reload`).
pub fn reload(characters: &DatabaseCharacters, uuid: &str) -> Result<Vec<HeldItem>, String> {
    characters.items_of(uuid)
}

/// Where the rules think the item is, for a client that moved it in its own model before asking
/// (`ASURA/crates/asura-world/src/item.rs:450`, `snap_back`). `None` for an item the character no
/// longer has, and nothing is sent then.
pub fn snap_back(rows: &[HeldItem], item_id: u32) -> Option<(u16, u8)> {
    rows.iter()
        .find(|row| row.item_id == item_id)
        // A bag's own item has no slot, so there is nowhere to put the client's copy back
        // (`ASURA/crates/asura-world/src/item.rs:450-460`).
        .filter(|row| row.bag_id != 0)
        .map(|row| (row.bag_id, row.slot))
}

/// A new owned row from a definition: the one place a definition becomes an instance. The row is not
/// placed - `bag_id`/`slot` are set when the rules choose a slot - and starts with the definition's
/// defaults: no dye, its inherent modifiers, its flags
/// (`ASURA/crates/asura-world/src/item.rs:1008`, `instantiate`).
pub fn instantiate(definition: &ItemDefinition, item_id: u32, quantity: u32) -> HeldItem {
    HeldItem {
        template_id: 0,
        file_id: definition.file_id,
        item_type: definition.item_type,
        dye_tint: 0,
        dye_colors: 0,
        materials: definition.materials,
        unk1: definition.unk1,
        flags: definition.flags,
        value: definition.value,
        model_id: definition.model_id,
        item_id,
        bag_id: 0,
        slot: 0,
        quantity,
        profession: definition.profession,
        name: definition.name.clone(),
        modifiers: definition.modifiers.clone(),
        definition_id: definition.id.clone(),
    }
}

/// A row as the load's item stream draws it: the same fields `creation::Item` carries.
///
/// `ASURA/crates/asura-world/src/item.rs:170-186`, `named_item`: a row is what the client is told
/// about an item, so a frame built after a change reads the row and not the definition.
pub fn item_of_row(row: &HeldItem) -> crate::creation::Item {
    crate::creation::Item {
        id: row.item_id,
        bag_id: (row.bag_id != 0).then_some(row.bag_id),
        slot: (row.bag_id != 0).then_some(row.slot),
        file_id: row.file_id,
        item_type: row.item_type,
        dye_tint: row.dye_tint,
        dye_colors: row.dye_colors,
        materials: row.materials,
        unk1: row.unk1,
        flags: row.flags,
        value: row.value,
        model_id: row.model_id,
        quantity: row.quantity,
        profession: row.profession,
        name: row.name.clone(),
        modifiers: row.modifiers.clone(),
    }
}

/// The bulk `UPDATE_AGENT_VISUAL_EQUIPMENT` (`0x006E`): the nine worn ids, out of the rows.
pub fn equipped_item_ids(rows: &[HeldItem], bags: &[Bag]) -> [u32; 9] {
    let mut ids = [0u32; 9];
    let Some(bag) = bags.iter().find(|bag| bag.model == crate::characters::EQUIPPED_ITEMS_MODEL)
    else {
        return ids;
    };
    for row in rows {
        if row.bag_id == bag.id
            && let Some(held) = ids.get_mut(usize::from(row.slot))
        {
            *held = row.item_id;
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::characters::Op;

    fn bag(id: u16, model: u8, slots: u8) -> Bag {
        Bag {
            id,
            model,
            kind: bag_kind(model),
            slots,
            assoc_item: 0,
        }
    }

    fn bag_kind(model: u8) -> u8 {
        match model {
            1..=3 => 1,
            4 => 2,
            5 => 4,
            _ => 1,
        }
    }

    fn row(item_id: u32, bag_id: u16, slot: u8, item_type: u8) -> HeldItem {
        HeldItem {
            template_id: 0,
            file_id: 1,
            item_type,
            dye_tint: 0,
            dye_colors: 0,
            materials: 0,
            unk1: 0,
            flags: 0,
            value: 0,
            model_id: 1,
            item_id,
            bag_id,
            slot,
            quantity: 1,
            profession: 0,
            name: Vec::new(),
            modifiers: Vec::new(),
            definition_id: String::new(),
        }
    }

    #[test]
    fn a_row_becomes_the_entry_the_rules_read() {
        let held = row(7, 1, 2, 15);
        let entry = entry_of(&held, None).expect("a valid row");
        assert_eq!(entry.id, 7);
        assert_eq!(entry.at, Some((1, 2)));
        assert_eq!(entry.kind, ItemKind::of(15));
        assert_eq!(entry.quantity, 1);
        assert_eq!(entry.stack_max, 1, "an unknown single item stacks to one");
    }

    #[test]
    fn an_unknown_stack_above_one_uses_the_wire_limit() {
        let mut held = row(7, 1, 2, 21);
        held.quantity = 40;
        assert_eq!(entry_of(&held, None).expect("a row").stack_max, STACK_LIMIT);
    }

    #[test]
    fn a_move_is_one_placement_and_a_destroy_is_one_delete() {
        let moved = row_changes(
            &[Change::Moved {
                item: 7,
                from: (1, 2),
                to: (3, 4),
            }],
            &[],
        );
        assert_eq!(
            moved,
            vec![RepositoryRow::Place {
                item_id: 7,
                bag_id: 3,
                slot: 4
            }]
        );

        let removed = row_changes(&[Change::Removed { item: 7 }], &[]);
        assert_eq!(removed, vec![RepositoryRow::Delete { item_id: 7 }]);
    }

    #[test]
    fn an_insert_takes_its_place_from_the_rules_not_the_row() {
        let new = row(9, 0, 0, 21);
        let changes = vec![Change::Inserted {
            item: 9,
            at: (1, 5),
        }];
        let writes = row_changes(&changes, std::slice::from_ref(&new));
        match &writes[0] {
            RepositoryRow::Insert { item } => {
                assert_eq!(item.item_id, 9);
                assert_eq!((item.bag_id, item.slot), (1, 5));
            }
            other => panic!("expected an insert, got {other:?}"),
        }
    }

    #[test]
    fn the_model_refuses_two_rows_in_one_slot() {
        let bags = [bag(1, 1, 10)];
        let rows = [row(7, 1, 2, 15), row(8, 1, 2, 15)];
        let error = model_of(&rows, &bags, None).expect_err("two items in one slot");
        assert!(error.contains("both in bag 1 slot 2"), "{error}");
    }

    #[test]
    fn a_definition_the_pack_names_answers_its_own_stack_limit() {
        let text = std::fs::read_to_string("../config/World/Vanilla/items.json")
            .expect("the committed catalogue reads");
        let catalogue = Catalogue::from_json(&text).expect("the catalogue parses");
        let mut held = row(7, 1, 2, 9);
        held.file_id = 293_940;
        held.model_id = 22_269;
        held.quantity = 40;
        held.definition_id = "birthday_cupcake".to_string();
        assert_eq!(
            entry_of(&held, Some(&catalogue)).expect("a row").stack_max,
            250
        );
        // Without the id the wire fields still resolve the same definition: this is the fallback a row
        // written before the column uses.
        held.definition_id.clear();
        assert_eq!(
            entry_of(&held, Some(&catalogue)).expect("a row").stack_max,
            250
        );
    }

    #[test]
    fn an_operation_that_changes_nothing_writes_nothing() {
        // A move to where the item already is: `InvModel::apply` answers no changes, so `commit_ops`
        // must not open a transaction or reload.
        let bags = [bag(1, 1, 10)];
        let rows = [row(7, 1, 2, 15)];
        let model = model_of(&rows, &bags, None).expect("a model");
        let mut model = model;
        let changes = model
            .apply(
                Op::Move {
                    item: 7,
                    bag: 1,
                    slot: 2,
                },
                crate::characters::Wearer {
                    primary_profession: 1,
                    storage_open: false,
                },
            )
            .expect("a no-op is not a refusal");
        assert!(changes.is_empty());
    }
}
