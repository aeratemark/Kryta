//! One district's NPCs: the bodies, the agent ids its copy hands out, and what each client has been
//! told about them.
//!
//! Ported from `ASURA/crates/asura-world/src/npc/table.rs` (1,255 lines, the local repository as it
//! stands on 2026-11-22) and the NPC half of `ASURA/crates/asura-world/src/instance.rs` (`with_npcs`
//! 178-225, `join`'s agent id 307-316, `npc_arrival` 450-460, `leave` 424). ASURA's instance is this
//! tree's district, so the table lives beside the other district state.
//!
//! **Ported thin** (stages N3-N4 of `docs/asura-port-plan.md`, 2026-11-22): the population, the
//! agent ids, the load's NPC bursts, the per-client sight with compass churn, the behaviour trees
//! (`Mind::Tree`, `check_trees`, `BehaviorSource`), the burrow cycle (`table.rs:50-197`) and the walk
//! half of `tick` (`table.rs:723-822`). Not carried yet: the Script leaf and its Lua source
//! (`npc/lua.rs`; the pack's `worm_lakeside` and `worm_queen` therefore park and stand until it
//! arrives), the fight (`fight`, `obey`, `combatants`) and the `/npc` command (`table.rs:531-637`).
//!
//! Agent ids are the copy's own, not the player's (`instance.rs:180-205` and `:307-316`): the bodies
//! take consecutive ids from 2 in their file's order before any player joins, and a player's agent is
//! the lowest id no body and no other player of the copy holds. The live client agrees - in
//! `recordings/client/feature/maps/pre-searing-load-3.jsonl` player 0x35's agent is 40 (seq 221) and
//! the NPCs and players of one map interleave in one id space (seq 210-298).
//!
//! The range is a 2D distance; the plane does not enter it (`table.rs:15-16`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agents::Agent;
use crate::agents::npc_content::{MapNpcs, NpcContent, NpcPlacement, SightRules};
use crate::behaviour::{BehaviorSource, BurrowRequest, Ctx, PackBehaviors, Pathing, Status, Tree};
use crate::server::packets::npc_packets::{self, Msg};

/// The reference's first free agent id; the bodies take the ids from it, in file order
/// (`instance.rs:181-182`).
pub const FIRST_AGENT_ID: u32 = 2;

/// The 2.00 s emerge and submerge animations. Measured, not authored: 373 transition intervals at
/// a 2.000 s median, every sample inside +/-60 ms (`table.rs:50-53`).
const BURROW_TRANSITION_MS: u32 = 2_000;

/// Where one body is in its vanish/appear cycle (`table.rs:55-66`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BurrowPhase {
    /// Created with status 0x1000; the 2.00 s emerge animation.
    Emerging,
    /// Status 0; visible and targetable for the script's `visible_ms`.
    Out,
    /// Status 0x1000; the 2.00 s submerge animation.
    Submerging,
    /// Removed from every client for the script's `hidden_ms`.
    Hidden,
}

/// One body's vanish/appear state and the terms the script chose (`table.rs:68-79`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BurrowState {
    phase: BurrowPhase,
    left_ms: u32,
    visible_ms: u32,
    hidden_ms: u32,
    /// A script owns the surface/hidden choice: the spans are commands, not a cycle.
    manual: bool,
    /// A transition a script asked for, applied by the next `advance`.
    request: Option<BurrowRequest>,
}

/// A phase change that has wire to send or a view to repair (`table.rs:81-92`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BurrowEvent {
    /// The emerge animation ended: `0x00F1 [agent, 0]`.
    Out,
    /// The visible span ended: `0x00F1 [agent, 0x1000]`.
    Submerging,
    /// The submerge animation ended: a bare `0x0021`.
    Hidden,
    /// The hidden span ended: the five-message re-create.
    Emerging,
}

impl BurrowState {
    fn new(visible_ms: u32, hidden_ms: u32) -> BurrowState {
        BurrowState {
            phase: BurrowPhase::Emerging,
            left_ms: BURROW_TRANSITION_MS,
            visible_ms,
            hidden_ms,
            manual: false,
            request: None,
        }
    }

    /// A script-owned cycle: the body starts emerging like a `Burrow` body, then waits for
    /// `ctx:burrow()`/`ctx:emerge()` instead of cycling on its own (`table.rs:106-117`). The Script
    /// leaf waits for the Lua runtime; until then the test is the only caller.
    #[allow(dead_code)]
    fn manual() -> BurrowState {
        BurrowState {
            phase: BurrowPhase::Emerging,
            left_ms: BURROW_TRANSITION_MS,
            visible_ms: 0,
            hidden_ms: 0,
            manual: true,
            request: None,
        }
    }

    /// The phase name a script reads as `ctx.phase` (`table.rs:119-127`).
    fn phase_name(self) -> &'static str {
        match self.phase {
            BurrowPhase::Emerging => "emerging",
            BurrowPhase::Out => "out",
            BurrowPhase::Submerging => "submerging",
            BurrowPhase::Hidden => "hidden",
        }
    }

    /// Record a script's request. The next `advance` applies it through the measured transitions
    /// (`table.rs:129-132`).
    fn request(&mut self, request: BurrowRequest) {
        self.request = Some(request);
    }

    /// The status word a client entering now must be created with (`table.rs:134-140`).
    fn status(self) -> u32 {
        match self.phase {
            BurrowPhase::Emerging | BurrowPhase::Submerging => npc_packets::BURROW_TRANSITION,
            BurrowPhase::Out | BurrowPhase::Hidden => 0,
        }
    }

    fn hidden(self) -> bool {
        self.phase == BurrowPhase::Hidden
    }

    /// Advance the phase clock by one tick, returning the transition it crossed, if any. A tick
    /// larger than the remaining span crosses one phase; the next tick carries the remainder
    /// (`table.rs:146-196`).
    fn advance(&mut self, elapsed_ms: u32) -> Option<BurrowEvent> {
        if let Some(request) = self.request.take() {
            match request {
                BurrowRequest::Submerge { hidden_ms } if self.phase == BurrowPhase::Out => {
                    self.phase = BurrowPhase::Submerging;
                    self.left_ms = BURROW_TRANSITION_MS;
                    self.hidden_ms = hidden_ms;
                    return Some(BurrowEvent::Submerging);
                }
                BurrowRequest::Emerge if self.phase == BurrowPhase::Hidden => {
                    self.phase = BurrowPhase::Emerging;
                    self.left_ms = BURROW_TRANSITION_MS;
                    return Some(BurrowEvent::Emerging);
                }
                _ => {}
            }
        }
        if self.manual {
            // A script decides when a surfaced body submerges. A hidden body with a positive span
            // emerges by itself when the span runs out; a zero span waits for `ctx:emerge()`
            // (`table.rs:165-173`).
            match self.phase {
                BurrowPhase::Out => return None,
                BurrowPhase::Hidden if self.hidden_ms == 0 => return None,
                _ => {}
            }
        }
        if elapsed_ms < self.left_ms {
            self.left_ms -= elapsed_ms;
            return None;
        }
        let over = elapsed_ms - self.left_ms;
        let (phase, left_ms, event) = match self.phase {
            BurrowPhase::Emerging => (BurrowPhase::Out, self.visible_ms, BurrowEvent::Out),
            BurrowPhase::Out => (
                BurrowPhase::Submerging,
                BURROW_TRANSITION_MS,
                BurrowEvent::Submerging,
            ),
            BurrowPhase::Submerging => (BurrowPhase::Hidden, self.hidden_ms, BurrowEvent::Hidden),
            BurrowPhase::Hidden => (
                BurrowPhase::Emerging,
                BURROW_TRANSITION_MS,
                BurrowEvent::Emerging,
            ),
        };
        self.phase = phase;
        self.left_ms = left_ms.saturating_sub(over);
        Some(event)
    }
}

/// What drives a body (`table.rs:199-210`).
#[derive(Debug, Clone)]
enum Mind {
    /// No behavior: the body stands where it was created.
    Still,
    /// A running tree.
    Tree(Tree),
    /// The tree succeeded; the body stays where it ended.
    Done,
    /// The tree failed, or could not be built; the body stands, and the reason was logged once.
    Parked,
}

/// One NPC body in one district (`table.rs:212-228`, without its fight state).
#[derive(Debug, Clone)]
pub struct NpcAgent {
    /// Its placement, by index into the map's list.
    placement: usize,
    /// Where it is.
    pub body: Agent,
    mind: Mind,
    /// Its vanish/appear cycle, when its behavior has one.
    burrow: Option<BurrowState>,
}

/// A client the table may tell something: a player that has entered the district and finished
/// loading, and where it stands (`table.rs:230-238`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewer {
    /// The player.
    pub player_id: u32,
    /// Where its agent is.
    pub position: (f32, f32),
}

/// What one client has been told (`table.rs:240-250`).
#[derive(Debug, Clone, Default)]
struct Sight {
    /// The definition indices this connection has been sent.
    definitions: HashSet<u32>,
    /// The bodies it has been sent a create for and no remove since.
    visible: BTreeSet<u32>,
    /// Bodies it has ever been sent a create for, so a burrow re-create knows to use the five-message
    /// burst instead of the full entry.
    created: BTreeSet<u32>,
}

/// What one tick produced (`table.rs:252-262`, without the fight's `stopped`).
#[derive(Debug, Default)]
pub struct TickOutput {
    /// The messages for each player, in the order they are to be sent.
    pub deliveries: Vec<(u32, Vec<Msg>)>,
    /// Lines for the log: a body parked by a failing tree, with its placement and the reason.
    pub notes: Vec<String>,
}

/// The tree a placement runs, built: `Ok(None)` for a body with no behavior (`table.rs:278-291`).
fn tree_for(
    placement: &NpcPlacement,
    map: &MapNpcs,
    behaviors: &dyn BehaviorSource,
) -> Result<Option<Tree>, String> {
    let Some(name) = &placement.behavior else {
        return Ok(None);
    };
    let spec = behaviors
        .spec(name)
        .ok_or_else(|| format!("behavior {name} is not one the behavior source can build"))?;
    Tree::build(&spec, placement.route.as_deref(), &map.routes).map(Some)
}

/// Build every placement's tree once, so an unknown behavior or a construction error is named before
/// a body parks on its first district (`table.rs:293-310`). The walk is the same `tree_for` the seed
/// uses; a pack whose Lua source is not loaded yet reports its script behaviors here.
pub fn check_trees(content: &NpcContent, behaviors: &dyn BehaviorSource) -> Vec<String> {
    let mut maps: Vec<&Arc<MapNpcs>> = content.maps.values().collect();
    maps.sort_by_key(|map| map.map_id);
    let mut problems = Vec::new();
    for map in maps {
        for placement in &map.placements {
            if let Err(err) = tree_for(placement, map, behaviors) {
                problems.push(format!(
                    "map {} placement {}: {err}",
                    map.map_id, placement.id
                ));
            }
        }
    }
    problems
}

/// `msgs` with the body's pace in front of each leg, for a placement that states a `walk_speed`
/// (`table.rs:312-325`).
fn paced(msgs: Vec<Msg>, agent_id: u32, walk_speed: Option<f32>) -> Vec<Msg> {
    let Some(speed) = walk_speed else {
        return msgs;
    };
    let mut out = Vec::with_capacity(msgs.len() + 1);
    for msg in msgs {
        if matches!(msg, Msg::MoveTo { .. }) {
            out.push(Msg::Pace { agent_id, speed });
        }
        out.push(msg);
    }
    out
}

/// One district's NPCs. The default is a table with no bodies: a map the pack has no NPCs for
/// (`table.rs:264-276`).
#[derive(Debug, Clone, Default)]
pub struct NpcTable {
    map: Arc<MapNpcs>,
    rules: SightRules,
    agents: BTreeMap<u32, NpcAgent>,
    by_placement: HashMap<String, u32>,
    sights: HashMap<u32, Sight>,
}

impl NpcTable {
    /// Create every body of a map, in its file's order, under consecutive agent ids from `first_id`
    /// (`table.rs:327-397`). A tree that does not build parks its body and says why in the returned
    /// notes; the pack check at load ([`check_trees`]) is what keeps that from happening.
    pub fn seed(
        map: Arc<MapNpcs>,
        rules: SightRules,
        behaviors: &dyn BehaviorSource,
        first_id: u32,
    ) -> (NpcTable, Vec<String>) {
        let mut table = NpcTable {
            rules,
            ..NpcTable::default()
        };
        let mut notes = Vec::new();
        for (index, placement) in map.placements.iter().enumerate() {
            let agent_id = first_id + index as u32;
            let (mind, burrow) = match tree_for(placement, &map, behaviors) {
                Ok(None) => (Mind::Still, None),
                Ok(Some(tree)) => {
                    let burrow = tree
                        .burrow_terms()
                        .map(|(visible_ms, hidden_ms)| BurrowState::new(visible_ms, hidden_ms));
                    (Mind::Tree(tree), burrow)
                }
                Err(err) => {
                    notes.push(format!(
                        "map {} placement {}: parked: {err}",
                        map.map_id, placement.id
                    ));
                    (Mind::Parked, None)
                }
            };
            let body = Agent::new(
                agent_id,
                placement.position,
                placement.plane,
                placement.direction,
                placement.walk_speed.unwrap_or(placement.definition.speed),
            );
            table.agents.insert(
                agent_id,
                NpcAgent {
                    placement: index,
                    body,
                    mind,
                    burrow,
                },
            );
            table.by_placement.insert(placement.id.clone(), agent_id);
        }
        table.map = map;
        (table, notes)
    }

    /// Whether `agent_id` is one of this table's bodies (`table.rs:399-402`).
    pub fn holds(&self, agent_id: u32) -> bool {
        self.agents.contains_key(&agent_id)
    }

    /// Whether this client has been sent `agent_id`'s create and not its remove (`table.rs:404-411`).
    pub fn knows(&self, player_id: u32, agent_id: u32) -> bool {
        self.sights
            .get(&player_id)
            .is_some_and(|sight| sight.visible.contains(&agent_id))
    }

    /// How many bodies the district has (`table.rs:413-416`).
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Whether the district has no bodies (`table.rs:418-421`).
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// The agent a placement became in this district: how a script names a body (`table.rs:423-426`).
    pub fn agent_of(&self, placement: &str) -> Option<u32> {
        self.by_placement.get(placement).copied()
    }

    /// One body, by agent id (`table.rs:443-446`).
    pub fn agent(&self, agent_id: u32) -> Option<&NpcAgent> {
        self.agents.get(&agent_id)
    }

    /// The placement a body was created from (`table.rs:448-452`).
    pub fn placement_of(&self, agent_id: u32) -> Option<&NpcPlacement> {
        let agent = self.agents.get(&agent_id)?;
        self.map.placements.get(agent.placement)
    }

    /// Where every body stands now (`table.rs:470-476`).
    pub fn positions(&self) -> Vec<(u32, (f32, f32))> {
        self.agents
            .iter()
            .map(|(agent_id, agent)| (*agent_id, agent.body.position))
            .collect()
    }

    /// The burst that makes one body appear on one client, with its current walk when it is walking,
    /// and the client's record of it (`table.rs:639-672`, without the fight's dead bit).
    fn enter(map: &MapNpcs, sight: &mut Sight, agent: &NpcAgent) -> Vec<Msg> {
        let agent_id = agent.body.agent_id;
        let placement = &map.placements[agent.placement];
        let known = !sight
            .definitions
            .insert(placement.definition.definition_index);
        sight.visible.insert(agent_id);
        sight.created.insert(agent_id);
        let status = agent
            .burrow
            .map_or(placement.effects, |burrow| burrow.status());
        let mut msgs = npc_packets::entry_with_status(placement, &agent.body, known, status);
        if 0.0 < agent.body.motion.speed {
            let to = agent.body.motion.destination;
            msgs.push(Msg::MoveTo {
                agent_id,
                to: (to.x, to.y),
                plane: to.plane,
                current_plane: agent.body.plane,
            });
        }
        paced(msgs, agent_id, placement.walk_speed)
    }

    /// Bring one client's record up to where it stands: a remove for each body that left its range,
    /// and an entry burst, by agent id, for each that came into it (`table.rs:674-702`).
    fn look(&mut self, viewer: Viewer) -> (Vec<Msg>, Vec<(u32, Vec<Msg>)>) {
        let sight = self.sights.entry(viewer.player_id).or_default();
        let wanted: BTreeSet<u32> = (self.agents.iter())
            .filter(|(_, agent)| {
                !agent.burrow.is_some_and(BurrowState::hidden)
                    && self.rules.sees(viewer.position, agent.body.position)
            })
            .map(|(agent_id, _)| *agent_id)
            .collect();
        let leaving: Vec<u32> = sight.visible.difference(&wanted).copied().collect();
        let entering: Vec<u32> = wanted.difference(&sight.visible).copied().collect();
        let removes = leaving
            .into_iter()
            .map(|agent_id| {
                sight.visible.remove(&agent_id);
                Msg::Remove { agent_id }
            })
            .collect();
        let entries = entering
            .into_iter()
            .map(|agent_id| {
                let burst = Self::enter(&self.map, sight, &self.agents[&agent_id]);
                (agent_id, burst)
            })
            .collect();
        (removes, entries)
    }

    /// A client is loading into the district: the bodies within its range, each as its entry burst, by
    /// agent id. Its record starts empty, so the first body of each definition carries the definition
    /// (`table.rs:704-713`).
    pub fn arrive(&mut self, player_id: u32, position: (f32, f32)) -> Vec<(u32, Vec<Msg>)> {
        self.sights.remove(&player_id);
        self.look(Viewer {
            player_id,
            position,
        })
        .1
    }

    /// A client has left: forget what it was told (`table.rs:715-718`).
    pub fn forget(&mut self, player_id: u32) {
        self.sights.remove(&player_id);
    }

    /// One viewer's part of a tick: the bodies that left its range, then the ones that came into it
    /// (`table.rs:871-873`). The bodies' walks are [`Self::tick`]: this is only the view.
    pub fn sweep(&mut self, player_id: u32, position: (f32, f32)) -> Vec<Msg> {
        let (mut msgs, entries) = self.look(Viewer {
            player_id,
            position,
        });
        msgs.extend(entries.into_iter().flat_map(|(_, burst)| burst));
        msgs
    }

    /// One tick: advance and think for every active body, then tell each viewer what changed - the
    /// walks of the bodies it can see, a remove for each body that left its range, and an entry burst
    /// for each that came into it (`table.rs:720-879`, without the fight and the `/npc` command).
    pub fn tick(&mut self, delta: f32, viewers: &[Viewer], pathing: &dyn Pathing) -> TickOutput {
        let mut output = TickOutput::default();
        if self.agents.is_empty() {
            return output;
        }
        let elapsed_ms = (delta * 1000.0).round() as u32;

        // What every active body did.
        let mut said: Vec<(u32, Vec<Msg>)> = Vec::new();
        let mut removed: Vec<u32> = Vec::new();
        let mut emerged: Vec<u32> = Vec::new();
        for (agent_id, agent) in self.agents.iter_mut() {
            let position = agent.body.position;
            if !viewers
                .iter()
                .any(|viewer| self.rules.sees(viewer.position, position))
            {
                continue;
            }
            let placement = &self.map.placements[agent.placement];
            let mut out = Vec::new();
            if let Some(waypoint) = agent.body.advance(delta) {
                out.push(Msg::MoveTo {
                    agent_id: *agent_id,
                    to: (waypoint.pos.x, waypoint.pos.y),
                    plane: waypoint.pos.plane,
                    current_plane: agent.body.plane,
                });
            }
            // A burrower's phase clock runs while the body is active (`table.rs:757-777`).
            if let Some(event) = agent
                .burrow
                .as_mut()
                .and_then(|burrow| burrow.advance(elapsed_ms))
            {
                match event {
                    BurrowEvent::Out => out.push(Msg::Status {
                        agent_id: *agent_id,
                        status: 0,
                    }),
                    BurrowEvent::Submerging => out.push(Msg::Status {
                        agent_id: *agent_id,
                        status: npc_packets::BURROW_TRANSITION,
                    }),
                    BurrowEvent::Hidden => removed.push(*agent_id),
                    BurrowEvent::Emerging => emerged.push(*agent_id),
                }
            }
            if let Mind::Tree(tree) = &mut agent.mind {
                let burrow_phase = agent.burrow.as_ref().map(|burrow| burrow.phase_name());
                let mut ctx = Ctx {
                    body: &mut agent.body,
                    map_id: self.map.map_id,
                    elapsed_ms,
                    pathing,
                    out: &mut out,
                    failure: None,
                    in_combat: false,
                    burrow_phase,
                    burrow_request: None,
                };
                match tree.tick(&mut ctx) {
                    Status::Running => {}
                    Status::Success => agent.mind = Mind::Done,
                    Status::Failure => {
                        let reason = ctx.failure.take().unwrap_or_else(|| "no reason".into());
                        output.notes.push(format!(
                            "map {} placement {}: parked: {reason}",
                            self.map.map_id, placement.id
                        ));
                        agent.mind = Mind::Parked;
                    }
                }
                if let Some(request) = ctx.burrow_request.take()
                    && let Some(burrow) = agent.burrow.as_mut()
                {
                    burrow.request(request);
                }
            }
            if !out.is_empty() {
                said.push((*agent_id, paced(out, *agent_id, placement.walk_speed)));
            }
        }

        // Each viewer: what its visible bodies did, then the bodies that left, then the ones that
        // came (`table.rs:830-878`).
        for viewer in viewers {
            let seen = self.sights.entry(viewer.player_id).or_default();
            let mut gone = Vec::new();
            // A submerging body is removed after its 2.00 s animation; the remove belongs to the
            // viewers that saw the animation, not to the range sweep.
            for agent_id in &removed {
                if seen.visible.remove(agent_id) {
                    gone.push(Msg::Remove {
                        agent_id: *agent_id,
                    });
                }
            }
            // A hidden body re-appears with the five-message burst Rurik measured, sent only to a
            // client that was already told this body once. A client that has not is handled by the
            // range sweep below, which sends the ordinary entry with the emerge status.
            for agent_id in &emerged {
                if !seen.created.contains(agent_id)
                    || seen.visible.contains(agent_id)
                    || !self
                        .rules
                        .sees(viewer.position, self.agents[agent_id].body.position)
                {
                    continue;
                }
                let agent = &self.agents[agent_id];
                let placement = &self.map.placements[agent.placement];
                gone.extend(npc_packets::burrow_recreate(placement, &agent.body));
                seen.visible.insert(*agent_id);
            }
            let mut msgs: Vec<Msg> = said
                .iter()
                .filter(|(agent_id, _)| seen.visible.contains(agent_id))
                .flat_map(|(_, out)| out.iter().cloned())
                .collect();
            msgs.extend(gone);
            let (removes, entries) = self.look(*viewer);
            msgs.extend(removes);
            msgs.extend(entries.into_iter().flat_map(|(_, burst)| burst));
            if !msgs.is_empty() {
                output.deliveries.push((viewer.player_id, msgs));
            }
        }
        output
    }
}

/// One running copy's agents: its NPC table and the agent ids its players hold. ASURA keeps both in
/// its `Instance` (`instance.rs:146-165`).
#[derive(Debug, Default)]
struct Running {
    npcs: NpcTable,
    players: BTreeSet<u32>,
    /// The entered players' positions, as the tree tick's viewers.
    viewers: BTreeMap<u32, (f32, f32)>,
    /// When the bodies last thought: the copy's own clock, not a connection's.
    last_tick: Option<Instant>,
    /// Frames a body tick addressed to one client, waiting for that client's next sweep.
    pending: HashMap<u32, Vec<Vec<u8>>>,
}

/// Every running copy's NPCs and agent ids, keyed by map and district.
pub struct NpcDistricts {
    copies: HashMap<(u32, u32), Running>,
    /// Where behavior trees come from (`table.rs:26-48`). The built-in source knows `patrol`; the
    /// Lua source that will own `worm_lakeside` and `worm_queen` is not built yet.
    behaviors: Arc<dyn BehaviorSource + Send + Sync>,
    /// Lines for the log from a seed or a tick, drained by [`Self::tick`].
    notes: Vec<String>,
}

impl std::fmt::Debug for NpcDistricts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NpcDistricts")
            .field("copies", &self.copies)
            .field("notes", &self.notes)
            .finish_non_exhaustive()
    }
}

impl Default for NpcDistricts {
    fn default() -> Self {
        Self::new()
    }
}

impl NpcDistricts {
    /// No copies yet, with the pack's built-in behaviors.
    pub fn new() -> Self {
        Self::with_behaviors(Arc::new(PackBehaviors))
    }

    /// No copies yet, with a chosen behavior source.
    pub fn with_behaviors(behaviors: Arc<dyn BehaviorSource + Send + Sync>) -> Self {
        NpcDistricts {
            copies: HashMap::new(),
            behaviors,
            notes: Vec::new(),
        }
    }

    /// The tree problems of every placement, for a start-up report (`table.rs:293-310`).
    pub fn check_trees(&self, content: &NpcContent) -> Vec<String> {
        check_trees(content, self.behaviors.as_ref())
    }

    /// The copy, created with its map's bodies the first time anything asks for it - which is a
    /// player's agent id, so the bodies always exist before any player (`instance.rs:178-205`, R29).
    fn copy(&mut self, map_id: u32, district: u32, content: &NpcContent) -> &mut Running {
        let behaviors = Arc::clone(&self.behaviors);
        let mut notes = Vec::new();
        let copy = self.copies.entry((map_id, district)).or_insert_with(|| {
            let (npcs, seed_notes) = match content.map(map_id as i32) {
                Some(map) => NpcTable::seed(map, content.sight, behaviors.as_ref(), FIRST_AGENT_ID),
                None => (
                    NpcTable {
                        rules: content.sight,
                        ..NpcTable::default()
                    },
                    Vec::new(),
                ),
            };
            notes = seed_notes;
            Running {
                npcs,
                ..Running::default()
            }
        });
        self.notes.extend(notes);
        copy
    }

    /// A joining player's agent id in this copy: the lowest id no body and no other player holds
    /// (`instance.rs:307-316`).
    pub fn allocate(&mut self, map_id: u32, district: u32, content: &NpcContent) -> u32 {
        let copy = self.copy(map_id, district, content);
        let agent_id = (1..)
            .find(|id| !copy.players.contains(id) && !copy.npcs.holds(*id))
            .unwrap_or(1);
        copy.players.insert(agent_id);
        agent_id
    }

    /// A player left the copy: its agent id is free again and what it was told is forgotten
    /// (`instance.rs:422-424`).
    pub fn release(&mut self, map_id: u32, district: u32, player_id: u32, agent_id: u32) {
        if let Some(copy) = self.copies.get_mut(&(map_id, district)) {
            copy.players.remove(&agent_id);
            copy.viewers.remove(&player_id);
            copy.pending.remove(&player_id);
            copy.npcs.forget(player_id);
        }
    }

    /// An agent id for something the world creates that is not a joining avatar - a dropped item's
    /// ground agent. The id is the lowest no body, player or other ground item holds, and it is
    /// reserved in the same set a player's id is, so the two cannot collide
    /// (`ASURA/crates/asura-world/src/instance.rs:428-448`, `allocate_agent_id`).
    pub fn allocate_other(&mut self, map_id: u32, district: u32, content: &NpcContent) -> u32 {
        let copy = self.copy(map_id, district, content);
        let agent_id = (1..)
            .find(|id| !copy.players.contains(id) && !copy.npcs.holds(*id))
            .unwrap_or(1);
        copy.players.insert(agent_id);
        agent_id
    }

    /// A ground item left the copy - picked up - so its agent id is free again. Unlike a player's
    /// release, nothing this copy taught a client is forgotten: a ground item was never a viewer.
    pub fn release_other(&mut self, map_id: u32, district: u32, agent_id: u32) {
        if let Some(copy) = self.copies.get_mut(&(map_id, district)) {
            copy.players.remove(&agent_id);
        }
    }

    /// The NPC bodies within a loading client's range, each as the frames of its entry burst, by agent
    /// id - the NPC half of the load's agent array (`instance.rs:450-460`).
    pub fn arrival(
        &mut self,
        map_id: u32,
        district: u32,
        content: &NpcContent,
        player_id: u32,
        position: (f32, f32),
    ) -> Vec<(u32, Vec<Vec<u8>>)> {
        let copy = self.copy(map_id, district, content);
        copy.viewers.insert(player_id, position);
        // A copy nobody was in did not tick. Starting its clock here means the first body tick after
        // a return advances one period, not the whole absence: ASURA's dormant bodies do the same.
        copy.last_tick = Some(Instant::now());
        copy.npcs
            .arrive(player_id, position)
            .into_iter()
            .map(|(agent_id, msgs)| (agent_id, npc_packets::frames(&msgs)))
            .collect()
    }

    /// Record where a client stands and advance the copy's bodies when a period has passed since the
    /// last body tick. The first client to tick in a period advances them for everyone; the others
    /// only record their own position, so two connections do not run the town twice as fast.
    ///
    /// The walked bodies' messages are queued per player and drained by [`Self::sweep`], which is the
    /// per-connection half of ASURA's `TickOutput.deliveries` (`table.rs:830-877`).
    pub fn tick(
        &mut self,
        key: (u32, u32),
        player_id: u32,
        position: (f32, f32),
        now: Instant,
        period: Duration,
        pathing: &dyn Pathing,
    ) -> Vec<String> {
        let mut notes = std::mem::take(&mut self.notes);
        let Some(copy) = self.copies.get_mut(&key) else {
            return notes;
        };
        copy.viewers.insert(player_id, position);
        if copy.npcs.is_empty() {
            return notes;
        }
        let due = match copy.last_tick {
            None => {
                copy.last_tick = Some(now);
                false
            }
            Some(last) => now.saturating_duration_since(last) >= period,
        };
        if !due {
            return notes;
        }
        let last = copy.last_tick.replace(now).unwrap_or(now);
        // A late caller (a stalled connection, or a load that outlasted the period) advances one
        // period, never the whole gap: a body must not jump down its route because nobody ticked.
        let delta = now
            .saturating_duration_since(last)
            .min(period)
            .as_secs_f32();
        let viewers: Vec<Viewer> = copy
            .viewers
            .iter()
            .map(|(player_id, position)| Viewer {
                player_id: *player_id,
                position: *position,
            })
            .collect();
        let out = copy.npcs.tick(delta, &viewers, pathing);
        notes.extend(out.notes);
        for (player_id, msgs) in out.deliveries {
            copy.pending
                .entry(player_id)
                .or_default()
                .extend(npc_packets::frames(&msgs));
        }
        notes
    }

    /// One client's part of a tick, as frames: the body messages queued for it, then the bodies that
    /// left its range and the ones that came into it (`table.rs:865-873`). Empty for a copy that is
    /// not running.
    pub fn sweep(
        &mut self,
        map_id: u32,
        district: u32,
        player_id: u32,
        position: (f32, f32),
    ) -> Vec<Vec<u8>> {
        match self.copies.get_mut(&(map_id, district)) {
            Some(copy) => {
                copy.viewers.insert(player_id, position);
                let mut frames = copy.pending.remove(&player_id).unwrap_or_default();
                frames.extend(npc_packets::frames(&copy.npcs.sweep(player_id, position)));
                frames
            }
            None => Vec::new(),
        }
    }

    /// How many bodies a running copy has.
    pub fn count(&self, map_id: u32, district: u32) -> usize {
        self.copies
            .get(&(map_id, district))
            .map_or(0, |copy| copy.npcs.len())
    }

    /// The copy closed: its bodies go with it (`instance.rs:146-165`, an instance drops its table).
    pub fn close(&mut self, map_id: u32, district: u32) {
        self.copies.remove(&(map_id, district));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::npc_content::RouteMode;
    use crate::agents::npc_content::tests::{definition, map, placement, sight};
    use crate::behaviour::Spec;

    fn content() -> NpcContent {
        NpcContent::assemble(
            sight(),
            vec![definition("guard", 0x5B3), definition("cook", 0x5C8)],
            vec![map(
                148,
                vec![
                    placement("a", "guard", (0.0, 0.0)),
                    placement("b", "guard", (100.0, 0.0)),
                    placement("c", "cook", (9000.0, 0.0)),
                ],
            )],
        )
        .expect("assembles")
    }

    fn town() -> NpcTable {
        NpcTable::seed(
            content().map(148).expect("map"),
            sight(),
            &PackBehaviors,
            41,
        )
        .0
    }

    fn headers(msgs: &[Msg]) -> Vec<&'static str> {
        msgs.iter()
            .map(|msg| match msg {
                Msg::Definition(_) => "def",
                Msg::Model(_) => "model",
                Msg::Create(_) => "create",
                Msg::Remove { .. } => "remove",
                Msg::MoveTo { .. } => "move",
                _ => "-",
            })
            .filter(|name| *name != "-")
            .collect()
    }

    // `table.rs:1046-1055`.
    #[test]
    fn bodies_take_ids_in_file_order_and_are_found_by_placement() {
        let table = town();
        assert_eq!(table.len(), 3);
        assert_eq!(table.agent_of("a"), Some(41));
        assert_eq!(table.agent_of("c"), Some(43));
        assert!(table.holds(42) && !table.holds(44));
        assert_eq!(table.placement_of(42).map(|p| p.id.as_str()), Some("b"));
        assert!(table.placement_of(44).is_none());
    }

    // `table.rs:1057-1065`.
    #[test]
    fn an_arriving_client_gets_each_definition_once_before_its_first_body() {
        let mut table = town();
        let bursts = table.arrive(1, (0.0, 0.0));
        let ids: Vec<u32> = bursts.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, vec![41, 42], "the cook is out of range");
        assert_eq!(headers(&bursts[0].1), vec!["def", "model", "create"]);
        assert_eq!(headers(&bursts[1].1), vec!["create"], "guard already known");
    }

    // `table.rs:1067-1091`, the sight half.
    #[test]
    fn walking_into_range_creates_and_walking_out_removes() {
        let mut table = town();
        table.arrive(1, (0.0, 0.0));
        let msgs = table.sweep(1, (8000.0, 0.0));
        assert_eq!(
            headers(&msgs),
            vec!["remove", "remove", "def", "model", "create"],
            "both guards leave, the cook arrives with its definition: {msgs:?}"
        );
        let msgs = table.sweep(1, (0.0, 0.0));
        assert_eq!(
            headers(&msgs),
            vec!["remove", "create", "create"],
            "the guards come back without their definition"
        );
        assert!(table.sweep(1, (0.0, 0.0)).is_empty(), "nothing changed");
    }

    // `table.rs:1093-1102`.
    #[test]
    fn two_clients_have_separate_records_and_a_forgotten_one_starts_over() {
        let mut table = town();
        table.arrive(1, (0.0, 0.0));
        let second = table.arrive(2, (0.0, 0.0));
        assert_eq!(headers(&second[0].1), vec!["def", "model", "create"]);
        table.forget(2);
        let again = table.arrive(2, (0.0, 0.0));
        assert_eq!(headers(&again[0].1), vec!["def", "model", "create"]);
    }

    /// `instance.rs:180-205` and `:307-316`: bodies from 2, players at the lowest free id.
    #[test]
    fn players_take_the_lowest_id_no_body_holds_and_bodies_come_first() {
        let content = content();
        let mut copies = NpcDistricts::new();
        assert_eq!(copies.allocate(148, 1, &content), 1);
        assert_eq!(copies.count(148, 1), 3, "seeded before the first player");
        assert_eq!(copies.allocate(148, 1, &content), 5, "2-4 are the bodies");
        copies.release(148, 1, 7, 1);
        assert_eq!(copies.allocate(148, 1, &content), 1, "a freed id is reused");
        assert_eq!(
            copies.allocate(148, 2, &content),
            1,
            "another copy has its own ids"
        );
        assert_eq!(copies.allocate(149, 1, &content), 1, "a map without NPCs");
        assert_eq!(copies.allocate(149, 1, &content), 2);
        copies.close(148, 1);
        assert_eq!(copies.count(148, 1), 0);
    }

    #[test]
    fn an_arrival_is_frames_in_agent_id_order() {
        let content = content();
        let mut copies = NpcDistricts::new();
        copies.allocate(148, 1, &content);
        let arrival = copies.arrival(148, 1, &content, 1, (0.0, 0.0));
        let ids: Vec<u32> = arrival.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, vec![2, 3]);
        let headers: Vec<u16> = arrival[0]
            .1
            .iter()
            .map(|frame| u16::from_le_bytes([frame[0], frame[1]]))
            .collect();
        assert_eq!(headers, vec![0x56, 0x57, 0x9B, 0xF0, 0x20, 0x6D]);
    }

    fn patrolling(behaviors: &dyn BehaviorSource) -> (NpcTable, Vec<String>) {
        let mut walker = placement("w", "guard", (0.0, 0.0));
        walker.behavior = Some("patrol".to_string());
        walker.route = Some("square".to_string());
        let mut file = map(148, vec![walker]);
        file.routes
            .push(crate::behaviour::tests::square(RouteMode::Loop, 0));
        let content = NpcContent::assemble(sight(), vec![definition("guard", 0x5B3)], vec![file])
            .expect("assembles");
        NpcTable::seed(content.map(148).expect("map"), sight(), behaviors, 9)
    }

    /// `table.rs:1143-1168`.
    #[test]
    fn a_patrol_moves_only_while_someone_is_in_range() {
        let (mut table, notes) = patrolling(&PackBehaviors);
        assert!(notes.is_empty());
        table.arrive(1, (0.0, 0.0));
        let near = Viewer {
            player_id: 1,
            position: (0.0, 0.0),
        };
        let out = table.tick(0.0, &[near], &crate::behaviour::tests::Flat);
        assert_eq!(headers(&out.deliveries[0].1), vec!["move"]);
        table.tick(0.5, &[near], &crate::behaviour::tests::Flat);
        let walked = table.agent(9).expect("body").body.position;
        assert_eq!(walked, (100.0, 0.0), "the first leg ends at its point");
        let far = Viewer {
            player_id: 1,
            position: (20000.0, 0.0),
        };
        let out = table.tick(0.5, &[far], &crate::behaviour::tests::Flat);
        assert_eq!(headers(&out.deliveries[0].1), vec!["remove"]);
        assert_eq!(
            table.agent(9).expect("body").body.position,
            walked,
            "dormant: nobody in range, nothing moved"
        );
    }

    /// `table.rs:1203-1217`.
    #[test]
    fn a_body_that_enters_mid_walk_is_created_with_its_walk() {
        let (mut table, _) = patrolling(&PackBehaviors);
        let watcher = Viewer {
            player_id: 1,
            position: (0.0, 0.0),
        };
        table.arrive(1, watcher.position);
        table.tick(0.0, &[watcher], &crate::behaviour::tests::Flat);
        let bursts = table.arrive(2, (0.0, 0.0));
        assert_eq!(
            headers(&bursts[0].1),
            vec!["def", "model", "create", "move"]
        );
    }

    /// `table.rs:1219-1231`.
    #[test]
    fn a_tree_that_cannot_be_built_parks_its_body_and_is_caught_at_load() {
        struct NoBehaviors;
        impl BehaviorSource for NoBehaviors {
            fn spec(&self, _name: &str) -> Option<Spec> {
                None
            }
        }
        let (table, notes) = patrolling(&NoBehaviors);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("placement w: parked"), "{notes:?}");
        assert_eq!(table.len(), 1, "the body still exists and stands");
        let mut walker = placement("w", "guard", (0.0, 0.0));
        walker.behavior = Some("patrol".to_string());
        let content = NpcContent::assemble(
            sight(),
            vec![definition("guard", 0x5B3)],
            vec![map(148, vec![walker])],
        )
        .expect("assembles");
        let problems = check_trees(&content, &PackBehaviors);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("names no route"), "{problems:?}");
    }

    /// `table.rs:1233-1254`.
    #[test]
    fn a_failing_leg_parks_the_body_and_says_so_once() {
        let mut walker = placement("w", "guard", (0.0, 0.0));
        walker.behavior = Some("patrol".to_string());
        walker.route = Some("square".to_string());
        let mut file = map(148, vec![walker]);
        let mut route = crate::behaviour::tests::square(RouteMode::Loop, 0);
        route.points[0].position = (-1.0, 0.0);
        file.routes.push(route);
        let content = NpcContent::assemble(sight(), vec![definition("guard", 0x5B3)], vec![file])
            .expect("assembles");
        let (mut table, _) =
            NpcTable::seed(content.map(148).expect("map"), sight(), &PackBehaviors, 9);
        let near = Viewer {
            player_id: 1,
            position: (0.0, 0.0),
        };
        let out = table.tick(0.1, &[near], &crate::behaviour::tests::Flat);
        assert_eq!(out.notes.len(), 1, "{:?}", out.notes);
        assert!(out.notes[0].contains("no path"));
        let out = table.tick(0.1, &[near], &crate::behaviour::tests::Flat);
        assert!(out.notes.is_empty(), "parked once, not every tick");
    }

    /// `table.rs:1021-1044`, the cycle a `Burrow` spec drives.
    #[test]
    fn a_burrow_cycles_through_the_measured_transitions() {
        let mut burrow = BurrowState::new(3000, 4000);
        assert_eq!(burrow.phase_name(), "emerging");
        assert_eq!(burrow.status(), npc_packets::BURROW_TRANSITION);
        assert_eq!(burrow.advance(1999), None);
        assert_eq!(burrow.advance(1), Some(BurrowEvent::Out));
        assert_eq!(burrow.phase_name(), "out");
        assert_eq!(burrow.status(), 0);
        assert_eq!(burrow.advance(3000), Some(BurrowEvent::Submerging));
        assert_eq!(burrow.advance(2000), Some(BurrowEvent::Hidden));
        assert!(burrow.hidden());
        assert_eq!(burrow.advance(4000), Some(BurrowEvent::Emerging));
        assert_eq!(burrow.phase_name(), "emerging");

        // A script-owned cycle waits for its commands (`table.rs:1021-1044`).
        let mut manual = BurrowState::manual();
        assert_eq!(manual.advance(2000), Some(BurrowEvent::Out));
        assert_eq!(manual.advance(10_000), None, "a script owns the surface");
        manual.request(BurrowRequest::Submerge { hidden_ms: 0 });
        assert_eq!(manual.advance(0), Some(BurrowEvent::Submerging));
        assert_eq!(manual.advance(2000), Some(BurrowEvent::Hidden));
        assert_eq!(manual.advance(60_000), None, "0 waits for emerge");
        manual.request(BurrowRequest::Emerge);
        assert_eq!(manual.advance(0), Some(BurrowEvent::Emerging));
    }

    #[test]
    fn the_shipped_packs_patrols_build_and_only_its_lua_scripts_wait() {
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the crate is in the repository")
            .join("config")
            .join("World")
            .join("Vanilla");
        let content = crate::agents::npc_file::load(&pack, sight()).expect("the pack loads");
        let problems = check_trees(&content, &PackBehaviors);
        assert_eq!(problems.len(), 19, "{problems:?}");
        assert!(
            problems
                .iter()
                .all(|problem| problem.contains("behavior worm_")),
            "only the Lua worm scripts wait: {problems:?}"
        );
        assert!(
            !problems.iter().any(|problem| problem.contains("map 148")),
            "both Ascalon patrols build: {problems:?}"
        );
    }

    #[test]
    fn two_connections_do_not_tick_the_town_twice_as_fast() {
        let mut walker = placement("w", "guard", (0.0, 0.0));
        walker.behavior = Some("patrol".to_string());
        walker.route = Some("square".to_string());
        let mut file = map(148, vec![walker]);
        file.routes
            .push(crate::behaviour::tests::square(RouteMode::Loop, 0));
        let content = NpcContent::assemble(sight(), vec![definition("guard", 0x5B3)], vec![file])
            .expect("assembles");
        let mut copies = NpcDistricts::with_behaviors(Arc::new(PackBehaviors));
        copies.allocate(148, 1, &content);
        copies.arrival(148, 1, &content, 1, (0.0, 0.0));
        copies.arrival(148, 1, &content, 2, (0.0, 0.0));
        let start = Instant::now();
        let period = Duration::from_millis(100);
        copies.tick(
            (148, 1),
            1,
            (0.0, 0.0),
            start,
            period,
            &crate::behaviour::tests::Flat,
        );
        let notes = copies.tick(
            (148, 1),
            1,
            (0.0, 0.0),
            start + period,
            period,
            &crate::behaviour::tests::Flat,
        );
        assert!(notes.is_empty());
        let second = copies.tick(
            (148, 1),
            2,
            (0.0, 0.0),
            start + period,
            period,
            &crate::behaviour::tests::Flat,
        );
        assert!(second.is_empty());
        for player in [1, 2] {
            let frames = copies.sweep(148, 1, player, (0.0, 0.0));
            assert_eq!(frames.len(), 1, "{frames:?}");
            assert_eq!(
                u16::from_le_bytes([frames[0][0], frames[0][1]]),
                0x29,
                "the first body tick starts the leg"
            );
        }
    }
}
