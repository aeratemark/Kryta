//! `game/Skills/` - the reference's `src/server/game/Spells/`, renamed. Its content is ported from
//! `ASURA/crates/asura-world/src/skill/` (ASURA_testfork `071735e`), with one change of design:
//! **what a skill does is a Lua script** (`docs/decisions/0016-skills-are-lua-scripts.md`).
//!
//! The parts, as ASURA keeps them apart (`skill/mod.rs:1-16`):
//!
//! * [`model`] and [`engine`] are the mechanics, with no file, socket or database in them, tested on
//!   their own (`Tests.rs`, ASURA's 26 tests plus the script seam's 3);
//! * [`catalog`] reads the pack's `skills.json`, `combat.json` and `attributes.json`. A skill row names
//!   its `script`; the script is run by `game/Scripting/SkillScripts.rs` and queues the same operations
//!   ASURA's `effects` lists declared;
//! * [`rules`] is ASURA's `asura-rules/src/skills.rs`: whether a bar or an attribute spend may stand;
//! * [`wire`] turns what the engine did into this tree's frames and says who hears each.
//!
//! **Ported thin, and said so** (`docs/port/09-parity-audit.md`, 7j): ASURA's `skill/character.rs` (the
//! character's learned set, bar and ranks in the session, the load block's frames and the nine client
//! requests) and the instance half (`instance.rs:169-230`, `733-904`: one `Combat` per copy, ticked with
//! the copy's clock) are the next round. The schema and the row reader for a character's skills are here
//! (`data/sql/base/db_characters/0013-character-skills.sql`, `Characters/Repository.rs`).

#[path = "Catalog.rs"]
pub mod catalog;
#[path = "Engine.rs"]
pub mod engine;
#[path = "Model.rs"]
pub mod model;
#[path = "Rules.rs"]
pub mod rules;
#[path = "Wire.rs"]
pub mod wire;

#[cfg(test)]
#[path = "Tests.rs"]
mod tests;

use std::collections::{BTreeSet, HashMap};

use crate::agents::npc_content::NpcCombat;
use crate::data_stores::item_modifier::{ModifierEffect, ModifierWord};
use catalog::Pack;
use engine::{ATTRIBUTES, Engine, Event, Side, Slot, Spec};
use model::Weapon;
use wire::To;

/// Where one batch of messages goes. `skill/mod.rs:38-45`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipient {
    /// One player's connection, by player id.
    Player(u32),
    /// Every client in the copy that has entered it.
    Everyone,
}

/// One copy's combatants, and which player drives each agent. `skill/mod.rs:47-120`.
pub struct Combat {
    /// The ported engine.
    pub engine: Engine,
    owners: HashMap<u32, u32>,
}

impl Default for Combat {
    fn default() -> Combat {
        Combat::new(0x5EED_A5A5)
    }
}

impl Combat {
    /// A copy's fight, its rolls seeded.
    pub fn new(seed: u64) -> Combat {
        Combat {
            engine: Engine::new(seed),
            owners: HashMap::new(),
        }
    }

    /// A copy's fight whose scripted skills ask `scripts` (KRYTA).
    pub fn with_scripts(
        seed: u64,
        scripts: std::sync::Arc<std::sync::Mutex<dyn engine::SkillScripts>>,
    ) -> Combat {
        let mut combat = Combat::new(seed);
        combat.engine.set_scripts(scripts);
        combat
    }

    /// A player's agent joins the fight.
    pub fn join_player(&mut self, agent: u32, player_id: u32, spec: Spec, position: (f32, f32)) {
        self.owners.insert(agent, player_id);
        self.engine.join(agent, spec, position);
    }

    /// An agent leaves.
    pub fn leave(&mut self, agent: u32) {
        self.owners.remove(&agent);
        self.engine.leave(agent);
    }

    /// The player that drives an agent, if it is in the fight.
    pub fn owner_of(&self, agent: u32) -> Option<u32> {
        self.owners.get(&agent).copied()
    }

    /// The agents a player drives, in agent order: what an operator surface names when it asks who
    /// is in the copy (`CombatDistricts::players`).
    pub fn agents(&self) -> Vec<u32> {
        let mut agents: Vec<u32> = self.owners.keys().copied().collect();
        agents.sort_unstable();
        agents
    }

    /// Everything the engine did since the last call, as frames for their recipients and the agents the
    /// batch names. A message for an agent no player drives is dropped: there is nobody to tell.
    /// `skill/mod.rs:81-113`.
    pub fn deliveries(&mut self, pack: &Pack) -> Vec<(Recipient, Vec<u32>, Vec<Vec<u8>>)> {
        let mut out: Vec<(Recipient, Vec<u32>, Vec<Vec<u8>>)> = Vec::new();
        for event in self.engine.drain() {
            let (to, frames) = wire::frames(&event, &pack.rules);
            let recipient = match to {
                To::All => Recipient::Everyone,
                To::Owner(agent) => match self.owners.get(&agent) {
                    Some(player_id) => Recipient::Player(*player_id),
                    None => continue,
                },
            };
            let subjects = wire::subjects(&event);
            // Consecutive messages for the same recipient and the same agents travel together, in order.
            match out.last_mut() {
                Some((last, batch_subjects, batch))
                    if *last == recipient && *batch_subjects == subjects =>
                {
                    batch.extend(frames)
                }
                _ => out.push((recipient, subjects, frames)),
            }
        }
        out
    }

    /// The events a load block's vitals are, for one agent.
    pub fn vitals(&self, agent: u32, pack: &Pack) -> Vec<Event> {
        self.engine.vitals(agent, &pack.rules)
    }
}

/// The weapon an attack skill can be used with and the armor rating the worn pieces add up to.
/// `skill/mod.rs:122-187`, taking the equipped bag's items as `(item_type, modifier words)` because this
/// tree's inventory rows are read elsewhere (`Characters/ItemOps.rs`). A body with no armor word at all has
/// the pack's default rating.
pub fn equipment(worn: &[(u8, Vec<u32>)], pack: &Pack) -> (Option<Weapon>, i32) {
    let mut weapon = None;
    let mut armor = 0.0;
    let mut armored = false;
    for (item_type, words) in worn {
        let effects = words
            .iter()
            .map(|word| ModifierWord::new(*word).decode().effect());
        if let Some(row) = pack.weapons.get(item_type) {
            for effect in effects {
                if let ModifierEffect::DamageRange { min, max } = effect {
                    weapon = Some(Weapon {
                        item_type: *item_type,
                        attribute: row.attribute,
                        period_ms: row.period_ms,
                        kind: row.kind,
                        ranged: row.ranged,
                        min: u32::from(min),
                        max: u32::from(max).max(u32::from(min)),
                        // A player's arrow: model 143 with the byte 1 (`skill/mod.rs:150-152`).
                        projectile: row.ranged.then_some((143, 1)),
                    });
                }
            }
        } else if let Some(share) = pack.armor_slots.get(item_type) {
            for effect in effects {
                if let ModifierEffect::Armor { amount } = effect {
                    armor += share * f64::from(amount);
                    armored = true;
                }
            }
        }
    }
    let armor = if armored {
        armor.round() as i32
    } else {
        pack.player.armor_default
    };
    (weapon, armor)
}

/// A character's skills: what it learned, its bar and its ranks (ASURA `skill/character.rs`'s
/// `Loadout`, the part the row and the engine share).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loadout {
    /// Learned skill ids.
    pub learned: BTreeSet<u32>,
    /// The eight (skill, copy) slots.
    pub bar: [Slot; 8],
    /// One rank per attribute index.
    pub ranks: [u8; ATTRIBUTES],
}

impl Default for Loadout {
    fn default() -> Loadout {
        Loadout {
            learned: BTreeSet::new(),
            bar: EMPTY_BAR,
            ranks: NO_RANKS,
        }
    }
}

impl Loadout {
    /// From the character's row; no row is a new character.
    pub fn from_profile(profile: Option<crate::characters::repository::SkillProfile>) -> Loadout {
        let Some(profile) = profile else {
            return Loadout::default();
        };
        let mut ranks = NO_RANKS;
        for (slot, rank) in ranks.iter_mut().zip(profile.ranks.iter()) {
            *slot = *rank;
        }
        Loadout {
            learned: profile.learned.into_iter().collect(),
            bar: profile.bar,
            ranks,
        }
    }

    /// The row that stores it.
    pub fn to_profile(&self) -> crate::characters::repository::SkillProfile {
        crate::characters::repository::SkillProfile {
            learned: learned_list(&self.learned),
            bar: self.bar,
            ranks: self.ranks.to_vec(),
        }
    }
}

/// What a player is to the engine. `skill/mod.rs:189-215`, with the traits named one by one.
pub fn player_spec(
    loadout: &Loadout,
    level: u8,
    primary: u8,
    secondary: u8,
    worn: &[(u8, Vec<u32>)],
    pack: &Pack,
) -> Spec {
    let level = u32::from(level.max(1));
    let (weapon, armor) = equipment(worn, pack);
    Spec {
        side: Side::Player,
        level,
        primary,
        secondary,
        ranks: loadout.ranks,
        bar: loadout.bar,
        max_health: pack.player.max_health(level),
        max_energy: pack.player.energy_max,
        energy_pips: pack.player.energy_regen_pips,
        armor,
        weapon,
    }
}

/// An NPC's combat block, as the engine sees it. `skill/mod.rs:217-246`.
pub fn npc_spec(combat: &NpcCombat, level: u32) -> Spec {
    let side = if combat.hostile {
        Side::Foe
    } else {
        Side::Neutral
    };
    let mut spec = Spec::creature(side, level.max(1), combat.health, combat.armor);
    spec.max_energy = combat.energy;
    spec.energy_pips = combat.energy_regen;
    spec.primary = combat.profession;
    spec.ranks = [combat.rank; ATTRIBUTES];
    // Every skill on the bar is copy 0: a creature has one of each.
    for (slot, skill) in spec.bar.iter_mut().zip(combat.skills.iter()) {
        *slot = (*skill, 0);
    }
    spec.weapon = combat.attack.as_ref().and_then(|attack| {
        Some(Weapon {
            item_type: attack.item_type,
            // The creature's rank is the same in every attribute, so any index reads it.
            attribute: 0,
            period_ms: attack.period_ms,
            kind: model::DamageKind::parse(&attack.damage_type)?,
            ranged: attack.ranged,
            min: attack.min,
            max: attack.max.max(attack.min),
            projectile: attack.projectile.map(|model| (model, attack.projectile_flag)),
        })
    });
    spec
}

/// A learned set as a sorted list, the order rows and messages use.
pub fn learned_list(learned: &BTreeSet<u32>) -> Vec<u32> {
    learned.iter().copied().collect()
}

/// An empty bar.
pub const EMPTY_BAR: [Slot; 8] = [(0, 0); 8];
/// No ranks anywhere.
pub const NO_RANKS: [u8; ATTRIBUTES] = [0; ATTRIBUTES];

#[cfg(test)]
mod delivery_tests {
    use super::*;
    use crate::server::packets::{agent_packets, skill_packets};

    /// `skill/mod.rs:259-291`, through the committed pack. Orison's script is asked, so the host is set.
    #[test]
    fn a_slot_message_reaches_its_owner_and_a_cast_reaches_everyone() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/World/Vanilla");
        let pack = catalog::load(&dir).expect("the committed pack");
        let scripts = crate::scripting::skill_scripts::LuaSkills::load(&dir, &pack.skills).unwrap();
        let mut combat = Combat::with_scripts(0x5EED_A5A5, std::sync::Arc::new(std::sync::Mutex::new(scripts)));
        let mut spec = Spec::creature(Side::Player, 1, 100, 60);
        spec.max_energy = 25;
        spec.bar[0] = (281, 0);
        combat.join_player(5, 1, spec, (0.0, 0.0));
        combat
            .engine
            .use_skill(5, 281, 0, 0, &pack.skills, &pack.rules)
            .expect("Orison on itself");
        let out = combat.deliveries(&pack);
        let header = |frame: &Vec<u8>| u16::from_le_bytes([frame[0], frame[1]]);
        assert_eq!(out[0].0, Recipient::Player(1), "energy and the slot are the owner's");
        assert_eq!(out[0].1, vec![5]);
        assert_eq!(
            out[0].2.iter().map(header).collect::<Vec<_>>(),
            vec![
                agent_packets::AGENT_PROPERTY_UPDATE_FLOAT,
                skill_packets::GAME_SMSG_SKILL_ACTIVATE
            ]
        );
        assert_eq!(out[1].0, Recipient::Everyone, "the cast is drawn by everyone");
        assert_eq!(out[1].1, vec![5, 5], "the caster and its target");
        assert_eq!(
            header(&out[1].2[0]),
            skill_packets::GAME_SMSG_AGENT_PROPERTY_UPDATE_INT_TARGET
        );
    }
}
