//! One running copy's fight: the engine, the creatures' brains, and what a tick produces.
//!
//! This tree's arrangement of `ASURA/crates/asura-world/src/instance.rs`'s `combat` and `fight`
//! fields (`:146-165`, `:355-378`, `Instance::step`): a copy of a map is a district here, so its
//! combat lives beside its NPC table. The brain is `game/src/Combat/`; the mechanics are
//! `game/src/Skills/Engine.rs`; this file owns one of each per copy and the clock that advances them
//! (`ASURA` `npc/fight.rs:238-429`, `skill/engine.rs:2317-2326`).
//!
//! The fight is ticked by the district's own clock, once per `World.SimulationTickMs`, by whichever
//! connection reaches the period first - the same rule the NPC table follows, so two players in one
//! town do not run the fight twice as fast.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agents::npc_content::{Aggression, NpcCombat};
use crate::combat::{Fight, FightRules, Order, Reach, Seen, Style};
use crate::skills::catalog::Pack;
use crate::skills::engine::{Spec, ATTRIBUTES};
use crate::skills::{npc_spec, Combat, Loadout, Recipient, EMPTY_BAR};

/// One body that fights, as the district's NPC table knows it: where it stands and what it is.
#[derive(Debug, Clone)]
pub struct Fighter {
    /// Its agent id.
    pub agent: u32,
    /// Where it stands.
    pub position: (f32, f32),
    /// Its level.
    pub level: u32,
    /// Its combat block.
    pub combat: NpcCombat,
}

/// One running copy's fight.
#[derive(Default)]
struct Running {
    combat: Combat,
    fight: Fight,
    seeded: bool,
    /// Every agent the fight holds, in the order it joined. The engine's own map is private, so the
    /// district keeps the list it put there.
    agents: Vec<u32>,
    /// When the fight last thought: the copy's own clock, not a connection's.
    last_tick: Option<Instant>,
}

/// Every running copy's fight.
pub struct CombatDistricts {
    copies: HashMap<(u32, u32), Running>,
    pack: Arc<Pack>,
    rules: FightRules,
    /// The Lua host a scripted skill asks (`game/Scripting/SkillScripts.rs`). `None` until the world
    /// loads the scripts; a scripted skill then completes and does nothing.
    scripts: Option<Arc<std::sync::Mutex<dyn crate::skills::engine::SkillScripts>>>,
}

impl CombatDistricts {
    /// No copies yet, with the pack, the brain's numbers and the skill scripts this world runs by.
    pub fn new(
        pack: Arc<Pack>,
        rules: FightRules,
        scripts: Option<Arc<std::sync::Mutex<dyn crate::skills::engine::SkillScripts>>>,
    ) -> CombatDistricts {
        CombatDistricts {
            copies: HashMap::new(),
            pack,
            rules,
            scripts,
        }
    }

    /// The pack in force.
    pub fn pack(&self) -> &Pack {
        &self.pack
    }

    /// The rules in force.
    pub fn rules(&self) -> &FightRules {
        &self.rules
    }

    fn copy(&mut self, key: (u32, u32)) -> &mut Running {
        self.copies.entry(key).or_default()
    }

    /// Put a player's agent in this copy's fight, creating the copy and seeding its creatures the
    /// first time anyone joins (`instance.rs:178-225`, `with_npcs`).
    pub fn join_player(
        &mut self,
        key: (u32, u32),
        agent: u32,
        player_id: u32,
        spec: Spec,
        position: (f32, f32),
        fighters: &[Fighter],
    ) {
        let host = self.scripts.clone();
        let copy = self.copy(key);
        if !copy.seeded {
            if let Some(host) = host {
                copy.combat.engine.set_scripts(host);
            }
            for fighter in fighters {
                // A creature is nobody's agent: it joins the engine directly, and every message about
                // it goes to the instance (`skill/character.rs:204-215` is the player's half).
                copy.combat.engine.join(
                    fighter.agent,
                    npc_spec(&fighter.combat, fighter.level),
                    fighter.position,
                );
                copy.agents.push(fighter.agent);
                if fighter.combat.hostile {
                    let style = match &fighter.combat.attack {
                        Some(attack) if attack.ranged => Style::Ranged,
                        Some(_) => Style::Melee,
                        None => Style::Caster,
                    };
                    copy.fight.add(
                        fighter.agent,
                        fighter.position,
                        style,
                        fighter.combat.skills.clone(),
                        fighter.combat.aggression,
                    );
                    if fighter.combat.stationary {
                        copy.fight.pin(fighter.agent);
                    }
                }
            }
            copy.seeded = true;
        }
        // The player's equipment and bar come from its row and bags; the engine keeps both.
        copy.combat.join_player(agent, player_id, spec, position);
        if !copy.agents.contains(&agent) {
            copy.agents.push(agent);
        }
    }

    /// Whether this copy's player deaths charge the death penalty (`World::death_charges`).
    pub fn set_charged_deaths(&mut self, key: (u32, u32), charged: bool) {
        if let Some(copy) = self.copies.get_mut(&key) {
            copy.combat.engine.set_charged_deaths(charged);
        }
    }

    /// Clear an agent's death penalty: a resign resets it (`E12`).
    pub fn clear_penalty(&mut self, key: (u32, u32), agent: u32) {
        if let Some(copy) = self.copies.get_mut(&key) {
            copy.combat.engine.clear_death_penalty(agent);
        }
    }

    /// A player left: its agent leaves the fight.
    pub fn leave(&mut self, key: (u32, u32), agent: u32) {        if let Some(copy) = self.copies.get_mut(&key) {
            copy.combat.leave(agent);
            copy.agents.retain(|held| *held != agent);
        }
    }

    /// The copy closed: its fight goes with it.
    pub fn close(&mut self, map_id: u32, district: u32) {
        self.copies.remove(&(map_id, district));
    }

    /// Replace a player's bar (`ASURA` `skill/character.rs:388-395`).
    pub fn set_bar(&mut self, key: (u32, u32), agent: u32, bar: [(u32, u32); 8]) {
        if let Some(copy) = self.copies.get_mut(&key) {
            copy.combat.engine.set_bar(agent, bar);
        }
    }

    /// Replace a player's ranks.
    pub fn set_ranks(&mut self, key: (u32, u32), agent: u32, ranks: [u8; ATTRIBUTES]) {
        if let Some(copy) = self.copies.get_mut(&key) {
            copy.combat.engine.set_ranks(agent, ranks);
        }
    }

    /// Replace a player's weapon and armor.
    pub fn set_equipment(
        &mut self,
        key: (u32, u32),
        agent: u32,
        weapon: Option<crate::skills::model::Weapon>,
        armor: i32,
    ) {
        if let Some(copy) = self.copies.get_mut(&key) {
            copy.combat.engine.set_equipment(agent, weapon, armor);
        }
    }

    /// The vitals a load block states for one agent, as frames.
    pub fn vitals(&self, key: (u32, u32), agent: u32) -> Vec<Vec<u8>> {
        let Some(copy) = self.copies.get(&key) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for event in copy.combat.vitals(agent, &self.pack) {
            let (_, frames) = crate::skills::wire::frames(&event, &self.pack.rules);
            out.extend(frames);
        }
        out
    }

    /// Use a skill on a target. Returns the refusal, when there was one; every message the engine
    /// produced is returned for delivery.
    pub fn use_skill(
        &mut self,
        key: (u32, u32),
        agent: u32,
        skill: u32,
        copy: u32,
        target: u32,
    ) -> (
        Result<(), crate::skills::engine::Refusal>,
        Vec<(Recipient, Vec<u32>, Vec<Vec<u8>>)>,
    ) {
        let pack = Arc::clone(&self.pack);
        let Some(running) = self.copies.get_mut(&key) else {
            return (Err(crate::skills::engine::Refusal::NoCaster), Vec::new());
        };
        let result = running
            .combat
            .engine
            .use_skill(agent, skill, copy, target, &pack.skills, &pack.rules);
        let deliveries = running.combat.deliveries(&pack);
        (result, deliveries)
    }

    /// Put an agent's ordinary attack on a target.
    pub fn attack(
        &mut self,
        key: (u32, u32),
        agent: u32,
        target: u32,
    ) -> (
        Result<(), crate::skills::engine::Refusal>,
        Option<f32>,
        Vec<(Recipient, Vec<u32>, Vec<Vec<u8>>)>,
    ) {
        let pack = Arc::clone(&self.pack);
        let Some(running) = self.copies.get_mut(&key) else {
            return (Err(crate::skills::engine::Refusal::NoCaster), None, Vec::new());
        };
        let result = running.combat.engine.attack(agent, target);
        let reach = running.combat.engine.attack_reach(agent, &pack.rules);
        let deliveries = running.combat.deliveries(&pack);
        (result, reach, deliveries)
    }

    /// Stop an agent's ordinary attack.
    pub fn stop_attack(&mut self, key: (u32, u32), agent: u32) {
        if let Some(running) = self.copies.get_mut(&key) {
            running.combat.engine.stop_attack(agent);
        }
    }

    /// Kill an agent outright, as `/resign` does.
    pub fn kill(
        &mut self,
        key: (u32, u32),
        agent: u32,
    ) -> Vec<(Recipient, Vec<u32>, Vec<Vec<u8>>)> {
        let pack = Arc::clone(&self.pack);
        let Some(running) = self.copies.get_mut(&key) else {
            return Vec::new();
        };
        running.combat.engine.kill(agent, &pack.skills);
        running.combat.deliveries(&pack)
    }

    /// Apply a direct loss, for the operator's `/damage` test (`Engine::hurt`).
    pub fn hurt(
        &mut self,
        key: (u32, u32),
        agent: u32,
        amount: f64,
    ) -> Vec<(Recipient, Vec<u32>, Vec<Vec<u8>>)> {
        let pack = Arc::clone(&self.pack);
        let Some(running) = self.copies.get_mut(&key) else {
            return Vec::new();
        };
        running.combat.engine.hurt(agent, amount);
        running.combat.deliveries(&pack)
    }

    /// Stand a player up where it fell, for the operator's `/revive` test.
    pub fn revive(
        &mut self,
        key: (u32, u32),
        agent: u32,
        position: (f32, f32),
    ) -> Vec<(Recipient, Vec<u32>, Vec<Vec<u8>>)> {
        let pack = Arc::clone(&self.pack);
        let Some(running) = self.copies.get_mut(&key) else {
            return Vec::new();
        };
        running.combat.engine.revive(agent, position, &pack.rules);
        running.combat.deliveries(&pack)
    }

    /// Whether an agent still stands in this copy.
    pub fn alive(&self, key: (u32, u32), agent: u32) -> bool {
        self.copies
            .get(&key)
            .and_then(|running| running.combat.engine.actor(agent))
            .is_some_and(|actor| actor.alive)
    }

    /// The player agents in this copy, as `(player_id, agent)`.
    pub fn players(&self, key: (u32, u32)) -> Vec<(u32, u32)> {
        let Some(running) = self.copies.get(&key) else {
            return Vec::new();
        };
        running
            .combat
            .agents()
            .into_iter()
            .filter_map(|agent| running.combat.owner_of(agent).map(|player| (player, agent)))
            .collect()
    }

    /// One tick of the copy, if a period has passed since the last one. Returns the movement orders
    /// the caller must carry out on the NPC table, and the messages the engine produced.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        key: (u32, u32),
        now: Instant,
        period: Duration,
        positions: &[(u32, (f32, f32))],
    ) -> (
        Vec<(u32, Order)>,
        Vec<(Recipient, Vec<u32>, Vec<Vec<u8>>)>,
    ) {
        let pack = Arc::clone(&self.pack);
        let rules = self.rules;
        let Some(running) = self.copies.get_mut(&key) else {
            return (Vec::new(), Vec::new());
        };
        if !running.seeded {
            return (Vec::new(), Vec::new());
        }
        let due = match running.last_tick {
            None => {
                running.last_tick = Some(now);
                false
            }
            Some(last) => now.saturating_duration_since(last) >= period,
        };
        if !due {
            return (Vec::new(), Vec::new());
        }
        let last = running.last_tick.replace(now).unwrap_or(now);
        let elapsed = now
            .saturating_duration_since(last)
            .min(period)
            .as_millis() as u64;

        // The bodies the brain and the engine share.
        for (agent, position) in positions {
            running.combat.engine.set_position(*agent, *position);
        }
        let actors: Vec<(u32, (f32, f32), bool, bool)> = running
            .agents
            .clone()
            .into_iter()
            .filter_map(|agent| {
                running.combat.engine.actor(agent).map(|actor| {
                    (
                        agent,
                        actor.position,
                        actor.alive,
                        actor.spec.side == crate::skills::engine::Side::Player,
                    )
                })
            })
            .collect();
        let mut bodies = BTreeMap::new();
        let mut players = Vec::new();
        for (agent, position, alive, is_player) in actors {
            let seen = Seen {
                agent,
                position,
                alive,
            };
            if is_player {
                players.push(seen);
            } else {
                bodies.insert(agent, seen);
            }
        }
        let reach = Reach {
            melee: pack.rules.melee_range,
            ranged: pack.rules.bow_range,
            spell: pack.rules.spell_range,
        };
        let orders = running
            .fight
            .think(elapsed, &rules, reach, &bodies, &players);
        let mut walking = Vec::new();
        for (agent, order) in orders {
            match order {
                Order::Attack { target } => {
                    let _ = running.combat.engine.attack(agent, target);
                }
                Order::UseSkills { skills, target } => {
                    for skill in skills {
                        if running
                            .combat
                            .engine
                            .use_skill(agent, skill, 0, target, &pack.skills, &pack.rules)
                            .is_ok()
                        {
                            break;
                        }
                    }
                }
                Order::StopAttack => running.combat.engine.stop_attack(agent),
                Order::Restore => running.combat.engine.restore(agent, &pack.rules),
                Order::Revive { at } => running.combat.engine.revive(agent, at, &pack.rules),
                Order::Fall | Order::Walk { .. } | Order::Halt => walking.push((agent, order)),
            }
        }
        running.combat.engine.tick(elapsed, &pack.skills, &pack.rules);
        for (agent, attacker) in running.combat.engine.take_provoked() {
            running.fight.provoke(agent, attacker);
        }
        let deliveries = running.combat.deliveries(&pack);
        (walking, deliveries)
    }

    /// How many bodies the copy's fight holds.
    pub fn count(&self, map_id: u32, district: u32) -> usize {
        self.copies
            .get(&(map_id, district))
            .map_or(0, |copy| copy.agents.len())
    }

    /// An empty set of ranks, for a load that has no row yet.
    pub fn empty_loadout() -> Loadout {
        Loadout {
            learned: Default::default(),
            bar: EMPTY_BAR,
            ranks: crate::skills::NO_RANKS,
        }
    }
}

/// The style a creature fights in, from its block, for the log and for a test.
pub fn style_of(combat: &NpcCombat) -> Style {
    match &combat.attack {
        Some(attack) if attack.ranged => Style::Ranged,
        Some(_) => Style::Melee,
        None => Style::Caster,
    }
}

/// Whether a creature fights on sight.
pub fn aggression_of(combat: &NpcCombat) -> Aggression {
    combat.aggression
}
