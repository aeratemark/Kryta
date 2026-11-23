//! The skill family's handlers: using a skill, putting an attack on a foe, editing the bar and the
//! attribute ranks, and the `/skill` test command.
//!
//! Ported from `ASURA/crates/asura-world/src/skill/character.rs` (744 lines, the local repository as
//! it stands on 2026-11-23): `handle_use_skill`, `handle_attack_agent`, the four bar edits, the three
//! attribute edits and `handle_skill_command`. The decisions themselves are `game/Skills/Rules.rs`
//! and `game/Skills/Engine.rs`; this file is the translation from a client's message to them and the
//! answer back, as every handler in this tree is (`docs/port/10-separation-guide.md`, section 2).

use common::logging::LogLevel;
use shared::cryptography::AuthCrypt;
use shared::network::socket::SessionIo;

use crate::server::WorldSession;
use crate::characters::skills as character_skills;
use crate::server::packets::skill_packets as wire;
use crate::skills::engine::Refusal;
use crate::skills::rules::{self as bar_rules, AttributeEdit, BarEdit};

impl WorldSession {
    /// The character's uuid, for the row a bar or attribute edit writes.
    fn skill_uuid(&self) -> Option<String> {
        let character = self.character.lock().ok().and_then(|value| *value)?;
        Some(crate::characters::uuid_text(&character))
    }

    /// Write the loadout back to the character's row. A row that cannot be written is logged; the
    /// session keeps the loadout it has either way (`ASURA` `skill/character.rs:75-84`).
    fn save_skills(&self, loadout: &crate::skills::Loadout) {
        let (Some(pool), Some(uuid)) = (self.characters.as_ref(), self.skill_uuid()) else {
            return;
        };
        let characters =
            crate::characters::DatabaseCharacters::new(std::sync::Arc::clone(pool), self.log.clone());
        if let Err(error) = characters.record_skill_profile(&uuid, &loadout.to_profile()) {
            self.say(
                LogLevel::Warn,
                format!("skills: cannot save the skills of {uuid}: {error}"),
            );
        }
    }

    /// Whether the bar or a rank may change here: a town or an outpost, as the client itself only
    /// offers (`ASURA` `skill/character.rs:129-132`).
    fn may_change_skills(&self) -> bool {
        self.maps
            .iter()
            .find(|map| map.id == self.map_id())
            .and_then(|map| map.kind)
            == Some(crate::world::map_kind::OUTPOST)
    }

    /// `GAME_CMSG_USE_SKILL` and `GAME_CMSG_USE_ATTACK_SKILL`: use a bar slot on a target.
    pub(crate) async fn answer_use_skill(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some(request) = wire::read_use_skill(message.get(2..).unwrap_or_default()) else {
            self.report_unanswered(peer, 0x8046, "a use-skill request", message.len());
            return;
        };
        if self.world.skills().is_none() {
            // No pack: the request is answered with its release alone, as ASURA answers it
            // (`skill/character.rs:245-255`).
            let frame = wire::skill_ack(self.agent_id(), request.skill, request.copy);
            self.send(io, peer, crypt, &[frame], "a skill's release")
                .await;
            return;
        }
        let result = self.world.combat_use_skill(
            self.map_id(),
            self.district(),
            self.agent_id(),
            request.skill,
            request.copy,
            request.target,
        );
        if let Err(refusal) = &result
            && !matches!(refusal, Refusal::Busy | Refusal::Recharging)
        {
            self.say(
                LogLevel::Debug,
                format!(
                    "skills: agent {} could not use skill {} on {}: {refusal:?}",
                    self.agent_id(),
                    request.skill,
                    request.target
                ),
            );
        }
    }

    /// `GAME_CMSG_ATTACK_AGENT`: the player's ordinary attack goes on a foe. The live server walks
    /// the attacker into reach; this tree's client walks itself, and the engine swings once it is
    /// there (`ASURA/crates/asura-world/src/skill/character.rs:283-356`).
    pub(crate) async fn answer_attack_agent(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some((target, _call)) = wire::read_attack_agent(message.get(2..).unwrap_or_default())
        else {
            self.report_unanswered(peer, 0x8026, "an attack request", message.len());
            return;
        };
        let (result, _reach) = self.world.combat_attack(
            self.map_id(),
            self.district(),
            self.agent_id(),
            target,
        );
        if let Err(refusal) = result
            && refusal != Refusal::BadTarget
        {
            self.say(
                LogLevel::Debug,
                format!(
                    "skills: agent {} could not attack {target}: {refusal:?}",
                    self.agent_id()
                ),
            );
        }
        let _ = (io, crypt);
    }

    /// One bar edit: the proposed bar is checked against the character's learned set and
    /// professions, and the answer is always the bar as it now stands
    /// (`ASURA/crates/asura-world/src/skill/character.rs:360-405`).
    async fn answer_bar(
        &self,
        io: &mut SessionIo,
        peer: &str,
        agent: u32,
        edit: BarEdit,
        crypt: &mut AuthCrypt,
    ) {
        let outcome = match self.world.skills() {
            None => Err("no skill pack".to_string()),
            Some(_) if agent != self.agent_id() => {
                Err(format!("agent {agent} is not this player's"))
            }
            Some(_) if !self.may_change_skills() => {
                Err(format!("{:?}", bar_rules::BarRefusal::NotHere))
            }
            Some(pack) => {
                let loadout = self
                    .skills
                    .lock()
                    .map(|skills| skills.clone())
                    .unwrap_or_default();
                bar_rules::propose(&loadout.bar, &edit)
                    .and_then(|bar| {
                        bar_rules::validate(
                            &bar,
                            self.profession(),
                            self.secondary_profession(),
                            &|id| loadout.learned.contains(&id),
                            &character_skills::facts(pack),
                        )
                        .map(|()| bar)
                    })
                    .map_err(|refusal| format!("{refusal:?}"))
            }
        };
        match outcome {
            Ok(bar) => {
                if let Ok(mut skills) = self.skills.lock() {
                    skills.bar = bar;
                }
                self.world
                    .combat_set_bar(self.map_id(), self.district(), self.agent_id(), bar);
                if let Ok(skills) = self.skills.lock() {
                    self.save_skills(&skills);
                }
            }
            Err(why) => self.say(
                LogLevel::Debug,
                format!("skills: bar edit {edit:?} refused: {why}"),
            ),
        }
        let bar = self
            .skills
            .lock()
            .map(|skills| skills.bar)
            .unwrap_or(crate::skills::EMPTY_BAR);
        let answer = wire::skillbar(self.agent_id(), &bar, false);
        self.send(io, peer, crypt, &[answer], "the bar").await;
    }

    /// `GAME_CMSG_SKILLBAR_SKILL_SET`.
    pub(crate) async fn answer_skillbar_set(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some((agent, slot, skill, copy)) =
            wire::read_skillbar_set(message.get(2..).unwrap_or_default())
        else {
            return;
        };
        self.answer_bar(
            io,
            peer,
            agent,
            BarEdit::Set {
                slot,
                skill: (skill, copy),
            },
            crypt,
        )
        .await;
    }

    /// `GAME_CMSG_SKILLBAR_LOAD`: a whole template.
    pub(crate) async fn answer_skillbar_load(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some((agent, ids)) =
            wire::read_skillbar_load(message.get(2..).unwrap_or_default())
        else {
            return;
        };
        self.answer_bar(io, peer, agent, BarEdit::Load { ids }, crypt)
            .await;
    }

    /// `GAME_CMSG_SKILLBAR_SKILL_REPLACE`: two skills on the bar exchange slots.
    pub(crate) async fn answer_skillbar_swap(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some((agent, first, second)) =
            wire::read_skillbar_swap(message.get(2..).unwrap_or_default())
        else {
            return;
        };
        self.answer_bar(io, peer, agent, BarEdit::Swap { first, second }, crypt)
            .await;
    }

    /// `GAME_CMSG_SKILLBAR_SKILL_MOVE`: a skill on the bar moves to an empty slot.
    pub(crate) async fn answer_skillbar_move(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some((agent, skill, slot)) =
            wire::read_skillbar_move(message.get(2..).unwrap_or_default())
        else {
            return;
        };
        self.answer_bar(io, peer, agent, BarEdit::Move { skill, slot }, crypt)
            .await;
    }

    /// One attribute edit: the proposed ranks are checked against the professions, the cap and the
    /// earned points, and the answer is always the snapshot (`ASURA` `skill/character.rs:508-552`).
    async fn answer_attributes(
        &self,
        io: &mut SessionIo,
        peer: &str,
        agent: u32,
        edit: AttributeEdit,
        crypt: &mut AuthCrypt,
    ) {
        let Some(pack) = self.world.skills() else {
            return;
        };
        let outcome = if agent != self.agent_id() {
            Err(format!("agent {agent} is not this player's"))
        } else {
            let loadout = self
                .skills
                .lock()
                .map(|skills| skills.clone())
                .unwrap_or_default();
            let sheet = self.skill_sheet();
            bar_rules::apply_attribute_edit(
                &loadout.ranks,
                &edit,
                sheet.profession,
                sheet.secondary_profession,
                u32::from(sheet.level.max(1)),
                self.may_change_skills(),
                &character_skills::table(pack),
            )
            .map_err(|refusal| format!("{refusal:?}"))
        };
        match outcome {
            Ok(ranks) => {
                if let Ok(mut skills) = self.skills.lock() {
                    skills.ranks.copy_from_slice(&ranks[..crate::skills::engine::ATTRIBUTES]);
                }
                let ranks = self
                    .skills
                    .lock()
                    .map(|skills| skills.ranks)
                    .unwrap_or(crate::skills::NO_RANKS);
                self.world.combat_set_ranks(
                    self.map_id(),
                    self.district(),
                    self.agent_id(),
                    ranks,
                );
                if let Ok(skills) = self.skills.lock() {
                    self.save_skills(&skills);
                }
            }
            Err(why) => self.say(
                LogLevel::Debug,
                format!("skills: attribute edit {edit:?} refused: {why}"),
            ),
        }
        let sheet = self.skill_sheet();
        let loadout = self.skills.lock().map(|skills| skills.clone()).unwrap_or_default();
        let answer = character_skills::attribute_frames(
            self.agent_id(),
            &loadout,
            &sheet,
            pack,
        );
        self.send(io, peer, crypt, &answer, "the attributes")
            .await;
    }

    /// A sheet for the fields an attribute answer needs, from the session's stored character. The
    /// load already handed the session its row; this is the same row's professions and level.
    fn skill_sheet(&self) -> crate::characters::CharacterSheet {
        let uuid = self
            .character
            .lock()
            .ok()
            .and_then(|character| *character)
            .unwrap_or([0u8; 16]);
        self.sheet
            .lock()
            .ok()
            .and_then(|sheet| sheet.clone())
            .unwrap_or_else(|| crate::characters::CharacterSheet::missing(uuid))
    }

    /// `GAME_CMSG_ATTRIBUTE_INCREASE`.
    pub(crate) async fn answer_attribute_increase(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some((agent, attribute)) =
            wire::read_attribute_change(message.get(2..).unwrap_or_default())
        else {
            return;
        };
        self.answer_attributes(io, peer, agent, AttributeEdit::Increase(attribute), crypt)
            .await;
    }

    /// `GAME_CMSG_ATTRIBUTE_DECREASE`.
    pub(crate) async fn answer_attribute_decrease(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some((agent, attribute)) =
            wire::read_attribute_change(message.get(2..).unwrap_or_default())
        else {
            return;
        };
        self.answer_attributes(io, peer, agent, AttributeEdit::Decrease(attribute), crypt)
            .await;
    }

    /// `GAME_CMSG_ATTRIBUTE_LOAD`: a whole allocation.
    pub(crate) async fn answer_attribute_load(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let Some((agent, pairs)) =
            wire::read_attribute_load(message.get(2..).unwrap_or_default())
        else {
            return;
        };
        self.answer_attributes(io, peer, agent, AttributeEdit::Load(pairs), crypt)
            .await;
    }

    /// `/skill`: not the reference's - an ASURA test command, because no quest or trainer teaches
    /// skills yet (`ASURA/crates/asura-world/src/skill/character.rs:628-744`).
    pub(crate) async fn answer_skill_command(
        &self,
        io: &mut SessionIo,
        peer: &str,
        argument: &str,
        crypt: &mut AuthCrypt,
    ) {
        let Some(pack) = self.world.skills() else {
            self.answer_line(io, peer, crypt, "Skills are unavailable: the pack has no skills.")
                .await;
            return;
        };
        let mut words = argument.split_whitespace();
        let verb = words.next().unwrap_or_default();
        let what = words.next().unwrap_or_default();
        let profession = self.profession();
        let secondary = self.secondary_profession();
        let usable = |id: u32| {
            pack.skills.get(id).is_some_and(|skill| {
                skill.profession == 0
                    || skill.profession == profession
                    || skill.profession == secondary
            })
        };
        let mut frames = Vec::new();
        let mut loadout = self.skills.lock().map(|skills| skills.clone()).unwrap_or_default();
        let line = match (verb, what) {
            ("list", _) => {
                let names: Vec<String> = loadout
                    .learned
                    .iter()
                    .map(|id| match pack.skills.get(*id) {
                        Some(skill) => format!("{id} {}", skill.name),
                        None => id.to_string(),
                    })
                    .collect();
                if names.is_empty() {
                    "No skills learned. /skill learn <id> or /skill learn all.".to_string()
                } else {
                    format!("Learned: {}", names.join(", "))
                }
            }
            ("learn", "all") => {
                let mut ids: Vec<u32> = pack
                    .skills
                    .by_id
                    .keys()
                    .copied()
                    .filter(|id| usable(*id))
                    .collect();
                ids.sort_unstable();
                let new: Vec<u32> = ids
                    .into_iter()
                    .filter(|id| loadout.learned.insert(*id))
                    .collect();
                frames.push(character_skills::learned_frame(&loadout));
                for id in &new {
                    frames.push(wire::skill_learned_count(*id as u16, 1));
                }
                if let Ok(mut skills) = self.skills.lock() {
                    *skills = loadout.clone();
                }
                self.save_skills(&loadout);
                format!("Learned {} skill(s).", new.len())
            }
            ("learn", id) => match id.parse::<u32>() {
                Ok(id) if usable(id) => {
                    if loadout.learned.insert(id) {
                        frames.push(character_skills::learned_frame(&loadout));
                        frames.push(wire::skill_learned(id as u16, 1));
                        if let Ok(mut skills) = self.skills.lock() {
                            *skills = loadout.clone();
                        }
                        self.save_skills(&loadout);
                    }
                    format!(
                        "Learned {}.",
                        pack.skills
                            .get(id)
                            .map(|skill| skill.name.as_str())
                            .unwrap_or("")
                    )
                }
                _ => "That is not a pre-Searing skill of your professions.".to_string(),
            },
            ("forget", id) => match id.parse::<u32>() {
                Ok(id) if loadout.learned.remove(&id) => {
                    for slot in loadout.bar.iter_mut() {
                        if slot.0 == id {
                            *slot = (0, 0);
                        }
                    }
                    self.world.combat_set_bar(
                        self.map_id(),
                        self.district(),
                        self.agent_id(),
                        loadout.bar,
                    );
                    frames.push(wire::skill_learned_count(id as u16, 0));
                    frames.push(character_skills::learned_frame(&loadout));
                    frames.push(wire::skillbar(self.agent_id(), &loadout.bar, false));
                    if let Ok(mut skills) = self.skills.lock() {
                        *skills = loadout.clone();
                    }
                    self.save_skills(&loadout);
                    "Forgotten.".to_string()
                }
                _ => "That skill is not learned.".to_string(),
            },
            _ => "Usage: /skill list | /skill learn <id> | /skill learn all | /skill forget <id>"
                .to_string(),
        };
        self.send(io, peer, crypt, &frames, "a skill command's frames")
            .await;
        self.answer_line(io, peer, crypt, &line).await;
    }

    /// The professions the skill handler needs. The sheet's own values are stored on the session
    /// when a map begins; a session that has none speaks the creation default.
    fn profession(&self) -> u8 {
        self.skill_sheet().profession
    }

    /// See [`Self::profession`].
    fn secondary_profession(&self) -> u8 {
        self.skill_sheet().secondary_profession
    }
}
