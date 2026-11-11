//! `kryta-manage`: the administration tool - every piece of game data an operator changes, on one page.
//!
//! It is not a server. No client talks to it, it serves no game protocol, and neither server program
//! knows it exists. What it does is read and write the state the two programs already keep: rows in the
//! login and character databases, and the JSON content files. The data access is the nouns' own code
//! (`game::accounts`, `game::characters`), so what this page changes is exactly what the server reads.
//!
//! Why a page rather than SQL by hand: the state is spread over two databases and four content files,
//! and a wrong `UPDATE` against a live account is not something a person should be doing from memory.
//!
//! Settings are in `manage.json` (`docs/port/05-tooling.md`); the tool refuses to start without a
//! database to point at.

mod pages;

use std::sync::{Arc, Mutex};

use common::configuration::{Config, Policy};
use common::logging::{Log, LogLevel};
use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};
use shared::network::http::{HttpError, Request, Response, read_request, write_response};
use shared::network::socket::{Session, SessionFuture, SessionIo};
use tokio::net::TcpListener;

/// One database the schema page reads: its label, the pool, and where its files live.
pub struct DatabaseSchema {
    /// What the page calls it.
    pub label: &'static str,
    /// The database itself.
    pub pool: Arc<Mutex<DatabaseWorkerPool>>,
    /// The directory its schema files live in.
    pub dir: &'static str,
}

/// One schema file, as the database's own record and the disk agree or not.
pub struct SchemaFile {
    /// The file's name, the one the updater records.
    pub name: String,
    /// `applied`, `changed`, `new`, or `recorded, not on disk`.
    pub state: &'static str,
}

/// What the tool serves from: the account table, and the token a request must carry when one is set.
pub struct Manage {
    accounts: Arc<game::accounts::DatabaseAccounts>,
    /// The character database, when the tool opened one: the characters page.
    characters: Option<Arc<game::characters::DatabaseCharacters>>,
    /// The world database, when the tool opened one: the content page.
    maps: Option<Arc<game::maps::DatabaseMaps>>,
    /// The item templates, when the world database is open: what the content page counts.
    items: Option<Arc<game::data_stores::DatabaseItems>>,
    /// The instance table, when the world database is open: what the content page shows running.
    instances: Option<Arc<game::districts::DatabaseInstances>>,
    /// The world list, read from `realmlist` and merged with `data/realms.json`.
    worlds: Option<Arc<shared::worlds::WorldList>>,
    /// The databases whose updater records the schema page shows.
    schemas: Vec<DatabaseSchema>,
    /// The two server programs' own settings files, when the tool is told where they are: the
    /// settings page shows them. A file is shown and validated, never rewritten (`manage.json`).
    world_config: Option<String>,
    auth_config: Option<String>,
    token: Option<String>,
    log: Log,
}

impl Manage {
    /// One page, answered.
    fn answer(&self, request: &Request) -> Response {
        if !self.authorised(request) {
            return Response::html(401, pages::unauthorised());
        }

        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/") => pages::dashboard(self),
            ("GET", "/accounts") => pages::accounts(self),
            ("GET", "/characters") => pages::characters(self),
            ("GET", "/worlds") => pages::worlds(self),
            ("GET", "/content") => pages::content(self),
            ("POST", "/content/spawn") => self.add_spawn(request),
            ("POST", "/content/spawn/remove") => self.remove_spawn(request),
            ("POST", "/content/party-cap") => self.set_party_cap(request),
            ("GET", "/settings") => pages::settings(self),
            ("GET", "/schema") => pages::schema(self),
            ("POST", "/characters/delete") => self.delete_character(request),
            ("POST", "/characters/level") => self.set_level(request),
            ("POST", "/accounts/eula") => self.set_eula(request),
            ("POST", "/accounts/create") => self.create(request),
            ("POST", "/accounts/delete") => self.delete(request),
            ("GET", "/health") => Response::text(200, "ok\n"),
            (_, path) => Response::html(404, pages::missing(path)),
        }
    }

    /// Whether a request may be answered.
    ///
    /// With no token configured, every request from this machine is allowed and nothing else: the page
    /// binds loopback, and a tool an operator starts is meant to be usable. With a token, the request
    /// carries it as `?token=` or as the `auth` cookie.
    fn authorised(&self, request: &Request) -> bool {
        let Some(expected) = self.token.as_deref() else {
            return true;
        };
        if expected.is_empty() {
            return true;
        }
        if request.query("token") == Some(expected) {
            return true;
        }
        request
            .header("cookie")
            .map(|cookies| {
                cookies
                    .split(';')
                    .any(|cookie| cookie.trim() == format!("auth={expected}"))
            })
            .unwrap_or(false)
    }

    fn set_eula(&self, request: &Request) -> Response {
        let form = form_fields(&request.body_text());
        let Some(username) = form.get("username") else {
            return Response::html(400, pages::message("A request with no account name."));
        };
        let version: i64 = match form.get("version").map(|value| value.parse()) {
            Some(Ok(version)) => version,
            _ => 0,
        };
        match self.accounts.set_eula(username, version) {
            Ok(true) => {
                self.say(format!("{username}: the EULA version is now {version}"));
                pages::redirect("/accounts")
            }
            Ok(false) => Response::html(
                404,
                pages::message(&format!("No account is called {username}.")),
            ),
            Err(error) => Response::html(500, pages::message(&format!("The account table said: {error}"))),
        }
    }

    fn create(&self, request: &Request) -> Response {
        let form = form_fields(&request.body_text());
        let Some(username) = form.get("username") else {
            return Response::html(400, pages::message("A request with no account name."));
        };
        let user_id = form
            .get("user_id")
            .cloned()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("00000000-0000-0000-0000-{:012}", random_u64() % 1_000_000_000_000));
        let token = form
            .get("token")
            .cloned()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("00000000-0000-0000-0000-{:012}", random_u64() % 1_000_000_000_000));
        match self.accounts.create(username, &user_id, &token) {
            Ok(true) => {
                self.say(format!("{username}: created"));
                pages::redirect("/accounts")
            }
            Ok(false) => Response::html(
                409,
                pages::message(&format!("An account is already called {username}.")),
            ),
            Err(error) => Response::html(500, pages::message(&format!("The account table said: {error}"))),
        }
    }

    fn delete(&self, request: &Request) -> Response {
        let form = form_fields(&request.body_text());
        let Some(username) = form.get("username") else {
            return Response::html(400, pages::message("A request with no account name."));
        };
        match self.accounts.delete(username) {
            Ok(true) => {
                self.say(format!("{username}: removed (its characters are untouched)"));
                pages::redirect("/accounts")
            }
            Ok(false) => Response::html(
                404,
                pages::message(&format!("No account is called {username}.")),
            ),
            Err(error) => Response::html(500, pages::message(&format!("The account table said: {error}"))),
        }
    }

    /// `POST /characters/delete`: remove a character and the records that are its own - its items,
    /// its unlocks, its row. A character is *written* by the creation confirm and not here; removing
    /// one is the operator's act, and the page's button is the reason this route exists.
    fn delete_character(&self, request: &Request) -> Response {
        let Some(characters) = self.characters.as_ref() else {
            return Response::html(
                503,
                pages::message("No character database is configured in manage.json."),
            );
        };
        let form = form_fields(&request.body_text());
        let Some(uuid) = form.get("uuid").cloned() else {
            return Response::html(400, pages::message("A request with no character id."));
        };
        match characters.remove(&uuid) {
            Ok(true) => {
                self.say(format!("character {uuid}: removed, with its items and unlocks"));
                pages::redirect("/characters")
            }
            Ok(false) => Response::html(
                404,
                pages::message(&format!("No character has the id {uuid}.")),
            ),
            Err(error) => Response::html(
                500,
                pages::message(&format!("The character database said: {error}")),
            ),
        }
    }

    /// `POST /characters/level`: set a character's level, 1 to 20 (R46, R48). The range is the
    /// game's rule, so it is checked here rather than in the row.
    fn set_level(&self, request: &Request) -> Response {
        let Some(characters) = self.characters.as_ref() else {
            return Response::html(
                503,
                pages::message("No character database is configured in manage.json."),
            );
        };
        let form = form_fields(&request.body_text());
        let Some(uuid) = form.get("uuid").cloned() else {
            return Response::html(400, pages::message("A request with no character id."));
        };
        let Some(level) = form.get("level").and_then(|value| value.parse::<u8>().ok()) else {
            return Response::html(400, pages::message("A level is a number."));
        };
        if !(1..=20).contains(&level) {
            return Response::html(
                400,
                pages::message("A level is between 1 and 20, the game's own range (R48)."),
            );
        }
        match characters.set_level(&uuid, level) {
            Ok(true) => {
                self.say(format!("character {uuid}: level {level}"));
                pages::redirect("/characters")
            }
            Ok(false) => Response::html(
                404,
                pages::message(&format!("No character has the id {uuid}.")),
            ),
            Err(error) => Response::html(
                500,
                pages::message(&format!("The character database said: {error}")),
            ),
        }
    }

    /// The accounts, for a page that lists them.
    pub fn list_accounts(&self) -> Result<Vec<game::accounts::Account>, String> {
        self.accounts.list()
    }

    /// The characters, when a character database is configured.
    pub fn list_characters(&self) -> Option<Vec<game::characters::Character>> {
        self.characters.as_ref().map(|characters| characters.list())
    }

    /// What one character holds, read through the repository the load uses.
    pub fn list_items_of(&self, uuid: &str) -> Option<Vec<game::characters::HeldItem>> {
        self.characters
            .as_ref()
            .and_then(|characters| characters.items_of(uuid).ok())
    }

    /// The maps one character may travel to, read through the repository the load's gate uses (R12).
    pub fn list_unlocks(&self, uuid: &str) -> Option<Vec<u32>> {
        self.characters
            .as_ref()
            .and_then(|characters| characters.unlocked_maps(uuid).ok())
    }

    /// The world list, when the login database answered.
    pub fn list_worlds(&self) -> Option<Vec<shared::worlds::Realm>> {
        self.worlds
            .as_ref()
            .map(|worlds| worlds.worlds().values().cloned().collect())
    }

    /// The maps and their spawn points, when a world database is configured.
    pub fn list_maps(&self) -> Option<(Vec<game::world::MapEntry>, Vec<game::maps::MapSpawn>)> {
        let maps = self.maps.as_ref()?;
        maps.load().ok()
    }

    /// The arrival points, when a world database is configured: where a portal crossing lands (R21).
    pub fn list_arrivals(&self) -> Option<Vec<game::maps::Arrival>> {
        self.maps.as_ref()?.arrivals().ok()
    }

    /// The copies the instance table records, when a world database is configured.
    pub fn list_instances(&self) -> Option<Vec<game::districts::InstanceRow>> {
        self.instances.as_ref()?.rows().ok()
    }

    /// The party cap a map states, or `None` when the world's own setting applies (R15).
    pub fn party_cap(&self, map_id: u32) -> Option<u32> {
        self.maps.as_ref()?.party_cap(map_id).ok().flatten()
    }

    /// The item templates, when a world database is configured.
    pub fn list_items(&self) -> Option<Vec<game::data_stores::ItemDefinition>> {
        let items = self.items.as_ref()?;
        items.list().ok()
    }

    /// Where the two programs' settings files are, when the tool was told: the settings page reads
    /// them from these paths and never writes them.
    pub fn config_paths(&self) -> (Option<&str>, Option<&str>) {
        (
            self.world_config.as_deref(),
            self.auth_config.as_deref(),
        )
    }

    /// Add a spawn point to a map: what the load's entry placement reads (R13).
    pub fn add_spawn(&self, request: &Request) -> Response {
        let Some(maps) = self.maps.as_ref() else {
            return Response::html(
                503,
                pages::message("No world database is configured in manage.json."),
            );
        };
        let form = form_fields(&request.body_text());
        let Some(map_id) = form.get("map_id").and_then(|value| value.parse::<u32>().ok()) else {
            return Response::html(400, pages::message("The form names no map."));
        };
        let x = form.get("x").and_then(|value| value.parse::<f32>().ok());
        let y = form.get("y").and_then(|value| value.parse::<f32>().ok());
        let plane = form
            .get("plane")
            .and_then(|value| value.parse::<i16>().ok())
            .unwrap_or(0);
        let (Some(x), Some(y)) = (x, y) else {
            return Response::html(
                400,
                pages::message("A spawn point needs an x and a y as numbers."),
            );
        };
        match maps.add_spawn(map_id, x, y, plane) {
            Ok(index) => {
                self.say(format!(
                    "map {map_id}: spawn point {index} added at {x}, {y} (plane {plane})"
                ));
                pages::redirect("/content")
            }
            Err(error) => Response::html(
                500,
                pages::message(&format!("The world database said: {error}")),
            ),
        }
    }

    /// Remove a spawn point from a map.
    pub fn remove_spawn(&self, request: &Request) -> Response {
        let Some(maps) = self.maps.as_ref() else {
            return Response::html(
                503,
                pages::message("No world database is configured in manage.json."),
            );
        };
        let form = form_fields(&request.body_text());
        let Some(map_id) = form.get("map_id").and_then(|value| value.parse::<u32>().ok()) else {
            return Response::html(400, pages::message("The form names no map."));
        };
        let Some(index) = form.get("index").and_then(|value| value.parse::<i16>().ok()) else {
            return Response::html(400, pages::message("The form names no spawn point."));
        };
        match maps.remove_spawn(map_id, index) {
            Ok(true) => {
                self.say(format!("map {map_id}: spawn point {index} removed"));
                pages::redirect("/content")
            }
            Ok(false) => Response::html(
                404,
                pages::message(&format!("Map {map_id} has no spawn point {index}.")),
            ),
            Err(error) => Response::html(
                500,
                pages::message(&format!("The world database said: {error}")),
            ),
        }
    }

    /// `POST /content/party-cap`: a map's own party cap, as a row (R15). `0` clears it, so the
    /// world's `PartySizeCap` applies again.
    pub fn set_party_cap(&self, request: &Request) -> Response {
        let Some(maps) = self.maps.as_ref() else {
            return Response::html(
                503,
                pages::message("No world database is configured in manage.json."),
            );
        };
        let form = form_fields(&request.body_text());
        let Some(map_id) = form.get("map_id").and_then(|value| value.parse::<u32>().ok()) else {
            return Response::html(400, pages::message("The form names no map."));
        };
        let Some(cap) = form.get("cap").and_then(|value| value.parse::<u32>().ok()) else {
            return Response::html(400, pages::message("A party cap is a number; 0 clears it."));
        };
        match maps.set_party_cap(map_id, cap) {
            Ok(true) => {
                self.say(format!(
                    "map {map_id}: party cap {} (R15)",
                    if cap == 0 {
                        "cleared, the world's own applies".to_string()
                    } else {
                        cap.to_string()
                    }
                ));
                pages::redirect("/content")
            }
            Ok(false) => Response::html(
                404,
                pages::message(&format!("No map is called {map_id}.")),
            ),
            Err(error) => Response::html(
                500,
                pages::message(&format!("The world database said: {error}")),
            ),
        }
    }

    /// Each database's schema files, with what its own `updates` record says about them.
    ///
    /// The record is the database's, read by the updater's own reader; the files are the ones on
    /// disk. A file recorded with another hash has been edited after it was applied - the updater
    /// refuses to apply it again, and this page is where that is seen before a server refuses to start.
    pub fn schema_page(&self) -> Vec<(&'static str, Vec<SchemaFile>)> {
        let mut out = Vec::new();
        for database in &self.schemas {
            let applied = match database.pool.lock() {
                Ok(pool) => database::updater::apply::read_applied(&pool).unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            let files = database::updater::collect_files(
                database.dir,
                database::updater::UpdateState::Released,
            )
            .unwrap_or_default();
            let mut rows = Vec::new();
            for file in &files {
                let name = file.name();
                let state = match applied.iter().find(|(recorded, _)| *recorded == name) {
                    Some((_, hash)) => {
                        let text = std::fs::read_to_string(&file.path).unwrap_or_default();
                        if database::updater::apply::sha1_hex(&text).eq_ignore_ascii_case(hash) {
                            "applied"
                        } else {
                            "changed"
                        }
                    }
                    None => "new",
                };
                rows.push(SchemaFile { name, state });
            }
            for (name, _) in &applied {
                if !files.iter().any(|file| file.name() == *name) {
                    rows.push(SchemaFile {
                        name: name.clone(),
                        state: "recorded, not on disk",
                    });
                }
            }
            out.push((database.label, rows));
        }
        out
    }

    fn say(&self, text: impl Into<String>) {
        self.log.out(LogLevel::Info, "manage", text);
    }
}

/// One connection to the tool.
pub struct ManageSession {
    manage: Arc<Manage>,
}

impl ManageSession {
    /// Answer one connection: read a request, answer it, close.
    pub async fn run(self, mut io: SessionIo) {
        let request = match read_request(&mut io).await {
            Ok(request) => request,
            Err(HttpError::Incomplete) => {
                let _ = io.close().await;
                return;
            }
            Err(error) => {
                let _ = write_response(
                    &mut io,
                    &Response::text(400, format!("a request was expected: {error}\\n")),
                )
                .await;
                let _ = io.close().await;
                return;
            }
        };

        self.manage.say(format!("{} {}", request.method, request.path));
        let response = self.manage.answer(&request);
        let _ = write_response(&mut io, &response).await;
        let _ = io.close().await;
    }
}

impl Session for ManageSession {
    fn run(self: Box<Self>, mut io: SessionIo) -> SessionFuture {
        Box::pin(async move {
            let request = match read_request(&mut io).await {
                Ok(request) => request,
                Err(HttpError::Incomplete) => {
                    let _ = io.close().await;
                    return;
                }
                Err(error) => {
                    let _ = write_response(
                        &mut io,
                        &Response::text(400, format!("a request was expected: {error}\n")),
                    )
                    .await;
                    let _ = io.close().await;
                    return;
                }
            };

            self.manage.say(format!("{} {} ->", request.method, request.path));
            let response = self.manage.answer(&request);
            let _ = write_response(&mut io, &response).await;
            let _ = io.close().await;
        })
    }
}

/// The fields of a URL-encoded form body.
fn form_fields(body: &str) -> std::collections::HashMap<String, String> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (unescape(key), unescape(value)))
        .collect()
}

/// Decode one URL-encoded field: `+` is a space, and `%XX` is a byte.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Some entropy for an identifier a request did not state.
fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    if getrandom(&mut bytes).is_err() {
        return 1;
    }
    u64::from_le_bytes(bytes)
}

/// The operating system's entropy, through the same call the servers use.
fn getrandom(bytes: &mut [u8]) -> Result<(), ()> {
    shared::cryptography::handshake::random_into(bytes).map_err(|_| ())
}

fn main() -> std::process::ExitCode {
    let mut config_path = std::path::PathBuf::from("tools/manage/manage.json");
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-c" | "--config" => match arguments.next() {
                Some(path) => config_path = std::path::PathBuf::from(path),
                None => {
                    eprintln!("-c needs a path");
                    return std::process::ExitCode::FAILURE;
                }
            },
            other => {
                eprintln!("unknown argument: {other}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    match run(&config_path) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("kryta-manage: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(config_path: &std::path::Path) -> Result<(), String> {
    let mut config = Config::load_json(config_path, false, Policy::default())
        .map_err(|error| error.to_string())?;

    let log = Log::new();
    log.initialize(&config).map_err(|error| error.to_string())?;
    log.out(
        LogLevel::Info,
        "manage",
        format!("kryta-manage reading {}", config_path.display()),
    );

    let (bind, _) = config.value_string("Bind", "127.0.0.1").map_err(|error| error.to_string())?;
    let (port, _) = config.value::<u16>("Port", 6602).map_err(|error| error.to_string())?;
    let (token, _) = config.value_string("Token", "").map_err(|error| error.to_string())?;
    config.mark_critical("LoginDatabaseInfo");
    let settings = read_settings(&config, DatabaseKind::Login)?;
    let mut pool = DatabaseWorkerPool::open(DatabaseKind::Login, &settings, &log)?;
    pool.ping()
        .map_err(|error| format!("the login database does not answer: {error}"))?;

    // The world list is the login's own reader, merged with the operator's file: the same rows the
    // auth program sends a client to, not a second reading of them.
    let mut world_list = shared::worlds::WorldList::new();
    let loaded = world_list.load(pool.connection_mut(), &log)?;
    log.out(
        LogLevel::Info,
        "manage",
        format!("World list: {loaded} row(s) in the realmlist table."),
    );
    let (worlds_file, _) = config
        .value_string("WorldsFile", "data/realms.json")
        .map_err(|error| error.to_string())?;
    if !worlds_file.is_empty() {
        world_list.merge_file(&worlds_file, &log)?;
    }
    let login = Arc::new(Mutex::new(pool));
    let accounts = Arc::new(game::accounts::DatabaseAccounts::new(
        Arc::clone(&login),
        log.clone(),
    ));

    // The character database: what the login lists, read through the noun's own repository. A page
    // without one says so rather than failing every request.
    let character_pool = match config.value_string("CharacterDatabaseInfo", "") {
        Ok((info, _)) if !info.is_empty() => {
            let settings = read_settings(&config, DatabaseKind::Character)?;
            match DatabaseWorkerPool::open(DatabaseKind::Character, &settings, &log) {
                Ok(mut pool) => {
                    pool.ping()
                        .map_err(|error| format!("the character database does not answer: {error}"))?;
                    Some(Arc::new(Mutex::new(pool)))
                }
                Err(error) => {
                    log.out(LogLevel::Warn, "manage", format!("the character database is off: {error}"));
                    None
                }
            }
        }
        _ => None,
    };
    let characters = character_pool.as_ref().map(|pool| {
        Arc::new(game::characters::DatabaseCharacters::new(
            Arc::clone(pool),
            log.clone(),
        ))
    });

    // The world database: the maps and their spawn points, read by the same repository the load uses.
    let world_pool = match config.value_string("WorldDatabaseInfo", "") {
        Ok((info, _)) if !info.is_empty() => {
            let settings = read_settings(&config, DatabaseKind::World)?;
            match DatabaseWorkerPool::open(DatabaseKind::World, &settings, &log) {
                Ok(mut pool) => {
                    pool.ping()
                        .map_err(|error| format!("the world database does not answer: {error}"))?;
                    Some(Arc::new(Mutex::new(pool)))
                }
                Err(error) => {
                    log.out(LogLevel::Warn, "manage", format!("the world database is off: {error}"));
                    None
                }
            }
        }
        _ => None,
    };
    let maps = world_pool.as_ref().map(|pool| {
        Arc::new(game::maps::DatabaseMaps::new(
            Arc::clone(pool),
            log.clone(),
        ))
    });
    let items = world_pool.as_ref().map(|pool| {
        Arc::new(game::data_stores::DatabaseItems::new(
            Arc::clone(pool),
            log.clone(),
        ))
    });
    let instances = world_pool.as_ref().map(|pool| {
        Arc::new(game::districts::DatabaseInstances::new(
            Arc::clone(pool),
            log.clone(),
        ))
    });

    // The schema page: each database's own record of what it has applied, against the files on disk.
    let mut schemas = vec![DatabaseSchema {
        label: "login",
        pool: Arc::clone(&login),
        dir: "data/sql/base/db_auth",
    }];
    if let Some(pool) = character_pool {
        schemas.push(DatabaseSchema {
            label: "characters",
            pool,
            dir: "data/sql/base/db_characters",
        });
    }
    if let Some(pool) = world_pool {
        schemas.push(DatabaseSchema {
            label: "world",
            pool,
            dir: "data/sql/base/db_world",
        });
    }

    // The two programs' own settings files, shown by the settings page. Relative to where the tool
    // is started, which is the repository root in the manual test's own commands.
    let (world_config, _) = config
        .value_string("WorldConfigFile", "worldserver/worldserver.json")
        .map_err(|error| error.to_string())?;
    let (auth_config, _) = config
        .value_string("AuthConfigFile", "authserver/authserver.json")
        .map_err(|error| error.to_string())?;
    let world_config = if world_config.is_empty() {
        None
    } else {
        Some(world_config)
    };
    let auth_config = if auth_config.is_empty() {
        None
    } else {
        Some(auth_config)
    };

    let manage = Arc::new(Manage {
        accounts,
        characters,
        maps,
        items,
        instances,
        worlds: Some(Arc::new(world_list)),
        schemas,
        world_config,
        auth_config,
        token: Some(token),
        log: log.clone(),
    });

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;

    runtime.block_on(async move {
        let listener = TcpListener::bind(format!("{bind}:{port}"))
            .await
            .map_err(|error| format!("cannot listen on {bind}:{port}: {error}"))?;
        log.out(
            LogLevel::Info,
            "manage",
            format!("READY: http://{bind}:{port}/ - the administration tool"),
        );

        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(error) => {
                    log.out(
                        LogLevel::Warn,
                        "manage",
                        format!("a connection could not be accepted: {error}"),
                    );
                    continue;
                }
            };
            let manage = Arc::clone(&manage);
            let log = log.clone();
            // One connection at a time, on the current thread: an operator's page is not a load.
            let session = ManageSession {
                manage: Arc::clone(&manage),
            };
            let io = SessionIo::new(stream);
            session.run(io).await;
            log.out(LogLevel::Debug, "manage", format!("{peer} closed"));
        }
    })
}
