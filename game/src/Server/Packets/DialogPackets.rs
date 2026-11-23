//! The dialog family: an NPC's text, who says it, the choices under it, and the client's choice.
//!
//! Ported 1:1 from `ASURA/crates/asura-protocol/src/game/dialog.rs` (113 lines, read from the local
//! ASURA tree on 2026-11-23): every builder, the reader and every test. Measured on a live server
//! (`recordings/client/feature/movement/movement-live-20261109-200703.jsonl` in ASURA's tree, seq
//! 5171-5185 and 5260-5270): a talk is answered with one `0x0080` body, `0x0081` naming the NPC, one
//! `0x007E` per choice, then `0x009F {12, npc, 0}` (`vendor_packets::talked_to`). The client answers a
//! choice with `0x803B {dialog id}` (seq 5245, 5277). The body and the choice labels are coded strings:
//! string ids and markers, not text.

use crate::server::protocol::tables::game as msg;

/// The units one `0x0080` carries: its declared 122 minus one, as a body longer than that is split over
/// several (preforged `gw_npc_dialogue.h`, `open_encoded`: the client appends each part to its buffer)
/// (`dialog.rs:12-14`).
pub const DIALOG_BODY_UNITS: usize = 121;
/// The units a choice's label may hold (`0x007E`'s string field) (`dialog.rs:15-16`).
pub const DIALOG_LABEL_UNITS: usize = 128;
/// The skill field of a choice that names no skill, as preforged sends it. The live server sent `0` on
/// one choice and `0xFFFFFFFF` on the rest of the same dialog (seq 5178, 5179) (`dialog.rs:17-20`).
pub const DIALOG_NO_SKILL: u32 = 0xFFFF_FFFF;

/// `GAME_CMSG_SEND_DIALOG` (`0x803B`): the client chose a dialog button.
pub const SEND_DIALOG: u16 = msg::GAME_CMSG_SEND_DIALOG;

/// A frame: the `u16` header and then the body.
fn frame(header: u16, body: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// A string field: its `u16` unit count, then the units (ASURA's `Builder::units`).
fn units(body: &mut Vec<u8>, units: &[u16], max: usize) {
    let used = &units[..units.len().min(max)];
    body.extend_from_slice(&(used.len() as u16).to_le_bytes());
    for unit in used {
        body.extend_from_slice(&unit.to_le_bytes());
    }
}

/// `GAME_SMSG_DIALOG_BODY` (`0x0080`): the text, split into as many messages as it takes. A unit that
/// would end a part as the first half of a surrogate pair moves to the next part (`dialog.rs:22-43`).
pub fn dialog_body(text: &[u16]) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let mut count = DIALOG_BODY_UNITS.min(text.len() - at);
        if at + count < text.len() && (0xD800..=0xDBFF).contains(&text[at + count - 1]) {
            count -= 1;
        }
        let mut body = Vec::with_capacity(2 + 2 * count);
        units(&mut body, &text[at..at + count], DIALOG_BODY_UNITS + 1);
        frames.push(frame(msg::GAME_SMSG_DIALOG_BODY, body));
        at += count;
    }
    frames
}

/// `GAME_SMSG_DIALOG_SENDER` (`0x0081`): the agent saying the body, which shows it (`dialog.rs:45-50`).
pub fn dialog_sender(agent_id: u32) -> Vec<u8> {
    frame(msg::GAME_SMSG_DIALOG_SENDER, agent_id.to_le_bytes().to_vec())
}

/// `GAME_SMSG_DIALOG_BUTTON` (`0x007E`): one choice: its icon, its label, the id the client sends back,
/// and a skill (`dialog.rs:52-61`).
pub fn dialog_button(icon: u8, label: &[u16], dialog_id: u32, skill: u32) -> Vec<u8> {
    let mut body = vec![icon];
    units(&mut body, label, DIALOG_LABEL_UNITS);
    body.extend_from_slice(&dialog_id.to_le_bytes());
    body.extend_from_slice(&skill.to_le_bytes());
    frame(msg::GAME_SMSG_DIALOG_BUTTON, body)
}

/// Read a `0x803B` (`GAME_CMSG_SEND_DIALOG`) body: the chosen dialog id (`dialog.rs:63-67`).
pub fn read_send_dialog(body: &[u8]) -> Option<u32> {
    let bytes: [u8; 4] = body.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_body_is_one_message() {
        // seq 5176: [0x8102, 0x3E7E, 0xD7B5, 0xDBE4, 0x49F2] (`dialog.rs:73-81`).
        let frames = dialog_body(&[0x8102, 0x3E7E, 0xD7B5, 0xDBE4, 0x49F2]);
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0],
            vec![0x80, 0x00, 5, 0, 0x02, 0x81, 0x7E, 0x3E, 0xB5, 0xD7, 0xE4, 0xDB, 0xF2, 0x49]
        );
    }

    #[test]
    fn a_long_body_is_split_and_never_inside_a_pair() {
        // `dialog.rs:83-99`.
        let mut text = vec![0x41u16; 120];
        text.push(0xD800);
        text.push(0xDC00);
        text.extend(std::iter::repeat_n(0x42, 10));
        let frames = dialog_body(&text);
        assert_eq!(frames.len(), 2);
        assert_eq!(u16::from_le_bytes([frames[0][2], frames[0][3]]), 120);
        assert_eq!(u16::from_le_bytes([frames[1][2], frames[1][3]]), 12);
    }

    #[test]
    fn a_choice_is_icon_label_id_skill() {
        // `dialog.rs:101-112`.
        let frame = dialog_button(0x0B, &[0x8102, 0x3EB5], 0x87, DIALOG_NO_SKILL);
        assert_eq!(
            frame,
            vec![
                0x7E, 0x00, 0x0B, 2, 0, 0x02, 0x81, 0xB5, 0x3E, 0x87, 0, 0, 0, 0xFF, 0xFF, 0xFF,
                0xFF
            ]
        );
        assert_eq!(read_send_dialog(&[0x8B, 0, 0, 0]), Some(0x8B));
        assert_eq!(read_send_dialog(&[0x8B, 0, 0]), None);
    }
}
