//! `AddModulesScripts`: the loader for scripts that live out of tree.
//!
//! Reference: `void AddModulesScripts()`, handed to `sScriptMgr->SetModulesLoader`. It is empty
//! until the first `mod-*` crate exists, for the same reason `AddScripts` is: a module registers
//! itself, and the tree has no module yet (`AGENTS.md`, `modules/`).

use crate::script_mgr::ScriptMgr;

/// Register every module script this build carries.
pub fn add_modules_scripts(_mgr: &mut ScriptMgr) {}
