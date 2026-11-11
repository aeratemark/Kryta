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
    // The world program opens three databases: the world's content, a character's sheets, and the
    // login database's `travel` record - the hand-off a login writes and this program claims, which
    // is why the two programs need not talk to each other (`docs/port/06-world-plan.md`, "The three
    // seams"). The login schema is applied here too, so a world started first still finds its table.
    let (sql_trace, notes) = config
        .value_bool("Database.Trace", false)
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }

    config.mark_critical("LoginDatabaseInfo");
    let mut opened = Vec::new();
    let mut character_pool: Option<std::sync::Arc<std::sync::Mutex<DatabaseWorkerPool>>> = None;
    let mut world_pool: Option<std::sync::Arc<std::sync::Mutex<DatabaseWorkerPool>>> = None;
    let mut login_pool: Option<std::sync::Arc<std::sync::Mutex<DatabaseWorkerPool>>> = None;
    for (kind, label, schema) in [
        (DatabaseKind::Login, "Login", "data/sql/base/db_auth"),
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
        } else if kind == DatabaseKind::World {
            // The maps are rows, so the world pool is kept: the map catalogue is read from it below.
            world_pool = Some(std::sync::Arc::new(std::sync::Mutex::new(pool)));
        } else if kind == DatabaseKind::Login {
            // A connection claims its travel record from a session thread, so this pool is shared
            // with the listener as well.
            login_pool = Some(std::sync::Arc::new(std::sync::Mutex::new(pool)));
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
    // The maps are rows in the world database (`docs/decisions/0009-world-schema.md`), read the way the
    // reference reads its map records at `SetInitialWorldSettings`: one query, held for the life of the
    // program. A world started against an empty database hosts no map, and says so.
    let world_maps = match world_pool.as_ref() {
        Some(pool) => {
            let maps = game::maps::DatabaseMaps::new(std::sync::Arc::clone(pool), log.clone());
            Some(maps)
        }
        None => None,
    };
    let maps = match world_maps.as_ref() {
        Some(maps) => world.load_maps(maps, &log)?,
        None => 0,
    };
    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!(
            "World of {} map(s): {}.",
            maps,
            world
                .maps()
                .iter()
                .map(|map| format!("{} '{}' (type {})", map.id, map.name, map.kind))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );
    // The creation screen's own content: the bags and items a client is given, and what a new
    // character wears per campaign and profession. Reference: the same `SetInitialWorldSettings`
    // pass that loads a world's maps; a creation screen's equipment is content like any other.
    let (inventory_file, notes) = config
        .value_string("InventoryFile", "data/inventory.json")
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    let (equipment_file, notes) = config
        .value_string("EquipmentFile", "data/equipment.json")
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    // The world's own numbers: every threshold an instance, a district or a spawn uses, read from this
    // program's file with the game's value as the default (R53, and docs/game-rules.md for each row).
    let (world_settings, notes) = game::world::world_config::WorldConfig::read(&config)?;
    for note in notes {
        log.out(LogLevel::Info, "server.worldserver", note);
    }
    world.set_settings(world_settings);
    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!(
            "World settings: a district holds {} player(s), a party is capped at {}.",
            world.settings().district_allotment,
            world.settings().party_size_cap
        ),
    );

    let (bags, items, sets) = world.load_creation(&inventory_file, &equipment_file, &log)?;
    log.out(
        LogLevel::Info,
        "server.worldserver",
        format!(
            "Creation screen: {bags} bag(s) and {items} item(s) from {inventory_file}, {sets} profession set(s) from {equipment_file}."
        ),
    );

    // The instance table (`data/sql/base/db_world/0003-instances.sql`): the copies a session opens
    // are rows, and a fresh world disposes what an earlier run left - nobody is inside it.
    if let Some(pool) = world_pool.as_ref() {
        let store = std::sync::Arc::new(game::districts::DatabaseInstances::new(
            std::sync::Arc::clone(pool),
            log.clone(),
        ));
        match store.dispose_all() {
            Ok(count) if count > 0 => log.out(
                LogLevel::Info,
                "server.worldserver",
                format!(
                    "Disposed {count} instance row(s) left by an earlier run; a fresh world has nobody inside it."
                ),
            ),
            Ok(_) => {}
            Err(error) => log.out(
                LogLevel::Error,
                "server.worldserver",
                format!("Cannot read the instance table: {error}"),
            ),
        }
        world.set_instances(store, log.clone());
    } else {
        log.out(
            LogLevel::Warn,
            "server.worldserver",
            "No world database is open, so instances are kept in memory only and no copy is recorded.",
        );
    }

    // The item templates: one row per distinct item the creation record lists name, seeded once.
    // Reference: the same `SetInitialWorldSettings` pass that reads the world's item templates.
    if let Some(pool) = world_pool.as_ref() {
        let items = game::data_stores::DatabaseItems::new(std::sync::Arc::clone(pool), log.clone());
        let definitions = world.creation().definitions();
        match items.seed(&definitions) {
            Ok(written) => match items.count() {
                Ok(total) => log.out(
                    LogLevel::Info,
                    "server.worldserver",
                    format!(
                        "Item templates: {total} row(s) from {} definition(s); {written} written now.",
                        definitions.len()
                    ),
                ),
                Err(error) => log.out(
                    LogLevel::Warn,
                    "server.worldserver",
                    format!("The item templates were seeded but cannot be counted: {error}"),
                ),
            },
            Err(error) => log.out(
                LogLevel::Error,
                "server.worldserver",
                format!("The item templates could not be seeded: {error}"),
            ),
        }
        // And back into memory: a load resolves a character's held items against these rows.
        match items.list() {
            Ok(templates) => {
                log.out(
                    LogLevel::Info,
                    "server.worldserver",
                    format!("Item templates read back: {} row(s) held in memory.", templates.len()),
                );
                world.set_templates(templates);
            }
            Err(error) => log.out(
                LogLevel::Error,
                "server.worldserver",
                format!("The item templates cannot be read back: {error}"),
            ),
        }
    }
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
    let (status_interval, notes) = config
        .value::<u64>("StatusInterval", 30)
        .map_err(|error| error.to_string())?;
    for note in notes {
        report(&note);
    }
    let status_interval = status_interval.max(1);

    let sockets =
        game::server::WorldSocketMgr::start(
            std::sync::Arc::clone(&world),
            character_pool.clone(),
            login_pool.clone(),
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
            "READY: listening on {bind}:{bound} as world {realm}; a client's gameplay connection is copied here by the login program; status reported every {status_interval} s."
        ),
    );

    // Stages after this one, in the reference's order, and none exists yet:
    //   CliRunnable                   the console, when `Console.Enable`
    //   ACSoapThread                  the SOAP endpoint, when `SOAP.Enabled`, port 7878
    //   sScriptMgr->OnStartup
    // On shutdown: `sWorldSessionMgr->KickAll`, `sMapMgr->UnloadAll`,
    // `sScriptMgr->OnAfterUnloadAllMaps`, `sScriptMgr->OnShutdown`.

    // The status line: what this world is up to, on a timer, so "is it alive and what has reached it"
    // is answerable from the log alone - the auth program's own arrangement, with the world's state:
    // the maps it hosts and the instances it currently holds open.
    let stop_status = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let accepted = sockets.accepted_handle();
    let status_writer = {
        let log = log.clone();
        let status_world = std::sync::Arc::clone(&world);
        let stop_status = std::sync::Arc::clone(&stop_status);
        let started = std::time::Instant::now();
        std::thread::Builder::new()
            .name("status".to_string())
            .spawn(move || {
                while !stop_status.load(std::sync::atomic::Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_secs(status_interval));
                    if stop_status.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    let instances = match status_world.districts().lock() {
                        Ok(districts) => districts.open(),
                        Err(_) => Vec::new(),
                    };
                    let open = if instances.is_empty() {
                        "none".to_string()
                    } else {
                        instances
                            .iter()
                            .map(|(map_id, district, members)| {
                                format!("map {map_id} district {district} ({members} inside)")
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    log.out(
                        LogLevel::Info,
                        "server.worldserver",
                        format!(
                            "status: up {} s, listening on {bind}:{bound} as world {realm}, {} connection(s) accepted, {} map(s) hosted, {} instance(s) open: {open}.",
                            started.elapsed().as_secs(),
                            accepted.load(std::sync::atomic::Ordering::Relaxed),
                            status_world.maps().len(),
                            instances.len()
                        ),
                    );
                }
            })
            .map_err(|error| format!("cannot start the status writer: {error}"))?
    };

    // The instance tick (R33): every `World.SweepIntervalMs` milliseconds (1000 by default), copies
    // that have been empty past the hold (R37) are disposed (R40) and their rows removed.
    // `InstanceTick` turns the clock off; a world without a clock keeps every copy it opened and says
    // so in its configuration dump.
    let stop_tick = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if world.settings().instance_tick {
        let sweep = std::time::Duration::from_millis(u64::from(world.settings().sweep_interval_ms));
        let tick_world = std::sync::Arc::clone(&world);
        let tick_log = log.clone();
        let stop_tick = std::sync::Arc::clone(&stop_tick);
        std::thread::Builder::new()
            .name("instance-tick".to_string())
            .spawn(move || {
                while !stop_tick.load(std::sync::atomic::Ordering::Relaxed) {
                    std::thread::sleep(sweep);
                    if stop_tick.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    let hold = u64::from(tick_world.settings().explorable_disconnect_hold);
                    let disposed = match tick_world.districts().lock() {
                        Ok(mut districts) => districts.sweep(unix_now(), hold),
                        Err(_) => Vec::new(),
                    };
                    for (map_id, district) in disposed {
                        tick_log.out(
                            LogLevel::Info,
                            "server.worldserver",
                            format!(
                                "Instance on map {map_id}, district {district}, was empty for {hold} s and is disposed (R37, R40)."
                            ),
                        );
                    }
                }
            })
            .map_err(|error| format!("the instance tick could not start: {error}"))?;
    }

    wait_for_shutdown();
    stop_tick.store(true, std::sync::atomic::Ordering::Relaxed);
    stop_status.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = status_writer.join();
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
    if let Some(pool) = login_pool {
        match pool.lock() {
            Ok(mut pool) => pool.close(),
            Err(_) => log.out(
                LogLevel::Error,
                "server.worldserver",
                "the login database pool was poisoned; it closes when the process ends",
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

/// Seconds since the unix epoch, for the hold the instance tick measures.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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
