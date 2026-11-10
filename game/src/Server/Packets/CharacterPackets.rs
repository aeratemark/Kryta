//! The character message family: a character's profession, skills, skillbar and faction standing.
//!
//! Reference: `src/server/game/Server/Packets/CharacterPackets.{h,cpp}`. The creation screen is the
//! first caller: a character being made has a profession, a skillbar of nothing, and an empty
//! faction record, and the client wants all four before it draws the screen.
//!
//! Every layout is this client's. The source is
//! `ASURA/recordings/client/feature/character_creation/character_creation-live-necro_factions.jsonl`
//! - a live client and the server it played on - and each builder names its frame.

use crate::server::protocol::frame;

/// `GAME_SMSG_PLAYER_UPDATE_PROFESSION`: the primary and secondary profession of an agent.
pub const PLAYER_UPDATE_PROFESSION: u16 = 0x00B7;
/// `GAME_SMSG_PLAYER_UPDATE_UNLOCKED_PROFESSIONS`: which professions the account may choose.
pub const PLAYER_UPDATE_UNLOCKED_PROFESSIONS: u16 = 0x00B6;
/// `GAME_SMSG_UPDATE_UNLOCKED_SKILLS`: an agent's unlocked-skill bitmap.
pub const UPDATE_UNLOCKED_SKILLS: u16 = 0x00DB;
/// `GAME_SMSG_SKILLBAR_UPDATE`: the eight slots of the skillbar.
pub const SKILLBAR_UPDATE: u16 = 0x00DA;
/// `GAME_SMSG_CHARACTER_UPDATE_FACTIONS`: a character's progress record.
pub const CHARACTER_UPDATE_FACTIONS: u16 = 0x00E9;
/// `GAME_SMSG_CHAR_CREATION_SUCCESS`: the character was written.
pub const CHAR_CREATION_SUCCESS: u16 = 0x0188;
/// `GAME_SMSG_CHAR_CREATION_ERROR`: it was not, and the code says why.
pub const CHAR_CREATION_ERROR: u16 = 0x018B;
/// The error code a taken name is answered with.
///
/// Evidence: `ASURA`'s `GM_ERROR_CHARACTER_NAME_ALREADY_EXIST`, read from the reference's
/// `GmError.h` (`GameSrv.c:1939`).
pub const GM_ERROR_CHARACTER_NAME_ALREADY_EXIST: u32 = 0x0106;

/// The profession a fresh creation screen starts on: Warrior.
///
/// Evidence: `ASURA`'s `DEFAULT_PRIMARY_PROFESSION`, recorded from the reference's
/// `HandleCharCreationRequestPlayer`, which sets Warrior and nothing else until the client says so.
pub const DEFAULT_PROFESSION: u8 = 1;

/// The unlocked-profession bitmap the creation screen is told before any character exists.
///
/// Evidence: ASURA's `config/Server/defaults.json`, `creation_screen_unlocked_professions: 2` - bit
/// 1, Warrior. The reference hardcodes the same value.
pub const CREATION_UNLOCKED_PROFESSIONS: u32 = 2;

/// How many dwords the unlocked-skill message carries: the whole bitmap, counted by nothing.
///
/// Evidence: the recording's `0x00DB` is 260 bytes - the four-byte header and 256 bytes - so it is
/// sixty-four dwords with no count in front of them (a count of zero would leave the rest
/// unaccounted for).
pub const UNLOCKED_SKILLS_DWORDS: usize = 64;

/// How many dwords a character's faction record carries.
///
/// Evidence: the recording's `0x00E9` is 64 bytes: the header and fifteen dwords.
pub const FACTION_RECORD_DWORDS: usize = 15;

/// How many slots a skillbar has.
pub const SKILLBAR_SLOTS: usize = 8;

/// The 26 bytes a creation confirm is answered with after the character's id and name.
///
/// Evidence: the recording's one `0x0188`, seq 464 - 64 bytes whose tail this is, with the name and
/// uuid before it. The fields are not named in any source this tree has, so they are reproduced as
/// measured rather than invented: a message the client accepted once is a better payload than one
/// guessed at.
pub const CREATION_SUCCESS_TAIL: [u8; 26] = [
    0x00, 0x00, 0x9C, 0x22, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xE7, 0x54, 0x00, 0x00, 0x5A, 0x8C,
    0xBC, 0xDC, 0x12, 0x6F, 0x00, 0x00, 0xA0, 0x01, 0x00, 0x00,
];

/// A `u16` count followed by that many dwords.
fn dwords(values: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + values.len() * 4);
    out.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// `0x00B7`: the agent, the primary profession, the secondary, and the PvP flag - four dwords,
/// twenty bytes.
///
/// Evidence: seq 319 - `b7 00 00 00 1c 00 00 00 01 00 00 00 00 00 00 00 00 00 00 00`.
pub fn player_update_profession(agent: u32, primary: u32, secondary: u32, is_pvp: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    for value in [agent, primary, secondary, is_pvp] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    frame(PLAYER_UPDATE_PROFESSION, &body)
}

/// `0x00B6`: the agent and the unlocked-profession bitmap.
///
/// Evidence: seq 1050 - `b6 00 00 00 09 00 00 00 00 00 00 00`, twelve bytes.
pub fn player_update_unlocked_professions(agent: u32, unlocked: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&unlocked.to_le_bytes());
    frame(PLAYER_UPDATE_UNLOCKED_PROFESSIONS, &body)
}

/// `0x00DB`: the agent's unlocked-skill bitmap, sixty-four dwords with no count in front of them.
///
/// Evidence: seq 270 and its four siblings, 260 bytes each.
pub fn update_unlocked_skills(bitmap: &[u32]) -> Vec<u8> {
    let mut body = vec![0u8; UNLOCKED_SKILLS_DWORDS * 4];
    for (index, value) in bitmap.iter().take(UNLOCKED_SKILLS_DWORDS).enumerate() {
        body[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    frame(UPDATE_UNLOCKED_SKILLS, &body)
}

/// `0x00DA`: the agent, a counted list of eight slots, and a trailing word - forty-four bytes.
///
/// Evidence: seq 1051 - `da 00 00 00 09 00 00 00 08 00 00 00` and thirty-two zero bytes: the agent,
/// the count `8`, eight slot dwords, and two bytes after them.
pub fn skillbar_update(agent: u32, slots: &[u32]) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + 2 + SKILLBAR_SLOTS * 4 + 2);
    body.extend_from_slice(&agent.to_le_bytes());
    let mut eight = vec![0u32; SKILLBAR_SLOTS];
    for (index, value) in slots.iter().take(SKILLBAR_SLOTS).enumerate() {
        eight[index] = *value;
    }
    body.extend_from_slice(&dwords(&eight));
    body.extend_from_slice(&0u16.to_le_bytes());
    frame(SKILLBAR_UPDATE, &body)
}

/// `0x00E9`: a character's progress record - fifteen dwords, all zero for a character that is being
/// made.
///
/// Evidence: seq 1058, 64 bytes: the header and fifteen dwords.
pub fn character_update_factions(progress: &[u32; FACTION_RECORD_DWORDS]) -> Vec<u8> {
    let mut body = Vec::with_capacity(FACTION_RECORD_DWORDS * 4);
    for value in progress {
        body.extend_from_slice(&value.to_le_bytes());
    }
    frame(CHARACTER_UPDATE_FACTIONS, &body)
}

/// `0x0188`: the character was written - its id, its name, and the twenty-six bytes the answer
/// carries after them.
///
/// Evidence: seq 464, 64 bytes for a name of eight units: the header, the sixteen-byte id, a counted
/// name (`08 00` and sixteen bytes), and `CREATION_SUCCESS_TAIL`.
pub fn char_creation_success(character: [u8; 16], name: &str) -> Vec<u8> {
    let units: Vec<u16> = name.encode_utf16().take(20).collect();
    let mut body = Vec::with_capacity(16 + 2 + units.len() * 2 + CREATION_SUCCESS_TAIL.len());
    body.extend_from_slice(&character);
    body.extend_from_slice(&(units.len() as u16).to_le_bytes());
    for unit in &units {
        body.extend_from_slice(&unit.to_le_bytes());
    }
    body.extend_from_slice(&CREATION_SUCCESS_TAIL);
    frame(CHAR_CREATION_SUCCESS, &body)
}

/// `0x018B`: the creation was refused with this code - eight bytes.
pub fn char_creation_error(code: u32) -> Vec<u8> {
    frame(CHAR_CREATION_ERROR, &code.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profession_is_the_agent_and_three_dwords() {
        // The recording's own frame, byte for byte.
        assert_eq!(
            player_update_profession(0x1C, 1, 0, 0),
            vec![
                0xB7, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ]
        );
    }

    #[test]
    fn the_unlocked_professions_are_the_agent_and_a_bitmap() {
        assert_eq!(
            player_update_unlocked_professions(9, 0),
            vec![0xB6, 0x00, 0x00, 0x00, 9, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn the_unlocked_skills_are_sixty_four_dwords_and_no_count() {
        let frame = update_unlocked_skills(&[]);
        assert_eq!(frame.len(), 260, "the recording's own size");
        assert_eq!(&frame[..4], &[0xDB, 0x00, 0x00, 0x00]);
        assert_eq!(&frame[4..8], &0u32.to_le_bytes(), "the first dword, not a count");
    }

    #[test]
    fn the_skillbar_is_eight_slots_and_a_trailing_word() {
        let frame = skillbar_update(9, &[]);
        assert_eq!(frame.len(), 44, "the recording's own size");
        assert_eq!(&frame[8..10], &8u16.to_le_bytes(), "eight slots");
    }

    #[test]
    fn the_faction_record_is_fifteen_dwords() {
        let frame = character_update_factions(&[0u32; FACTION_RECORD_DWORDS]);
        assert_eq!(frame.len(), 64, "the recording's own size");
    }

    #[test]
    fn the_success_answer_is_sixty_four_bytes_for_an_eight_unit_name() {
        let frame = char_creation_success([0x11; 16], "Tester X");
        assert_eq!(frame.len(), 64, "the recording's own 64 bytes");
        assert_eq!(&frame[..4], &[0x88, 0x01, 0x00, 0x00]);
        assert_eq!(&frame[4..20], &[0x11; 16], "the character's id");
        assert_eq!(&frame[20..22], &8u16.to_le_bytes(), "the name's count");
        assert_eq!(&frame[38..64], &CREATION_SUCCESS_TAIL, "the measured tail");
    }

    #[test]
    fn a_refusal_is_the_header_and_a_code() {
        assert_eq!(
            char_creation_error(GM_ERROR_CHARACTER_NAME_ALREADY_EXIST),
            vec![0x8B, 0x01, 0x00, 0x00, 0x06, 0x01, 0x00, 0x00]
        );
    }
}
