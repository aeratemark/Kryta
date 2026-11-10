//! The instance message family: what a world says when it puts a client inside a map.
//!
//! Reference: `src/server/game/Server/Packets/InstancePackets.{h,cpp}`. One file per family is this
//! tree's rule (`AGENTS.md`), and this is the first family to have content - the messages a
//! character-creation connection is answered with.
//!
//! Sizes and layouts are this client's, measured from a server that works with it; each builder
//! names where its bytes came from. A message here is a whole wire frame: the `u16` header and then
//! its fields, with no length anywhere.

/// `GAME_SMSG_INSTANCE_LOAD_HEAD`: the load begins.
///
/// Two bytes of flags and two zero bytes. Evidence: the reference's own send log,
/// `SEND 0x017C 6 bytes : 7C 01 3F 3F 00 00`, and a working server's builder that writes exactly
/// those four bytes after the header (`instance_load_head(0x3F, 0x3F)`).
pub const INSTANCE_LOAD_HEAD: u16 = 0x017C;

/// `GAME_SMSG_INSTANCE_PLAYER_DATA_START`: the player-data stream opens.
pub const INSTANCE_PLAYER_DATA_START: u16 = 0x0186;

/// `GAME_SMSG_INSTANCE_PLAYER_DATA_DONE`: the player-data stream is over.
///
/// Two bytes, the header only. Its only caller is the character-creation flow - a map load never
/// carries it - which is why it sits beside the creation start here.
pub const INSTANCE_PLAYER_DATA_DONE: u16 = 0x018A;

/// `GAME_SMSG_CHAR_CREATION_START`: the creation instance may show its screen.
///
/// Evidence: the reference's own send log; two bytes, the header only. A creation connection is
/// answered with the load head and this, and nothing else - the map's own block (player name, load
/// info, the agent) belongs to a map that has geometry, and the creation instance has none.
pub const CHAR_CREATION_START: u16 = 0x0189;

/// A frame: the `u16` header and then the body.
fn frame(header: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// `0x017C`: the reference's own flags - both unlock bits set, then two zero bytes.
pub fn instance_load_head() -> Vec<u8> {
    frame(INSTANCE_LOAD_HEAD, &[0x3F, 0x3F, 0x00, 0x00])
}

/// `0x0189`: the header and nothing else.
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

/// What a character-creation connection is answered with, in the reference's order.
pub fn creation_load_block() -> Vec<u8> {
    let mut block = instance_load_head();
    block.extend_from_slice(&char_creation_start());
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_load_head_is_the_traced_six_bytes() {
        // `7C 01 3F 3F 00 00` - the reference's own send log, one line per message.
        assert_eq!(instance_load_head(), vec![0x7C, 0x01, 0x3F, 0x3F, 0x00, 0x00]);
    }

    #[test]
    fn the_creation_start_is_the_header_alone() {
        assert_eq!(char_creation_start(), vec![0x89, 0x01]);
    }

    #[test]
    fn the_creation_block_is_the_two_messages_in_that_order() {
        assert_eq!(
            creation_load_block(),
            vec![0x7C, 0x01, 0x3F, 0x3F, 0x00, 0x00, 0x89, 0x01]
        );
    }
}
