//! The agent message family: an agent's creation, its properties and its attributes.
//!
//! Reference: `src/server/game/Server/Packets/MiscPackets` and the agent handlers in
//! `PlayerHandler`/`WorldSession`. A body in this game is an *agent* - the client's word for
//! anything that can be seen, a player's own included - so a player's health and energy arrive as
//! agent properties. One file per family, as this tree does it.
//!
//! Every layout here is this client's, measured; each names where its bytes came from. The creation
//! frame (`GAME_SMSG_WORLD_CREATE_AGENT`, 99 bytes) is ported from the reference's own builder as
//! `ASURA` transcribes it; the map's content - which agents a map has - arrives with plan step 6.

/// `GAME_SMSG_AGENT_PROPERTY_UPDATE_INT`: one integer property of one agent.
pub const AGENT_PROPERTY_UPDATE_INT: u16 = 0x009F;
/// `GAME_SMSG_AGENT_UPDATE_ATTRIBUTE_POINTS`: attribute points spent and unspent.
pub const AGENT_UPDATE_ATTRIBUTE_POINTS: u16 = 0x0037;
/// `GAME_SMSG_AGENT_UPDATE_ATTRIBUTES`: the agent's attribute ranks, one dword each.
pub const AGENT_UPDATE_ATTRIBUTES: u16 = 0x003A;
/// `GAME_SMSG_WORLD_CREATE_AGENT` (`0x0020`): one agent created, 99 bytes fixed.
pub const WORLD_CREATE_AGENT: u16 = 0x0020;
/// `GAME_SMSG_AGENT_PROPERTY_UPDATE_FLOAT` (`0x00A2`): one float property of one agent.
pub const AGENT_PROPERTY_UPDATE_FLOAT: u16 = 0x00A2;
/// `GAME_SMSG_AGENT_UPDATE_MORALE` (`0x009C`): one agent's morale as a percentage.
pub const AGENT_UPDATE_MORALE: u16 = 0x009C;
/// `GAME_SMSG_AGENT_INITIAL_EFFECTS` (`0x00F0`): the effect bitmask an agent starts with.
pub const AGENT_INITIAL_EFFECTS: u16 = 0x00F0;

/// The property an agent's energy is reported through.
///
/// Evidence: `ASURA`'s `agent.rs`, which records the reference's own numbers: energy 41, health 42,
/// and 64 for the third property the creation screen is sent.
pub const AGENT_PROPERTY_ENERGY: u32 = 41;
/// The property an agent's health is reported through.
pub const AGENT_PROPERTY_HEALTH: u32 = 42;
/// The third property the creation screen is sent, always zero there.
pub const AGENT_PROPERTY_VALUE64: u32 = 64;

/// One agent, in the field order `GAME_SMSG_WORLD_CREATE_AGENT` writes it.
///
/// Source: `ASURA`'s `crates/asura-protocol/src/game/mod.rs`, its `CreateAgent` struct and
/// `world_create_agent` builder, which transcribe the reference's `GameSrv_BuildCreateAgentMsg`
/// field for field. The one field that differs between a player and an item on the ground is
/// `agent_type`; the rest is what the reference fills from its `GmAgent`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreateAgent {
    /// The id the rest of the stream refers to this body by.
    pub agent_id: u32,
    /// What the client draws: a character class word for a player, an item model for a drop.
    pub model_id: u32,
    /// `1` for a living body, `4` for a dropped item.
    pub agent_type: u8,
    /// Where the body stands, in the map's own units.
    pub position: (f32, f32),
    /// The pathing plane, not a height.
    pub plane: u16,
    /// Which way it faces, as a unit vector.
    pub direction: (f32, f32),
    /// Walking speed the client uses until a `0x002B` says otherwise.
    pub speed_base: f32,
    /// The team the body belongs to; 0 for a neutral body.
    pub player_team_token: u32,
}

/// `0x0020`: one agent created, 99 bytes fixed - the frame that puts a body in a map.
///
/// The field order and the five constants are the reference's own (`GameSrv_BuildCreateAgentMsg` as
/// ASURA transcribes it): the byte after the type is 5, the byte after the direction is 1, the float
/// after the speed is 1.0, and the dword after it is `0x41400000` - "12.0 reinterpreted", as the
/// reference writes it. The two `f32::INFINITY` pairs are what it sends for the unmodelled vectors.
/// They are carried as measured rather than reasoned about.
pub fn world_create_agent(agent: &CreateAgent) -> Vec<u8> {
    let mut body = Vec::with_capacity(97);
    body.extend_from_slice(&agent.agent_id.to_le_bytes());
    body.extend_from_slice(&agent.model_id.to_le_bytes());
    body.push(agent.agent_type);
    body.push(5); // h000B
    body.extend_from_slice(&agent.position.0.to_le_bytes());
    body.extend_from_slice(&agent.position.1.to_le_bytes());
    body.extend_from_slice(&agent.plane.to_le_bytes());
    body.extend_from_slice(&agent.direction.0.to_le_bytes());
    body.extend_from_slice(&agent.direction.1.to_le_bytes());
    body.push(1); // h001E
    body.extend_from_slice(&agent.speed_base.to_le_bytes());
    body.extend_from_slice(&1.0f32.to_le_bytes()); // h0023
    body.extend_from_slice(&0x4140_0000u32.to_le_bytes()); // h0027
    body.extend_from_slice(&agent.player_team_token.to_le_bytes());
    for _ in 0..5 {
        body.extend_from_slice(&0u32.to_le_bytes()); // h002F..h003F
    }
    body.extend_from_slice(&0f32.to_le_bytes()); // h0043.x
    body.extend_from_slice(&0f32.to_le_bytes()); // h0043.y
    body.extend_from_slice(&f32::INFINITY.to_le_bytes()); // h004B.x
    body.extend_from_slice(&f32::INFINITY.to_le_bytes()); // h004B.y
    body.extend_from_slice(&0u16.to_le_bytes()); // h0053
    body.extend_from_slice(&0u32.to_le_bytes()); // h0055
    body.extend_from_slice(&f32::INFINITY.to_le_bytes()); // h0059.x
    body.extend_from_slice(&f32::INFINITY.to_le_bytes()); // h0059.y
    body.extend_from_slice(&0u16.to_le_bytes()); // h0061
    frame(WORLD_CREATE_AGENT, &body)
}

/// A frame: the `u16` header and then the body.
fn frame(header: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A `u16` count followed by that many dwords - the counted field a list is sent as.
fn dwords(values: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + values.len() * 4);
    out.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// `0x009F`: property id, agent id, value - three dwords, fourteen bytes.
pub fn agent_property_update_int(property: u32, agent: u32, value: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&property.to_le_bytes());
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&value.to_le_bytes());
    frame(AGENT_PROPERTY_UPDATE_INT, &body)
}

/// `0x0037`: the agent, then the unspent and the spent points, one byte each.
pub fn agent_update_attribute_points(agent: u32, unused: u8, used: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(6);
    body.extend_from_slice(&agent.to_le_bytes());
    body.push(unused);
    body.push(used);
    frame(AGENT_UPDATE_ATTRIBUTE_POINTS, &body)
}

/// `0x003A`: the agent, then a counted list of its attribute ranks.
pub fn agent_update_attributes(agent: u32, attributes: &[u32]) -> Vec<u8> {
    let mut body = Vec::with_capacity(4 + 2 + attributes.len() * 4);
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&dwords(attributes));
    frame(AGENT_UPDATE_ATTRIBUTES, &body)
}

/// `0x00A2`: one float property of an agent - the property, the agent, the value.
///
/// Source: the game table's row (`GAME_SMSG_AGENT_PROPERTY_UPDATE_FLOAT`, 14 bytes) and `ASURA`'s
/// `agent_property_update_float` builder, field for field.
pub fn agent_property_update_float(property: u32, agent: u32, value: f32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&property.to_le_bytes());
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&value.to_le_bytes());
    frame(AGENT_PROPERTY_UPDATE_FLOAT, &body)
}

/// `0x009C`: one agent's morale as a percentage, 100 being the unmodified value.
///
/// Source: `ASURA`'s `agent_update_morale` builder and the trace it cites - a live server sends one
/// per body at the load (`9C 00 | 1F 00 00 00 | 64 00 00 00`, agent 0x1F at 100).
pub fn agent_update_morale(agent: u32, morale: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&morale.to_le_bytes());
    frame(AGENT_UPDATE_MORALE, &body)
}

/// `0x00F0`: the effect bitmask an agent starts with.
///
/// Source: the game table's row (`GAME_SMSG_AGENT_INITIAL_EFFECTS`, 10 bytes) and `ASURA`'s
/// `agent_initial_effects` builder: the agent and the effects, two dwords.
pub fn agent_initial_effects(agent: u32, effects: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&agent.to_le_bytes());
    body.extend_from_slice(&effects.to_le_bytes());
    frame(AGENT_INITIAL_EFFECTS, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_property_is_three_dwords_in_the_measured_order() {
        // `property`, `agent`, `value` - the order the reference's own builder writes them.
        assert_eq!(
            agent_property_update_int(AGENT_PROPERTY_HEALTH, 1, 100),
            vec![0x9F, 0x00, 42, 0, 0, 0, 1, 0, 0, 0, 100, 0, 0, 0]
        );
    }

    #[test]
    fn the_attribute_points_are_two_bytes_after_the_agent() {
        assert_eq!(
            agent_update_attribute_points(1, 50, 50),
            vec![0x37, 0x00, 1, 0, 0, 0, 50, 50]
        );
    }

    #[test]
    fn the_attributes_are_a_counted_list() {
        let frame = agent_update_attributes(1, &[0u32; 42]);
        assert_eq!(frame.len(), 2 + 4 + 2 + 42 * 4);
        assert_eq!(&frame[..6], &[0x3A, 0x00, 1, 0, 0, 0]);
        assert_eq!(&frame[6..8], &42u16.to_le_bytes());
    }

    #[test]
    fn a_float_property_is_the_property_the_agent_and_the_value() {
        let frame = agent_property_update_float(0x2B, 1, 0.5);
        assert_eq!(frame.len(), 14, "the table's own size");
        assert_eq!(&frame[..2], &[0xA2, 0x00]);
        assert_eq!(&frame[2..6], &0x2Bu32.to_le_bytes(), "the property");
        assert_eq!(&frame[6..10], &1u32.to_le_bytes(), "the agent");
        assert_eq!(&frame[10..14], &0.5f32.to_le_bytes());
    }

    #[test]
    fn a_morale_is_the_agent_and_its_percentage() {
        // The trace's own frame: `9C 00 | 1F 00 00 00 | 64 00 00 00` - agent 0x1F, 100.
        assert_eq!(
            agent_update_morale(0x1F, 100),
            vec![0x9C, 0x00, 0x1F, 0x00, 0x00, 0x00, 100, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn the_initial_effects_are_the_agent_and_the_bitmask() {
        let frame = agent_initial_effects(2, 0x400);
        assert_eq!(frame.len(), 10, "the table's own size");
        assert_eq!(&frame[..2], &[0xF0, 0x00]);
        assert_eq!(&frame[2..6], &2u32.to_le_bytes(), "the agent");
        assert_eq!(&frame[6..10], &0x400u32.to_le_bytes(), "the effects");
    }

    #[test]
    fn an_agent_is_the_reference_s_ninety_nine_bytes() {
        // The reference's `GameSrv_BuildCreateAgentMsg` order, constant by constant: the id and the
        // model, the type and the 5, the position, the plane, the direction, the 1 and the speed,
        // the 1.0, the 0x41400000, the team, five zero dwords, the two zero floats, the infinite
        // vectors, and the closing words.
        let agent = CreateAgent {
            agent_id: 7,
            model_id: 0x2710,
            agent_type: 1,
            position: (9541.87, 4796.46),
            plane: 0,
            direction: (0.0, 1.0),
            speed_base: 288.0,
            player_team_token: 2,
        };
        let frame = world_create_agent(&agent);
        assert_eq!(frame.len(), 99, "the table's own size");
        assert_eq!(&frame[..2], &[0x20, 0x00]);
        assert_eq!(&frame[2..6], &7u32.to_le_bytes(), "the agent");
        assert_eq!(&frame[6..10], &0x2710u32.to_le_bytes(), "the model");
        assert_eq!(frame[10], 1, "the type");
        assert_eq!(frame[11], 5, "h000B");
        assert_eq!(&frame[12..16], &9541.87f32.to_le_bytes());
        assert_eq!(&frame[16..20], &4796.46f32.to_le_bytes());
        assert_eq!(&frame[20..22], &0u16.to_le_bytes(), "the plane");
        assert_eq!(&frame[22..26], &0f32.to_le_bytes());
        assert_eq!(&frame[26..30], &1f32.to_le_bytes());
        assert_eq!(frame[30], 1, "h001E");
        assert_eq!(&frame[31..35], &288f32.to_le_bytes(), "the walking speed");
        assert_eq!(&frame[35..39], &1f32.to_le_bytes(), "h0023");
        assert_eq!(&frame[39..43], &0x4140_0000u32.to_le_bytes(), "h0027");
        assert_eq!(&frame[43..47], &2u32.to_le_bytes(), "the team");
        for offset in [47usize, 51, 55, 59, 63] {
            assert_eq!(&frame[offset..offset + 4], &0u32.to_le_bytes(), "h002F..h003F");
        }
        for offset in [67usize, 71] {
            assert_eq!(&frame[offset..offset + 4], &0f32.to_le_bytes());
        }
        for offset in [75usize, 79, 89, 93] {
            assert_eq!(f32::from_le_bytes(frame[offset..offset + 4].try_into().unwrap()), f32::INFINITY);
        }
        assert_eq!(&frame[83..85], &0u16.to_le_bytes());
        assert_eq!(&frame[85..89], &0u32.to_le_bytes());
        assert_eq!(&frame[97..99], &0u16.to_le_bytes());
    }
}
