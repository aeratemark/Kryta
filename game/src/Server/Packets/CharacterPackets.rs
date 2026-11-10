//! The character message family: a character's profession, skills, skillbar and faction standing.
//!
//! Reference: `src/server/game/Server/Packets/CharacterPackets.{h,cpp}`. The creation screen is the
//! first caller: a character being made has a profession, a skillbar of nothing, and an empty
//! faction record, and the client wants all four before it draws the screen.
//!
//! Every header and layout here is this client's, measured from a working server; each one names
//! where its bytes came from.

/// `GAME_SMSG_PLAYER_UPDATE_PROFESSION`: the primary and secondary profession of an agent.
pub const PLAYER_UPDATE_PROFESSION: u16 = 0x00B7;
/// `GAME_SMSG_PLAYER_UPDATE_UNLOCKED_PROFESSIONS`: which professions the account may choose.
pub const PLAYER_UPDATE_UNLOCKED_PROFESSIONS: u16 = 0x00B6;
/// `GAME_SMSG_UPDATE_UNLOCKED_SKILLS`: a counted list of unlocked skills.
pub const UPDATE_UNLOCKED_SKILLS: u16 = 0x00DB;
/// `GAME_SMSG_SKILLBAR_UPDATE`: the eight slots of the skillbar and their PvP masks.
pub const SKILLBAR_UPDATE: u16 = 0x00DA;
/// `GAME_SMSG_CHARACTER_UPDATE_FACTIONS`: a character's progress record.
pub const CHARACTER_UPDATE_FACTIONS: u16 = 0x00E9;
/// The cap on the Kurzick faction.
pub const FACTION_MAX_KURZICK: u16 = 0x00EA;
/// The cap on the Luxon faction.
pub const FACTION_MAX_LUXON: u16 = 0x00EB;
/// The cap on the Balthazar faction.
pub const FACTION_MAX_BALTHAZAR: u16 = 0x00EC;
/// The cap on the Imperial faction.
pub const FACTION_MAX_IMPERIAL: u16 = 0x00ED;

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

/// A frame: the `u16` header and then the body.
fn frame(header: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A `u16` count followed by that many dwords.
fn dwords(values: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + values.len() * 4);
    out.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// `0x00B7`: the agent, then primary, secondary and the PvP flag.
pub fn player_update_profession(agent: u32, primary: u8, secondary: u8, is_pvp: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(7);
    body.extend_from_slice(&agent.to_le_bytes());
    body.push(primary);
    body.push(secondary);
    body.push(is_pvp);
    frame(PLAYER_UPDATE_PROFESSION, &body)
}

/// `0x00B6`: the agent and the unlocked-profession bitmap.
pub fn player_update_unlocked_professions(agent: u32, unlocked: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&unlocked.to_le_bytes());
    frame(PLAYER_UPDATE_UNLOCKED_PROFESSIONS, &body)
}

/// `0x00DB`: a counted list of skill ids.
pub fn update_unlocked_skills(skills: &[u32]) -> Vec<u8> {
    frame(UPDATE_UNLOCKED_SKILLS, &dwords(skills))
}

/// `0x00DA`: the agent, eight slots, eight PvP masks, one trailing byte.
pub fn skillbar_update(agent: u32, skills: &[u32], pvp_masks: &[u32], unknown: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(9 + 2 + 2 + skills.len() * 4 + pvp_masks.len() * 4);
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&dwords(skills));
    body.extend_from_slice(&dwords(pvp_masks));
    body.push(unknown);
    frame(SKILLBAR_UPDATE, &body)
}

/// `0x00E9`: a character's progress record - thirteen dwords, all zero for a character that is
/// being made.
pub fn character_update_factions(progress: &[u32; 13]) -> Vec<u8> {
    let mut body = Vec::with_capacity(progress.len() * 4);
    for value in progress {
        body.extend_from_slice(&value.to_le_bytes());
    }
    frame(CHARACTER_UPDATE_FACTIONS, &body)
}

/// One of the four faction caps: the header names the faction, then the cap.
pub fn faction_max(header: u16, cap: u32) -> Vec<u8> {
    frame(header, &cap.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profession_is_the_agent_and_three_bytes() {
        assert_eq!(
            player_update_profession(1, DEFAULT_PROFESSION, 0, 0),
            vec![0xB7, 0x00, 1, 0, 0, 0, 1, 0, 0]
        );
    }

    #[test]
    fn the_unlocked_professions_are_the_agent_and_a_bitmap() {
        assert_eq!(
            player_update_unlocked_professions(1, CREATION_UNLOCKED_PROFESSIONS),
            vec![0xB6, 0x00, 1, 0, 0, 0, 2, 0, 0, 0]
        );
    }

    #[test]
    fn an_empty_skill_list_is_a_zero_count() {
        assert_eq!(update_unlocked_skills(&[]), vec![0xDB, 0x00, 0, 0]);
    }

    #[test]
    fn the_skillbar_is_two_counted_lists_and_a_byte() {
        let frame = skillbar_update(1, &[0u32; 8], &[0u32; 8], 1);
        // header 2, agent 4, count 2 + eight slots, count 2 + eight masks, one byte.
        assert_eq!(frame.len(), 2 + 4 + 34 + 34 + 1);
        assert_eq!(&frame[6..8], &8u16.to_le_bytes(), "eight slots");
        assert_eq!(&frame[40..42], &8u16.to_le_bytes(), "eight masks");
        assert_eq!(*frame.last().unwrap(), 1, "the trailing byte");
    }

    #[test]
    fn the_faction_record_is_thirteen_dwords() {
        let frame = character_update_factions(&[0u32; 13]);
        assert_eq!(frame.len(), 2 + 13 * 4);
    }

    #[test]
    fn a_faction_cap_is_its_header_and_one_dword() {
        assert_eq!(
            faction_max(FACTION_MAX_KURZICK, 10_000),
            vec![0xEA, 0x00, 0x10, 0x27, 0, 0]
        );
    }
}
