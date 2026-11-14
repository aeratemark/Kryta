//! `game/Scripting/` - the reference's `src/server/game/Scripting/` (109 files).
//!
//! Ported here: the manager and the world-script interface - `ScriptMgr.h` (940 lines),
//! `ScriptMgr.cpp` (292), `ScriptObject.h` (115) and `ScriptDefs/WorldScript.{h,cpp}` (110 and 60).
//! They are the part a program can drive today: a loader is set, `initialize` runs it, and the
//! engine raises the moments it has. The reference's shape is kept - a script is a named object that
//! declares the hooks it answers, the manager numbers it, and a hook is answered only by the scripts
//! that declared it (`ScriptRegistry<TScript>::AddScript(script, enabledHooks)`, `ScriptMgr.h:796` -
//! `EnabledHooks[hookType]` is why a hook is not broadcast to every script).
//!
//! **Ported thin, and said so.** `WorldHook` has the reference's `OnUpdate`, `OnStartup` and
//! `OnShutdown` only. The other ten world hooks (`WorldScript.h:30-43`: the open state, the
//! configuration reload pair, the custom database table, the motd, the shutdown code and mask, the
//! cancellation, the map unload, the cache version, the world-initialized moment) are each called by
//! an engine moment this tree does not have yet *and* carry arguments this tree has no counterpart
//! for - a locale, a `ShutdownExitCode`, a `ShutdownMask`, a client cache version, an MOTD text.
//! Writing those types here first would be inventing them; the hooks arrive with the moments that
//! raise them.
//!
//! Reference: `ScriptMgr::SetScriptLoader`, `SetModulesLoader`, `Initialize`, `Unload`,
//! `OnWorldUpdate`, `OnStartup`, `OnShutdown`.

use std::collections::BTreeMap;
use std::time::Duration;

/// What a loader is: a function that registers scripts on the manager it is given.
///
/// Reference: `ScriptLoaderCallbackType` (`ScriptMgr.h:127`), the callback the reference uses to
/// break the `game` <-> `scripts` cycle: the manager lives in the game library, the scripts that fill
/// it live outside it, and a function pointer is the bridge.
pub type ScriptLoader = fn(&mut ScriptMgr);

/// What [`ScriptMgr::initialize`] found, for the program to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    /// Scripts the loaders registered.
    pub scripts: usize,
    /// Hooks those scripts answer, over all of them.
    pub hooks: usize,
}

/// What nearly every script type shares.
///
/// Reference: `ScriptObject` (`ScriptObject.h:38`) - a name, and whether the database must name it
/// (`IsDatabaseBound`, false by default).
pub trait ScriptObject {
    /// The name the script is registered under, and the name the database would assign it by.
    fn name(&self) -> &str;

    /// Whether this script must be assigned in the database (`ScriptObject::IsDatabaseBound`).
    fn is_database_bound(&self) -> bool {
        false
    }
}

/// One moment the world raises for its scripts.
///
/// Reference: `enum WorldHook` (`WorldScript.h:30-43`) and the three implementations in
/// `WorldScript.cpp:53-67` (`OnWorldUpdate`, `OnStartup`, `OnShutdown`). See this module's header for
/// the ten that are not here yet and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldHook {
    /// `WORLDHOOK_ON_UPDATE`: every world tick. The reference's own comment: don't execute too heavy
    /// code here.
    OnUpdate,
    /// `WORLDHOOK_ON_STARTUP`: the world is started.
    OnStartup,
    /// `WORLDHOOK_ON_SHUTDOWN`: the world is actually shut down.
    OnShutdown,
}

/// A script that answers the world's own moments.
///
/// Reference: `class WorldScript : public ScriptObject` (`WorldScript.h:45`). Every method is a no-op
/// by default, as there; a script declares which ones it answers when it registers.
///
/// `Send` because the manager is shared: the reference calls its scripts from the connection threads
/// and its world loop alike, and the world this manager lives in is handed to the listener.
pub trait WorldScript: ScriptObject + Send {
    /// Called on every world tick.
    fn on_update(&mut self, _diff: Duration) {}

    /// Called when the world is started.
    fn on_startup(&mut self) {}

    /// Called when the world is actually shut down.
    fn on_shutdown(&mut self) {}
}

/// One registered script: what the reference keeps in `ScriptPointerList` plus its enabled hooks.
///
/// The box is dropped when the manager is, which is the reference's `ScriptMgr::Unload`
/// (`ScriptMgr.cpp:150`) for this shape: nothing is left registered after the manager goes.
struct Registered {
    hooks: Vec<WorldHook>,
    script: Box<dyn WorldScript>,
}

impl std::fmt::Debug for Registered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registered")
            .field("name", &self.script.name())
            .field("hooks", &self.hooks)
            .finish()
    }
}

/// The registry itself.
///
/// The reference is a singleton (`sScriptMgr`) because of the `game` <-> `scripts` cycle; here the
/// manager is a value the caller owns, which is the same choice this tree makes for the log and the
/// configuration, and no script has to reach for a global to be called.
#[derive(Debug, Default)]
pub struct ScriptMgr {
    loader: Option<ScriptLoader>,
    modules_loader: Option<ScriptLoader>,
    /// The registered scripts, in the order they were registered: the index is the id the reference
    /// gives (`ScriptPointerList[_scriptIdCounter++]`, `ScriptMgr.h:820`).
    scripts: Vec<Registered>,
    /// The names that are taken, so one name cannot be registered twice.
    ids: BTreeMap<String, u32>,
    /// Whether `initialize` has run: a program that loads scripts twice would answer every hook twice.
    initialized: bool,
}

impl ScriptMgr {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// The function that registers scripts, set before [`ScriptMgr::initialize`].
    ///
    /// Reference: `ScriptMgr::SetScriptLoader(AddScripts)` (`Main.cpp:272`).
    pub fn set_script_loader(&mut self, loader: ScriptLoader) {
        self.loader = Some(loader);
    }

    /// The function that registers scripts living out of tree.
    ///
    /// Reference: `ScriptMgr::SetModulesLoader(AddModulesScripts)` (`Main.cpp:273`). It runs after
    /// the in-tree loader, which is the order the reference calls them in (`ScriptMgr.cpp:140-141`).
    pub fn set_modules_loader(&mut self, loader: ScriptLoader) {
        self.modules_loader = Some(loader);
    }

    /// Register one world script, under the hooks it answers.
    ///
    /// Reference: `ScriptRegistry<TScript>::AddScript(script, enabledHooks)` (`ScriptMgr.h:796`), and
    /// `IncreaseScriptCount` at `:820`. A name registered twice is refused rather than counted twice:
    /// two scripts answering one hook is a bug in the loader, and the count is what the program
    /// reports. The id is the order of registration, as the reference's counter is.
    pub fn add_world_script(
        &mut self,
        script: Box<dyn WorldScript>,
        hooks: &[WorldHook],
    ) -> Result<u32, String> {
        let name = script.name().to_string();
        if self.ids.contains_key(&name) {
            return Err(format!("'{name}' is registered twice"));
        }
        let id = self.scripts.len() as u32;
        self.ids.insert(name, id);
        self.scripts.push(Registered {
            hooks: hooks.to_vec(),
            script,
        });
        Ok(id)
    }

    /// Run the loaders, once, and answer what they registered.
    ///
    /// Reference: `ScriptMgr::Initialize` (`ScriptMgr.cpp:124`) - the script loader, then the modules
    /// loader. A second call answers what the first one found instead of running them again.
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
            hooks: self.scripts.iter().map(|entry| entry.hooks.len()).sum(),
        }
    }

    /// Whether [`ScriptMgr::initialize`] has run.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// How many scripts are registered (`ScriptMgr::GetScriptCount`, `ScriptMgr.h:123`).
    pub fn script_count(&self) -> u32 {
        self.scripts.len() as u32
    }

    /// The world is started (`ScriptMgr::OnStartup`, `WorldScript.cpp:62`).
    pub fn on_startup(&mut self) {
        self.call(WorldHook::OnStartup, |script| script.on_startup());
    }

    /// Every world tick (`ScriptMgr::OnWorldUpdate`, `WorldScript.cpp:53`).
    pub fn on_world_update(&mut self, diff: Duration) {
        self.call(WorldHook::OnUpdate, |script| script.on_update(diff));
    }

    /// The world is shut down (`ScriptMgr::OnShutdown`, `WorldScript.cpp:67`).
    pub fn on_shutdown(&mut self) {
        self.call(WorldHook::OnShutdown, |script| script.on_shutdown());
    }

    /// Answer one hook, to the scripts that declared it and no others -
    /// `CALL_ENABLED_HOOKS(WorldScript, hook, ...)` (`ScriptMgrMacros.h:73-77`).
    fn call(&mut self, hook: WorldHook, mut answer: impl FnMut(&mut Box<dyn WorldScript>)) {
        for entry in &mut self.scripts {
            if entry.hooks.contains(&hook) {
                answer(&mut entry.script);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// What a script did, in the order it did it: the record a test reads.
    type Record = Arc<Mutex<Vec<String>>>;

    /// A script that writes down every hook it is called for.
    struct Recorder {
        name: String,
        record: Record,
    }

    impl ScriptObject for Recorder {
        fn name(&self) -> &str {
            &self.name
        }
    }

    impl WorldScript for Recorder {
        fn on_update(&mut self, diff: Duration) {
            self.record
                .lock()
                .unwrap()
                .push(format!("{}:update:{}", self.name, diff.as_millis()));
        }

        fn on_startup(&mut self) {
            self.record
                .lock()
                .unwrap()
                .push(format!("{}:startup", self.name));
        }

        fn on_shutdown(&mut self) {
            self.record
                .lock()
                .unwrap()
                .push(format!("{}:shutdown", self.name));
        }
    }

    fn recorder(name: &str, record: &Record) -> Box<dyn WorldScript> {
        Box::new(Recorder {
            name: name.to_string(),
            record: Arc::clone(record),
        })
    }

    /// A loader that registers nothing, which is what this tree has today.
    fn no_scripts(_mgr: &mut ScriptMgr) {}

    fn two_scripts(mgr: &mut ScriptMgr) {
        let record = Arc::new(Mutex::new(Vec::new()));
        mgr.add_world_script(
            recorder("mod-a", &record),
            &[WorldHook::OnStartup, WorldHook::OnShutdown],
        )
        .unwrap();
        mgr.add_world_script(recorder("mod-b", &record), &[WorldHook::OnShutdown])
            .unwrap();
    }

    #[test]
    fn a_loader_registers_what_it_has_and_the_summary_counts_it() {
        let mut mgr = ScriptMgr::new();
        mgr.set_script_loader(two_scripts);
        assert_eq!(
            mgr.initialize().unwrap(),
            Summary {
                scripts: 2,
                hooks: 3
            }
        );
        assert!(mgr.is_initialized());
        assert_eq!(mgr.script_count(), 2);
    }

    #[test]
    fn a_hook_is_answered_only_by_the_scripts_that_declared_it() {
        let mut mgr = ScriptMgr::new();
        let record: Record = Arc::new(Mutex::new(Vec::new()));
        mgr.add_world_script(
            recorder("mod-a", &record),
            &[WorldHook::OnStartup, WorldHook::OnShutdown],
        )
        .unwrap();
        mgr.add_world_script(recorder("mod-b", &record), &[WorldHook::OnShutdown])
            .unwrap();

        mgr.on_startup();
        mgr.on_shutdown();
        assert_eq!(
            *record.lock().unwrap(),
            vec!["mod-a:startup", "mod-a:shutdown", "mod-b:shutdown"]
        );
    }

    #[test]
    fn the_world_update_hook_carries_the_tick_it_measures() {
        let mut mgr = ScriptMgr::new();
        let record: Record = Arc::new(Mutex::new(Vec::new()));
        mgr.add_world_script(recorder("mod-a", &record), &[WorldHook::OnUpdate])
            .unwrap();
        mgr.on_world_update(Duration::from_millis(1000));
        assert_eq!(*record.lock().unwrap(), vec!["mod-a:update:1000"]);
    }

    #[test]
    fn a_registry_with_no_scripts_is_initialised_and_empty() {
        // The state of this tree today: the loader exists and registers nothing.
        let mut mgr = ScriptMgr::new();
        mgr.set_script_loader(no_scripts);
        assert_eq!(mgr.initialize().unwrap(), Summary::default());
        assert_eq!(mgr.script_count(), 0);
        // And a moment with nobody listening is not a fault: each hook is answered by nobody.
        mgr.on_startup();
        mgr.on_world_update(Duration::from_millis(1));
        mgr.on_shutdown();
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
        let record: Record = Arc::new(Mutex::new(Vec::new()));
        mgr.add_world_script(recorder("mod-a", &record), &[WorldHook::OnStartup])
            .unwrap();
        let error = mgr
            .add_world_script(recorder("mod-a", &record), &[WorldHook::OnStartup])
            .unwrap_err();
        assert!(error.contains("registered twice"), "{error}");
    }
    /// The order the reference calls the loaders in: the tree's scripts, then the modules'
    /// (`ScriptMgr::Initialize`, `ScriptMgr.cpp:140-141`). Loaders are function pointers, so the
    /// record they write to is a static.
    #[test]
    fn the_modules_loader_runs_after_the_script_loader() {
        use std::sync::Mutex as StdMutex;
        static ORDER: StdMutex<Vec<&'static str>> = StdMutex::new(Vec::new());
        fn in_tree(_mgr: &mut ScriptMgr) {
            ORDER.lock().unwrap().push("scripts");
        }
        fn out_of_tree(_mgr: &mut ScriptMgr) {
            ORDER.lock().unwrap().push("modules");
        }
        ORDER.lock().unwrap().clear();

        let mut mgr = ScriptMgr::new();
        mgr.set_script_loader(in_tree);
        mgr.set_modules_loader(out_of_tree);
        mgr.initialize().unwrap();
        assert_eq!(*ORDER.lock().unwrap(), vec!["scripts", "modules"]);
    }
}
