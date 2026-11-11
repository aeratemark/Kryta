//! The end-to-end run: the client's login conversation against the auth **program**.
//!
//! The session tests in this crate play the conversation against a socket manager a test builds.
//! This test plays it against the binary `authserver` itself: it spawns the program from this
//! package, waits for its own READY line, then performs the client's own sequence - the file check,
//! the auth version message, the key exchange, and the portal login message - and reads the answer:
//! the character list the login composes from the character database.
//!
//! It is `#[ignore]` because it needs the login and character databases the program's configuration
//! names. It writes nothing: a login only reads.
//!
//! Run with `cargo test -p authserver --test end_to_end -- --ignored --nocapture`, with the two
//! servers of the same databases not running: this test binds its own port, but the portal's 6601 is
//! not available while another auth program holds it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use common::configuration::{Config, Policy};
use common::logging::Log;
use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};
use shared::cryptography::handshake::{
    self, CLIENT_SEED_HEADER, MASTER_SECRET_LEN, PUBLIC_LEN, SERVER_SEED_LEN,
};

/// The file check's question, as a capture prints it (`docs/evidence/client-protocol.md` section 7).
const FILE_CHECK_QUESTION: [u8; 21] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0xf1, 0x00, 0x10, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00,
];

/// The file check's answer, as a capture prints it: the header, the length and the ids.
const FILE_CHECK_ANSWER_HEADER: [u8; 4] = [0xf1, 0x02, 0x20, 0x00];

/// The auth port is told to bind 0, so the operating system picks one and the program prints it.
const PORT_ENV: &str = "AC_REALM_SERVER_PORT";
/// The portal would bind 6601, which another program may hold; the login conversation needs it not.
const PORTAL_ENV: &str = "AC_PORTAL_ENABLE";
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

/// Start the auth program on its own port, and answer the port it bound.
fn start_auth() -> (Child, Receiver<String>, u16) {
    let exe = env!("CARGO_BIN_EXE_authserver");
    let mut child = Command::new(exe)
        .arg("-c")
        .arg("authserver/authserver.json")
        .current_dir("..")
        .env(PORT_ENV, "0")
        .env(PORTAL_ENV, "0")
        .env(STATUS_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the auth program starts");
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
    // "READY: listening on 0.0.0.0:41234; status reported every 5 s." - the address is the fourth
    // word, and the port is its last colon-separated part.
    let address = ready
        .split_whitespace()
        .nth(3)
        .expect("the READY line names the address it bound");
    let port: u16 = address
        .trim_end_matches(';')
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

/// Read until the connection has been quiet for a moment, answering everything decrypted.
fn read_quiet(stream: &mut TcpStream, reading: &mut common::cryptography::Arc4) -> Vec<u8> {
    let mut all = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return all,
            Ok(n) => {
                let mut plain = chunk[..n].to_vec();
                reading.apply(&mut plain);
                all.extend_from_slice(&plain);
            }
            Err(_) => return all,
        }
    }
}

/// The account the login presents: the owner's own row, read from the login database.
fn account_of(user: &str) -> [u8; 16] {
    let log = Log::new();
    let config = Config::load_json("authserver.json", false, Policy::default())
        .expect("the auth program's own configuration");
    let settings = read_settings(&config, DatabaseKind::Login).expect("the login settings");
    let mut pool = DatabaseWorkerPool::open(DatabaseKind::Login, &settings, &log)
        .expect("the login pool opens");
    let rows = pool
        .query_with(
            "SELECT \"user_id\" FROM \"account\" WHERE \"username\" = $1",
            &[&user],
        )
        .expect("the account is readable");
    assert!(!rows.is_empty(), "the account {user} exists");
    let text: String = rows[0].get(0);
    game::characters::uuid_bytes(&text).expect("the account's uuid")
}

/// The travel rows the account has, as (map, district, language).
fn travel_rows(account: &[u8; 16]) -> Vec<(i64, i64, i64)> {
    let log = Log::new();
    let config = Config::load_json("authserver.json", false, Policy::default())
        .expect("the auth program's own configuration");
    let settings = read_settings(&config, DatabaseKind::Login).expect("the login settings");
    let mut pool = DatabaseWorkerPool::open(DatabaseKind::Login, &settings, &log)
        .expect("the login pool opens");
    let account_text = game::characters::uuid_text(account);
    let rows = pool
        .query_with(
            "SELECT \"map_id\"::bigint, \"district\"::bigint, \"language\"::bigint \
             FROM \"travel\" WHERE \"account\" = $1 ORDER BY \"id\" DESC",
            &[&account_text],
        )
        .expect("the travel table is readable");
    rows.iter()
        .map(|row| {
            (
                row.get::<usize, i64>(0),
                row.get::<usize, i64>(1),
                row.get::<usize, i64>(2),
            )
        })
        .collect()
}

/// Remove every travel row the account has: the test's own hand-off.
fn remove_travel(account: &[u8; 16]) {
    let log = Log::new();
    let config = Config::load_json("authserver.json", false, Policy::default())
        .expect("the auth program's own configuration");
    let settings = read_settings(&config, DatabaseKind::Login).expect("the login settings");
    let mut pool = DatabaseWorkerPool::open(DatabaseKind::Login, &settings, &log)
        .expect("the login pool opens");
    let account_text = game::characters::uuid_text(account);
    let _ = pool.query_with("DELETE FROM \"travel\" WHERE \"account\" = $1", &[&account_text]);
}

/// A character of this test's own, written for the account and removed afterwards: the login's list
/// is composed from the character database's rows, and a row the test owns proves it without
/// depending on what a client created by hand.
fn write_character(account: &[u8; 16], name: &str) -> String {
    let log = Log::new();
    let config = Config::load_json("authserver.json", false, Policy::default())
        .expect("the auth program's own configuration");
    let settings = read_settings(&config, DatabaseKind::Character).expect("the character settings");
    let mut pool = DatabaseWorkerPool::open(DatabaseKind::Character, &settings, &log)
        .expect("the character pool opens");
    let uuid = "00000000-0000-0000-0000-0000000000E2";
    let account_text = game::characters::uuid_text(account);
    pool.query_with(
        "DELETE FROM \"characters\" WHERE \"uuid\" = $1",
        &[&uuid],
    )
    .expect("the row is removable");
    pool.query_with(
        "INSERT INTO \"characters\" (\"uuid\", \"account_uuid\", \"name\", \"appearance\", \"primary_profession\", \"unlocked_professions\", \"campaign\", \"level\") \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        &[
            &uuid,
            &account_text,
            &name,
            &0i64,
            &1i64,
            &2i64,
            &0i64,
            &1i64,
        ],
    )
    .expect("the row is written");
    uuid.to_string()
}

/// Remove the row `write_character` wrote.
fn remove_character(uuid: &str) {
    let log = Log::new();
    let config = Config::load_json("authserver.json", false, Policy::default())
        .expect("the auth program's own configuration");
    let settings = read_settings(&config, DatabaseKind::Character).expect("the character settings");
    let mut pool = DatabaseWorkerPool::open(DatabaseKind::Character, &settings, &log)
        .expect("the character pool opens");
    let _ = pool.query_with("DELETE FROM \"characters\" WHERE \"uuid\" = $1", &[&uuid]);
}

/// The 62-byte settings block the login carries for one character: the row's own, read back through
/// the same repository the login uses.
fn settings_blob_of(uuid: &str) -> Vec<u8> {
    let log = Log::new();
    let config = Config::load_json("authserver.json", false, Policy::default())
        .expect("the auth program's own configuration");
    let settings = read_settings(&config, DatabaseKind::Character).expect("the character settings");
    let pool = DatabaseWorkerPool::open(DatabaseKind::Character, &settings, &log)
        .expect("the character pool opens");
    let characters = game::characters::DatabaseCharacters::new(
        std::sync::Arc::new(std::sync::Mutex::new(pool)),
        log,
    );
    characters
        .list()
        .into_iter()
        .find(|character| game::characters::uuid_text(&character.uuid) == uuid)
        .map(|character| character.settings.to_blob())
        .expect("the character's row is there")
}

/// The world program's own `WorldServerPort`: the hand-off's address is compared with the world
/// program's configuration, not with a number written into the test.
fn world_server_port() -> u16 {
    let config = Config::load_json("../worldserver/worldserver.json", false, Policy::default())
        .expect("the world program's own configuration");
    config
        .value::<u16>("WorldServerPort", 6113)
        .expect("the key is a port")
        .0
}

#[test]
#[ignore = "needs the login and character databases the auth program's configuration names"]
fn an_end_to_end_login_against_the_live_program() {
    let (mut child, lines, port) = start_auth();
    println!("the auth program is listening on port {port}");

    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("the program accepts the connection");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("the socket reads with a timeout");

    // 1. The file check, before anything else on this socket: the client compares the answer.
    stream
        .write_all(&FILE_CHECK_QUESTION)
        .expect("the file check question");
    let answer = read_exact(&mut stream, 32);
    assert_eq!(
        &answer[..4], &FILE_CHECK_ANSWER_HEADER,
        "the file check answers its own header and length: {}",
        shared::network::socket::hex(&answer)
    );

    // 2. The auth version message: the bare little-endian header, the version, and twelve bytes.
    let mut version = vec![0u8; 16];
    version[0..4].copy_from_slice(&0x000C_0400u32.to_le_bytes());
    version[4..8].copy_from_slice(&39167u32.to_le_bytes());
    stream
        .write_all(&version)
        .expect("the auth version message");

    // 3. The key exchange: the client's public value, the server's seed, and the master secret
    // recovered from it - the same arithmetic the two programs run.
    let mut public = [0u8; PUBLIC_LEN];
    public[0] = 2;
    let mut seed = CLIENT_SEED_HEADER.to_le_bytes().to_vec();
    seed.extend_from_slice(&public);
    stream.write_all(&seed).expect("the seed message");
    let answer = read_exact(&mut stream, SERVER_SEED_LEN);
    assert_eq!(
        &answer[..2],
        &handshake::SERVER_SEED_HEADER.to_le_bytes(),
        "the server's seed message"
    );
    let shared = handshake::shared_secret(&public).expect("the exchange's own arithmetic");
    let mut master = [0u8; MASTER_SECRET_LEN];
    for index in 0..MASTER_SECRET_LEN {
        master[index] = answer[2 + index] ^ shared[index];
    }
    let key = handshake::derive_key(&master);
    let mut reading = common::cryptography::Arc4::new(&key);
    let mut sending = common::cryptography::Arc4::new(&key);

    // The greeting: the session info and the heartbeat, sixteen bytes, the first scrambled bytes.
    let mut greeting = read_exact(&mut stream, 16);
    reading.apply(&mut greeting);
    assert_eq!(
        &greeting[..2],
        &[0x01, 0x00],
        "the session info: {}",
        shared::network::socket::hex(&greeting)
    );

    // 4. The portal login: the account's own id, a session id, and the two counted strings. The
    // wire is 54 bytes - the measured login message - not the 150 that the game-side table declares
    // as a maximum: the two counted strings carry what they carry, and the server's walk follows
    // them.
    let account = account_of("user1@example.com");
    let character_name = "E2E Login One";
    let created = write_character(&account, character_name);
    let mut login = vec![0u8; 54];
    login[0..2].copy_from_slice(&0x8038u16.to_le_bytes());
    login[2..6].copy_from_slice(&7u32.to_le_bytes());
    login[6..22].copy_from_slice(&account);
    login[22..38].copy_from_slice(&[0u8; 16]);
    // The two counted strings: four units and two units, as the trace's own login carried.
    let name: Vec<u16> = "user".encode_utf16().collect();
    login[38..40].copy_from_slice(&(name.len() as u16).to_le_bytes());
    for (index, unit) in name.iter().enumerate() {
        let at = 40 + index * 2;
        login[at..at + 2].copy_from_slice(&unit.to_le_bytes());
    }
    let at = 40 + name.len() * 2;
    let account_type: Vec<u16> = "ok".encode_utf16().collect();
    login[at..at + 2].copy_from_slice(&(account_type.len() as u16).to_le_bytes());
    for (index, unit) in account_type.iter().enumerate() {
        let offset = at + 2 + index * 2;
        login[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
    }
    assert_eq!(login.len(), 54, "the login message is 54 bytes");
    let mut scrambled = login;
    sending.apply(&mut scrambled);
    stream.write_all(&scrambled).expect("the login message");

    // 5. The answer: the character list, which the login composes from the character database.
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("the socket reads with a timeout");
    let answered = read_quiet(&mut stream, &mut reading);
    while let Ok(line) = lines.try_recv() {
        if line.contains("Connection from") || line.contains("login") || line.contains("listed") {
            println!("auth> {line}");
        }
    }
    assert!(
        answered.len() >= 2,
        "the login answer is there at all: {}",
        shared::network::socket::hex(&answered)
    );
    assert_eq!(
        &answered[..2],
        &[0x07, 0x00],
        "the first message is the character list: {}",
        shared::network::socket::hex(&answered[..32.min(answered.len())])
    );
    // The character's name is in the list, as a counted UTF-16 string: the count precedes it.
    let name_units: Vec<u8> = character_name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert!(
        answered
            .windows(name_units.len())
            .any(|window| window == name_units.as_slice()),
        "the answer names the account's character '{character_name}': {}",
        shared::network::socket::hex(&answered)
    );
    // And the 62-byte settings block is the row's own: the frame carries it right after the name,
    // counted. The row is read back through the same repository the login uses.
    let expected_settings = settings_blob_of(&created);
    let settings_at = 30 + name_units.len();
    assert_eq!(
        &answered[settings_at - 2..settings_at],
        &(game::characters::CHARACTER_SETTINGS_LEN as u16).to_le_bytes(),
        "the settings block's count"
    );
    assert_eq!(
        &answered[settings_at..settings_at + game::characters::CHARACTER_SETTINGS_LEN],
        expected_settings.as_slice(),
        "the settings block the login carries is the row's own"
    );

    // 6. The map request: the client names the map it wants, and the login answers with the world it
    // was told to dial and records the hand-off the world claims (R9). The wire is the traced six
    // dwords: request, map type, map, region, district, language.
    remove_travel(&account);
    let mut request = vec![0u8; 26];
    request[0..2].copy_from_slice(&0x8029u16.to_le_bytes());
    request[2..6].copy_from_slice(&3u32.to_le_bytes());
    request[6..10].copy_from_slice(&1u32.to_le_bytes());
    request[10..14].copy_from_slice(&148u32.to_le_bytes());
    request[18..22].copy_from_slice(&1u32.to_le_bytes());
    let mut scrambled = request;
    sending.apply(&mut scrambled);
    stream.write_all(&scrambled).expect("the map request");
    let answered = read_quiet(&mut stream, &mut reading);
    assert!(
        answered.len() >= 52,
        "the map answer is 42 bytes and the response 10: {}",
        shared::network::socket::hex(&answered)
    );
    assert_eq!(
        &answered[..2],
        &[0x09, 0x00],
        "the game server info: {}",
        shared::network::socket::hex(&answered[..answered.len().min(24)])
    );
    assert_eq!(
        &answered[42..44],
        &[0x03, 0x00],
        "the request response follows the info"
    );
    // The address it names is the world's own: the family, the port big-endian, and the address -
    // the bytes the client dials. The port is the world program's `WorldServerPort`.
    let world_port = world_server_port();
    let host = [
        0x02, 0x00,
        (world_port >> 8) as u8,
        (world_port & 0xFF) as u8,
        0x7F, 0x00, 0x00, 0x01,
    ];
    assert!(
        answered
            .windows(host.len())
            .any(|window| window == host.as_slice()),
        "the info names 127.0.0.1:{world_port}: {}",
        shared::network::socket::hex(&answered[..answered.len().min(44)])
    );
    // The login recorded the hand-off for the world it names: the map it was asked for, and the
    // district and language the request carried.
    let travels = travel_rows(&account);
    assert_eq!(
        travels.first().copied(),
        Some((148, 1, 0)),
        "the travel the login recorded: {travels:?}"
    );

    let _ = stream.shutdown(std::net::Shutdown::Both);
    let _ = child.kill();
    let _ = child.wait();
    remove_character(&created);
    remove_travel(&account);
    while let Ok(line) = lines.try_recv() {
        if line.contains("Connection from") || line.contains("login") || line.contains("listed") {
            println!("auth> {line}");
        }
    }
}
