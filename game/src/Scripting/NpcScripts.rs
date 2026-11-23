//! NPC scripts: the Lua host that runs the pack's `scripts/runtime.lua` and its `dialogs/*.lua`
//! (`docs/decisions/0019-npc-and-quest-logic-is-lua.md`).
//!
//! The owner's decision (2026-11-23): what an NPC says and what a choice does is decided by Lua - the
//! game-logic half of `pseudo_lua/runtime.lua`, made real - while every message stays the Rust port of
//! ASURA's. So the split is the skill scripts' (decision 0016): **the script decides what happens; the
//! ported Rust decides how it reaches the client.** This host asks the runtime four questions - which
//! page a talk opens, what a page shows this character, what a choice does, and which page follows a
//! quest step - and hands the answers to `game/Handlers/NpcHandler.rs`, which sends them in ASURA's
//! order (`ASURA/crates/asura-world/src/vendor/mod.rs:218-298`, `:723-900`).
//!
//! What it replaces is ASURA's `dialogs.json` reader (`ASURA/crates/asura-world/src/dialog.rs`): the
//! same pages, choices, conditions and actions, with the same checks at load, written as Lua so a page
//! or a choice may carry a scripted condition (`when = function(ctx) ... end`) and a dialog its own
//! `on_talk` / `on_choose` hooks.
//!
//! Text is one of three things, turned into the client's units here ([`text_units`]):
//!
//! * a Lua string - a custom string, sent as literal text: `0x0108 0x0107 <UTF-16 units> 0x0001`, the
//!   form preforged's `dialogue::open` sends (`gw_npc_dialogue.h`) and ASURA's `dialog.rs:10-12`
//!   documents. Until kryta-CS can install strings into `Gw.dat`, this is how a custom line is shown;
//! * `gw(...)` - a real GW string: the game's own encoded units, as a capture carried them;
//! * `mix(...)` - a hacked-up GW string: its parts' units joined in order. Experimental: whether the
//!   client draws a given combination is only known once it has been tried.
//!
//! The state is sandboxed (`table`, `string`, `math`; `dofile`, `loadfile` and `load` removed), every
//! call has an instruction budget, and every file is read once, at start-up ([`NpcScripts::load`]):
//! a syntax error, a dialog that does not check or a text that does not convert stops the program
//! naming it (AGENTS.md rule 7). Nothing on a request path reads a file.

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use mlua::{Function, HookTriggers, Lua, LuaOptions, RegistryKey, StdLib, Table, Value};

use crate::server::packets::dialog_packets::{DIALOG_LABEL_UNITS, DIALOG_NO_SKILL};

/// The runtime inside a pack.
pub const RUNTIME_FILE: &str = "scripts/runtime.lua";
/// The folder inside a pack that holds one dialog file per NPC.
pub const DIALOG_DIR: &str = "dialogs";

/// How many VM instructions one call may run, in thousands, before it is stopped.
const BUDGET_THOUSANDS: u32 = 1_000;

/// The marker that opens a literal string, and the one that closes it (`gw_npc_dialogue.h`;
/// `game/Handlers/NpcHandler.rs` `NpcTarget::name_text` reads the same form back).
const LITERAL_OPEN: [u16; 2] = [0x0108, 0x0107];
const LITERAL_CLOSE: u16 = 0x0001;

/// The character a talk is for, as the runtime reads it (`ctx.char`).
#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    /// Its name.
    pub name: String,
    /// Its level.
    pub level: u32,
    /// Its primary profession, by the client's number.
    pub primary: u8,
    /// Its secondary profession, by the client's number; 0 for none.
    pub secondary: u8,
}

/// Who is talking to whom: the map, the NPC's name and agent, and the character.
#[derive(Debug, Clone, Copy)]
pub struct Talk<'a> {
    /// The map the NPC stands on.
    pub map: i32,
    /// The NPC's name, as its placement carries it.
    pub npc: &'a str,
    /// The NPC's agent.
    pub agent: u32,
    /// The character.
    pub character: &'a Character,
}

/// A page as one character sees it: the body and the choices shown to it.
#[derive(Debug, Clone, PartialEq)]
pub struct PageView {
    /// The `0x0080` units.
    pub body: Vec<u16>,
    /// The `0x007E` choices, in the order they are drawn.
    pub buttons: Vec<ButtonView>,
}

/// One choice as it is drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonView {
    /// The icon (`0x007E`'s first byte).
    pub icon: u8,
    /// The label units.
    pub label: Vec<u16>,
    /// The id the client sends back in `0x803B`.
    pub id: u32,
    /// The skill the choice names; [`DIALOG_NO_SKILL`] when it names none.
    pub skill: u32,
}

/// What choosing a button does (`ASURA/crates/asura-world/src/dialog.rs:175-206`, `Action`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Close the dialog and answer nothing.
    Close,
    /// Open a window of this kind, owned by the NPC (`0x0083`).
    Window {
        /// The window kind.
        window: u8,
    },
    /// Open the NPC's merchant window, through the vendor content and rules.
    Merchant,
    /// Show another page of this dialog (numbered from 1).
    Page {
        /// The page.
        page: usize,
    },
    /// A quest button: offer the quest, take its turn-in, or complete its talk objective.
    Quest {
        /// The client's quest id.
        quest: u32,
        /// Which end of the quest.
        step: QuestStep,
        /// The page shown after the step, when the content names one (numbered from 1).
        page: Option<usize>,
    },
    /// A choice whose answer no capture shows. Logged, and nothing opens.
    Unanswered,
}

/// Which end of a quest a choice is (`dialog.rs:208-219`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestStep {
    /// The NPC offers the quest.
    Offer,
    /// The NPC takes the turn-in.
    Reward,
    /// Talking to this NPC is the quest's objective.
    Credit,
}

/// The loaded runtime and dialogs: one Lua state for the world.
pub struct NpcScripts {
    lua: Lua,
    runtime: RegistryKey,
    /// Thousands of instructions the call in progress has run.
    spent: Arc<AtomicU32>,
    /// One call at a time, so the budget is the call's own.
    gate: Mutex<()>,
    dialogs: usize,
    npcs: usize,
}

impl std::fmt::Debug for NpcScripts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NpcScripts")
            .field("dialogs", &self.dialogs)
            .field("npcs", &self.npcs)
            .finish_non_exhaustive()
    }
}

/// A custom string as the client's literal form: the markers, the text's UTF-16 units without control
/// characters, and the terminator. Any character survives, not only ASCII: a unit is a UTF-16 unit.
pub fn literal(text: &str) -> Vec<u16> {
    let mut units = LITERAL_OPEN.to_vec();
    units.extend(text.encode_utf16().filter(|unit| *unit >= 0x20));
    units.push(LITERAL_CLOSE);
    units
}

/// A text value as the client's units: a string is a custom string, `gw(...)` the game's own units,
/// `mix(...)` its parts joined in order.
pub fn text_units(value: &Value) -> Result<Vec<u16>, String> {
    match value {
        Value::String(text) => {
            let text = text.to_str().map_err(|err| format!("a string that is not UTF-8: {err}"))?;
            Ok(literal(&text))
        }
        Value::Table(table) => {
            let kind: Option<String> = table.get("kind").map_err(|err| err.to_string())?;
            match kind.as_deref() {
                Some("gw") => {
                    let list: Table = table.get("units").map_err(|err| err.to_string())?;
                    let mut out = Vec::new();
                    for unit in list.sequence_values::<Value>() {
                        let unit = unit.map_err(|err| err.to_string())?;
                        let number = match unit {
                            Value::Integer(n) => n,
                            Value::Number(n) if n.fract() == 0.0 => n as i64,
                            other => {
                                return Err(format!("gw() takes whole numbers, found {}", other.type_name()));
                            }
                        };
                        let unit = u16::try_from(number)
                            .map_err(|_| format!("gw() unit {number} is not from 0 to 65535"))?;
                        out.push(unit);
                    }
                    Ok(out)
                }
                Some("mix") => {
                    let parts: Table = table.get("parts").map_err(|err| err.to_string())?;
                    let mut out = Vec::new();
                    for part in parts.sequence_values::<Value>() {
                        let part = part.map_err(|err| err.to_string())?;
                        out.extend(text_units(&part)?);
                    }
                    Ok(out)
                }
                _ => Err("a table that is not gw(...) or mix(...)".to_string()),
            }
        }
        other => Err(format!(
            "a {} is not a text; write a string, gw(...) or mix(...)",
            other.type_name()
        )),
    }
}

impl NpcScripts {
    /// Read the pack's runtime and every dialog file, and check them all. `Ok(None)` for a pack with
    /// neither; a pack with dialogs and no runtime is a fault.
    pub fn load(pack: &Path) -> Result<Option<NpcScripts>, String> {
        let runtime_path = pack.join(RUNTIME_FILE);
        let dialog_dir = pack.join(DIALOG_DIR);
        if !runtime_path.exists() {
            if dialog_dir.exists() {
                return Err(format!(
                    "{} exists, and {} does not: the dialogs have nothing to run them",
                    dialog_dir.display(),
                    runtime_path.display()
                ));
            }
            return Ok(None);
        }
        let lua = Lua::new_with(StdLib::TABLE | StdLib::STRING | StdLib::MATH, LuaOptions::default())
            .map_err(|err| format!("NPC scripts: the Lua state did not open: {err}"))?;
        for name in ["dofile", "loadfile", "load"] {
            lua.globals()
                .set(name, Value::Nil)
                .map_err(|err| format!("NPC scripts: {err}"))?;
        }
        let spent = Arc::new(AtomicU32::new(0));
        let budget = Arc::clone(&spent);
        lua.set_hook(HookTriggers::new().every_nth_instruction(1000), move |_lua, _debug| {
            if budget.fetch_add(1, Ordering::Relaxed) >= BUDGET_THOUSANDS {
                Err(mlua::Error::runtime("the script ran past its instruction budget"))
            } else {
                Ok(mlua::VmState::Continue)
            }
        });

        let source = std::fs::read_to_string(&runtime_path)
            .map_err(|err| format!("{}: {err}", runtime_path.display()))?;
        let runtime: Table = lua
            .load(source.as_str())
            .set_name(RUNTIME_FILE)
            .eval()
            .map_err(|err| format!("{RUNTIME_FILE}: {err}"))?;
        let add_dialog: Function = runtime
            .get("add_dialog")
            .map_err(|err| format!("{RUNTIME_FILE}: no add_dialog: {err}"))?;

        let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(&dialog_dir) {
            Ok(entries) => entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().and_then(|kind| kind.to_str()) == Some("lua"))
                .collect(),
            Err(_) => Vec::new(),
        };
        files.sort();
        for path in &files {
            let relative = format!(
                "{DIALOG_DIR}/{}",
                path.file_name().and_then(|name| name.to_str()).unwrap_or_default()
            );
            let source =
                std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
            spent.store(0, Ordering::Relaxed);
            let chunk: Value = lua
                .load(source.as_str())
                .set_name(relative.as_str())
                .eval()
                .map_err(|err| format!("{relative}: {err}"))?;
            spent.store(0, Ordering::Relaxed);
            add_dialog
                .call::<()>((relative.as_str(), chunk))
                .map_err(|err| format!("{relative}: {err}"))?;
        }

        // The structure, as ASURA's `DialogFile::check` (`dialog.rs:229-370`), then every text.
        spent.store(0, Ordering::Relaxed);
        let check: Function = runtime.get("check").map_err(|err| format!("{RUNTIME_FILE}: {err}"))?;
        let mut problems: Vec<String> =
            check.call(()).map_err(|err| format!("{RUNTIME_FILE}: check: {err}"))?;
        spent.store(0, Ordering::Relaxed);
        let texts: Function = runtime.get("texts").map_err(|err| format!("{RUNTIME_FILE}: {err}"))?;
        let texts: Table = texts.call(()).map_err(|err| format!("{RUNTIME_FILE}: texts: {err}"))?;
        for entry in texts.sequence_values::<Table>() {
            let entry = entry.map_err(|err| err.to_string())?;
            let place: String = entry.get("where").map_err(|err| err.to_string())?;
            let text: Value = entry.get("text").map_err(|err| err.to_string())?;
            let label: bool = entry.get("label").map_err(|err| err.to_string())?;
            match text_units(&text) {
                Err(err) => problems.push(format!("{place}: {err}")),
                Ok(units) if units.is_empty() => problems.push(format!("{place}: an empty text")),
                Ok(units) if label && DIALOG_LABEL_UNITS < units.len() => problems.push(format!(
                    "{place}: a label of {} units; the client takes at most {DIALOG_LABEL_UNITS}",
                    units.len()
                )),
                Ok(_) => {}
            }
        }
        if !problems.is_empty() {
            return Err(format!(
                "{} dialog problem(s) in {}:\n  {}",
                problems.len(),
                pack.display(),
                problems.join("\n  ")
            ));
        }

        let count: Function = runtime.get("count").map_err(|err| format!("{RUNTIME_FILE}: {err}"))?;
        let (dialogs, npcs): (usize, usize) =
            count.call(()).map_err(|err| format!("{RUNTIME_FILE}: count: {err}"))?;
        let runtime = lua
            .create_registry_value(runtime)
            .map_err(|err| format!("NPC scripts: {err}"))?;
        Ok(Some(NpcScripts {
            lua,
            runtime,
            spent,
            gate: Mutex::new(()),
            dialogs,
            npcs,
        }))
    }

    /// How many dialogs are loaded, and for how many NPC names.
    pub fn counts(&self) -> (usize, usize) {
        (self.dialogs, self.npcs)
    }

    /// Call `R.<name>(args)` with the budget reset, one call at a time.
    fn call<A, R>(&self, name: &str, args: impl FnOnce(&Lua) -> mlua::Result<A>) -> Result<R, String>
    where
        A: mlua::IntoLuaMulti,
        R: mlua::FromLuaMulti,
    {
        let _gate = self.gate.lock().map_err(|_| "the NPC script host is poisoned".to_string())?;
        let run = || -> mlua::Result<R> {
            let runtime: Table = self.lua.registry_value(&self.runtime)?;
            let function: Function = runtime.get(name)?;
            let args = args(&self.lua)?;
            self.spent.store(0, Ordering::Relaxed);
            function.call(args)
        };
        run().map_err(|err| format!("R.{name}: {err}"))
    }

    fn context(lua: &Lua, talk: &Talk<'_>) -> mlua::Result<Table> {
        let ctx = lua.create_table()?;
        ctx.set("map", talk.map)?;
        ctx.set("npc", talk.npc)?;
        ctx.set("agent", talk.agent)?;
        let character = lua.create_table()?;
        character.set("name", talk.character.name.as_str())?;
        character.set("level", talk.character.level)?;
        character.set("primary", talk.character.primary)?;
        character.set("secondary", talk.character.secondary)?;
        // The quest state arrives with the quests round; until then nothing is accepted.
        character.set("quests", lua.create_table()?)?;
        ctx.set("char", character)?;
        Ok(ctx)
    }

    /// The page a talk opens, numbered from 1; `None` when the NPC has no dialog here, or no page of
    /// it is open to this character.
    pub fn talk(&self, talk: &Talk<'_>) -> Result<Option<usize>, String> {
        self.call("talk", |lua| Self::context(lua, talk))
    }

    /// Page `page` as this character sees it.
    pub fn view(&self, talk: &Talk<'_>, page: usize) -> Result<Option<PageView>, String> {
        let view: Option<Table> = self.call("view", |lua| Ok((Self::context(lua, talk)?, page)))?;
        let Some(view) = view else {
            return Ok(None);
        };
        let read = || -> Result<PageView, String> {
            let body: Value = view.get("body").map_err(|err| err.to_string())?;
            let body = text_units(&body).map_err(|err| format!("body: {err}"))?;
            let list: Table = view.get("buttons").map_err(|err| err.to_string())?;
            let mut buttons = Vec::new();
            for button in list.sequence_values::<Table>() {
                let button = button.map_err(|err| err.to_string())?;
                let label: Value = button.get("label").map_err(|err| err.to_string())?;
                buttons.push(ButtonView {
                    icon: button.get("icon").map_err(|err| err.to_string())?,
                    label: text_units(&label).map_err(|err| format!("label: {err}"))?,
                    id: button.get("id").map_err(|err| err.to_string())?,
                    skill: button
                        .get::<Option<u32>>("skill")
                        .map_err(|err| err.to_string())?
                        .unwrap_or(DIALOG_NO_SKILL),
                });
            }
            Ok(PageView { body, buttons })
        };
        read().map(Some).map_err(|err| format!("page {page}: {err}"))
    }

    /// What choosing `id` on page `page` does; `None` when the page does not show it to this
    /// character, which refuses the choice (`vendor/mod.rs:812-823`).
    pub fn choose(&self, talk: &Talk<'_>, page: usize, id: u32) -> Result<Option<Action>, String> {
        let action: Option<Table> =
            self.call("choose", |lua| Ok((Self::context(lua, talk)?, page, id)))?;
        let Some(action) = action else {
            return Ok(None);
        };
        read_action(&action).map(Some)
    }

    /// The first open page that offers or rewards a quest: what follows a quest step
    /// (`vendor/mod.rs:841-853`).
    pub fn next_quest_page(&self, talk: &Talk<'_>) -> Result<Option<usize>, String> {
        self.call("next_quest_page", |lua| Self::context(lua, talk))
    }

    /// The talk range the dialog gives the NPC called `npc` on `map`, if it gives one
    /// (`dialog.rs:373-376`, `talk_range_for`).
    pub fn talk_range(&self, map: i32, npc: &str) -> Option<f32> {
        self.call::<_, Option<f32>>("talk_range", |_| Ok((map, npc)))
            .ok()
            .flatten()
    }
}

fn read_action(action: &Table) -> Result<Action, String> {
    let kind: String = action.get("kind").map_err(|err| format!("action: {err}"))?;
    let number = |key: &str| -> Result<Option<i64>, String> {
        action
            .get::<Option<i64>>(key)
            .map_err(|err| format!("action.{key}: {err}"))
    };
    let page = |value: Option<i64>| -> Result<Option<usize>, String> {
        value
            .map(|page| usize::try_from(page).map_err(|_| format!("action.page {page}")))
            .transpose()
    };
    Ok(match kind.as_str() {
        "close" => Action::Close,
        "unanswered" => Action::Unanswered,
        "merchant" => Action::Merchant,
        "window" => Action::Window {
            window: number("window")?
                .and_then(|window| u8::try_from(window).ok())
                .ok_or("action.window is not a byte")?,
        },
        "page" => Action::Page {
            page: page(number("page")?)?.ok_or("action.page is missing")?,
        },
        "quest" => {
            let step: Option<String> = action.get("step").map_err(|err| format!("action.step: {err}"))?;
            Action::Quest {
                quest: number("quest")?
                    .and_then(|quest| u32::try_from(quest).ok())
                    .ok_or("action.quest is not a quest id")?,
                step: match step.as_deref() {
                    None | Some("offer") => QuestStep::Offer,
                    Some("reward") => QuestStep::Reward,
                    Some("credit") => QuestStep::Credit,
                    Some(other) => return Err(format!("action.step {other:?}")),
                },
                page: page(number("page")?)?,
            }
        }
        other => return Err(format!("action kind {other:?}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/World/Vanilla")
    }

    fn tester() -> Character {
        Character {
            name: "Tester".to_string(),
            level: 2,
            primary: 1,
            secondary: 0,
        }
    }

    #[test]
    fn a_custom_string_is_the_literal_form_in_utf16() {
        assert_eq!(literal("Hi"), vec![0x0108, 0x0107, 0x48, 0x69, 0x0001]);
        // Not only ASCII: one UTF-16 unit per character here, a control character dropped.
        assert_eq!(
            literal("\u{e9}\u{fc}\n!"),
            vec![0x0108, 0x0107, 0x00E9, 0x00FC, 0x21, 0x0001]
        );
        // It reads back as text through the same rule a placement's name does.
        let units = literal("Welcome to Ascalon.");
        let inner = &units[2..units.len() - 1];
        assert_eq!(String::from_utf16(inner).unwrap(), "Welcome to Ascalon.");
    }

    #[test]
    fn the_three_kinds_of_text_convert() {
        let lua = Lua::new();
        lua.load(
            r#"
            function gw(...) return { kind = "gw", units = { ... } } end
            function mix(...) return { kind = "mix", parts = { ... } } end
            "#,
        )
        .exec()
        .unwrap();
        let custom: Value = lua.load(r#"return "Hi""#).eval().unwrap();
        let real: Value = lua.load("return gw(33026, 15998)").eval().unwrap();
        let mixed: Value = lua.load(r#"return mix(gw(33026), "Hi", gw(2))"#).eval().unwrap();
        let wrong: Value = lua.load("return gw(70000)").eval().unwrap();
        assert_eq!(text_units(&custom).unwrap(), literal("Hi"));
        assert_eq!(text_units(&real).unwrap(), vec![33026, 15998]);
        assert_eq!(
            text_units(&mixed).unwrap(),
            vec![33026, 0x0108, 0x0107, 0x48, 0x69, 0x0001, 2]
        );
        assert!(text_units(&wrong).unwrap_err().contains("70000"));
    }

    #[test]
    fn the_committed_dialogs_load_and_check() {
        let scripts = NpcScripts::load(&pack())
            .expect("every committed dialog checks")
            .expect("the pack has a runtime");
        // ASURA's dialogs.json: 43 dialogs, for 42 NPC names (Prince Rurik speaks on maps 146 and 148).
        assert_eq!(scripts.counts().0, 43);
        assert_eq!(scripts.counts().1, 42);
        assert_eq!(scripts.talk_range(148, "Sir Tydus"), Some(200.0));
    }

    #[test]
    fn the_purveyor_opens_its_captured_menu_and_its_merchant_choice() {
        // `movement-live-20261109-200703.jsonl` seq 5176-5185: one body, seven choices.
        let scripts = NpcScripts::load(&pack()).unwrap().unwrap();
        let character = tester();
        let talk = Talk {
            map: 148,
            npc: "The Purveyor [Bonus Merchant]",
            agent: 40,
            character: &character,
        };
        let page = scripts.talk(&talk).unwrap().expect("a page opens");
        assert_eq!(page, 1);
        let view = scripts.view(&talk, page).unwrap().unwrap();
        assert_eq!(view.body, vec![33026, 15998, 55221, 56292, 18930]);
        assert_eq!(view.buttons.len(), 7);
        assert_eq!(view.buttons[0].skill, 0, "the one choice the live server sent skill 0 on");
        assert_eq!(view.buttons[1].skill, DIALOG_NO_SKILL);
        assert_eq!(scripts.choose(&talk, page, 139).unwrap(), Some(Action::Merchant));
        assert_eq!(scripts.choose(&talk, page, 140).unwrap(), Some(Action::Window { window: 5 }));
        assert_eq!(scripts.choose(&talk, page, 999).unwrap(), None, "not on the page: refused");
    }

    #[test]
    fn a_quest_page_stays_closed_while_no_quest_is_loaded() {
        // Sir Tydus's first eight pages wait for a quest state; with no quests they are closed, as
        // ASURA answers with no quests.json, and a talk opens his ninth: the greeting, whose "page"
        // choice opens the tenth, an answer page.
        let scripts = NpcScripts::load(&pack()).unwrap().unwrap();
        let character = tester();
        let talk = Talk {
            map: 148,
            npc: "Sir Tydus",
            agent: 41,
            character: &character,
        };
        assert_eq!(scripts.talk(&talk).unwrap(), Some(9));
        let view = scripts.view(&talk, 9).unwrap().unwrap();
        assert_eq!(view.body, vec![13188, 46997, 46533, 25935]);
        assert_eq!(view.buttons.iter().map(|b| b.id).collect::<Vec<_>>(), vec![132, 128]);
        assert_eq!(scripts.choose(&talk, 9, 132).unwrap(), Some(Action::Page { page: 10 }));
        assert_eq!(scripts.choose(&talk, 9, 128).unwrap(), Some(Action::Close));
        assert_eq!(scripts.next_quest_page(&talk).unwrap(), None);
    }

    #[test]
    fn a_script_can_choose_its_own_page_and_a_broken_dialog_is_named() {
        let dir = std::env::temp_dir().join(format!("kryta-npc-scripts-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("dialogs")).unwrap();
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::copy(pack().join(RUNTIME_FILE), dir.join(RUNTIME_FILE)).unwrap();
        std::fs::write(
            dir.join("dialogs/greeter.lua"),
            r#"return {
                npc = "Greeter",
                on_talk = function(ctx) if ctx.char.level >= 10 then return 2 end end,
                [148] = { pages = {
                    { body = "Hello, newcomer.", buttons = {
                        { icon = 15, label = "Bye", id = 1, action = { kind = "close" } } } },
                    { body = mix(gw(33026), " veteran"), buttons = {},
                      when = function(ctx) return ctx.char.level >= 10 end },
                } },
            }"#,
        )
        .unwrap();
        let scripts = NpcScripts::load(&dir).unwrap().unwrap();
        let mut character = tester();
        let talk = |character| Talk {
            map: 148,
            npc: "Greeter",
            agent: 9,
            character,
        };
        assert_eq!(scripts.talk(&talk(&character)).unwrap(), Some(1));
        let view = scripts.view(&talk(&character), 1).unwrap().unwrap();
        assert_eq!(view.body, literal("Hello, newcomer."));
        assert_eq!(view.buttons[0].label, literal("Bye"));
        character.level = 12;
        assert_eq!(scripts.talk(&talk(&character)).unwrap(), Some(2));

        std::fs::write(
            dir.join("dialogs/broken.lua"),
            r#"return { npc = "Broken", [148] = { pages = {
                { body = "x", buttons = { { icon = 1, label = "y", id = 5, action = { kind = "page", page = 9 } } } },
            } } }"#,
        )
        .unwrap();
        let fault = NpcScripts::load(&dir).unwrap_err();
        assert!(fault.contains("broken.lua") && fault.contains("page 9"), "{fault}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
