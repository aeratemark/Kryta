//! The gameplay conversation: the second connection, from its version message to a loaded map.
//!
//! Reference: `src/server/game/Server/WorldSocket.cpp` and `WorldSession.cpp` - the object a
//! gameplay connection becomes, and the session behind it. What happens here, in the reference's
//! order:
//!
//! 1. The client's 64-byte version message, which names the world it was sent to (`0x000C0500`).
//! 2. The seed exchange, the same one the login port makes: `0x4200` in, `0x1601` out, one key
//!    derived from the master, one RC4 per direction.
//! 3. The block the world sends unasked. For the character-creation instance that is two messages -
//!    the load head and the creation start - because the creation instance has no geometry and no
//!    agent of its own; a map with geometry sends its own block, which does not exist yet.
//!
//! Reference for the sizes: `shared/cryptography/handshake.rs` and `Packets/InstancePackets.rs`.
//! What this does *not* do yet: check the token pair against a recorded travel. The login draws a
//! `player_token` and the front door forwards it, but nothing records it where a world can read it,
//! so the pair is logged and not compared. That is stated here rather than implied by silence.

use common::logging::{Log, LogLevel};
use shared::cryptography::handshake::{self, CLIENT_SEED_HEADER, CLIENT_SEED_LEN, SERVER_SEED_LEN};
use shared::cryptography::AuthCrypt;
use shared::network::socket::{Session, SessionFuture, SessionIo};

use crate::server::packets::{
    character_packets, creation_load_block, misc_packets,
};
use crate::server::protocol::GameVersion;
use crate::server::protocol::{Frame, FrameError};
use database::database::Param;
use crate::world::World;

/// The agent id a character-creation connection's player is drawn as.
///
/// A creation screen shows one body - the character being made - and a world here has no agent
/// registry yet, so the first id is the one it gets. The reference allocates from its own pool; the
/// number is the world's to give, and nothing else reads it until a map exists.
pub(crate) const CREATION_AGENT: u32 = 1;

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
    /// The character database, when the program opened one: a creation screen's confirm writes a row
    /// there. A session without it can draw the screen but cannot finish it.
    characters: Option<std::sync::Arc<std::sync::Mutex<database::database::DatabaseWorkerPool>>>,
    /// The bags, items and profession sets a creation screen is built from, when the program read them.
    creation: Option<crate::creation::Tables>,
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
            characters: None,
            creation: Some(world.creation().clone()),
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

        // 3. The block the world sends unasked, once the connection is scrambled.
        let Some(map) = self.maps.iter().find(|map| map.id == version.map_id) else {
            self.say(
                LogLevel::Error,
                format!(
                    "Gameplay connection from {peer} asks for map {}, which this world does not host ({} map(s) in the catalogue); closing it.",
                    version.map_id,
                    self.maps.len()
                ),
            );
            let _ = io.close().await;
            return;
        };

        if !map.is_creation() {
            self.say(
                LogLevel::Error,
                format!(
                    "Gameplay connection from {peer} asks for map {} (type {}), which this world cannot load yet: only the character-creation instance has a block.",
                    map.id, map.kind
                ),
            );
            let _ = io.close().await;
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
        loop {
            // Everything already unscrambled and not yet answered, before any more is read.
            while let Some(bytes) = pending.first_chunk::<2>() {
                let header = u16::from_le_bytes(*bytes);
                // The table decides where the message ends: a counted field carries its own count, so
                // the walk is exact for every header the protocol has, not just the ones answered here.
                let used = match Frame::decode_game_message(&pending, true) {
                    Ok((frame, used)) => {
                        self.log.out(
                            LogLevel::Debug,
                            "network.trace",
                            format!(
                                "trace {peer} game in  {} bytes, {}",
                                used,
                                frame.label()
                            ),
                        );
                        used
                    }
                    Err(FrameError::Incomplete { .. }) => break,
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
                let frames = vec![
                    character_packets::player_update_profession(
                        CREATION_AGENT,
                        creation.profession,
                        0,
                        0,
                    ),
                    character_packets::player_update_unlocked_professions(
                        CREATION_AGENT,
                        character_packets::CREATION_UNLOCKED_PROFESSIONS,
                    ),
                ];
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
                    Param::Integer(0), // last outpost: nothing here can place a character yet
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
                let settings = crate::characters::CharacterSettings {
                    last_outpost: 0,
                    appearance,
                    campaign: creation.campaign,
                    level: 1,
                    is_pvp: creation.campaign == 0,
                    secondary_profession: 0,
                    helm_status: 1,
                    pieces: Vec::new(),
                };
                let answer =
                    character_packets::char_creation_success(id, &name, &settings.to_blob());
                let _ = self
                    .send(io, peer, crypt, &[answer], "the character creation")
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

impl Session for WorldSession {
    fn run(self: Box<Self>, mut io: SessionIo) -> SessionFuture {
        Box::pin(async move {
            self.converse(&mut io).await;
        })
    }
}

/// Read exactly as many bytes as the slice holds.
async fn read_exactly(io: &mut SessionIo, out: &mut [u8]) -> Result<(), String> {
    let mut filled = 0;
    while filled < out.len() {
        match io.read(&mut out[filled..]).await {
            Ok(0) => return Err("the peer closed".to_string()),
            Ok(count) => filled += count,
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
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

    /// The world a test serves: world 2, the character-creation instance.
    fn world() -> World {
        let mut world = World::new(2);
        // A directory per test thread: two tests in one process would otherwise remove each other's
        // catalogue between the write and the read.
        let dir = std::env::temp_dir().join(format!(
            "kryta-ww-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("maps.json");
        std::fs::write(&path, r#"{"maps": [{"id": 0, "type": 11}]}"#).unwrap();

        world.load_maps(&path, &Log::new()).unwrap();
        assert_eq!(world.maps(), &[MapEntry { id: 0, kind: map_kind::CREATION }]);

        // And the creation content, which the same start-up pass reads: a screen with no items has no
        // inventory model to draw.
        let (bags, items, sets) = world
            .load_creation("../data/inventory.json", "../data/equipment.json", &Log::new())
            .expect("the creation data");
        assert_eq!((bags, items, sets), (6, 9, 32), "the reference's own counts");
        world
    }

    fn factory(log: Log, world: std::sync::Arc<World>, master: [u8; MASTER_SECRET_LEN]) -> shared::network::socket::SessionFactory {
        Arc::new(move || {
            Box::new(WorldSession::with_master(log.clone(), &world, master)) as Box<dyn Session>
        })
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

        // The confirm, in the shape the recording shows: the four-byte header, a counted name, then
        // the thirty-four bytes this test leaves zero after the appearance's four.
        let name = "Kryta Test";
        let units: Vec<u16> = name.encode_utf16().collect();
        let appearance_at = 4 + units.len() * 2;
        // Thirty-four bytes follow the name, and the appearance word is the first four of them.
        let mut confirm = vec![0u8; 58];
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
        let mut pool = pool.lock().unwrap();
        let rows = pool
            .query_with(
                "SELECT \"name\", \"appearance\" FROM \"characters\" WHERE \"uuid\" = $1",
                &[&uuid_text],
            )
            .expect("the row is readable");
        assert_eq!(rows.len(), 1, "one row was written");
        assert_eq!(rows[0].get::<usize, String>(0), name);
        log.shutdown();
    }

}