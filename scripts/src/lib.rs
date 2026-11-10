//! Content behaviour, written against hooks.
//!
//! Reference: `src/server/scripts/`. Two pieces are the structure: `ScriptMgr`, the registry of
//! hooks, and `AddScripts`, the list of what is written against them. The engine never names a
//! quest, a district or a boss; content does, here and in `modules/`.

#[path = "ScriptMgr/mod.rs"]
pub mod script_mgr;
#[path = "AddScripts.rs"]
mod add_scripts;
#[path = "AddModulesScripts.rs"]
mod add_modules_scripts;

pub use add_modules_scripts::add_modules_scripts;
pub use add_scripts::add_scripts;
pub use script_mgr::{ScriptLoader, ScriptMgr, Summary};
