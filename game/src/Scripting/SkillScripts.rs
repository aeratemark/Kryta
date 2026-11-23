//! Skill scripts: the Lua host the skill engine asks what a skill does
//! (`docs/decisions/0016-skills-are-lua-scripts.md`).
//!
//! Every skill row in the pack's `skills.json` names a script (`skills/<name>.lua`). A script is a chunk
//! that returns a table with an `on_use(cast)` function. When the skill completes, the engine hands the
//! script a `cast` table - the skill, its rank, the rank-read `scale`/`bonus`/`duration`, and copies of
//! the caster and the target - and the script *queues* operations on it:
//!
//! ```lua
//! return {
//!     on_use = function(cast)
//!         if cast.target.hexed then
//!             cast:heal{ to = "target", amount = cast.scale * 2 }
//!         else
//!             cast:heal{ to = "target", amount = "scale", divine_favor = true }
//!         end
//!     end,
//! }
//! ```
//!
//! A queued operation is the same word and fields ASURA's `skills.json` `effects` list used, read by the
//! same reader (`crate::skills::catalog::read_op`), and run by the ported engine. So the script decides
//! *what* happens; every number, timer, message and rule of *how* it happens stays ASURA's.
//!
//! The state is sandboxed: only Lua's `table`, `string` and `math` libraries are open (no `io`, `os`,
//! `package` or `debug`), and a call that runs past its instruction budget is stopped and reported as a
//! fault rather than stalling the district.
//!
//! Every script is read and checked once, at start-up ([`LuaSkills::load`]): a missing file, a syntax
//! error or a chunk without `on_use` stops the program naming the skill (AGENTS.md rule 7). Nothing on
//! a request path reads a file.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use mlua::{Function, HookTriggers, Lua, LuaOptions, LuaSerdeExt, RegistryKey, StdLib, Table, Value};

use crate::skills::catalog::read_op;
use crate::skills::engine::{CastView, SkillScripts, Snapshot};
use crate::skills::model::{Op, SkillDef, Skills};

/// The operation words a script can queue: `cast:<word>{...}` and the nested constructor `op.<word>{...}`.
/// The list is `catalog::read_op`'s, which refuses any other word.
pub const OP_WORDS: [&str; 15] = [
    "attack",
    "damage",
    "heal",
    "life_steal",
    "condition",
    "knockdown",
    "interrupt",
    "effect",
    "field",
    "chain",
    "drain_energy",
    "remove_hex",
    "resurrect",
    "lose_adrenaline",
    "unsupported",
];

/// How many VM instructions one `on_use` call may run, in thousands, before it is stopped.
const BUDGET_THOUSANDS: u32 = 1_000;

/// The prelude every script runs after: the `op` constructors and the `cast` methods.
const PRELUDE: &str = r#"
op = {}
Cast = {}
Cast.__index = Cast
for _, word in ipairs(OP_WORDS) do
    op[word] = function(fields)
        local queued = {}
        for key, value in pairs(fields or {}) do queued[key] = value end
        queued.op = word
        return queued
    end
    Cast[word] = function(self, fields)
        table.insert(self._queue, op[word](fields))
    end
end
function Cast:run(queued)
    assert(type(queued) == "table" and type(queued.op) == "string", "cast:run takes an op.<word>{...}")
    table.insert(self._queue, queued)
end
"#;

/// The loaded scripts, one Lua state for the world.
pub struct LuaSkills {
    lua: Lua,
    /// Each scripted skill's `on_use`.
    on_use: HashMap<u32, RegistryKey>,
    /// Thousands of instructions the call in progress has run.
    spent: Arc<AtomicU32>,
}

impl LuaSkills {
    /// Read and check every script the pack's skills name, relative to the pack directory.
    pub fn load(pack: &Path, skills: &Skills) -> Result<LuaSkills, String> {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH,
            LuaOptions::default(),
        )
        .map_err(|err| format!("skill scripts: the Lua state did not open: {err}"))?;
        let spent = Arc::new(AtomicU32::new(0));
        let budget = Arc::clone(&spent);
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(1000),
            move |_lua, _debug| {
                if budget.fetch_add(1, Ordering::Relaxed) >= BUDGET_THOUSANDS {
                    Err(mlua::Error::runtime("the script ran past its instruction budget"))
                } else {
                    Ok(mlua::VmState::Continue)
                }
            },
        );
        lua.globals()
            .set("OP_WORDS", OP_WORDS.to_vec())
            .and_then(|()| lua.load(PRELUDE).set_name("skill prelude").exec())
            .map_err(|err| format!("skill scripts: the prelude failed: {err}"))?;

        let mut ids: Vec<&SkillDef> = skills.by_id.values().filter(|def| def.script.is_some()).collect();
        ids.sort_by_key(|def| def.id);
        let mut on_use = HashMap::new();
        for def in ids {
            let relative = def.script.as_deref().unwrap_or_default();
            let path = pack.join(relative);
            let name = format!("skill {} ({})", def.id, def.name);
            let source = std::fs::read_to_string(&path)
                .map_err(|err| format!("{name}: {} is not readable: {err}", path.display()))?;
            spent.store(0, Ordering::Relaxed);
            let chunk: Table = lua
                .load(source.as_str())
                .set_name(relative)
                .eval()
                .map_err(|err| format!("{name}: {relative} does not return a table: {err}"))?;
            let function: Function = chunk
                .get("on_use")
                .map_err(|err| format!("{name}: {relative} has no on_use function: {err}"))?;
            let key = lua
                .create_registry_value(function)
                .map_err(|err| format!("{name}: {err}"))?;
            on_use.insert(def.id, key);
        }
        Ok(LuaSkills { lua, on_use, spent })
    }

    /// How many skills have a script loaded.
    pub fn len(&self) -> usize {
        self.on_use.len()
    }

    /// Whether no skill has a script.
    pub fn is_empty(&self) -> bool {
        self.on_use.is_empty()
    }

    fn snapshot(&self, view: &Snapshot) -> mlua::Result<Table> {
        let table = self.lua.create_table()?;
        table.set("agent", view.agent)?;
        table.set("alive", view.alive)?;
        table.set("health", view.health)?;
        table.set("max_health", view.max_health)?;
        table.set("energy", view.energy)?;
        table.set("max_energy", view.max_energy)?;
        table.set("level", view.level)?;
        table.set("x", view.position.0)?;
        table.set("y", view.position.1)?;
        table.set("hexed", view.hexed)?;
        table.set("enchanted", view.enchanted)?;
        table.set("attacking", view.attacking)?;
        table.set("casting", view.casting)?;
        let conditions = self.lua.create_table()?;
        for condition in &view.conditions {
            conditions.set(condition.word(), true)?;
        }
        table.set("conditions", conditions)?;
        table.set("effects", view.effects.clone())?;
        Ok(table)
    }

    fn call(&self, def: &SkillDef, view: &CastView) -> mlua::Result<Vec<serde_json::Value>> {
        let Some(key) = self.on_use.get(&def.id) else {
            return Err(mlua::Error::runtime("no script is loaded for this skill"));
        };
        let function: Function = self.lua.registry_value(key)?;
        let cast = self.lua.create_table()?;
        cast.set("skill", view.skill)?;
        cast.set("name", def.name.as_str())?;
        cast.set("rank", view.rank)?;
        cast.set("scale", view.scale)?;
        cast.set("bonus", view.bonus)?;
        cast.set("duration", view.duration)?;
        cast.set("aoe", def.aoe)?;
        cast.set("caster", self.snapshot(&view.caster)?)?;
        cast.set("target", self.snapshot(&view.target)?)?;
        cast.set("_queue", self.lua.create_table()?)?;
        let class: Table = self.lua.globals().get("Cast")?;
        cast.set_metatable(Some(class));
        self.spent.store(0, Ordering::Relaxed);
        function.call::<()>(cast.clone())?;
        let queue: Table = cast.get("_queue")?;
        let mut out = Vec::new();
        for entry in queue.sequence_values::<Value>() {
            out.push(self.lua.from_value::<serde_json::Value>(entry?)?);
        }
        Ok(out)
    }
}

impl SkillScripts for LuaSkills {
    fn ops(&mut self, def: &SkillDef, view: &CastView) -> Result<Vec<Op>, String> {
        let queued = self.call(def, view).map_err(|err| err.to_string())?;
        queued
            .iter()
            .enumerate()
            .map(|(n, entry)| read_op(entry).map_err(|err| format!("queued operation {}: {err}", n + 1)))
            .collect()
    }
}

// SAFETY of `Send`: mlua's `send` feature makes `Lua` Send; the engine holds the host behind a Mutex,
// so one call runs at a time.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::catalog;

    fn pack_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/World/Vanilla")
    }

    fn pack() -> catalog::Pack {
        catalog::load(&pack_dir()).expect("the committed pack")
    }

    #[test]
    fn every_committed_skill_names_a_script_that_loads() {
        let pack = pack();
        let scripts = LuaSkills::load(&pack_dir(), &pack.skills).expect("every script loads");
        assert_eq!(scripts.len(), pack.skills.by_id.len());
    }

    fn view(def: &SkillDef) -> CastView {
        let body = |agent| Snapshot {
            agent,
            alive: true,
            health: 400.0,
            max_health: 480,
            energy: 20.0,
            max_energy: 25,
            level: 20,
            position: (0.0, 0.0),
            conditions: Vec::new(),
            effects: Vec::new(),
            hexed: false,
            enchanted: false,
            attacking: false,
            casting: false,
        };
        CastView {
            skill: def.id,
            rank: 10,
            scale: 50.0,
            bonus: 5.0,
            duration: 8.0,
            caster: body(1),
            target: body(2),
        }
    }

    #[test]
    fn each_committed_script_queues_operations_the_reader_accepts() {
        let pack = pack();
        let mut scripts = LuaSkills::load(&pack_dir(), &pack.skills).unwrap();
        for def in pack.skills.by_id.values() {
            let ops = scripts.ops(def, &view(def)).unwrap_or_else(|err| panic!("{}: {err}", def.name));
            // A script with only an `unsupported` row queues nothing: the cast is refused first.
            if def.unsupported().is_none() {
                assert!(!ops.is_empty(), "{} queues nothing", def.name);
            }
        }
    }

    #[test]
    fn a_script_can_branch_on_the_cast_and_a_runaway_one_is_stopped() {
        let dir = std::env::temp_dir().join(format!("kryta-skill-scripts-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("skills")).unwrap();
        std::fs::write(
            dir.join("skills/branch.lua"),
            "return { on_use = function(cast)\n  if cast.target.hexed then cast:heal{ to = \"target\", amount = cast.scale * 2 }\n  else cast:damage{ to = \"target\", kind = \"fire\", amount = \"scale\" } end\nend }",
        )
        .unwrap();
        std::fs::write(
            dir.join("skills/spin.lua"),
            "return { on_use = function(cast) while true do end end }",
        )
        .unwrap();
        let pack = pack();
        let mut branch = pack.skills.by_id.values().next().unwrap().clone();
        branch.id = 90_001;
        branch.script = Some("skills/branch.lua".into());
        let mut spin = branch.clone();
        spin.id = 90_002;
        spin.script = Some("skills/spin.lua".into());
        let skills = Skills {
            by_id: [(branch.id, branch.clone()), (spin.id, spin.clone())].into_iter().collect(),
        };
        let mut scripts = LuaSkills::load(&dir, &skills).unwrap();

        let mut hexed = view(&branch);
        hexed.target.hexed = true;
        assert!(matches!(scripts.ops(&branch, &hexed).unwrap()[..], [Op::Heal { .. }]));
        assert!(matches!(scripts.ops(&branch, &view(&branch)).unwrap()[..], [Op::Damage { .. }]));
        let fault = scripts.ops(&spin, &view(&spin)).unwrap_err();
        assert!(fault.contains("budget"), "{fault}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
