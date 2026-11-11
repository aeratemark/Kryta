//! The misc family: the small messages that belong to no larger subject.
//!
//! Reference: `src/server/game/Server/Packets/MiscPackets.{h,cpp}`, which is where the reference
//! keeps exactly this kind of message. Here that is the ping pair and the tick, the hard-mode flag,
//! the current-map notice - every one of them a message a working server sent on a load
//! (`recordings/gwps/session/login-travel-session.log`, `recordings/gwps/world/load-stream.log`) -
//! and the PvP halves of the creation screen, because a creation account has unlocked nothing and
//! an empty list is what says so.

/// `GAME_SMSG_PING_REQUEST` (`0x000C`): the header alone.
pub const PING_REQUEST: u16 = 0x000C;
/// `GAME_SMSG_PING_REPLY` (`0x000D`): one dword - the tick it answers.
pub const PING_REPLY: u16 = 0x000D;
/// `GAME_SMSG_WORLD_SIMULATION_TICK` (`0x001E`): one dword - the tick the simulation is on.
pub const WORLD_SIMULATION_TICK: u16 = 0x001E;
/// `GAME_SMSG_HARD_MODE_UNLOCKED` (`0x0070`): one byte.
pub const HARD_MODE_UNLOCKED: u16 = 0x0070;
/// `GAME_SMSG_MAP_UPDATE_CURRENT` (`0x0099`): the map, and one byte.
pub const MAP_UPDATE_CURRENT: u16 = 0x0099;
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

/// `0x000C`: the header alone. A load sends it once, unasked.
///
/// Source: `recordings/gwps/world/load-stream.log` - `SEND 0x000C 2 bytes : 0C 00`, right after the
/// load head block and before the item stream.
pub fn ping_request() -> Vec<u8> {
    frame(PING_REQUEST, &[])
}

/// `0x000D`: the dword the reference answers the ping with.
///
/// Source: the same trace - `SEND 0x000D 6 bytes : 0D 00 14 00 00 00` (20).
pub fn ping_reply(tick: u32) -> Vec<u8> {
    frame(PING_REPLY, &tick.to_le_bytes())
}

/// `0x001E`: the simulation tick. The reference sent 500 once the entry point was sent.
///
/// Source: `recordings/gwps/world/load-stream.log` - `SEND 0x001E 6 bytes : 1E 00 F4 01 00 00`.
pub fn world_simulation_tick(tick: u32) -> Vec<u8> {
    frame(WORLD_SIMULATION_TICK, &tick.to_le_bytes())
}

/// `0x0070`: the hard-mode flag, one byte (1 = unlocked).
///
/// Source: `recordings/gwps/world/load-stream.log` - `SEND 0x0070 3 bytes : 70 00 01`.
pub fn hard_mode_unlocked(unlocked: u8) -> Vec<u8> {
    frame(HARD_MODE_UNLOCKED, &[unlocked])
}

/// `0x0099`: the map the character is now in, and one byte (0 in every capture).
///
/// Source: `recordings/gwps/world/load-stream.log` - `SEND 0x0099 5 bytes : 99 00 92 00 00` (map 146).
pub fn map_update_current(map_id: u16, unknown: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(3);
    body.extend_from_slice(&map_id.to_le_bytes());
    body.push(unknown);
    frame(MAP_UPDATE_CURRENT, &body)
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

    #[test]
    fn the_ping_and_the_tick_are_the_recording_s_bytes() {
        assert_eq!(ping_request(), vec![0x0C, 0x00]);
        assert_eq!(ping_reply(20), vec![0x0D, 0x00, 0x14, 0x00, 0x00, 0x00]);
        assert_eq!(
            world_simulation_tick(500),
            vec![0x1E, 0x00, 0xF4, 0x01, 0x00, 0x00]
        );
        assert_eq!(hard_mode_unlocked(1), vec![0x70, 0x00, 0x01]);
        assert_eq!(map_update_current(146, 0), vec![0x99, 0x00, 0x92, 0x00, 0x00]);
    }
}
