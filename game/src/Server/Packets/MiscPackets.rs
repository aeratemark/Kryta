//! The misc family: the small messages that belong to no larger subject.
//!
//! Reference: `src/server/game/Server/Packets/MiscPackets.{h,cpp}`, which is where the reference
//! keeps exactly this kind of message. Here that is the PvP halves of the creation screen - the
//! unlocked-skill list, the unlocked-hero bitmap and the end of the PvP item stream - because a
//! creation account has unlocked nothing and an empty list is what says so.

/// `GAME_SMSG_PVP_UPDATE_UNLOCKED_SKILLS`: a counted list of unlocked PvP skills.
pub const PVP_UPDATE_UNLOCKED_SKILLS: u16 = 0x001D;
/// `GAME_SMSG_PVP_UPDATE_UNLOCKED_HEROES`: a counted bitmap of unlocked PvP heroes.
pub const PVP_UPDATE_UNLOCKED_HEROES: u16 = 0x0018;
/// `GAME_SMSG_PVP_ITEM_STREAM_END`: the PvP item stream is over.
pub const PVP_ITEM_STREAM_END: u16 = 0x001B;

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

/// `0x001D`: a counted list of unlocked skills.
pub fn pvp_update_unlocked_skills(skills: &[u32]) -> Vec<u8> {
    frame(PVP_UPDATE_UNLOCKED_SKILLS, &dwords(skills))
}

/// `0x0018`: a counted bitmap of unlocked heroes.
pub fn pvp_update_unlocked_heroes(bitmap: &[u32]) -> Vec<u8> {
    frame(PVP_UPDATE_UNLOCKED_HEROES, &dwords(bitmap))
}

/// `0x001B`: the header alone, which ends the stream.
pub fn pvp_item_stream_end() -> Vec<u8> {
    frame(PVP_ITEM_STREAM_END, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_pvp_half_is_a_header_and_a_zero_count() {
        assert_eq!(pvp_update_unlocked_skills(&[]), vec![0x1D, 0x00, 0, 0]);
        assert_eq!(pvp_update_unlocked_heroes(&[]), vec![0x18, 0x00, 0, 0]);
        assert_eq!(pvp_item_stream_end(), vec![0x1B, 0x00]);
    }
}
