//! `game/Terrain/` - the reference's `src/server/game/Grids/` (18 files).
//!
//! Renamed: the reference calls this directory `Grids/`, which is its terrain and its path finder.
//!
//! What is here: the map's walkable graph as a value a handler can ask (`Pathfind.rs`) - the types and
//! the three answers of `ASURA/crates/asura-world/src/agent.rs:570`, with no graph behind them yet
//! because the archive reader they come from (`game/Pack/`) is not built. Every lookup answers
//! `NoGraph`, which is the answer the working implementation gives a map it holds no graph for.
//!
//! Not ported yet: the finder itself (`ASURA/crates/asura-world/src/pathfind.rs`, 67 KB) and the data
//! it runs on (`gwdat.rs`, `fa_compress.rs`, `map_data.rs`, `path_tables.rs`).

#[path = "Pathfind.rs"]
pub mod pathfind;

#[path = "MapData.rs"]
pub mod map_data;

#[path = "PathTables.rs"]
pub mod path_tables;

pub use map_data::{Bounds, NodeRef, PathStaticData, Plane, Portal, SinkNode, Trapezoid, Vec2f, XNode, YNode};
pub use path_tables::{path_reciproc, path_sqrt};
pub use pathfind::{Graphs, Pos, Walk, Waypoint};
