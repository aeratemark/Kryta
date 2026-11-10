//! The agent message family: an agent's properties and attributes.
//!
//! Reference: `src/server/game/Server/Packets/MiscPackets` and the agent handlers in
//! `PlayerHandler`/`WorldSession`. A body in this game is an *agent* - the client's word for
//! anything that can be seen, a player's own included - so a player's health and energy arrive as
//! agent properties. One file per family, as this tree does it.
//!
//! Every layout here is this client's, measured; each names where its bytes came from.

/// `GAME_SMSG_AGENT_PROPERTY_UPDATE_INT`: one integer property of one agent.
pub const AGENT_PROPERTY_UPDATE_INT: u16 = 0x009F;
/// `GAME_SMSG_AGENT_UPDATE_ATTRIBUTE_POINTS`: attribute points spent and unspent.
pub const AGENT_UPDATE_ATTRIBUTE_POINTS: u16 = 0x0037;
/// `GAME_SMSG_AGENT_UPDATE_ATTRIBUTES`: the agent's attribute ranks, one dword each.
pub const AGENT_UPDATE_ATTRIBUTES: u16 = 0x003A;

/// The property an agent's energy is reported through.
///
/// Evidence: `ASURA`'s `agent.rs`, which records the reference's own numbers: energy 41, health 42,
/// and 64 for the third property the creation screen is sent.
pub const AGENT_PROPERTY_ENERGY: u32 = 41;
/// The property an agent's health is reported through.
pub const AGENT_PROPERTY_HEALTH: u32 = 42;
/// The third property the creation screen is sent, always zero there.
pub const AGENT_PROPERTY_VALUE64: u32 = 64;

use crate::server::protocol::frame;

/// A `u16` count followed by that many dwords - the counted field a list is sent as.
fn dwords(values: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + values.len() * 4);
    out.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// `0x009F`: property id, agent id, value - three dwords, sixteen bytes.
///
/// Evidence: ASURA/recordings/client/feature/character_creation/character_creation-live-necro_factions.jsonl''s
/// seq 668 - `9f 00 00 00 24 00 00 00 08 00 00 00 01 00 00 00`.
pub fn agent_property_update_int(property: u32, agent: u32, value: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&property.to_le_bytes());
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&value.to_le_bytes());
    frame(AGENT_PROPERTY_UPDATE_INT, &body)
}

/// `0x0037`: the agent, then the unspent and the spent points, one dword each - sixteen bytes.
///
/// Evidence: the same recording''s seq 1048 - `37 00 00 00 09 00 00 00 00 00 00 00 00 00 00 00`.
pub fn agent_update_attribute_points(agent: u32, unused: u32, used: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&unused.to_le_bytes());
    body.extend_from_slice(&used.to_le_bytes());
    frame(AGENT_UPDATE_ATTRIBUTE_POINTS, &body)
}

/// `0x003A`: the agent, then a counted list of its attribute ranks.
pub fn agent_update_attributes(agent: u32, attributes: &[u32]) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + 2 + attributes.len() * 4);
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&dwords(attributes));
    frame(AGENT_UPDATE_ATTRIBUTES, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_property_is_three_dwords_in_the_measured_order() {
        // `property`, `agent`, `value` - the order the reference's own builder writes them.
        assert_eq!(
            agent_property_update_int(AGENT_PROPERTY_HEALTH, 1, 100),
            vec![0x9F, 0x00, 0, 0, 42, 0, 0, 0, 1, 0, 0, 0, 100, 0, 0, 0]
        );
    }

    #[test]
    fn the_attribute_points_are_two_bytes_after_the_agent() {
        assert_eq!(
            agent_update_attribute_points(1, 50, 50),
            vec![0x37, 0x00, 0, 0, 1, 0, 0, 0, 50, 0, 0, 0, 50, 0, 0, 0]
        );
    }

    #[test]
    fn the_attributes_are_a_counted_list() {
        let frame = agent_update_attributes(1, &[0u32; 42]);
        assert_eq!(frame.len(), 4 + 4 + 2 + 42 * 4);
        assert_eq!(&frame[..8], &[0x3A, 0x00, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&frame[8..10], &42u16.to_le_bytes());
    }
}
