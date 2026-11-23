//! Talking to an NPC: the walk there, the window it opens, the trades made in it, and the gold that
//! moves.
//!
//! The reference's `src/server/game/Handlers/NPCHandler.cpp` is where a gossip or a vendor list is
//! answered; this is that file for Tyria. What every answer is comes from ASURA, ported 1:1:
//!
//! * `ASURA/crates/asura-world/src/approach.rs:248-320` - `start`, `forget`, `handle_interact_living`,
//!   `handle_interact_player` - and `instance.rs:885-961`, `steer_approaches`, here one connection's
//!   own approach per tick (this tree ticks per connection: `World::npc_tick`);
//! * `ASURA/crates/asura-world/src/vendor/mod.rs` (924 lines, read 2026-11-23): `Open`, `Windows`,
//!   `in_reach`, `storage_open`, `window_ids`, `fresh_ids`, `open_merchant`, `talk`/`answer_talk`,
//!   `owned_items`, `handle_transaction`, `taken`, `placed`, `handle_change_gold`.
//!
//! What each answer is, and where it was measured (`vendor/mod.rs:10-24`, ASURA `docs/53-npc-services.md`):
//!
//! | event | frames |
//! |---|---|
//! | merchant talk | `0x00C4`, `0x0161` per stock row, `0x0084`, `0x00CA {1, 1.0}`, `0x00C3 {11, 0}`, `0x009F {12, npc, 0}` |
//! | collector talk | `0x00C4`, `0x0161` cost row and one per offer, `0x0084`, `0x00C5 {2, line}`, `0x009F {12, npc, 0}` |
//! | purchase | `0x014F` price, `0x0162` new item, `0x013E` its place, `0x00CC {1}` |
//! | exchange | `0x0139` or `0x014D` per trophy stack, `0x0162` reward, `0x013E`, `0x00CC {2}` |
//! | sale | `0x014D` or `0x0139`, `0x0140` price, `0x00CC {11}` |
//! | Xunlai talk | `0x0083 {npc, 0, 3}` |
//! | gold at Xunlai | `0x014F` / `0x0150` debits, then `0x0140` / `0x0141` credits |
//!
//! A refused request is not answered: no live refusal is captured, and preforged answers none.
//!
//! The dialog half (2026-11-23): `answer_talk`'s dialog branch (`vendor/mod.rs:256-290`),
//! `handle_send_dialog` (`:723-900`) and the dialog's talk range in `in_reach` (`:88-107`), ported
//! whole. What a page says and what a choice does is asked of the pack's Lua runtime
//! (`game/Scripting/NpcScripts.rs`, decision 0019) where ASURA reads `dialogs.json`; every frame and
//! its order stay ASURA's.
//!
//! **Ported thin** (`docs/port/09-parity-audit.md`, 7j): the quest marker refresh (`:201-216`) and the
//! quest steps a choice takes (`quest::handle_offer`, `handle_reward`, `handle_credit`) wait for
//! ASURA's `quest.rs`; a quest choice is logged and the page that follows is still shown. The
//! `/xunlai` test command (`:902-923`) is not carried; an attack's walk (`Purpose::Attack`) is never
//! started here.

use common::logging::LogLevel;
use shared::cryptography::AuthCrypt;
use shared::network::socket::SessionIo;

use crate::characters::{
    BagKind, Change, CharacterSheet, Entry, HeldItem, ItemKind, Op, RowChange, Wearer,
};
use crate::movement::approach::{self, Approach, Purpose};
use crate::scripting::npc_scripts::{self, Action, PageView};
use crate::server::packets::{dialog_packets, guild_packets, item_packets, vendor_packets as protocol};
use crate::server::world_session::WorldSession;
use crate::trader::content::{self, Service, Vendors};
use crate::trader::rules::{self, Offered, Plan, Trophy, Window};

use item_packets::ITEM_STREAM;

/// The window a connection has open (`vendor/mod.rs:37-62`).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Open {
    /// None.
    #[default]
    Nothing,
    /// A merchant's or a collector's window, with what it offered under which id.
    Trade {
        /// The NPC it belongs to.
        npc: u32,
        /// What it offered.
        window: Window,
    },
    /// The Xunlai storage window.
    Storage {
        /// The agent the window belongs to.
        npc: u32,
    },
    /// An NPC's dialog, on `page` of the dialog the pack gives that NPC (numbered from 1, as the Lua
    /// runtime numbers it; ASURA counts from 0).
    Dialog {
        /// The agent saying it.
        npc: u32,
        /// The page a talk or a choice opened.
        page: usize,
    },
}

/// A connection's windows: the one open now, and the ids each NPC's window has already declared
/// (`vendor/mod.rs:64-76`). The live server declares a window's rows the first time a player opens it
/// on a map and lists the same ids again without declaring them when the player comes back
/// (preforged record 467378 and 470670). Kept per connection, which is per map.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Windows {
    /// The window open now.
    pub open: Open,
    declared: std::collections::BTreeMap<u32, Vec<u32>>,
}

/// An NPC as a talk sees it: its agent, the placement's name and where it stands
/// (`ASURA/crates/asura-world/src/instance.rs:91-115`, `NpcTarget`).
#[derive(Debug, Clone, PartialEq)]
pub struct NpcTarget {
    /// Its agent.
    pub agent_id: u32,
    /// The map-local key of the placement it was made from.
    pub placement: String,
    /// The placement's own name, the coded units its `0x009B` carries.
    pub name: Option<Vec<u16>>,
    /// Where it stands now.
    pub position: (f32, f32),
    /// The plane it stands on.
    pub plane: u16,
}

impl NpcTarget {
    /// The name as text, when it is a literal: `0x0108 0x0107 ... 0x0001` (`instance.rs:102-114`).
    pub fn name_text(&self) -> Option<String> {
        let units = self.name.as_deref()?;
        let inner = units.strip_prefix(&[0x0108, 0x0107])?;
        let inner = inner.strip_suffix(&[0x0001]).unwrap_or(inner);
        Some(
            char::decode_utf16(inner.iter().copied())
                .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect(),
        )
    }
}

impl WorldSession {
    // ---- the walk ------------------------------------------------------------------------------------

    /// Put the player on its way to `target`, replacing any approach it had. Refused, with a line, for
    /// its own agent and for an agent the copy does not hold (`approach.rs:250-265`).
    fn start_approach(&self, peer: &str, target: u32, purpose: Purpose, range: f32) {
        let (map_id, district) = (self.map_id(), self.district());
        if target == self.agent_id() || !self.world.has_agent(map_id, district, target) {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: cannot walk to agent {target}: not another agent in this copy."
                ),
            );
            return;
        }
        if let Ok(mut slot) = self.approach.lock() {
            *slot = Some(Approach::new(target, purpose, range));
        }
    }

    /// The player moved by itself: the walk to an agent is over (`approach.rs:267-272`).
    pub(crate) fn forget_approach(&self) {
        if let Ok(mut slot) = self.approach.lock() {
            *slot = None;
        }
    }

    /// `GAME_CMSG_INTERACT_LIVING` (`0x8039`): an agent id and a byte (`approach.rs:274-301`). An NPC
    /// the player cannot walk up to (Sir Tydus on his stand) is answered from further out, by the talk
    /// range its dialog names; the usual range is never shortened (`:288-298`).
    pub(crate) fn answer_interact_living(&self, peer: &str, message: &[u8]) {
        let body = message.get(2..).unwrap_or(&[]);
        if body.len() != 5 {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: an interact request of {} bytes.",
                    body.len()
                ),
            );
            return;
        }
        let target = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let mut range = self.settings.interact_range;
        if let Some(talk_range) = self
            .world
            .npc_target(self.map_id(), self.district(), target)
            .and_then(|npc| npc.name_text())
            .and_then(|name| {
                self.world
                    .npc_scripts()
                    .and_then(|scripts| scripts.talk_range(self.map_id() as i32, &name))
            })
        {
            range = range.max(talk_range);
        }
        self.start_approach(peer, target, Purpose::Interact, range);
    }

    /// `GAME_CMSG_INTERACT_PLAYER` (`0x8033`): an agent id (`approach.rs:303-320`).
    pub(crate) fn answer_interact_player(&self, peer: &str, message: &[u8]) {
        let body = message.get(2..).unwrap_or(&[]);
        if body.len() != 4 {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: a follow request of {} bytes.",
                    body.len()
                ),
            );
            return;
        }
        let target = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        self.start_approach(peer, target, Purpose::Follow, self.settings.interact_range);
    }

    /// One tick of this connection's walk to an agent: the frames everyone in the copy is told, and
    /// the NPC this player has just arrived at, whose talk the caller answers
    /// (`ASURA/crates/asura-world/src/instance.rs:885-961`, `steer_approaches`, for one slot).
    pub(crate) fn steer_approach(&self, elapsed: std::time::Duration) -> (Vec<Vec<u8>>, Option<u32>) {
        let Some(mut current) = self.approach.lock().ok().and_then(|mut slot| slot.take()) else {
            return (Vec::new(), None);
        };
        let (map_id, district) = (self.map_id(), self.district());
        let npc = self.world.npc_target(map_id, district, current.target);
        let target = match &npc {
            Some(npc) => Some(crate::terrain::Pos {
                x: npc.position.0,
                y: npc.position.1,
                plane: npc.plane,
            }),
            None => self
                .peers
                .lock()
                .ok()
                .and_then(|peers| peers.standing_of_agent(map_id, district, current.target))
                .map(|standing| crate::terrain::Pos {
                    x: standing.position.0,
                    y: standing.position.1,
                    plane: standing.plane,
                }),
        };
        let elapsed_ms = u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX);
        let graphs = std::sync::Arc::clone(&self.graphs);
        let outcome = self.with_agent(|walker| match target {
            None => (approach::abandon(walker), None),
            Some(target) => {
                let steered = approach::steer(
                    &mut current,
                    walker,
                    target,
                    map_id as i32,
                    graphs.as_ref(),
                    elapsed_ms,
                );
                (steered.frames, Some((steered.done, steered.arrived)))
            }
        });
        let Some((frames, steered)) = outcome else {
            return (Vec::new(), None);
        };
        let mut arrived = None;
        if let Some((done, at)) = steered {
            // At the NPC: this connection answers the talk. A player is never talked to.
            if at && npc.is_some() {
                arrived = Some(current.target);
            }
            if !done && let Ok(mut slot) = self.approach.lock() {
                *slot = Some(current);
            }
        }
        (frames, arrived)
    }

    // ---- the windows -----------------------------------------------------------------------------------

    /// Whether the player stands within the pack's service range of `npc`; its own agent is always in
    /// reach, and an NPC whose dialog names a longer talk range (one on a stand) is in reach from that
    /// far too (`vendor/mod.rs:88-107`).
    fn in_reach(&self, npc: u32, vendors: &Vendors) -> bool {
        if npc == self.agent_id() {
            return true;
        }
        let Some(target) = self
            .world
            .npc_target(self.map_id(), self.district(), npc)
        else {
            return false;
        };
        let mut range = vendors.rules.service_range;
        if let Some(talk_range) = target.name_text().and_then(|name| {
            self.world
                .npc_scripts()
                .and_then(|scripts| scripts.talk_range(self.map_id() as i32, &name))
        }) {
            range = range.max(talk_range);
        }
        let here = self
            .with_agent(|agent| agent.position)
            .unwrap_or((f32::MAX, f32::MAX));
        crate::agents::distance_between(here, target.position) <= range
    }

    /// Whether a storage window is open and its owner still in reach: what lets an item move into or out
    /// of storage (`vendor/mod.rs:109-116`).
    pub(crate) fn storage_open(&self) -> bool {
        let open = self
            .vendor
            .lock()
            .map(|windows| windows.open.clone())
            .unwrap_or_default();
        let Open::Storage { npc } = open else {
            return false;
        };
        self.in_reach(npc, self.world.vendors())
    }

    /// The rules' view of the character, with storage open when its window is
    /// (`ASURA/crates/asura-world/src/item.rs:371-374`).
    pub(crate) fn wearer_for(&self, sheet: &CharacterSheet) -> Wearer {
        Wearer {
            primary_profession: sheet.profession,
            storage_open: self.storage_open(),
        }
    }

    /// The carried and the stored gold this connection holds, read from the sheet the first time
    /// (`ASURA/crates/asura-world/src/player.rs`, `traits.gold` and `storage_gold`).
    pub(crate) fn purse(&self, sheet: &CharacterSheet) -> (u32, u32) {
        match self.purse.lock() {
            Ok(mut purse) => *purse.get_or_insert((sheet.gold, sheet.gold_storage)),
            Err(_) => (sheet.gold, sheet.gold_storage),
        }
    }

    fn set_purse(&self, balances: (u32, u32)) {
        if let Ok(mut purse) = self.purse.lock() {
            *purse = Some(balances);
        }
    }

    fn set_open(&self, open: Open) {
        if let Ok(mut windows) = self.vendor.lock() {
            windows.open = open;
        }
    }

    /// The ids a window lists its rows under, and whether they still need declaring
    /// (`vendor/mod.rs:118-147`).
    fn window_ids(&self, npc: u32, count: usize) -> Result<(Vec<u32>, bool), String> {
        if let Ok(windows) = self.vendor.lock()
            && let Some(ids) = windows.declared.get(&npc)
            && ids.len() == count
        {
            return Ok((ids.clone(), false));
        }
        let characters = self
            .characters()
            .ok_or_else(|| "this world has no character database".to_string())?;
        let mut ids = Vec::with_capacity(count);
        for _ in 0..count {
            ids.push(characters.allocate_item_id()?);
        }
        if let Ok(mut windows) = self.vendor.lock() {
            windows.declared.insert(npc, ids.clone());
        }
        Ok((ids, true))
    }

    /// A merchant's window, as the live server sends it (`vendor/mod.rs:149-199`).
    fn open_merchant(
        &self,
        peer: &str,
        npc: u32,
        name: &str,
        vendors: &Vendors,
        merchant: &content::Merchant,
    ) -> Result<Vec<Vec<u8>>, String> {
        let rows: Vec<&content::ItemRow> = merchant
            .stock
            .iter()
            .filter_map(|key| vendors.item(key))
            .collect();
        let (ids, declare) = self.window_ids(npc, rows.len())?;
        let mut frames = vec![protocol::window_owner(npc)];
        let mut stock = Vec::with_capacity(rows.len());
        for (row, id) in rows.iter().zip(&ids) {
            if declare {
                frames.push(item_packets::create_named_item(&row.named(*id, 1)));
            }
            stock.push(Offered {
                id: *id,
                key: row.id.clone(),
                value: row.value,
                flags: row.flags,
                profession: 0,
            });
        }
        frames.push(protocol::window_add_items(&ids));
        frames.push(protocol::window_buy_pane(
            protocol::BUY_PANE_MERCHANT,
            vendors.rules.buy_scale,
        ));
        frames.push(protocol::merchant_window_open(protocol::KIND_SELL, 0));
        frames.push(protocol::talked_to(npc));
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: opened {name:?}'s window: {} item(s).",
                stock.len()
            ),
        );
        self.set_open(Open::Trade {
            npc,
            window: Window::Merchant {
                stock,
                unsellable_models: vendors.unsellable_models.clone(),
            },
        });
        Ok(frames)
    }

    // ---- the dialog ------------------------------------------------------------------------------------

    /// The character as the dialog runtime reads it (`ctx.char`).
    fn dialog_character(sheet: &CharacterSheet) -> npc_scripts::Character {
        npc_scripts::Character {
            name: sheet.name.clone(),
            level: u32::from(sheet.level),
            primary: sheet.profession,
            secondary: sheet.secondary_profession,
        }
    }

    /// The frames a talk (or a page choice) sends: the body, the speaker, every choice shown, and the
    /// `0x009F {12, npc, 0}` the live server sends after an answer to a talk
    /// (`ASURA/crates/asura-world/src/dialog.rs:357-371`, `Page::frames`).
    fn dialog_frames(npc: u32, view: &PageView) -> Vec<Vec<u8>> {
        let mut frames = dialog_packets::dialog_body(&view.body);
        frames.push(dialog_packets::dialog_sender(npc));
        for button in &view.buttons {
            frames.push(dialog_packets::dialog_button(
                button.icon,
                &button.label,
                button.id,
                button.skill,
            ));
        }
        frames.push(protocol::talked_to(npc));
        frames
    }

    /// The dialog's branch of a talk (`vendor/mod.rs:256-290`): the first page open to this character,
    /// sent and remembered. `Some(frames)` when the dialog answers - an empty batch for one out of
    /// reach - and `None` when the NPC has no open page here, so its window answers instead.
    fn talk_dialog(
        &self,
        peer: &str,
        sheet: &CharacterSheet,
        npc: u32,
        name: &str,
    ) -> Option<Vec<Vec<u8>>> {
        let scripts = self.world.npc_scripts()?;
        let character = Self::dialog_character(sheet);
        let talk = npc_scripts::Talk {
            map: self.map_id() as i32,
            npc: name,
            agent: npc,
            character: &character,
        };
        let opened = scripts
            .talk(&talk)
            .and_then(|page| match page {
                Some(page) => scripts.view(&talk, page).map(|view| view.map(|view| (page, view))),
                None => Ok(None),
            });
        let (page, view) = match opened {
            Ok(Some(opened)) => opened,
            Ok(None) => return None,
            Err(error) => {
                // A script fault answers like a closed dialog, and is kept for the log.
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer}: {name:?}'s dialog failed: {error}"),
                );
                return None;
            }
        };
        // The reach check is the vendor rule's; a dialog does not need the trade file, so a pack
        // without vendors.json still talks (the rule is then the walk's own range).
        let vendors = self.world.vendors();
        if 0.0 < vendors.rules.service_range && !self.in_reach(npc, vendors) {
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: out of reach of {name:?}; no dialog."),
            );
            return Some(Vec::new());
        }
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: opened {name:?}'s dialog, page {page}: {} choice(s).",
                view.buttons.len()
            ),
        );
        self.set_open(Open::Dialog { npc, page });
        Some(Self::dialog_frames(npc, &view))
    }

    /// `GAME_CMSG_SEND_DIALOG` (`0x803B`): the player chose a dialog button (`vendor/mod.rs:723-900`).
    /// The open dialog, the NPC it belongs to and the choice's action decide the answer:
    ///
    /// * `close` answers nothing; `unanswered` is logged and answers nothing;
    /// * `window` opens `0x0083 {npc, kind, 0xCCCCCCCC}` and names the speaker (`0x0081`), the live
    ///   order at seq 5247-5248;
    /// * `merchant` opens the NPC's window through the vendor content, with the speaker and the
    ///   talked-to property in the live order (seq 5281-5288);
    /// * `page` shows another page;
    /// * `quest` takes the quest step - which waits for ASURA's `quest.rs` and is logged here - and
    ///   then shows the page the choice names, or the first page that offers or rewards a quest.
    ///
    /// A choice the page does not show this character is refused and not answered.
    pub(crate) async fn answer_send_dialog(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        message: &[u8],
        sheet: &CharacterSheet,
    ) {
        let body = message.get(2..).unwrap_or(&[]);
        let Some(choice) = dialog_packets::read_send_dialog(body) else {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: a dialog choice of {} bytes does not read; ignored.",
                    body.len()
                ),
            );
            return;
        };
        let open = self
            .vendor
            .lock()
            .map(|windows| windows.open.clone())
            .unwrap_or_default();
        let Open::Dialog { npc, page } = open else {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: chose dialog {choice:#x} with no dialog open; ignored."
                ),
            );
            return;
        };
        let (map_id, district) = (self.map_id(), self.district());
        let Some(name) = self
            .world
            .npc_target(map_id, district, npc)
            .and_then(|target| target.name_text())
        else {
            self.set_open(Open::Nothing);
            return;
        };
        let Some(scripts) = self.world.npc_scripts() else {
            self.set_open(Open::Nothing);
            return;
        };
        let character = Self::dialog_character(sheet);
        let talk = npc_scripts::Talk {
            map: map_id as i32,
            npc: &name,
            agent: npc,
            character: &character,
        };
        // A choice the character was not shown (a quest offer it is not eligible for, a turn-in that
        // is not open) is refused the same way as one the page does not have (`vendor/mod.rs:812-823`).
        let action = match scripts.choose(&talk, page, choice) {
            Ok(Some(action)) => action,
            Ok(None) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: chose dialog {choice:#x}, which {name:?}'s page {page} does not offer."
                    ),
                );
                return;
            }
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer}: {name:?}'s dialog failed: {error}"),
                );
                self.set_open(Open::Nothing);
                return;
            }
        };
        self.set_open(Open::Nothing);
        self.say(
            LogLevel::Debug,
            format!("Gameplay connection from {peer}: chose dialog {choice:#x} ({action:?}) at {name:?}."),
        );
        let show = |next: usize| -> Option<Vec<Vec<u8>>> {
            match scripts.view(&talk, next) {
                Ok(Some(view)) => {
                    self.set_open(Open::Dialog { npc, page: next });
                    Some(Self::dialog_frames(npc, &view))
                }
                Ok(None) => None,
                Err(error) => {
                    self.say(
                        LogLevel::Warn,
                        format!("Gameplay connection from {peer}: {name:?}'s dialog failed: {error}"),
                    );
                    None
                }
            }
        };
        let frames = match action {
            Action::Quest { quest, step, page } => {
                // The step is an operation of ASURA's `quest.rs` (`handle_offer`, `handle_reward`,
                // `handle_credit`), which is not ported yet: nothing is recorded or paid.
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: quest {quest} {step:?} at {name:?}: the quest subject is not ported yet; nothing is recorded."
                    ),
                );
                // What the NPC says next: the page the step names, or else the first page that now
                // offers or rewards a quest (`vendor/mod.rs:841-853`). Nothing open closes the dialog.
                let next = match page {
                    Some(page) => Some(page),
                    None => scripts.next_quest_page(&talk).unwrap_or_else(|error| {
                        self.say(
                            LogLevel::Warn,
                            format!("Gameplay connection from {peer}: {name:?}'s dialog failed: {error}"),
                        );
                        None
                    }),
                };
                next.and_then(show).unwrap_or_default()
            }
            Action::Close => Vec::new(),
            Action::Unanswered => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: {name:?}'s dialog {choice:#x} has no captured answer; nothing opens."
                    ),
                );
                Vec::new()
            }
            Action::Page { page } => show(page).unwrap_or_default(),
            Action::Window { window } => vec![
                protocol::window_open(npc, window, protocol::WINDOW_DIALOG_PERMISSION),
                dialog_packets::dialog_sender(npc),
            ],
            Action::Merchant => {
                let vendors = self.world.vendors();
                let Some(Service::Merchant(merchant)) = vendors.service(map_id as i32, &name) else {
                    self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: {name:?}'s dialog {choice:#x} opens a merchant window, but vendors.json has no stock for it."
                        ),
                    );
                    return;
                };
                match self.open_merchant(peer, npc, &name, vendors, merchant) {
                    Ok(mut frames) => {
                        // The live choice's order: the window, then the speaker, then the talked-to
                        // property (seq 5281-5288). `open_merchant` ends with the property, so put the
                        // speaker before it.
                        if let Some(talked_to) = frames.pop() {
                            frames.push(dialog_packets::dialog_sender(npc));
                            frames.push(talked_to);
                        }
                        frames
                    }
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!(
                                "Gameplay connection from {peer}: {name:?}'s window cannot be opened: {error}"
                            ),
                        );
                        return;
                    }
                }
            }
        };
        if !frames.is_empty() {
            self.send(io, peer, crypt, &frames, "a dialog").await;
        }
    }

    /// Answer a talk: the player is at `npc`. A dialog the pack gives the NPC opens first; otherwise the
    /// NPC's window, and an NPC with neither answers with nothing. Any window already open is closed
    /// (`vendor/mod.rs:218-388`).
    pub(crate) async fn talk(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
        npc: u32,
    ) {
        self.set_open(Open::Nothing);
        let (map_id, district) = (self.map_id(), self.district());
        let Some(target) = self.world.npc_target(map_id, district, npc) else {
            return;
        };
        let Some(name) = target.name_text() else {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: talks to {npc} ({}), which has no literal name; no window.",
                    target.placement
                ),
            );
            return;
        };
        // A dialog comes first: The Purveyor's captured menu is what opens its merchant window, so the
        // window is behind a choice and not a talk (`movement-live-20261109-200703.jsonl` seq 5176-5288).
        if let Some(frames) = self.talk_dialog(peer, sheet, npc, &name) {
            if !frames.is_empty() {
                self.send(io, peer, crypt, &frames, "an NPC's dialog").await;
            }
            return;
        }
        let vendors = self.world.vendors();
        let Some(service) = vendors.service(map_id as i32, &name) else {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: talks to {name:?} ({}); it opens no window.",
                    target.placement
                ),
            );
            return;
        };
        if !self.in_reach(npc, vendors) {
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: out of reach of {name:?}; no window."),
            );
            return;
        }
        let frames = match service {
            Service::Merchant(merchant) => {
                match self.open_merchant(peer, npc, &name, vendors, merchant) {
                    Ok(frames) => frames,
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!(
                                "Gameplay connection from {peer}: {name:?}'s window cannot be opened: {error}"
                            ),
                        );
                        return;
                    }
                }
            }
            Service::Collector(collector) => {
                let Some(trophy) = vendors.item(&collector.trophy) else {
                    return;
                };
                // Only the offers for this character's primary profession, and those for any.
                let primary = sheet.profession;
                let offers: Vec<(&content::Offer, &content::ItemRow)> = collector
                    .offers
                    .iter()
                    .filter(|offer| offer.profession == 0 || offer.profession == primary)
                    .filter_map(|offer| Some((offer, vendors.item(&offer.reward)?)))
                    .collect();
                if offers.is_empty() {
                    self.say(
                        LogLevel::Debug,
                        format!(
                            "Gameplay connection from {peer}: {name:?} has no offer for profession {primary}."
                        ),
                    );
                    return;
                }
                let (ids, declare) = match self.window_ids(npc, offers.len() + 1) {
                    Ok(ids) => ids,
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!(
                                "Gameplay connection from {peer}: {name:?}'s window cannot be opened: {error}"
                            ),
                        );
                        return;
                    }
                };
                let mut frames = vec![protocol::window_owner(npc)];
                if declare {
                    frames.push(item_packets::create_named_item(
                        &trophy.named(ids[0], collector.cost),
                    ));
                }
                let mut window_offers = Vec::with_capacity(offers.len());
                for ((offer, row), id) in offers.iter().zip(&ids[1..]) {
                    if declare {
                        frames.push(item_packets::create_named_item(&row.named(*id, 1)));
                        if offer.profession != 0 {
                            frames.push(item_packets::item_set_profession(*id, offer.profession));
                        }
                    }
                    window_offers.push(Offered {
                        id: *id,
                        key: row.id.clone(),
                        value: row.value,
                        flags: row.flags,
                        profession: offer.profession,
                    });
                }
                frames.push(protocol::window_add_items(&ids));
                frames.push(protocol::window_collector(
                    protocol::KIND_COLLECTOR,
                    &collector.heading,
                ));
                frames.push(protocol::talked_to(npc));
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: opened collector {name:?}: {} offer(s).",
                        window_offers.len()
                    ),
                );
                self.set_open(Open::Trade {
                    npc,
                    window: Window::Collector {
                        trophy: Trophy {
                            item_type: trophy.item_type,
                            model_id: trophy.model_id,
                            name: trophy.name.clone(),
                            cost: collector.cost,
                        },
                        offers: window_offers,
                    },
                });
                frames
            }
            Service::Storage => {
                self.say(
                    LogLevel::Info,
                    format!("Gameplay connection from {peer}: opened Xunlai storage at {name:?}."),
                );
                self.set_open(Open::Storage { npc });
                vec![protocol::storage_window_open(npc)]
            }
        };
        self.send(io, peer, crypt, &frames, "an NPC's window").await;
    }

    // ---- the trades ------------------------------------------------------------------------------------

    /// What the rules read of the character's items: everything, with whether it sits in a carrying
    /// bag (`vendor/mod.rs:390-412`).
    fn owned_items(&self, rows: &[HeldItem]) -> Vec<rules::Owned> {
        let bags = self.bags();
        rows.iter()
            .map(|row| {
                let carried = row.bag_id != 0
                    && bags.iter().any(|bag| {
                        bag.id == row.bag_id
                            && BagKind::of(i32::from(bag.model)) == BagKind::Carrying
                    });
                rules::Owned {
                    id: row.item_id,
                    carried,
                    item_type: row.item_type,
                    model_id: row.model_id,
                    name: row.name.clone(),
                    value: row.value,
                    quantity: row.quantity,
                }
            })
            .collect()
    }

    /// `GAME_CMSG_TRANSACT_ITEMS` (`0x804D`) and `GAME_CMSG_BUY_MATERIALS` (`0x804A`): a trade in the
    /// open window, decided by the rules, written in one transaction, answered as the live server
    /// answers. A refused request changes nothing and is not answered (`vendor/mod.rs:414-626`).
    pub(crate) async fn answer_transaction(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        message: &[u8],
        sheet: &CharacterSheet,
    ) {
        let header = u16::from_le_bytes([message[0], message[1]]);
        let Some(request) = protocol::read_transaction(header, &message[2..]) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a trade request of {} bytes does not read; ignored.",
                    message.len() - 2
                ),
            );
            return;
        };
        let open = self
            .vendor
            .lock()
            .map(|windows| windows.open.clone())
            .unwrap_or_default();
        let Open::Trade { npc, window } = open else {
            self.say(
                LogLevel::Debug,
                format!(
                    "Gameplay connection from {peer}: a trade with no trade window open; ignored."
                ),
            );
            return;
        };
        let vendors = self.world.vendors();
        if !self.in_reach(npc, vendors) {
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: out of reach of the window's NPC; trade refused."
                ),
            );
            self.set_open(Open::Nothing);
            return;
        }
        let Some(characters) = self.characters() else {
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let rows = match crate::characters::item_ops::reload(&characters, &uuid) {
            Ok(rows) => rows,
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!(
                        "Gameplay connection from {peer}: the character's items cannot be read: {error}"
                    ),
                );
                return;
            }
        };
        let (gold, stored) = self.purse(sheet);
        let plan = match rules::authorize(
            &window,
            &request,
            &self.owned_items(&rows),
            gold,
            sheet.profession,
            &vendors.limits(),
        ) {
            Ok(plan) => plan,
            Err(refused) => {
                self.say(
                    LogLevel::Info,
                    format!("Gameplay connection from {peer}: trade {request:?} refused: {refused:?}."),
                );
                return;
            }
        };
        let gold_row = |to: u32| RowChange::Gold { from: gold, to };
        let bags = self.bags();
        let wearer = self.wearer_for(sheet);
        let commit = |ops: &[Op], new_rows: &[HeldItem], extra: Vec<RowChange>| {
            crate::characters::item_ops::commit_ops_with(
                &characters,
                &uuid,
                &rows,
                &bags,
                self.world.item_catalogue(),
                ops,
                wearer,
                new_rows,
                extra,
            )
        };
        let frames = match plan {
            Plan::Buy {
                key,
                quantity,
                price,
                stack_max,
            } => {
                let Some(row) = vendors.item(&key) else {
                    return;
                };
                let id = match characters.allocate_item_id() {
                    Ok(id) => id,
                    Err(error) => {
                        self.say(LogLevel::Warn, format!("Gameplay connection from {peer}: {error}"));
                        return;
                    }
                };
                let new_row = row.owned(id, quantity, 0);
                let entry = Entry {
                    id,
                    at: None,
                    kind: ItemKind::of(row.item_type),
                    profession: 0,
                    quantity,
                    stack_max,
                };
                let changes = match commit(
                    &[Op::Insert { entry }],
                    std::slice::from_ref(&new_row),
                    vec![gold_row(gold - price)],
                ) {
                    Ok(Ok(applied)) => applied.changes,
                    Ok(Err(refusal)) => {
                        self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: the purchase of {key} refused: {refusal:?}."
                            ),
                        );
                        return;
                    }
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!("Gameplay connection from {peer}: the purchase could not be written: {error}"),
                        );
                        return;
                    }
                };
                self.set_purse((gold - price, stored));
                let mut frames = vec![protocol::gold_character_remove(ITEM_STREAM, price)];
                frames.push(item_packets::item_update(&row.named(id, quantity)));
                frames.extend(placed(&changes));
                frames.push(guild_packets::trader_transaction_done(protocol::KIND_BUY));
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: bought {quantity} {key} for {price} gold as item {id}."
                    ),
                );
                frames
            }
            Plan::Sell {
                item,
                quantity,
                whole: _,
                price,
            } => {
                let changes = match commit(
                    &[Op::Take {
                        item,
                        count: quantity,
                    }],
                    &[],
                    vec![gold_row(gold + price)],
                ) {
                    Ok(Ok(applied)) => applied.changes,
                    Ok(Err(refusal)) => {
                        self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: the sale of item {item} refused: {refusal:?}."
                            ),
                        );
                        return;
                    }
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!("Gameplay connection from {peer}: the sale could not be written: {error}"),
                        );
                        return;
                    }
                };
                self.set_purse((gold + price, stored));
                let mut frames = taken(&changes);
                frames.push(protocol::gold_character_add(ITEM_STREAM, price));
                frames.push(guild_packets::trader_transaction_done(protocol::KIND_SELL));
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: sold {quantity} of item {item} for {price} gold."
                    ),
                );
                frames
            }
            Plan::Exchange { debits, key } => {
                let Some(row) = vendors.item(&key) else {
                    return;
                };
                let profession = match &window {
                    Window::Collector { offers, .. } => offers
                        .iter()
                        .find(|offer| offer.key == key)
                        .map_or(0, |offer| offer.profession),
                    Window::Merchant { .. } => 0,
                };
                let id = match characters.allocate_item_id() {
                    Ok(id) => id,
                    Err(error) => {
                        self.say(LogLevel::Warn, format!("Gameplay connection from {peer}: {error}"));
                        return;
                    }
                };
                let new_row = row.owned(id, 1, profession);
                let mut ops: Vec<Op> = debits
                    .iter()
                    .map(|&(item, count)| Op::Take { item, count })
                    .collect();
                ops.push(Op::Insert {
                    entry: Entry {
                        id,
                        at: None,
                        kind: ItemKind::of(row.item_type),
                        profession,
                        quantity: 1,
                        stack_max: 1,
                    },
                });
                let changes = match commit(&ops, std::slice::from_ref(&new_row), Vec::new()) {
                    Ok(Ok(applied)) => applied.changes,
                    Ok(Err(refusal)) => {
                        self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: the exchange for {key} refused: {refusal:?}."
                            ),
                        );
                        return;
                    }
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!("Gameplay connection from {peer}: the exchange could not be written: {error}"),
                        );
                        return;
                    }
                };
                let mut frames = taken(&changes);
                frames.push(item_packets::item_update(&row.named(id, 1)));
                if profession != 0 {
                    frames.push(item_packets::item_set_profession(id, profession));
                }
                frames.extend(placed(&changes));
                frames.push(guild_packets::trader_transaction_done(
                    protocol::KIND_COLLECTOR,
                ));
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: exchanged trophies for {key} as item {id}."
                    ),
                );
                frames
            }
        };
        self.send(io, peer, crypt, &frames, "a trade").await;
    }

    /// `GAME_CMSG_ITEM_CHANGE_GOLD` (`0x807C`): at a Xunlai window, the balances the client asks to end
    /// with; accepted when the total is kept and neither cap is passed, written in one transaction, and
    /// answered with the debits, then the credits (`vendor/mod.rs:653-721`).
    pub(crate) async fn answer_change_gold(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        message: &[u8],
        sheet: &CharacterSheet,
    ) {
        let Some(requested) = protocol::read_change_gold(&message[2..]) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a gold change of {} bytes does not read; ignored.",
                    message.len() - 2
                ),
            );
            return;
        };
        if !self.storage_open() {
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: gold moved with no storage window open; refused."),
            );
            return;
        }
        let vendors = self.world.vendors();
        let before = self.purse(sheet);
        let after = match rules::move_gold(before, requested, &vendors.limits()) {
            Ok(after) => after,
            Err(refused) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: gold move {before:?} -> {requested:?} refused: {refused:?}."
                    ),
                );
                return;
            }
        };
        if after == before {
            return;
        }
        let Some(characters) = self.characters() else {
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        if let Err(error) = characters.apply_item_changes(
            &uuid,
            &[
                RowChange::Gold {
                    from: before.0,
                    to: after.0,
                },
                RowChange::StorageGold {
                    from: before.1,
                    to: after.1,
                },
            ],
        ) {
            self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: the gold move could not be written: {error}"),
            );
            return;
        }
        self.set_purse(after);
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: moved gold: carried {} -> {}, stored {} -> {}.",
                before.0, after.0, before.1, after.1
            ),
        );
        let frames = protocol::gold_deltas(ITEM_STREAM, before, after);
        self.send(io, peer, crypt, &frames, "a gold move").await;
    }
}

/// What left the bags: `0x0139` for a stack that stays, `0x014D` for one that is gone
/// (`vendor/mod.rs:628-638`).
fn taken(changes: &[Change]) -> Vec<Vec<u8>> {
    changes
        .iter()
        .filter_map(|change| match *change {
            Change::Quantity { item, quantity } => {
                Some(item_packets::item_update_quantity(item, quantity))
            }
            Change::Removed { item } => Some(item_packets::item_remove(ITEM_STREAM, item)),
            _ => None,
        })
        .collect()
}

/// Where a new item went: `0x013E`, as the live purchase places it (record 487669;
/// `vendor/mod.rs:640-651`).
fn placed(changes: &[Change]) -> Vec<Vec<u8>> {
    changes
        .iter()
        .filter_map(|change| match *change {
            Change::Inserted { item, at } => Some(item_packets::item_move_to_location(
                ITEM_STREAM,
                item,
                at.0,
                at.1,
            )),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // `instance.rs:102-114`: a literal name reads; a coded string id does not.
    #[test]
    fn a_literal_name_reads_and_a_coded_one_does_not() {
        let mut units = vec![0x0108, 0x0107];
        units.extend("Sanura".encode_utf16());
        units.push(0x0001);
        let target = NpcTarget {
            agent_id: 54,
            placement: "148-1".to_string(),
            name: Some(units),
            position: (0.0, 0.0),
            plane: 0,
        };
        assert_eq!(target.name_text().as_deref(), Some("Sanura"));
        let coded = NpcTarget {
            name: Some(vec![0x2B33, 0xD6E2]),
            ..target
        };
        assert_eq!(coded.name_text(), None);
    }

    #[test]
    fn a_purchase_places_and_a_sale_takes() {
        let changes = [
            Change::Quantity { item: 7, quantity: 2 },
            Change::Removed { item: 8 },
            Change::Inserted { item: 9, at: (1, 3) },
        ];
        let headers = |frames: Vec<Vec<u8>>| -> Vec<u16> {
            frames
                .iter()
                .map(|frame| u16::from_le_bytes([frame[0], frame[1]]))
                .collect()
        };
        assert_eq!(
            headers(taken(&changes)),
            vec![item_packets::ITEM_UPDATE_QUANTITY, item_packets::ITEM_REMOVE]
        );
        assert_eq!(
            headers(placed(&changes)),
            vec![item_packets::ITEM_MOVE_TO_LOCATION]
        );
    }
}
