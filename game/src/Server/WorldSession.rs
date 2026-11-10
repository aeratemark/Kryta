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
    agent_packets, character_packets, creation_load_block, instance_packets, misc_packets,
};
use crate::server::protocol::GameVersion;
use crate::world::World;

/// The agent id a character-creation connection's player is drawn as.
///
/// A creation screen shows one body - the character being made - and a world here has no agent
/// registry yet, so the first id is the one it gets. The reference allocates from its own pool; the
/// number is the world's to give, and nothing else reads it until a map exists.
const CREATION_AGENT: u32 = 1;

/// The client's asks on a gameplay connection, as the reference's dispatch reads them.
///
/// Sizes from `ASURA/docs/reference/client-protocol.md`'s table: the request-player and
/// request-armors asks are the header alone, a profession change is a campaign and a profession, the
/// confirm carries the character's name and appearance, and a ping is the header alone.
const CHAR_CREATION_REQUEST_PLAYER: u16 = 0x8089;
const CHAR_CREATION_REQUEST_ARMORS: u16 = 0x808A;
const CHAR_CREATION_CHANGE_PROF: u16 = 0x8060;
const CHAR_CREATION_CONFIRM: u16 = 0x808B;
/// `GAME_CMSG_PING_REQUEST`: two bytes on the gameplay connection.
const GAME_PING: u16 = 0x800D;

/// How many bytes each message this world knows is.
///
/// A gameplay message carries no length, so the table is what splits the stream - the same rule the
/// login connection follows (`authserver/src/Server/stream.rs`). A header that is not here stops the
/// reading and is reported with its bytes rather than guessed at.
fn game_message_size(header: u16) -> Option<usize> {
    match header {
        CHAR_CREATION_REQUEST_PLAYER => Some(2),
        CHAR_CREATION_REQUEST_ARMORS => Some(2),
        CHAR_CREATION_CHANGE_PROF => Some(4),
        CHAR_CREATION_CONFIRM => Some(58),
        GAME_PING => Some(2),
        _ => None,
    }
}

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
/// Reference: `GameSrv_HandleCharCreationRequestPlayer`, which is `instance_player_data_start`, the
/// inventory stream, the faction caps, the attribute and profession state, the skillbar, the faction
/// record and the agent's properties, then `instance_player_data_done`. **What is missing here is the
/// inventory**: this tree has no item framing or starting equipment yet, so the screen draws a
/// character with no equipment rather than not drawing one at all.
fn creation_player_block(creation: &Creation) -> Vec<Vec<u8>> {
    let agent = CREATION_AGENT;
    let mut frames = vec![instance_packets::instance_player_data_start()];

    for header in [
        character_packets::FACTION_MAX_KURZICK,
        character_packets::FACTION_MAX_LUXON,
        character_packets::FACTION_MAX_BALTHAZAR,
        character_packets::FACTION_MAX_IMPERIAL,
    ] {
        frames.push(character_packets::faction_max(header, 10_000));
    }

    frames.push(agent_packets::agent_update_attribute_points(agent, 50, 50));
    frames.push(character_packets::player_update_profession(
        agent,
        creation.profession,
        0,
        0,
    ));
    frames.push(character_packets::player_update_unlocked_professions(
        agent,
        character_packets::CREATION_UNLOCKED_PROFESSIONS,
    ));
    frames.push(character_packets::skillbar_update(
        agent,
        &[0u32; 8],
        &[0u32; 8],
        1,
    ));
    frames.push(character_packets::update_unlocked_skills(&[]));
    frames.push(character_packets::character_update_factions(&[0u32; 13]));
    frames.push(agent_packets::agent_update_attributes(agent, &[0u32; 42]));

    // The creation path's own numbers: `GameSrv_SendPlayerHealthEnergy` sends energy 20, health 100
    // and a zero for the third property. A map load's numbers differ, and mixing them is a mismatch.
    frames.push(agent_packets::agent_property_update_int(
        agent_packets::AGENT_PROPERTY_ENERGY,
        agent,
        20,
    ));
    frames.push(agent_packets::agent_property_update_int(
        agent_packets::AGENT_PROPERTY_HEALTH,
        agent,
        100,
    ));
    frames.push(agent_packets::agent_property_update_int(
        agent_packets::AGENT_PROPERTY_VALUE64,
        agent,
        0,
    ));

    frames.push(instance_packets::instance_player_data_done());
    frames
}


/// One gameplay connection.
pub struct WorldSession {
    log: Log,
    /// Which world this is: a connection that names another is not this world's.
    realm: u32,
    /// The maps the world hosts, so a connection can be told what it may enter.
    maps: Vec<crate::world::MapEntry>,
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
            master: None,
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
            loop {
                let Some(bytes) = pending.first_chunk::<2>() else {
                    break;
                };
                let header = u16::from_le_bytes(*bytes);
                let Some(size) = game_message_size(header) else {
                    self.say(
                        LogLevel::Warn,
                        format!(
                            "Gameplay connection from {peer}: a message with header 0x{header:04X} has no size this world knows, so the stream cannot be split any further. The bytes so far are {}.",
                            shared::network::socket::hex(&pending)
                        ),
                    );
                    let _ = io.close().await;
                    return;
                };
                if pending.len() < size {
                    break;
                }
                let message: Vec<u8> = pending.drain(..size).collect();

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
                    .answer(io, &peer, header, &message, &mut creation, &mut crypt)
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
    async fn answer(
        &self,
        io: &mut SessionIo,
        peer: &str,
        header: u16,
        message: &[u8],
        creation: &mut Creation,
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
                let frames = creation_player_block(creation);
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
                // The confirm writes the character's rows and answers with a status. Neither the
                // character schema nor the answer exists in this tree yet, so the connection is
                // closed rather than left waiting on a screen that will never finish.
                self.say(
                    LogLevel::Error,
                    format!(
                        "Gameplay connection from {peer}: the creation screen confirmed a character ({} bytes), and this world cannot write one yet - the character schema is not built. The connection is closed.",
                        message.len()
                    ),
                );
                let _ = io.close().await;
                Some(false)
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
        let expected = creation_player_block(&Creation::start());
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
        assert_eq!(
            headers,
            vec![
                0x0186, // instance_player_data_start
                0x00EA, 0x00EB, 0x00EC, 0x00ED, // the four faction caps
                0x0037, // attribute points
                0x00B7, // profession
                0x00B6, // unlocked professions
                0x00DA, // skillbar
                0x00DB, // unlocked skills
                0x00E9, // faction record
                0x003A, // attributes
                0x009F, 0x009F, 0x009F, // energy, health, and the third property
                0x018A, // instance_player_data_done
            ]
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
}
