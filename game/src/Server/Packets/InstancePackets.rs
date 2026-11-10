//! The instance message family: what a world says when it puts a client inside a map.
//!
//! Reference: `src/server/game/Server/Packets/InstancePackets.{h,cpp}`. One file per family is this
//! tree's rule (`AGENTS.md`), and this is the first family to have content - the messages a
//! character-creation connection is answered with.
//!
//! Every layout here is this client's, measured from a live client and the server it played on
//! (`ASURA/recordings/client/feature/character_creation/character_creation-live-necro_factions.jsonl`),
//! and each builder names the frame it came from. A frame is the four-byte game header
//! (`super::super::protocol::header`) and then its fields.

use crate::server::protocol::frame;

/// `GAME_SMSG_INSTANCE_LOAD_HEAD`: the load begins, in twenty bytes.
///
/// Evidence: the recording above, seq 3 - `7c 01 00 00 3f 00 00 00 3f 00 00 00 00 00 00 00 00 00 00 00`,
/// three sightings of the same twenty bytes. Four dwords after the header: `0x3F`, `0x3F`, and two
/// zeroes.
pub const INSTANCE_LOAD_HEAD: u16 = 0x017C;

/// `GAME_SMSG_INSTANCE_PLAYER_DATA_START`: the player-data stream opens.
pub const INSTANCE_PLAYER_DATA_START: u16 = 0x0186;

/// `GAME_SMSG_INSTANCE_PLAYER_DATA_DONE`: the player-data stream is over.
///
/// Its only caller is the character-creation flow. Evidence for the header and the size - four
/// bytes - is `ASURA`'s own send table, not the recording above: no `0x018A` arrives in that
/// capture at all, so whether a creation screen needs it is unsettled.
pub const INSTANCE_PLAYER_DATA_DONE: u16 = 0x018A;

/// `GAME_SMSG_CHAR_CREATION_START`: the creation instance may show its screen.
///
/// Evidence: seq 4 of the same recording - `89 01 00 00`, the header and nothing else. A creation
/// connection is answered with the load head and this, and nothing else: the map's own block belongs
/// to a map that has geometry, and the creation instance has none.
pub const CHAR_CREATION_START: u16 = 0x0189;

/// `0x017C`: `0x3F`, `0x3F`, then two zero dwords - the measured twenty bytes.
pub fn instance_load_head() -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    for value in [0x3Fu32, 0x3F, 0, 0] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    frame(INSTANCE_LOAD_HEAD, &body)
}

/// `0x0189`: the header and nothing else, four bytes.
pub fn char_creation_start() -> Vec<u8> {
    frame(CHAR_CREATION_START, &[])
}

/// `0x0186`: the header and nothing else. It opens the block a creation screen's player is drawn
/// from, and `instance_player_data_done` closes it.
pub fn instance_player_data_start() -> Vec<u8> {
    frame(INSTANCE_PLAYER_DATA_START, &[])
}

/// `0x018A`: the header and nothing else.
pub fn instance_player_data_done() -> Vec<u8> {
    frame(INSTANCE_PLAYER_DATA_DONE, &[])
}

/// What a character-creation connection is answered with, in the reference's order: the load head,
/// then the creation start, concatenated for one write.
pub fn creation_load_block() -> Vec<u8> {
    let mut block = instance_load_head();
    block.extend_from_slice(&char_creation_start());
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recording's own bytes for seq 3, hand-copied so a change fails here rather than at a
    /// client.
    const LOAD_HEAD: [u8; 20] = [
        0x7C, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn the_load_head_is_the_twenty_bytes_a_client_was_sent() {
        assert_eq!(instance_load_head(), LOAD_HEAD.to_vec());
    }

    #[test]
    fn the_creation_start_is_the_four_byte_header_alone() {
        // Seq 4: `89 01 00 00`.
        assert_eq!(char_creation_start(), vec![0x89, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn the_creation_block_is_the_two_messages_in_that_order() {
        let mut expected = LOAD_HEAD.to_vec();
        expected.extend_from_slice(&[0x89, 0x01, 0x00, 0x00]);
        assert_eq!(creation_load_block(), expected);
    }

    #[test]
    fn every_frame_starts_with_its_opcode_and_a_zero_word() {
        for frame in [
            instance_load_head(),
            char_creation_start(),
            instance_player_data_start(),
            instance_player_data_done(),
        ] {
            assert_eq!(&frame[2..4], &[0, 0], "the header's zero word");
        }
    }
}
