//! A one-time repair run, not a test: the world database's schema is reset so the schema-only files
//! apply from scratch and the content tables come up empty. Delete this file after running it.
//!
//! `cargo test -p worldserver --test db_reset -- --nocapture`

use common::configuration::{Config, Policy};
use common::logging::Log;
use database::database::{read_settings, DatabaseKind, DatabaseWorkerPool};

#[test]
fn reset_world_schema() {
    let log = Log::new();
    let config = Config::load(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/worldserver.json")),
        false,
        Policy::default(),
    )
    .expect("the world configuration");

    let (line, _notes) = config
        .value_string("WorldDatabaseInfo", "")
        .expect("the world database line");
    let parts: Vec<&str> = line.split(';').collect();
    println!(
        "WorldDatabaseInfo: host={} db={} fields={}",
        parts.first().copied().unwrap_or(""),
        parts.get(4).copied().unwrap_or(""),
        parts.len()
    );

    let settings = read_settings(&config, DatabaseKind::World).expect("the world pool settings");
    let mut pool = DatabaseWorkerPool::open_with_trace(DatabaseKind::World, &settings, &log, false)
        .expect("the world pool opens");

    for statement in ["DROP SCHEMA IF EXISTS public CASCADE", "CREATE SCHEMA public"] {
        pool.query_with(statement, &[])
            .unwrap_or_else(|error| panic!("{statement}: {error}"));
        println!("ran: {statement}");
    }
}
