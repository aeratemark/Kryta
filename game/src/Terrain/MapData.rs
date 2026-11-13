//! A map's path graph: the types the reference's A* consumes, and the box its trapezoids occupy.
//!
//! Ported from `ASURA/crates/asura-world/src/map_data.rs`, first section, **every type with its own
//! name, its own fields in their own order**. Nothing here is renamed, regrouped or tidied: the field
//! order is the order the reference's structs read (`GmMapDataImport.h`, `GmPaths.h`), and the dump
//! format (`graph_dump.rs`, ported next) writes exactly these fields in this order.
//!
//! The importer that builds these from a `Gw.dat` blob is a later piece of this subject. The graph
//! itself - planes, trapezoids, split nodes, portals - is what the finder searches, and this file is
//! that graph and nothing else.
//!
//! Reference anchors:
//!
//! * `Vec2f` - `Vec2fSub`, `Vec2fCross` (`GmPolygon.h`);
//! * `NodeRef` - the reference stores pointers, this stores an array position; `NodeType` plus an
//!   index is the same identity (`GmMapDataImport.c:9`);
//! * `XNode`, `YNode`, `SinkNode`, `Trapezoid`, `Portal`, `Plane` - the import's own records;
//! * `PathStaticData` - one map's planes and the trapezoid counter they share;
//! * `Bounds` - the `worldDims` an agent's position is checked against.

/// `PATH_DATA_SIGNATURE` / `PATH_DATA_VERSION`: the first eight bytes of the path section.
pub const PATH_DATA_SIGNATURE: u32 = 0xEEFE_704C;

/// The version a path section states, which is the only one this reader accepts.
pub const PATH_DATA_VERSION: u32 = 12;

/// `STAGE_SHIFT`: a chunk id's stage lives in its top nibble, its `ChunkId` in the rest.
pub const STAGE_SHIFT: u32 = 28;

/// The stage a map's own data sits at: `GmImportMap` builds `(2 << STAGE_SHIFT) | ChunkId_Path`.
pub const STAGE_MAP: u32 = 2;

/// The three arrays a node id can name, in the order their numbering starts (`GmMapDataImport.c:9`).
pub const FIRST_X_NODE_INDEX: u32 = 0;

/// Where the y-node numbering starts.
pub const FIRST_Y_NODE_INDEX: u32 = 0x4000_0000;

/// Where the sink-node numbering starts.
pub const FIRST_SINK_NODE_INDEX: u32 = 0x8000_0000;

/// `NodeType`, the byte `PathImport_ReadRootNode` reads: a split along a line.
pub const NODE_TYPE_X: u8 = 0;

/// `NodeType`: a split along the horizontal.
pub const NODE_TYPE_Y: u8 = 1;

/// `NodeType`: a leaf, which is a trapezoid.
pub const NODE_TYPE_SINK: u8 = 2;

/// A 2-D position, the reference's `Vec2f`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec2f {
    /// The first axis.
    pub x: f32,
    /// The second axis.
    pub y: f32,
}

impl Vec2f {
    /// `Vec2fSub`: the vector from `other` to this one.
    ///
    /// The name is the reference function's, so it stays even though Rust has an operator for it.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, other: Vec2f) -> Vec2f {
        Vec2f {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    /// `Vec2fCross`: the z of the cross product, which is the sign of "left or right of a direction".
    pub fn cross(self, other: Vec2f) -> f32 {
        (self.x * other.y) - (self.y * other.x)
    }
}

/// A pointer to a node, as a position in one of a plane's three node arrays. `NodeType` plus the array
/// index is what the reference's node id encodes, so this is the same identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRef {
    /// A node in the plane's x-node array.
    X(usize),
    /// A node in the plane's y-node array.
    Y(usize),
    /// A node in the plane's sink-node array.
    Sink(usize),
}

/// `XNode`: a split along a line, `left`/`right` being the halves.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct XNode {
    /// The point the line passes through.
    pub pos: Vec2f,
    /// The direction the line runs in.
    pub dir: Vec2f,
    /// The half on the left, when it holds a node.
    pub left: Option<NodeRef>,
    /// The half on the right, when it holds a node.
    pub right: Option<NodeRef>,
}

/// `YNode`: a split along the horizontal, `above`/`bellow` being the halves (the reference's spelling).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct YNode {
    /// The point the split passes through.
    pub pos: Vec2f,
    /// The half above it, when it holds a node.
    pub above: Option<NodeRef>,
    /// The half below it, when it holds a node (the reference spells it `bellow`).
    pub bellow: Option<NodeRef>,
}

/// `SinkNode`: a leaf, which is a trapezoid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SinkNode {
    /// The trapezoid's position in the plane's own array.
    pub trap: usize,
}

/// `PathTrapezoid`: a convex cell of the plane. The six floats are its corners; `portal_left` and
/// `portal_right` name portals, not trapezoids.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Trapezoid {
    /// `trap_id`: unique across **all** planes, because it comes from the static data's counter.
    pub trap_id: u32,
    /// The neighbour above-left, as an index into `portals_traps`.
    pub top_left: Option<usize>,
    /// The neighbour above-right.
    pub top_right: Option<usize>,
    /// The neighbour below-left.
    pub bottom_left: Option<usize>,
    /// The neighbour below-right.
    pub bottom_right: Option<usize>,
    /// The portal on its left side.
    pub portal_left: u16,
    /// The portal on its right side.
    pub portal_right: u16,
    /// The top edge's height.
    pub yt: f32,
    /// The bottom edge's height.
    pub yb: f32,
    /// The top-left corner's x.
    pub xtl: f32,
    /// The top-right corner's x.
    pub xtr: f32,
    /// The bottom-left corner's x.
    pub xbl: f32,
    /// The bottom-right corner's x.
    pub xbr: f32,
}

/// `PathPortal`: a way from one plane to another through a trapezoid's side.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Portal {
    /// The plane this portal belongs to.
    pub portal_plane_id: u16,
    /// The plane it leads to.
    pub neighbor_plane_id: u16,
    /// The portal's flags, as the import read them.
    pub flags: u8,
    /// The portal on the other plane with the same shared id, as `(plane, portal)`.
    pub pair: Option<(u16, usize)>,
    /// How many trapezoids name this portal.
    pub traps_count: u32,
    /// A position in the plane's `portals_traps`, which is a list of trapezoids.
    pub traps: usize,
}

/// `PathPlane`: one plane of a map's geometry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plane {
    /// Which plane this is.
    pub plane_id: u16,
    /// The plane's vectors, as the import read them.
    pub vectors: Vec<Vec2f>,
    /// The plane's convex cells.
    pub trapezoids: Vec<Trapezoid>,
    /// The splits along a line.
    pub xnodes: Vec<XNode>,
    /// The splits along the horizontal.
    pub ynodes: Vec<YNode>,
    /// The leaves.
    pub sink_nodes: Vec<SinkNode>,
    /// The ways from this plane to another.
    pub portals: Vec<Portal>,
    /// The reference's `h000C`: a count of records the tag-11 section holds, eight bytes each.
    pub h000c: u32,
    /// The trapezoids a portal leads to, `None` where the reference would store a NULL.
    pub portals_traps: Vec<Option<usize>>,
    /// Where walking the splits starts (`None` where the reference's pointer would be NULL).
    pub root_node: Option<NodeRef>,
}

/// `PathStaticData`: every plane of one map, and the trapezoid counter they share.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PathStaticData {
    /// The map's planes, in the order the import read them.
    pub planes: Vec<Plane>,
    /// `traps_count`: the counter every trapezoid's `trap_id` comes from.
    pub traps_count: u32,
}

/// The box a plane's trapezoids occupy: the `worldDims` the client checks an agent against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// The lowest x.
    pub x0: f32,
    /// The lowest y.
    pub y0: f32,
    /// The highest x.
    pub x1: f32,
    /// The highest y.
    pub y1: f32,
}

impl Bounds {
    /// Whether a point is inside the box, edges included.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.x0 <= x && x <= self.x1 && self.y0 <= y && y <= self.y1
    }
}

impl PathStaticData {
    /// The box of one plane, as the union of its trapezoids' corners.
    ///
    /// The client asserts that an agent's position is inside the `worldDims` of the plane it is on
    /// (`agint.h:929`, `pos.y >= worldDims.y0`), so a destination outside this box is a crash rather
    /// than a walk. A plane with no trapezoids has no box at all and `None` says so.
    pub fn plane_bounds(&self, plane: u16) -> Option<Bounds> {
        let plane = self.planes.get(usize::from(plane))?;
        let mut bounds: Option<Bounds> = None;
        for trapezoid in &plane.trapezoids {
            let left = trapezoid
                .xtl
                .min(trapezoid.xbl)
                .min(trapezoid.xtr)
                .min(trapezoid.xbr);
            let right = trapezoid
                .xtl
                .max(trapezoid.xbl)
                .max(trapezoid.xtr)
                .max(trapezoid.xbr);
            let low = trapezoid.yt.min(trapezoid.yb);
            let high = trapezoid.yt.max(trapezoid.yb);
            bounds = Some(match bounds {
                None => Bounds {
                    x0: left,
                    y0: low,
                    x1: right,
                    y1: high,
                },
                Some(current) => Bounds {
                    x0: current.x0.min(left),
                    y0: current.y0.min(low),
                    x1: current.x1.max(right),
                    y1: current.y1.max(high),
                },
            });
        }
        bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Vec2fSub` and `Vec2fCross` are the reference's own, sign included.
    #[test]
    fn the_vector_operations_are_the_references() {
        let a = Vec2f { x: 3.0, y: 4.0 };
        let b = Vec2f { x: 1.0, y: 1.0 };
        assert_eq!(a.sub(b), Vec2f { x: 2.0, y: 3.0 });
        assert_eq!(a.cross(b), (3.0 * 1.0) - (4.0 * 1.0));
        // The cross product's sign says which side of a direction the other vector is on.
        let right = Vec2f { x: 1.0, y: 0.0 };
        let up = Vec2f { x: 0.0, y: 1.0 };
        assert_eq!(right.cross(up), 1.0, "up is to the left of right in this order");
        assert_eq!(up.cross(right), -1.0, "and the other way round is negative");
    }

    /// The plane's box is the union of its trapezoids' corners, edges included.
    #[test]
    fn a_planes_bounds_are_the_union_of_its_trapezoids() {
        let data = PathStaticData {
            traps_count: 2,
            planes: vec![Plane {
                plane_id: 0,
                trapezoids: vec![
                    Trapezoid {
                        trap_id: 0,
                        yt: 10.0,
                        yb: 0.0,
                        xtl: 1.0,
                        xtr: 5.0,
                        xbl: 0.0,
                        xbr: 4.0,
                        ..Trapezoid::default()
                    },
                    Trapezoid {
                        trap_id: 1,
                        yt: 20.0,
                        yb: -5.0,
                        xtl: -3.0,
                        xtr: 2.0,
                        xbl: -4.0,
                        xbr: 3.0,
                        ..Trapezoid::default()
                    },
                ],
                ..Plane::default()
            }],
        };
        let bounds = data.plane_bounds(0).expect("the plane has trapezoids");
        assert_eq!(bounds.x0, -4.0);
        assert_eq!(bounds.y0, -5.0);
        assert_eq!(bounds.x1, 5.0);
        assert_eq!(bounds.y1, 20.0);
        assert!(bounds.contains(-4.0, -5.0), "edges are inside");
        assert!(!bounds.contains(-4.5, 0.0), "and past them is not");
        assert_eq!(data.plane_bounds(7), None, "a plane the map does not have");
        let empty = PathStaticData::default();
        assert_eq!(empty.plane_bounds(0), None, "a plane with no trapezoids has no box");
    }

    /// The constants are the reference's, and the node index bases do not overlap.
    #[test]
    fn the_constants_are_the_references() {
        assert_eq!(PATH_DATA_SIGNATURE, 0xEEFE_704C);
        assert_eq!(PATH_DATA_VERSION, 12);
        assert_eq!(STAGE_SHIFT, 28);
        assert_eq!(STAGE_MAP, 2);
        assert_eq!(FIRST_X_NODE_INDEX, 0);
        assert_eq!(FIRST_Y_NODE_INDEX, 0x4000_0000);
        assert_eq!(FIRST_SINK_NODE_INDEX, 0x8000_0000);

        assert_eq!([NODE_TYPE_X, NODE_TYPE_Y, NODE_TYPE_SINK], [0, 1, 2]);
    }
}
