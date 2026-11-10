//! `AddScripts`: the function a program hands the script manager as its loader.
//!
//! Reference: `void AddScripts()` in `src/server/scripts/ScriptMgr.h`'s world, whose body calls
//! `AddSC_*` once per script module in the tree. This tree has no script modules yet, so the body
//! registers nothing - and registering nothing is a *result*, not a placeholder: the program reports
//! `0 scripts, 0 hooks`, and the day the first module lands this function grows one line.

use crate::script_mgr::ScriptMgr;

/// Register every script this build has.
pub fn add_scripts(_mgr: &mut ScriptMgr) {
    // No script module exists yet. The first one adds `mgr.add_script("name", hooks)` here.
}
