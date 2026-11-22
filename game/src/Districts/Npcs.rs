//! One district's NPCs: the bodies, the agent ids its copy hands out, and what each client has been
//! told about them.
//!
//! Ported from `ASURA/crates/asura-world/src/npc/table.rs` (1,255 lines, the local repository as it
//! stands on 2026-11-22) and the NPC half of `ASURA/crates/asura-world/src/instance.rs` (`with_npcs`
//! 178-225, `join`'s agent id 307-316, `npc_arrival` 450-460, `leave` 424). ASURA's instance is this
//! tree's district, so the table lives beside the other district state.
//!
//! **Ported thin** (stage N3 of `docs/asura-port-plan.md`): the population, the agent ids, the load's
//! NPC bursts and the per-client sight with compass churn. Every body stands where it was created.
//! Not carried yet: the behaviour trees (`Mind::Tree`, `check_trees`, `BehaviorSource`, stage N4), the
//! burrow cycle (`table.rs:50-197`), the fight (`fight`, `obey`, `combatants`), the `/npc` command
//! (`table.rs:531-637`) and the walk half of `tick` (`table.rs:723-822`): with no behaviour nothing
//! walks, so a tick here is only the sight sweep (`table.rs:871-873`).
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

use crate::agents::Agent;
use crate::agents::npc_content::{MapNpcs, NpcContent, NpcPlacement, SightRules};
use crate::server::packets::npc_packets::{self, Msg};

/// The reference's first free agent id; the bodies take the ids from it, in file order
/// (`instance.rs:181-182`).
pub const FIRST_AGENT_ID: u32 = 2;

/// One NPC body in one district (`table.rs:212-228`, without its mind, burrow and fight state).
#[derive(Debug, Clone)]
pub struct NpcAgent {
    /// Its placement, by index into the map's list.
    placement: usize,
    /// Where it is.
    pub body: Agent,
}

/// What one client has been told (`table.rs:240-250`, without the burrow's `created`).
#[derive(Debug, Clone, Default)]
struct Sight {
    /// The definition indices this connection has been sent.
    definitions: HashSet<u32>,
    /// The bodies it has been sent a create for and no remove since.
    visible: BTreeSet<u32>,
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
    /// (`table.rs:327-397`). A placement with a behaviour stands until stage N4 ports the trees.
    pub fn seed(map: Arc<MapNpcs>, rules: SightRules, first_id: u32) -> NpcTable {
        let mut table = NpcTable {
            rules,
            ..NpcTable::default()
        };
        for (index, placement) in map.placements.iter().enumerate() {
            let agent_id = first_id + index as u32;
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
                },
            );
            table.by_placement.insert(placement.id.clone(), agent_id);
        }
        table.map = map;
        table
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
    /// and the client's record of it (`table.rs:639-672`, without the burrow status and the dead bit,
    /// which arrive with their subjects).
    fn enter(map: &MapNpcs, sight: &mut Sight, agent: &NpcAgent) -> Vec<Msg> {
        let agent_id = agent.body.agent_id;
        let placement = &map.placements[agent.placement];
        let known = !sight
            .definitions
            .insert(placement.definition.definition_index);
        sight.visible.insert(agent_id);
        let mut msgs = npc_packets::entry(placement, &agent.body, known);
        if 0.0 < agent.body.motion.speed {
            let to = agent.body.motion.destination;
            msgs.push(Msg::MoveTo {
                agent_id,
                to: (to.x, to.y),
                plane: to.plane,
                current_plane: agent.body.plane,
            });
        }
        msgs
    }

    /// Bring one client's record up to where it stands: a remove for each body that left its range,
    /// and an entry burst, by agent id, for each that came into it (`table.rs:674-702`).
    fn look(&mut self, player_id: u32, position: (f32, f32)) -> (Vec<Msg>, Vec<(u32, Vec<Msg>)>) {
        let sight = self.sights.entry(player_id).or_default();
        let wanted: BTreeSet<u32> = (self.agents.iter())
            .filter(|(_, agent)| self.rules.sees(position, agent.body.position))
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
        self.look(player_id, position).1
    }

    /// A client has left: forget what it was told (`table.rs:715-718`).
    pub fn forget(&mut self, player_id: u32) {
        self.sights.remove(&player_id);
    }

    /// One viewer's part of a tick: the bodies that left its range, then the ones that came into it
    /// (`table.rs:871-873`). The walks of visible bodies (`table.rs:865-869`) are not here: no body
    /// walks until stage N4.
    pub fn sweep(&mut self, player_id: u32, position: (f32, f32)) -> Vec<Msg> {
        let (mut msgs, entries) = self.look(player_id, position);
        msgs.extend(entries.into_iter().flat_map(|(_, burst)| burst));
        msgs
    }
}

/// One running copy's agents: its NPC table and the agent ids its players hold. ASURA keeps both in
/// its `Instance` (`instance.rs:146-165`).
#[derive(Debug, Default)]
struct Running {
    npcs: NpcTable,
    players: BTreeSet<u32>,
}

/// Every running copy's NPCs and agent ids, keyed by map and district.
#[derive(Debug, Default)]
pub struct NpcDistricts {
    copies: HashMap<(u32, u32), Running>,
}

impl NpcDistricts {
    /// No copies yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The copy, created with its map's bodies the first time anything asks for it - which is a
    /// player's agent id, so the bodies always exist before any player (`instance.rs:178-205`, R29).
    fn copy(&mut self, map_id: u32, district: u32, content: &NpcContent) -> &mut Running {
        self.copies.entry((map_id, district)).or_insert_with(|| {
            let npcs = match content.map(map_id as i32) {
                Some(map) => NpcTable::seed(map, content.sight, FIRST_AGENT_ID),
                None => NpcTable {
                    rules: content.sight,
                    ..NpcTable::default()
                },
            };
            Running {
                npcs,
                players: BTreeSet::new(),
            }
        })
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
        self.copy(map_id, district, content)
            .npcs
            .arrive(player_id, position)
            .into_iter()
            .map(|(agent_id, msgs)| (agent_id, npc_packets::frames(&msgs)))
            .collect()
    }

    /// One client's sight sweep on a tick, as frames: removes for bodies that left its range, entries
    /// for bodies that came into it. Empty for a copy that is not running.
    pub fn sweep(
        &mut self,
        map_id: u32,
        district: u32,
        player_id: u32,
        position: (f32, f32),
    ) -> Vec<Vec<u8>> {
        match self.copies.get_mut(&(map_id, district)) {
            Some(copy) if !copy.npcs.is_empty() => {
                npc_packets::frames(&copy.npcs.sweep(player_id, position))
            }
            _ => Vec::new(),
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
    use crate::agents::npc_content::tests::{definition, map, placement, sight};

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
        NpcTable::seed(content().map(148).expect("map"), sight(), 41)
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
}
