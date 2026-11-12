//! The gameplay conversation: the second connection, from its version message to a loaded map.
//!
//! Reference: `src/server/game/Server/WorldSocket.cpp` and `WorldSession.cpp` - the object a
//! gameplay connection becomes, and the session behind it. What happens here, in the reference's
//! order:
//!
//! 1. The client's 64-byte version message, which names the world it was sent to (`0x000C0500`).
//! 2. The seed exchange, the same one the login port makes: `0x4200` in, `0x1601` out, one key
//!    derived from the master, one RC4 per direction.
//! 3. The travel record the login wrote is claimed once, by the token pair the version message
//!    presents (`docs/port/06-world-plan.md`, "The three seams"); the record's map decides what
//!    loads, and a connection with no record keeps the map its message names.
//! 4. The block the world sends unasked. For the character-creation instance that is two messages -
//!    the load head and the creation start - because the creation instance has no geometry and no
//!    agent of its own. For a map it is the load head, the player-data start, the character's name
//!    and the instance's record, followed by the ping pair, and then the client's asks are answered
//!    in the order a working server measured (`recordings/gwps/world/load-stream.log`).
//!
//! Reference for the sizes: `shared/cryptography/handshake.rs` and `Packets/InstancePackets.rs`.

use common::logging::{Log, LogLevel};
use shared::cryptography::handshake::{self, CLIENT_SEED_HEADER, CLIENT_SEED_LEN, SERVER_SEED_LEN};
use shared::cryptography::AuthCrypt;
use shared::network::socket::{Session, SessionFuture, SessionIo};

use crate::server::packets::{
    agent_packets, character_packets, creation_load_block, instance_packets, item_packets,
    misc_packets, party_packets,
};
use crate::server::protocol::GameVersion;
use crate::server::protocol::FrameError;
use database::database::Param;
use crate::world::World;

/// The agent id a character-creation connection's player is drawn as.
///
/// A creation screen shows one body - the character being made - and a world here has no agent
/// registry yet, so the first id is the one it gets. The reference allocates from its own pool; the
/// number is the world's to give, and nothing else reads it until a map exists.
pub(crate) const CREATION_AGENT: u32 = 1;

/// What a load reads from the character's own row: the name it shows, the campaign and profession
/// whose starting set its item stream carries, the id its items are recorded under, and the gold it
/// carries (R45: gold is per character).
#[derive(Debug, Clone)]
pub(crate) struct CharacterSheet {
    pub name: String,
    pub campaign: u8,
    pub profession: u8,
    pub uuid: [u8; 16],
    pub gold: u32,
    /// The packed look the client builds the model from, or 0 when the row records none.
    pub appearance: u32,
    /// The character's level, which the progress record and the agent's own property carry.
    pub level: u8,
    /// The professions the character has unlocked, as a bitmap.
    pub unlocked_professions: u32,
}

/// The two dword lists a load sends at full length whether or not a character has used them: the
/// hard-mode mission and bonus bitmaps. An empty list is not the same message - it is an array the
/// client can index past the end of (`ASURA/crates/asura-world/src/player.rs`, `UNLOCKED_MAP_ZEROS`,
/// copied from the reference's own send log: 31 zero words each).
const UNLOCKED_MAP_ZEROS: &[u32] = &[0u32; 31];

/// The team token a load gives the client. Evidence: the reference's own trace carried `0D F0 AD BA`
/// in `0x00F2` (`recordings/gwps/world/load-stream.log`), which is `ASURA`'s `PLAYER_TEAM_TOKEN`.
const PLAYER_TEAM_TOKEN: u32 = 0xBAAD_F00D;

/// Full morale, which a fresh agent has (`ASURA/crates/asura-world/src/player.rs`, `MORALE_FULL`).
const MORALE_FULL: u32 = 100;

/// The client's asks on a gameplay connection, as the reference's dispatch reads them.
///
/// Sizes from `docs/evidence/client-protocol.md`'s table: the request-player and
/// request-armors asks are the header alone, a profession change is a campaign and a profession, the
/// confirm carries the character's name and appearance, and a ping is the header alone.
const CHAR_CREATION_REQUEST_PLAYER: u16 = 0x8089;
const CHAR_CREATION_REQUEST_ARMORS: u16 = 0x808A;
const CHAR_CREATION_CHANGE_PROF: u16 = 0x8060;
const CHAR_CREATION_CONFIRM: u16 = 0x808B;
/// `GAME_CMSG_PING_REQUEST`: two bytes on the gameplay connection.
const GAME_PING: u16 = 0x800D;

/// `GAME_CMSG_ITEM_MOVE` (`0x8072`): a drag in the inventory - the item, the bag and the slot.
const CLIENT_ITEM_MOVE: u16 = 0x8072;
/// `GAME_CMSG_ITEM_DESTROY` (`0x8069`): the item to destroy.
const CLIENT_ITEM_DESTROY: u16 = 0x8069;
/// `GAME_CMSG_EQUIP_ITEM` (`0x8030`): the item to wear.
const CLIENT_EQUIP_ITEM: u16 = 0x8030;

/// The client's movement reports: `0x803D` carries the direction as well, the others a point.
const CLIENT_MOVE_TO_COORD_WITH_DIR: u16 = 0x803D;
const CLIENT_MOVE_TO_COORD: u16 = 0x803E;
const CLIENT_LAST_POS_BEFORE_MOVE_CANCELED: u16 = 0x8047;

/// The client's in-game travel request - `PARTY_TRAVEL`, `0x00B1`, nine bytes (E220, E28).
const CLIENT_PARTY_TRAVEL: u16 = 0x00B1;

// The simulation's tick period is a setting (R33): `World.SimulationTickMs` (default 105 - the
// reference sent one `0x001E` every 90-110 ms, average 105 ms over 1780 ticks,
// `recordings/asura/feature/travel/first-crossing-20261031.jsonl`; the client's world time is this
// world's tick, so the cadence is an operator's number, not a law). `WorldConfig::simulation_tick_ms`
// carries it and the program logs it.

/// The statement a creation confirm runs.
///
/// `ON CONFLICT (name) DO NOTHING` is the rule the reference enforces with a unique name: a name that
/// is taken writes nothing, and the empty answer is what says so. The account is the portal's own uuid
/// from the gameplay connection's version message, so the world needs no second database.
const INSERT_CHARACTER: &str = "INSERT INTO \"characters\" (\"uuid\", \"account_uuid\", \"name\", \
\"appearance\", \"primary_profession\", \"secondary_profession\", \"unlocked_professions\", \
\"campaign\", \"level\", \"is_pvp\", \"helm_status\", \"gold\", \"last_outpost\", \"experience\") \
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
ON CONFLICT (\"name\") DO NOTHING RETURNING 1";

/// What a character-creation connection has chosen so far.
///
/// Reference: `char_creation_campaign` and `primary_profession` on the reference's player, which is
/// the whole of what a creation screen keeps until its confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Creation {
    campaign: u8,
    profession: u8,
}

impl Creation {
    /// The reference's starting state: Warrior, and no campaign until the client says so.
    fn start() -> Self {
        Self {
            campaign: 0,
            profession: character_packets::DEFAULT_PROFESSION,
        }
    }
}

/// The player-data block a creation screen is drawn from, in the reference's order.
///
/// Reference: `GameSrv_HandleCharCreationRequestPlayer` - the player-data start, the inventory stream,
/// the four weapon sets and the gold, the faction caps, the attribute and profession state, the
/// skillbar, the faction record and the agent's properties, then the player-data end. The block is
/// `crate::creation::player_data_block`, because the inventory it carries is content.
fn creation_player_block(tables: Option<&crate::creation::Tables>, creation: &Creation) -> Vec<Vec<u8>> {
    match tables {
        Some(tables) => {
            crate::creation::player_data_block(tables, creation.campaign, creation.profession)
        }
        // A session with no creation data still answers: the block without the inventory, which is
        // what says a profession's equipment is missing rather than that the screen failed.
        None => crate::creation::player_data_block(&crate::creation::Tables::default(), creation.campaign, creation.profession),
    }
}


/// One gameplay connection.
pub struct WorldSession {
    log: Log,
    /// Which world this is: a connection that names another is not this world's.
    realm: u32,
    /// The maps the world hosts, so a connection can be told what it may enter.
    maps: Vec<crate::world::MapEntry>,
    /// Where a character may stand on each map, for the entry placement of R13.
    spawns: Vec<crate::maps::MapSpawn>,
    /// The numbers this world runs by: the spawn scatter and the rest.
    settings: crate::world::world_config::WorldConfig,
    /// The character database, when the program opened one: a creation screen's confirm writes a row
    /// there. A session without it can draw the screen but cannot finish it.
    characters: Option<std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>>,
    /// The login database, when the program opened one: the travel record a login wrote is claimed
    /// here, once, and decides the map and the district the connection loads
    /// (`docs/port/06-world-plan.md`, "The three seams").
    login: Option<std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>>,
    /// The world's running district copies, shared with every session: entering a map claims a place
    /// in a district, and the connection gives it back when it ends.
    districts: std::sync::Arc<std::sync::Mutex<crate::districts::Districts>>,
    /// The district this connection entered, so [`Self::leave_district`] can give the place back.
    /// `None` until a place has been entered.
    entered: std::sync::Mutex<Option<(u32, u32)>>,
    /// The player's own agent: where the client says it is. The client moves it; the world keeps
    /// what it reports, which is what a later load or a second client would be told.
    agent: std::sync::Mutex<Option<crate::agents::Agent>>,
    /// The bags, items and profession sets a creation screen is built from, when the program read them.
    creation: Option<crate::creation::Tables>,
    /// The item templates the world holds, so a load can resolve a character's held items.
    templates: Vec<crate::data_stores::ItemDefinition>,
    /// The master secret to draw the exchange from, when the caller wants a known one. A program
    /// leaves it empty and every connection draws its own.
    master: Option<[u8; shared::cryptography::handshake::MASTER_SECRET_LEN]>,
}

impl WorldSession {
    /// A session for one connection, for this world.
    pub fn new(log: Log, world: &World) -> Self {
        Self {
            log,
            realm: world.realm(),
            maps: world.maps().to_vec(),
            spawns: world.spawns().to_vec(),
            settings: world.settings().clone(),
            characters: None,
            login: None,
            districts: world.districts(),
            entered: std::sync::Mutex::new(None),
            agent: std::sync::Mutex::new(None),
            creation: Some(world.creation().clone()),
            templates: world.templates().to_vec(),
            master: None,
        }
    }

    /// A session that can write a character.
    pub fn with_characters(
        log: Log,
        world: &World,
        characters: std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>,
    ) -> Self {
        Self {
            characters: Some(characters),
            ..Self::new(log, world)
        }
    }

    /// A session that can write a character and claim a travel.
    ///
    /// The program's own shape: the travel table is in the login database, and the version message's
    /// token pair is what claims a row.
    pub fn with_characters_and_login(
        log: Log,
        world: &World,
        characters: std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>,
        login: std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>,
    ) -> Self {
        Self {
            characters: Some(characters),
            login: Some(login),
            ..Self::new(log, world)
        }
    }

    /// A session whose exchange uses this master secret instead of one drawn from entropy.
    ///
    /// For a test that wants to read what the client would read; a program never calls this.
    pub fn with_master(
        log: Log,
        world: &World,
        master: [u8; shared::cryptography::handshake::MASTER_SECRET_LEN],
    ) -> Self {
        Self {
            master: Some(master),
            ..Self::new(log, world)
        }
    }

    /// A session whose creation screen has no inventory: the shape a test uses when it wants the
    /// frames without the content files.
    pub fn without_creation_data(log: Log, world: &World) -> Self {
        Self {
            creation: None,
            ..Self::new(log, world)
        }
    }

    /// A session with both a known master and a character database.
    ///
    /// The shape a test that plays a whole character creation uses: it needs to read the exchange's
    /// answer and to see the row the confirm writes.
    pub fn with_master_and_characters(
        log: Log,
        world: &World,
        master: [u8; shared::cryptography::handshake::MASTER_SECRET_LEN],
        characters: std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>,
    ) -> Self {
        Self {
            characters: Some(characters),
            ..Self::with_master(log, world, master)
        }
    }

    /// A session with a known master, a character database and a login database.
    ///
    /// The shape a test that plays a whole travel uses: the row the login would have written is
    /// claimed by the connection, and the map it names is the one loaded.
    pub fn with_master_and_characters_and_login(
        log: Log,
        world: &World,
        master: [u8; shared::cryptography::handshake::MASTER_SECRET_LEN],
        characters: std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>,
        login: std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>,
    ) -> Self {
        Self {
            characters: Some(characters),
            login: Some(login),
            ..Self::with_master(log, world, master)
        }
    }

    /// The inventory a load draws: the character's own rows when it has them - what a confirm wrote -
    /// and the creation record set otherwise, which is what every character starts from.
    fn load_inventory(&self, sheet: &CharacterSheet) -> crate::creation::Inventory {
        if let Some(pool) = self.characters.as_ref() {
            let characters = crate::characters::DatabaseCharacters::new(
                std::sync::Arc::clone(pool),
                self.log.clone(),
            );
            match characters.items_of(&crate::characters::uuid_text(&sheet.uuid)) {
                Ok(held) if !held.is_empty() => {
                    let bags = self
                        .creation
                        .as_ref()
                        .map(crate::creation::bags)
                        .unwrap_or(&[]);
                    return crate::creation::inventory_from_held(
                        bags,
                        &held,
                        &self.templates,
                        &self.log,
                    );
                }
                Ok(_) => {}
                Err(error) => self.say(
                    LogLevel::Warn,
                    format!(
                        "The character's items cannot be read ({error}); the creation record's set is drawn instead."
                    ),
                ),
            }
        }
        self.creation
            .as_ref()
            .map(|tables| tables.inventory(sheet.campaign, sheet.profession))
            .unwrap_or_default()
    }

    /// Where an item of this character would be worn: the slot its kind is worn in, in the bag the
    /// setting says is equipped.
    ///
    /// `None`, with the reason said, when the item is not the character's, no template describes it,
    /// its kind is not worn, or the setting names no equipped bag.
    fn equip_place(
        &self,
        characters: &crate::characters::DatabaseCharacters,
        sheet: &CharacterSheet,
        item_id: u32,
    ) -> Option<(u16, u8)> {
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let held = match characters.items_of(&uuid) {
            Ok(held) => held,
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("The character's items cannot be read: {error}"),
                );
                return None;
            }
        };
        let Some(row) = held.iter().find(|row| row.item_id == item_id) else {
            self.say(
                LogLevel::Info,
                format!("Item {item_id} is not the character's, so it cannot be worn."),
            );
            return None;
        };
        let template = self
            .templates
            .iter()
            .find(|template| row.template_id != 0 && template.id == row.template_id)
            .or_else(|| {
                self.templates
                    .iter()
                    .find(|template| template.file_id == row.file_id)
            });
        let Some(template) = template else {
            self.say(
                LogLevel::Warn,
                format!("No item template describes item {item_id}; it cannot be worn."),
            );
            return None;
        };
        let Some(slot) = crate::creation::equipped_slot(template.item_type) else {
            self.say(
                LogLevel::Info,
                format!(
                    "Item {item_id} is kind {}, which is not worn.",
                    template.item_type
                ),
            );
            return None;
        };
        let Some(bag) = self
            .creation
            .as_ref()
            .and_then(crate::creation::equipped_bag)
        else {
            self.say(
                LogLevel::Warn,
                "The setting names no equipped bag, so nothing can be worn.",
            );
            return None;
        };
        Some((bag.id, slot))
    }

    fn say(&self, level: LogLevel, text: impl Into<String>) {
        self.log.out(level, "server.worldserver", text);
    }

    /// The whole conversation, as far as it goes.
    async fn converse(&self, io: &mut SessionIo) {
        let peer = io
            .peer()
            .map(|address| address.to_string())
            .unwrap_or_else(|| "an unknown address".to_string());

        // 1. The version message. Its first four bytes are what the connection was classified with.
        let mut message = vec![0u8; crate::server::protocol::GAME_CMSG_VERSION_LEN];
        if let Err(error) = read_exactly(io, &mut message).await {
            self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer} ended inside its version message: {error}"),
            );
            let _ = io.close().await;
            return;
        }

        let version = match GameVersion::decode(&message) {
            Ok((version, _)) => version,
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer}: {error}; closing it."),
                );
                let _ = io.close().await;
                return;
            }
        };

        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: build {}, world {}, map {}, player token 0x{:08X}.",
                version.version, version.map_token, version.map_id, version.player_token
            ),
        );

        // The world's own id is the check the reference makes through its transfer record: a
        // connection that names another world is not this one's.
        if version.map_token != self.realm {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer} names world {} and this world is {}; closing it.",
                    version.map_token, self.realm
                ),
            );
            let _ = io.close().await;
            return;
        }

        // 2. The seed exchange, the same as the login's.
        let mut seed = vec![0u8; CLIENT_SEED_LEN];
        if let Err(error) = read_exactly(io, &mut seed).await {
            self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer} ended inside its seed: {error}"),
            );
            let _ = io.close().await;
            return;
        }
        let header = u16::from_le_bytes([seed[0], seed[1]]);
        if header != CLIENT_SEED_HEADER {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer} sent a seed with header 0x{header:04X}; the exchange starts with 0x{CLIENT_SEED_HEADER:04X}."
                ),
            );
            let _ = io.close().await;
            return;
        }

        let shared = match handshake::shared_secret(&seed[2..]) {
            Ok(shared) => shared,
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer}: the exchange cannot proceed: {error}"),
                );
                let _ = io.close().await;
                return;
            }
        };

        let master = match self.master {
            Some(master) => master,
            None => match handshake::random_master() {
                Ok(master) => master,
                Err(error) => {
                    self.say(
                        LogLevel::Error,
                        format!(
                            "Gameplay connection from {peer}: {error}; the exchange will use zeros."
                        ),
                    );
                    [0u8; handshake::MASTER_SECRET_LEN]
                }
            },
        };
        let answer = handshake::server_seed(&master, &shared);
        debug_assert_eq!(answer.len(), SERVER_SEED_LEN);
        if let Err(error) = io.write_all(&answer).await {
            self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer} could not be answered: {error}"),
            );
            let _ = io.close().await;
            return;
        }

        let mut crypt = AuthCrypt::new(&handshake::derive_key(&master));
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: the exchange is answered. Everything after it is scrambled."
            ),
        );

        // 3. The travel record. The login wrote one when it answered the client's request for a map
        // (`0x8029`), and this connection presents the same world, account and token; claiming it is
        // what makes the map and the district the login's decision rather than the client's. A
        // connection with no record - the creation screen, or a client that never asked - keeps the
        // map its version message names (`docs/port/06-world-plan.md`, "The three seams").
        let travel = match self.login.as_ref() {
            Some(login) => {
                let account = crate::characters::uuid_text(&version.account);
                let travels = crate::maps::DatabaseTravels::new(
                    std::sync::Arc::clone(login),
                    self.log.clone(),
                );
                match travels.claim(self.realm, &account, version.player_token) {
                    Ok(Some(travel)) => {
                        self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: the travel record is claimed - map {}, district {}, language {}.",
                                travel.map_id, travel.district, travel.language
                            ),
                        );
                        Some(travel)
                    }
                    Ok(None) => {
                        self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: no travel record was claimed; the map {} is the one its own version message names.",
                                version.map_id
                            ),
                        );
                        None
                    }
                    Err(error) => {
                        self.say(
                            LogLevel::Error,
                            format!(
                                "Gameplay connection from {peer}: the travel table could not be read ({error}); the version message's map is used."
                            ),
                        );
                        None
                    }
                }
            }
            None => None,
        };
        let map_id = travel.map(|travel| travel.map_id).unwrap_or(version.map_id);
        // The language the client named, or the world's own when it named none: a travel may carry
        // `-1` (`0xFFFFFFFF`) for "any", which is not a language the client's district record can
        // hold. Measured 2026-11-12: the map request that follows a character click carries exactly
        // that, so an unclamped `-1` reaches the load block as 255.
        let language = travel
            .map(|travel| {
                if travel.language > u32::from(u8::MAX) {
                    0
                } else {
                    travel.language as u8
                }
            })
            .unwrap_or(0);

        // 4. The block the world sends unasked, once the connection is scrambled.
        let Some(map) = self.maps.iter().find(|map| map.id == map_id) else {
            self.say(
                LogLevel::Error,
                format!(
                    "Gameplay connection from {peer} asks for map {map_id}, which this world does not host ({} map(s) in the catalogue); closing it.",
                    self.maps.len()
                ),
            );
            let _ = io.close().await;
            return;
        };

        // A map that is a place is loaded: the head, the player-data start, the character's name and the
        // instance's own record, unasked - the order `docs/evidence/` and ASURA's
        // `reference/instance-initialization.md` measured on a live load, and the rows
        // `shared/src/Packets/../Tables.rs` names.
        if !map.is_creation() {
            // R12: a claimed travel only reaches a map the character has unlocked. A character
            // with no unlock rows at all predates the ledger (or was never written one), so the
            // world's first outpost is unlocked for it here - the same starting unlock a confirm
            // writes - and the rule holds from the first load on.
            if let Some(pool) = self.characters.as_ref() {
                let characters = crate::characters::DatabaseCharacters::new(
                    std::sync::Arc::clone(pool),
                    self.log.clone(),
                );
                let uuid = crate::characters::uuid_text(&version.character);
                match characters.unlocked_maps(&uuid) {
                    Ok(unlocked) if unlocked.contains(&map.id) => {}
                    Ok(unlocked) if unlocked.is_empty() => {
                        let first = self
                            .maps
                            .iter()
                            .filter(|map| !map.is_creation())
                            .map(|map| map.id)
                            .min();
                        match first {
                            Some(first) => {
                                if let Err(error) = characters.unlock_map(&uuid, first) {
                                    self.say(
                                        LogLevel::Warn,
                                        format!(
                                            "Gameplay connection from {peer}: the starting unlock cannot be written: {error}"
                                        ),
                                    );
                                }
                                self.say(
                                    LogLevel::Info,
                                    format!(
                                        "Gameplay connection from {peer}: the character has no unlock rows yet, so map {first} - the world's first outpost - is unlocked for it (R12)."
                                    ),
                                );
                            }
                            None => {
                                self.say(
                                    LogLevel::Warn,
                                    format!(
                                        "Gameplay connection from {peer}: the character has no unlocks and the world hosts no outpost; the connection is closed (R12)."
                                    ),
                                );
                                let _ = io.close().await;
                                return;
                            }
                        }
                    }
                    Ok(unlocked) => {
                        self.say(
                            LogLevel::Warn,
                            format!(
                                "Gameplay connection from {peer} asks for map {} ({}) but the character has not unlocked it (unlocked: {unlocked:?}); the connection is closed, because the refusal's own wire shape is not captured (R12, R58).",
                                map.id, map.name
                            ),
                        );
                        let _ = io.close().await;
                        return;
                    }
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!(
                                "Gameplay connection from {peer}: the character's unlocks cannot be read: {error}"
                            ),
                        );
                        let _ = io.close().await;
                        return;
                    }
                }
            }

            // The district the connection enters: the travel's own number when it named one (R57),
            // otherwise the first with room (R22) - and a named full district is refused rather than
            // moved (R58). The allotment is the world's setting (R5).
            let entered = match self.districts.lock() {
                Ok(mut districts) => districts.enter(
                    map,
                    travel.map(|travel| travel.district).unwrap_or(0),
                    u32::from(language),
                    self.settings.district_allotment,
                ),
                Err(_) => Err("the world's district copies are poisoned".to_string()),
            };
            let district = match entered {
                Ok(district) => district,
                Err(refusal) => {
                    self.say(
                        LogLevel::Warn,
                        format!(
                            "Gameplay connection from {peer}: {refusal}; the connection is closed, because the refusal's own wire shape is not captured (R58)."
                        ),
                    );
                    let _ = io.close().await;
                    return;
                }
            };
            if let Ok(mut entered) = self.entered.lock() {
                *entered = Some((map.id, district));
            }

            // R14: the outpost the character is now in is its last outpost, so a later load - or the
            // login's list - places it there. The row is updated through the noun's own repository.
            if let Some(pool) = self.characters.as_ref() {
                let characters = crate::characters::DatabaseCharacters::new(
                    std::sync::Arc::clone(pool),
                    self.log.clone(),
                );
                if let Err(error) = characters.set_last_outpost(
                    &crate::characters::uuid_text(&version.character),
                    map.id as u16,
                ) {
                    self.say(
                        LogLevel::Warn,
                        format!(
                            "Gameplay connection from {peer}: the character's last outpost cannot be recorded: {error}"
                        ),
                    );
                }
            }

            let sheet = self.character_sheet(&version);
            let is_explorable = u8::from(map.kind == crate::world::map_kind::EXPLORABLE);
            let frames = vec![
                instance_packets::instance_load_head(),
                instance_packets::instance_player_data_start(),
                instance_packets::instance_load_player_name(&sheet.name),
                instance_packets::instance_load_info(
                    CREATION_AGENT,
                    map.id as u16,
                    is_explorable,
                    district,
                    language,
                    0,
                ),
            ];
            let mut block = Vec::new();
            for frame in &frames {
                block.extend_from_slice(frame);
            }
            crypt.encrypt(&mut block);
            match io.write_all(&block).await {
                Ok(()) => self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: map {} ({}, type {}) is loading - {} message(s), {} bytes: the load head, the player-data start, the name '{}' and the instance's record (district {district}, language {language}).",
                        map.id, map.name, map.kind, frames.len(), block.len(), sheet.name
                    ),
                ),
                Err(error) => {
                    self.say(
                        LogLevel::Warn,
                        format!("Gameplay connection from {peer} could not be sent its load block: {error}"),
                    );
                    let _ = io.close().await;
                    return;
                }
            }

            // What the client asks next, and what a load answers: the items, where it stands, and
            // then its own body and the end of the load.
            self.load_map(io, &peer, map, &sheet, &mut crypt).await;
            return;
        }

        let mut block = creation_load_block();
        crypt.encrypt(&mut block);
        match io.write_all(&block).await {
            Ok(()) => self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: the character-creation instance is opened, {} bytes (load head and creation start).",
                    block.len()
                ),
            ),
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer} could not be sent its load block: {error}"),
                );
                let _ = io.close().await;
                return;
            }
        }

        // From here the client asks what it needs for the creation screen, and what it asks for is
        // answered in the reference's order: the player-data block, the PvP half, a profession
        // change, and the confirm.
        let mut creation = Creation::start();
        let mut pending: Vec<u8> = Vec::new();
        let mut buffer = vec![0u8; 1024];
        // A message that never finishes framing is otherwise invisible: the client sends one, this
        // world waits for bytes that are not coming, and all the client can say is its own connection
        // error. It happens when a header's row disagrees with the client's own packing, so a wait
        // longer than a client tolerates is said out loud, once.
        let mut stalled: Option<(u16, usize, std::time::Instant)> = None;
        loop {
            // Everything already unscrambled and not yet answered, before any more is read.
            while let Some(bytes) = pending.first_chunk::<2>() {
                let header = u16::from_le_bytes(*bytes);
                // The table decides where the message ends: a counted field carries its own count, so
                // the walk is exact for every header the protocol has, not just the ones answered here.
                let used = match crate::server::protocol::decode_game_message(&pending, true) {
                    Ok((frame, used)) => {
                        self.log.out(
                            LogLevel::Debug,
                            "network.trace",
                            format!(
                                "trace {peer} game in  {} bytes, {}",
                                used,
                                crate::server::protocol::label(frame.header())
                            ),
                        );
                        used
                    }
                    Err(FrameError::Incomplete { needed }) => {
                        let now = std::time::Instant::now();
                        let held = pending.len();
                        match stalled {
                            Some((seen, seen_len, since))
                                if seen == header && seen_len == held =>
                            {
                                if now.duration_since(since) >= std::time::Duration::from_millis(900)
                                {
                                    self.say(
                                        LogLevel::Warn,
                                        format!(
                                            "Gameplay connection from {peer}: a message with header 0x{header:04X} has been waiting {held} byte(s) for {} ms, and the row needs at least {needed}; the client is not sending the rest. The bytes so far are {}.",
                                            now.duration_since(since).as_millis(),
                                            shared::network::socket::hex(&pending[..pending.len().min(64)])
                                        ),
                                    );
                                    stalled = None;
                                }
                            }
                            _ => stalled = Some((header, held, now)),
                        }
                        break;
                    }
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!(
                                "Gameplay connection from {peer}: a message with header 0x{header:04X} cannot be framed ({error}), so the stream cannot be split any further. The bytes so far are {}.",
                                shared::network::socket::hex(&pending[..pending.len().min(64)])
                            ),
                        );
                        let _ = io.close().await;
                        return;
                    }
                };
                let message: Vec<u8> = pending.drain(..used).collect();

                self.log.out(
                    LogLevel::Debug,
                    "network.trace",
                    format!(
                        "trace {peer} game in  {} bytes: {}",
                        message.len(),
                        shared::network::socket::hex(&message[..message.len().min(64)])
                    ),
                );

                match self
                    .answer(io, &peer, header, &message, &mut creation, &version, &mut crypt)
                    .await
                {
                    Some(true) => {}
                    Some(false) => return,
                    None => {
                        // A message this world does not answer: say so, and keep the connection.
                        self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: message 0x{header:04X} with {} bytes, which this world does not answer yet.",
                                message.len()
                            ),
                        );
                    }
                }
            }

            let read = match io.read(&mut buffer).await {
                Ok(0) => {
                    self.say(
                        LogLevel::Info,
                        format!("Gameplay connection from {peer} closed."),
                    );
                    return;
                }
                Ok(read) => read,
                Err(error) => {
                    self.say(
                        LogLevel::Warn,
                        format!("Gameplay connection from {peer} could not be read: {error}"),
                    );
                    return;
                }
            };

            let mut plain = buffer[..read].to_vec();
            crypt.decrypt(&mut plain);
            pending.extend_from_slice(&plain);
        }
    }

    /// Answer one gameplay message.
    ///
    /// `Some(true)` keeps the connection, `Some(false)` ends it, `None` is a message this world has
    /// no behaviour for - the caller logs its header and size, which is what says what is missing.
    #[allow(clippy::too_many_arguments)]
    async fn answer(
        &self,
        io: &mut SessionIo,
        peer: &str,
        header: u16,
        message: &[u8],
        creation: &mut Creation,
        version: &GameVersion,
        crypt: &mut AuthCrypt,
    ) -> Option<bool> {
        match header {
            // The creation screen's own asks, in the order a working server answers them.
            CHAR_CREATION_REQUEST_PLAYER => {
                // The body is the two bytes of the client's current choice, which is a campaign and
                // a profession; the reference reads them and re-draws from that choice.
                if message.len() >= 4 {
                    creation.campaign = message[2];
                    creation.profession = message[3];
                }
                let frames = creation_player_block(self.creation.as_ref(), creation);
                self.send(io, peer, crypt, &frames, "the creation screen's player data")
                    .await;
                Some(true)
            }
            CHAR_CREATION_REQUEST_ARMORS => {
                let frames = vec![
                    misc_packets::pvp_update_unlocked_skills(&[]),
                    misc_packets::pvp_update_unlocked_heroes(&[]),
                    misc_packets::pvp_item_stream_end(),
                ];
                self.send(io, peer, crypt, &frames, "the creation screen's PvP half")
                    .await;
                Some(true)
            }
            CHAR_CREATION_CHANGE_PROF => {
                let previous = *creation;
                if message.len() >= 4 {
                    creation.campaign = message[2];
                    creation.profession = message[3];
                }
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: the creation screen chose campaign {} and profession {}.",
                        creation.campaign, creation.profession
                    ),
                );
                // The reference's own answer (`GameSrv_HandleCharCreationChangeProf`), in its order:
                // the previous set comes off the client first, then the profession, then the new set
                // is created and placed. A client still holding the old set asserts on an item created
                // into a slot it thinks is occupied (`Assertion: !bagParent->GetItem(slot)`,
                // `ItCliInv.cpp(105)`, which `ASURA/crates/asura-world/src/creation.rs` records
                // against build 39112 when it omitted the removal).
                let mut frames = Vec::new();
                let tables = self.creation.clone().unwrap_or_default();
                for item in tables
                    .inventory(previous.campaign, previous.profession)
                    .equipped_items()
                {
                    frames.push(item_packets::item_remove(item_packets::ITEM_STREAM, item.id));
                }
                frames.push(character_packets::update_unlocked_skills(&[]));
                frames.push(character_packets::player_update_profession(
                    CREATION_AGENT,
                    creation.profession,
                    0,
                    0,
                ));
                let equipped_bag =
                    crate::creation::equipped_bag(&tables).map(|bag| bag.id).unwrap_or(0);
                for item in tables
                    .inventory(creation.campaign, creation.profession)
                    .equipped_items()
                {
                    let slot = item.slot;
                    frames.push(item_packets::create_named_item(
                        &crate::creation::named_item(item),
                    ));
                    if let Some(slot) = slot {
                        frames.push(item_packets::item_move_to_location(
                            item_packets::ITEM_STREAM,
                            item.id,
                            equipped_bag,
                            slot,
                        ));
                    }
                }
                self.send(io, peer, crypt, &frames, "the new profession")
                    .await;
                Some(true)
            }
            CHAR_CREATION_CONFIRM => {
                // The confirm carries the name as a counted string and then the packed appearance
                // word; the reference reads exactly those and writes the rows.
                self.confirm_creation(io, peer, message, creation, version, crypt)
                    .await;
                Some(true)
            }
            // A ping on the gameplay connection: two bytes, answered with the same two bytes.
            GAME_PING => {
                let mut answer = message.to_vec();
                crypt.encrypt(&mut answer);
                match io.write_all(&answer).await {
                    Ok(()) => Some(true),
                    Err(_) => Some(false),
                }
            }
            _ => None,
        }
    }

    /// What a load needs from the character's row: the name it shows, and the campaign and profession
    /// whose starting set the item stream carries.
    ///
    /// A connection whose row cannot be read is loaded as a fresh Prophecies Warrior named "Player":
    /// a load gives the client a body either way, and the row is what makes the name and the set its
    /// own.
    fn character_sheet(&self, version: &crate::server::protocol::GameVersion) -> CharacterSheet {
        let fallback = CharacterSheet {
            name: "Player".to_string(),
            campaign: 0,
            profession: character_packets::DEFAULT_PROFESSION,
            uuid: version.character,
            gold: 0,
            appearance: 0,
            level: 1,
            unlocked_professions: character_packets::CREATION_UNLOCKED_PROFESSIONS,
        };
        let Some(pool) = self.characters.as_ref() else {
            return fallback;
        };
        let uuid = crate::characters::uuid_text(&version.character);
        let reply = match pool.lock() {
            Ok(pool) => pool.query_reply(
                "SELECT \"name\", \"campaign\", \"primary_profession\", \"gold\"::bigint, \
                 \"appearance\"::bigint, \"level\"::bigint, \"unlocked_professions\"::bigint \
                 FROM \"characters\" WHERE \"uuid\" = $1",
                vec![Param::Text(uuid)],
            ),
            Err(_) => return fallback,
        };
        let rows = match reply.recv() {
            Ok(Ok(rows)) => rows,
            Ok(Err(error)) => {
                self.say(
                    LogLevel::Error,
                    format!("the character's row cannot be read: {error}"),
                );
                return fallback;
            }
            Err(_) => {
                self.say(
                    LogLevel::Error,
                    "the character database answered no query: it is shutting down",
                );
                return fallback;
            }
        };
        rows.first()
            .map(|row| CharacterSheet {
                name: row.get::<usize, String>(0),
                campaign: row.get::<usize, i64>(1) as u8,
                profession: row.get::<usize, i64>(2) as u8,
                uuid: version.character,
                gold: row.get::<usize, i64>(3) as u32,
                appearance: row.get::<usize, i64>(4) as u32,
                level: row.get::<usize, i64>(5) as u8,
                unlocked_professions: row.get::<usize, i64>(6) as u32,
            })
            .unwrap_or(fallback)
    }

    /// The rest of a map's load: the ping pair, the inventory and the map's record, where the
    /// character stands, its own body, and the end of the load.
    ///
    /// The client asks and the world answers, in the order a working server measured
    /// (`recordings/gwps/world/load-stream.log`): `0x8091` wants the items and is answered with the
    /// inventory stream and the map's record, `0x8088` wants the entry point and is answered with a
    /// point of this map's own list (scattered per R13) and the first tick, and `0x8090` wants the
    /// players and ends the load. What is still missing is named here rather than implied: the
    /// agent stream of step 6 and the party stream of step 7 are not sent, and the PvP halves carry
    /// the empty values a fresh character has.
    async fn load_map(
        &self,
        io: &mut SessionIo,
        peer: &str,
        map: &crate::world::MapEntry,
        sheet: &CharacterSheet,
        crypt: &mut AuthCrypt,
    ) {
        // The unasked pair the measured load follows the head block with.
        self.send(
            io,
            peer,
            crypt,
            &[misc_packets::ping_request(), misc_packets::ping_reply(0x14)],
            "the ping pair",
        )
        .await;

        let mut pending: Vec<u8> = Vec::new();
        let mut buffer = vec![0u8; 1024];
        let inventory = self.load_inventory(sheet);
        let mut tick: u32 = 500;
        let mut loaded = false;
        // Where the character stands, chosen once: the entry-position answer and the agent the load
        // creates have to be the same place.
        let entry = self.entry_point(map);
        let load_started = std::time::Instant::now();

        loop {
            while let Some(bytes) = pending.first_chunk::<2>() {
                let header = u16::from_le_bytes(*bytes);
                let used = match crate::server::protocol::decode_game_message(&pending, true) {
                    Ok((_, used)) => used,
                    Err(FrameError::Incomplete { .. }) => break,
                    Err(error) => {
                        self.say(
                            LogLevel::Warn,
                            format!(
                                "Gameplay connection from {peer}: a message could not be framed ({error}); the load stops here."
                            ),
                        );
                        let _ = io.close().await;
                        return;
                    }
                };
                let message: Vec<u8> = pending.drain(..used).collect();

                match header {
                    // The items ask: the inventory stream and the map's record, measured in one
                    // block. The manifest pair that ends it names the map in its second form.
                    instance_packets::CLIENT_LOAD_REQUEST_ITEMS => {
                        let mut frames = crate::creation::load_item_stream(&inventory, sheet.gold);
                        frames.push(misc_packets::hard_mode_unlocked(1));
                        frames.push(character_packets::character_update_info());
                        frames.push(misc_packets::map_update_current(map.id as u16, 0));
                        frames.push(instance_packets::ready_for_map_spawn(0));
                        frames.push(instance_packets::instance_manifest_phase(0));
                        frames.push(instance_packets::instance_manifest_phase(1));
                        frames.push(instance_packets::instance_manifest_done(2, 877, 0));
                        frames.push(instance_packets::instance_manifest_phase(0));
                        frames.push(instance_packets::instance_manifest_phase(1));
                        frames.push(instance_packets::instance_manifest_done(
                            0,
                            map.id as u16,
                            0,
                        ));
                        self.send(io, peer, crypt, &frames, "the inventory and the map's record")
                            .await;
                    }
                    // The spawn request: a random point of this map's own list, scattered (R13),
                    // and the first simulation tick.
                    instance_packets::CLIENT_LOAD_REQUEST_SPAWN => {
                        let point = entry;
                        let mut frames = match point {
                            Some(point) => {
                                self.say(
                                    LogLevel::Info,
                                    format!(
                                        "Gameplay connection from {peer}: the character stands at {:.2}, {:.2} (plane {}) on map {}.",
                                        point.0, point.1, point.2, map.id
                                    ),
                                );
                                if let Ok(mut agent) = self.agent.lock() {
                                    *agent = Some(crate::agents::Agent::at(
                                        CREATION_AGENT,
                                        point.0,
                                        point.1,
                                    ));
                                }
                                vec![instance_packets::instance_load_spawn_point(
                                    map.file_id,
                                    point.0,
                                    point.1,
                                    point.2,
                                )]
                            }
                            None => {
                                self.say(
                                    LogLevel::Warn,
                                    format!(
                                        "Gameplay connection from {peer}: map {} has no spawn point, so the client is told nothing.",
                                        map.id
                                    ),
                                );
                                Vec::new()
                            }
                        };
                        frames.push(misc_packets::world_simulation_tick(500));
                        self.send(io, peer, crypt, &frames, "the entry position").await;
                    }
                    // The players request: the whole block the reference answers it with, frame for
                    // frame and in its own order (`GameSrv_HandleInstanceLoadRequestPlayers`, ported
                    // from `ASURA/crates/asura-world/src/player.rs`, `player_block`). It is one block,
                    // not a menu: the PvP unlocks and the unlockable-item table, the account features,
                    // the unlocked lists and the team token, the skills and the progress record, the
                    // visibility and player flags, the player and the agent it is drawn as, the
                    // agent's own properties and look, the party, and the message that ends the load.
                    instance_packets::CLIENT_LOAD_REQUEST_PLAYERS => {
                        let agent = CREATION_AGENT;
                        let player = CREATION_AGENT;
                        let position = entry
                            .map(|point| (point.0, point.1))
                            .unwrap_or((0.0, 0.0));
                        let plane = entry.map(|point| point.2).unwrap_or(0);
                        // The look the client builds the model from: the character's own bits, or the
                        // world's default when its row carries none (an all-zero word is
                        // `Profession_None`, which the client cannot draw).
                        let appearance = if sheet.appearance == 0 {
                            self.settings.appearance
                        } else {
                            sheet.appearance
                        };
                        // The item ids the agent is drawn wearing, from the equipped bag, in its own
                        // order. A slot the character wears nothing in is zero.
                        let mut equipped = [0u32; 9];
                        if let Some(bag) = self
                            .creation
                            .as_ref()
                            .and_then(crate::creation::equipped_bag)
                        {
                            for item in inventory.items_in(bag.id) {
                                if let Some(slot) = item.slot
                                    && let Some(held) = equipped.get_mut(slot as usize)
                                {
                                    *held = item.id;
                                }
                            }
                        }
                        // The instances the character has reached, as the client addresses them: bit
                        // `n % 32` of word `n / 32`.
                        let mut unlocked = UNLOCKED_MAP_ZEROS.to_vec();
                        if let Some(pool) = self.characters.as_ref() {
                            let characters = crate::characters::DatabaseCharacters::new(
                                std::sync::Arc::clone(pool),
                                self.log.clone(),
                            );
                            match characters
                                .unlocked_maps(&crate::characters::uuid_text(&sheet.uuid))
                            {
                                Ok(maps) => {
                                    for map_id in maps {
                                        let word = (map_id / 32) as usize;
                                        if let Some(slot) = unlocked.get_mut(word) {
                                            *slot |= 1 << (map_id % 32);
                                        }
                                    }
                                }
                                Err(error) => self.say(
                                    LogLevel::Warn,
                                    format!(
                                        "Gameplay connection from {peer}: the character's reached instances cannot be read ({error}); the list is sent with only what this load knows."
                                    ),
                                ),
                            }
                        }
                        let mut progress = [0u32; character_packets::FACTIONS_DWORDS];
                        // Experience has no column yet; level and morale do, and the client shows 0
                        // for a level it is not told (`ASURA/crates/asura-world/src/player.rs`).
                        progress[9] = u32::from(sheet.level);
                        progress[10] = MORALE_FULL;
                        let load_ms = u32::try_from(load_started.elapsed().as_millis())
                            .unwrap_or(u32::MAX);

                        let mut frames = Vec::with_capacity(40);
                        frames.push(misc_packets::pvp_update_unlocked_skills(&[]));
                        frames.push(misc_packets::pvp_update_unlocked_heroes(&[0xFFFF_FFFF; 8]));
                        for item in crate::data_stores::pvp_items::PVP_ITEMS {
                            frames.push(misc_packets::pvp_item_stream_add(item.id, item.params));
                        }
                        frames.push(misc_packets::pvp_item_stream_end());
                        for (feature, param1, param2) in [
                            (1u16, 10u16, 0u16),
                            (99, 3, 0),
                            (100, 5, 0),
                            (101, 5, 5),
                            (102, 2, 2),
                            (111, 1, 0),
                            (124, 1, 0),
                            (125, 1, 0),
                            (131, 1, 0),
                        ] {
                            frames.push(misc_packets::account_feature(feature, param1, param2));
                        }
                        frames.push(misc_packets::map_update_unlocked_list([
                            &[],
                            &[],
                            UNLOCKED_MAP_ZEROS,
                            UNLOCKED_MAP_ZEROS,
                            &unlocked,
                        ]));
                        frames.push(misc_packets::instance_loaded(PLAYER_TEAM_TOKEN));
                        frames.push(agent_packets::agent_update_attribute_points(agent, 50, 50));
                        frames.push(character_packets::player_update_profession(
                            agent,
                            sheet.profession,
                            0,
                            0,
                        ));
                        frames.push(character_packets::player_update_unlocked_professions(
                            agent,
                            sheet.unlocked_professions,
                        ));
                        frames.push(character_packets::skillbar_update(
                            agent,
                            &[0u32; 8],
                            &[0u32; 8],
                            1,
                        ));
                        frames.push(character_packets::update_unlocked_skills(&[]));
                        frames.push(character_packets::character_update_factions(&progress));
                        frames.push(misc_packets::char_visibility_flags(0xFFFF_FFFF, 0xFFFF_FFFF));
                        frames.push(misc_packets::player_update_flags(player as u16, 0, 7));
                        frames.push(agent_packets::agent_update_attributes(agent, &[0u32; 42]));
                        frames.push(misc_packets::world_update_load_time(load_ms));
                        frames.push(instance_packets::player_create(
                            player,
                            agent,
                            appearance,
                            &sheet.name,
                        ));
                        frames.push(character_packets::player_update_profession(
                            agent,
                            sheet.profession,
                            0,
                            0,
                        ));
                        frames.push(agent_packets::world_create_agent(
                            &agent_packets::CreateAgent {
                                agent_id: agent,
                                model_id: agent_packets::CHAR_CLASS_PLAYER_BASE | player,
                                agent_type: agent_packets::AGENT_TYPE_LIVING,
                                position,
                                plane,
                                // Our spawn rows carry no facing yet: the reference's own create
                                // message carries a unit vector (measured: -0.995, -0.104 on one
                                // body), and an agent with no direction is not a thing the client
                                // draws, so the neutral +y stands in until the angle is a column.
                                direction: (0.0, 1.0),
                                speed_base: self.settings.agent_speed,
                                player_team_token: PLAYER_TEAM_TOKEN,
                            },
                        ));
                        frames.push(agent_packets::agent_update_morale(agent, MORALE_FULL));
                        frames.push(agent_packets::agent_property_update_int(
                            agent_packets::AGENT_PROPERTY_ENERGY,
                            agent,
                            self.settings.agent_energy,
                        ));
                        frames.push(agent_packets::agent_property_update_int(
                            agent_packets::AGENT_PROPERTY_HEALTH,
                            agent,
                            self.settings.agent_health,
                        ));
                        frames.push(agent_packets::agent_property_update_float(
                            agent_packets::AGENT_PROPERTY_ENERGY_REGEN,
                            agent,
                            self.settings.agent_energy_regen,
                        ));
                        frames.push(agent_packets::agent_property_update_int(
                            agent_packets::AGENT_PROPERTY_PUBLIC_LEVEL,
                            agent,
                            u32::from(sheet.level),
                        ));
                        frames.push(agent_packets::agent_initial_effects(agent, 0));
                        frames.push(misc_packets::update_agent_visual_equipment(agent, equipped));
                        frames.push(misc_packets::agent_display_cape(agent, 1));
                        frames.push(agent_packets::world_update_controlled_agent(agent, 3));
                        frames.push(party_packets::party_create(party_packets::PARTY_ID));
                        frames.push(party_packets::party_player_add(
                            party_packets::PARTY_ID,
                            player as u16,
                            1,
                        ));
                        frames.push(party_packets::party_member_stream_end(party_packets::PARTY_ID));
                        frames.push(character_packets::character_update_party(
                            party_packets::PARTY_ID,
                            1,
                        ));
                        frames.push(instance_packets::instance_load_finish());
                        let count = frames.len();
                        self.send(io, peer, crypt, &frames, "the players and the load's last message")
                            .await;
                        self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: map {} is loaded - the character '{}' is in it ({count} message(s)).",
                                map.id, sheet.name
                            ),
                        );
                        loaded = true;
                    }
                    // A drag in the inventory: where the item goes is a row, and the client is told
                    // the same placement back. `0x8072` is the client's own `9` bytes - the item, the
                    // bag and the slot - and `0x013E` is the placement the client already knows.
                    CLIENT_ITEM_MOVE => {
                        match item_move(&message) {
                            Some((item_id, bag_id, slot)) => {
                                let characters = self.characters.as_ref().map(|pool| {
                                    crate::characters::DatabaseCharacters::new(
                                        std::sync::Arc::clone(pool),
                                        self.log.clone(),
                                    )
                                });
                                let stored = match characters {
                                    Some(characters) => characters.move_item(
                                        &crate::characters::uuid_text(&sheet.uuid),
                                        item_id,
                                        bag_id,
                                        slot,
                                    ),
                                    None => Ok(false),
                                };
                                match stored {
                                    Ok(true) => {
                                        let frame = item_packets::item_move_to_location(
                                            item_packets::ITEM_STREAM,
                                            item_id,
                                            bag_id,
                                            slot,
                                        );
                                        self.send(io, peer, crypt, &[frame], "the item's new place")
                                            .await;
                                    }
                                    Ok(false) => self.say(
                                        LogLevel::Info,
                                        format!(
                                            "Gameplay connection from {peer}: item {item_id} is not the character's, so its move to bag {bag_id} slot {slot} is refused."
                                        ),
                                    ),
                                    Err(error) => self.say(
                                        LogLevel::Warn,
                                        format!(
                                            "Gameplay connection from {peer}: the item move could not be written: {error}"
                                        ),
                                    ),
                                }
                            }
                            None => self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: an item move of {} bytes cannot be read.",
                                    message.len()
                                ),
                            ),
                        }
                    }
                    // The client's own ping on a live connection: the reference logs it and
                    // answers nothing, so this does the same.
                    GAME_PING => {
                        self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: a ping on the loaded map, {message_len} bytes.",
                                message_len = message.len()
                            ),
                        );
                    }
                    // Where the client says it went. The client has already moved itself; the world
                    // keeps the point, which is the player's own agent's state.
                    CLIENT_MOVE_TO_COORD_WITH_DIR
                    | CLIENT_MOVE_TO_COORD
                    | CLIENT_LAST_POS_BEFORE_MOVE_CANCELED => {
                        if let Some((x, y)) = movement_point(&message) {
                            if let Ok(mut agent) = self.agent.lock()
                                && let Some(agent) = agent.as_mut()
                            {
                                agent.moved_to(x, y);
                            }
                            self.log.out(
                                LogLevel::Debug,
                                "network.trace",
                                format!(
                                    "trace {peer} game in  a movement: {x:.2}, {y:.2}"
                                ),
                            );
                        }
                    }
                    // The client's own in-game travel request. The redirect that answers it is not
                    // captured on this build, so the connection stays where it is and the request is
                    // named in the log rather than acted on (`docs/port/06-world-plan.md`, step 3).
                    CLIENT_PARTY_TRAVEL => match client_travel(&message) {
                        Some((map_id, territory, district, language, flag)) => self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: the client asks to travel to map {map_id} (territory {territory}, district {district}, language {language}, flag {flag}); the redirect's own wire is not captured yet, so the connection stays where it is."
                            ),
                        ),
                        None => self.say(
                            LogLevel::Warn,
                            format!(
                                "Gameplay connection from {peer}: a travel request of {} bytes cannot be read.",
                                message.len()
                            ),
                        ),
                    },
                    // The client destroys one of its items: the row goes, and the item is told gone
                    // with `0x014D`.
                    CLIENT_ITEM_DESTROY => {
                        match message
                            .get(2..6)
                            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                        {
                            Some(item_id) => {
                                let removed = self.characters.as_ref().map(|pool| {
                                    crate::characters::DatabaseCharacters::new(
                                        std::sync::Arc::clone(pool),
                                        self.log.clone(),
                                    )
                                    .remove_item(&crate::characters::uuid_text(&sheet.uuid), item_id)
                                });
                                match removed {
                                    Some(Ok(true)) => {
                                        let frame = item_packets::item_remove(
                                            item_packets::ITEM_STREAM,
                                            item_id,
                                        );
                                        self.send(io, peer, crypt, &[frame], "the item's removal")
                                            .await;
                                    }
                                    Some(Ok(false)) => self.say(
                                        LogLevel::Info,
                                        format!(
                                            "Gameplay connection from {peer}: item {item_id} is not the character's, so it is not destroyed."
                                        ),
                                    ),
                                    Some(Err(error)) => self.say(
                                        LogLevel::Warn,
                                        format!(
                                            "Gameplay connection from {peer}: the item could not be destroyed: {error}"
                                        ),
                                    ),
                                    None => self.say(
                                        LogLevel::Warn,
                                        format!(
                                            "Gameplay connection from {peer}: a destroy was asked for, and this world has no character database."
                                        ),
                                    ),
                                }
                            }
                            None => self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: a destroy of {} bytes cannot be read.",
                                    message.len()
                                ),
                            ),
                        }
                    }
                    // The client wears one of its items: where it goes is the slot its kind is worn
                    // in, in the bag the setting says is equipped, and the client is told the place.
                    CLIENT_EQUIP_ITEM => {
                        let Some(item_id) = message
                            .get(2..6)
                            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                        else {
                            self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: an equip of {} bytes cannot be read.",
                                    message.len()
                                ),
                            );
                            continue;
                        };
                        let Some(characters) = self.characters.as_ref().map(|pool| {
                            crate::characters::DatabaseCharacters::new(
                                std::sync::Arc::clone(pool),
                                self.log.clone(),
                            )
                        }) else {
                            self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: an equip was asked for, and this world has no character database."
                                ),
                            );
                            continue;
                        };
                        let Some((bag_id, slot)) = self.equip_place(&characters, sheet, item_id)
                        else {
                            continue;
                        };
                        let uuid = crate::characters::uuid_text(&sheet.uuid);
                        match characters.move_item(&uuid, item_id, bag_id, slot) {
                            Ok(true) => {
                                let frame = item_packets::item_move_to_location(
                                    item_packets::ITEM_STREAM,
                                    item_id,
                                    bag_id,
                                    slot,
                                );
                                self.send(io, peer, crypt, &[frame], "the item's new place")
                                    .await;
                            }
                            Ok(false) => self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: item {item_id} could not be moved to bag {bag_id} slot {slot}."
                                ),
                            ),
                            Err(error) => self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: the equip could not be written: {error}"
                                ),
                            ),
                        }
                    }
                    other => {
                        self.log.out(
                            LogLevel::Info,
                            "server.worldserver",
                            format!(
                                "Gameplay connection from {peer}: message {} ({} bytes) during the load, which this world does not answer yet.",
                                crate::server::protocol::label(other),
                                message.len()
                            ),
                        );
                    }
                }
            }

            // The simulation's tick (R33): while the map is loaded, one every
            // `World.SimulationTickMs` milliseconds (105 by default, the reference's measured cadence).
            let read = io
                .read_timeout(
                    &mut buffer,
                    std::time::Duration::from_millis(u64::from(self.settings.simulation_tick_ms)),
                )
                .await;
            match read {
                Ok(shared::network::socket::ReadOutcome::TimedOut) => {
                    if loaded {
                        tick = tick.wrapping_add(1);
                        let frame = misc_packets::world_simulation_tick(tick);
                        self.send(io, peer, crypt, &[frame], "a tick").await;
                    }
                }
                Ok(shared::network::socket::ReadOutcome::Closed) => return,
                Ok(shared::network::socket::ReadOutcome::Read(read)) => {
                    // The asks after the seed are scrambled with the key both sides derive, the
                    // same as in the run loop above.
                    let mut plain = buffer[..read].to_vec();
                    crypt.decrypt(&mut plain);
                    pending.extend_from_slice(&plain);
                }
                Err(error) => {
                    self.say(
                        LogLevel::Warn,
                        format!("Gameplay connection from {peer} could not be read: {error}"),
                    );
                    return;
                }
            }
        }
    }

    /// One point of a map's own spawn list, scattered by the world's setting (R13).
    ///
    /// The game's rule: every entry takes a random point of that map's list, lands up to `spawn_scatter`
    /// units around it, and faces the point's angle. The scatter is a setting, never a constant.
    fn entry_point(&self, map: &crate::world::MapEntry) -> Option<(f32, f32, u16)> {
        let points: Vec<crate::maps::MapSpawn> = self
            .spawns
            .iter()
            .filter(|spawn| spawn.map_id == map.id)
            .copied()
            .collect();
        if points.is_empty() {
            return None;
        }
        let mut pick = [0u8; 8];
        let choice = if getrandom::getrandom(&mut pick).is_ok() {
            (u64::from_le_bytes(pick) % points.len() as u64) as usize
        } else {
            0
        };
        let point = points[choice];
        let scatter = self.settings.spawn_scatter;
        let mut angle = [0u8; 8];
        let (dx, dy) = if getrandom::getrandom(&mut angle).is_ok() {
            let raw = u64::from_le_bytes(angle);
            let radians = (raw % 360) as f32 * std::f32::consts::PI / 180.0;
            let radius = ((raw / 360) % scatter.max(1.0) as u64) as f32;
            (radius * radians.cos(), radius * radians.sin())
        } else {
            (0.0, 0.0)
        };
        Some((point.x + dx, point.y + dy, point.plane.max(0) as u16))
    }

    /// The creation screen's confirm: write the character, and say whether it was written.
    ///
    /// Reference: `GameSrv_HandleCharCreationConfirm` - read the name and the packed appearance, write
    /// the rows, answer `0x0188` with the character's id, its name, the reference's own `0x2211` and
    /// the settings the list will carry; a name that is taken is answered with `0x018B` and the
    /// taken-name code. The row is this tree's own (`docs/decisions/0007-character-schema.md`), and
    /// the creation defaults are the reference's: level 1, the helmet shown, the unlocked-profession
    /// bit set to the profession that was chosen.
    ///
    /// The confirm's layout is the reference's message table: the `u16` header, a counted name whose
    /// field holds up to twenty units, then the packed appearance word (`ASURA`,
    /// `crates/asura-protocol/src/packets.rs`, the `0x808B` row: header, `String16` of twenty, then
    /// the body's dword).
    async fn confirm_creation(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        creation: &Creation,
        version: &GameVersion,
        crypt: &mut AuthCrypt,
    ) {
        let Some(count) = message.get(2..4).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize) else {
            return;
        };
        let name_at = 4;
        let end = name_at + count * 2;
        if message.len() < end + 4 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a creation confirm is {} bytes and its name needs {}; it cannot be read.",
                    message.len(),
                    end + 4
                ),
            );
            let _ = io.close().await;
            return;
        }
        let units: Vec<u16> = (0..count)
            .map(|index| {
                let at = name_at + index * 2;
                u16::from_le_bytes([message[at], message[at + 1]])
            })
            .collect();
        let name = String::from_utf16_lossy(&units);
        let appearance = u32::from_le_bytes([
            message[end],
            message[end + 1],
            message[end + 2],
            message[end + 3],
        ]);

        let Some(pool) = self.characters.as_ref() else {
            self.say(
                LogLevel::Error,
                format!(
                    "Gameplay connection from {peer}: a character was confirmed, but this world has no character database; it cannot be written."
                ),
            );
            let refusal = character_packets::char_creation_error(
                character_packets::GM_ERROR_CHARACTER_NAME_ALREADY_EXIST,
            );
            let _ = self.send(io, peer, crypt, &[refusal], "the refusal").await;
            return;
        };

        // A fresh identifier for the character: sixteen bytes the client will use from now on.
        let mut id = [0u8; 16];
        if getrandom::getrandom(&mut id).is_err() {
            self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: no entropy for a character id; using zero."),
            );
        }
        let account = crate::characters::uuid_text(&version.account);
        let unlocked = 1i64 << (creation.profession & 0x1F);

        // Where the character starts, which is not where it was made: the first outpost this world
        // hosts. `GameSrv_HandleCharCreationConfirm` writes the character's starting map into its own
        // row, and the login's list reads it back, so a row without one is a character the client can
        // only be sent back into the creation instance for.
        let starting_outpost = self
            .maps
            .iter()
            .filter(|map| map.kind == crate::world::map_kind::OUTPOST)
            .min_by_key(|map| map.id);
        let start_map = starting_outpost.map(|map| map.id as u16).unwrap_or(0);

        let reply = match pool.lock() {
            Ok(pool) => pool.query_reply(
                INSERT_CHARACTER,
                vec![
                    Param::Text(crate::characters::uuid_text(&id)),
                    Param::Text(account),
                    Param::Text(name.clone()),
                    Param::Integer(i64::from(appearance)),
                    Param::Integer(i64::from(creation.profession)),
                    Param::Integer(0), // secondary profession: the reference's creation default
                    Param::Integer(unlocked),
                    Param::Integer(i64::from(creation.campaign)),
                    Param::Integer(1), // level 1
                    Param::Flag(creation.campaign == 0),
                    Param::Integer(1), // the helmet is shown
                    Param::Integer(0), // gold: the reference's default for a new character
                    Param::Integer(i64::from(start_map)), // the outpost it starts in
                    Param::Integer(0), // experience
                ],
            ),
            Err(_) => {
                self.say(
                    LogLevel::Error,
                    format!("Gameplay connection from {peer}: the character database pool is poisoned."),
                );
                return;
            }
        };

        match reply.recv() {
            Ok(Ok(rows)) if rows.is_empty() => {
                // `ON CONFLICT (name) DO NOTHING` wrote nothing: the name is taken.
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: the name '{name}' is taken; the creation screen is told so."
                    ),
                );
                let refusal = character_packets::char_creation_error(
                    character_packets::GM_ERROR_CHARACTER_NAME_ALREADY_EXIST,
                );
                let _ = self.send(io, peer, crypt, &[refusal], "the refusal").await;
            }
            Ok(Ok(_)) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: the character '{name}' was created as {} (profession {}, campaign {}).",
                        crate::characters::uuid_text(&id),
                        creation.profession,
                        creation.campaign
                    ),
                );

                // The character's starting items are its own rows now: what a load draws from. The
                // set comes from the creation record list, which is a setting; the rows are state.
                let inventory = self
                    .creation
                    .as_ref()
                    .map(|tables| tables.inventory(creation.campaign, creation.profession))
                    .unwrap_or_default();
                let characters = crate::characters::DatabaseCharacters::new(
                    std::sync::Arc::clone(pool),
                    self.log.clone(),
                );
                match characters.write_starting_items(
                    &crate::characters::uuid_text(&id),
                    &inventory,
                    &self.templates,
                ) {
                    Ok(written) => self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: the character's {written} starting item row(s) are written."
                        ),
                    ),
                    Err(error) => self.say(
                        LogLevel::Error,
                        format!(
                            "Gameplay connection from {peer}: the character was created but its items are not written: {error}"
                        ),
                    ),
                }

                // R12: a character starts with one map unlocked. The world's first outpost is the
                // one - the lowest map id this world hosts of that kind - and the gate that reads
                // these rows arrives with the travel answer.
                if let Some(outpost) = starting_outpost {
                    match characters.unlock_map(&crate::characters::uuid_text(&id), outpost.id) {
                        Ok(true) => self.say(
                            LogLevel::Info,
                            format!(
                                "Gameplay connection from {peer}: map {} ('{}') is unlocked for the new character (R12).",
                                outpost.id, outpost.name
                            ),
                        ),
                        Ok(false) => {}
                        Err(error) => self.say(
                            LogLevel::Error,
                            format!(
                                "Gameplay connection from {peer}: the character was created but its starting map is not unlocked: {error}"
                            ),
                        ),
                    }
                }

                let settings = crate::characters::CharacterSettings {
                    last_outpost: start_map,
                    appearance,
                    campaign: creation.campaign,
                    level: 1,
                    is_pvp: creation.campaign == 0,
                    secondary_profession: 0,
                    helm_status: 1,
                    pieces: Vec::new(),
                };
                let mut answer = vec![character_packets::char_creation_success(
                    id,
                    &name,
                    &settings.to_blob(),
                )];
                // And where the character now is, in the **five-byte login shape**. The client
                // reconnects naming the map it believes it is in, and that belief came from the
                // transfer, which named the creation instance: without this it dials back into the
                // creation screen and waits there forever (`ASURA/crates/asura-world/src/creation.rs`,
                // `handle_confirm`, measured against a real client). The four-byte-header redirect
                // generation is ignored by this client, so it is not the frame to send here.
                match starting_outpost {
                    Some(outpost) => answer.push(misc_packets::map_update_current(
                        outpost.id as u16,
                        0,
                    )),
                    None => self.say(
                        LogLevel::Warn,
                        format!(
                            "Gameplay connection from {peer}: the character '{name}' is created and this world hosts no outpost, so the client is told no map and will dial the creation instance again."
                        ),
                    ),
                }
                let _ = self
                    .send(io, peer, crypt, &answer, "the character creation")
                    .await;
            }
            Ok(Err(error)) => {
                self.say(
                    LogLevel::Error,
                    format!(
                        "Gameplay connection from {peer}: the character '{name}' could not be written: {error}"
                    ),
                );
                let refusal = character_packets::char_creation_error(
                    character_packets::GM_ERROR_CHARACTER_NAME_ALREADY_EXIST,
                );
                let _ = self.send(io, peer, crypt, &[refusal], "the refusal").await;
            }
            Err(_) => {
                self.say(
                    LogLevel::Error,
                    format!(
                        "Gameplay connection from {peer}: the character database answered no query, so '{name}' was not written."
                    ),
                );
            }
        }
    }

    /// Scramble and send whole frames, and say so.
    async fn send(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        frames: &[Vec<u8>],
        what: &str,
    ) -> bool {
        let mut answer = Vec::new();
        for frame in frames {
            answer.extend_from_slice(frame);
        }
        crypt.encrypt(&mut answer);
        match io.write_all(&answer).await {
            Ok(()) => {
                self.log.out(
                    LogLevel::Debug,
                    "network.trace",
                    format!(
                        "trace {peer} game out {} bytes: {}",
                        answer.len(),
                        shared::network::socket::hex(&answer[..answer.len().min(64)])
                    ),
                );
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: {what} answered with {} message(s), {} bytes.",
                        frames.len(),
                        answer.len()
                    ),
                );
                true
            }
            Err(error) => {
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer} could not be answered: {error}"),
                );
                false
            }
        }
    }
}

/// Seconds since the unix epoch, the clock the district hold counts on (R37).
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// The item, the bag and the slot of a client's `0x8072`, or `None` when the message is short.
///
/// Source: the client's own message, measured in the client captures as `GAME_CMSG_ITEM_MOVE`
/// (`recordings/client/feature/maps/`, a 9-byte frame: the header, the item, the bag and the slot).
fn item_move(message: &[u8]) -> Option<(u32, u16, u8)> {
    let item = message.get(2..6)?;
    let bag = message.get(6..8)?;
    let slot = message.get(8)?;
    Some((
        u32::from_le_bytes([item[0], item[1], item[2], item[3]]),
        u16::from_le_bytes([bag[0], bag[1]]),
        *slot,
    ))
}

/// The x and y a movement message carries: a `Vect2` right after the header.
///
/// Source: the client's own movement rows in this tree's table - `0x803D`, `0x803E` and `0x8047`,
/// each `MsgHeader Vect2 ...`, so x is at 2 and y at 6.
fn movement_point(message: &[u8]) -> Option<(f32, f32)> {
    let x = message.get(2..6)?;
    let y = message.get(6..10)?;
    Some((
        f32::from_le_bytes([x[0], x[1], x[2], x[3]]),
        f32::from_le_bytes([y[0], y[1], y[2], y[3]]),
    ))
}

/// What a client's `0x00B1` travel asks for: the map, the territory, the district, the language and
/// the flag - the nine-byte frame `docs/game-rules.md` E220 measured and E28 splits.
fn client_travel(message: &[u8]) -> Option<(u16, u8, u16, u8, u8)> {
    let map = message.get(2..4)?;
    let territory = *message.get(4)?;
    let district = message.get(5..7)?;
    let language = *message.get(7)?;
    let flag = *message.get(8)?;
    Some((
        u16::from_le_bytes([map[0], map[1]]),
        territory,
        u16::from_le_bytes([district[0], district[1]]),
        language,
        flag,
    ))
}

impl Session for WorldSession {
    fn run(self: Box<Self>, mut io: SessionIo) -> SessionFuture {
        Box::pin(async move {
            self.converse(&mut io).await;
            // The connection ended, whatever ended it: its place in the district is given back. The
            // copy itself stays open - R37 holds an outpost for 15 minutes of inactivity and R40
            // disposes exactly as the lifecycle says - so this only keeps the count honest.
            self.leave_district();
        })
    }
}

/// Read exactly as many bytes as the slice holds.
async fn read_exactly(io: &mut SessionIo, out: &mut [u8]) -> Result<(), String> {    let mut filled = 0;
    while filled < out.len() {
        match io.read(&mut out[filled..]).await {
            Ok(0) => return Err("the peer closed".to_string()),
            Ok(count) => filled += count,
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

impl WorldSession {
    /// The connection ended: its place in the district is given back.
    ///
    /// The copy itself stays open - R37 holds an outpost for 15 minutes of inactivity and R40
    /// disposes exactly as the lifecycle says - so this only keeps the count honest; the lifecycle
    /// that acts on an empty copy arrives with the instance table.
    fn leave_district(&self) {
        let entered = self
            .entered
            .lock()
            .ok()
            .and_then(|mut entered| entered.take());
        if let Some((map_id, district)) = entered
            && let Ok(mut districts) = self.districts.lock()
        {
            districts.leave(map_id, district, unix_now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::packets::creation_load_block;
    use crate::server::protocol::GameVersion;
    use crate::world::{map_kind, MapEntry};
    use shared::cryptography::handshake::{self, CLIENT_SEED_HEADER, KEY_LEN, MASTER_SECRET_LEN};
    use std::io::{Read, Write};
    use std::sync::Arc;

    /// The world a test serves: world 2, the character-creation instance and one outpost.
    fn world() -> World {
        let mut world = World::new(2);
        world.set_maps(
            vec![
                MapEntry {
                    id: 0,
                    kind: map_kind::CREATION,
                    file_id: 0,
                    name: "Character creation".to_string(),
                },
                MapEntry {
                    id: 148,
                    kind: map_kind::OUTPOST,
                    file_id: 113_021,
                    name: "Ascalon City (pre-Searing)".to_string(),
                },
            ],
            vec![crate::maps::MapSpawn {
                map_id: 148,
                index: 0,
                x: 9541.87,
                y: 4796.46,
                plane: 0,
            }],
        );
        assert_eq!(world.maps().len(), 2);

        // And the creation content, which the same start-up pass reads: a screen with no items has no
        // inventory model to draw.
        let (bags, items, sets) = world
            .load_creation("../data/inventory.json", "../data/equipment.json", &Log::new())
            .expect("the creation data");
        assert_eq!((bags, items, sets), (6, 9, 32), "the reference's own counts");
        world.set_templates(templates_from(world.creation().definitions()));
        world
    }

    fn factory(log: Log, world: std::sync::Arc<World>, master: [u8; MASTER_SECRET_LEN]) -> shared::network::socket::SessionFactory {
        Arc::new(move || {
            Box::new(WorldSession::with_master(log.clone(), &world, master)) as Box<dyn Session>
        })
    }

    #[test]
    fn a_travel_request_is_the_nine_bytes_the_ledger_measured() {
        // E220's own example, as the client sends it: map 148, territory -2, no district, no
        // language, and the flag that was 1 in every captured request.
        let mut message = vec![0xB1, 0x00];
        message.extend_from_slice(&148u16.to_le_bytes());
        message.push(0xFE);
        message.extend_from_slice(&0u16.to_le_bytes());
        message.push(0);
        message.push(1);
        assert_eq!(client_travel(&message), Some((148, 0xFE, 0, 0, 1)));
        assert_eq!(client_travel(&message[..8]), None, "short messages are not read");
    }

    #[test]
    fn a_movement_point_is_the_vect2_at_the_head() {
        let mut message = vec![0x3D, 0x80];
        message.extend_from_slice(&9541.87f32.to_le_bytes());
        message.extend_from_slice(&4796.46f32.to_le_bytes());
        message.extend_from_slice(&[0u8; 16]);
        assert_eq!(movement_point(&message), Some((9541.87, 4796.46)));
    }

    /// The templates a test's world runs with, built from the creation content's own definitions: what
    /// the world program reads back from its table, without a database.
    fn templates_from(items: Vec<crate::creation::Item>) -> Vec<crate::data_stores::ItemDefinition> {        items
            .iter()
            .enumerate()
            .map(|(index, item)| crate::data_stores::ItemDefinition {
                id: index as i64 + 1,
                file_id: item.file_id,
                item_type: item.item_type,
                dye_tint: item.dye_tint,
                dye_colors: item.dye_colors,
                materials: item.materials,
                unk1: item.unk1,
                flags: item.flags,
                value: item.value,
                model_id: item.model_id,
                quantity: item.quantity,
                profession: item.profession,
                name: item.name.clone(),
                modifiers: item.modifiers.clone(),
            })
            .collect()
    }

    /// Read whole frames off a scrambled connection until the message that ends a load, and answer
    /// every frame that arrived: its header and its bytes.
    fn read_load_frames(
        client: &mut std::net::TcpStream,
        reading: &mut common::cryptography::Arc4,
    ) -> Vec<(u16, Vec<u8>)> {
        let mut frames = Vec::new();
        let mut pending: Vec<u8> = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            while pending.len() >= 2 {
                match crate::server::protocol::decode_game_message(&pending, false) {
                    Ok((frame, used)) => {
                        let header = frame.header();
                        let whole: Vec<u8> = pending.drain(..used).collect();
                        frames.push((header, whole));
                        if header == instance_packets::INSTANCE_LOAD_FINISH {
                            return frames;
                        }
                    }
                    Err(FrameError::Incomplete { .. }) => break,
                    Err(error) => panic!("a frame of the load did not frame: {error}"),
                }
            }
            let read = client.read(&mut buffer).expect("the load keeps arriving");
            assert!(read > 0, "the connection ended before the load did");
            let mut plain = buffer[..read].to_vec();
            reading.apply(&mut plain);
            pending.extend_from_slice(&plain);
        }
    }

    #[test]
    fn a_gameplay_connection_is_answered_with_the_creation_block() {
        // The whole conversation a client makes, played by a test: the version message, the seed
        // exchange, and the block the world sends unasked. The master is fixed so the test can read
        // what a client would read.
        let log = Log::new();
        let world = Arc::new(world());
        let master = [0x5Au8; MASTER_SECRET_LEN];

        let mut manager = shared::network::socket_mgr::SocketMgr::new(factory(
            log.clone(),
            Arc::clone(&world),
            master,
        ));
        let port = manager.start_network("127.0.0.1", 0, 1).unwrap();
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        // 1. The 64-byte version message, as the client sends it.
        let version = GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 2,
            map_id: 0,
            player_token: 0xA2E3_DB8C,
            account: [0xAA; 16],
            character: [0; 16],
            h0038: 0,
            h003c: 0,
        };
        client.write_all(&version.encode()).unwrap();

        // 2. The seed: a public value with one byte set, so the arithmetic is cheap.
        let mut public = [0u8; handshake::PUBLIC_LEN];
        public[0] = 2;
        let mut seed = CLIENT_SEED_HEADER.to_le_bytes().to_vec();
        seed.extend_from_slice(&public);
        client.write_all(&seed).unwrap();

        // 3. The server's seed: the master XORed with the shared secret.
        let mut answer = [0u8; handshake::SERVER_SEED_LEN];
        client.read_exact(&mut answer).unwrap();
        assert_eq!(&answer[..2], &0x1601u16.to_le_bytes());

        let shared = handshake::shared_secret(&public).unwrap();
        for index in 0..MASTER_SECRET_LEN {
            assert_eq!(answer[2 + index] ^ shared[index], 0x5A, "the master is the one drawn");
        }

        // 4. The block: scrambled with the key both sides derive, and it is exactly the creation
        // block - the load head and the creation start, which is what a working server sends.
        let mut block = vec![0u8; creation_load_block().len()];
        client.read_exact(&mut block).unwrap();
        let key: [u8; KEY_LEN] = handshake::derive_key(&master);
        let mut opening = common::cryptography::Arc4::new(&key);
        opening.apply(&mut block);
        assert_eq!(block, creation_load_block());
        assert_eq!(block, vec![0x7C, 0x01, 0x3F, 0x3F, 0x00, 0x00, 0x89, 0x01]);

        // 5. The creation screen's first ask: the player-data block, whole and in order.
        let expected = creation_player_block(world.creation().into(), &Creation::start());
        let expected_bytes: usize = expected.iter().map(Vec::len).sum();
        let mut ask = vec![0x89, 0x80];
        let mut sending = common::cryptography::Arc4::new(&key);
        // The client's own outbound stream: the version message and the seed were plaintext, so its
        // cipher starts here.
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();

        let mut answered = vec![0u8; expected_bytes];
        client.read_exact(&mut answered).unwrap();
        opening.apply(&mut answered);

        let mut whole = Vec::new();
        for frame in &expected {
            whole.extend_from_slice(frame);
        }
        assert_eq!(answered, whole, "the player-data block is the reference's order");

        // The headers in order, which is what the client dispatches on.
        let mut at = 0;
        let mut headers = Vec::new();
        for frame in &expected {
            headers.push(u16::from_le_bytes([frame[0], frame[1]]));
            at += frame.len();
        }
        assert_eq!(at, answered.len(), "every byte is accounted for");
        // The order the reference sends: the player-data start, the item stream (a bag's own item,
        // the bag, then each item and its slot), the weapon sets and the gold, then the player's own
        // state, and finally the player-data end.
        assert_eq!(headers[0], 0x0186, "instance_player_data_start");
        assert_eq!(headers[1], 0x0144, "the item stream opens");
        assert_eq!(
            *headers.last().unwrap(),
            0x018A,
            "instance_player_data_done"
        );
        for header in [
            0x013F, 0x0161, 0x013E, // a bag, its items, and where each sits
            0x0148, 0x0147, 0x0141, // the active weapon set, four weapon sets, the gold
            0x00EA, 0x00EB, 0x00EC, 0x00ED, // the four faction caps
            0x0037, 0x00B7, 0x00B6, 0x00DA, 0x00DB, 0x00E9, 0x003A,
            0x009F, // energy, health, and the third property
        ] {
            assert!(headers.contains(&header), "header 0x{header:04X} is in the block");
        }
        let item_count = headers.iter().filter(|header| **header == 0x0161).count();
        assert!(item_count > 9, "the screen is sent the profession's equipment too");

        // 6. The screen chooses another campaign and profession (`0x8060`: campaign, profession).
        // The reference answers `GameSrv_HandleCharCreationChangeProf`'s own way, and this is the
        // step whose omission crashed the client: it keeps holding a set the server no longer names,
        // so the old pieces come off (`0x014D`) before the new ones are created (`0x0161`) and placed
        // (`0x013E`). The profession is told in between (`0x00B7`), and nothing else is.
        let tables = world.creation();
        let previous_set = tables.inventory(0, 1);
        let previous = previous_set.equipped_items();
        let next_set = tables.inventory(1, 2);
        let next = next_set.equipped_items();
        let equipped_bag = crate::creation::equipped_bag(tables)
            .expect("the creation data names an equipped bag")
            .id;
        let mut expected: Vec<Vec<u8>> = Vec::new();
        for item in &previous {
            expected.push(item_packets::item_remove(item_packets::ITEM_STREAM, item.id));
        }
        expected.push(character_packets::update_unlocked_skills(&[]));
        expected.push(character_packets::player_update_profession(
            CREATION_AGENT,
            2,
            0,
            0,
        ));
        for item in &next {
            expected.push(item_packets::create_named_item(&crate::creation::named_item(
                item,
            )));
            if let Some(slot) = item.slot {
                expected.push(item_packets::item_move_to_location(
                    item_packets::ITEM_STREAM,
                    item.id,
                    equipped_bag,
                    slot,
                ));
            }
        }
        let expected_bytes: usize = expected.iter().map(Vec::len).sum();

        let mut choose = vec![0x60, 0x80, 1, 2];
        sending.apply(&mut choose);
        client.write_all(&choose).unwrap();

        let mut answered = vec![0u8; expected_bytes];
        client.read_exact(&mut answered).unwrap();
        opening.apply(&mut answered);
        let mut whole = Vec::new();
        for frame in &expected {
            whole.extend_from_slice(frame);
        }
        assert_eq!(
            answered, whole,
            "the change of profession is the reference's answer, byte for byte"
        );

        // The headers, in the reference's order: every old piece removed, the profession, then each
        // new piece created and placed.
        let mut at = 0usize;
        let mut answer_headers = Vec::new();
        for frame in &expected {
            answer_headers.push(u16::from_le_bytes([answered[at], answered[at + 1]]));
            at += frame.len();
        }
        assert_eq!(at, answered.len(), "every byte is accounted for");
        let mut wanted: Vec<u16> = vec![0x014D; previous.len()];
        wanted.push(0x00DB);
        wanted.push(0x00B7);
        for item in &next {
            wanted.push(0x0161);
            if item.slot.is_some() {
                wanted.push(0x013E);
            }
        }
        assert_eq!(
            answer_headers, wanted,
            "the old set goes first, then the profession, then the new set"
        );
        assert!(
            !answer_headers.contains(&0x00B6),
            "a profession change does not re-state the unlocked-profession bitmap"
        );

        manager.stop_network();
        log.shutdown();
    }

    #[test]
    fn a_world_that_is_not_this_one_is_closed() {
        let log = Log::new();
        let world = Arc::new(world());
        let mut manager = shared::network::socket_mgr::SocketMgr::new(factory(
            log.clone(),
            Arc::clone(&world),
            [0x5Bu8; MASTER_SECRET_LEN],
        ));
        let port = manager.start_network("127.0.0.1", 0, 1).unwrap();
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        let version = GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 99,
            map_id: 0,
            player_token: 1,
            account: [0; 16],
            character: [0; 16],
            h0038: 0,
            h003c: 0,
        };
        client.write_all(&version.encode()).unwrap();

        let mut byte = [0u8; 1];
        let mut total = 0;
        while let Ok(read) = client.read(&mut byte) {
            if read == 0 {
                break;
            }
            total += read;
        }
        assert_eq!(total, 0, "a connection for another world is closed without a word");

        manager.stop_network();
        log.shutdown();
    }

    /// A connection for a real map is loaded: the head block, the entry position and the end of the
    /// load. Played against a listener with no database, because a load needs no row - the name falls
    /// back to "Player" and says so.
    #[test]
    fn an_outpost_connection_is_loaded_with_its_entry_point() {
        // A console logger, so the session's own words are visible while this is being proven.
        let log = Log::new();
        log.initialize(&common::configuration::Config::parse(
            "Appender.Console = 1,6,0
Logger.root = 6,Console
",
            std::path::PathBuf::from("test.conf"),
            common::configuration::Policy::default(),
        ))
        .expect("the test can speak");
        let world = Arc::new(world());
        let master = [0x5Au8; MASTER_SECRET_LEN];
        let factory = factory(log.clone(), Arc::clone(&world), master);
        let mut manager = shared::network::socket_mgr::SocketMgr::new(factory);
        let port = manager.start_network("127.0.0.1", 0, 1).unwrap();
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        // 1. The version message, naming map 148 - the outpost - and a character id no row holds.
        let version = GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 2,
            map_id: 148,
            player_token: 0x1234,
            account: [0xAA; 16],
            character: [0xBB; 16],
            h0038: 0,
            h003c: 0,
        };
        client.write_all(&version.encode()).unwrap();

        // 2. The seed exchange.
        let mut public = [0u8; handshake::PUBLIC_LEN];
        public[0] = 2;
        let mut seed = CLIENT_SEED_HEADER.to_le_bytes().to_vec();
        seed.extend_from_slice(&public);
        client.write_all(&seed).unwrap();

        let mut answer = [0u8; handshake::SERVER_SEED_LEN];
        client.read_exact(&mut answer).unwrap();
        let key: [u8; KEY_LEN] = handshake::derive_key(&master);
        let mut reading = common::cryptography::Arc4::new(&key);
        let mut sending = common::cryptography::Arc4::new(&key);

        // 3. The head block: load head, player-data start, the name, the instance's record.
        let expected = [
            instance_packets::instance_load_head(),
            instance_packets::instance_player_data_start(),
            instance_packets::instance_load_player_name("Player"),
            instance_packets::instance_load_info(1, 148, 0, 1, 0, 0),
        ];
        let expected_bytes: usize = expected.iter().map(Vec::len).sum();
        let mut block = vec![0u8; expected_bytes];
        client.read_exact(&mut block).unwrap();
        reading.apply(&mut block);
        let mut whole = Vec::new();
        for frame in &expected {
            whole.extend_from_slice(frame);
        }
        assert_eq!(block, whole, "the load's head, in the measured order");
        let headers: Vec<u16> = expected
            .iter()
            .map(|frame| u16::from_le_bytes([frame[0], frame[1]]))
            .collect();
        assert_eq!(headers, vec![0x017C, 0x0186, 0x017D, 0x019A]);

        // 4. The ping pair the measured load follows the head block with.
        let mut ping = vec![0u8; 8];
        client.read_exact(&mut ping).unwrap();
        reading.apply(&mut ping);
        assert_eq!(
            ping,
            vec![0x0C, 0x00, 0x0D, 0x00, 0x14, 0x00, 0x00, 0x00],
            "the ping request and its reply, as a working server sent them"
        );

        // 5. The items ask: the inventory stream and the map's record, in the measured order.
        let mut ask = vec![0x91, 0x80, 0x00, 0x00];
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();

        let inventory = world
            .creation()
            .inventory(0, character_packets::DEFAULT_PROFESSION);
        let mut expected_items = crate::creation::load_item_stream(&inventory, 0);
        expected_items.push(misc_packets::hard_mode_unlocked(1));
        expected_items.push(character_packets::character_update_info());
        expected_items.push(misc_packets::map_update_current(148, 0));
        expected_items.push(instance_packets::ready_for_map_spawn(0));
        expected_items.push(instance_packets::instance_manifest_phase(0));
        expected_items.push(instance_packets::instance_manifest_phase(1));
        expected_items.push(instance_packets::instance_manifest_done(2, 877, 0));
        expected_items.push(instance_packets::instance_manifest_phase(0));
        expected_items.push(instance_packets::instance_manifest_phase(1));
        expected_items.push(instance_packets::instance_manifest_done(0, 148, 0));
        let expected_bytes: usize = expected_items.iter().map(Vec::len).sum();
        let mut items = vec![0u8; expected_bytes];
        client.read_exact(&mut items).unwrap();
        reading.apply(&mut items);
        let mut whole = Vec::new();
        for frame in &expected_items {
            whole.extend_from_slice(frame);
        }
        assert_eq!(items, whole, "the items and the record, in the measured order");
        // The measured header sequence, so a reordering fails here rather than at the client.
        let headers: Vec<u16> = expected_items
            .iter()
            .map(|frame| u16::from_le_bytes([frame[0], frame[1]]))
            .collect();
        for header in [0x0144, 0x0148, 0x0147, 0x0141, 0x00EA, 0x00ED, 0x0070, 0x0030, 0x0099, 0x01AC, 0x0199, 0x0198] {
            assert!(headers.contains(&header), "header 0x{header:04X} is in the answer");
        }
        assert_eq!(*headers.last().unwrap(), 0x0198, "the manifest ends the block");

        // 6. The spawn request: the world answers with the map file and a point of its own list,
        // and the first simulation tick.
        let mut ask = vec![0x88, 0x80];
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();

        let mut spawn = vec![0u8; 32];
        client.read_exact(&mut spawn).unwrap();
        reading.apply(&mut spawn);
        assert_eq!(&spawn[..2], &[0x96, 0x01], "the entry position");
        assert_eq!(
            &spawn[2..6],
            &113_021u32.to_le_bytes(),
            "map 148's file id, as the recording carried it"
        );
        // The point is scattered around the map's own spawn, so it is near it and not equal to it.
        let x = f32::from_le_bytes([spawn[6], spawn[7], spawn[8], spawn[9]]);
        let y = f32::from_le_bytes([spawn[10], spawn[11], spawn[12], spawn[13]]);
        assert!(
            (x - 9541.87).abs() <= 250.0 + 1.0 && (y - 4796.46).abs() <= 250.0 + 1.0,
            "the point {x}, {y} is within the scatter of the map's own"
        );
        assert_eq!(
            &spawn[26..],
            &[0x1E, 0x00, 0xF4, 0x01, 0x00, 0x00],
            "and the simulation's first tick, 500"
        );

        // 7. The players request: the whole load block, in the reference's order, ending at the
        // message that takes the client off the loading screen. Read until that message arrives.
        let mut ask = vec![0x90, 0x80, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB,
                           0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB, 0x00];
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();

        let blocks = read_load_frames(&mut client, &mut reading);
        let headers: Vec<u16> = blocks.iter().map(|(header, _)| *header).collect();
        // The reference's own order (`ASURA/crates/asura-world/src/player.rs`, `player_block`): the
        // PvP halves and the unlockable items, the account features, the unlocked lists and the team
        // token, the skills and the progress record, the flags, the player and the agent, the agent's
        // properties and look, the party, and the end.
        for header in [
            0x001D, 0x0018, 0x001A, 0x001B, // the PvP unlocks and the item table
            0x000F, // an account feature
            0x0094, 0x00F2, // the unlocked lists and the team token
            0x0037, 0x00B7, 0x00B6, 0x00DA, 0x00DB, // attribute points, profession, skills
            0x00E9, // the progress record
            0x00EF, 0x003C, 0x003A, 0x001F, // the flags, the attributes, the load time
            0x0059, 0x0020, 0x009C, // the player, the agent, its morale
            0x009F, 0x00A2, 0x00F0, // the agent's properties and effects
            0x006E, 0x0048, 0x0022, // what it wears, its cape, the agent the client drives
            0x01D3, 0x01CC, 0x01D4, 0x01B3, // the party
            0x018E, // and the load is over
        ] {
            assert!(
                headers.contains(&header),
                "the players answer carries 0x{header:04X}: {headers:04X?}"
            );
        }
        assert_eq!(headers.first(), Some(&0x001D), "it opens with the PvP half");
        assert_eq!(headers.last(), Some(&0x018E), "and ends with the load's own end");
        // The party comes after the agent and before the end, in the reference's own order: a client
        // told about a party it was never given asserts `m_partyClient`.
        let party_at = headers
            .iter()
            .position(|header| *header == 0x01D3)
            .expect("the party is created");
        assert_eq!(
            &headers[party_at..party_at + 4],
            &[0x01D3, 0x01CC, 0x01D4, 0x01B3],
            "the party before the character's own party word: {headers:04X?}"
        );
        // The agent's own body is the frame that puts a body in the map, and it carries the point the
        // entry-position answer named.
        assert!(
            headers.contains(&0x0020),
            "the load creates the agent it is played as: {headers:04X?}"
        );

        // 8. And the connection stays: the world ticks it, one per interval, with no prompt.
        let mut tick = vec![0u8; 6];
        client.read_exact(&mut tick).unwrap();
        reading.apply(&mut tick);
        assert_eq!(&tick[..2], &[0x1E, 0x00], "a world simulation tick");
        assert_eq!(
            &tick[2..],
            &501u32.to_le_bytes(),
            "the next tick after the load's first, 500"
        );

        // 9. The client's own traffic does not end the session: a movement, then a travel request,
        // and the world ticks again after both.
        let mut movement = vec![0x3D, 0x80];
        movement.extend_from_slice(&9550.0f32.to_le_bytes());
        movement.extend_from_slice(&4800.0f32.to_le_bytes());
        movement.extend_from_slice(&[0u8; 16]);
        sending.apply(&mut movement);
        client.write_all(&movement).unwrap();

        let mut travel = vec![0xB1, 0x00];
        travel.extend_from_slice(&148u16.to_le_bytes());
        travel.push(0xFE); // territory -2
        travel.extend_from_slice(&0u16.to_le_bytes());
        travel.push(0); // language
        travel.push(1); // flag
        sending.apply(&mut travel);
        client.write_all(&travel).unwrap();

        let mut tick = vec![0u8; 6];
        client.read_exact(&mut tick).unwrap();
        reading.apply(&mut tick);
        assert_eq!(&tick[..2], &[0x1E, 0x00], "the world is still ticking");

        manager.stop_network();
        log.shutdown();
    }

    #[test]
    #[ignore = "needs the login and character databases"]
    fn a_travel_record_decides_the_map_and_the_district() {
        // The login writes a travel row when it answers a client's request for a map (`0x8029`), and
        // this connection claims it once (`docs/port/06-world-plan.md`, "The three seams"). The
        // version message names the creation instance on purpose: what loads must be the map the
        // record names, with the district and language the request carried (R57).
        use common::configuration::{Config, Policy};
        use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};
        use database::updater::{collect_files, UpdateState};

        let _serial_auth = database::updater::apply::test_lock("auth");
        let _serial_characters = database::updater::apply::test_lock("characters");
        let log = Log::new();
        let config = Config::load_json("../worldserver/worldserver.json", false, Policy::default())
            .expect("the world program's own configuration");

        let settings = read_settings(&config, DatabaseKind::Login).expect("the login settings");
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::Login, &settings, &log)
            .expect("the login pool opens");
        let schema = collect_files(
            std::path::Path::new("../data/sql/base/db_auth"),
            UpdateState::Released,
        )
        .expect("the login schema");
        pool.apply_updates(&schema, &log).expect("the schema applies");
        let login = Arc::new(std::sync::Mutex::new(pool));

        let settings = read_settings(&config, DatabaseKind::Character)
            .expect("the character settings");
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::Character, &settings, &log)
            .expect("the character pool opens");
        let schema = collect_files(
            std::path::Path::new("../data/sql/base/db_characters"),
            UpdateState::Released,
        )
        .expect("the character schema");
        pool.apply_updates(&schema, &log).expect("the schema applies");
        let characters = Arc::new(std::sync::Mutex::new(pool));

        // The record the login would have written for this connection.
        let account = [0xAAu8; 16];
        crate::maps::DatabaseTravels::new(Arc::clone(&login), log.clone())
            .record(&crate::maps::Travel {
                account,
                world: 2,
                map_id: 148,
                district: 3,
                language: 2,
                player_token: 0x1234,
            })
            .expect("the travel is recorded");

        // The character the connection names: its row, and the items a confirm would have written.
        let character = [0xBBu8; 16];
        let character_text = crate::characters::uuid_text(&character);
        let prepared_world = world();
        let starting = prepared_world.creation().inventory(0, 1);
        {
            let mut pool = characters.lock().unwrap();
            pool.query_with(
                "DELETE FROM \"item_instance\" WHERE \"character_uuid\" = $1",
                &[&character_text],
            )
            .unwrap();
            pool.query_with(
                "DELETE FROM \"characters\" WHERE \"uuid\" = $1",
                &[&character_text],
            )
            .unwrap();
            pool.query_with(
                "INSERT INTO \"characters\" (\"uuid\", \"account_uuid\", \"name\", \"appearance\", \"primary_profession\", \"unlocked_professions\", \"campaign\", \"level\", \"gold\") \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                &[
                    &character_text,
                    &crate::characters::uuid_text(&account),
                    &"Travelled One",
                    &0i64,
                    &1i64,
                    &2i64,
                    &0i64,
                    &1i64,
                    &1234i64,
                ],
            )
            .unwrap();
        }
        crate::characters::DatabaseCharacters::new(Arc::clone(&characters), log.clone())
            .write_starting_items(&character_text, &starting, prepared_world.templates())
            .expect("the character's items are written");
        let world = Arc::new(prepared_world);
        let master = [0x33u8; MASTER_SECRET_LEN];
        let factory: shared::network::socket::SessionFactory = {
            let log = log.clone();
            let world = Arc::clone(&world);
            let characters = Arc::clone(&characters);
            let login = Arc::clone(&login);
            Arc::new(move || {
                Box::new(WorldSession::with_master_and_characters_and_login(
                    log.clone(),
                    &world,
                    master,
                    Arc::clone(&characters),
                    Arc::clone(&login),
                )) as Box<dyn Session>
            })
        };
        let mut manager = shared::network::socket_mgr::SocketMgr::new(factory);
        let port = manager.start_network("127.0.0.1", 0, 1).unwrap();
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        // 1. The version message: the creation instance, and the token pair the login drew.
        let version = GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 2,
            map_id: 0,
            player_token: 0x1234,
            account,
            character: [0xBB; 16],
            h0038: 0,
            h003c: 0,
        };
        client.write_all(&version.encode()).unwrap();

        // 2. The seed exchange.
        let mut public = [0u8; handshake::PUBLIC_LEN];
        public[0] = 2;
        let mut seed = CLIENT_SEED_HEADER.to_le_bytes().to_vec();
        seed.extend_from_slice(&public);
        client.write_all(&seed).unwrap();
        let mut answer = [0u8; handshake::SERVER_SEED_LEN];
        client.read_exact(&mut answer).unwrap();
        let key: [u8; KEY_LEN] = handshake::derive_key(&master);
        let mut reading = common::cryptography::Arc4::new(&key);
        let mut sending = common::cryptography::Arc4::new(&key);

        // 3. The block: the outpost's load, not the creation instance's, with the record's district
        // and language, and the character's own name.
        let expected = [
            instance_packets::instance_load_head(),
            instance_packets::instance_player_data_start(),
            instance_packets::instance_load_player_name("Travelled One"),
            instance_packets::instance_load_info(1, 148, 0, 3, 2, 0),
        ];
        let expected_bytes: usize = expected.iter().map(Vec::len).sum();
        let mut block = vec![0u8; expected_bytes];
        client.read_exact(&mut block).unwrap();
        reading.apply(&mut block);
        let mut whole = Vec::new();
        for frame in &expected {
            whole.extend_from_slice(frame);
        }
        assert_eq!(
            block, whole,
            "the travel's map, district and language, not the version message's"
        );

        // R12: the character had no unlock rows, so the load wrote the world's first outpost for
        // it before entering - the same starting unlock a confirm writes.
        let unlocked = crate::characters::DatabaseCharacters::new(
            Arc::clone(&characters),
            log.clone(),
        )
        .unlocked_maps(&character_text)
        .expect("the character's unlocks");
        assert_eq!(unlocked, vec![148], "the starting unlock the load wrote");

        // 4. The ping pair, then the character's own items - the rows the confirm wrote, drawn the
        // same way the creation set would be - and then where it stands on map 148's own file.
        let mut ping = vec![0u8; 8];
        client.read_exact(&mut ping).unwrap();
        reading.apply(&mut ping);
        assert_eq!(ping.len(), 8, "the measured ping pair");

        let mut ask = vec![0x91, 0x80, 0x00, 0x00];
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let mut expected_items = crate::creation::load_item_stream(&starting, 1234);
        expected_items.push(misc_packets::hard_mode_unlocked(1));
        expected_items.push(character_packets::character_update_info());
        expected_items.push(misc_packets::map_update_current(148, 0));
        expected_items.push(instance_packets::ready_for_map_spawn(0));
        expected_items.push(instance_packets::instance_manifest_phase(0));
        expected_items.push(instance_packets::instance_manifest_phase(1));
        expected_items.push(instance_packets::instance_manifest_done(2, 877, 0));
        expected_items.push(instance_packets::instance_manifest_phase(0));
        expected_items.push(instance_packets::instance_manifest_phase(1));
        expected_items.push(instance_packets::instance_manifest_done(0, 148, 0));
        let expected_bytes: usize = expected_items.iter().map(Vec::len).sum();
        let mut items = vec![0u8; expected_bytes];
        client.read_exact(&mut items).unwrap();
        reading.apply(&mut items);
        let mut whole = Vec::new();
        for frame in &expected_items {
            whole.extend_from_slice(frame);
        }
        assert_eq!(
            items, whole,
            "the character's held items, drawn from its rows, in the measured order"
        );

        let mut ask = vec![0x88, 0x80];
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let mut spawn = vec![0u8; 32];
        client.read_exact(&mut spawn).unwrap();
        reading.apply(&mut spawn);
        assert_eq!(
            &spawn[2..6],
            &113_021u32.to_le_bytes(),
            "the outpost's file, so the record decided the map"
        );
        assert_eq!(&spawn[26..32], &[0x1E, 0x00, 0xF4, 0x01, 0x00, 0x00], "the first tick");

        // 4c. A drag in the inventory: the item's row moves, and the client is told the placement.
        let mut ask = vec![0x72, 0x80];
        ask.extend_from_slice(&1u32.to_le_bytes());
        ask.extend_from_slice(&3u16.to_le_bytes());
        ask.push(7);
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let mut moved = vec![0u8; 11];
        client.read_exact(&mut moved).unwrap();
        reading.apply(&mut moved);
        assert_eq!(&moved[..2], &[0x3E, 0x01], "the placement");
        assert_eq!(&moved[2..4], &item_packets::ITEM_STREAM.to_le_bytes(), "the stream");
        assert_eq!(&moved[4..8], &1u32.to_le_bytes(), "the item");
        assert_eq!(&moved[8..10], &3u16.to_le_bytes(), "the bag");
        assert_eq!(moved[10], 7, "the slot");
        let held = crate::characters::DatabaseCharacters::new(Arc::clone(&characters), log.clone())
            .items_of(&character_text)
            .expect("the item rows are readable");
        let moved_row = held
            .iter()
            .find(|row| row.item_id == 1)
            .expect("the item is one of the character's");
        assert_eq!((moved_row.bag_id, moved_row.slot), (3, 7), "the row moved with it");

        // 4d. An equip: the item goes to the slot its kind is worn in, in the equipped bag.
        let worn = starting
            .items
            .iter()
            .find(|item| crate::creation::equipped_slot(item.item_type).is_some())
            .expect("the set carries something wearable");
        let worn_slot = crate::creation::equipped_slot(worn.item_type).unwrap();
        let equipped_bag = crate::creation::equipped_bag(world.creation())
            .expect("the setting names the equipped bag");
        let mut ask = vec![0x30, 0x80];
        ask.extend_from_slice(&worn.id.to_le_bytes());
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let mut placed = vec![0u8; 11];
        client.read_exact(&mut placed).unwrap();
        reading.apply(&mut placed);
        assert_eq!(&placed[..2], &[0x3E, 0x01], "the placement");
        assert_eq!(&placed[4..8], &worn.id.to_le_bytes(), "the item");
        assert_eq!(&placed[8..10], &equipped_bag.id.to_le_bytes(), "the equipped bag");
        assert_eq!(placed[10], worn_slot, "the slot its kind is worn in");

        // 4e. A destroy: the row goes, and the client is told the item is gone.
        let doomed = starting
            .items
            .iter()
            .find(|item| item.id != worn.id && item.id != 1)
            .expect("the set carries more than two items")
            .id;
        let mut ask = vec![0x69, 0x80];
        ask.extend_from_slice(&doomed.to_le_bytes());
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let mut gone = vec![0u8; 8];
        client.read_exact(&mut gone).unwrap();
        reading.apply(&mut gone);
        assert_eq!(
            &gone[..2],
            &[0x4D, 0x01],
            "the item's removal; got {}",
            shared::network::socket::hex(&gone)
        );
        assert_eq!(&gone[2..4], &item_packets::ITEM_STREAM.to_le_bytes(), "the stream");
        assert_eq!(&gone[4..8], &doomed.to_le_bytes(), "the item");
        let held = crate::characters::DatabaseCharacters::new(Arc::clone(&characters), log.clone())
            .items_of(&character_text)
            .expect("the item rows are readable");
        assert!(
            held.iter().all(|row| row.item_id != doomed),
            "the destroyed item's row is gone"
        );

        // 5. The record is claimed once: a second claim finds nothing.
        let claimed = crate::maps::DatabaseTravels::new(Arc::clone(&login), log.clone())
            .claim(2, &crate::characters::uuid_text(&account), 0x1234)
            .expect("the travel table reads");
        assert!(claimed.is_none(), "the row was consumed by the connection");

        // The test's own character and its items go, leaving the databases as they were found.
        {
            let mut pool = characters.lock().unwrap();
            pool.query_with(
                "DELETE FROM \"item_instance\" WHERE \"character_uuid\" = $1",
                &[&character_text],
            )
            .unwrap();
            pool.query_with("DELETE FROM \"characters\" WHERE \"uuid\" = $1", &[&character_text])
                .unwrap();
        }

        manager.stop_network();
        log.shutdown();
    }

    #[test]
    #[ignore = "needs the login and character databases"]
    fn a_claimed_map_the_character_has_not_unlocked_is_refused() {
        // R12: the gate the load applies to a claimed map. The character has unlocked map 148, the
        // login records a travel to map 147, and the connection is closed before the load block -
        // nothing enters, the district is not taken and the character's row is not touched. R58's
        // refusal has the same shape and the same reason: the refusal's own wire is not captured,
        // so a closed connection is the refusal.
        use common::configuration::{Config, Policy};
        use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};
        use database::updater::{collect_files, UpdateState};

        let _serial_auth = database::updater::apply::test_lock("auth");
        let _serial_characters = database::updater::apply::test_lock("characters");
        let log = Log::new();
        let config = Config::load_json("../worldserver/worldserver.json", false, Policy::default())
            .expect("the world program's own configuration");

        let settings = read_settings(&config, DatabaseKind::Login).expect("the login settings");
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::Login, &settings, &log)
            .expect("the login pool opens");
        let schema = collect_files(
            std::path::Path::new("../data/sql/base/db_auth"),
            UpdateState::Released,
        )
        .expect("the login schema");
        pool.apply_updates(&schema, &log).expect("the schema applies");
        let login = Arc::new(std::sync::Mutex::new(pool));

        let settings =
            read_settings(&config, DatabaseKind::Character).expect("the character settings");
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::Character, &settings, &log)
            .expect("the character pool opens");
        let schema = collect_files(
            std::path::Path::new("../data/sql/base/db_characters"),
            UpdateState::Released,
        )
        .expect("the character schema");
        pool.apply_updates(&schema, &log).expect("the schema applies");
        let characters = Arc::new(std::sync::Mutex::new(pool));

        // The travel the login would have written for this connection: map 147, which the character
        // below has not unlocked.
        let account = [0xCCu8; 16];
        crate::maps::DatabaseTravels::new(Arc::clone(&login), log.clone())
            .record(&crate::maps::Travel {
                account,
                world: 2,
                map_id: 147,
                district: 1,
                language: 0,
                player_token: 0x2345,
            })
            .expect("the travel is recorded");

        // The character: one row, one unlock (148), no items needed - the gate decides before any of
        // the load's own messages.
        let character = [0xDDu8; 16];
        let character_text = crate::characters::uuid_text(&character);
        {
            let mut pool = characters.lock().unwrap();
            pool.query_with(
                "DELETE FROM \"character_unlock\" WHERE \"character_uuid\" = $1",
                &[&character_text],
            )
            .unwrap();
            pool.query_with(
                "DELETE FROM \"characters\" WHERE \"uuid\" = $1",
                &[&character_text],
            )
            .unwrap();
            pool.query_with(
                "INSERT INTO \"characters\" (\"uuid\", \"account_uuid\", \"name\", \"appearance\", \"primary_profession\", \"unlocked_professions\", \"campaign\", \"level\", \"gold\") \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                &[
                    &character_text,
                    &crate::characters::uuid_text(&account),
                    &"Ununlocked One",
                    &0i64,
                    &1i64,
                    &2i64,
                    &0i64,
                    &1i64,
                    &0i64,
                ],
            )
            .unwrap();
        }
        let repository =
            crate::characters::DatabaseCharacters::new(Arc::clone(&characters), log.clone());
        repository
            .unlock_map(&character_text, 148)
            .expect("the character has one unlock");

        // The world hosts both outposts, so the claim is accepted and the gate is what refuses.
        let mut prepared = world();
        let mut maps = prepared.maps().to_vec();
        maps.push(crate::world::MapEntry {
            id: 147,
            kind: crate::world::map_kind::OUTPOST,
            file_id: 112_508,
            name: "Ascalon Academy".to_string(),
        });
        let mut spawns = prepared.spawns().to_vec();
        spawns.push(crate::maps::MapSpawn {
            map_id: 147,
            index: 0,
            x: 1000.0,
            y: 2000.0,
            plane: 0,
        });
        prepared.set_maps(maps, spawns);
        let world = Arc::new(prepared);

        let master = [0x44u8; MASTER_SECRET_LEN];
        let factory: shared::network::socket::SessionFactory = {
            let log = log.clone();
            let world = Arc::clone(&world);
            let characters = Arc::clone(&characters);
            let login = Arc::clone(&login);
            Arc::new(move || {
                Box::new(WorldSession::with_master_and_characters_and_login(
                    log.clone(),
                    &world,
                    master,
                    Arc::clone(&characters),
                    Arc::clone(&login),
                )) as Box<dyn Session>
            })
        };
        let mut manager = shared::network::socket_mgr::SocketMgr::new(factory);
        let port = manager.start_network("127.0.0.1", 0, 1).unwrap();
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        // 1. The version message, naming the creation instance, and the token pair the travel holds.
        let version = GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 2,
            map_id: 0,
            player_token: 0x2345,
            account,
            character,
            h0038: 0,
            h003c: 0,
        };
        client.write_all(&version.encode()).unwrap();

        // 2. The seed exchange: the server answers its seed, then closes - the refusal arrives
        // instead of the load block, because the claimed map is not in the character's unlocks.
        let mut public = [0u8; handshake::PUBLIC_LEN];
        public[0] = 2;
        let mut seed = CLIENT_SEED_HEADER.to_le_bytes().to_vec();
        seed.extend_from_slice(&public);
        client.write_all(&seed).unwrap();
        let mut answer = [0u8; handshake::SERVER_SEED_LEN];
        client.read_exact(&mut answer).unwrap();
        let mut block = [0u8; 64];
        let read = client.read(&mut block);
        assert_eq!(
            read.unwrap_or(0),
            0,
            "the connection is closed before the load block: {}",
            shared::network::socket::hex(&block)
        );

        // 3. Nothing was entered, and nothing was written: the unlock set stands and the character's
        // last outpost is still the row's own 0.
        let unlocked = repository
            .unlocked_maps(&character_text)
            .expect("the character's unlocks");
        assert_eq!(unlocked, vec![148], "the refused claim wrote no unlock");
        let listed = repository.for_account(&crate::characters::uuid_text(&account));
        assert_eq!(listed.len(), 1);
        assert_eq!(
            listed[0].settings.last_outpost, 0,
            "the refused claim wrote no last outpost"
        );

        // The character's own rows go, leaving the databases as they were found.
        {
            let mut pool = characters.lock().unwrap();
            pool.query_with(
                "DELETE FROM \"character_unlock\" WHERE \"character_uuid\" = $1",
                &[&character_text],
            )
            .unwrap();
            pool.query_with("DELETE FROM \"characters\" WHERE \"uuid\" = $1", &[&character_text])
                .unwrap();
        }

        manager.stop_network();
        log.shutdown();
    }

    #[test]
    #[ignore = "needs the character database"]
    fn a_creation_confirm_writes_a_character_row() {
        // The confirm, played against the real character database: the client's bytes go in, a row
        // comes out, and the answer says the same name. This is the stage's proof with a database in
        // it rather than a stub.
        use common::configuration::{Config, Policy};
        use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};
        use database::updater::{collect_files, UpdateState};

        // A console appender, so the session's own words are visible in this test's output.
        let _serial = database::updater::apply::test_lock("characters");
        let log = Log::new();
        let speaking = Config::parse(
            "Appender.Console = 1,6,0
Logger.root = 6,Console
",
            std::path::PathBuf::from("test.conf"),
            Policy::default(),
        );
        log.initialize(&speaking).expect("the test can speak");
        let config = Config::load_json("../worldserver/worldserver.json", false, Policy::default())
            .expect("the world program's own configuration");
        let settings =
            read_settings(&config, DatabaseKind::Character).expect("the character settings");
        let mut pool = DatabaseWorkerPool::open(DatabaseKind::Character, &settings, &log)
            .expect("the pool opens");
        let schema = collect_files(
            std::path::Path::new("../data/sql/base/db_characters"),
            UpdateState::Released,
        )
        .expect("the characters schema");
        pool.apply_updates(&schema, &log).expect("the schema applies");
        // The name this test uses may be left behind by an earlier run, and a taken name is a
        // refusal, so the row goes first: the test's own character and nobody else's.
        pool.query_with(
            "DELETE FROM characters WHERE name = $1",
            &[&"Kryta Test"],
        )
        .expect("no leftover character");
        let pool = Arc::new(std::sync::Mutex::new(pool));

        let world = Arc::new(world());
        let master = [0x77u8; MASTER_SECRET_LEN];
        let factory: shared::network::socket::SessionFactory = {
            let log = log.clone();
            let world = Arc::clone(&world);
            let pool = Arc::clone(&pool);
            Arc::new(move || {
                Box::new(WorldSession::with_master_and_characters(
                    log.clone(),
                    &world,
                    master,
                    Arc::clone(&pool),
                )) as Box<dyn Session>
            })
        };

        let mut manager = shared::network::socket_mgr::SocketMgr::new(factory);
        let port = manager.start_network("127.0.0.1", 0, 1).unwrap();
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        // The version message, with an account this test owns.
        let mut account = [0u8; 16];
        account[0] = 0xC1;
        account[15] = 0x0D;
        let version = GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 2,
            map_id: 0,
            player_token: 0x1234,
            account,
            character: [0; 16],
            h0038: 0,
            h003c: 0,
        };
        client.write_all(&version.encode()).unwrap();

        // The seed exchange.
        let mut public = [0u8; handshake::PUBLIC_LEN];
        public[0] = 2;
        let mut seed = CLIENT_SEED_HEADER.to_le_bytes().to_vec();
        seed.extend_from_slice(&public);
        client.write_all(&seed).unwrap();

        let mut answer = [0u8; handshake::SERVER_SEED_LEN];
        client.read_exact(&mut answer).unwrap();
        let mut reading = common::cryptography::Arc4::new(&handshake::derive_key(&master));
        let mut sending = common::cryptography::Arc4::new(&handshake::derive_key(&master));

        let mut block = vec![0u8; creation_load_block().len()];
        client.read_exact(&mut block).unwrap();
        reading.apply(&mut block);

        // The confirm, in the client's own size: sixteen bytes plus twice the name
        // (`docs/game-rules.md`, E73 - "32 bytes for an 8-character name, 30 for 7"), which is the
        // header, the count, the name, the packed appearance word and eight more bytes.
        let name = "Kryta Test";
        let units: Vec<u16> = name.encode_utf16().collect();
        let appearance_at = 4 + units.len() * 2;
        let mut confirm = vec![0u8; 16 + units.len() * 2];
        confirm[0..2].copy_from_slice(&0x808Bu16.to_le_bytes());
        confirm[2..4].copy_from_slice(&(units.len() as u16).to_le_bytes());
        for (index, unit) in units.iter().enumerate() {
            let at = 4 + index * 2;
            confirm[at..at + 2].copy_from_slice(&unit.to_le_bytes());
        }
        confirm[appearance_at..appearance_at + 4].copy_from_slice(&0x00AB_CDEFu32.to_le_bytes());
        let mut scrambled = confirm.clone();
        sending.apply(&mut scrambled);
        client.write_all(&scrambled).unwrap();

        // The answer: `0x0188` with the character's id, its name, and the twenty-six bytes measured
        // after them.
        let mut answered = vec![0u8; 2 + 16 + 2 + units.len() * 2 + 2 + 2 + crate::characters::CHARACTER_SETTINGS_LEN];
        client.read_exact(&mut answered).unwrap();
        reading.apply(&mut answered);
        assert_eq!(&answered[..2], &0x0188u16.to_le_bytes(), "the creation succeeded");

        manager.stop_network();

        // And the row is there, under the account the version message named.
        let uuid_text = crate::characters::uuid_text(&{
            let mut id = [0u8; 16];
            id.copy_from_slice(&answered[2..18]);
            id
        });
        let mut guard = pool.lock().unwrap();
        let rows = guard
            .query_with(
                "SELECT \"name\", \"appearance\" FROM \"characters\" WHERE \"uuid\" = $1",
                &[&uuid_text],
            )
            .expect("the row is readable");
        assert_eq!(rows.len(), 1, "one row was written");
        assert_eq!(rows[0].get::<usize, String>(0), name);
        drop(guard);

        // The character's items are rows now, and the rows draw the same stream the creation
        // record's set draws: one row per item, and the load's block is byte-identical.
        let characters = crate::characters::DatabaseCharacters::new(Arc::clone(&pool), log.clone());
        let held = characters.items_of(&uuid_text).expect("the item rows are readable");
        assert!(!held.is_empty(), "the confirm writes the character's starting items");
        let expected = world.creation().inventory(0, character_packets::DEFAULT_PROFESSION);
        assert_eq!(
            held.len(),
            expected.items.len(),
            "one row per item the set carries"
        );
        let templates = templates_from(world.creation().definitions());
        let from_rows = crate::creation::inventory_from_held(
            crate::creation::bags(world.creation()),
            &held,
            &templates,
            &log,
        );
        assert_eq!(
            crate::creation::load_item_stream(&from_rows, 0),
            crate::creation::load_item_stream(&expected, 0),
            "the rows draw the same item stream the set does"
        );

        // R12: the character starts with the world's first outpost unlocked - map 148, the only
        // outpost this world hosts.
        assert_eq!(
            characters
                .unlocked_maps(&uuid_text)
                .expect("the unlocks are readable"),
            vec![148],
            "the starting map is unlocked"
        );
        log.shutdown();
    }

}