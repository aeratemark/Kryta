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
mod navigation;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::configuration::{Config, Policy};
use common::logging::{Log, LogLevel};
use database::database::{DatabaseKind, DatabaseWorkerPool, read_settings};
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
    /// The creation record lists' paths: the content page shows the profession sets they hold.
    inventory_file: Option<String>,
    equipment_file: Option<String>,
    /// The portable graph cache the manager inspects and selected-map exports update.
    path_graphs_file: String,
    /// An explicit client archive used only by the manager's offline dump action.
    map_archive_file: String,
    /// The running world's local control endpoint, when `manage.json` names one: the status page asks
    /// it for `world.status`. This is the manager's connection to the world, never a second world.
    control: Option<Control>,
    /// The running stack's own loopback channel, discovered from its `stack.json`. The manager is a
    /// client of this channel and never starts or watches a process itself.
    supervisor: Option<Supervisor>,
    token: Option<String>,
    log: Log,
}

/// Where the running world answers operator operations: `worldserver`'s loopback control endpoint.
pub struct Control {
    /// The loopback port the world program listens on (`Control.Port` in its own settings).
    port: u16,
    /// The service credential the world program requires, read from its restricted file.
    credential: String,
    /// How long the status page waits for the world before reporting it unavailable.
    timeout: Duration,
}

/// Where the running supervisor accepts the lifecycle verbs it owns.
pub struct Supervisor {
    port: u16,
    timeout: Duration,
}

impl Manage {
    /// One page, answered.
    fn answer(&self, request: &Request) -> Response {
        if !self.authorised(request) {
            return Response::html(401, pages::unauthorised());
        }

        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/") => pages::dashboard(self),
            ("GET", "/services") => pages::services(self, None),
            ("POST", "/services/stop") => self.service_action(request, "stop"),
            ("POST", "/services/restart") => self.service_action(request, "restart"),
            ("GET", "/configuration") => pages::configuration(),
            ("GET", "/assets") => pages::assets(),
            ("GET", "/navigation") => pages::navigation_cache(self, None),
            ("POST", "/navigation/dump") => self.dump_navigation_page(request),
            ("GET", "/api/navigation/maps") => self.navigation_api(),
            ("POST", "/api/navigation/dump") => self.dump_navigation_api(request),
            ("GET", "/console") => pages::console(self, None),
            ("POST", "/console/run") => self.run_operation(request),
            ("GET", "/logs") => pages::logs(),
            ("GET", "/status") => pages::world_status(self),
            ("GET", "/accounts") => pages::accounts(
                self,
                request
                    .query("account")
                    .and_then(|value| value.parse::<i64>().ok()),
            ),
            ("GET", "/characters") => pages::characters(self, request.query("account")),
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
            ("POST", "/accounts/update") => self.update_account(request),
            ("POST", "/accounts/active") => self.set_account_active(request),
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

    /// The running world's own status, asked of its control endpoint.
    ///
    /// One operation, carried by the world's request queue and executed on the world's update thread,
    /// so a page can read the world without becoming a second owner of its state
    /// (`docs/port/14-operator-surfaces.md`, "First slice: local manager reads world status").
    pub fn world_status(&self) -> Result<String, String> {
        self.world_operation("world.status", &BTreeMap::new())
    }

    /// Call one registered operation through the same endpoint as `kryta-cli`. The world retains
    /// argument validation, queue ownership and execution; this browser route is only an adapter.
    pub fn world_operation(
        &self,
        operation: &str,
        arguments: &BTreeMap<String, String>,
    ) -> Result<String, String> {
        let Some(control) = self.control.as_ref() else {
            return Err(
                "no world control endpoint is configured: set ControlPort and ControlCredentialFile \
                 in manage.json, and Control.Enable with a credential file in worldserver.json"
                    .into(),
            );
        };
        let arguments = serde_json::Value::Object(
            arguments
                .iter()
                .map(|(name, value)| (name.clone(), serde_json::Value::String(value.clone())))
                .collect(),
        );
        let body = serde_json::json!({
            "request_id": "manage-console",
            "operation": operation,
            "arguments": arguments
        })
        .to_string();
        let request = format!(
            "POST /v1/operations HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            control.credential,
            body.len()
        );
        let address = SocketAddr::from(([127, 0, 0, 1], control.port));
        let mut stream =
            TcpStream::connect_timeout(&address, control.timeout).map_err(|error| {
                format!("the world control endpoint at {address} is unavailable: {error}")
            })?;
        stream
            .set_read_timeout(Some(control.timeout))
            .map_err(|error| error.to_string())?;
        stream
            .write_all(request.as_bytes())
            .map_err(|error| format!("the request to the world failed: {error}"))?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| format!("the world's answer could not be read: {error}"))?;
        let (_, body) = response
            .split_once("\r\n\r\n")
            .ok_or("the world answered with something that is not an HTTP response")?;
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|_| "the world answered with something that is not JSON")?;
        if value.get("ok").and_then(|ok| ok.as_bool()) == Some(true) {
            return Ok(value
                .get("result")
                .and_then(|result| result.as_str())
                .unwrap_or_default()
                .to_string());
        }
        Err(value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(|message| message.as_str())
            .unwrap_or("the world refused the operation")
            .to_string())
    }

    /// Ask the running supervisor for the state or one of its documented lifecycle verbs.
    pub fn supervisor_request(&self, line: &str) -> Result<String, String> {
        let Some(supervisor) = self.supervisor.as_ref() else {
            return Err(
                "no supervisor control channel is configured: set SupervisorStackFile to a stack.json \
                 with control_port, then start kryta-supervisor"
                    .into(),
            );
        };
        let address = SocketAddr::from(([127, 0, 0, 1], supervisor.port));
        let mut stream = TcpStream::connect_timeout(&address, supervisor.timeout)
            .map_err(|error| format!("the supervisor at {address} is unavailable: {error}"))?;
        stream
            .set_read_timeout(Some(supervisor.timeout))
            .map_err(|error| error.to_string())?;
        stream
            .write_all(format!("{}\n", line.trim()).as_bytes())
            .map_err(|error| format!("the request to the supervisor failed: {error}"))?;
        let mut answer = String::new();
        stream
            .read_to_string(&mut answer)
            .map_err(|error| format!("the supervisor's answer could not be read: {error}"))?;
        let answer = answer.trim().to_string();
        if answer.starts_with("err:") {
            return Err(answer);
        }
        Ok(answer)
    }

    fn service_action(&self, request: &Request, action: &str) -> Response {
        let form = form_fields(&request.body_text());
        let Some(name) = form.get("service").filter(|name| service_name(name)) else {
            return Response::html(
                400,
                pages::message(
                    "A service name contains only letters, digits, dot, dash, or underscore.",
                ),
            );
        };
        let result = self.supervisor_request(&format!("{action} {name}"));
        pages::services(self, Some((action, name, result)))
    }

    fn run_operation(&self, request: &Request) -> Response {
        let form = form_fields(&request.body_text());
        let Some(operation) = form
            .get("operation")
            .filter(|operation| !operation.trim().is_empty())
        else {
            return Response::html(400, pages::message("An operation name is required."));
        };
        let arguments =
            match operation_arguments(form.get("arguments").map(String::as_str).unwrap_or("")) {
                Ok(arguments) => arguments,
                Err(error) => return Response::html(400, pages::message(&error)),
            };
        let result = self.world_operation(operation, &arguments);
        pages::console(self, Some((operation, arguments, result)))
    }

    /// Inspect the extracted graphs. This manager API reads an operator asset, not state the running
    /// world owns, so it does not go through the world-operation queue.
    pub fn inspect_navigation(&self) -> Result<navigation::Inspection, String> {
        navigation::inspect(&self.path_graphs_file)
    }

    fn dump_navigation(&self, map_id: u32) -> Result<navigation::DumpResult, String> {
        if self.map_archive_file.trim().is_empty() {
            return Err("MapArchiveFile is empty; the manager has no archive selected for offline extraction".to_string());
        }
        let Some(source) = self.maps.as_ref() else {
            return Err("no world database is configured, so the manager cannot resolve a map id".to_string());
        };
        let (maps, _) = source.load()?;
        let map = maps
            .iter()
            .find(|map| map.id == map_id)
            .ok_or_else(|| format!("map {map_id} is not in the world database"))?;
        navigation::dump_map(&self.path_graphs_file, &self.map_archive_file, map)
    }

    fn navigation_api(&self) -> Response {
        match self.inspect_navigation() {
            Ok(inspection) => Response::new(
                200,
                "application/json; charset=utf-8",
                serde_json::json!({
                    "path": inspection.path,
                    "bytes": inspection.bytes,
                    "maps": inspection.maps.iter().map(|map| serde_json::json!({
                        "map_id": map.map_id,
                        "file_id": map.file_id,
                        "planes": map.planes,
                        "trapezoids": map.trapezoids,
                    })).collect::<Vec<_>>(),
                })
                .to_string(),
            ),
            Err(error) => Response::new(
                500,
                "application/json; charset=utf-8",
                serde_json::json!({"error": error}).to_string(),
            ),
        }
    }

    fn dump_navigation_api(&self, request: &Request) -> Response {
        let map_id = form_fields(&request.body_text())
            .get("map_id")
            .and_then(|value| value.parse::<u32>().ok());
        let Some(map_id) = map_id else {
            return Response::new(400, "application/json; charset=utf-8", "{\"error\":\"map_id is required\"}");
        };
        match self.dump_navigation(map_id) {
            Ok(result) => Response::new(
                200,
                "application/json; charset=utf-8",
                serde_json::json!({
                    "map_id": result.map_id,
                    "file_id": result.file_id,
                    "cache": result.cache,
                    "graph_count": result.graph_count,
                    "bytes": result.bytes,
                })
                .to_string(),
            ),
            Err(error) => Response::new(
                400,
                "application/json; charset=utf-8",
                serde_json::json!({"error": error}).to_string(),
            ),
        }
    }

    fn dump_navigation_page(&self, request: &Request) -> Response {
        let map_id = form_fields(&request.body_text())
            .get("map_id")
            .and_then(|value| value.parse::<u32>().ok());
        let result = match map_id {
            Some(map_id) => self.dump_navigation(map_id).map(|result| {
                format!(
                    "Dumped map {} from blob {:#X}; {} graph(s), {} bytes in {}.",
                    result.map_id, result.file_id, result.graph_count, result.bytes, result.cache
                )
            }),
            None => Err("A numeric map id is required.".to_string()),
        };
        pages::navigation_cache(self, Some(result))
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
            Err(error) => Response::html(
                500,
                pages::message(&format!("The account table said: {error}")),
            ),
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
            .unwrap_or_else(|| {
                format!(
                    "00000000-0000-0000-0000-{:012}",
                    random_u64() % 1_000_000_000_000
                )
            });
        let token = form
            .get("token")
            .cloned()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                format!(
                    "00000000-0000-0000-0000-{:012}",
                    random_u64() % 1_000_000_000_000
                )
            });
        match self.accounts.create(username, &user_id, &token) {
            Ok(true) => {
                self.say(format!("{username}: created"));
                pages::redirect("/accounts")
            }
            Ok(false) => Response::html(
                409,
                pages::message(&format!("An account is already called {username}.")),
            ),
            Err(error) => Response::html(
                500,
                pages::message(&format!("The account table said: {error}")),
            ),
        }
    }

    /// Update the three values the portal reads for an existing account. The stable row id comes from
    /// the page, so a renamed login remains the same account.
    fn update_account(&self, request: &Request) -> Response {
        let form = form_fields(&request.body_text());
        let Some(id) = form.get("id").and_then(|value| value.parse::<i64>().ok()) else {
            return Response::html(400, pages::message("A valid account id is required."));
        };
        let Some(username) = form
            .get("username")
            .filter(|value| !value.trim().is_empty())
        else {
            return Response::html(400, pages::message("An account login name is required."));
        };
        let Some(user_id) = form.get("user_id") else {
            return Response::html(400, pages::message("An account user id is required."));
        };
        let Some(token) = form.get("token") else {
            return Response::html(400, pages::message("An account token is required."));
        };
        match self.accounts.update(id, username, user_id, token) {
            Ok(true) => {
                self.say(format!("account {id}: updated as {username}"));
                pages::redirect("/accounts")
            }
            Ok(false) => Response::html(404, pages::message(&format!("No account has id {id}."))),
            Err(error) => Response::html(
                500,
                pages::message(&format!("The account table said: {error}")),
            ),
        }
    }

    /// Deactivate or reactivate an account without removing its row or character data. The auth server
    /// applies the state at its next login lookup; it does not terminate a connection already in play.
    fn set_account_active(&self, request: &Request) -> Response {
        let form = form_fields(&request.body_text());
        let Some(id) = form.get("id").and_then(|value| value.parse::<i64>().ok()) else {
            return Response::html(400, pages::message("A valid account id is required."));
        };
        let active = matches!(form.get("active").map(String::as_str), Some("true"));
        match self.accounts.set_active(id, active) {
            Ok(true) => {
                let state = if active { "activated" } else { "deactivated" };
                self.say(format!("account {id}: {state}"));
                pages::redirect("/accounts")
            }
            Ok(false) => Response::html(404, pages::message(&format!("No account has id {id}."))),
            Err(error) => Response::html(
                500,
                pages::message(&format!("The account table said: {error}")),
            ),
        }
    }

    fn delete(&self, request: &Request) -> Response {
        let form = form_fields(&request.body_text());
        let Some(username) = form.get("username") else {
            return Response::html(400, pages::message("A request with no account name."));
        };
        match self.accounts.delete(username) {
            Ok(true) => {
                self.say(format!(
                    "{username}: removed (its characters are untouched)"
                ));
                pages::redirect("/accounts")
            }
            Ok(false) => Response::html(
                404,
                pages::message(&format!("No account is called {username}.")),
            ),
            Err(error) => Response::html(
                500,
                pages::message(&format!("The account table said: {error}")),
            ),
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
                self.say(format!(
                    "character {uuid}: removed, with its items and unlocks"
                ));
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

    /// The characters of one account, for the account workspace. A character remains owned by the
    /// account that created it; this is not a second character inventory kept by the manager.
    pub fn characters_of_account(&self, account: &str) -> Option<Vec<game::characters::Character>> {
        self.characters
            .as_ref()
            .map(|characters| characters.for_account(account))
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
        (self.world_config.as_deref(), self.auth_config.as_deref())
    }

    /// The profession sets the creation record holds, read through the noun's own loader: what a
    /// new character may be given, and the content page's view of it. Loading per request keeps the
    /// page honest when the files change under it.
    pub fn creation_sets(&self) -> Option<Vec<game::creation::EquipmentSet>> {
        let inventory = self.inventory_file.as_deref()?;
        let equipment = self.equipment_file.as_deref()?;
        let log = Log::new();
        game::creation::Tables::load(inventory, equipment, &log)
            .ok()
            .map(|tables| tables.sets().to_vec())
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
        let Some(map_id) = form
            .get("map_id")
            .and_then(|value| value.parse::<u32>().ok())
        else {
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
        let Some(map_id) = form
            .get("map_id")
            .and_then(|value| value.parse::<u32>().ok())
        else {
            return Response::html(400, pages::message("The form names no map."));
        };
        let Some(index) = form
            .get("index")
            .and_then(|value| value.parse::<i16>().ok())
        else {
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
        let Some(map_id) = form
            .get("map_id")
            .and_then(|value| value.parse::<u32>().ok())
        else {
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
            Ok(false) => {
                Response::html(404, pages::message(&format!("No map is called {map_id}.")))
            }
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

        self.manage
            .say(format!("{} {}", request.method, request.path));
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

            self.manage
                .say(format!("{} {} ->", request.method, request.path));
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

/// A stack service is an identifier, not a free-form supervisor command. Keeping that boundary here
/// lets the page select only existing protocol verbs and prevents a form value from becoming a second
/// command language.
fn service_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

/// The console's argument area uses one `name=value` assignment per line, the same named values the
/// world endpoint accepts. The endpoint still decides whether the names and count are valid for its
/// registration.
fn operation_arguments(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut arguments = BTreeMap::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((name, value)) = line.split_once('=') else {
            return Err(format!("An argument must be name=value: {line}"));
        };
        let name = name.trim();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(format!(
                "An argument name is letters, digits, or underscore: {name}"
            ));
        }
        if arguments
            .insert(name.to_string(), value.trim().to_string())
            .is_some()
        {
            return Err(format!("An argument is named twice: {name}"));
        }
    }
    Ok(arguments)
}

/// Read only the existing supervisor control port from its stack file. The service rows remain owned
/// by the supervisor and are never parsed into manager state.
fn supervisor_port(path: &str) -> Result<u16, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read SupervisorStackFile {path}: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("SupervisorStackFile {path} is not JSON: {error}"))?;
    let port = value
        .get("control_port")
        .and_then(serde_json::Value::as_u64)
        .filter(|port| *port > 0 && *port <= u16::MAX as u64)
        .ok_or_else(|| format!("SupervisorStackFile {path} has no usable control_port"))?;
    Ok(port as u16)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_arguments_are_named_once_and_left_for_the_world_to_validate() {
        let arguments =
            operation_arguments("destination=ascalon\nmode=hard").expect("arguments parse");
        assert_eq!(arguments.get("destination"), Some(&"ascalon".to_string()));
        assert_eq!(arguments.get("mode"), Some(&"hard".to_string()));
        assert!(operation_arguments("destination=ascalon\ndestination=kryta").is_err());
        assert!(operation_arguments("not an assignment").is_err());
    }

    #[test]
    fn a_service_name_cannot_change_the_supervisor_line() {
        assert!(service_name("world"));
        assert!(service_name("world-2"));
        assert!(!service_name("world restart auth"));
        assert!(!service_name("world\nquit"));
    }

    #[test]
    fn only_a_real_control_port_is_read_from_a_stack_file() {
        let path =
            std::env::temp_dir().join(format!("kryta-manage-stack-{}.json", std::process::id()));
        std::fs::write(&path, "{\"control_port\":7612,\"services\":[]}").expect("stack is written");
        assert_eq!(
            supervisor_port(&path.to_string_lossy()).expect("port reads"),
            7612
        );
        std::fs::write(&path, "{\"control_port\":0}").expect("stack is rewritten");
        assert!(supervisor_port(&path.to_string_lossy()).is_err());
        let _ = std::fs::remove_file(path);
    }
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

    let (bind, _) = config
        .value_string("Bind", "127.0.0.1")
        .map_err(|error| error.to_string())?;
    let (port, _) = config
        .value::<u16>("Port", 6602)
        .map_err(|error| error.to_string())?;
    let (token, _) = config
        .value_string("Token", "")
        .map_err(|error| error.to_string())?;
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
                    pool.ping().map_err(|error| {
                        format!("the character database does not answer: {error}")
                    })?;
                    Some(Arc::new(Mutex::new(pool)))
                }
                Err(error) => {
                    log.out(
                        LogLevel::Warn,
                        "manage",
                        format!("the character database is off: {error}"),
                    );
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
                    log.out(
                        LogLevel::Warn,
                        "manage",
                        format!("the world database is off: {error}"),
                    );
                    None
                }
            }
        }
        _ => None,
    };
    let maps = world_pool
        .as_ref()
        .map(|pool| Arc::new(game::maps::DatabaseMaps::new(Arc::clone(pool), log.clone())));
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
    let (inventory_file, _) = config
        .value_string("InventoryFile", "data/inventory.json")
        .map_err(|error| error.to_string())?;
    let (equipment_file, _) = config
        .value_string("EquipmentFile", "data/equipment.json")
        .map_err(|error| error.to_string())?;
    let inventory_file = if inventory_file.is_empty() {
        None
    } else {
        Some(inventory_file)
    };
    let equipment_file = if equipment_file.is_empty() {
        None
    } else {
        Some(equipment_file)
    };
    let (path_graphs_file, _) = config
        .value_string("PathGraphsFile", "data/path-graphs.bin")
        .map_err(|error| error.to_string())?;
    let (map_archive_file, _) = config
        .value_string("MapArchiveFile", "")
        .map_err(|error| error.to_string())?;

    let (control_port, _) = config
        .value::<u16>("ControlPort", 0)
        .map_err(|error| error.to_string())?;
    let (control_credential_file, _) = config
        .value_string("ControlCredentialFile", "")
        .map_err(|error| error.to_string())?;
    let (control_timeout, _) = config
        .value::<u64>("ControlTimeoutSeconds", 5)
        .map_err(|error| error.to_string())?;
    let control = if control_port == 0 || control_credential_file.is_empty() {
        None
    } else {
        let credential = std::fs::read_to_string(&control_credential_file)
            .map_err(|error| {
                format!("cannot read ControlCredentialFile {control_credential_file}: {error}")
            })?
            .trim()
            .to_owned();
        if credential.len() < 32 {
            return Err(format!(
                "ControlCredentialFile {control_credential_file} must hold at least 32 characters"
            ));
        }
        Some(Control {
            port: control_port,
            credential,
            timeout: Duration::from_secs(control_timeout.max(1)),
        })
    };

    // The supervisor owns the port and the process list in stack.json. The manager reads only the
    // existing control_port, so a browser action remains a client request rather than a second
    // process supervisor.
    let (supervisor_stack_file, _) = config
        .value_string("SupervisorStackFile", "tools/supervisor/stack.json")
        .map_err(|error| error.to_string())?;
    let (supervisor_timeout, _) = config
        .value::<u64>("SupervisorTimeoutSeconds", 20)
        .map_err(|error| error.to_string())?;
    let supervisor = if supervisor_stack_file.is_empty() {
        None
    } else {
        match supervisor_port(&supervisor_stack_file) {
            Ok(port) => Some(Supervisor {
                port,
                timeout: Duration::from_secs(supervisor_timeout.max(1)),
            }),
            Err(error) => {
                log.out(LogLevel::Warn, "manage", error);
                None
            }
        }
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
        inventory_file,
        equipment_file,
        path_graphs_file,
        map_archive_file,
        control,
        supervisor,
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
        // The stack's handshake: printed only when a supervisor asked for it (`KRYTA_STACK`), because
        // nothing this tool says belongs on a screen the operator did not ask to fill. The record is
        // the appenders, and these same words go there.
        let ready = format!("READY: listening on http://{bind}:{port}/ - the administration tool");
        if std::env::var_os("KRYTA_STACK").is_some() {
            // Only ever read by the supervisor that set the variable, and never shown to anybody.
            println!("{ready}");
        }
        log.out(LogLevel::Info, "manage", ready);

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
