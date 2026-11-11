//! `game/Agents/` - the reference's `src/server/game/Entities/` (84 files).
//!
//! Renamed: the reference calls this directory `Entities/`.
//!
//! What is here now: the player's own agent - the id the load names, and where it stands, kept from
//! the client's own movement messages (`0x803D`, `0x803E`, `0x8047`, each a `Vect2` of x and y at
//! the head of its body). The reference's `Player`/`Unit`/`Creature` classes are this noun's next
//! parts: the map's own agents - the NPCs and their spawn rows - arrive with step 6 of the world
//! plan, which needs the agent-creation wire this tree has not captured.

/// One body the client draws: the player's own, for now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Agent {
    /// The id the load's messages name it by.
    pub id: u32,
    /// Where it stands, in the game's own units.
    pub x: f32,
    /// The other axis.
    pub y: f32,
    /// Which way it faces, in radians; zero until a movement names one.
    pub facing: f32,
}

impl Agent {
    /// A body at a point.
    pub fn at(id: u32, x: f32, y: f32) -> Self {
        Self {
            id,
            x,
            y,
            facing: 0.0,
        }
    }

    /// The client's report of where it went. The client is the one that moved it; this is what the
    /// world knows of it, and where a later load would place it.
    pub fn moved_to(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_moves_where_the_client_says() {
        let mut agent = Agent::at(1, 9541.87, 4796.46);
        agent.moved_to(9550.0, 4800.0);
        assert_eq!((agent.x, agent.y), (9550.0, 4800.0));
        assert_eq!(agent.id, 1);
    }
}
