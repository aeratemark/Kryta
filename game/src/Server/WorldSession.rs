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

use crate::server::packets::creation_load_block;
use crate::server::protocol::GameVersion;
use crate::world::World;

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

        // From here the client asks what it needs for the creation screen. Those answers are the
        // next stage; what arrives is logged so the log says exactly what is missing.
        let mut buffer = vec![0u8; 1024];
        loop {
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
            let header = if plain.len() >= 2 {
                u16::from_le_bytes([plain[0], plain[1]])
            } else {
                0
            };
            self.say(
                LogLevel::Info,
                format!(
                    "Gameplay connection from {peer}: message 0x{header:04X} with {read} bytes, which this world does not answer yet."
                ),
            );
            {
                let shown = plain.len().min(64);
                self.log.out(
                    LogLevel::Debug,
                    "network.trace",
                    format!(
                        "trace {peer} game in  {read} bytes: {}{}",
                        shared::network::socket::hex(&plain[..shown]),
                        if plain.len() > shown { " ..." } else { "" }
                    ),
                );
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
