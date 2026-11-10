//! The hook registry: what content is written against.
//!
//! Reference: `src/server/scripts/ScriptMgr.h` and `ScriptMgr.cpp`. The shape is the one that matters
//! and it is kept: a program sets a **loader**, calls `Initialize`, and the loader registers scripts
//! and the hooks they answer. A hook is a name the engine calls at a moment; a script is a named
//! group of hooks. Nothing is registered yet, so nothing is called - which is what a tree with no
//! scripts should do, and not what a stub should pretend.
//!
//! Reference: `ScriptMgr::SetScriptLoader`, `ScriptMgr::Initialize`, `ScriptMgr::OnStartup`.

use std::collections::BTreeMap;

/// What a loader is: a function that registers scripts on the manager it is given.
pub type ScriptLoader = fn(&mut ScriptMgr);

/// What `Initialize` found, for the program to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    /// Scripts the loader registered.
    pub scripts: usize,
    /// Hooks those scripts answer, over all of them.
    pub hooks: usize,
}

/// The registry itself.
///
/// The reference is a singleton (`sScriptMgr`). Here it is a value the caller owns, so a test can
/// build one without a process around it - the same choice this tree makes for the log and the
/// configuration.
#[derive(Debug, Default)]
pub struct ScriptMgr {
    loader: Option<ScriptLoader>,
    modules_loader: Option<ScriptLoader>,
    scripts: BTreeMap<String, usize>,
    /// Whether `Initialize` has run: a program that loads scripts twice would answer every hook twice.
    initialized: bool,
}

impl ScriptMgr {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// The function that registers scripts, set before [`ScriptMgr::initialize`].
    ///
    /// Reference: `ScriptMgr::SetScriptLoader(AddScripts)`.
    pub fn set_script_loader(&mut self, loader: ScriptLoader) {
        self.loader = Some(loader);
    }

    /// The function that registers scripts living out of tree.
    ///
    /// Reference: `ScriptMgr::SetModulesLoader(AddModulesScripts)`. It runs after the in-tree loader,
    /// which is the order the reference calls them in.
    pub fn set_modules_loader(&mut self, loader: ScriptLoader) {
        self.modules_loader = Some(loader);
    }

    /// Register one script under a name.
    ///
    /// Reference: `ScriptMgr::RegisterScript`, called from a loader. A name registered twice is
    /// refused rather than counted twice: two scripts answering one hook is a bug in the loader, and
    /// the count is what the program reports.
    pub fn add_script(&mut self, name: &str, hooks: usize) -> Result<(), String> {
        if self.scripts.contains_key(name) {
            return Err(format!("'{}' is registered twice", name));
        }
        self.scripts.insert(name.to_string(), hooks);
        Ok(())
    }

    /// Run the loader, once, and answer what it registered.
    ///
    /// Reference: `ScriptMgr::Initialize`, which calls the loader and then initialises what it built.
    /// A second call answers what the first one found instead of running the loader again.
    pub fn initialize(&mut self) -> Result<Summary, String> {
        if !self.initialized {
            if let Some(loader) = self.loader {
                loader(self);
            }
            if let Some(loader) = self.modules_loader {
                loader(self);
            }
            self.initialized = true;
        }
        Ok(self.summary())
    }

    /// What is registered now.
    pub fn summary(&self) -> Summary {
        Summary {
            scripts: self.scripts.len(),
            hooks: self.scripts.values().sum(),
        }
    }

    /// Whether [`ScriptMgr::initialize`] has run.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_scripts(mgr: &mut ScriptMgr) {
        mgr.add_script("mod-a", 3).unwrap();
        mgr.add_script("mod-b", 2).unwrap();
    }

    #[test]
    fn a_loader_registers_what_it_has_and_the_summary_counts_it() {
        let mut mgr = ScriptMgr::new();
        mgr.set_script_loader(two_scripts);
        assert_eq!(
            mgr.initialize().unwrap(),
            Summary {
                scripts: 2,
                hooks: 5
            }
        );
        assert!(mgr.is_initialized());
    }

    #[test]
    fn a_registry_with_no_scripts_is_initialised_and_empty() {
        // The state of this tree today: the loader exists and registers nothing.
        let mut mgr = ScriptMgr::new();
        mgr.set_script_loader(super::super::add_scripts);
        assert_eq!(mgr.initialize().unwrap(), Summary::default());
    }

    #[test]
    fn the_loader_runs_once_however_often_initialize_is_called() {
        let mut mgr = ScriptMgr::new();
        mgr.set_script_loader(two_scripts);
        let first = mgr.initialize().unwrap();
        let second = mgr.initialize().unwrap();
        assert_eq!(first, second, "hooks were not registered twice");
    }

    #[test]
    fn one_name_twice_is_refused() {
        let mut mgr = ScriptMgr::new();
        mgr.add_script("mod-a", 1).unwrap();
        let error = mgr.add_script("mod-a", 1).unwrap_err();
        assert!(error.contains("registered twice"), "{error}");
    }
}
