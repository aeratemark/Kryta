//! The party message family: the party a character is in, and who is in it.
//!
//! Reference: `src/server/game/Server/Packets/GroupPackets.{h,cpp}` - one file pair per message family,
//! and this tree's noun for `Groups` is `Parties`. Nothing of its *contents* crosses: these headers are
//! this client's.
//!
//! The family and its order are measured twice over, and both agree:
//!
//! * the reference's own capture of a visit to another district
//!   (`recordings/client/feature/travel/doors-20261031-095754.jsonl`, seq 319-322, and the same four
//!   again at 756-759, 2832-2835 and every other entry): `0x01D3` with the party id, `0x01CC` with the
//!   party, the player and `1`, `0x01D4` with the party id, then `0x01B3` with the party id and `1`.
//!   A second entry carries party 3 where the first carries party 1, so the dwords are the party, the
//!   player and a loaded byte in that order;
//! * the working implementation's builders, which send the same four before the load's last message
//!   (`ASURA/crates/asura-world/src/player.rs:382-388`, `ASURA/crates/asura-protocol/src/game/mod.rs`,
//!   `party_create`, `party_player_add`, `party_member_stream_end`).
//!
//! The order is not decoration: the client's party object is what `0x01B3` talks about, and a client
//! told about a party it was never given asserts (`Assertion: m_partyClient`,
//! `P:\Code\Gw\Party\Cli\PyCliParty.cpp(650)`, measured 2026-11-12 on the load of map 148).

/// `GAME_SMSG_PARTY_CREATE` (`0x01D3`): 4 bytes - the header and the party's id.
pub const PARTY_CREATE: u16 = 0x01D3;
/// `GAME_SMSG_PARTY_PLAYER_ADD` (`0x01CC`): 7 bytes - the party, the player, and whether it is loaded.
pub const PARTY_PLAYER_ADD: u16 = 0x01CC;
/// `GAME_SMSG_PARTY_MEMBER_STREAM_END` (`0x01D4`): 4 bytes - the header and the party's id.
pub const PARTY_MEMBER_STREAM_END: u16 = 0x01D4;
/// `GAME_SMSG_PLAYER_UPDATE_PARTY_SIZE` (`0x00B0`): a player's party size.
pub const PLAYER_UPDATE_PARTY_SIZE: u16 = 0x00B0;
/// `GAME_SMSG_PLAYER_UPDATE_PARTY` (`0x00B1`): the leader of a player's party.
pub const PLAYER_UPDATE_PARTY: u16 = 0x00B1;

/// The id a solo connection's party has.
///
/// Measured: every entry of the reference's capture but one names party `1`; the other names `3` for a
/// party of two, so the number is the party's own and `1` is what a fresh one gets.
/// `ASURA/crates/asura-world/src/player.rs:132` (`PARTY_ID`).
pub const PARTY_ID: u16 = 1;

/// A frame: the `u16` header and then its fields.
fn frame(header: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// `0x01D3`: the party exists.
pub fn party_create(party_id: u16) -> Vec<u8> {
    frame(PARTY_CREATE, &party_id.to_le_bytes())
}

/// `0x01CC`: one member of the party - which party, which player, and whether it is loaded into it.
pub fn party_player_add(party_id: u16, player_id: u16, is_loaded: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&party_id.to_le_bytes());
    body.extend_from_slice(&player_id.to_le_bytes());
    body.push(is_loaded);
    frame(PARTY_PLAYER_ADD, &body)
}

/// `GAME_SMSG_PLAYER_UPDATE_PARTY_SIZE` (`0x00B0`): twelve bytes - a four-byte game header, the player
/// and the size.
///
/// `ASURA/crates/asura-protocol/src/game/mod.rs:1327`, `player_update_party_size`: the table says five
/// bytes and a live capture carries twelve every time a map loads, so the two leading zero bytes are
/// written as body and the wire matches the capture.
pub fn player_update_party_size(player: u32, party_size: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(10);
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&player.to_le_bytes());
    body.extend_from_slice(&party_size.to_le_bytes());
    frame(PLAYER_UPDATE_PARTY_SIZE, &body)
}

/// `GAME_SMSG_PLAYER_UPDATE_PARTY` (`0x00B1`): which player leads the party this player is in -
/// captured as `b1 00 00 00 | 07 00 00 00 | 07 00 00 00`, player 7 leading its own party
/// (`ASURA/crates/asura-protocol/src/game/mod.rs:1341`).
pub fn player_update_party(player: u32, party_leader: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(10);
    body.extend_from_slice(&0u16.to_le_bytes());
    body.extend_from_slice(&player.to_le_bytes());
    body.extend_from_slice(&party_leader.to_le_bytes());
    frame(PLAYER_UPDATE_PARTY, &body)
}

/// `0x01D4`: the party's members have all been sent.
pub fn party_member_stream_end(party_id: u16) -> Vec<u8> {
    frame(PARTY_MEMBER_STREAM_END, &party_id.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_party_is_created_in_four_bytes() {
        assert_eq!(party_create(PARTY_ID), vec![0xD3, 0x01, 1, 0]);
    }

    #[test]
    fn a_member_is_the_party_the_player_and_one_byte() {
        assert_eq!(
            party_player_add(PARTY_ID, 1, 1),
            vec![0xCC, 0x01, 1, 0, 1, 0, 1]
        );
    }

    #[test]
    fn the_member_stream_ends_in_four_bytes() {
        assert_eq!(party_member_stream_end(PARTY_ID), vec![0xD4, 0x01, 1, 0]);
    }

    /// The reference's capture, as the wire: a four-byte header in the dump is a two-byte one here,
    /// and the dwords it holds become the word and the byte the frames carry.
    ///
    /// `doors-20261031-095754.jsonl` seq 319-322: `d3010000 01000000`, `cc010000 01000000 01000000
    /// 01000000`, `d4010000 01000000`, `b3010000 01000000 01000000`.
    #[test]
    fn the_family_is_the_one_the_capture_carries() {
        let header = |frame: &[u8]| u16::from_le_bytes([frame[0], frame[1]]);
        assert_eq!(header(&party_create(1)), 0x01D3);
        assert_eq!(header(&party_player_add(1, 1, 1)), 0x01CC);
        assert_eq!(header(&party_member_stream_end(1)), 0x01D4);
        assert_eq!(party_create(1).len(), 4);
        assert_eq!(party_player_add(1, 1, 1).len(), 7);
        assert_eq!(party_member_stream_end(1).len(), 4);
    }
}
