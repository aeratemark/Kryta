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
#![allow(dead_code)] // the ported search arrives ahead of its callers; see docs/port/09-parity-audit.md 7b
#![allow(clippy::manual_is_multiple_of)] // (len % 2) == 0 is the reference spelling in PathHeapPop
#![allow(clippy::collapsible_if)] // the nested if is the reference shape in PickNextPoint


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

// ---------------------------------------------------------------------------------------------
// ASURA/crates/asura-world/src/pathfind.rs, lines 33-454: the geometry the search is made of.
//
// Copied function for function, in the file's own order. The quirks are the reference's and stay:
// `closest_point_on_segment` stores the offset rather than the projected point because nothing reads
// the point, `heap_pop`'s sift-down is spelled with the extra "last full depth" case because that is
// the order equal-cost nodes pop in, and `precise_cross` treats near-parallel as parallel.
// ---------------------------------------------------------------------------------------------

use crate::terrain::map_data::{Trapezoid, Vec2f};
use crate::terrain::path_tables::{path_reciproc, path_sqrt};

/// `fclampf`.
fn clampf(value: f32, min: f32, max: f32) -> f32 {
    value.min(max).max(min)
}

/// `Vec2fIsZero`.
fn is_zero(vector: Vec2f) -> bool {
    vector.x == 0.0 && vector.y == 0.0
}

/// `Vec2fEqual`.
fn equal(left: Vec2f, right: Vec2f) -> bool {
    left.x == right.x && left.y == right.y
}

/// `Vec2fAdd`.
fn add(left: Vec2f, right: Vec2f) -> Vec2f {
    Vec2f {
        x: left.x + right.x,
        y: left.y + right.y,
    }
}

/// `Vec2fSub`.
fn sub(left: Vec2f, right: Vec2f) -> Vec2f {
    Vec2f {
        x: left.x - right.x,
        y: left.y - right.y,
    }
}

/// `Vec2fDot`.
fn dot(left: Vec2f, right: Vec2f) -> f32 {
    (left.x * right.x) + (left.y * right.y)
}

/// `Vec2fCross`: negative for "left of", positive for "right of" (`GmMath.h:25`).
fn cross(left: Vec2f, right: Vec2f) -> f32 {
    (left.x * right.y) - (left.y * right.x)
}

/// `Vec2fDiv`.
fn div(vector: Vec2f, scalar: f32) -> Vec2f {
    Vec2f {
        x: vector.x / scalar,
        y: vector.y / scalar,
    }
}

/// `PathVec2fDist`.
fn vec2f_dist(left: Vec2f, right: Vec2f) -> f32 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    path_sqrt((dx * dx) + (dy * dy))
}

/// `PathVec2fUnit`.
fn vec2f_unit(vector: Vec2f) -> Vec2f {
    let length = path_sqrt(dot(vector, vector));
    div(vector, length)
}

impl Pos {
    /// The two floats, as the reference's `pos.v2` sees them.
    fn v2(self) -> Vec2f {
        Vec2f {
            x: self.x,
            y: self.y,
        }
    }
}

/// `PathFindPoint`: a position and the trapezoid it was crossed into. The reference stores a
/// `PathTrapezoid *`; trapezoid ids are unique across a map's planes, so the id is the same identity.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct FindPoint {
    /// Where the point is.
    pos: Pos,
    /// The trapezoid it was crossed into, by trap id.
    trap: u32,
}

/// `PathFindNode`: one per trapezoid, filled in as the search reaches it.
#[derive(Debug, Clone, Copy, Default)]
struct FindNode {
    /// Whether the search has finished with it.
    closed: bool,
    /// The cost of the best route to it so far.
    cost_to_node: f32,
    /// The point the route entered it at.
    point: FindPoint,
    /// The next node of the chain back to the start.
    next: Option<u32>,
}

/// `PathHeapNode` (`GmPathHeap.h`).
#[derive(Debug, Clone, Copy)]
struct HeapNode {
    /// The cost the node was pushed with.
    cost: f32,
    /// The trapezoid it names.
    trap_id: u32,
}

/// `PathBuildStep` (`GmPaths.h:107`): a corridor edge the waypoint builder may cut back to.
#[derive(Debug, Clone, Copy)]
struct BuildStep {
    /// The point on the edge.
    pos: Vec2f,
    /// The direction the walk crosses it in.
    dir: Vec2f,
    /// The plane it is on.
    plane: u16,
    /// The trapezoid it leads into.
    next_trap: u32,
}

/// `PathHeapPush` (`GmPathHeap.c:3`).
fn heap_push(heap: &mut Vec<HeapNode>, node: HeapNode) {
    heap.push(node);
    if heap.len() < 2 {
        return;
    }
    let mut hole = heap.len() - 1;
    while hole > 0 {
        let parent = (hole - 1) >> 1;
        if heap[parent].cost < node.cost {
            break;
        }
        heap[hole] = heap[parent];
        hole = parent;
    }
    heap[hole] = node;
}

/// `PathHeapPop` (`GmPathHeap.c:24`): a binary heap whose sift-down is spelled with the extra "last full
/// depth" case rather than a plain two-children comparison. Kept as written, because the order it pops
/// equal-cost nodes in is the order the search visits them in.
fn heap_pop(heap: &mut Vec<HeapNode>) -> Option<HeapNode> {
    if heap.is_empty() {
        return None;
    }
    let len = heap.len() - 1;
    let last_node = heap[len];
    let popped = heap[0];
    if len == 0 {
        heap.clear();
        return Some(popped);
    }

    let mut idx = 0usize;
    let mut hole = 0usize;
    let last_full_depth_idx = len >> 1;
    while idx < last_full_depth_idx {
        idx = (idx * 2) + 2;
        if heap[idx - 1].cost < heap[idx].cost {
            idx -= 1;
        }
        if last_node.cost <= heap[idx].cost {
            break;
        }
        heap[hole] = heap[idx];
        hole = idx;
    }

    if idx == last_full_depth_idx && (len % 2) == 0 && heap[len - 1].cost < last_node.cost {
        heap[hole] = heap[len - 1];
        hole = len - 1;
    }

    heap[hole] = last_node;
    heap.truncate(len);
    Some(popped)
}

/// `PathReversePathFindNode`: the chain of `next` links, reversed in place.
fn reverse_path_find_node(nodes: &mut [FindNode], head: u32) -> Option<u32> {
    let mut previous = None;
    let mut current = Some(head);
    while let Some(id) = current {
        let next = nodes[id as usize].next;
        nodes[id as usize].next = previous;
        previous = Some(id);
        current = next;
    }
    previous
}

/// `IsPointInTrap`.
fn is_point_in_trap(pos: Pos, trap: &Trapezoid) -> bool {
    if pos.y <= trap.yb || trap.yt <= pos.y {
        return false;
    }

    let top = Vec2f {
        x: trap.xtl,
        y: trap.yt,
    };
    let bottom = Vec2f {
        x: trap.xbl,
        y: trap.yb,
    };
    if cross(pos.v2(), sub(top, bottom)) < 0.0 {
        return false;
    }

    let top = Vec2f {
        x: trap.xtr,
        y: trap.yt,
    };
    let bottom = Vec2f {
        x: trap.xbr,
        y: trap.yb,
    };
    if 0.0 < cross(pos.v2(), sub(top, bottom)) {
        return false;
    }

    true
}

/// `GetClosestPointOnLineSegment`.
///
/// The reference stores `oproj` - the offset from the projected point back to `pos` - rather than the
/// projected point itself. Nothing reads the point afterwards, only the distance it travels with, so the
/// quirk is invisible; it is reproduced rather than corrected.
fn closest_point_on_segment(
    pos: Pos,
    p1: Vec2f,
    p2: Vec2f,
    distance: &mut f32,
    closest: &mut Vec2f,
) {
    let p = sub(p2, p1);
    let v = sub(pos.v2(), p1);

    if p.x == 0.0 && p.y == 0.0 {
        let norm = dot(v, v);
        if norm < *distance {
            *distance = norm;
            *closest = p1;
        }
        return;
    }

    let coeff = dot(v, p) / dot(p, p);
    let projection = if (0.0..=1.0).contains(&coeff) {
        Vec2f {
            x: (coeff * p.x) + p1.x,
            y: (coeff * p.y) + p1.y,
        }
    } else if 1.0 <= coeff {
        p2
    } else {
        p1
    };

    let offset = sub(pos.v2(), projection);
    let norm = dot(offset, offset);
    if norm < *distance {
        *distance = norm;
        *closest = offset;
    }
}

/// `DistToTrap`: squared distance to the trapezoid's edges, or zero inside it.
fn dist_to_trap(pos: Pos, trap: &Trapezoid) -> f32 {
    if is_point_in_trap(pos, trap) {
        return 0.0;
    }

    let mut distance = f32::INFINITY;
    let mut point = Vec2f::default();

    let tl = Vec2f {
        x: trap.xtl,
        y: trap.yt,
    };
    let tr = Vec2f {
        x: trap.xtr,
        y: trap.yt,
    };
    let bl = Vec2f {
        x: trap.xbl,
        y: trap.yb,
    };
    let br = Vec2f {
        x: trap.xbr,
        y: trap.yb,
    };
    closest_point_on_segment(pos, tl, tr, &mut distance, &mut point);
    closest_point_on_segment(pos, tr, br, &mut distance, &mut point);
    closest_point_on_segment(pos, br, bl, &mut distance, &mut point);
    closest_point_on_segment(pos, bl, tl, &mut distance, &mut point);

    distance
}

/// `FindPointOnNextTrap`: where the line from the current position to the destination crosses the
/// neighbour's edge at `next_y`, clamped into the shared span.
fn find_point_on_next_trap(
    min_left_x: f32,
    max_right_x: f32,
    next_y: f32,
    src_pos: Vec2f,
    dst_pos: Vec2f,
) -> Vec2f {
    let mut percent = -1.0f32;

    if src_pos.y != dst_pos.y {
        let divisor = path_reciproc(dst_pos.y - src_pos.y);
        percent = (next_y - src_pos.y) * divisor;
    }

    if percent < 0.0 {
        let src_x = clampf(src_pos.x, min_left_x, max_right_x);
        let dst_x = clampf(dst_pos.x, min_left_x, max_right_x);
        // 0.9 and 0.1: the reference spells them as the doubles nearest those floats
        // (`0.8999999761581421f`, `0.1000000238418579f`), which is what these are.
        Vec2f {
            x: (src_x * 0.9) + (dst_x * 0.1),
            y: next_y,
        }
    } else {
        let new_x = ((dst_pos.x - src_pos.x) * percent) + src_pos.x;
        Vec2f {
            x: clampf(new_x, min_left_x, max_right_x),
            y: next_y,
        }
    }
}

/// `FindIntersectionPoint`: `t1`/`t2` such that `start1 + t1*dir1 == start2 + t2*dir2`.
fn find_intersection_point(
    start1: Vec2f,
    dir1: Vec2f,
    start2: Vec2f,
    dir2: Vec2f,
) -> Option<(f32, f32)> {
    let determinant = cross(dir1, dir2);
    if determinant == 0.0 {
        return None;
    }
    let between = sub(start1, start2);
    Some((
        cross(dir2, between) / determinant,
        cross(dir1, between) / determinant,
    ))
}

/// `PickNextPoint`: where a walk crosses the segment from `point1` to `point2`.
fn pick_next_point(point1: Vec2f, point2: Vec2f, cur_pos: Vec2f, dst_pos: Vec2f) -> Vec2f {
    let cur_to_dst = sub(dst_pos, cur_pos);
    let point1_to_point2 = sub(point2, point1);

    if let Some((_, t2)) = find_intersection_point(cur_pos, cur_to_dst, point1, point1_to_point2) {
        if 0.0 <= t2 {
            let t2 = t2.min(1.0);
            return Vec2f {
                x: point1.x + (point1_to_point2.x * t2),
                y: point1.y + (point1_to_point2.y * t2),
            };
        }
    }

    let norm2 = dot(point1_to_point2, point1_to_point2);
    let divisor = path_reciproc(norm2);

    let t1 = clampf(dot(point1, point1_to_point2) * divisor, 0.0, 1.0);
    let t2 = clampf(dot(point2, point1_to_point2) * divisor, 0.0, 1.0);
    let t = (t2 * 0.1) + (t1 * 0.9);
    Vec2f {
        x: point1.x + (point1_to_point2.x * t),
        y: point1.y + (point1_to_point2.y * t),
    }
}

/// `PathVec2fPreciseCross`: a cross product that treats near-parallel as parallel, using unit vectors to
/// decide when the raw product is too small to be meaningful.
fn precise_cross(left: Vec2f, right: Vec2f) -> f32 {
    if left.x == 0.0 && left.y == 0.0 {
        return 0.0;
    }
    if right.x == 0.0 && right.y == 0.0 {
        return 0.0;
    }

    let product = cross(left, right);
    if -0.01 < product && product < 0.01 {
        return 0.0;
    }
    if product <= -1.0 || 1.0 <= product {
        return product;
    }

    let product = cross(vec2f_unit(left), vec2f_unit(right));
    if product <= -0.01 || 0.01 <= product {
        return product;
    }
    0.0
}

// ---------------------------------------------------------------------------------------------
// ASURA/crates/asura-world/src/pathfind.rs, lines 456-557: the context a search runs in, and the two
// lookups it answers with. `PathFinding` itself (line 779 on) is the next piece.
// ---------------------------------------------------------------------------------------------

use crate::terrain::map_data::{PathStaticData, Portal};

/// A path finder over one map's imported graph.
///
/// The node array, the priority queue and the corridor steps are kept between searches, because the
/// reference keeps them in its context: a second search on the same map does not reallocate, and the
/// node array is cleared rather than rebuilt.
///
/// The graph itself is shared (`Arc`) rather than borrowed: a world keeps one finder per map for as long
/// as the map is served, and the graph is the same object for every walk. That is the reference's
/// `srv->map_context` with one difference - it has one context per server because it runs one server per
/// map, while this hosts several maps in one process.
pub struct PathContext {
    data: std::sync::Arc<PathStaticData>,
    /// Trap id to `(plane id, position in that plane's trapezoids)`. Trap ids are dense and assigned in
    /// plane order by the importer, so this is the reference's `array_at(&nodes, trap->trap_id)` and its
    /// `PathTrapezoid *` in one table.
    index: Vec<(u16, u32)>,
    nodes: Vec<FindNode>,
    prioq: Vec<HeapNode>,
    steps: Vec<BuildStep>,
}

impl PathContext {
    /// A finder over one map's graph, with the trap index built once.
    pub fn new(data: std::sync::Arc<PathStaticData>) -> PathContext {
        let mut index = vec![(0u16, 0u32); data.traps_count as usize];
        for plane in &data.planes {
            for (position, trapezoid) in plane.trapezoids.iter().enumerate() {
                if let Some(slot) = index.get_mut(trapezoid.trap_id as usize) {
                    *slot = (plane.plane_id, position as u32);
                }
            }
        }
        PathContext {
            data,
            index,
            nodes: Vec::new(),
            prioq: Vec::new(),
            steps: Vec::new(),
        }
    }

    /// The trapezoid a trap id names.
    fn trap(&self, trap_id: u32) -> Trapezoid {
        let (plane, position) = self.index[trap_id as usize];
        self.data.planes[usize::from(plane)].trapezoids[position as usize]
    }

    /// The plane a trap id lives on.
    fn plane_of(&self, trap_id: u32) -> u16 {
        self.index[trap_id as usize].0
    }

    /// A trapezoid's neighbour field (`Option<usize>`, a position in its own plane) as a trap id.
    fn neighbour(&self, trap_id: u32, local: Option<usize>) -> Option<u32> {
        local.map(|position| {
            let plane = usize::from(self.plane_of(trap_id));
            self.data.planes[plane].trapezoids[position].trap_id
        })
    }

    /// A portal of the plane `plane`.
    fn portal(&self, plane: u16, portal_id: usize) -> Option<Portal> {
        self.data
            .planes
            .get(usize::from(plane))?
            .portals
            .get(portal_id)
            .copied()
    }

    /// The trapezoids a portal leads to, as trap ids: the reference walks `portal->pair->traps` and
    /// dereferences each entry, so an entry the importer left NULL is skipped here rather than crashing.
    fn portal_pair_traps(&self, portal: &Portal) -> Vec<u32> {
        let Some(plane) = self.data.planes.get(usize::from(portal.portal_plane_id)) else {
            return Vec::new();
        };
        let mut traps = Vec::with_capacity(portal.traps_count as usize);
        for offset in 0..portal.traps_count as usize {
            let entry = plane
                .portals_traps
                .get(portal.traps + offset)
                .copied()
                .flatten();
            if let Some(position) = entry {
                traps.push(plane.trapezoids[position].trap_id);
            }
        }
        traps
    }

    /// `PathFindTrapezoid`, as a trap id.
    pub fn find_trapezoid(&self, pos: Pos) -> Option<u32> {
        let (plane, position) = self.data.trapezoid_at(pos.v2(), pos.plane)?;
        Some(self.data.planes[usize::from(plane)].trapezoids[position].trap_id)
    }

    /// Which plane a point lies on, if any does: the first plane whose search places it.
    ///
    /// This global lookup does not establish a connected surface for directional movement.
    pub fn plane_at(&self, point: Vec2f) -> Option<u16> {
        let count = u16::try_from(self.data.planes.len()).ok()?;
        (0..count).find(|plane| self.data.trapezoid_at(point, *plane).is_some())
    }

    // -----------------------------------------------------------------------------------------
    // ASURA/crates/asura-world/src/pathfind.rs, lines 559-777: one expansion of the search - the
    // node it queues, the neighbour it costs through a crossing, and the three ways a neighbour is
    // reached (above, bellow, through a portal). `PathFinding` itself (line 779 on) is next.
    // -----------------------------------------------------------------------------------------

    /// `PathAddNode`: record the cost of reaching `trap` and queue it.
    fn add_node(
        &mut self,
        parent: Option<u32>,
        trap: u32,
        pos: Pos,
        current_cost: f32,
        estimated_cost: f32,
    ) {
        let node = &mut self.nodes[trap as usize];
        node.closed = false;
        node.cost_to_node = current_cost;
        node.point.pos = pos;
        node.point.trap = trap;
        node.next = parent;
        heap_push(
            &mut self.prioq,
            HeapNode {
                cost: estimated_cost,
                trap_id: trap,
            },
        );
    }

    /// `PathVisitTrap`: cost a neighbour reached through a crossing point.
    fn visit_trap(
        &mut self,
        curr_node: u32,
        dst_pos: Vec2f,
        neighbour: u32,
        cross_pos: Pos,
        max_cost: f32,
    ) {
        let current = self.nodes[curr_node as usize];
        let distance = vec2f_dist(current.point.pos.v2(), cross_pos.v2());
        let cost_to_trap = current.cost_to_node + distance;

        if max_cost <= cost_to_trap {
            return;
        }

        let trap = self.trap(neighbour);

        let bottom_point = Vec2f {
            x: clampf(dst_pos.x, trap.xbl, trap.xbr),
            y: trap.yb,
        };
        let estimated_dest_bottom = vec2f_dist(bottom_point, dst_pos);

        let top_point = Vec2f {
            x: clampf(dst_pos.x, trap.xtl, trap.xtr),
            y: trap.yt,
        };
        let estimated_dest_top = vec2f_dist(top_point, dst_pos);

        let mut best_estimate = estimated_dest_bottom.min(estimated_dest_top);
        if trap.yb <= dst_pos.y && dst_pos.y <= trap.yt {
            // The reference subtracts `0.009999999776482582`, the double nearest the float 0.01.
            best_estimate -= 0.01;
        }

        let estimated_cost = cost_to_trap + best_estimate;
        self.add_node(
            Some(curr_node),
            neighbour,
            cross_pos,
            cost_to_trap,
            estimated_cost,
        );
    }

    /// `PathVisitAbove`.
    fn visit_above(&mut self, curr_node: u32, dst_pos: Vec2f, neighbour: u32, max_cost: f32) {
        let trap = self.trap(self.nodes[curr_node as usize].point.trap);
        let next = self.trap(neighbour);
        let xl = next.xbl.max(trap.xtl);
        let xr = next.xbr.min(trap.xtr);
        let src = self.nodes[curr_node as usize].point.pos;
        let crossing = find_point_on_next_trap(xl, xr, next.yb, src.v2(), dst_pos);
        self.visit_trap(
            curr_node,
            dst_pos,
            neighbour,
            Pos {
                x: crossing.x,
                y: crossing.y,
                plane: src.plane,
            },
            max_cost,
        );
    }

    /// `PathVisitBellow`.
    fn visit_bellow(&mut self, curr_node: u32, dst_pos: Vec2f, neighbour: u32, max_cost: f32) {
        let trap = self.trap(self.nodes[curr_node as usize].point.trap);
        let next = self.trap(neighbour);
        let xl = next.xtl.max(trap.xbl);
        let xr = next.xtr.min(trap.xbr);
        let src = self.nodes[curr_node as usize].point.pos;
        let crossing = find_point_on_next_trap(xl, xr, next.yt, src.v2(), dst_pos);
        self.visit_trap(
            curr_node,
            dst_pos,
            neighbour,
            Pos {
                x: crossing.x,
                y: crossing.y,
                plane: src.plane,
            },
            max_cost,
        );
    }

    /// `PathVisitPortalLeft` (and, with `left` false, `PathVisitPortalRight`): the trapezoids on the far
    /// side of a portal, entered where the walk's line crosses the portal's own edge.
    fn visit_portal(
        &mut self,
        curr_node: u32,
        dst_pos: Vec2f,
        portal_id: u16,
        left: bool,
        max_cost: f32,
    ) {
        let src = self.nodes[curr_node as usize].point.pos;
        let Some(portal) = self.portal(src.plane, usize::from(portal_id)) else {
            return;
        };
        // The reference dereferences `portal->pair` and asserts it is the mirror; a portal without one is
        // a crash there, so refusing here is the closest thing to parity that does not abort.
        let Some(pair) = portal.pair else {
            return;
        };
        let Some(pair_portal) = self.portal(pair.0, pair.1) else {
            return;
        };
        debug_assert_eq!(pair_portal.portal_plane_id, portal.neighbor_plane_id);
        debug_assert_eq!(pair_portal.neighbor_plane_id, portal.portal_plane_id);

        if (portal.flags & 0x4) != 0 {
            return;
        }

        let traps = self.portal_pair_traps(&pair_portal);
        let curr_trap = self.trap(self.nodes[curr_node as usize].point.trap);

        for portal_trap in traps {
            if self.nodes[portal_trap as usize].closed {
                continue;
            }
            let trap = self.trap(portal_trap);

            // The top point is the higher of the two trapezoids' top edges on the side the portal is on;
            // the bottom point is the lower of the two bottom edges. `left` picks which corner.
            let (top, bottom) = if left {
                let top = if trap.yt <= curr_trap.yt {
                    Vec2f {
                        x: trap.xtr,
                        y: trap.yt,
                    }
                } else {
                    Vec2f {
                        x: curr_trap.xtl,
                        y: curr_trap.yt,
                    }
                };
                let bottom = if curr_trap.yb <= trap.yb {
                    Vec2f {
                        x: trap.xbr,
                        y: trap.yb,
                    }
                } else {
                    Vec2f {
                        x: curr_trap.xbl,
                        y: curr_trap.yb,
                    }
                };
                (top, bottom)
            } else {
                let top = if trap.yt <= curr_trap.yt {
                    Vec2f {
                        x: trap.xtl,
                        y: trap.yt,
                    }
                } else {
                    Vec2f {
                        x: curr_trap.xtr,
                        y: curr_trap.yt,
                    }
                };
                let bottom = if curr_trap.yb <= trap.yb {
                    Vec2f {
                        x: trap.xbl,
                        y: trap.yb,
                    }
                } else {
                    Vec2f {
                        x: curr_trap.xbr,
                        y: curr_trap.yb,
                    }
                };
                (top, bottom)
            };

            if bottom.y <= top.y {
                let crossing = pick_next_point(top, bottom, src.v2(), dst_pos);
                self.visit_trap(
                    curr_node,
                    dst_pos,
                    portal_trap,
                    Pos {
                        x: crossing.x,
                        y: crossing.y,
                        plane: pair_portal.portal_plane_id,
                    },
                    max_cost,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// ASURA/crates/asura-world/src/pathfind.rs, lines 921-940: the corridor the waypoint builder follows,
// declared here so `PathFinding` (line 783) and `PathCreateWaypoints` (line 891) arrive together with
// the helper they hand themselves to. `BuildHelper`'s own `impl` (line 941 on) is the next piece.
// ---------------------------------------------------------------------------------------------

/// `PathFindBound` (`GmPaths.c:552`): one side of the corridor being followed, and the step it began at.
#[derive(Debug, Clone, Copy)]
struct Bound {
    /// The point on this side.
    point: FindPoint,
    /// The step of the corridor it began at.
    step_id: usize,
    /// The direction out of this side.
    vec: Vec2f,
}

/// `PathBuildHelper` (`GmPaths.c:558`).
struct BuildHelper<'c> {
    /// The finder whose steps and waypoints are being walked.
    context: &'c mut PathContext,
    /// The corridor's left side.
    left_bound: Bound,
    /// The corridor's right side.
    right_bound: Bound,
    /// Where the corridor stands.
    curr_start_point: FindPoint,
    /// The plane the corridor is on.
    current_plane: u16,
    /// The last point emitted, for the direction check.
    last_pos: Pos,
    /// Where the walk is going.
    dst_point: FindPoint,
    /// The waypoints being written.
    waypoints: &'c mut Vec<Waypoint>,
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
    /// `PathHeapPush`/`PathHeapPop` pop the cheapest node first, and keep doing it as the heap shrinks -
    /// the order is the order the search visits trapezoids in, which is why the reference's own
    /// sift-down is kept rather than a tidy one.
    #[test]
    fn the_heap_pops_in_cost_order() {
        let mut heap = Vec::new();
        for (cost, trap_id) in [(5.0f32, 5u32), (1.0, 1), (3.0, 3), (2.0, 2), (4.0, 4), (0.5, 6)] {
            heap_push(&mut heap, HeapNode { cost, trap_id });
        }
        let mut popped = Vec::new();
        while let Some(node) = heap_pop(&mut heap) {
            popped.push(node.cost);
        }
        assert_eq!(popped, vec![0.5, 1.0, 2.0, 3.0, 4.0, 5.0], "cheapest first");
        assert!(heap.is_empty());
    }

    /// `PathVec2fPreciseCross` calls near-parallel parallel: a product under a hundredth is zero, and a
    /// perpendicular pair keeps its sign.
    #[test]
    fn a_near_parallel_cross_is_treated_as_parallel() {
        let along = Vec2f { x: 1.0, y: 0.0 };
        assert_eq!(precise_cross(along, Vec2f { x: 1.0, y: 0.0 }), 0.0, "parallel");
        let nearly = Vec2f { x: 1.0, y: 0.001 };
        assert_eq!(precise_cross(along, nearly), 0.0, "nearly parallel");
        let across = Vec2f { x: 0.0, y: 1.0 };
        assert_eq!(precise_cross(along, across), 1.0, "perpendicular");
        assert_eq!(precise_cross(across, along), -1.0, "and the other order");
        assert_eq!(
            precise_cross(Vec2f::default(), across),
            0.0,
            "a zero vector has no direction"
        );
    }
}
