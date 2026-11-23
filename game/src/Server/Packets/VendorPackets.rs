//! The vendor family: an NPC's trade window (merchant, collector), its transactions, the gold the
//! character carries and the gold in storage, the Xunlai storage window, and the walk to an agent
//! that opens all of them (`0x002A`).
//!
//! Ported 1:1 from `ASURA/crates/asura-protocol/src/game/vendor.rs` (445 lines, read from the local
//! ASURA tree on 2026-11-23): every builder, both readers and every test. The two recordings ASURA
//! measured it from are cited where each value is used:
//!
//! * `recordings/client/feature/movement/movement-live-20261109-200703.jsonl` (ASURA's tree, a live
//!   server, both directions): a dialog, a `SEND_DIALOG` choice and the merchant window it opens
//!   (seq 5171-5288);
//! * preforged's `data/captures/pre-searing-side-quests-2026-10-12/checkpoint-1815/packets-43292-168891734.jsonl`
//!   (a live pre-Searing session, server-to-client only): seventeen trade windows, one purchase and one
//!   collector exchange, and the gold messages of every load. "Capture line N" counts packet records
//!   in that file from 0 (`ASURA/docs/53-npc-services.md`).
//!
//! What a client sends is preforged's reading of the 2022 client (`gw_collector_transaction.h`,
//! `gw_inventory_gold.h`), matched to this build's table by layout (`vendor.rs:16-18`).
//!
//! `0x002A` is ASURA's `game::agent_update_destination` (`asura-protocol/src/game/mod.rs:2311-2336`):
//! it is here because the walk to an agent (`game/Handlers/NpcHandler.rs`, ASURA's `approach.rs`) is
//! the only thing that sends it.

use crate::server::packets::agent_packets;
use crate::server::protocol::tables::game as msg;

// ---- the window kinds (`0x00C3` field 1, `0x00CC` field 1) --------------------------------------------

/// `0x00C3` and `0x00CC` kind: a merchant's sell tab, and a sale's completion (`vendor.rs:25-27`; live
/// `0x00C3 {11, 0}` at seq 5286).
pub const KIND_SELL: u8 = 11;
/// `0x00CC` kind: a purchase's completion (`vendor.rs:28-29`; capture line 487670).
pub const KIND_BUY: u8 = 1;
/// `0x00CA` kind of a merchant's buy pane (`vendor.rs:30-31`; live `{1, 1.0}` at seq 5285).
pub const BUY_PANE_MERCHANT: u8 = 1;
/// `0x00C5` and `0x00CC` kind: a collector's offer (`vendor.rs:32-34`; capture lines 537235, 537266).
pub const KIND_COLLECTOR: u8 = 2;

/// `AGENT_PROPERTY_UPDATE_INT` kind the live server sends on the NPC after every answer to a talk,
/// value 0 (`vendor.rs:36-39`; seq 5185, 5270, 5288; capture line 537236). Meaning not established;
/// copied.
pub const PROPERTY_TALKED_TO: u32 = 12;

/// The unnamed `0x00CA`: a byte and a dword, the buy pane (`vendor.rs:41-44`).
pub const WINDOW_BUY_PANE: u16 = 0x00CA;
/// The unnamed `0x00C5`: a byte and a coded string, a collector's window (`vendor.rs:45-47`).
pub const WINDOW_COLLECTOR: u16 = 0x00C5;
/// The most units `0x00C5`'s heading carries (`vendor.rs:86`, `.units(heading, 122)`).
pub const COLLECTOR_HEADING_UNITS: usize = 122;

/// A frame: the `u16` header and then the body.
fn frame(header: u16, body: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(&body);
    out
}

// ---- the window --------------------------------------------------------------------------------------

/// `GAME_SMSG_MERCHANT_WINDOW_UPDATE_OWNER` (`0x00C4`): the agent the window belongs to. First in every
/// trade window (`vendor.rs:51-57`; seq 5281).
pub fn window_owner(agent_id: u32) -> Vec<u8> {
    frame(
        msg::GAME_SMSG_MERCHANT_WINDOW_UPDATE_OWNER,
        agent_id.to_le_bytes().to_vec(),
    )
}

/// `GAME_SMSG_WINDOW_ADD_ITEMS` (`0x0084`): the ids the window offers, each already declared by
/// `0x0161`, in window order - a `u16` count and the dwords (`vendor.rs:59-65`; seq 5284).
pub fn window_add_items(item_ids: &[u32]) -> Vec<u8> {
    let mut body = Vec::with_capacity(2 + 4 * item_ids.len());
    body.extend_from_slice(&(item_ids.len() as u16).to_le_bytes());
    for id in item_ids {
        body.extend_from_slice(&id.to_le_bytes());
    }
    frame(msg::GAME_SMSG_WINDOW_ADD_ITEMS, body)
}

/// `0x00CA`: the buy pane, its kind and the price scale (`vendor.rs:67-71`). `{1, 1.0}` in every live
/// merchant window.
pub fn window_buy_pane(kind: u8, scale: f32) -> Vec<u8> {
    let mut body = vec![kind];
    body.extend_from_slice(&scale.to_le_bytes());
    frame(WINDOW_BUY_PANE, body)
}

/// `GAME_SMSG_MERCHANT_WINDOW_OPEN` (`0x00C3`): the window's second tab. `{11, 0}` is the merchant's
/// sell tab, sent last (`vendor.rs:73-80`; seq 5286).
pub fn merchant_window_open(kind: u8, data: u32) -> Vec<u8> {
    let mut body = vec![kind];
    body.extend_from_slice(&data.to_le_bytes());
    frame(msg::GAME_SMSG_MERCHANT_WINDOW_OPEN, body)
}

/// `0x00C5`: a collector's window, its kind (2) and the line it says over its offers
/// (`vendor.rs:82-88`).
pub fn window_collector(kind: u8, heading: &[u16]) -> Vec<u8> {
    let used = &heading[..heading.len().min(COLLECTOR_HEADING_UNITS)];
    let mut body = vec![kind];
    body.extend_from_slice(&(used.len() as u16).to_le_bytes());
    for unit in used {
        body.extend_from_slice(&unit.to_le_bytes());
    }
    frame(WINDOW_COLLECTOR, body)
}

/// The property the live server sends on the NPC after an answer to a talk (`vendor.rs:90-93`).
pub fn talked_to(npc_agent: u32) -> Vec<u8> {
    agent_packets::agent_property_update_int(PROPERTY_TALKED_TO, npc_agent, 0)
}

// ---- gold -------------------------------------------------------------------------------------------

/// `GAME_SMSG_UPDATE_GOLD_CHARACTER` (`0x0140`): add this much to the carried gold. A delta, not a
/// balance (`vendor.rs:97-105`; capture line 472025).
pub fn gold_character_add(stream: u16, amount: u32) -> Vec<u8> {
    let mut body = stream.to_le_bytes().to_vec();
    body.extend_from_slice(&amount.to_le_bytes());
    frame(msg::GAME_SMSG_UPDATE_GOLD_CHARACTER, body)
}

/// `GAME_SMSG_UPDATE_GOLD_STORAGE` (`0x0141`): add this much to the stored gold (`vendor.rs:107-111`;
/// capture line 472026).
pub fn gold_storage_add(stream: u16, amount: u32) -> Vec<u8> {
    let mut body = stream.to_le_bytes().to_vec();
    body.extend_from_slice(&amount.to_le_bytes());
    frame(msg::GAME_SMSG_UPDATE_GOLD_STORAGE, body)
}

/// `GAME_SMSG_GOLD_CHARACTER_REMOVE` (`0x014F`): take this much off the carried gold. A purchase pays
/// with it, before the item exists (`vendor.rs:113-120`; capture line 487667).
pub fn gold_character_remove(stream: u16, amount: u32) -> Vec<u8> {
    let mut body = stream.to_le_bytes().to_vec();
    body.extend_from_slice(&amount.to_le_bytes());
    frame(msg::GAME_SMSG_GOLD_CHARACTER_REMOVE, body)
}

/// `GAME_SMSG_GOLD_STORAGE_REMOVE` (`0x0150`): take this much off the stored gold. Not captured; the
/// layout is the table's and the meaning preforged's (`vendor.rs:122-129`).
pub fn gold_storage_remove(stream: u16, amount: u32) -> Vec<u8> {
    let mut body = stream.to_le_bytes().to_vec();
    body.extend_from_slice(&amount.to_le_bytes());
    frame(msg::GAME_SMSG_GOLD_STORAGE_REMOVE, body)
}

/// The deltas from one pair of balances to another: debits before credits, nothing for a balance that
/// does not change (`vendor.rs:131-158`).
pub fn gold_deltas(stream: u16, before: (u32, u32), after: (u32, u32)) -> Vec<Vec<u8>> {
    let (carried_before, stored_before) = before;
    let (carried_after, stored_after) = after;
    let mut frames = Vec::new();
    if carried_after < carried_before {
        frames.push(gold_character_remove(stream, carried_before - carried_after));
    }
    if stored_after < stored_before {
        frames.push(gold_storage_remove(stream, stored_before - stored_after));
    }
    if carried_after > carried_before {
        frames.push(gold_character_add(stream, carried_after - carried_before));
    }
    if stored_after > stored_before {
        frames.push(gold_storage_add(stream, stored_after - stored_before));
    }
    frames
}

// ---- Xunlai storage -----------------------------------------------------------------------------------

/// `GAME_SMSG_WINDOW_OPEN` (`0x0083`) kind of the Xunlai storage window (`vendor.rs:162-163`).
pub const WINDOW_STORAGE: u8 = 0;
/// The storage window's permission word: bit 1 deposit, bit 2 withdraw (`vendor.rs:164-167`).
/// Preforged's reading of the client; not captured on a live server.
pub const STORAGE_DEPOSIT_AND_WITHDRAW: u32 = 3;
/// The permission word a dialog choice's window carries on the live server (`vendor.rs:169-172`;
/// seq 5247). Copied.
pub const WINDOW_DIALOG_PERMISSION: u32 = 0xCCCC_CCCC;

/// `GAME_SMSG_WINDOW_OPEN` (`0x0083`): a window owned by `agent_id`, of `kind`, with a permission word
/// (`vendor.rs:174-182`).
pub fn window_open(agent_id: u32, kind: u8, permissions: u32) -> Vec<u8> {
    let mut body = agent_id.to_le_bytes().to_vec();
    body.push(kind);
    body.extend_from_slice(&permissions.to_le_bytes());
    frame(msg::GAME_SMSG_WINDOW_OPEN, body)
}

/// The Xunlai storage window, owned by `agent_id` (`vendor.rs:184-187`).
pub fn storage_window_open(agent_id: u32) -> Vec<u8> {
    window_open(agent_id, WINDOW_STORAGE, STORAGE_DEPOSIT_AND_WITHDRAW)
}

// ---- the walk to an agent ------------------------------------------------------------------------------

/// `GAME_SMSG_AGENT_UPDATE_DESTINATION` (`0x002A`): `agent` is walking to the agent `target`, which
/// stands at `destination` (`ASURA/crates/asura-protocol/src/game/mod.rs:2311-2336`; live seq 626 ->
/// 630 of `movement-live-20261109-200703.jsonl`).
pub fn agent_update_destination(
    agent_id: u32,
    destination: (f32, f32),
    plane: u16,
    current_plane: u16,
    target: u32,
) -> Vec<u8> {
    let mut body = agent_id.to_le_bytes().to_vec();
    body.extend_from_slice(&destination.0.to_le_bytes());
    body.extend_from_slice(&destination.1.to_le_bytes());
    body.extend_from_slice(&plane.to_le_bytes());
    body.extend_from_slice(&current_plane.to_le_bytes());
    body.extend_from_slice(&target.to_le_bytes());
    frame(msg::GAME_SMSG_AGENT_UPDATE_DESTINATION, body)
}

// ---- the requests ------------------------------------------------------------------------------------

/// One item a transaction names, and how many. A `0x804A` request carries no quantities; its entries
/// read 0, which means "all of it" (`vendor.rs:191-199`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeEntry {
    /// An owned item on the give side, a window's offered id on the receive side.
    pub item: u32,
    /// How many; 0 in a `0x804A` request.
    pub quantity: u8,
}

/// A trade window's request: `0x804A` (whole stacks) or `0x804D` (with quantities)
/// (`vendor.rs:201-216`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// The header it came on.
    pub header: u16,
    /// The window kind: 1 buy, 11 sell, 2 collector.
    pub kind: u8,
    /// The gold the client gives (a purchase's price).
    pub give_gold: u32,
    /// What the client gives (a sale's item, a collector's trophies).
    pub give: Vec<TradeEntry>,
    /// The gold the client expects back (a sale's price).
    pub receive_gold: u32,
    /// What the client wants (a purchase's stock id, a collector's offer id).
    pub receive: Vec<TradeEntry>,
}

/// `GAME_CMSG_TRANSACT_ITEMS` (`0x804D`).
pub const TRANSACT_ITEMS: u16 = msg::GAME_CMSG_TRANSACT_ITEMS;
/// `GAME_CMSG_BUY_MATERIALS` (`0x804A`): a merchant sale and a collector exchange use it too.
pub const BUY_MATERIALS: u16 = msg::GAME_CMSG_BUY_MATERIALS;
/// `GAME_CMSG_ITEM_CHANGE_GOLD` (`0x807C`).
pub const CHANGE_GOLD: u16 = msg::GAME_CMSG_ITEM_CHANGE_GOLD;
/// `GAME_CMSG_INTERACT_LIVING` (`0x8039`): an agent id and a byte.
pub const INTERACT_LIVING: u16 = msg::GAME_CMSG_INTERACT_LIVING;
/// `GAME_CMSG_INTERACT_PLAYER` (`0x8033`): an agent id.
pub const INTERACT_PLAYER: u16 = msg::GAME_CMSG_INTERACT_PLAYER;

/// The most ids one side of a request carries (`vendor.rs:224-225`).
pub const TRADE_ENTRIES: usize = 16;

struct Reader<'a> {
    body: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Option<u8> {
        let value = *self.body.get(self.at)?;
        self.at += 1;
        Some(value)
    }

    fn u16(&mut self) -> Option<u16> {
        let bytes: [u8; 2] = self.body.get(self.at..self.at + 2)?.try_into().ok()?;
        self.at += 2;
        Some(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Option<u32> {
        let bytes: [u8; 4] = self.body.get(self.at..self.at + 4)?.try_into().ok()?;
        self.at += 4;
        Some(u32::from_le_bytes(bytes))
    }

    /// One side: a counted list of ids and, for `0x804D`, a counted list of byte quantities of the
    /// same length (`vendor.rs:251-274`).
    fn side(&mut self, with_quantities: bool) -> Option<Vec<TradeEntry>> {
        let count = usize::from(self.u16()?);
        if count > TRADE_ENTRIES {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(TradeEntry {
                item: self.u32()?,
                quantity: 0,
            });
        }
        if with_quantities {
            if usize::from(self.u16()?) != count {
                return None;
            }
            for entry in &mut entries {
                entry.quantity = self.u8()?;
            }
        }
        Some(entries)
    }

    fn done(&self) -> bool {
        self.at == self.body.len()
    }
}

/// Read a `0x804A` or `0x804D` body - the frame without its header. `None` for any other header, a
/// list longer than the row allows, mismatched quantity lists, or bytes left over (`vendor.rs:281-304`).
pub fn read_transaction(header: u16, body: &[u8]) -> Option<Transaction> {
    let with_quantities = match header {
        TRANSACT_ITEMS => true,
        BUY_MATERIALS => false,
        _ => return None,
    };
    let mut reader = Reader { body, at: 0 };
    let kind = reader.u8()?;
    let give_gold = reader.u32()?;
    let give = reader.side(with_quantities)?;
    let receive_gold = reader.u32()?;
    let receive = reader.side(with_quantities)?;
    reader.done().then_some(Transaction {
        header,
        kind,
        give_gold,
        give,
        receive_gold,
        receive,
    })
}

/// Read a `0x807C` body: the carried and the stored balance the client asks to end with
/// (`vendor.rs:306-313`).
pub fn read_change_gold(body: &[u8]) -> Option<(u32, u32)> {
    let mut reader = Reader { body, at: 0 };
    let carried = reader.u32()?;
    let stored = reader.u32()?;
    reader.done().then_some((carried, stored))
}

#[cfg(test)]
mod tests {
    use super::*;

    // `vendor.rs:323-342`.
    #[test]
    fn a_merchant_window_is_the_live_order_and_widths() {
        assert_eq!(window_owner(0x36), vec![0xC4, 0x00, 0x36, 0, 0, 0]);
        assert_eq!(
            window_add_items(&[0x46D, 0x45E]),
            vec![0x84, 0x00, 2, 0, 0x6D, 0x04, 0, 0, 0x5E, 0x04, 0, 0]
        );
        assert_eq!(
            window_buy_pane(BUY_PANE_MERCHANT, 1.0),
            vec![0xCA, 0x00, 1, 0x00, 0x00, 0x80, 0x3F]
        );
        assert_eq!(
            merchant_window_open(KIND_SELL, 0),
            vec![0xC3, 0x00, 11, 0, 0, 0, 0]
        );
        assert_eq!(
            talked_to(0x36),
            vec![0x9F, 0x00, 12, 0, 0, 0, 0x36, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    // `vendor.rs:344-354`: capture line 537235, Brownlow's line over Skale Fins.
    #[test]
    fn a_collector_window_carries_its_kind_and_heading() {
        let heading = [
            0x2B33, 0xD6E2, 0xD8C7, 0x0A6A, 0x010A, 0x2906, 0x0001, 0x0101, 0x0105,
        ];
        let wire = window_collector(KIND_COLLECTOR, &heading);
        assert_eq!(&wire[..5], &[0xC5, 0x00, 2, 9, 0]);
        assert_eq!(&wire[5..7], &[0x33, 0x2B]);
        assert_eq!(wire.len(), 5 + 18);
    }

    // `vendor.rs:356-369`.
    #[test]
    fn gold_moves_debit_first_and_skip_what_does_not_change() {
        let frames = gold_deltas(1, (100, 50), (40, 110));
        let headers: Vec<u16> = frames
            .iter()
            .map(|frame| u16::from_le_bytes([frame[0], frame[1]]))
            .collect();
        assert_eq!(
            headers,
            vec![
                msg::GAME_SMSG_GOLD_CHARACTER_REMOVE,
                msg::GAME_SMSG_UPDATE_GOLD_STORAGE
            ]
        );
        assert_eq!(frames[0], vec![0x4F, 0x01, 1, 0, 60, 0, 0, 0]);
        assert!(gold_deltas(1, (7, 7), (7, 7)).is_empty());
    }

    // `vendor.rs:371-377`.
    #[test]
    fn the_storage_window_names_its_owner_and_both_permissions() {
        assert_eq!(
            storage_window_open(0x2A),
            vec![0x83, 0x00, 0x2A, 0, 0, 0, 0, 3, 0, 0, 0]
        );
    }

    // `vendor.rs:379-387`: seq 5247.
    #[test]
    fn a_dialog_choice_window_copies_the_live_kind_and_permission() {
        assert_eq!(
            window_open(0x36, 5, WINDOW_DIALOG_PERMISSION),
            vec![0x83, 0x00, 0x36, 0, 0, 0, 5, 0xCC, 0xCC, 0xCC, 0xCC]
        );
    }

    // `vendor.rs:389-414`: Rurik's retail purchase.
    #[test]
    fn a_purchase_request_reads_its_price_and_its_stock_id() {
        let mut body = vec![1u8];
        body.extend_from_slice(&40u32.to_le_bytes());
        body.extend_from_slice(&[0, 0, 0, 0]);
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&[1, 0]);
        body.extend_from_slice(&2474u32.to_le_bytes());
        body.extend_from_slice(&[1, 0, 1]);
        let request = read_transaction(TRANSACT_ITEMS, &body).expect("a request");
        assert_eq!(request.kind, KIND_BUY);
        assert_eq!(request.give_gold, 40);
        assert!(request.give.is_empty());
        assert_eq!(
            request.receive,
            vec![TradeEntry {
                item: 2474,
                quantity: 1
            }]
        );
        let mut longer = body.clone();
        longer.push(0);
        assert!(read_transaction(TRANSACT_ITEMS, &longer).is_none());
    }

    // `vendor.rs:416-435`: Rurik's retail sale.
    #[test]
    fn a_whole_stack_sale_has_no_quantities() {
        let mut body = vec![KIND_SELL];
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&[1, 0]);
        body.extend_from_slice(&77u32.to_le_bytes());
        body.extend_from_slice(&20u32.to_le_bytes());
        body.extend_from_slice(&[0, 0]);
        let request = read_transaction(BUY_MATERIALS, &body).expect("a request");
        assert_eq!(
            request.give,
            vec![TradeEntry {
                item: 77,
                quantity: 0
            }]
        );
        assert_eq!(request.receive_gold, 20);
        assert!(request.receive.is_empty());
    }

    // `vendor.rs:437-443`.
    #[test]
    fn a_gold_change_is_two_final_balances() {
        let mut body = 25u32.to_le_bytes().to_vec();
        body.extend_from_slice(&75u32.to_le_bytes());
        assert_eq!(read_change_gold(&body), Some((25, 75)));
        assert_eq!(read_change_gold(&body[..7]), None);
    }

    // `asura-world/src/approach.rs:352-366`: the destination's agent, point and target.
    #[test]
    fn a_destination_names_the_walker_the_point_and_the_target() {
        let wire = agent_update_destination(276, (1000.0, 0.0), 0, 0, 48);
        assert_eq!(&wire[..2], &[0x2A, 0x00]);
        let body = &wire[2..];
        assert_eq!(&body[0..4], &276u32.to_le_bytes());
        assert_eq!(&body[4..8], &1000.0f32.to_le_bytes());
        assert_eq!(&body[16..20], &48u32.to_le_bytes());
    }
}
