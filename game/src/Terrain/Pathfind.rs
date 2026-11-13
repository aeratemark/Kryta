//! The map's walkable graph, and what a walk request is answered with.
//!
//! Reference: the reference's path finder over a map's pmap (`GmPaths.h`, `PathFindQuery`), and the
//! working implementation's port of it, `ASURA/crates/asura-world/src/pathfind.rs`. The types a caller
//! passes and receives are the working implementation's own (`pathfind::Pos`, `pathfind::Waypoint`,
//! `Walk` in `agent.rs:570`), so the handlers that use this file are that file's handlers.
//!
//! **The graph itself is not here yet.** It is read out of the client's own archive - the path tables
//! and the bloated map blob, `gwdat.rs`, `fa_compress.rs`, `map_data.rs` and `path_tables.rs` in the
//! working implementation - and this tree's `game/Pack/` is not built. So every lookup answers
//! `Walk::NoGraph`, which is exactly what the working implementation answers for a map it holds no
//! graph for, and both its handlers already say what to do with that answer: a walk request sends
//! **nothing** (`agent.rs:858-879`), and a manual movement sends nothing either (`agent.rs:1023-1029`).
//!
//! `docs/port/09-parity-audit.md` records the graph as missing. When the archive reader lands, this
//! file gains the finder and nothing above it changes.

/// A point on a map, in the game's own units, on one of the map's planes.
///
/// `ASURA/crates/asura-world/src/pathfind.rs:113`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pos {
    /// East-west.
    pub x: f32,
    /// North-south.
    pub y: f32,
    /// Which surface: the floor is 0, and slopes and bridges have their own ids.
    pub plane: u16,
}

/// One point of a walk, and the trapezoid it was crossed into.
///
/// `ASURA/crates/asura-world/src/pathfind.rs:131`. The reference stores a `PathTrapezoid *`; trapezoid
/// ids are unique across a map's planes, so the id is the same identity.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Waypoint {
    /// Where the point is.
    pub pos: Pos,
    /// The trapezoid it stands on, or 0 when the point is not on one.
    pub trap_id: u32,
}

/// What the finder said about a walk.
///
/// `ASURA/crates/asura-world/src/agent.rs:570`.
#[derive(Debug, Clone)]
pub enum Walk {
    /// A path, in the reference's reduced form. It may hold a single waypoint naming the destination.
    Found(Vec<Waypoint>),
    /// The finder ran and refused: the destination is not reachable from where the agent stands.
    NoPath,
    /// This world has no graph for the map, so nothing can be said about reachability - which is every
    /// map here until the archive reader exists.
    NoGraph,
}

/// The graphs this world holds, one per map.
#[derive(Debug, Clone, Default)]
pub struct Graphs;

impl Graphs {
    /// The graphs a world with no archive reader has: none.
    pub fn new() -> Self {
        Self
    }

    /// Ask for a walk from `from` to `to` on `map_id`.
    ///
    /// `ASURA/crates/asura-world/src/pathfind.rs`, `World::graphs().walk(...)`. Every map here answers
    /// `NoGraph`; a caller must handle it exactly as the working implementation does.
    pub fn walk(&self, _map_id: u32, _from: Pos, _to: Pos) -> Walk {
        Walk::NoGraph
    }

    /// Follow manual input through adjacent walkable cells, rather than finding a route.
    ///
    /// `ASURA/crates/asura-world/src/pathfind.rs`, `walk_direction`. Manual movement uses this one, not
    /// `walk`: choosing a destination plane globally can send a player off a bridge to reach the point.
    pub fn walk_direction(&self, _map_id: u32, _from: Pos, _offset: (f32, f32)) -> Walk {
        Walk::NoGraph
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_world_without_an_archive_reader_has_no_graph_and_says_so() {
        let graphs = Graphs::new();
        let here = Pos {
            x: 0.0,
            y: 0.0,
            plane: 0,
        };
        assert!(matches!(graphs.walk(148, here, here), Walk::NoGraph));
        assert!(matches!(
            graphs.walk_direction(148, here, (1.0, 0.0)),
            Walk::NoGraph
        ));
    }
}
