//! worldserver - the game program.
//!
//! Reference: `src/server/apps/worldserver/Main.cpp` at 30a36ec4d. The stages below are that
//! file's order, and each one names the reference call it translates.
//!
//! Only the first stage exists: the command line. The program stops at the second with a
//! reason and exit code 1 rather than printing that it ran a stage it has not ported.

// The stages that use these are not ported yet. The module exists so the boundary exists.
#[allow(dead_code)]
#[path = "CommandLine/mod.rs"]
mod command_line;
#[allow(dead_code)]
#[path = "RemoteAccess/mod.rs"]
mod remote_access;

use common::configuration::{Complaint, Config, Policy, PolicyOverride, Severity};
use common::logging::{Log, LogLevel};
use common::utilities::create_pid_file;
use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};
use database::updater::{collect_files, UpdateState};
use std::path::PathBuf;
use std::process::ExitCode;

/// The config file read when `-c` names none.
/// Reference: `_ACORE_CORE_CONFIG  "worldserver.json"`, with this project's own file format: JSON.
const DEFAULT_CONFIG: &str = "worldserver.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("worldserver: {reason}");
            ExitCode::from(1)
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    // Stage 1. Reference: `GetConsoleArguments(argc, argv, configFile)`.
    let options = Options::parse(args)?;

    // Reference: `if (vm.count("help") || vm.count("version")) return 0;`
    if options.help {
        print_usage();
        return Ok(());
    }
    if options.version {
        println!("Kryta worldserver {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Stage 2. Reference: `sConfigMgr->Configure(...)` then `LoadAppConfigs()`.
    //
    // The severity policy can be overridden on the command line, and a key it does not know is
    // refused rather than ignored - the reference reports it the same way.
    let mut policy = Policy::default();
    if let Some(text) = &options.config_policy {
        let control = PolicyOverride::parse(text).map_err(|error| error.to_string())?;
        if !control.apply(&mut policy) {
            return Err(format!(
                "--config-policy names no value this program has: '{}'",
                control.key
            ));
        }
    }

    let mut config =
        Config::load_json(&options.config, false, policy).map_err(|error| error.to_string())?;
    config.set_dry_run(options.dry_run);

    for complaint in config.complaints() {
        report(complaint);
    }

    // Stage 3. Reference: `sLog->Initialize(sConfigMgr->GetOption<bool>("Log.Async.Enable", ...))`.
    let log = Log::new();
    let notes = log.initialize(&config).map_err(|error| error.to_string())?;
    for note in notes {
        println!("warning: {note}");
    }

    // Reference: `Acore::Banner::Show("worldserver-daemon", ...)`, whose first line names the
    // configuration file.
    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!(
            "> Using configuration file       {}",
            config.filename().display()
        ),
    );
    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!("Kryta worldserver {}", env!("CARGO_PKG_VERSION")),
    );

    // What this process actually took from its configuration, when asked. Masked: a connection
    // string holds a password and a log line gets pasted into issue reports.
    let (show_config, notes) = config
        .value_bool("Log.EffectiveConfig", false)
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    if show_config {
        for (key, value) in config.describe() {
            log.out(
                LogLevel::Debug,
                "server.loading",
                format!("> Config: {key} = {value}"),
            );
        }
    }
    // Stage 4. Reference: `CreatePIDFile(pidFile)`, when `PidFile` is set - the same stage the auth
    // program has, with the same two lines.
    let (pid_file, notes) = config
        .value_string("PidFile", "")
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    if !pid_file.is_empty() {
        match create_pid_file(&pid_file) {
            Some(pid) => log.out(
                LogLevel::Info,
                "server.worldserver",
                format!("Daemon PID: {pid}\n"),
            ),
            None => {
                log.out(
                    LogLevel::Error,
                    "server.worldserver",
                    format!("Cannot create PID file {pid_file} (possible error: permission)\n"),
                );
                log.shutdown();
                return Err(format!(
                    "cannot create PID file {pid_file} (possible error: permission)"
                ));
            }
        }
    }

    // Stage 7. Reference: `sScriptMgr->SetScriptLoader(AddScripts)`, `SetModulesLoader`, then
    // `sScriptMgr->Initialize()` with `Initializing Scripts...` printed before it. The reference
    // starts its io-context threads (its `ThreadPool`) before this; that pool is this tree's socket
    // layer, which owns its runtimes and is created with the listener, so the thread stage lands
    // there and not as a stage of its own (`docs/port/01-structure.md`).
    let mut scripts = scripts::ScriptMgr::new();
    scripts.set_script_loader(scripts::add_scripts);
    scripts.set_modules_loader(scripts::add_modules_scripts);
    log.out(LogLevel::Info, "server.loading", "Initializing Scripts...");
    let registered = scripts.initialize().map_err(|error| error.to_string())?;
    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!(
            "Scripts: {} script(s), {} hook(s) registered.",
            registered.scripts, registered.hooks
        ),
    );

    // Stage 5. Reference: `StartDB()`, which opens the World and Character pools and runs the schema
    // updater on each - the stage the auth program has, twice.
    //
    // The world program opens two databases rather than one. Neither schema is the reference's: the
    // login database's `realmlist` transfers because a world list is a world list, but the world's
    // content and a character's sheets are Tyria's, so both schemas start empty and are filled by
    // the stages that own them.
    let (sql_trace, notes) = config
        .value_bool("Database.Trace", false)
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }

    let mut opened = Vec::new();
    let mut character_pool: Option<std::sync::Arc<std::sync::Mutex<DatabaseWorkerPool>>> = None;
    for (kind, label, schema) in [
        (DatabaseKind::World, "World", "data/sql/base/db_world"),
        (DatabaseKind::Character, "Character", "data/sql/base/db_characters"),
    ] {
        let settings = read_settings(&config, kind)?;
        let mut pool = DatabaseWorkerPool::open_with_trace(kind, &settings, &log, sql_trace)?;

        let schema = std::path::Path::new(schema);
        let files = collect_files(schema, UpdateState::Released).map_err(|error| {
            format!(
                "the {label} schema at {} is not readable: {error}",
                schema.display()
            )
        })?;
        let summary = pool.apply_updates(&files, &log)?;
        pool.ping()
            .map_err(|error| format!("the {label} database does not answer: {error}"))?;

        log.out(
            LogLevel::Info,
            "server.worldserver",
            format!(
                "Started {label} database connection pool: {} file(s) applied, {} already current.",
                summary.updated, summary.skipped
            ),
        );

        if kind == DatabaseKind::Character {
            // A creation confirm writes a character from a session thread, so the character pool is
            // shared with the listener the way the login program shares its own with the portal.
            character_pool = Some(std::sync::Arc::new(std::sync::Mutex::new(pool)));
        } else {
            opened.push(pool);
        }
    }

    // Stage 8. Reference: `sWorld->SetInitialWorldSettings()` - everything the game is made of,
    // loaded before the listener opens. Here that is the map catalogue, and the log says how many
    // maps it holds; zero would be a statement, and the block below only serves what is listed.
    let (realm, notes) = config
        .value::<u32>("RealmID", 1)
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }

    let mut world = game::world::World::new(realm);
    let (maps_file, notes) = config
        .value_string("MapsFile", "data/maps.json")
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    let maps = world.load_maps(&maps_file, &log)?;
    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!(
            "World of {} map(s) loaded from {maps_file}: {}.",
            maps,
            world
                .maps()
                .iter()
                .map(|map| format!("{} (type {})", map.id, map.kind))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );
    let world = std::sync::Arc::new(world);

    // Stage 9. Reference: `sWorldSocketMgr.StartWorldNetwork(*ioContext, worldListener, worldPort,
    // networkThreads)` - the listener, on the address and port its own configuration names, with the
    // number of network threads `Network.Threads` asks for.
    let (bind, notes) = config
        .value_string("BindIP", "0.0.0.0")
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    let (port, notes) = config
        .value::<u16>("WorldServerPort", 8085)
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    let (network_threads, notes) = config
        .value::<usize>("Network.Threads", 1)
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    let (trace, notes) = config
        .value_bool("Network.Trace", false)
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }

    let sockets =
        game::server::WorldSocketMgr::start(
            std::sync::Arc::clone(&world),
            character_pool.clone(),
            log.clone(),
            &bind,
            port,
            network_threads,
            trace,
        )
            .map_err(|error| format!("the world listener could not start: {error}"))?;
    let bound = sockets.port().unwrap_or(port);

    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!(
            "READY: listening on {bind}:{bound} as world {realm}; a client's gameplay connection is copied here by the login program."
        ),
    );

    // Stages after this one, in the reference's order, and none exists yet:
    //   CliRunnable                   the console, when `Console.Enable`
    //   ACSoapThread                  the SOAP endpoint, when `SOAP.Enabled`, port 7878
    //   sScriptMgr->OnStartup
    // On shutdown: `sWorldSessionMgr->KickAll`, `sMapMgr->UnloadAll`,
    // `sScriptMgr->OnAfterUnloadAllMaps`, `sScriptMgr->OnShutdown`.
    wait_for_shutdown();
    log.out(
        LogLevel::Info,
        "server.worldserver",
        "Stopping: the listener is closing.",
    );
    let accepted = sockets.accepted_count();
    sockets.stop();
    for pool in &mut opened {
        pool.close();
    }
    if let Some(pool) = character_pool {
        match pool.lock() {
            Ok(mut pool) => pool.close(),
            Err(_) => log.out(
                LogLevel::Error,
                "server.worldserver",
                "the character database pool was poisoned; it closes when the process ends",
            ),
        }
    }
    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!("Stopped. {accepted} connection(s) were accepted while it ran."),
    );
    log.shutdown();
    Ok(())
}

/// Block until the operator stops the program.
///
/// Reference: `ioContext->run()` returning when the signal handler stopped it. A Ctrl-C is that
/// signal here, the same as the auth program's.
fn wait_for_shutdown() {
    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    if ctrlc::set_handler(move || {
        let _ = sender.send(());
    })
    .is_err()
    {
        return;
    }
    let _ = receiver.recv();
}

/// Print a complaint at the severity the policy gave it.
///
/// Reference: `LogWithSeverity`, whose first act is to say nothing at `Skip`.
fn report(complaint: &Complaint) {
    match complaint.severity {
        Severity::Skip => {}
        Severity::Warn => println!("warning: {}", complaint.message),
        Severity::Error => eprintln!("error: {}", complaint.message),
        Severity::Fatal => eprintln!("fatal: {}", complaint.message),
    }
}

/// What the command line said.
///
/// Reference: the `options_description` in `GetConsoleArguments`. It is the same list the
/// auth program has, because the reference gives both programs the same options.
#[derive(Debug, Default)]
struct Options {
    /// `-h`, `--help`. Print usage and exit 0.
    help: bool,
    /// `-v`, `--version`. Print the build and exit 0.
    version: bool,
    /// `-d`, `--dry-run`. Apply the database updates and stop.
    dry_run: bool,
    /// `-c`, `--config`. The file to read.
    config: PathBuf,
    /// `--config-policy`. Overrides the severity policy for unknown config keys.
    config_policy: Option<String>,
}

impl Options {
    /// Read the arguments.
    ///
    /// Reference: `command_line_parser(...).allow_unregistered()`. An argument this program
    /// does not know is kept rather than refused, so it does not return an error for one.
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self {
            config: PathBuf::from(DEFAULT_CONFIG),
            ..Self::default()
        };

        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "-h" | "--help" => options.help = true,
                "-v" | "--version" => options.version = true,
                "-d" | "--dry-run" => options.dry_run = true,
                "-c" | "--config" => {
                    options.config = PathBuf::from(
                        rest.next().ok_or("--config needs a path")?,
                    );
                }
                "--config-policy" => {
                    options.config_policy =
                        Some(rest.next().ok_or("--config-policy needs a value")?.clone());
                }
                other => {
                    if let Some(value) = other.strip_prefix("--config=") {
                        options.config = PathBuf::from(value);
                    } else if let Some(value) = other.strip_prefix("--config-policy=") {
                        options.config_policy = Some(value.to_string());
                    }
                }
            }
        }

        Ok(options)
    }
}

fn print_usage() {
    println!("Allowed options:");
    println!("  -h [ --help ]             print usage message");
    println!("  -v [ --version ]          print version build info");
    println!("  -d [ --dry-run ]          Dry run");
    println!("  -c [ --config ] arg       use <arg> as configuration file (default: {DEFAULT_CONFIG})");
    println!("  --config-policy policy    override config severity policy");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Options {
        Options::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn the_config_file_defaults_to_the_reference_name() {
        assert_eq!(parse(&[]).config, PathBuf::from("worldserver.json"));
    }

    #[test]
    fn a_config_path_is_read_in_both_forms() {
        assert_eq!(parse(&["-c", "a.conf"]).config, PathBuf::from("a.conf"));
        assert_eq!(parse(&["--config=b.conf"]).config, PathBuf::from("b.conf"));
    }

    #[test]
    fn help_and_version_are_separate_flags() {
        let options = parse(&["-v"]);
        assert!(options.version && !options.help);
    }

    #[test]
    fn dry_run_is_its_own_flag() {
        assert!(parse(&["--dry-run"]).dry_run);
    }

    #[test]
    fn an_unknown_argument_is_kept_not_refused() {
        assert!(Options::parse(&["--something-new".to_string()]).is_ok());
    }
}
