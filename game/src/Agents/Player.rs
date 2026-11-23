//! The player a load draws: the reference's own `Player` object, whose
//! `SendInitialPacketsBeforeAddToMap` and `SendInitialPacketsAfterAddToMap`
//! (`src/server/game/Entities/Player/Player.cpp`) are the load's message burst - not a message
//! handler's work and not the session's. This tree's `Agents/` is the reference's `Entities/`,
//! which is why the player entity's frames live beside the agent's body.
//!
//! Reference for every frame and its order: `ASURA/crates/asura-world/src/player.rs` (540 lines),
//! `handle_request_players` -> `player_block`. The reads that decide *what* the block says - the
//! character's row, R12's unlock, the entry point - stay where they belong (the session and the
//! nouns it asks); this file is the frames.

use crate::server::packets::{
    agent_packets, character_packets, instance_packets, misc_packets, party_packets,
};

/// The team token a load gives the client. Evidence: the reference's own trace carried `0D F0 AD BA`
/// in `0x00F2` (`recordings/gwps/world/load-stream.log`), which is `ASURA`'s `PLAYER_TEAM_TOKEN`.
pub(crate) const PLAYER_TEAM_TOKEN: u32 = 0xBAAD_F00D;

/// The two dword lists a load sends at full length whether or not a character has used them: the
/// hard-mode mission and bonus bitmaps. An empty list is not the same message - it is an array the
/// client can index past the end of (`ASURA/crates/asura-world/src/player.rs`, `UNLOCKED_MAP_ZEROS`,
/// copied from the reference's own send log: 31 zero words each).
pub(crate) const UNLOCKED_MAP_ZEROS: &[u32] = &[0u32; 31];

/// Full morale, which a fresh agent has (`ASURA/crates/asura-world/src/player.rs`, `MORALE_FULL`).
const MORALE_FULL: u32 = 100;

/// `0x00E9`, the progress record, for a character's progress at full morale: the load's own statement,
/// and the whole block a change of experience restates when an award cannot say it
/// (`ASURA/crates/asura-world/src/player.rs:346-353` and `:547-558`).
pub(crate) fn progress_record(progress: &crate::characters::experience::Progress) -> Vec<u8> {
    let mut record = [0u32; character_packets::FACTIONS_DWORDS];
    record[0] = progress.experience;
    record[9] = u32::from(progress.level);
    record[10] = MORALE_FULL;
    record[13] = progress.skill_points;
    record[14] = progress.skill_points_total;
    character_packets::character_update_factions(&record)
}

/// The character a load is told as: the two ids it is named by, where it stands, what it
/// wears, the instances it has reached, and what its progress record says about it.
pub(crate) struct Player {
    pub agent: u32,
    pub player: u32,
    pub position: (f32, f32),
    pub plane: u16,
    pub direction: (f32, f32),
    pub appearance: u32,
    /// The item ids the agent is drawn wearing, from the equipped bag, in its own order. A slot
    /// the character wears nothing in is zero.
    pub equipped: [u32; 9],
    /// The gold the character carries (R45: gold is per character).
    pub gold: u32,
    /// The gold its Xunlai storage holds.
    pub gold_storage: u32,
    pub profession: u8,
    pub unlocked_professions: u32,
    /// Bit 0 Prophecies, bit 1 Factions, bit 2 Nightfall: a finished campaign unlocks hard mode.
    pub campaigns_finished: u8,
    pub level: u8,
    /// Total experience and the skill points held and earned: the progress record's first, fourteenth
    /// and fifteenth dwords (`ASURA/crates/asura-world/src/player.rs:346-353`).
    pub experience: u32,
    pub skill_points: u32,
    pub skill_points_total: u32,
    pub name: String,
}

impl Player {
    /// The `0x8091` answer: the inventory stream and the map's record, measured in one block. The
    /// manifest pair that ends it names the map in its second form
    /// (`ASURA/crates/asura-world/src/map.rs:22-58`; ASURA answers it in `item.rs`,
    /// `handle_request_items`, and the reference in `Player::SendItems`).
    pub(crate) fn items_answer(
        &self,
        inventory: &crate::creation::Inventory,
        map: &crate::world::MapEntry,
    ) -> Vec<Vec<u8>> {
        let mut frames = crate::creation::load_item_stream_purse(inventory, self.gold, self.gold_storage);
        // `0x0070`: hard mode is a campaign completion reward. A character that has finished any of
        // Prophecies, Factions or Nightfall has the normal/hard toggle; one that has finished none
        // does not. Pre-Searing is not a special case in the protocol - a character who has finished
        // Prophecies cannot enter it.
        frames.push(misc_packets::hard_mode_unlocked(u8::from(
            self.campaigns_finished != 0,
        )));
        frames.push(character_packets::character_update_info(""));
        frames.push(misc_packets::map_update_current(map.id as u16, 0));
        frames.push(instance_packets::ready_for_map_spawn(0));
        frames.push(instance_packets::instance_manifest_phase(0));
        frames.push(instance_packets::instance_manifest_phase(1));
        frames.push(instance_packets::instance_manifest_done(2, 877, 0));
        frames.push(instance_packets::instance_manifest_phase(0));
        frames.push(instance_packets::instance_manifest_phase(1));
        frames.push(instance_packets::instance_manifest_done(
            0,
            map.id as u16,
            0,
        ));
        frames
    }

    /// The `0x8088` answer: the entry point and nothing else - `spawn_block` in the working
    /// implementation is exactly one `0x0196` frame (`ASURA/crates/asura-world/src/map.rs:13-20`),
    /// and the simulation's ticks come from the ticker, not from this answer. ASURA answers it in
    /// `agent::handle_request_spawn`.
    pub(crate) fn spawn_answer(&self, map: &crate::world::MapEntry) -> Vec<u8> {
        instance_packets::instance_load_spawn_point(
            map.file_id,
            self.position.0,
            self.position.1,
            self.plane,
        )
    }

    /// The `0x8090` answer: the whole block the client's players request is answered with, frame
    /// for frame and in the reference's own order (`GameSrv_HandleInstanceLoadRequestPlayers`,
    /// `ASURA/crates/asura-world/src/player.rs`, `player_block`). It is one block, not a menu: the
    /// PvP unlocks and the unlockable-item table, the account features, the unlocked lists and the
    /// team token, the skills and the progress record, the visibility and player flags, the player
    /// and the agent it is drawn as, the agent's own properties and look, the party, and the
    /// message that ends the load.
    ///
    /// The players already in this copy of the map are introduced after it, and that is the session's
    /// to do: it is the district that knows who else is there, and the client must read its own team
    /// token (`0x00F2`) before it creates anyone else. So is the bitmap of reached instances - it is
    /// the character's rows, read with the arrival's own R12 write, and handed here as the list the
    /// block names.
    pub(crate) fn load_block(
        &self,
        settings: &crate::world::world_config::WorldConfig,
        load_ms: u32,
        unlocked: &[u32],
        titles: &crate::characters::titles::VanillaTitles,
        skills: Option<&crate::characters::skills::SkillLoad>,
    ) -> Vec<Vec<u8>> {
        let agent = self.agent;
        let player = self.player;
        let position = self.position;
        let plane = self.plane;
        // The entry point's own facing (its spawn point's angle, or a doorway arrival's), and
        // `(1, 0)` when it states none - `map_spawns.rs:44-53`.
        let direction = self.direction;
        let appearance = self.appearance;
        let equipped = self.equipped;
        // The character record's own numbers, in the reference's order: experience first, level tenth,
        // morale eleventh, the skill points held and earned last (`ASURA/crates/asura-world/src/
        // player.rs:341-353`; `docs/54-experience.md` section 1 there). The client shows 0 for a number
        // it is not told.
        let progress = progress_record(&crate::characters::experience::Progress {
            experience: self.experience,
            level: self.level,
            skill_points: self.skill_points,
            skill_points_total: self.skill_points_total,
        });
        let mut frames = Vec::with_capacity(40);
        frames.push(misc_packets::pvp_update_unlocked_skills(&[]));
        frames.push(misc_packets::pvp_update_unlocked_heroes(&[0xFFFF_FFFF; 8]));
        for item in crate::data_stores::pvp_items::PVP_ITEMS {
            frames.push(misc_packets::pvp_item_stream_add(item.id, item.params));
        }
        frames.push(misc_packets::pvp_item_stream_end());
        for (feature, param1, param2) in [
            (1u16, 10u16, 0u16),
            (99, 3, 0),
            (100, 5, 0),
            (101, 5, 5),
            (102, 2, 2),
            (111, 1, 0),
            (124, 1, 0),
            (125, 1, 0),
            (131, 1, 0),
        ] {
            frames.push(misc_packets::account_feature(feature, param1, param2));
        }
        frames.push(misc_packets::map_update_unlocked_list([
            &[],
            &[],
            UNLOCKED_MAP_ZEROS,
            UNLOCKED_MAP_ZEROS,
            unlocked,
        ]));
        frames.push(misc_packets::instance_loaded(PLAYER_TEAM_TOKEN));
        // The character's own attribute points when the skill pack is read; the reference's 50/50
        // otherwise (`ASURA/crates/asura-world/src/skill/character.rs:137-145`).
        let (free, total) = skills
            .map(|skills| skills.attribute_points)
            .unwrap_or((50, 50));
        frames.push(agent_packets::agent_update_attribute_points(
            agent, free, total,
        ));
        frames.push(character_packets::player_update_profession(
            agent,
            self.profession,
            0,
            0,
        ));
        frames.push(character_packets::player_update_unlocked_professions(
            agent,
            self.unlocked_professions,
        ));
        // The character's own bar: eight skill ids and eight copy indices, as the skill pack holds
        // them (`ASURA/crates/asura-world/src/skill/character.rs:186-189`).
        let bar = skills.map(|skills| skills.bar).unwrap_or(crate::skills::EMPTY_BAR);
        let bar_ids: Vec<u32> = bar.iter().map(|slot| slot.0).collect();
        let bar_copies: Vec<u32> = bar.iter().map(|slot| slot.1).collect();
        frames.push(character_packets::skillbar_update(
            agent,
            &bar_ids,
            &bar_copies,
            1,
        ));
        // Retail primes both pools with 1 and states the morale right after the skillbar, before the
        // agent exists (`ASURA/recordings/client/feature/maps/maps-live-20261123-022154.jsonl` seq
        // 650-653, 1939-1940, 2305-2306; `doors-20261031-095754.jsonl` seq 4005-4008).
        frames.push(agent_packets::agent_property_update_int(
            agent_packets::AGENT_PROPERTY_ENERGY,
            agent,
            1,
        ));
        frames.push(agent_packets::agent_property_update_int(
            agent_packets::AGENT_PROPERTY_HEALTH,
            agent,
            1,
        ));
        frames.push(agent_packets::agent_update_morale(agent, MORALE_FULL));
        frames.push(character_packets::update_unlocked_skills(
            skills.map(|skills| skills.learned.as_slice()).unwrap_or(&[]),
        ));
        frames.push(progress);
        frames.push(misc_packets::char_visibility_flags(
            0xFFFF_FFFF,
            0xFFFF_FFFF,
        ));
        frames.push(misc_packets::player_update_flags(player as u16, 0, 7));
        // ASURA crates/asura-protocol/src/packets.rs:771-774 defines the packets
        // without a title sender. Build 39167 Northlands, seq 534-582:
        // definitions follow 0x00E9, 0x00EF and 0x003C, before load finish.
        frames.extend(crate::characters::titles::load_frames(titles));
        // The attribute snapshot for the character's professions: ids, base ranks, effective ranks
        // (`ASURA/crates/asura-world/src/skill/character.rs:149-164`); 42 zeros without the pack.
        let attributes = skills
            .map(|skills| skills.attributes.as_slice())
            .unwrap_or(&[]);
        if attributes.is_empty() {
            frames.push(agent_packets::agent_update_attributes(agent, &[0u32; 42]));
        } else {
            frames.push(agent_packets::agent_update_attributes(agent, attributes));
        }
        // The real maxima follow the attributes, still before the agent is created, in retail's order:
        // energy, its regeneration, health - and no current-pool fractions at load
        // (`maps-live-20261123-022154.jsonl` seq 676-680, 1965-1967, 2335-2337). Sent after the create
        // instead, the client sometimes drew no health or energy bar.
        // Maximum health is the pack's `health_base + health_per_level * level` (`combat.json`,
        // `ASURA/crates/asura-world/src/skill/character.rs:191-198`, `vitals`).
        let health = skills.map_or_else(
            || {
                settings.agent_health_base.saturating_add(
                    settings
                        .agent_health_per_level
                        .saturating_mul(u32::from(self.level)),
                )
            },
            |skills| skills.max_health,
        );
        let energy = skills.map_or(settings.agent_energy, |skills| skills.max_energy);
        let energy_regen = skills.map_or(settings.agent_energy_regen, |skills| skills.energy_regen);
        frames.push(agent_packets::agent_property_update_int(
            agent_packets::AGENT_PROPERTY_ENERGY,
            agent,
            energy,
        ));
        frames.push(agent_packets::agent_property_update_float(
            agent_packets::AGENT_PROPERTY_ENERGY_REGEN,
            agent,
            energy_regen,
        ));
        frames.push(agent_packets::agent_property_update_int(
            agent_packets::AGENT_PROPERTY_HEALTH,
            agent,
            health,
        ));
        frames.push(misc_packets::world_update_load_time(load_ms));
        frames.push(instance_packets::player_create(
            player, agent, appearance, &self.name,
        ));
        frames.push(character_packets::player_update_profession(
            agent,
            self.profession,
            0,
            0,
        ));
        frames.push(agent_packets::world_create_agent(
            &agent_packets::CreateAgent {
                agent_id: agent,
                model_id: agent_packets::CHAR_CLASS_PLAYER_BASE | player,
                agent_type: agent_packets::AGENT_TYPE_LIVING,
                position,
                plane,
                // The direction the entry point states - the spawn point's own angle, or
                // a doorway arrival's, or `(1, 0)` when the row states neither
                // (`ASURA/crates/asura-world/src/map_spawns.rs:44-53`).
                direction,
                speed_base: settings.agent_speed,
                player_team_token: PLAYER_TEAM_TOKEN,
                ..agent_packets::CreateAgent::LIVING
            },
        ));
        // No current-pool fractions here: retail sends none in the load. The client starts the bars
        // full on its own when the exploration grid sits inside this block, right after the morale,
        // where `splice_exploration_grid` puts it. With the grid before the load instead, the bars
        // drew empty and only these fractions filled them - the workaround this replaced, proven
        // unnecessary in the live client on 2026-11-23 (`maps-live-20261123-022154.jsonl` seq
        // 653-655, 1940-1943, 2306-2310).
        frames.push(agent_packets::agent_property_update_int(
            agent_packets::AGENT_PROPERTY_PUBLIC_LEVEL,
            agent,
            u32::from(self.level),
        ));
        frames.push(agent_packets::agent_initial_effects(agent, 0));
        frames.push(misc_packets::update_agent_visual_equipment(agent, equipped));
        frames.push(misc_packets::agent_display_cape(agent, 1));
        frames.push(agent_packets::world_update_controlled_agent(agent, 3));
        frames.push(party_packets::party_create(party_packets::PARTY_ID));
        frames.push(party_packets::party_player_add(
            party_packets::PARTY_ID,
            player as u16,
            1,
        ));
        frames.push(party_packets::party_member_stream_end(
            party_packets::PARTY_ID,
        ));
        frames.push(character_packets::character_update_party(
            party_packets::PARTY_ID,
            1,
        ));
        frames.push(instance_packets::instance_load_finish());
        frames.push(crate::server::packets::title_packets::rank_display(
            player as u16,
            titles.initial_display_rank(),
        ));
        frames
    }
}

/// Put the exploration grid (`0x008B`, `0x008A`) into a load block where retail sends it: right after
/// the morale (`0x009C`), inside the vitals block. The client's HUD bars rely on this place - sent
/// before the load, the bars draw empty. Without a morale frame, the grid goes first.
pub(crate) fn splice_exploration_grid(block: &mut Vec<Vec<u8>>, grid: Vec<Vec<u8>>) {
    let morale = agent_packets::AGENT_UPDATE_MORALE.to_le_bytes();
    let at = block
        .iter()
        .position(|frame| frame.len() >= 2 && frame[..2] == morale)
        .map_or(0, |at| at + 1);
    block.splice(at..at, grid);
}

#[cfg(test)]
mod tests {
    use super::Player;
    use crate::server::packets::instance_packets;
    use crate::server::packets::agent_packets;

    /// REGRESSION GUARD - the empty health/energy bars (broken and "fixed" four times by 2026-11-23).
    ///
    /// Retail's mechanism, proven in the live client: the exploration grid sits inside the vitals
    /// block right after the morale, and the load sends no current-pool fractions (properties 33/34).
    /// Earlier sessions sent the grid before the load and patched the empty bars with fractions; others
    /// then removed the fractions to "match retail" and the bars broke again. If this test fails, do
    /// not edit the test: put the grid back after the morale, or prove the bars in the live client.
    #[test]
    fn the_exploration_grid_follows_the_morale_and_no_pool_fractions_are_sent() {
        let player = Player {
            agent: 7,
            player: 3,
            position: (0.0, 0.0),
            plane: 0,
            direction: (1.0, 0.0),
            appearance: 0,
            equipped: [0; 9],
            gold: 0,
            gold_storage: 0,
            profession: 1,
            unlocked_professions: 0,
            campaigns_finished: 0,
            level: 1,
            experience: 0,
            skill_points: 0,
            skill_points_total: 0,
            name: "Test".into(),
        };
        let mut frames = player.load_block(
            &crate::world::world_config::WorldConfig::default(),
            0,
            &[],
            &crate::characters::titles::VanillaTitles::default(),
            None,
        );
        for property in [
            agent_packets::AGENT_PROPERTY_ENERGY_FRACTION,
            agent_packets::AGENT_PROPERTY_HEALTH_FRACTION,
        ] {
            assert!(
                !frames.contains(&agent_packets::agent_property_update_float(property, 7, 1.0)),
                "retail sends no pool fraction {property} in the load"
            );
        }
        super::splice_exploration_grid(
            &mut frames,
            crate::maps::cartography::Grid::empty(64, 128).frames(),
        );
        let header = |frame: &Vec<u8>| u16::from_le_bytes([frame[0], frame[1]]);
        let morale = frames
            .iter()
            .position(|frame| header(frame) == agent_packets::AGENT_UPDATE_MORALE)
            .expect("the load carries the player's morale");
        assert_eq!(
            header(&frames[morale + 1]),
            instance_packets::MAP_INIT_START,
            "the exploration grid must follow the morale, or the HUD bars draw empty"
        );
        assert_eq!(header(&frames[morale + 2]), instance_packets::CARTOGRAPHY_DATA);
    }
}
