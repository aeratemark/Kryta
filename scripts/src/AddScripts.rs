//! `AddScripts`: the function a program hands the script manager as its loader.
//!
//! Reference: `void AddScripts()`, whose body calls `AddSC_*` once per script module in the tree -
//! `src/server/scripts/Custom/custom_script_loader.cpp` is the shape a new script arrives in
//! (`AddCustomScripts`, one commented-out call per script until it exists). This tree has no script
//! module yet, so the body registers nothing - and registering nothing is a *result*, not a
//! placeholder: the program reports `0 scripts, 0 hooks`, and the day the first script lands this
//! function grows one line.

use game::scripting::ScriptMgr;

/// Register every script this build has.
pub fn add_scripts(_mgr: &mut ScriptMgr) {
    // No script exists yet. The first one adds
    // `mgr.add_world_script(Box::new(MyScript::new()), &[WorldHook::OnStartup])` here.
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::scripting::Summary;

    /// The loader is the reference's shape and the state of this tree: it runs, and it has nothing
    /// to register yet.
    #[test]
    fn the_loader_runs_and_registers_nothing_yet() {
        let mut mgr = ScriptMgr::new();
        mgr.set_script_loader(add_scripts);
        assert_eq!(mgr.initialize().unwrap(), Summary::default());
        assert_eq!(mgr.script_count(), 0);
    }
}
