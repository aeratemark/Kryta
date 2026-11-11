//! The end-to-end run: the client's conversation against the world **program**.
//!
//! The session tests in `game` play the conversation against a socket manager a test builds. This
//! test plays it against the binary `worldserver` itself: it spawns the program from this package,
//! waits for its own READY line, and drives a creation screen and a load of map 148 exactly as the
//! manual test says a client does - the version message, the key exchange, the creation confirm, the
//! asks the load is built from, the travel request and the ticks. What is proven is the program: its
//! configuration reading, its listener, its three pools and its sessions, together.
//!
//! It is `#[ignore]` because it needs the databases the program's configuration names, and it is a
//! **live** test: it writes a character row and an instance row, then removes both.
//!
//! Run with `cargo test -p worldserver --test end_to_end -- --ignored --nocapture`, with the two
//! servers of the same databases not running - the live world's sweep would otherwise dispose the
//! instance row this test checks.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::configuration::{Config, Policy};
use common::logging::Log;
use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};
use game::server::packets::instance_packets;
use game::server::protocol::GameVersion;
use shared::cryptography::handshake::{
    self, CLIENT_SEED_HEADER, MASTER_SECRET_LEN, PUBLIC_LEN, SERVER_SEED_LEN,
};

/// The port the program is told to bind: the environment variable wins over the file, and `0` means
/// the operating system picks one - which the program then prints in its READY line.
const PORT_ENV: &str = "AC_WORLD_SERVER_PORT";
/// The status interval, so the program's own liveness reaches this test's log quickly.
const STATUS_ENV: &str = "AC_STATUS_INTERVAL";

/// Wait for one line of the program's output that contains `needle`, and answer it.
fn wait_for(reader: &Receiver<String>, needle: &str, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match reader.recv_timeout(remaining) {
            Ok(line) => {
                if line.contains(needle) {
                    return line;
                }
            }
            Err(_) => panic!("the program never printed a line containing '{needle}'"),
        }
    }
}

/// Start the world program with its own port, and answer the port it bound.
fn start_world() -> (Child, Receiver<String>, u16) {
    let exe = env!("CARGO_BIN_EXE_worldserver");
    let mut child = Command::new(exe)
        .arg("-c")
        .arg("worldserver/worldserver.json")
        .current_dir("..")
        .env(PORT_ENV, "0")
        .env(STATUS_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the world program starts");
    let stdout = child.stdout.take().expect("the program's stdout is piped");
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    });
    let ready = wait_for(&rx, "READY: listening on", Duration::from_secs(60));
    // "READY: listening on 127.0.0.1:41234 as world 2; ..." - the address is the fourth word.
    let address = ready
        .split_whitespace()
        .nth(3)
        .expect("the READY line names the address it bound");
    let port: u16 = address
        .rsplit(':')
        .next()
        .expect("the address has a port")
        .parse()
        .expect("the port is a number");
    (child, rx, port)
}

/// Read exactly `size` bytes, or fail saying how many arrived.
fn read_exact(stream: &mut TcpStream, size: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; size];
    stream
        .read_exact(&mut buffer)
        .unwrap_or_else(|error| panic!("expected {size} byte(s), failed: {error}"));
    buffer
}

/// Read complete frames until the header `terminal` has been seen `count` times, answering every
/// complete frame read, decrypted. The walker is the server table's own: it knows where each message
/// ends, so a counted field is followed to its own length rather than to a guessed number.
fn read_frames(
    stream: &mut TcpStream,
    reading: &mut common::cryptography::Arc4,
    terminal: u16,
    count: usize,
) -> Vec<(u16, Vec<u8>)> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut frames: Vec<(u16, Vec<u8>)> = Vec::new();
    let mut seen = 0;
    let mut chunk = [0u8; 4096];
    loop {
        while !buffer.is_empty() {
            match game::server::protocol::decode_game_message(&buffer, false) {
                Ok((frame, used)) => {
                    let bytes = buffer[..used].to_vec();
                    let header = frame.header();
                    buffer.drain(..used);
                    if header == terminal {
                        seen += 1;
                    }
                    frames.push((header, bytes));
                    if header == terminal && seen == count {
                        return frames;
                    }
                }
                Err(_) => break,
            }
        }
        let read = match stream.read(&mut chunk) {
            Ok(read) => read,
            Err(error) => {
                let walked: Vec<u16> = frames.iter().map(|(header, _)| *header).collect();
                panic!(
                    "the answer stalls with {error}; walked {walked:04X?}; {} bytes left: {}",
                    buffer.len(),
                    shared::network::socket::hex(&buffer[..buffer.len().min(64)])
                );
            }
        };
        assert_ne!(
            read, 0,
            "the program closed the connection before {terminal:#06X}"
        );
        let mut plain = chunk[..read].to_vec();
        reading.apply(&mut plain);
        buffer.extend_from_slice(&plain);
    }
}

/// The version message and the key exchange, answering the cipher each direction reads with.
fn open_connection(
    port: u16,
    version: GameVersion,
) -> (
    TcpStream,
    common::cryptography::Arc4,
    common::cryptography::Arc4,
) {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("the program accepts the connection");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("the socket reads with a timeout");
    stream
        .write_all(&version.encode())
        .expect("the version message is written");
    let mut public = [0u8; PUBLIC_LEN];
    public[0] = 2;
    let mut seed = CLIENT_SEED_HEADER.to_le_bytes().to_vec();
    seed.extend_from_slice(&public);
    stream
        .write_all(&seed)
        .expect("the seed message is written");
    let answer = read_exact(&mut stream, SERVER_SEED_LEN);
    assert_eq!(
        &answer[..2],
        &handshake::SERVER_SEED_HEADER.to_le_bytes(),
        "the server's seed message"
    );
    // The master is what the server's twenty bytes turn into once the same XOR is applied on this
    // side: master = answer[..] XOR shared[..], and the key is the client's digest of the master.
    let shared = handshake::shared_secret(&public).expect("the exchange's own arithmetic");
    let mut master = [0u8; MASTER_SECRET_LEN];
    for index in 0..MASTER_SECRET_LEN {
        master[index] = answer[2 + index] ^ shared[index];
    }
    let key = handshake::derive_key(&master);
    let reading = common::cryptography::Arc4::new(&key);
    let sending = common::cryptography::Arc4::new(&key);
    (stream, reading, sending)
}

/// What the creation conversation is answered: the block, then the confirm, which writes the row.
fn create_character(port: u16, account: [u8; 16], name: &str) -> ([u8; 16], String) {
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
    let (mut stream, mut reading, mut sending) = open_connection(port, version);

    // The creation block: the load head and the creation start, in that order.
    let mut block = read_exact(&mut stream, instance_packets::creation_load_block().len());
    reading.apply(&mut block);
    assert_eq!(
        &block[..2],
        &[0x7C, 0x01],
        "the creation instance's load head: {}",
        shared::network::socket::hex(&block)
    );
    assert_eq!(&block[6..8], &[0x89, 0x01], "the creation start");

    // The confirm, in the shape the recording shows: a counted name and thirty-four bytes after it.
    let units: Vec<u16> = name.encode_utf16().collect();
    assert!(units.len() <= 20, "the name fits the client's field");
    let appearance_at = 4 + units.len() * 2;
    let mut confirm = vec![0u8; appearance_at + 34];
    confirm[0..2].copy_from_slice(&0x808Bu16.to_le_bytes());
    confirm[2..4].copy_from_slice(&(units.len() as u16).to_le_bytes());
    for (index, unit) in units.iter().enumerate() {
        let at = 4 + index * 2;
        confirm[at..at + 2].copy_from_slice(&unit.to_le_bytes());
    }
    confirm[appearance_at..appearance_at + 4].copy_from_slice(&0x00AB_CDEFu32.to_le_bytes());
    let mut scrambled = confirm;
    sending.apply(&mut scrambled);
    stream
        .write_all(&scrambled)
        .expect("the confirm is written");

    // `0x0188`: the character's id, its name, and the settings block.
    let size = 2 + 16 + 2 + units.len() * 2 + 2 + 2 + game::characters::CHARACTER_SETTINGS_LEN;
    let mut answered = read_exact(&mut stream, size);
    reading.apply(&mut answered);
    assert_eq!(
        &answered[..2],
        &0x0188u16.to_le_bytes(),
        "the creation succeeded: {}",
        shared::network::socket::hex(&answered)
    );
    let mut id = [0u8; 16];
    id.copy_from_slice(&answered[2..18]);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    (id, game::characters::uuid_text(&id))
}

/// The three pools the program's configuration names: characters, world and login.
type Pools = (
    Arc<Mutex<DatabaseWorkerPool>>,
    Arc<Mutex<DatabaseWorkerPool>>,
    Arc<Mutex<DatabaseWorkerPool>>,
);

/// Open the pools the program's configuration names: characters, world and login.
fn open_pools() -> Pools {
    let log = Log::new();
    let config = Config::load_json("worldserver.json", false, Policy::default())
        .expect("the world program's own configuration");
    let mut opened = Vec::new();
    for kind in [
        DatabaseKind::Character,
        DatabaseKind::World,
        DatabaseKind::Login,
    ] {
        let settings = read_settings(&config, kind).expect("the pool's settings");
        let mut pool = DatabaseWorkerPool::open(kind, &settings, &log)
            .unwrap_or_else(|error| panic!("the {kind:?} pool opens: {error}"));
        pool.query_with("SELECT 1", &[])
            .unwrap_or_else(|error| panic!("the {kind:?} pool answers: {error}"));
        opened.push(Arc::new(Mutex::new(pool)));
    }
    let login = opened.pop().expect("three pools");
    let world = opened.pop().expect("three pools");
    let characters = opened.pop().expect("three pools");
    (characters, world, login)
}

#[test]
#[ignore = "needs the databases the world program's configuration names"]
fn an_end_to_end_creation_and_load_against_the_live_program() {
    let name = "E2E Kryta One";
    let (mut child, _lines, port) = start_world();
    println!("the world program is listening on port {port}");

    let (characters, world_pool, login) = open_pools();
    let created = Arc::new(Mutex::new(None::<String>));
    let created_for_cleanup = Arc::clone(&created);
    let copy = Arc::new(Mutex::new(None::<i64>));
    let copy_for_cleanup = Arc::clone(&copy);

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // The account the character is written under: the login database's own column. The world
        // does not verify it beyond writing it into the row, and the login's list would answer it.
        let account_text: String = {
            let mut pool = login.lock().unwrap();
            let rows = pool
                .query_with(
                    "SELECT \"user_id\" FROM \"account\" WHERE \"username\" = 'user1@example.com'",
                    &[],
                )
                .expect("the account is readable");
            assert!(
                !rows.is_empty(),
                "the owner's account exists: user1@example.com"
            );
            rows[0].get(0)
        };
        let account = game::characters::uuid_bytes(&account_text).expect("the account's uuid");
        println!("the character is written under account {account_text}");

        // The creation screen, against the program.
        let (id, uuid_text) = create_character(port, account, name);
        *created.lock().unwrap() = Some(uuid_text.clone());
        println!("the creation wrote {uuid_text}");
        assert_ne!(id, [0u8; 16], "the answer carries the character's id");

        // The row is there, with its starting items and its starting unlock - the write is the
        // program's own, through the same repository the load draws from.
        {
            let repository =
                game::characters::DatabaseCharacters::new(Arc::clone(&characters), Log::new());
            let held = repository
                .items_of(&uuid_text)
                .expect("the items are readable");
            println!("the confirm wrote {} item row(s)", held.len());
            assert!(!held.is_empty(), "the confirm wrote the starting items");
            let unlocked = repository
                .unlocked_maps(&uuid_text)
                .expect("the unlocks are readable");
            assert_eq!(unlocked, vec![148], "the starting unlock is map 148 (R12)");
        }

        // The load of map 148, in the measured order, against the program. The version message
        // names the map itself: this connection presents no travel record, so the map is its own.
        let version = GameVersion {
            version: 39167,
            h0008: 1,
            map_token: 2,
            map_id: 148,
            player_token: 0x2345,
            account,
            character: id,
            h0038: 0,
            h003c: 0,
        };
        let (mut stream, mut reading, mut sending) = open_connection(port, version);

        // The block: head, player-data start, the name, the instance's record.
        let block_size = instance_packets::instance_load_head().len()
            + instance_packets::instance_player_data_start().len()
            + instance_packets::instance_load_player_name(name).len()
            + instance_packets::instance_load_info(1, 148, 0, 1, 0, 0).len();
        let mut block = read_exact(&mut stream, block_size);
        reading.apply(&mut block);
        assert_eq!(
            &block[..2],
            &[0x7C, 0x01],
            "the outpost's load head, not the creation instance's: {}",
            shared::network::socket::hex(&block)
        );
        let name_size = instance_packets::instance_load_player_name(name).len();
        let name_at = instance_packets::instance_load_head().len()
            + instance_packets::instance_player_data_start().len();
        assert_eq!(
            &block[name_at..name_at + name_size],
            instance_packets::instance_load_player_name(name).as_slice(),
            "the character's own name, from its row"
        );
        assert_eq!(
            &block[name_at + name_size..],
            instance_packets::instance_load_info(1, 148, 0, 1, 0, 0).as_slice(),
            "the instance's record: map 148, an outpost, district 1"
        );

        // The ping pair, unasked.
        let mut ping = read_exact(&mut stream, 8);
        reading.apply(&mut ping);
        assert_eq!(&ping[..2], &[0x0C, 0x00], "the ping request");
        assert_eq!(&ping[2..4], &[0x0D, 0x00], "its reply");

        // The items ask: the character's items, the map's record and the manifest. The answer ends
        // at the manifest's second end message.
        let mut ask = vec![0x91, 0x80, 0x00, 0x00];
        sending.apply(&mut ask);
        stream.write_all(&ask).expect("the items ask");
        let items = read_frames(&mut stream, &mut reading, 0x0198, 2);
        let headers: Vec<u16> = items.iter().map(|(header, _)| *header).collect();
        for header in [0x0144u16, 0x0070, 0x0030, 0x0099, 0x01AC, 0x0199, 0x0198] {
            assert!(
                headers.contains(&header),
                "the items answer carries 0x{header:04X}: {headers:04X?}"
            );
        }
        assert_eq!(
            *headers.last().unwrap(),
            0x0198,
            "the manifest ends the answer"
        );

        // The spawn ask: where it stands, and the first tick.
        let mut ask = vec![0x88, 0x80];
        sending.apply(&mut ask);
        stream.write_all(&ask).expect("the spawn ask");
        let mut spawn = read_exact(&mut stream, 26 + 6);
        reading.apply(&mut spawn);
        assert_eq!(&spawn[..2], &[0x96, 0x01], "the entry position");
        assert_eq!(
            &spawn[2..6],
            &112_509u32.to_le_bytes(),
            "map 148's file id, as the recording carried it"
        );
        let x = f32::from_le_bytes([spawn[6], spawn[7], spawn[8], spawn[9]]);
        let y = f32::from_le_bytes([spawn[10], spawn[11], spawn[12], spawn[13]]);
        assert!(
            (x - 9541.87).abs() <= 250.0 && (y - 4796.46).abs() <= 250.0,
            "the point is scattered within the world's spawn scatter of the map's own spawn ({x}, {y})"
        );
        assert_eq!(&spawn[26..28], &[0x1E, 0x00], "the first simulation tick");

        // The players ask: the PvP halves, the character's body and the end of the load. The ask's
        // own wire is nineteen bytes: a sixteen-byte blob and one byte (docs/port/02-wire.md row 14).
        let mut ask = vec![0u8; 19];
        ask[0] = 0x90;
        ask[1] = 0x80;
        sending.apply(&mut ask);
        stream.write_all(&ask).expect("the players ask");
        let players = read_frames(&mut stream, &mut reading, 0x018E, 1);
        assert_eq!(&players[0].1[..2], &[0x1D, 0x00], "no unlocked PvP skills");
        assert_eq!(
            players.last().unwrap().0,
            0x018E,
            "the load's last message: {players:?}"
        );

        // From here the world ticks the connection: one `0x001E` arrives on the interval.
        let mut tick = read_exact(&mut stream, 6);
        reading.apply(&mut tick);
        assert_eq!(&tick[..2], &[0x1E, 0x00], "the simulation tick");

        // The item operations the manual test's steps 9 to 11 do, against the program: a drag, an
        // equip and a destroy. Each one is a row in the character database and an answer to the
        // client; the items are the ones the confirm wrote, read back through the same repository.
        let tables = game::creation::Tables::load(
            "../data/inventory.json",
            "../data/equipment.json",
            &Log::new(),
        )
        .expect("the creation data");
        let templates = game::data_stores::DatabaseItems::new(
            Arc::clone(&world_pool),
            Log::new(),
        )
        .list()
        .expect("the item templates are readable");
        let held = game::characters::DatabaseCharacters::new(
            Arc::clone(&characters),
            Log::new(),
        )
        .items_of(&uuid_text)
        .expect("the character's items are readable");
        assert!(!held.is_empty(), "the load had items to operate on");

        // 9. A drag: where the item goes is the row, and the client is told the same placement.
        let dragged = &held[0];
        let mut ask = vec![0u8; 9];
        ask[0..2].copy_from_slice(&0x8072u16.to_le_bytes());
        ask[2..6].copy_from_slice(&dragged.item_id.to_le_bytes());
        ask[6..8].copy_from_slice(&dragged.bag_id.to_le_bytes());
        ask[8] = dragged.slot;
        sending.apply(&mut ask);
        stream.write_all(&ask).expect("the drag");
        let mut placed = read_exact(&mut stream, 11);
        reading.apply(&mut placed);
        assert_eq!(&placed[..2], &[0x3E, 0x01], "the placement: {}", shared::network::socket::hex(&placed));
        assert_eq!(&placed[4..8], &dragged.item_id.to_le_bytes(), "the item");
        assert_eq!(&placed[8..10], &dragged.bag_id.to_le_bytes(), "the bag");
        assert_eq!(placed[10], dragged.slot, "the slot");

        // 10. An equip: the item goes to the slot its kind is worn in, in the equipped bag.
        let template_of = |item_id: u32| {
            let row = held.iter().find(|row| row.item_id == item_id);
            row.and_then(|row| {
                templates
                    .iter()
                    .find(|template| template.id == row.template_id)
                    .cloned()
            })
        };
        let wearable = held.iter().find_map(|row| {
            let template = template_of(row.item_id)?;
            game::creation::equipped_slot(template.item_type)?;
            Some((row.item_id, template.item_type))
        });
        if let Some((item_id, item_type)) = wearable {
            let worn_slot = game::creation::equipped_slot(item_type).expect("the kind is worn");
            let equipped_bag = game::creation::equipped_bag(&tables).expect("the equipped bag");
            let mut ask = vec![0u8; 6];
            ask[0..2].copy_from_slice(&0x8030u16.to_le_bytes());
            ask[2..6].copy_from_slice(&item_id.to_le_bytes());
            sending.apply(&mut ask);
            stream.write_all(&ask).expect("the equip");
            let mut placed = read_exact(&mut stream, 11);
            reading.apply(&mut placed);
            assert_eq!(&placed[..2], &[0x3E, 0x01], "the placement");
            assert_eq!(&placed[4..8], &item_id.to_le_bytes(), "the item");
            assert_eq!(placed[8..10], equipped_bag.id.to_le_bytes(), "the equipped bag");
            assert_eq!(placed[10], worn_slot, "the slot its kind is worn in");
        }

        // 11. A destroy: the row goes, and the client is told the item is gone.
        let doomed = held
            .iter()
            .find(|row| row.item_id != dragged.item_id && template_of(row.item_id).is_some())
            .map(|row| row.item_id)
            .expect("the set holds more than one item");
        let mut ask = vec![0u8; 6];
        ask[0..2].copy_from_slice(&0x8069u16.to_le_bytes());
        ask[2..6].copy_from_slice(&doomed.to_le_bytes());
        sending.apply(&mut ask);
        stream.write_all(&ask).expect("the destroy");
        let mut gone = read_exact(&mut stream, 8);
        reading.apply(&mut gone);
        assert_eq!(&gone[..2], &[0x4D, 0x01], "the item's removal");
        assert_eq!(&gone[4..8], &doomed.to_le_bytes(), "the item");
        {
            let held = game::characters::DatabaseCharacters::new(
                Arc::clone(&characters),
                Log::new(),
            )
            .items_of(&uuid_text)
            .expect("the character's items are readable");
            assert!(
                held.iter().all(|row| row.item_id != doomed),
                "the destroyed item's row is gone"
            );
        }

        // The instance is a row in the world database while the connection is inside it. Its id is
        // kept, so the cleanup removes this test's copy and nothing else.
        {
            let mut pool = world_pool.lock().unwrap();
            let rows = pool
                .query_with(
                    "SELECT \"id\"::bigint, \"map_id\"::bigint, \"district\"::bigint \
                     FROM \"instance\" WHERE \"map_id\" = 148 ORDER BY \"district\"",
                    &[],
                )
                .expect("the instance table is readable");
            assert!(!rows.is_empty(), "a copy of map 148 is open");
            assert_eq!(rows[0].get::<usize, i64>(1), 148);
            assert_eq!(
                rows[0].get::<usize, i64>(2),
                1,
                "district 1, the first with room"
            );
            *copy.lock().unwrap() = Some(rows[0].get::<usize, i64>(0));
        }

        // The client's travel request is read and does not end the connection: a tick follows it.
        let mut travel = vec![0u8; 9];
        travel[0..2].copy_from_slice(&0x00B1u16.to_le_bytes());
        travel[2..4].copy_from_slice(&148u16.to_le_bytes());
        travel[4] = 0;
        travel[5..7].copy_from_slice(&1u16.to_le_bytes());
        travel[7] = 0;
        travel[8] = 0;
        sending.apply(&mut travel);
        stream.write_all(&travel).expect("the travel request");
        let mut tick = read_exact(&mut stream, 6);
        reading.apply(&mut tick);
        assert_eq!(&tick[..2], &[0x1E, 0x00], "the world keeps ticking");

        // Ending the socket releases the copy: its row is emptied or disposed by the world.
        let _ = stream.shutdown(std::net::Shutdown::Both);
        let mut released = false;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            {
                let mut pool = world_pool.lock().unwrap();
                let rows = pool
                    .query_with(
                        "SELECT \"empty_since\" FROM \"instance\" WHERE \"map_id\" = 148",
                        &[],
                    )
                    .expect("the instance table is readable");
                // The column is `timestamptz`: a null is an empty copy that is still held.
                let empty: Option<std::time::SystemTime> =
                    rows.first().and_then(|row| row.get(0));
                if rows.is_empty() || empty.is_none() {
                    released = true;
                }
            }
            if released {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        assert!(
            released,
            "the copy is empty or gone after the connection ends"
        );
    }));

    // The program is stopped: the test's own rows are what it leaves.
    let _ = child.kill();
    let _ = child.wait();
    while let Ok(line) = _lines.try_recv() {
        if line.contains("Gameplay connection") || line.contains("panicked") || line.contains("WARN")
        {
            println!("world> {line}");
        }
    }

    // Cleanup: the character this test wrote, and the copy the world recorded.
    if let Some(uuid_text) = created_for_cleanup.lock().unwrap().clone() {
        let mut pool = characters.lock().unwrap();
        for statement in [
            "DELETE FROM \"item_instance\" WHERE \"character_uuid\" = $1",
            "DELETE FROM \"character_unlock\" WHERE \"character_uuid\" = $1",
            "DELETE FROM \"characters\" WHERE \"uuid\" = $1",
        ] {
            let _ = pool.query_with(statement, &[&uuid_text]);
        }
    }
    if let Some(id) = *copy_for_cleanup.lock().unwrap() {
        let mut pool = world_pool.lock().unwrap();
        let _ = pool.query_with("DELETE FROM \"instance\" WHERE \"id\" = $1", &[&id]);
    }

    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
