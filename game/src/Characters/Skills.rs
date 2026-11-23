//! A character's side of skills: its learned set, bar and ranks, the load block's messages for them,
//! and what a bar or attribute edit writes back.
//!
//! Ported from `ASURA/crates/asura-world/src/skill/character.rs` (744 lines, the local repository as
//! it stands on 2026-11-23), the parts that are the character's rather than the session's. The
//! request handlers and `/skill` live in `game/Handlers/SkillHandler.rs`, because this tree's
//! session owns the connection a refusal is answered on.
//!
//! Every answer to a bar or attribute request is the authoritative state, accepted or not: a
//! well-framed refused edit is answered with the bar or ranks as they stand, never with silence and
//! never with a disconnect (`ASURA` `gw_skillbar.h` `apply`, `gw_attributes.h` `propose`).

use crate::characters::CharacterSheet;
use crate::server::packets::skill_packets as wire;
use crate::skills::catalog::Pack;
use crate::skills::rules::{self as rules, AttributeTable, SkillFacts};
use crate::skills::{Loadout, EMPTY_BAR, NO_RANKS};

/// The attribute points: free and total, for `AGENT_UPDATE_ATTRIBUTE_POINTS`.
pub fn attribute_points(loadout: &Loadout, level: u32, pack: &Pack) -> (u8, u8) {
    let table = table(pack);
    let total = table.points(level);
    let free = total.saturating_sub(table.spent(&loadout.ranks));
    (
        u8::try_from(free).unwrap_or(u8::MAX),
        u8::try_from(total).unwrap_or(u8::MAX),
    )
}

/// The attribute table, read from `attributes.json`.
pub fn table(pack: &Pack) -> AttributeTable<'_> {
    AttributeTable {
        professions: &pack.attributes.professions,
        rank_cost: &pack.attributes.rank_cost,
        level_points: &pack.attributes.level_points,
        max_rank: pack.attributes.max_rank,
        max_attributes: pack.attributes.max_attributes,
    }
}

/// The facts a bar check reads for one skill.
pub fn facts(pack: &Pack) -> impl Fn(u32) -> Option<SkillFacts> + '_ {
    |id| {
        pack.skills.get(id).map(|skill| SkillFacts {
            profession: skill.profession,
            elite: skill.elite,
            pve_only: skill.pve_only,
        })
    }
}

/// `AGENT_UPDATE_ATTRIBUTES` for the character's professions: each listed attribute's id, base rank
/// and effective rank (no equipment bonus is applied yet, so the two are equal).
pub fn attributes_words(
    loadout: &Loadout,
    sheet: &CharacterSheet,
    pack: &Pack,
) -> Vec<u32> {
    let listed: Vec<(u32, u32, u32)> = table(pack)
        .listed(sheet.profession, sheet.secondary_profession)
        .into_iter()
        .map(|attribute| {
            let rank = u32::from(loadout.ranks.get(attribute).copied().unwrap_or(0));
            (attribute as u32, rank, rank)
        })
        .collect();
    let mut words = Vec::with_capacity(listed.len() * 3);
    words.extend(listed.iter().map(|entry| entry.0));
    words.extend(listed.iter().map(|entry| entry.1));
    words.extend(listed.iter().map(|entry| entry.2));
    words
}

/// The attribute answer preforged sends after an edit: the snapshot, then free and total points.
pub fn attribute_frames(
    agent: u32,
    loadout: &Loadout,
    sheet: &CharacterSheet,
    pack: &Pack,
) -> Vec<Vec<u8>> {
    let (free, total) = attribute_points(loadout, u32::from(sheet.level.max(1)), pack);
    let listed: Vec<(u32, u32, u32)> = table(pack)
        .listed(sheet.profession, sheet.secondary_profession)
        .into_iter()
        .map(|attribute| {
            let rank = u32::from(loadout.ranks.get(attribute).copied().unwrap_or(0));
            (attribute as u32, rank, rank)
        })
        .collect();
    vec![
        wire::attributes(agent, &listed),
        wire::attribute_points_free(agent, free),
        wire::attribute_points_total(agent, total),
    ]
}

/// The learned set as `GAME_SMSG_UPDATE_UNLOCKED_SKILLS` carries it.
pub fn learned_frame(loadout: &Loadout) -> Vec<u8> {
    crate::server::packets::character_packets::update_unlocked_skills(
        &rules::learned_bitmap(loadout.learned.iter().copied()),
    )
}

/// The bar as the load block carries it (`create` 1).
pub fn bar_frame(agent: u32, loadout: &Loadout) -> Vec<u8> {
    wire::skillbar(agent, &loadout.bar, true)
}

/// The three vitals a load block sends: maximum energy, maximum health and the energy regeneration
/// rate (`ASURA/crates/asura-world/src/skill/character.rs:191-199`, `vitals`).
pub fn vitals(sheet: &CharacterSheet, pack: &Pack) -> (u32, u32, f32) {
    let level = u32::from(sheet.level.max(1));
    let energy = pack.player.energy_max;
    let regen = f64::from(pack.player.energy_regen_pips) * pack.rules.energy_pip_per_second
        / f64::from(energy.max(1));
    (energy, pack.player.max_health(level), regen as f32)
}

/// What a load block states about a character's skills, computed once and handed to the frames.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillLoad {
    /// Unspent and earned attribute points.
    pub attribute_points: (u8, u8),
    /// The attribute snapshot's words: ids, base ranks, effective ranks.
    pub attributes: Vec<u32>,
    /// The bar.
    pub bar: [(u32, u32); 8],
    /// The learned set's bitmap.
    pub learned: Vec<u32>,
    /// Maximum energy.
    pub max_energy: u32,
    /// Maximum health.
    pub max_health: u32,
    /// Energy regeneration, as a fraction of the maximum per second.
    pub energy_regen: f32,
}

impl SkillLoad {
    /// The empty block a world with no skill pack sends: the reference's own constants.
    pub fn empty() -> SkillLoad {
        SkillLoad {
            attribute_points: (50, 50),
            attributes: vec![0u32; 42],
            bar: EMPTY_BAR,
            learned: Vec::new(),
            max_energy: 25,
            max_health: 400,
            energy_regen: 0.0,
        }
    }

    /// A character's own numbers, from its loadout and the pack.
    pub fn of(loadout: &Loadout, sheet: &CharacterSheet, pack: &Pack) -> SkillLoad {
        let bitmap = rules::learned_bitmap(loadout.learned.iter().copied());
        let (max_energy, max_health, energy_regen) = vitals(sheet, pack);
        SkillLoad {
            attribute_points: attribute_points(
                loadout,
                u32::from(sheet.level.max(1)),
                pack,
            ),
            attributes: attributes_words(loadout, sheet, pack),
            bar: loadout.bar,
            learned: bitmap,
            max_energy,
            max_health,
            energy_regen,
        }
    }
}

/// A bar with every slot empty, for a character with no row.
pub const DEFAULT_LOADOUT: Loadout = Loadout {
    learned: std::collections::BTreeSet::new(),
    bar: EMPTY_BAR,
    ranks: NO_RANKS,
};
