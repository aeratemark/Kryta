//! Walking a player to another agent: spacebar or a click on an NPC (`GAME_CMSG_INTERACT_LIVING`,
//! `0x8039`) or on a party member (`GAME_CMSG_INTERACT_PLAYER`, `0x8033`).
//!
//! Ported 1:1 from `ASURA/crates/asura-world/src/approach.rs` (468 lines, read from the local ASURA
//! tree on 2026-11-23): `Purpose`, the four constants, `Approach`, `Steered`, `halt`, `abandon`,
//! `shorten`, `steer` (`:27-246`) and every test. The client's requests (`:248-320`) are
//! `game/Handlers/NpcHandler.rs`, because this tree keeps a handler in `Handlers/`.
//!
//! **Ported thin:** `Purpose::Attack` (`0x8026`) is carried in the type and in `steer`, but nothing
//! here starts one; the fight owns that walk when it arrives.
//!
//! What the live server does, measured (`approach.rs:5-16`; ASURA `docs/52-skills.md`, "Walking to a
//! target"):
//!
//! * it answers each request with `0x002A` for the requesting player's own agent: the target's
//!   position, two planes and the target's agent id (`movement-live-20261109-200703.jsonl` seq 626 ->
//!   630); the client walks itself there;
//! * while the target walks, the same message goes out again about every half second (seq 721, 729,
//!   747, 752); a target that stands still is not announced again;
//! * when the walker is there, `0x0028` for it (seq 637, 5129, 5557);
//! * a player already at the NPC gets no walk at all (seq 5260).
//!
//! **Ours, by decision 0018** (`docs/decisions/0018-the-walk-to-an-agent-follows-the-route.md`): a
//! client told only `0x002A` walks itself to the target in a straight line, through walls, while the
//! server's copy walks the finder's route. So the route is densified like a click's
//! (`Handlers/MovementHandler.rs`, `densify(..., LIVE_CADENCE_STEP)`), and after the `0x002A` the
//! client is also sent `0x0029` for the route's first point; the clock's `step_map` then sends each
//! next point as the copy crosses it, exactly as it does for a click.

use crate::agents::{Agent, LIVE_CADENCE_STEP, densify, distance_between};
use crate::behaviour::Pathing;
use crate::server::packets::{agent_packets, vendor_packets};
use crate::terrain::pathfind::{Pos, Walk, Waypoint};

/// Why the player walks to the agent: what ends the walk (`approach.rs:27-36`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// `INTERACT_LIVING`: walk there, stop, done.
    Interact,
    /// `INTERACT_PLAYER`: walk there, and walk again whenever the target walks away.
    Follow,
    /// `ATTACK_AGENT`: like a follow, for as long as the fight's attack is on the target.
    Attack,
}

/// How often a walking target's new position is announced: the live cadence (`approach.rs:38-39`).
pub const ANNOUNCE_MS: u32 = 500;
/// How far a target must have moved to be announced or routed again (`approach.rs:41-43`).
pub const MOVED: f32 = 1.0;
/// A stopped walker sets off again only once the target is this many ranges away
/// (`approach.rs:45-48`).
pub const RESUME_FACTOR: f32 = 1.25;
/// The route ends this far inside the range, so the walk's end is an arrival (`approach.rs:50-51`).
pub const STOP_SHORT: f32 = 0.9;

/// One player's walk to an agent (`approach.rs:53-73`).
#[derive(Debug, Clone, PartialEq)]
pub struct Approach {
    /// The agent walked to.
    pub target: u32,
    /// What ends the walk.
    pub purpose: Purpose,
    /// How near is there.
    pub range: f32,
    routed_to: Option<(f32, f32)>,
    announced: Option<(f32, f32)>,
    since_announce_ms: u32,
    stopped: bool,
    under_way: bool,
    /// The route point the walker's client was last pointed at with `0x0029` (decision 0018).
    pointed: Option<(f32, f32)>,
    since_point_ms: u32,
}

impl Approach {
    /// A walk to `target` that is there within `range`, not yet routed or announced
    /// (`approach.rs:75-89`).
    pub fn new(target: u32, purpose: Purpose, range: f32) -> Approach {
        Approach {
            target,
            purpose,
            range,
            routed_to: None,
            announced: None,
            since_announce_ms: 0,
            stopped: false,
            under_way: false,
            pointed: None,
            since_point_ms: 0,
        }
    }
}

/// What one tick of an approach produced (`approach.rs:91-101`).
#[derive(Debug, Default)]
pub struct Steered {
    /// What everyone in the copy is told.
    pub frames: Vec<Vec<u8>>,
    /// The approach is over and leaves the slot.
    pub done: bool,
    /// An interaction's walker is at the agent: the connection answers the talk now.
    pub arrived: bool,
}

/// Stop the server's copy of the walk, keeping the walker's movement type (`approach.rs:103-108`).
fn halt(walker: &mut Agent) {
    walker.cancel_movement();
    walker.motion.waypoints.clear();
    walker.motion.waypoint_idx = 0;
}

/// End an approach the player did not finish: a walk in progress stops, and everyone is told
/// (`approach.rs:110-119`).
pub fn abandon(walker: &mut Agent) -> Vec<Vec<u8>> {
    if 0.0 < walker.motion.speed {
        halt(walker);
        vec![agent_packets::agent_stop_moving(walker.agent_id).1]
    } else {
        Vec::new()
    }
}

/// Cut a route where it first comes within `stop` of `target` (`approach.rs:121-164`).
pub fn shorten(
    waypoints: Vec<Waypoint>,
    from: Pos,
    target: (f32, f32),
    stop: f32,
) -> Vec<Waypoint> {
    let mut kept = Vec::with_capacity(waypoints.len());
    let mut at = from;
    for waypoint in &waypoints {
        let end = (waypoint.pos.x, waypoint.pos.y);
        if stop < distance_between(end, target) {
            kept.push(*waypoint);
            at = waypoint.pos;
            continue;
        }
        let (dx, dy) = (end.0 - at.x, end.1 - at.y);
        let (fx, fy) = (at.x - target.0, at.y - target.1);
        let a = dx * dx + dy * dy;
        let b = 2.0 * (fx * dx + fy * dy);
        let c = fx * fx + fy * fy - stop * stop;
        let t = if a > 0.0 && c > 0.0 {
            let disc = (b * b - 4.0 * a * c).max(0.0);
            ((-b - disc.sqrt()) / (2.0 * a)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if t > 0.0 {
            kept.push(Waypoint {
                pos: Pos {
                    x: at.x + dx * t,
                    y: at.y + dy * t,
                    plane: at.plane,
                },
                trap_id: waypoint.trap_id,
            });
        }
        return kept;
    }
    waypoints
}

/// One tick of one approach: the walker's copy, the target where it stands now, the elapsed time
/// (`approach.rs:166-246`).
pub fn steer(
    approach: &mut Approach,
    walker: &mut Agent,
    target: Pos,
    map_id: i32,
    pathing: &dyn Pathing,
    elapsed_ms: u32,
) -> Steered {
    approach.since_announce_ms = approach.since_announce_ms.saturating_add(elapsed_ms);
    approach.since_point_ms = approach.since_point_ms.saturating_add(elapsed_ms);
    let there = (target.x, target.y);
    let distance = walker.distance_to(there);
    let mut steered = Steered::default();

    if distance <= approach.range {
        if approach.under_way || 0.0 < walker.motion.speed {
            halt(walker);
            steered
                .frames
                .push(agent_packets::agent_stop_moving(walker.agent_id).1);
        }
        approach.under_way = false;
        approach.stopped = true;
        approach.routed_to = None;
        approach.announced = None;
        approach.pointed = None;
        steered.done = approach.purpose == Purpose::Interact;
        steered.arrived = steered.done;
        return steered;
    }
    if approach.stopped && distance <= approach.range * RESUME_FACTOR {
        return steered;
    }

    let walking = 0.0 < walker.motion.speed;
    let moved = !matches!(approach.routed_to, Some(at) if distance_between(at, there) <= MOVED);
    // The route's first point, when the walker's client is to be pointed at it this tick
    // (decision 0018).
    let mut point = None;
    if moved || !walking {
        let route = match pathing.leg(map_id, walker.here(), target) {
            // Cut short of the target, then sampled like a click's walk (decision 0018).
            Walk::Found(waypoints) => densify(
                shorten(waypoints, walker.here(), there, approach.range * STOP_SHORT),
                walker.here(),
                LIVE_CADENCE_STEP,
            ),
            Walk::NoPath | Walk::NoGraph => {
                steered.frames = abandon(walker);
                steered.done = true;
                return steered;
            }
        };
        if walker.move_type != 1 || walker.move_speed != 1.0 {
            walker.move_type = 1;
            walker.move_speed = 1.0;
            steered
                .frames
                .push(agent_packets::agent_update_speed(walker.agent_id, 1.0, 1).1);
        }
        let Some(first) = walker.begin_walk(route) else {
            steered.done = true;
            return steered;
        };
        // A walk that sets off is pointed at once; a walk re-routed after a moving target is
        // pointed again at the announce cadence, not on every tick it is re-routed.
        if !walking || approach.pointed.is_none() || ANNOUNCE_MS <= approach.since_point_ms {
            point = Some(first);
        }
        approach.routed_to = Some(there);
        approach.stopped = false;
        approach.under_way = true;
    }

    let first = approach.announced.is_none();
    let changed = !matches!(approach.announced, Some(at) if distance_between(at, there) <= MOVED);
    if first || (changed && ANNOUNCE_MS <= approach.since_announce_ms) {
        steered.frames.push(vendor_packets::agent_update_destination(
            walker.agent_id,
            there,
            target.plane,
            walker.plane,
            approach.target,
        ));
        approach.announced = Some(there);
        approach.since_announce_ms = 0;
    }
    // After the `0x002A`, the route's first point, so the client walks the route rather than a
    // straight line (decision 0018; the same message a click answers with, `MovementHandler.rs`).
    if let Some(first) = point {
        steered.frames.push(
            agent_packets::agent_move_to_point(
                walker.agent_id,
                (first.pos.x, first.pos.y),
                first.pos.plane,
                walker.plane,
            )
            .1,
        );
        approach.pointed = Some((first.pos.x, first.pos.y));
        approach.since_point_ms = 0;
    }
    steered
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGENT_UPDATE_DESTINATION: u16 = 0x002A;
    const AGENT_STOP_MOVING: u16 = 0x0028;
    const AGENT_MOVE_TO_POINT: u16 = 0x0029;

    /// A flat world: one straight leg, except to a point with a negative x (`approach.rs:326-338`).
    struct Flat;
    impl Pathing for Flat {
        fn leg(&self, _map_id: i32, _from: Pos, to: Pos) -> Walk {
            if to.x < 0.0 {
                return Walk::NoPath;
            }
            Walk::Found(vec![Waypoint { pos: to, trap_id: 0 }])
        }
    }

    fn walker() -> Agent {
        Agent::new(276, (0.0, 0.0), 0, (1.0, 0.0), 288.0)
    }

    fn at(x: f32, y: f32) -> Pos {
        Pos { x, y, plane: 0 }
    }

    fn headers(frames: &[Vec<u8>]) -> Vec<u16> {
        frames
            .iter()
            .map(|frame| u16::from_le_bytes([frame[0], frame[1]]))
            .collect()
    }

    #[test]
    fn a_far_target_is_announced_and_walked_to_short_of_it() {
        let mut approach = Approach::new(48, Purpose::Interact, 50.0);
        let mut me = walker();
        let out = steer(&mut approach, &mut me, at(1000.0, 0.0), 1, &Flat, 100);
        assert_eq!(
            headers(&out.frames),
            vec![AGENT_UPDATE_DESTINATION, AGENT_MOVE_TO_POINT]
        );
        let body = &out.frames[0][2..];
        assert_eq!(&body[0..4], &276u32.to_le_bytes());
        assert_eq!(&body[4..8], &1000.0f32.to_le_bytes());
        assert_eq!(&body[16..20], &48u32.to_le_bytes());
        assert!(!out.done);
        assert!(0.0 < me.motion.speed);
        let end = me.motion.waypoints.last().expect("a route").pos;
        assert!((end.x - 955.0).abs() < 0.5, "stops 45 units short: {end:?}");
    }

    #[test]
    fn arriving_stops_the_walker_and_ends_an_interaction() {
        let mut approach = Approach::new(48, Purpose::Interact, 50.0);
        let mut me = walker();
        steer(&mut approach, &mut me, at(300.0, 0.0), 1, &Flat, 100);
        for _ in 0..10 {
            me.advance(0.1);
        }
        let out = steer(&mut approach, &mut me, at(300.0, 0.0), 1, &Flat, 100);
        assert_eq!(headers(&out.frames), vec![AGENT_STOP_MOVING]);
        assert!(out.done);
        assert!(out.arrived, "an interaction that arrives is answered");
        assert_eq!(me.motion.speed, 0.0);
    }

    #[test]
    fn a_player_already_there_hears_nothing() {
        let mut approach = Approach::new(54, Purpose::Interact, 50.0);
        let mut me = walker();
        let out = steer(&mut approach, &mut me, at(30.0, 0.0), 1, &Flat, 100);
        assert!(out.frames.is_empty());
        assert!(out.done);
        assert!(out.arrived, "the dialog at once (seq 5260)");
    }

    #[test]
    fn a_walking_target_is_announced_again_at_the_cadence_and_a_still_one_is_not() {
        let mut approach = Approach::new(48, Purpose::Follow, 50.0);
        let mut me = walker();
        steer(&mut approach, &mut me, at(2000.0, 0.0), 1, &Flat, 100);
        for _ in 0..10 {
            let out = steer(&mut approach, &mut me, at(2000.0, 0.0), 1, &Flat, 100);
            assert!(out.frames.is_empty());
        }
        let out = steer(&mut approach, &mut me, at(2000.0, 40.0), 1, &Flat, 100);
        assert_eq!(
            headers(&out.frames),
            vec![AGENT_UPDATE_DESTINATION, AGENT_MOVE_TO_POINT]
        );
        let out = steer(&mut approach, &mut me, at(2000.0, 80.0), 1, &Flat, 100);
        assert!(out.frames.is_empty(), "within the cadence");
        for step in 0..3 {
            let y = 120.0 + 40.0 * step as f32;
            let out = steer(&mut approach, &mut me, at(2000.0, y), 1, &Flat, 100);
            assert!(out.frames.is_empty());
        }
        let out = steer(&mut approach, &mut me, at(2000.0, 240.0), 1, &Flat, 100);
        assert_eq!(
            headers(&out.frames),
            vec![AGENT_UPDATE_DESTINATION, AGENT_MOVE_TO_POINT]
        );
    }

    #[test]
    fn a_follow_sets_off_again_when_the_target_walks_away() {
        let mut approach = Approach::new(48, Purpose::Follow, 50.0);
        let mut me = walker();
        let out = steer(&mut approach, &mut me, at(40.0, 0.0), 1, &Flat, 100);
        assert!(!out.done && out.frames.is_empty());
        let out = steer(&mut approach, &mut me, at(60.0, 0.0), 1, &Flat, 100);
        assert!(out.frames.is_empty());
        assert_eq!(me.motion.speed, 0.0);
        let out = steer(&mut approach, &mut me, at(400.0, 0.0), 1, &Flat, 100);
        assert_eq!(
            headers(&out.frames),
            vec![AGENT_UPDATE_DESTINATION, AGENT_MOVE_TO_POINT]
        );
        assert!(0.0 < me.motion.speed);
    }

    #[test]
    fn no_route_ends_the_approach_without_a_walk() {
        let mut approach = Approach::new(48, Purpose::Attack, 144.0);
        let mut me = walker();
        let out = steer(&mut approach, &mut me, at(-500.0, 0.0), 1, &Flat, 100);
        assert!(out.done);
        assert!(!out.arrived);
        assert!(out.frames.is_empty());
        assert_eq!(me.motion.speed, 0.0);
    }

    #[test]
    fn a_route_is_cut_where_it_enters_the_range() {
        let route = vec![
            Waypoint {
                pos: at(100.0, 0.0),
                trap_id: 1,
            },
            Waypoint {
                pos: at(100.0, 100.0),
                trap_id: 2,
            },
            Waypoint {
                pos: at(100.0, 200.0),
                trap_id: 3,
            },
        ];
        let cut = shorten(route, at(0.0, 0.0), (100.0, 200.0), 50.0);
        assert_eq!(cut.len(), 3);
        assert_eq!(cut[1].pos, at(100.0, 100.0));
        assert_eq!(cut[2].pos.x, 100.0);
        assert!((cut[2].pos.y - 150.0).abs() < 0.01, "{:?}", cut[2]);
        assert_eq!(cut[2].trap_id, 3);
    }
}
