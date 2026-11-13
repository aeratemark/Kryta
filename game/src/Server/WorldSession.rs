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
    agent_packets, character_packets, chat_packets, creation_load_block, instance_packets,
    item_packets, misc_packets, party_packets,
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

/// The effect word a travel puts on the travelling agent before the redirect
/// (`ASURA/crates/asura-world/src/travel.rs:19`, `TRAVEL_EFFECT`; the live capture's own `0x00F1` frame
/// carries it).
const TRAVEL_EFFECT: u32 = 0x100;

/// An IPv4 `host:port` as the four address bytes and the port, which is what a redirect carries.
fn parse_address(text: &str) -> Option<([u8; 4], u16)> {
    let (host, port) = text.rsplit_once(':')?;
    let octets: Vec<u8> = host
        .split('.')
        .map(|part| part.trim().parse::<u8>().ok())
        .collect::<Option<Vec<u8>>>()?;
    let octets: [u8; 4] = octets.as_slice().try_into().ok()?;
    Some((octets, port.trim().parse::<u16>().ok()?))
}

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

/// Where a claimed travel said to stand: the receiving map's arrival, and the facing it states.
///
/// `ASURA/crates/asura-world/src/travel.rs:359-362` writes the arrival's point as the character's
/// position before the redirect, and `portals::arrival_at` recovers its facing when the next connection
/// enters the map. Here both travel on the hand-off row, and the facing is looked up in the arrival rows
/// the world read with its maps.
#[derive(Debug, Clone, Copy)]
struct Claimed {
    /// The map the landing belongs to: a landing for another map is not this connection's.
    map_id: u32,
    /// The point, and the plane it is on.
    landing: crate::maps::travels::Landing,
    /// The direction the arriving agent is created facing, as `Arrival::direction` gives it.
    facing: (f32, f32),
}

/// The last client-reported position checked for a doorway, and the segment between two finite
/// reports.
///
/// `ASURA/crates/asura-world/src/player.rs:43-74`: `portal_position` and `last_step`. The doorway test
/// is the wall the step crosses, so the pair is what a crossing needs - not the agent's position, which
/// a simulation tick may already have advanced.
#[derive(Debug, Clone, Copy, Default)]
struct PortalStep {
    position: (f32, f32),
    last_step: Option<((f32, f32), (f32, f32))>,
}

/// `GAME_CMSG_ITEM_MOVE` (`0x8072`): a drag in the inventory - the item, the bag and the slot.
const CLIENT_ITEM_MOVE: u16 = 0x8072;
/// `GAME_CMSG_ITEM_DESTROY` (`0x8069`): the item to destroy.
const CLIENT_DROP_ITEM: u16 = 0x802C;
const CLIENT_UNEQUIP_ITEM: u16 = 0x804F;
const CLIENT_PING_REPLY: u16 = 0x8009;
const CLIENT_HEARTBEAT: u16 = 0x800C;
const CLIENT_DISCONNECT: u16 = 0x8008;
/// `GAME_CMSG_EQUIP_ITEM` (`0x8030`): the item to wear.
const CLIENT_EQUIP_ITEM: u16 = 0x8030;

/// The client's movement reports: `0x803D` carries the direction as well, `0x803E` the point a walk is
/// asked for, `0x8047` the stopping point of a cancel, and `0x8028` the cancel itself.
const CLIENT_MOVE_TO_COORD_WITH_DIR: u16 = 0x803D;
const CLIENT_MOVE_TO_COORD: u16 = 0x803E;
const CLIENT_LAST_POS_BEFORE_MOVE_CANCELED: u16 = 0x8047;
const CLIENT_CANCEL_MOVEMENT: u16 = 0x8028;

/// The client's in-game travel request - `GAME_CMSG_PARTY_TRAVEL`, `0x80B1`, nine bytes
/// (`ASURA/crates/asura-protocol/src/packets.rs`; the client's own frame is `B1 80 | <instance> | ...`).
/// This tree listened for `0x00B1`, which is the *server's* party message, until 2026-11-12.
const CLIENT_PARTY_TRAVEL: u16 = 0x80B1;

/// `GAME_CMSG_TRAVEL_INIT` (`0x80C1`, ten bytes): the client touched a travel object. Two agent ids;
/// `ASURA/crates/asura-world/src/dispatch.rs:73` answers it with `travel::handle_travel_init`.
const CLIENT_TRAVEL_INIT: u16 = 0x80C1;

/// `GAME_CMSG_SEND_CHAT_MESSAGE` (`0x8064`): the client speaks - `{agent u32, text String16}`, whose
/// first unit is the channel (`ASURA/crates/asura-protocol/src/packets.rs`, and
/// `ASURA/docs/reference/client-protocol.md:920-926`).
const CLIENT_CHAT_MESSAGE: u16 = 0x8064;

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
    /// The account the version message named, kept for the travel hand-off a redirect writes: the
    /// login database's travel row is keyed by account and player token.
    account: std::sync::Mutex<[u8; 16]>,
    /// Whether a travel is already in flight: the client is about to reconnect, so a second crossing
    /// must not start another one (`ASURA/crates/asura-world/src/travel.rs:600`, `player.moved`).
    moved: std::sync::Mutex<bool>,
    /// The map's walkable graphs, which a walk request is answered from: one finder per map, imported
    /// from the client archive at start-up (`ASURA/crates/asura-world/src/pathfind.rs:1484`). A map
    /// whose graph was never bound answers `Walk::NoGraph`.
    graphs: std::sync::Arc<crate::terrain::pathfind::PathGraphs>,
    /// The doorways a step is judged against, and the arrivals a crossing lands on.
    portals: std::sync::Arc<crate::maps::Portals>,
    /// The arrival points the world read with its maps (R21), so a crossing knows where the receiving
    /// map puts the character.
    arrivals: Vec<crate::maps::Arrival>,
    /// Where a claimed travel says the receiving map puts this character, when it was a doorway
    /// crossing: the load stands it there and faces it along the arrival, rather than scattering it
    /// around a spawn point (R13/R21).
    landing: std::sync::Mutex<Option<Claimed>>,
    /// The last client-reported position checked for a doorway, and the segment between finite
    /// reports: `ASURA/crates/asura-world/src/player.rs:43-74` (`portal_position`, `last_step`,
    /// `record_portal_step`). Simulation ticks never move it - a tick can cross a wall between
    /// reports, and that movement must remain in the next check.
    portal: std::sync::Mutex<PortalStep>,
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
    /// The players loaded in this world's running copies: a connection tells them it arrived, and is
    /// told when one of them does (game/Districts/Peers.rs).
    peers: std::sync::Arc<std::sync::Mutex<crate::districts::Peers>>,
    /// The frames other sessions pushed to this one, which it scrambles with its own cipher and writes
    /// when it next looks. A session that has not loaded a map has no channel at all.
    outbox: std::sync::Mutex<Option<std::sync::mpsc::Receiver<Vec<u8>>>>,
    /// The id this connection's player and agent are known by, drawn the moment a map load begins.
    ///
    /// One number for both, as every message in this tree already treats them: the load draws the body
    /// as `CHAR_CLASS_PLAYER_BASE | player`, the party records name the player, and the agent messages
    /// name the agent. It is **drawn per connection** (`Peers::allocate`), not a constant, because two
    /// players in one map are two players: a second connection that carried the first one's id would be
    /// the same body to every client. `None` until a real map is loaded - the creation instance has no
    /// population to be told apart in.
    id: std::sync::Mutex<Option<u32>>,
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
            account: std::sync::Mutex::new([0u8; 16]),
            moved: std::sync::Mutex::new(false),
            graphs: std::sync::Arc::clone(world.graphs()),
            portals: std::sync::Arc::clone(world.portals()),
            arrivals: world.arrivals().to_vec(),
            landing: std::sync::Mutex::new(None),
            portal: std::sync::Mutex::new(PortalStep::default()),
            characters: None,
            login: None,
            districts: world.districts(),
            entered: std::sync::Mutex::new(None),
            agent: std::sync::Mutex::new(None),
            creation: Some(world.creation().clone()),
            templates: world.templates().to_vec(),
            master: None,
            peers: world.peers(),
            outbox: std::sync::Mutex::new(None),
            id: std::sync::Mutex::new(None),
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

    /// The bags this world's content gives a character, or none when the world read no creation data.
    fn bags(&self) -> Vec<crate::creation::Bag> {
        self.creation
            .as_ref()
            .map(|tables| crate::creation::bags(tables).to_vec())
            .unwrap_or_default()
    }

    /// The character database, when the program opened one.
    fn characters(&self) -> Option<crate::characters::DatabaseCharacters> {
        self.characters.as_ref().map(|pool| {
            crate::characters::DatabaseCharacters::new(
                std::sync::Arc::clone(pool),
                self.log.clone(),
            )
        })
    }

    /// `GAME_CMSG_ITEM_MOVE` (`0x8072`): a drag in the inventory.
    ///
    /// Ported from `ASURA/crates/asura-world/src/item.rs:255-312`, whole: the row moves when the
    /// client's placement is one it may have, a refused move is answered with where the item **really
    /// is** - the client moved it in its own model before asking - and the agent's worn items are sent
    /// again when the equipped set changed, because that is the one thing the client cannot work out for
    /// itself.
    async fn answer_item_move(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let Some((item_id, bag_id, slot)) = item_move(message) else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an item move of {} bytes cannot be read.",
                    message.len()
                ),
            );
            return;
        };
        let Some(characters) = self.characters() else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an item move was asked for, and this world has no character database."
                ),
            );
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let bags = self.bags();
        let before = characters.equipped_item_ids(&uuid, &bags).unwrap_or([0u32; 9]);

        let mut frames = Vec::new();
        match characters.move_item(&uuid, item_id, bag_id, slot, &bags) {
            Ok(crate::characters::MoveOutcome::At { bag_id, slot }) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} moved to bag {bag_id} slot {slot}."
                    ),
                );
                frames.push(item_packets::item_move_to_location(
                    item_packets::ITEM_STREAM,
                    item_id,
                    bag_id,
                    slot,
                ));
            }
            Ok(crate::characters::MoveOutcome::Unchanged) => {}
            Ok(crate::characters::MoveOutcome::Refused { place, reason }) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} to bag {bag_id} slot {slot} refused: {reason:?}."
                    ),
                );
                // The client moved it in its own model before asking, so put it back.
                if let Some((bag, slot)) = place {
                    frames.push(item_packets::item_move_to_location(
                        item_packets::ITEM_STREAM,
                        item_id,
                        bag,
                        slot,
                    ));
                }
            }
            Err(error) => self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: the item move could not be written: {error}"),
            ),
        }
        if characters.equipped_item_ids(&uuid, &bags).unwrap_or(before) != before {
            frames.push(self.equipment_frame(&uuid, &bags, &characters));
        }
        if !frames.is_empty() {
            self.send(io, peer, crypt, &frames, "the item's new place").await;
        }
    }

    /// `GAME_CMSG_EQUIP_ITEM` (`0x8030`): wear an item, evicting whatever the equipped slot held.
    ///
    /// Ported from `ASURA/crates/asura-world/src/item.rs:314-436`, whole: the item must be the
    /// character's, its kind must be one that is worn, the equipped bag must exist, whatever is in that
    /// slot comes off to the first free carrying slot first (and with nowhere to put it, nothing is
    /// worn), and the agent's worn items are sent again when the set changed.
    async fn answer_equip_item(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
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
            return;
        };
        let Some(characters) = self.characters() else {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: an equip was asked for, and this world has no character database."
                ),
            );
            return;
        };
        let Some((bag_id, slot)) = self.equip_place(&characters, sheet, item_id) else {
            return;
        };
        let uuid = crate::characters::uuid_text(&sheet.uuid);
        let bags = self.bags();
        let before = characters.equipped_item_ids(&uuid, &bags).unwrap_or([0u32; 9]);
        let mut frames = Vec::new();

        // Whatever is worn in that slot has to come off first, and there has to be somewhere to put it.
        let occupant = characters
            .items_of(&uuid)
            .unwrap_or_default()
            .into_iter()
            .find(|row| row.bag_id == bag_id && row.slot == slot && row.item_id != item_id);
        if let Some(occupant) = occupant {
            let Some((to_bag, to_slot)) = characters
                .first_free_carrying_slot(&uuid, &bags)
                .unwrap_or_default()
            else {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: no free carrying slot to take item {} off, so item {item_id} cannot be worn.",
                        occupant.item_id
                    ),
                );
                return;
            };
            match characters.move_item(&uuid, occupant.item_id, to_bag, to_slot, &bags) {
                Ok(crate::characters::MoveOutcome::At { bag_id, slot }) => {
                    self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: item {} taken off to bag {bag_id} slot {slot}.",
                            occupant.item_id
                        ),
                    );
                    frames.push(item_packets::item_move_to_location(
                        item_packets::ITEM_STREAM,
                        occupant.item_id,
                        bag_id,
                        slot,
                    ));
                }
                outcome => {
                    self.say(
                        LogLevel::Warn,
                        format!(
                            "Gameplay connection from {peer}: could not take item {} off to make room for {item_id}: {outcome:?}",
                            occupant.item_id
                        ),
                    );
                    return;
                }
            }
        }

        match characters.move_item(&uuid, item_id, bag_id, slot, &bags) {
            Ok(crate::characters::MoveOutcome::At { bag_id, slot }) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: item {item_id} worn in equipped slot {slot} of bag {bag_id}."
                    ),
                );
                frames.push(item_packets::item_move_to_location(
                    item_packets::ITEM_STREAM,
                    item_id,
                    bag_id,
                    slot,
                ));
            }
            Ok(crate::characters::MoveOutcome::Unchanged) => {}
            Ok(crate::characters::MoveOutcome::Refused { place, reason }) => {
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: equipping item {item_id} refused: {reason:?}."
                    ),
                );
                if let Some((bag, slot)) = place {
                    frames.push(item_packets::item_move_to_location(
                        item_packets::ITEM_STREAM,
                        item_id,
                        bag,
                        slot,
                    ));
                }
            }
            Err(error) => self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: the equip could not be written: {error}"),
            ),
        }

        if characters.equipped_item_ids(&uuid, &bags).unwrap_or(before) != before {
            frames.push(self.equipment_frame(&uuid, &bags, &characters));
        }
        if !frames.is_empty() {
            self.send(io, peer, crypt, &frames, "the item's new place").await;
        }
    }

    /// `UPDATE_AGENT_VISUAL_EQUIPMENT` for this character: the nine ids the agent is drawn wearing.
    ///
    /// `ASURA/crates/asura-world/src/item.rs:438-440`, `equipment_frame`. A character whose rows cannot
    /// be read is drawn wearing nothing, which is what the load does with the same failure.
    fn equipment_frame(
        &self,
        uuid: &str,
        bags: &[crate::creation::Bag],
        characters: &crate::characters::DatabaseCharacters,
    ) -> Vec<u8> {
        let equipped = characters.equipped_item_ids(uuid, bags).unwrap_or([0u32; 9]);
        misc_packets::update_agent_visual_equipment(self.player_id(), equipped)
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

        if let Ok(mut account) = self.account.lock() {
            *account = version.account;
        }

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
                                "Gameplay connection from {peer}: the travel record is claimed - map {}, district {}, language {}{}.",
                                travel.map_id,
                                travel.district,
                                travel.language,
                                match travel.landing {
                                    Some(landing) => format!(
                                        ", landing at ({:.1}, {:.1}) plane {} from map {}",
                                        landing.x, landing.y, landing.plane, landing.from_map
                                    ),
                                    None => String::new(),
                                }
                            ),
                        );
                        // A crossing states where the receiving map puts the character; the load
                        // stands it there instead of scattering it around a spawn point (R13/R21,
                        // `ASURA/crates/asura-world/src/travel.rs:446-468`), and faces it along the
                        // arrival the receiving map states for the map it came from.
                        if let (Some(landing), Ok(mut claimed)) = (travel.landing, self.landing.lock())
                        {
                            let facing = self
                                .arrivals
                                .iter()
                                .find(|arrival| {
                                    arrival.map_id == travel.map_id
                                        && arrival.from_map == landing.from_map
                                })
                                .map(|arrival| arrival.direction())
                                .unwrap_or((1.0, 0.0));
                            *claimed = Some(Claimed {
                                map_id: travel.map_id,
                                landing,
                                facing,
                            });
                        }
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
            // 1 for every load: the reference hardcodes it (`ASURA/crates/asura-world/src/map.rs:34-42`).
            let is_explorable = 1;
            let mut frames = vec![
                instance_packets::instance_load_head(),
                instance_packets::instance_player_data_start(),
                instance_packets::instance_load_player_name(&sheet.name),
                instance_packets::instance_load_info(
                    self.player_id(),
                    map.id as u16,
                    is_explorable,
                    district,
                    language,
                    0,
                ),
            ];
            // The exploration grid, right after the instance's record: that is where a live server
            // sends it, once per instance (`ASURA/crates/asura-world/src/map.rs:22-48`), and a client
            // without it asserts on its own world map the first time the map view opens
            // (`GmMapView.cpp(1731)`, client build 39112).
            let grid = crate::maps::cartography::init_frames(crate::maps::cartography::grid_for(
                map.id,
            ));
            frames.extend(grid);
            // And the destinations it can resolve, in the batches a load uses - the working
            // implementation sends these in the same phase (`ASURA/crates/asura-world/src/map.rs:49-58`).
            let (table, notes) = crate::maps::instance_table::table_for(&self.maps);
            for note in &notes {
                self.say(
                    LogLevel::Warn,
                    format!("Gameplay connection from {peer}: instance table: {note}"),
                );
            }
            frames.extend(table.frames());
            let mut block = Vec::new();
            for frame in &frames {
                block.extend_from_slice(frame);
            }
            crypt.encrypt(&mut block);
            match io.write_all(&block).await {
                Ok(()) => self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: {} is loading - {} message(s), {} bytes: the load head, the player-data start, the name '{}', the instance's record (district {district}, language {language}), the exploration grid ({} x {}, nothing explored) and the instance table ({} pair(s)).",
                        map.label(),
                        frames.len(),
                        block.len(),
                        sheet.name,
                        crate::maps::cartography::grid_for(map.id).0,
                        crate::maps::cartography::grid_for(map.id).1,
                        table.pairs.len()
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
                    self.player_id(),
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
            // The client is going away (GAME_CMSG_DISCONNECT, 0x8008, two bytes): the working
            // implementation's dispatch ends the connection on it
            // (ASURA/crates/asura-world/src/dispatch.rs:86).
            CLIENT_DISCONNECT => Some(false),
            // `GAME_CMSG_PING_REQUEST` (`0x800D`): the client asks to be pinged back, and the answer is
            // the ping reply - `0x000D` and the value this world runs with, **not** the request echoed
            // (`ASURA/crates/asura-world/src/connection.rs:579-587`, `handle_ping_request`).
            GAME_PING => {
                self.send(
                    io,
                    peer,
                    crypt,
                    &[misc_packets::ping_reply(self.settings.ping_value)],
                    "the ping reply",
                )
                .await;
                Some(true)
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

    /// Preserve the full reported movement even when simulation ticks advanced the agent meanwhile.
    ///
    /// `ASURA/crates/asura-world/src/player.rs:60-74`, `record_portal_step`: the segment between two
    /// finite client reports, and nothing at all when the client reported the same point again.
    fn record_portal_step(&self, position: (f32, f32)) {
        let Ok(mut portal) = self.portal.lock() else {
            return;
        };
        portal.last_step =
            (portal.position != position).then_some((portal.position, position));
        portal.position = position;
    }

    /// Where the character entered: the first report is measured from here, not from nowhere.
    ///
    /// `ASURA/crates/asura-world/src/connection.rs:318-319`: `portal_position` starts at the standing
    /// position the load put the agent on, so a step across a doorway at the entry point is a step.
    fn start_portal_position(&self, position: (f32, f32)) {
        if let Ok(mut portal) = self.portal.lock() {
            portal.position = position;
            portal.last_step = None;
        }
    }

    /// Take the segment the doorway test consumes, if a report left one.
    fn take_portal_step(&self) -> Option<((f32, f32), (f32, f32))> {
        self.portal.lock().ok().and_then(|mut portal| portal.last_step.take())
    }

    /// A crossing decided from the player's **own step**: where the character stood before this report
    /// and where it stands after it.
    ///
    /// `ASURA/crates/asura-world/src/travel.rs:319-388` (`portal_crossed`), ported whole: a crossing
    /// already in flight owns the connection, the map's own arches decide whether the step crosses a
    /// wall, an arch that names no destination does nothing and says so, and the receiving map's
    /// arrival decides where the traveller stands. It ends in the same `begin_transfer` every other
    /// trigger ends in, because the reference has one cycle and not one per trigger.
    async fn cross_portal(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
        from: (f32, f32),
        to: (f32, f32),
    ) {
        if let Ok(moved) = self.moved.lock()
            && *moved
        {
            return;
        }
        let from_map = self.map_id();
        let Some(crossed) = self.portals.crossed(from_map, from, to) else {
            return;
        };
        let Some(to_map) = crossed.to_map else {
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: the character crossed a doorway of map {from_map} from ({:.0}, {:.0}) to ({:.0}, {:.0}); no destination is recorded for that doorway.",
                    from.0, from.1, to.0, to.1
                ),
            );
            return;
        };
        // The receiving map decides where the traveller stands: an arrival it states for this map is a
        // position and a facing; without one the destination falls back to its own spawn point.
        let arrival = self
            .arrivals
            .iter()
            .find(|arrival| arrival.map_id == to_map && arrival.from_map == from_map)
            .copied();
        let landing = arrival.map(|arrival| (arrival.x, arrival.y));
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: the character crossed the doorway of map {from_map} at ({:.0}, {:.0}) -> ({:.0}, {:.0}): crossing to {to_map}{}",
                from.0,
                from.1,
                to.0,
                to.1,
                match arrival {
                    Some(arrival) => format!(
                        " (arrival at ({:.1}, {:.1}) facing {:.2})",
                        arrival.x,
                        arrival.y,
                        arrival.angle.unwrap_or(0.0)
                    ),
                    None => " (no arrival stated; the destination's spawn point answers)".to_string(),
                }
            ),
        );
        let Some(destination) = self.maps.iter().find(|map| map.id == to_map).cloned() else {
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: instance {to_map} cannot be served by this world; the crossing is refused."
                ),
            );
            return;
        };
        self.begin_transfer(io, peer, crypt, sheet, &destination, landing)
            .await;
    }

    /// `GAME_CMSG_PARTY_TRAVEL` (`0x80B1`, nine bytes): the client asks for an instance from the
    /// world map.
    ///
    /// `ASURA/crates/asura-world/src/travel.rs`, `handle_party_travel`: the first two bytes are the
    /// instance it wants; a map this world does not host is refused by saying so, and a map it hosts goes
    /// to `begin_transfer`. The nine bytes are the client's own frame - `B1 80 | 94 00 | 02 | 00 00 | 0...`
    /// in the working implementation's log - of which only the instance is read here, exactly as there.
    async fn answer_party_travel(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let body = &message[2..];
        if body.len() < 2 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a travel request is {} bytes and names no instance; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let map_id = u32::from(u16::from_le_bytes([body[0], body[1]]));
        let Some(destination) = self.maps.iter().find(|map| map.id == map_id).cloned() else {
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: instance {map_id} cannot be served by this world; the travel is refused."
                ),
            );
            return;
        };
        self.begin_transfer(io, peer, crypt, sheet, &destination, None).await;
    }

    /// The frames a travel puts on the wire, in the working implementation's own order
    /// (`ASURA/crates/asura-world/src/travel.rs:480-518`): the player's agent takes the travel effect,
    /// its walk stops, the redirect names where to reconnect and with what token, the map update follows
    /// in its twelve-byte form, and the travelling body is withdrawn from the map it is leaving - the party
    /// size, the party, the player and the agent.
    async fn begin_transfer(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
        destination: &crate::world::MapEntry,
        landing: Option<(f32, f32)>,
    ) {
        // A travel already in flight owns this connection: the client is about to reconnect, and a
        // second redirect would send it two.
        match self.moved.lock() {
            Ok(mut moved) if !*moved => *moved = true,
            _ => return,
        }

        // The address the client will dial. The working implementation publishes one
        // (`--public`, `ASURA/crates/asura-world/src/main.rs:73`); this world states it in its own file,
        // and this stack's client dials the address it was launched against (`docs/port/02-wire.md`).
        let Some((host, port)) = parse_address(&self.settings.public_address) else {
            self.say(
                LogLevel::Error,
                format!(
                    "Gameplay connection from {peer}: World.PublicAddress is '{}', which is not an IPv4 address and port; the travel is refused.",
                    self.settings.public_address
                ),
            );
            return;
        };

        // A number drawn per travel, so the world can tell one client's connection from another's: the
        // arriving connection presents it, and the hand-off row is what names the map it is for.
        let mut token_bytes = [0u8; 4];
        if getrandom::getrandom(&mut token_bytes).is_err() {
            self.say(
                LogLevel::Warn,
                format!("Gameplay connection from {peer}: no entropy for the player token; using zero."),
            );
        }
        let player_token = u32::from_le_bytes(token_bytes);
        let account = match self.account.lock() {
            Ok(account) => *account,
            Err(_) => [0u8; 16],
        };
        let language = 0;
        let district = 0;

        // The hand-off, written before the redirect: the connection the client is about to open reads it
        // to learn the map and the district. It is the login database's row, because that is where this
        // stack's login writes the one a client asks for (`docs/port/06-world-plan.md`, "The three seams").
        let mut recorded = false;
        if let Some(login) = self.login.as_ref() {
            let travels = crate::maps::DatabaseTravels::new(
                std::sync::Arc::clone(login),
                self.log.clone(),
            );
            let travel = crate::maps::Travel {
                account,
                world: self.realm,
                map_id: destination.id,
                district,
                language,
                player_token,
                // The landing a crossing hands over; every other trigger states none, which clears
                // the saved position rather than keeping it (travel.rs:396-400, 446-468).
                landing: landing.map(|(x, y)| crate::maps::travels::Landing {
                    x,
                    y,
                    plane: 0,
                    from_map: self.map_id(),
                }),
            };
            match travels.record(&travel) {
                Ok(()) => {
                    recorded = true;
                    self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: the travel to map {} is recorded for world {}; the world claims it when the connection presents token 0x{player_token:08X}.",
                            destination.id, self.realm
                        ),
                    );
                }
                Err(error) => self.say(
                    LogLevel::Error,
                    format!(
                        "Gameplay connection from {peer}: the travel to map {} was not recorded ({error}); the client would reconnect with no hand-off, so the redirect is not sent.",
                        destination.id
                    ),
                ),
            }
        }
        if !recorded {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a travel to map {} cannot be handed off without the login database; the connection stays where it is.",
                    destination.id
                ),
            );
            return;
        }

        let agent = self.player_id();
        let player = self.player_id();
        let is_explorable = self.settings.travel_explorable.flag(destination.kind);
        let frames = vec![
            agent_packets::agent_update_effects(agent, TRAVEL_EFFECT).1,
            agent_packets::agent_stop_moving(agent).1,
            misc_packets::transfer_game_server_info(
                host,
                port,
                self.realm,
                0, // region: the reference sends its district's region, and this world has no regions
                player_token,
                is_explorable,
                destination.id,
            )
            .1,
            misc_packets::map_update_current_redirect(destination.id).1,
            party_packets::player_update_party_size(player, 1),
            party_packets::player_update_party(player, 0),
            character_packets::player_destroy(player).1,
            agent_packets::world_remove_agent(agent).1,
        ];
        self.send(io, peer, crypt, &frames, "the redirect").await;
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: the character '{}' travels to map {} ('{}'); the client is told to dial {}.{}.{}.{}:{} with token 0x{player_token:08X}.",
                sheet.name,
                destination.id,
                destination.name,
                host[0],
                host[1],
                host[2],
                host[3],
                port
            ),
        );
    }

    /// `GAME_CMSG_TRAVEL_INIT` (`0x80C1`, ten bytes): the client touched a travel object - a portal, a
    /// door, a signpost.
    ///
    /// `ASURA/crates/asura-world/src/travel.rs:132-169` (`handle_travel_init`), ported whole: the two
    /// agent ids are read - the object the player used, and a second one that has been zero in every
    /// measurement - and the touch is named in the log. **It names an agent, not a map**, so the
    /// destination is this server's knowledge and not the client's: the object it names leads somewhere
    /// only because a row says so. The doorway crossing itself is decided from the player's own step
    /// (`travel.rs:319-388`, `portal_crossed`), not from this message, so this arm starts no transfer -
    /// which is exactly what it does in the working implementation.
    async fn answer_travel_init(&self, peer: &str, message: &[u8], sheet: &CharacterSheet) {
        let body = &message[2..];
        if body.len() < 8 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a travel touch is {} bytes and needs 10; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let agent = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        let other = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
        self.say(
            LogLevel::Info,
            format!(
                "Gameplay connection from {peer}: the character '{}' touched a travel object in map {}: TRAVEL_INIT agent 0x{agent:08X}, second 0x{other:08X}; no portal row names that agent, so no travel starts.",
                sheet.name,
                self.map_id()
            ),
        );
    }

    /// `/tp <alias>` and `/teleport <alias>`: the reference's `GameSrv_HandleTeleportCommand`
    /// (`GmCommands.c:155-216`), ported whole from
    /// `ASURA/crates/asura-world/src/travel.rs:184-258` (`teleport`).
    ///
    /// A teleport is not a position change - it is a **server transfer**, the same cycle a portal
    /// starts, so it ends in `begin_transfer` exactly as the client's own request does. The reference's
    /// three refusals are here verbatim, in its order: an argument that names nothing, an alias its
    /// table does not list (`Unknown map name. Type /maps.`), a target with no map config, a target
    /// this world has no row for, and finally the command's own "You are already here."
    ///
    /// What is not here, and is named rather than hidden: the working implementation also loads a
    /// destination on demand (`--on-demand`, `travel.rs:216-247`); this world has no such switch, so a
    /// map it has no row for is refused instead of loaded.
    async fn answer_teleport_command(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
        argument: &str,
    ) {
        if crate::chat::alias_key(argument).is_empty() {
            // `GmCommand_ToAscii` refuses an empty argument, and the answer is the invalid-command line.
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: /tp without a map name."),
            );
            return;
        }
        // The alias table, not the map names: the reference accepts `lakeside` as well as
        // `lakeside county`, so matching against a map's own name would refuse half the aliases its
        // own `/maps` prints.
        let Some(target) = crate::chat::travel_target(argument) else {
            // `GmCommands.c:172-177`: "Unknown map name. Type /maps." through the gated chat sender,
            // which with the gate off is a log line.
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: Unknown map name. Type /maps."),
            );
            return;
        };
        if !target.hosted {
            // `GmCommands.c:180-190`: the reference refuses a target with no `GmMapConfig` rather than
            // letting its map load fall back to Kamadan.
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: That map has no config; the server would fall back, so the teleport is refused ({}).",
                    target.label
                ),
            );
            return;
        }
        let Some(destination) = self
            .maps
            .iter()
            .find(|map| map.id == target.map_id)
            .cloned()
        else {
            // The table knows the map; this world hosts no row for it. The reference pre-spawns one
            // server per map, so its answer would be a transfer; this one can only say so.
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: {} ({}) is not hosted by this world; the teleport is refused.",
                    target.label, target.map_id
                ),
            );
            return;
        };
        if target.map_id == self.map_id() {
            // `GmCommands.c:198-202`. **The command's own refusal, not a travel policy**: the
            // client-driven requests (`0x80B1`) are carried out without this check, because there the
            // client has already decided and a second opinion is a gate. A typed alias that names the
            // instance the character stands in is the reference's "You are already here." and nothing
            // more.
            self.say(
                LogLevel::Info,
                format!("Gameplay connection from {peer}: You are already here."),
            );
            return;
        }
        self.begin_transfer(io, peer, crypt, sheet, &destination, None).await;
    }

    /// `GAME_CMSG_SEND_CHAT_MESSAGE` (`0x8064`): the client speaks.
    ///
    /// Ported whole from `ASURA/crates/asura-world/src/chat.rs:193-353` (`handle_chat_message`). The
    /// frame is `{agent u32, text String16}` - the table's own field list
    /// (`ASURA/docs/reference/client-protocol.md:920-926`), which is why the text's count sits four
    /// bytes in - and the **first unit of the text is the channel**: `!` all, `#` group, `$` trade,
    /// `/` a command, `@`/`%`/`"` dropped as the reference drops them, and anything else refused.
    ///
    /// A player's line is **sent**, exactly as the reference sends it: the text (`0x005D`) and who spoke
    /// it (`0x0061`), because the reference's own chat gate covers the text the *server* generates and
    /// not a line a player typed. Every answer the server generates is printed instead - `/maps`' table
    /// and every refusal - because that is what the reference's gate does with them.
    async fn answer_chat_message(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
        sheet: &CharacterSheet,
    ) {
        let body = &message[2..];
        if body.len() < 6 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a chat line is {} bytes and carries no text; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let units = u16::from_le_bytes([body[4], body[5]]) as usize;
        let text: Vec<u16> = (0..units)
            .map(|index| {
                let at = 6 + index * 2;
                u16::from_le_bytes([
                    body.get(at).copied().unwrap_or(0),
                    body.get(at + 1).copied().unwrap_or(0),
                ])
            })
            .collect();
        let Some(channel) = text.first().copied() else {
            // An empty message, which the reference drops.
            return;
        };
        let spoken: String = char::decode_utf16(text[1.min(text.len())..].iter().copied())
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();

        match u8::try_from(channel).unwrap_or(0) as char {
            // `Channel_All`, `Channel_Group`, `Channel_Trade`: the three channels the reference speaks
            // on. Sent ungated, as the reference sends them.
            speaker @ ('!' | '#' | '$') => {
                let channel = match speaker {
                    '#' => chat_packets::CHANNEL_GROUP,
                    '$' => chat_packets::CHANNEL_TRADE,
                    _ => chat_packets::CHANNEL_ALL,
                };
                // The player's id and its agent's are one number here (`self.player_id()`, the id the
                // load drew the body as); the working implementation reads them from its player.
                let frames = vec![
                    chat_packets::chat_message_core(&spoken),
                    chat_packets::chat_message_local(self.player_id(), channel),
                ];
                self.send(io, peer, crypt, &frames, "a spoken line").await;
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: {} spoke {spoken:?} on channel {channel}.",
                        sheet.name
                    ),
                );
            }
            '/' => {
                // The command surface, from the reference's own two places: `GmCommands.c`'s
                // `GameSrv_TryHandleGmCommand` (`/tp`, `/teleport`, `/maps`) and `GmChat.c`'s
                // `GameSrv_HandleChatCommand` (`/stuck`). `crate::chat::COMMANDS` names them once.
                let spoken = spoken.trim();
                let (name, argument) = match spoken.split_once(' ') {
                    Some((name, rest)) => (name, rest.trim_start()),
                    None => (spoken, ""),
                };
                match crate::chat::command(name) {
                    Some(named) if named.names.contains(&"stuck") && argument.is_empty() => {
                        // `GameSrv_HandleChatCommand`'s `stuck`: broadcast where the agent is now.
                        // **Not** `AGENT_STOP_MOVING` - that is what `0x8028` answers - and the working
                        // implementation sends the position alone (`chat.rs:260-287`); the tick it also
                        // broadcasts is the ticker's own, so no second clock is invented.
                        let Some(frame) = self.with_agent(|agent| {
                            agent_packets::agent_update_position(
                                agent.agent_id,
                                agent.position,
                                agent.plane,
                            )
                            .1
                        }) else {
                            return;
                        };
                        self.send(io, peer, crypt, &[frame], "where the agent stands")
                            .await;
                    }
                    Some(named) if named.names.contains(&"maps") && argument.is_empty() => {
                        // `GmCommand_ListTargets`, as it behaves with the reference's own defaults: the
                        // table goes to the log, and nothing goes on the wire. That is parity, not a
                        // gate - sending it crashed a live client, which is why the answer is printed
                        // (`chat.rs:288-324`). The chat half is written too, and printed at debug,
                        // which is the level this tree's traces are read at.
                        let answer = crate::chat::maps_answer(&self.maps);
                        for line in &answer.table {
                            self.say(LogLevel::Info, format!("Gameplay connection from {peer}: {line}"));
                        }
                        for line in &answer.chat {
                            self.say(
                                LogLevel::Debug,
                                format!(
                                    "Gameplay connection from {peer}: /maps (chat, not sent): {line}"
                                ),
                            );
                        }
                    }
                    Some(named) if named.names.contains(&"tp") && !argument.is_empty() => {
                        self.answer_teleport_command(io, peer, crypt, sheet, argument)
                            .await;
                    }
                    _ => {
                        // `GameSrv_SendInvalidCommand`: the reference's text is "Unknown command",
                        // which covers `/tp` with no map name and every name not in the table; with its
                        // chat gate off it is a log line and nothing else.
                        self.say(
                            LogLevel::Info,
                            format!("Gameplay connection from {peer}: Unknown command"),
                        );
                    }
                }
            }
            other => {
                // `@`, `%` and `"` are ignored by the reference; anything else it refuses.
                if matches!(other, '@' | '%' | '"') {
                    self.say(
                        LogLevel::Debug,
                        format!(
                            "Gameplay connection from {peer}: channel {other:?} ignored, as the reference ignores it."
                        ),
                    );
                } else {
                    self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: chat on channel {other:?} refused, as the reference refuses it."
                        ),
                    );
                }
            }
        }
    }

    /// `GAME_CMSG_MOVE_TO_COORD` (`0x803E`): the agent is asked to walk to a point.
    ///
    /// `ASURA/crates/asura-world/src/agent.rs:710` (`handle_move_to_coord`), ported whole: the finder
    /// is asked for a path from where the agent stands; an empty path sends **nothing** (the
    /// reference's own `if (0 < agent->waypoints.len)`), and a refusal also sends nothing, because the
    /// one message that could answer it - `0x002C` with the server's own copy - disconnected a client
    /// with `007` once and teleported it into a wall once (`agent.rs:864-871`). A path that is found is
    /// densified to the live cadence (`densify`, `agent.rs:646`) and answered with `0x0029`, which
    /// every other client in the map is told as well.
    async fn answer_move_to_coord(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let body = &message[2..];
        if body.len() < 12 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a walk request is {} bytes and its point needs 16; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let destination = (
            f32::from_le_bytes([body[0], body[1], body[2], body[3]]),
            f32::from_le_bytes([body[4], body[5], body[6], body[7]]),
        );
        let plane = u32::from_le_bytes([body[8], body[9], body[10], body[11]]) as u16;
        let to = crate::terrain::Pos {
            x: destination.0,
            y: destination.1,
            plane,
        };
        let Some((agent_id, from, planes)) = self.with_agent(|agent| {
            (agent.agent_id, agent.here(), agent.plane)
        }) else {
            return;
        };
        let walked = self.graphs.walk(self.map_id() as i32, from, to);
        match walked {
            crate::terrain::Walk::Found(waypoints) => {
                let waypoints = crate::agents::densify(waypoints, from, crate::agents::LIVE_CADENCE_STEP);
                let begun = self.with_agent(|agent| {
                    let speed = (agent.move_type != 1 || agent.move_speed != 1.0)
                        .then(|| agent_packets::agent_update_speed(agent_id, 1.0, 1));
                    agent.move_type = 1;
                    agent.move_speed = 1.0;
                    let first = agent.begin_walk(waypoints)?;
                    Some((
                        agent_packets::agent_move_to_point(
                            agent_id,
                            (first.pos.x, first.pos.y),
                            first.pos.plane,
                            agent.plane,
                        ),
                        speed,
                    ))
                });
                if let Some((walk, speed)) = begun.flatten() {
                    let mut reply: Vec<Vec<u8>> = speed.into_iter().map(|(_, f)| f).collect();
                    reply.push(walk.1);
                    // Everyone else is told where this agent is walking: the reference broadcasts the
                    // same message (`ASURA/crates/asura-world/src/agent.rs:849-855`,
                    // `GameSrv_BroadcastAgentMoveToPoint`), and a client that is not told keeps drawing
                    // the walker where it last stood.
                    if let Ok(peers) = self.peers.lock() {
                        peers.broadcast(self.player_id(), self.map_id(), self.district(), &reply);
                    }
                    self.send(io, peer, crypt, &reply, "the walk").await;
                }
            }
            crate::terrain::Walk::NoPath | crate::terrain::Walk::NoGraph => {
                self.with_agent(|agent| agent.cancel_movement());
                self.say(
                    LogLevel::Info,
                    format!(
                        "Gameplay connection from {peer}: agent {agent_id} cannot walk from ({:.1}, {:.1}) plane {} to ({:.1}, {:.1}) plane {plane}: standing still, and nothing is sent.",
                        from.x, from.y, planes, destination.0, destination.1
                    ),
                );
            }
        }
    }

    /// `GAME_CMSG_POSITION_UPDATE` (`0x803D`): where the client is, and the offset it is walking in.
    ///
    /// `ASURA/crates/asura-world/src/agent.rs:887` (`handle_position_update`), ported whole: the report
    /// is adopted under its elapsed-time budget, a manual walk follows the offset through the map's
    /// connected cells (`walk_direction`), and the answer is the direction, the movement type and the
    /// destination - the live capture's own order (`agent.rs:1000-1029`). A report that is not finite,
    /// or whose type is outside 1-8, is dropped exactly as it is there (`agent.rs:923-929`).
    async fn answer_position_update(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let body = &message[2..];
        if body.len() < 24 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a position report is {} bytes and needs 26; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let position = (
            f32::from_le_bytes([body[0], body[1], body[2], body[3]]),
            f32::from_le_bytes([body[4], body[5], body[6], body[7]]),
        );
        let plane = u32::from_le_bytes([body[8], body[9], body[10], body[11]]) as u16;
        let offset = (
            f32::from_le_bytes([body[12], body[13], body[14], body[15]]),
            f32::from_le_bytes([body[16], body[17], body[18], body[19]]),
        );
        let move_type = u32::from_le_bytes([body[20], body[21], body[22], body[23]]) as u8;
        if ![position.0, position.1, offset.0, offset.1]
            .iter()
            .all(|value| value.is_finite())
            || !(1..=8).contains(&move_type)
        {
            return;
        }
        let now = std::time::Instant::now();
        let Some((adopted, walk_was_running, reply)) = self.with_agent(|agent| {
            let walk_was_running = 0.0 < agent.motion.speed;
            let adopted = agent.adopt_report(position, plane, now);
            let reply = if adopted != crate::agents::Adopted::Refused {
                let from = agent.here();
                let walked = self.graphs.walk_direction(
                    self.map_id() as i32,
                    from,
                    offset,
                    crate::terrain::pathfind::directional::DirectionalSettings::default(),
                );
                let waypoints = match walked {
                    crate::terrain::Walk::Found(waypoints) => waypoints,
                    crate::terrain::Walk::NoPath | crate::terrain::Walk::NoGraph => Vec::new(),
                };
                let norm = (offset.0 * offset.0 + offset.1 * offset.1).sqrt();
                let direction = if norm > 0.0 {
                    (offset.0 / norm, offset.1 / norm)
                } else {
                    (0.0, 0.0)
                };
                agent.begin_manual_walk(waypoints, direction, move_type)
            } else {
                Vec::new()
            };
            (adopted, walk_was_running, reply)
        }) else {
            return;
        };
        let reply: Vec<Vec<u8>> = reply.into_iter().map(|(_, frame)| frame).collect();
        self.say(
            LogLevel::Debug,
            format!(
                "Gameplay connection from {peer}: a position report ({:.1}, {:.1}) plane {plane}: {adopted:?}, server walk running={walk_was_running}, {} message(s) answered.",
                position.0,
                position.1,
                reply.len()
            ),
        );
        // The step this report moved the character along: the doorway test is the wall the step
        // crosses, not the position a tick may already have advanced
        // (`ASURA/crates/asura-world/src/agent.rs:1038-1041`, `record_portal_step`).
        self.record_portal_step(position);
        if !reply.is_empty() {
            // Everyone else is told what this report made the agent do, before the walker itself is
            // (`ASURA/crates/asura-world/src/agent.rs:1041-1042`: `instance.broadcast`, then `send`).
            if let Ok(peers) = self.peers.lock() {
                peers.broadcast(self.player_id(), self.map_id(), self.district(), &reply);
            }
            self.send(io, peer, crypt, &reply, "the walk the report asked for")
                .await;
        }
    }

    /// `GAME_CMSG_CANCEL_MOVEMENT` (`0x8028`): the client stopped walking.
    ///
    /// `ASURA/crates/asura-world/src/agent.rs:1062` (`handle_cancel_movement`): the agent's own walk is
    /// cancelled - destination to where it stands, speed to zero - and every client is told it stopped
    /// with `0x0028`. Cancelling is the load-bearing half: without it the server keeps advancing its
    /// copy along the abandoned path and answers the next click from a position the client left behind.
    async fn answer_cancel_movement(
        &self,
        io: &mut SessionIo,
        peer: &str,
        crypt: &mut AuthCrypt,
    ) {
        let Some((stop, was_walking)) = self.with_agent(|agent| {
            let was_walking = 0.0 < agent.motion.speed;
            agent.cancel_movement();
            (agent_packets::agent_stop_moving(agent.agent_id), was_walking)
        }) else {
            return;
        };
        self.say(
            LogLevel::Debug,
            format!(
                "Gameplay connection from {peer}: the client cancelled walking{}.",
                if was_walking {
                    " and a server-side walk was running: cancelled"
                } else {
                    ""
                }
            ),
        );
        // Every client is told it stopped, before the walker itself is
        // (`ASURA/crates/asura-world/src/agent.rs:1103-1104`).
        if let Ok(peers) = self.peers.lock() {
            peers.broadcast(
                self.player_id(),
                self.map_id(),
                self.district(),
                std::slice::from_ref(&stop.1),
            );
        }
        self.send(io, peer, crypt, &[stop.1], "the stop").await;
    }

    /// `GAME_CMSG_LAST_POS_BEFORE_MOVE_CANCELED` (`0x8047`): the stopping point a cancel carries.
    ///
    /// `ASURA/crates/asura-world/src/agent.rs:1117` (`handle_last_position`), ported whole: a
    /// correction within the world's `MaxAllowedCorrection` and on the same plane is taken, and the
    /// answer is the movement type 9 followed by a destination at that point; a refused correction
    /// sends **nothing at all**, because the only position it could name is the server's own stale
    /// guess, and the client obeys that message - measured live as one refusal ordering a client 196.7
    /// units backwards (`agent.rs:1108-1116`). The server's own copy stops walking either way.
    async fn answer_last_position(
        &self,
        io: &mut SessionIo,
        peer: &str,
        message: &[u8],
        crypt: &mut AuthCrypt,
    ) {
        let body = &message[2..];
        if body.len() < 12 {
            self.say(
                LogLevel::Warn,
                format!(
                    "Gameplay connection from {peer}: a stopping point is {} bytes and needs 16; it cannot be read.",
                    message.len()
                ),
            );
            return;
        }
        let position = (
            f32::from_le_bytes([body[0], body[1], body[2], body[3]]),
            f32::from_le_bytes([body[4], body[5], body[6], body[7]]),
        );
        let plane = u32::from_le_bytes([body[8], body[9], body[10], body[11]]) as u16;
        let ceiling = self.settings.max_allowed_correction;
        let Some((taken, reply)) = self.with_agent(|agent| {
            let taken = agent.adopt_correction(position, plane, ceiling);
            let reply = if taken {
                agent.stop_manual_walk()
            } else {
                agent.stand_still();
                Vec::new()
            };
            (taken, reply)
        }) else {
            return;
        };
        self.say(
            LogLevel::Debug,
            format!(
                "Gameplay connection from {peer}: a stopping point ({:.1}, {:.1}) plane {plane} against a ceiling of {ceiling:.1}: {}.",
                position.0,
                position.1,
                if taken {
                    "taken - type 9 and a destination at that point"
                } else {
                    "refused - the agent stops and nothing is sent"
                }
            ),
        );
        // A correction the server accepted is a movement the reports lagged behind; it crosses a
        // doorway the same way a walk does (`ASURA/crates/asura-world/src/agent.rs:1175-1179`). A point
        // that is not finite leaves no segment at all.
        if position.0.is_finite() && position.1.is_finite() {
            self.record_portal_step(position);
        } else if let Ok(mut portal) = self.portal.lock() {
            portal.last_step = None;
        }
        if !reply.is_empty() {
            let reply: Vec<Vec<u8>> = reply.into_iter().map(|(_, frame)| frame).collect();
            if let Ok(peers) = self.peers.lock() {
                peers.broadcast(self.player_id(), self.map_id(), self.district(), &reply);
            }
            self.send(io, peer, crypt, &reply, "the stopping point").await;
        }
    }

    /// Write everything other sessions have pushed at this one, and nothing when they pushed nothing.
    async fn flush_outbox(&self, io: &mut SessionIo, peer: &str, crypt: &mut AuthCrypt) {
        let mut frames: Vec<Vec<u8>> = Vec::new();
        if let Ok(outbox) = self.outbox.lock()
            && let Some(receiver) = outbox.as_ref()
        {
            while let Ok(frame) = receiver.try_recv() {
                frames.push(frame);
            }
        }
        if !frames.is_empty() {
            self.send(io, peer, crypt, &frames, "what another session announced").await;
        }
    }

    /// Run something against this session's own agent, when it has one.
    ///
    /// Every message that moves a body goes through here, so this is also where the peer registry learns
    /// where the body stands: an introduction names the point a player is at **now**, not the one it
    /// entered at (`ASURA/crates/asura-world/src/instance.rs:249-255`, `standing`, and `PeerView::from`,
    /// which reads the live agent).
    fn with_agent<T>(&self, work: impl FnOnce(&mut crate::agents::Agent) -> T) -> Option<T> {
        let mut agent = self.agent.lock().ok()?;
        let agent = agent.as_mut()?;
        let answer = work(agent);
        if let Ok(mut peers) = self.peers.lock() {
            peers.moved(
                self.player_id(),
                crate::districts::Standing {
                    position: agent.position,
                    plane: agent.plane,
                    direction: agent.facing,
                },
            );
        }
        Some(answer)
    }

    /// The id this connection's player and agent are known by: drawn once, when a map load begins, and
    /// the first id until then (the creation instance's own body).
    fn player_id(&self) -> u32 {
        match self.id.lock() {
            Ok(id) => id.unwrap_or(CREATION_AGENT),
            Err(_) => CREATION_AGENT,
        }
    }

    /// Draw this connection's id, once. A connection that already has one keeps it.
    fn draw_player_id(&self) {
        if let Ok(mut id) = self.id.lock()
            && id.is_none()
            && let Ok(mut peers) = self.peers.lock()
        {
            *id = Some(peers.allocate());
        }
    }

    /// The district this connection entered, when it has entered one: the number a copy is known by
    /// (`game/Districts/`), and the half a peer registry keys on.
    fn district(&self) -> u32 {
        match self.entered.lock() {
            Ok(entered) => entered.map(|(_, district)| district).unwrap_or(0),
            Err(_) => 0,
        }
    }

    /// The map this connection is in, as the graph lookup wants it.
    fn map_id(&self) -> u32 {
        match self.entered.lock() {
            Ok(entered) => entered.map(|(map_id, _)| map_id).unwrap_or(0),
            Err(_) => 0,
        }
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
        // This connection's own id, drawn before anything names it: the load block, the entry point and
        // every message after them carry the same number (`Peers::allocate`).
        self.draw_player_id();
        let inventory = self.load_inventory(sheet);
        // The instant the last tick went out, so the next one carries how long it really has been.
        let mut last_tick = std::time::Instant::now();
        let mut loaded = false;
        // Where the character stands, chosen once: the entry-position answer and the agent the load
        // creates have to be the same place.
        let entry = self.entry_point(map);
        let load_started = std::time::Instant::now();

        loop {
            // Whatever other sessions pushed at this one - an introduction, a farewell, a move - is
            // written before anything else is read: it travels plaintext and is scrambled here, with
            // this connection's own cipher (`game/Districts/Peers.rs`).
            if loaded {
                self.flush_outbox(io, peer, crypt).await;
            }
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
                        frames.push(character_packets::character_update_info(""));
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
                                    *agent = Some(crate::agents::Agent::new(
                                        self.player_id(),
                                        (point.0, point.1),
                                        point.2,
                                        // A doorway crossing hands over its arrival's facing; every
                                        // other entry faces the neutral +y, because a spawn row
                                        // states no angle (the working implementation's create
                                        // message carries the spawn point's own vector - measured:
                                        // -0.995, -0.104 on one body - and a spawn angle column is
                                        // the gap, named in the audit).
                                        self.claimed_facing(map).unwrap_or((0.0, 1.0)),
                                        self.settings.agent_speed,
                                    ));
                                }
                                self.start_portal_position((point.0, point.1));
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
                        let agent = self.player_id();
                        let player = self.player_id();
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

                        // Everyone already in this copy of the map is introduced first: the player, its
                        // party size, its party and its agent, in that order
                        // (`ASURA/crates/asura-world/src/player.rs:499-501`, `introduction`).
                        let standing = crate::districts::Standing {
                            position,
                            plane,
                            direction: (0.0, 1.0),
                        };
                        let me = crate::districts::PeerView {
                            player_id: player,
                            agent_id: agent,
                            name: sheet.name.clone(),
                            standing,
                            equipped,
                            map_id: map.id,
                            district: self.district(),
                        };
                        let (sending, receiving) = std::sync::mpsc::channel();
                        let others = match self.peers.lock() {
                            Ok(mut peers) => peers.join(me.clone(), sending),
                            Err(_) => Vec::new(),
                        };
                        if let Ok(mut outbox) = self.outbox.lock() {
                            *outbox = Some(receiving);
                        }
                        let mut frames = Vec::with_capacity(40);
                        for peer in &others {
                            frames.extend(crate::districts::introduction(
                                peer,
                                appearance,
                                self.settings.agent_speed,
                                PLAYER_TEAM_TOKEN,
                            ));
                        }
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
                                "Gameplay connection from {peer}: map {} is loaded - the character '{}' is in it ({count} message(s), {} other player(s) introduced).",
                                map.id,
                                sheet.name,
                                others.len()
                            ),
                        );
                        // And the others are told this body is here: the introduction, then the items it
                        // wears in the form a bag slot uses (`CREATE_UNNAMED_ITEM`, `GameSrv_BroadcastPlayerEquippedItems`,
                        // `GmItem.c:129-160`), then what the agent is drawn wearing and its cape - the
                        // reference's own order (`ASURA/crates/asura-world/src/player.rs:509-532`).
                        let mut mine = crate::districts::introduction(
                            &me,
                            appearance,
                            self.settings.agent_speed,
                            PLAYER_TEAM_TOKEN,
                        );
                        if let Some(bag) = self
                            .creation
                            .as_ref()
                            .and_then(crate::creation::equipped_bag)
                        {
                            for item in inventory.items_in(bag.id) {
                                mine.push(item_packets::create_unnamed_item(
                                    &crate::creation::named_item(item),
                                ));
                            }
                        }
                        mine.push(misc_packets::update_agent_visual_equipment(agent, equipped));
                        mine.push(misc_packets::agent_display_cape(agent, 1));
                        if let Ok(peers) = self.peers.lock() {
                            peers.broadcast(player, map.id, self.district(), &mine);
                        }
                        loaded = true;
                    }
                    // A drag in the inventory: where the item goes is a row, the rules are ASURA's, and
                    // the client is told the placement back. `0x8072` is the client's own `9` bytes - the
                    // item, the bag and the slot - and `0x013E` is the placement.
                    CLIENT_ITEM_MOVE => {
                        self.answer_item_move(io, peer, &message, crypt, sheet).await;
                    }
                    // `GAME_CMSG_PING_REPLY` (`0x8009`, 10 bytes) and `GAME_CMSG_HEARTBEAT` (`0x800C`,
                    // 2 bytes): the working implementation names both and discards both
                    // (`ASURA/crates/asura-world/src/dispatch.rs:116-122`), because the reference does.
                    CLIENT_PING_REPLY | CLIENT_HEARTBEAT => {}
                    // The client's own ping on a live connection: the reference logs it and
                    // answers nothing, so this does the same.
                    // `GAME_CMSG_PING_REQUEST` (`0x800D`) on a loaded connection: the same answer as
                    // before the load, because the working implementation has one handler for the
                    // connection's whole life (`ASURA/crates/asura-world/src/connection.rs:579-587`).
                    GAME_PING => {
                        self.send(
                            io,
                            peer,
                            crypt,
                            &[misc_packets::ping_reply(self.settings.ping_value)],
                            "the ping reply",
                        )
                        .await;
                    }
                    // The client's movement. Four messages, four handlers, ported from the working
                    // implementation's own (`ASURA/crates/asura-world/src/agent.rs`): `0x803E` asks for
                    // a walk, `0x803D` reports where the client is and the direction it is walking in,
                    // `0x8028` cancels, and `0x8047` is the stopping point a cancel carries.
                    CLIENT_MOVE_TO_COORD => {
                        self.answer_move_to_coord(io, peer, &message, crypt).await;
                    }
                    CLIENT_MOVE_TO_COORD_WITH_DIR => {
                        self.answer_position_update(io, peer, &message, crypt).await;
                        // The step the report moved the agent along, checked against the map's own
                        // walls (`ASURA/crates/asura-world/src/dispatch.rs:44-54`).
                        if let Some((from, to)) = self.take_portal_step() {
                            self.cross_portal(io, peer, crypt, sheet, from, to).await;
                        }
                    }
                    CLIENT_CANCEL_MOVEMENT => {
                        self.answer_cancel_movement(io, peer, crypt).await;
                    }
                    CLIENT_LAST_POS_BEFORE_MOVE_CANCELED => {
                        self.answer_last_position(io, peer, &message, crypt).await;
                        // A correction the server accepted is a movement the reports lagged behind, so
                        // it is checked for a doorway the same way a walk is
                        // (`ASURA/crates/asura-world/src/dispatch.rs:60-68`).
                        if let Some((from, to)) = self.take_portal_step() {
                            self.cross_portal(io, peer, crypt, sheet, from, to).await;
                        }
                    }
                    // The client's own in-game travel request. `ASURA/crates/asura-world/src/travel.rs`
                    // answers it with the same cycle `/tp` ends in: a map this world hosts goes to
                    // `begin_transfer`, and one it cannot serve is refused in the log.
                    CLIENT_PARTY_TRAVEL => {
                        self.answer_party_travel(io, peer, &message, crypt, sheet)
                            .await;
                    }
                    // The client touched a travel object (`0x80C1`): named in the log and nothing more,
                    // because a crossing is decided from the player's own step, not from the touch
                    // (`ASURA/crates/asura-world/src/travel.rs:132-169`).
                    CLIENT_TRAVEL_INIT => {
                        self.answer_travel_init(peer, &message, sheet).await;
                    }
                    // The client speaks: a line on a channel, or one of the reference's commands
                    // (`ASURA/crates/asura-world/src/chat.rs`).
                    CLIENT_CHAT_MESSAGE => {
                        self.answer_chat_message(io, peer, &message, crypt, sheet)
                            .await;
                    }
                    // The client destroys one of its items: the row goes, and the item is told gone
                    // with `0x014D`.
                    // The client drops `count` of an item (`GAME_CMSG_DROP_ITEM`, `0x802C`: the item as a
                    // dword and the count as a byte). The working implementation's `drop_item`
                    // (`ASURA/crates/asura-world/src/item.rs:187`) takes them off the stack and puts
                    // **nothing on the wire**: both messages a drop could send crashed a live client -
                    // `ITEM_REMOVE` with the stack re-created cloned the item, and the ground agent
                    // (`0x0020`, `AgentType_Item`) crashed with that message as the log's last line -
                    // and neither appears in any capture (`item.rs:236-246`). So the stack is the state
                    // and silence is the answer.
                    CLIENT_DROP_ITEM => {
                        let item_id = message
                            .get(2..6)
                            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                        let count = message.get(6).copied();
                        let (Some(item_id), Some(count)) = (item_id, count) else {
                            self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: a drop of {} bytes cannot be read.",
                                    message.len()
                                ),
                            );
                            return;
                        };
                        if item_id == 0 || count == 0 {
                            self.say(
                                LogLevel::Info,
                                format!(
                                    "Gameplay connection from {peer}: a drop of {count} of item {item_id} names something that is not an item; ignored."
                                ),
                            );
                            return;
                        }
                        let Some(pool) = self.characters.as_ref() else {
                            return;
                        };
                        let characters = crate::characters::DatabaseCharacters::new(
                            std::sync::Arc::clone(pool),
                            self.log.clone(),
                        );
                        let uuid = crate::characters::uuid_text(&sheet.uuid);
                        match characters.take_from_stack(&uuid, item_id, u32::from(count)) {
                            Ok(Some(remaining)) => self.say(
                                LogLevel::Info,
                                format!(
                                    "Gameplay connection from {peer}: {count} of item {item_id} dropped; {remaining} left in the bag, and nothing is sent."
                                ),
                            ),
                            Ok(None) => self.say(
                                LogLevel::Info,
                                format!(
                                    "Gameplay connection from {peer}: a drop names item {item_id}, which the character does not carry in that quantity; ignored."
                                ),
                            ),
                            Err(error) => self.say(
                                LogLevel::Warn,
                                format!(
                                    "Gameplay connection from {peer}: the drop could not be written: {error}"
                                ),
                            ),
                        }
                    }
                    // `GAME_CMSG_UNEQUIP_ITEM` (`0x804F`): the working implementation answers it by
                    // saying what it cannot do - the message carries no item id and no capture shows
                    // what it means, and dragging the item out of the equipped bag works instead
                    // (`ASURA/crates/asura-world/src/item.rs:555`).
                    CLIENT_UNEQUIP_ITEM => self.say(
                        LogLevel::Info,
                        format!(
                            "Gameplay connection from {peer}: an unequip (0x804F) is not ported: it carries no item id and no capture shows what it means; dragging the item out of the equipped bag is the path that works."
                        ),
                    ),
                    // The client wears one of its items: the slot its kind is worn in, in the equipped
                    // bag, with whatever was worn there taken off to the first free carrying slot - and
                    // the agent's worn items sent again, which is the one thing the client cannot work
                    // out for itself (`ASURA/crates/asura-world/src/item.rs:314-436`).
                    CLIENT_EQUIP_ITEM => {
                        self.answer_equip_item(io, peer, &message, crypt, sheet).await;
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
                        // **How long it has been since the last tick**, not a counter: the client
                        // advances its simulation - and its sync copy of every agent - by this number
                        // (`ASURA/crates/asura-world/src/instance.rs:369`, `world_simulation_tick`);
                        // a counter here makes everything the client draws run at the wrong speed.
                        let now = std::time::Instant::now();
                        let elapsed = now.duration_since(last_tick);
                        last_tick = now;
                        // The instance's own clock advances every agent by the same elapsed time, and
                        // announces the waypoint a walk crosses - every client is drawing that agent
                        // from the last destination it was told (`ASURA/crates/asura-world/src/instance.rs:369-386`).
                        let crossed = self.with_agent(|agent| {
                            let crossed = agent.advance(elapsed.as_secs_f32());
                            crossed.map(|waypoint| {
                                agent_packets::agent_move_to_point(
                                    agent.agent_id,
                                    (waypoint.pos.x, waypoint.pos.y),
                                    waypoint.pos.plane,
                                    agent.plane,
                                )
                            })
                        });
                        let mut frames = vec![misc_packets::world_simulation_tick(
                            u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX),
                        )];
                        if let Some(Some(walk)) = crossed {
                            frames.push(walk.1);
                        }
                        self.send(io, peer, crypt, &frames, "a tick").await;
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

    /// The facing a claimed doorway crossing handed over, when this map is the one it named.
    ///
    /// `ASURA/crates/asura-world/src/travel.rs:359-362` and `portals::arrival_at`: the receiving map's
    /// arrival states where a character arriving from a given map stands and which way it faces.
    fn claimed_facing(&self, map: &crate::world::MapEntry) -> Option<(f32, f32)> {
        match self.landing.lock() {
            Ok(claimed) => claimed
                .filter(|claimed| claimed.map_id == map.id)
                .map(|claimed| claimed.facing),
            Err(_) => None,
        }
    }

    /// One point of a map's own spawn list, scattered by the world's setting (R13) - or, when the
    /// travel this connection claimed was a doorway crossing, the landing that doorway states (R21).
    ///
    /// The game's rule: every entry takes a random point of that map's list, lands up to
    /// `spawn_scatter` units around it, and faces the point's angle. The scatter is a setting, never a
    /// constant. A crossing states its own point and its own facing, which the receiving map owns
    /// (`ASURA/crates/asura-world/src/travel.rs:319-338`, `portals::arrival`).
    fn entry_point(&self, map: &crate::world::MapEntry) -> Option<(f32, f32, u16)> {
        if let Ok(claimed) = self.landing.lock()
            && let Some(claimed) = *claimed
            && claimed.map_id == map.id
        {
            return Some((
                claimed.landing.x,
                claimed.landing.y,
                claimed.landing.plane.max(0) as u16,
            ));
        }
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
        // And the body is taken back out of everyone else's world: the player destroyed, then its agent
        // removed, in that order (`ASURA/crates/asura-world/src/instance.rs:466-473`, `farewell`).
        if let Ok(mut peers) = self.peers.lock()
            && let Some(peer) = peers.leave(self.player_id())
        {
            let frames = crate::districts::farewell(peer.player_id, peer.agent_id);
            peers.broadcast(peer.player_id, peer.map_id, peer.district, &frames);
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
                    instance_value: None,
                },
                MapEntry {
                    id: 148,
                    kind: map_kind::OUTPOST,
                    file_id: 113_021,
                    name: "Ascalon City (pre-Searing)".to_string(),
                    instance_value: Some(0xBB14_F539),
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

    /// `ASURA/crates/asura-world/src/player.rs:60-74`: the doorway test consumes the segment between two
    /// finite reports, once, and a point reported twice is not a step at all. A simulation tick between
    /// the reports must not shorten it, which is why the segment is kept beside the agent.
    #[test]
    fn a_reported_step_is_the_segment_between_two_reports_and_is_consumed_once() {
        let world = world();
        let session = WorldSession::new(Log::new(), &world);
        session.start_portal_position((7277.4, 5417.9));
        session.record_portal_step((7277.4, 5417.9));
        assert!(
            session.take_portal_step().is_none(),
            "a report of the point the character entered at is not a step"
        );
        session.record_portal_step((7184.3, 5307.3));
        assert_eq!(
            session.take_portal_step(),
            Some(((7277.4, 5417.9), (7184.3, 5307.3))),
            "the step is the pair of reports"
        );
        assert!(
            session.take_portal_step().is_none(),
            "a step is consumed once, so a second message cannot cross the same wall"
        );
        session.record_portal_step((7184.3, 5307.3));
        assert!(
            session.take_portal_step().is_none(),
            "a report of the same point is not a step"
        );
    }

    #[test]
    fn a_travel_request_names_the_instance_in_its_first_two_bytes() {        // The client's own frame, as the working implementation's log records it: `B1 80 | 94 00 | 02 |
        // 00 00 | 0...` - the header `0x80B1`, then the instance. This tree read it as `0x00B1` (the
        // server's own party message) and parsed five fields the working implementation does not read.
        let mut message = vec![0xB1, 0x80];
        message.extend_from_slice(&148u16.to_le_bytes());
        message.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(message.len(), 9, "the frame is nine bytes");
        assert_eq!(
            u16::from_le_bytes([message[0], message[1]]),
            CLIENT_PARTY_TRAVEL,
            "the header is the party travel's"
        );
        assert_eq!(
            u16::from_le_bytes([message[2], message[3]]),
            148,
            "and the instance is at its head"
        );
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
        read_frames_until(client, reading, instance_packets::INSTANCE_LOAD_FINISH)
    }

    /// Read whole frames off a scrambled connection until `terminal` arrives, answering every frame.
    fn read_frames_until(
        client: &mut std::net::TcpStream,
        reading: &mut common::cryptography::Arc4,
        terminal: u16,
    ) -> Vec<(u16, Vec<u8>)> {
        read_frames_until_n(client, reading, terminal, 1)
    }

    /// The same, but waiting for the `times`-th `terminal`: a load's manifest carries two ends.
    fn read_frames_until_n(
        client: &mut std::net::TcpStream,
        reading: &mut common::cryptography::Arc4,
        terminal: u16,
        times: usize,
    ) -> Vec<(u16, Vec<u8>)> {
        let mut frames = Vec::new();
        let mut pending: Vec<u8> = Vec::new();
        let mut buffer = [0u8; 4096];
        let mut seen = 0usize;
        loop {
            while pending.len() >= 2 {
                match crate::server::protocol::decode_game_message(&pending, false) {
                    Ok((frame, used)) => {
                        let header = frame.header();
                        let whole: Vec<u8> = pending.drain(..used).collect();
                        frames.push((header, whole));
                        if header == terminal {
                            seen += 1;
                            if seen == times {
                                return frames;
                            }
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

        // 3. The head block: load head, player-data start, the name, the instance's record, and the
        // exploration grid a live server sends right after it.
        let grid = crate::maps::cartography::init_frames(crate::maps::cartography::grid_for(148));
        let mut expected = vec![
            instance_packets::instance_load_head(),
            instance_packets::instance_player_data_start(),
            instance_packets::instance_load_player_name("Player"),
            // The explorable byte is 1 for every load: the reference hardcodes it
            // (`ASURA/crates/asura-world/src/map.rs:34-42`).
            instance_packets::instance_load_info(1, 148, 1, 1, 0, 0),
        ];
        expected.extend(grid);
        // And the instance table: one batch, because this world's one map with a value states it.
        let (table, notes) = crate::maps::instance_table::table_for(world.maps());
        assert!(notes.is_empty(), "every map's value is stated or absent: {notes:?}");
        expected.extend(table.frames());
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
        assert_eq!(
            headers,
            vec![0x017C, 0x0186, 0x017D, 0x019A, 0x008B, 0x008A, 0x01A0]
        );

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
        expected_items.push(character_packets::character_update_info(""));
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

        // 8. And the connection stays: the world ticks it, one per interval, with no prompt. The
        // payload is how long it has been since the last tick, not a counter - the client advances its
        // simulation by it (`ASURA/crates/asura-world/src/instance.rs:369`).
        let mut tick = vec![0u8; 6];
        client.read_exact(&mut tick).unwrap();
        reading.apply(&mut tick);
        assert_eq!(&tick[..2], &[0x1E, 0x00], "a world simulation tick");
        let elapsed = u32::from_le_bytes([tick[2], tick[3], tick[4], tick[5]]);
        assert!(
            elapsed > 0 && elapsed <= 1000,
            "the tick carries the elapsed milliseconds of one interval, not a counter: {elapsed}"
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

    /// Play one connection up to a loaded map, and answer the frames its players request carried.
    ///
    /// The exchange, the head block, the ping pair, the items, the entry point and the players - the
    /// same steps `an_outpost_connection_is_loaded_with_its_entry_point` walks in detail, kept here so a
    /// second player can be put in the same map without repeating them.
    fn load_a_player(
        port: u16,
        master: [u8; MASTER_SECRET_LEN],
        character: [u8; 16],
    ) -> (
        std::net::TcpStream,
        common::cryptography::Arc4,
        Vec<u16>,
    ) {
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let version = GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 2,
            map_id: 148,
            player_token: 0x1234,
            account: [0xAA; 16],
            character,
            h0038: 0,
            h003c: 0,
        };
        client.write_all(&version.encode()).unwrap();
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

        // The head block ends with the instance table; the ping pair follows it unasked.
        read_frames_until(&mut client, &mut reading, instance_packets::INSTANCE_TABLE);
        let mut ping = vec![0u8; 8];
        client.read_exact(&mut ping).unwrap();
        reading.apply(&mut ping);

        // The items ask, the entry point, and the players.
        let mut ask = vec![0x91, 0x80, 0x00, 0x00];
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let items = read_frames_until_n(
            &mut client,
            &mut reading,
            instance_packets::INSTANCE_MANIFEST_DONE,
            2,
        );
        assert_eq!(
            items.last().map(|(header, _)| *header),
            Some(instance_packets::INSTANCE_MANIFEST_DONE),
            "the items answer ends with the second manifest end"
        );
        let mut ask = vec![0x88, 0x80];
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let mut spawn = vec![0u8; 32];
        client.read_exact(&mut spawn).unwrap();
        reading.apply(&mut spawn);
        let mut ask = vec![0x90, 0x80];
        ask.extend_from_slice(&[0u8; 17]);
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let players = read_load_frames(&mut client, &mut reading);
        let headers = players.iter().map(|(header, _)| *header).collect();
        (client, reading, headers)
    }

    /// Two players in one copy of a map: the second is told about the first before the load's own block,
    /// the first is told the second arrived, and told again when it leaves.
    ///
    /// `ASURA/crates/asura-world/src/player.rs:499-532` and `instance.rs:432-473`: the introduction is
    /// the player, its party size, its party and its agent; the farewell is the player destroyed and the
    /// agent removed.
    #[test]
    fn two_connections_in_one_map_introduce_each_other() {
        let log = Log::new();
        let world = Arc::new(world());
        let master = [0x5Au8; MASTER_SECRET_LEN];
        let factory = factory(log.clone(), Arc::clone(&world), master);
        let mut manager = shared::network::socket_mgr::SocketMgr::new(factory);
        let port = manager.start_network("127.0.0.1", 0, 1).unwrap();

        let (client, mut reading, first) = load_a_player(port, master, [0xBB; 16]);
        assert_eq!(
            first.first(),
            Some(&0x001D),
            "the first player has no introductions before its own block: {first:04X?}"
        );
        assert_eq!(
            first.iter().filter(|header| **header == 0x0020).count(),
            1,
            "one body in the map, its own: {first:04X?}"
        );

        let (second_client, _second_reading, second) =
            load_a_player(port, master, [0xCC; 16]);
        // The second player's answer opens with the first player's introduction, then its own block.
        assert_eq!(
            &second[..4],
            &[0x0059, 0x00B0, 0x00B1, 0x0020],
            "the introduction comes before the block: {second:04X?}"
        );
        assert_eq!(
            second[4], 0x001D,
            "and the block itself follows it: {second:04X?}"
        );

        // And the first player is told the second arrived: the introduction, the items it wears in the
        // unnamed form, what the agent is drawn wearing, and its cape (`player.rs:509-532`).
        let mut first_client = client;
        let arrived = read_frames_until(
            &mut first_client,
            &mut reading,
            misc_packets::AGENT_DISPLAY_CAPE,
        );
        // The world keeps ticking the connection while it waits, so the ticks are set aside first: what
        // is being read is the announcement, in the order the working implementation sends it.
        let headers: Vec<u16> = arrived
            .iter()
            .map(|(header, _)| *header)
            .filter(|header| *header != misc_packets::WORLD_SIMULATION_TICK)
            .collect();
        assert_eq!(
            &headers[..4],
            &[0x0059, 0x00B0, 0x00B1, 0x0020],
            "the other player's introduction: {headers:04X?}"
        );
        assert!(
            headers.contains(&item_packets::CREATE_UNNAMED_ITEM),
            "and what it wears, in the form a bag slot uses: {headers:04X?}"
        );
        assert!(
            headers.contains(&misc_packets::UPDATE_AGENT_VISUAL_EQUIPMENT),
            "and the nine items its agent is drawn wearing: {headers:04X?}"
        );
        assert_eq!(headers.last(), Some(&misc_packets::AGENT_DISPLAY_CAPE));

        // The second player leaves; the first is told the player is gone and the agent with it.
        let _ = second_client.shutdown(std::net::Shutdown::Both);
        let gone = read_frames_until(&mut first_client, &mut reading, 0x0021);
        let headers: Vec<u16> = gone
            .iter()
            .map(|(header, _)| *header)
            .filter(|header| *header != misc_packets::WORLD_SIMULATION_TICK)
            .collect();
        assert_eq!(
            headers,
            vec![0x005A, 0x0021],
            "the farewell, in the reference's order"
        );

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
                landing: None,
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
        // and language, and the character's own name. The block carries the exploration grid and the
        // instance table after the instance's record, so it is read frame by frame up to the table.
        let grid = crate::maps::cartography::init_frames(crate::maps::cartography::grid_for(148));
        let (table, notes) = crate::maps::instance_table::table_for(world.maps());
        assert!(notes.is_empty(), "every map's value is stated or absent: {notes:?}");
        let mut expected = vec![
            instance_packets::instance_load_head(),
            instance_packets::instance_player_data_start(),
            instance_packets::instance_load_player_name("Travelled One"),
            instance_packets::instance_load_info(1, 148, 0, 3, 2, 0),
        ];
        expected.extend(grid);
        expected.extend(table.frames());
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
        expected_items.push(character_packets::character_update_info(""));
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

        // 4d. An equip: the item goes to the slot its kind is worn in, in the equipped bag, and whatever
        // was worn there comes off to the first free carrying slot
        // (`ASURA/crates/asura-world/src/item.rs:314-436`). An item already in its own worn slot is
        // `MoveOutcome::Unchanged` and answers nothing, so the one picked is somewhere else.
        let equipped_bag = crate::creation::equipped_bag(world.creation())
            .expect("the setting names the equipped bag");
        // The starting set's wearable items are already worn, so the sequence is the player's own: take
        // one off to the backpack first, then wear it again.
        let worn = starting
            .items
            .iter()
            .find(|item| crate::creation::equipped_slot(item.item_type).is_some())
            .expect("the set carries something wearable");
        let worn_slot = crate::creation::equipped_slot(worn.item_type).unwrap();
        let backpack = crate::creation::bags(world.creation())
            .iter()
            .find(|bag| bag.model == crate::characters::BACKPACK_MODEL)
            .copied()
            .expect("the content has a backpack");
        let free = (0..backpack.slots)
            .find(|slot| {
                !starting
                    .items
                    .iter()
                    .any(|item| item.bag_id == Some(backpack.id) && item.slot == Some(*slot))
            })
            .expect("the backpack has a free slot");
        let mut take_off = vec![0u8; 9];
        take_off[0..2].copy_from_slice(&0x8072u16.to_le_bytes());
        take_off[2..6].copy_from_slice(&worn.id.to_le_bytes());
        take_off[6..8].copy_from_slice(&backpack.id.to_le_bytes());
        take_off[8] = free;
        sending.apply(&mut take_off);
        client.write_all(&take_off).unwrap();
        let mut off = vec![0u8; 11];
        client.read_exact(&mut off).unwrap();
        reading.apply(&mut off);
        assert_eq!(&off[..2], &[0x3E, 0x01], "the drag's placement");
        assert_eq!(&off[8..10], &backpack.id.to_le_bytes(), "into the backpack");
        // Taking it off changes what the agent is drawn wearing, so that is sent again too.
        let mut off_drawn = vec![0u8; 2 + 4 + 9 * 4];
        client.read_exact(&mut off_drawn).unwrap();
        reading.apply(&mut off_drawn);
        assert_eq!(&off_drawn[..2], &[0x6E, 0x00], "the agent's worn items");
        let occupant = starting
            .items
            .iter()
            .find(|item| {
                item.bag_id == Some(equipped_bag.id)
                    && item.slot == Some(worn_slot)
                    && item.id != worn.id
            })
            .map(|item| item.id);
        let mut ask = vec![0x30, 0x80];
        ask.extend_from_slice(&worn.id.to_le_bytes());
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        if let Some(occupant) = occupant {
            let mut off = vec![0u8; 11];
            client.read_exact(&mut off).unwrap();
            reading.apply(&mut off);
            assert_eq!(&off[..2], &[0x3E, 0x01], "the eviction's placement");
            assert_eq!(&off[4..8], &occupant.to_le_bytes(), "the item taken off");
        }
        let mut placed = vec![0u8; 11];
        client.read_exact(&mut placed).unwrap();
        reading.apply(&mut placed);
        assert_eq!(&placed[..2], &[0x3E, 0x01], "the placement");
        assert_eq!(&placed[4..8], &worn.id.to_le_bytes(), "the item");
        assert_eq!(&placed[8..10], &equipped_bag.id.to_le_bytes(), "the equipped bag");
        assert_eq!(placed[10], worn_slot, "the slot its kind is worn in");
        // And what the agent is drawn wearing is sent again.
        let mut drawn = vec![0u8; 2 + 4 + 9 * 4];
        client.read_exact(&mut drawn).unwrap();
        reading.apply(&mut drawn);
        assert_eq!(&drawn[..2], &[0x6E, 0x00], "the agent's worn items");

        // 4e. A drop (`0x802C`, the item and a count byte): the stack is the state and **nothing goes
        // back**, which is what the working implementation does and what its notes on the two
        // candidates say each cost a live client (`ASURA/crates/asura-world/src/item.rs:187-246`).
        let dropped = starting
            .items
            .iter()
            .find(|item| item.id != worn.id && item.id != 1 && item.quantity > 1)
            .expect("the set carries a stack of more than one")
            .id;
        let before = crate::characters::DatabaseCharacters::new(Arc::clone(&characters), log.clone())
            .items_of(&character_text)
            .expect("the item rows are readable")
            .iter()
            .find(|row| row.item_id == dropped)
            .map(|row| row.quantity)
            .expect("the stack is one of the character's");
        let mut ask = vec![0x2C, 0x80];
        ask.extend_from_slice(&dropped.to_le_bytes());
        ask.push(1);
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        // **No answer**: the next thing the connection says is the answer to what is asked next -
        // here the entry position, which is `0x0196` and the first tick. A drop puts nothing on the
        // wire at all (`ASURA/crates/asura-world/src/item.rs:236-246`).
        let mut ask = vec![0x88, 0x80];
        sending.apply(&mut ask);
        client.write_all(&ask).unwrap();
        let mut next = vec![0u8; 32];
        client.read_exact(&mut next).unwrap();
        reading.apply(&mut next);
        assert_eq!(
            &next[..2],
            &[0x96, 0x01],
            "the drop is answered with nothing, so the entry position is next; got {}",
            shared::network::socket::hex(&next)
        );
        let after = crate::characters::DatabaseCharacters::new(Arc::clone(&characters), log.clone())
            .items_of(&character_text)
            .expect("the item rows are readable")
            .iter()
            .find(|row| row.item_id == dropped)
            .map(|row| row.quantity)
            .expect("the stack is still there");
        assert_eq!(after, before - 1, "one was taken off the stack");

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
                landing: None,
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
            instance_value: None,
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