# Guidelines: values, files, and the code that reads them

Draft for the owner, 2026-11-06. When approved this file replaces `AGENTS.md` section 4a and the
decision rows D34 and D35, which are now the decision log further down this file. Until it is
approved, `AGENTS.md` is what binds.

## What the project is, read from the code

- 10 crates, 81 files, 34,863 lines (`docs/37-rework-layers.md`).
- A setting can live in two places today: a JSON file under `config/`, or a number compiled into
  Rust. The database holds operational state - characters, sessions, leases - not settings.
- 14 files are read at start-up: 8 JSON files and 1 TOML file under `config/`, `.env` for secrets,
  and the world pack, 430 JSON files of which 128 are map folders.
- Content still compiled into Rust: `default_equipment.rs` 2,957 lines, `pvp_items.rs` 1,566,
  `path_tables.rs`, `fa_compress_tables.rs`, `travel_targets.rs`.

## The principle every guideline below comes from

The JSON file is the surface. A person opens it, reads it, and changes what it says. The code is
the reader of that surface and nothing more. Everything below protects one of two things: that the
surface is complete, or that the reader is a reader.

## The guidelines

**G1 - One value, one home.** A value lives in exactly one file. A second copy in code, in SQL, or
in another file is a second answer to the same question, and the two will drift.
*Check:* grep the value's key across `crates/` and `config/`; one hit.

**G2 - What a person changes is on the surface.** If a number can be changed without changing what
the client understands, it belongs in a file. The argument "the contract never changes" does not
apply: the value is still obscured in code and unreachable by the person who owns it.
*Check:* a compiled constant that answers an operational question is a violation, and the question
"who would change this" has an answer other than "a programmer".

**G3 - Every value says what it means.** One sentence beside each value, in the file, because JSON
has no comments. A reader who has never seen the code must be able to say what the number does.
*Check:* a key without a `doc` sentence.

**G4 - No provenance, no reporting in a file.** "measured", "implemented", "unverified" describe
how much we trust a value. They belong in `docs/` beside the recording. The file answers "what is
it", the document answers "why do we believe it".
*Check:* a status word in a `config/` file.

**G5 - A file holds values, not decisions.** "If the map is not an outpost, place the character in
its last one" is a decision. It lives in `asura-rules`, takes the file's values as arguments, holds
none of its own, reads no file and opens no connection.
*Check:* `crates/asura-rules/src/lib.rs` and any crate that calls it.

**G6 - A typo is an error.** A key the reader does not know is refused, never ignored. Silent
acceptance runs the server at a value nobody wrote.
*Check:* every deserialised struct refuses unknown fields.

**G7 - No hidden feature.** Nothing answers for the file. No compiled fallback table, no second
reader for the same content, no alias that substitutes one value for another, no code path that
produces an answer the file should have given. If a value is missing, the code says so and stops.
*Check:* a default in code for a value that also exists in a file; a reader that is not
`asura-config::*`; dead SQL over content tables.

**G8 - A value nobody measured is not a value.** The key is `null`, and the caller refuses the
action with a logged reason. It never picks a stand-in on the player's behalf.
*Check:* a `null` that reaches a default instead of a refusal.

**G9 - A marker must drive something.** A flag in a file that the code collects and then ignores is
a lie about the file's authority. `immutable` is the example: the file is editable, so the marker
exists to say this value must not be changed - and a marker the code only prints, or only checks,
must be stated as exactly that in this file, or removed.
*Check:* every flag has a line of code whose behaviour depends on it, or a line here saying it is
advice only.

**G10 - Straightforward beats clever.** One shape for reading a file: same path rule, same error
behaviour, same log line. No extra validation beyond what the client or the arithmetic requires.
No value converted silently. If two files behave differently, that difference is written down in
this file.
*Check:* read the readers side by side; any difference not stated here is a violation.

## The missing-file rule

A file is missing when the code needs it. Nothing about a file is missing in advance.

| class | when it is read | a missing file means | today |
|---|---|---|---|
| needed before the process can serve | at start-up | refuse to start | `file_check.json`, `defaults.json` (`asura-gateway/src/main.rs:93`-`:99`), `services.toml`, the pack (`asura-world/src/main.rs:178`) |
| needed by one action | when that action happens | that action refuses with a reason, the process runs on | movement, spawns, portals, directional geometry, character spawn and defaults, a map's own files |
| absence is content | never "missing" | this content is not here | a map with no folder: the pack walk reports what exists (`asura-world/src/main.rs:182`-`:185`) |

So "halt" has one meaning per class, and the class is a property of the file, decided once.

## One type carries the ruling

The class is declared where the path is, and the type makes the declaration impossible to skip:

```
ConfigFile<T> {
    path: &'static str,
    need: Need,          // AtStart | OnUse
}
```

`load()` resolves the path, reads the file, parses it with unknown keys refused, and returns `T` or
an error naming the file, the key and the requirement. `AtStart` fails the boot. `OnUse` returns
the error to the caller, which refuses that action.

Every reader is one line: `ConfigFile::new("config/Server/intervals.json", Need::AtStart)`. One
path rule, one parse rule, one error shape, one log line, and the requirement written down beside
the path instead of remembered.

The pack walk is not a `ConfigFile`: a map folder that does not exist is content the server does not
have, not a file to halt on.

## Where the code stands against these today

| guideline | violation | where |
|---|---|---|
| G1 | a compiled default for a value that also has a file | `asura-config/src/lib.rs:101`, `:252`, `:328` (the three `Default` impls) |
| G1 | a second reader of the pack's `basic.json` | `asura-world/src/instance_table.rs:101` beside `asura-config/src/world.rs:26` |
| G4 | provenance inside a config file | `config/Server/intervals.json:2`: the `note` key reports what is measured and what is unverified |
| G7 | five queries over content tables nothing writes | `asura-db/src/game.rs:60`, `:73`, `:90`, `:103`, `:442` |
| G7 | a doc comment that sends an author to SQL for content | `asura-db/src/lib.rs:26` |
| G9 | `immutable` is read only to print a line | `asura-config/src/lib.rs:589`, `:646` |
| G10 | two error behaviours for a missing file | fall back: `intervals`, `movement`, `spawns`, `portals`, `directional_movement`. refuse: the rest |

## Layer decisions: why the layers are arranged as they are

> Moved here on 2026-11-06; it was its own file, `40-guidelines.md`. The text below is as it was written.

One entry per decision, with the reason and the evidence. This is the log the checklist in
`docs/37-rework-layers.md` points to: the checklist tracks what moved, this file tracks why the
rest was left where it is.

Status values: **accepted** (decided, do not re-open without a reason), **provisional** (decided
for now, the named condition would change it), **open** (not decided).

| # | decision | status | reason | evidence |
|---|---|---|---|---|
| D1 | A layer is a boundary one change can be made behind, and the project has eighteen of them | accepted | A change is made, tested and reverted per layer; a finer split makes each move unverifiable, a coarser one makes the blast radius unknown | `docs/37-rework-layers.md`, 89 files, 38,689 lines measured |
| D2 | A layer counts as covered only under six conditions: file is truth, every reader switched, old source deleted, equivalence diff empty, ascii plus touched tests pass, client tested where visible | accepted | A layer that "mostly moved" is two sources of truth, which is the defect being removed | `docs/37-rework-layers.md` |
| D3 | Content is JSON; `config/services.toml` stays TOML; YAML rejected | accepted | TOML has no null, and absence is meaningful in this content (`to_instance: Option<i32>`, `arrive: Option<(f32,f32)>`); the tooling already emits JSON; no crate has `serde_json` yet | `crates/asura-world/src/portals.rs:20-25`, `data/portals.json`, `crates/asura-supervisor/Cargo.toml` |
| D4 | The code is the truth, not a commit title or a document | accepted | `travel.rs:154-166` prints the `0x80C1` touch and returns; commit `8bcc029` is titled "the touch now starts the crossing". The code runs, the title does not | `crates/asura-world/src/travel.rs:154-166`, `git log` |
| D5 | Layer 1 (wire and codec) takes no configuration, ever | accepted | Its sizes and field orders are fixed by the game client; a config that could change one breaks the client, and editing one falsifies evidence | `docs/37-rework-layers.md` layer 1 |
| D6 | A file lives in the layer that owns the fact, not the layer that first consumed it | accepted | The four misplaced things in layer 1 are there because a generator or a test was written next to them | `pvp_items.rs`, `peer.rs`, `login_fixtures.rs` in `asura-protocol` |
| D7 | Layer 1's defect list is worked in this order: test client and fixtures first, `pvp_items.rs` once the content format exists, message-number consolidation last | provisional | Two moves are local (12 call sites); the number consolidation is 245 references over 111 constants in two crates, and its only safety net is "no number changes" | measured: 12 call sites in 4 test files; 245 references |
| D8 | Message numbers have one owner: `packets.rs`; `game/mod.rs`'s constants point at it instead of repeating it | accepted, execution deferred | Two copies of the same number are two answers to "what is message 0x013A" | 403 consts in `packets.rs`, 109 in `game/mod.rs` |
| D9 | The `map_portal` shape is the model for every content move: rows in one place with a `source` column per row, read once at start-up, the compiled table kept as fallback until the diff is empty | accepted | It is the only content move this tree has completed, and it is reversible | `crates/asura-db/migrations/0025_map_portals.sql`, `crates/asura-world/src/main.rs:199-222` |
| D10 | Layers that hold by design stay in Rust: the wire catalogue, the archive-derived map data, cartography, and the recordings | accepted | They are evidence or measured data, not settings; a file that could change them would be a way to lie about the client | `docs/37-rework-layers.md` layers 1, 13, 14, 18 |
| D11 | `game/pvp_items.rs` (1,567 lines, 387 entries, frozen, one reader) leaves the wire layer, but its new home is not chosen | open | Two candidates: a content file under `config/`, or an items module in the content layer. The generator that made it is retired, so the data cannot be regenerated either way | `crates/asura-protocol/src/game/pvp_items.rs:1-12`, `crates/asura-world/src/player.rs:162` |
| D12 | Whether a content file mirrors into the database, keys it, or owns content outright is undecided | superseded by D20 | Every content layer's target depended on it | `docs/37-rework-layers.md` layer 15 |
| D13 | `client::peer` (421 lines, a fake game client) is compiled only behind the `test-client` feature, which `asura-auth` and `asura-world` turn on in `[dev-dependencies]`; the server's production build carries no test client. `client/login_fixtures.rs` is **not** a fixture - it is the 591-byte `AUTH_SMSG_ACCOUNT_SETTINGS` payload that `messages::account_settings` sends on every login (`messages.rs:577`) - so it stays, and only its name is wrong | accepted | A test client in the production path is code that cannot fail in production and is never exercised there; a payload misnamed a fixture invites someone to delete it as dead test data | `crates/asura-protocol/Cargo.toml`, `crates/asura-protocol/src/client/mod.rs`, `crates/asura-auth/tests/login.rs:21`, `crates/asura-db` unused |
| D14 | The 387-row PvP item table moved from the wire layer to layer 11 as `crates/asura-world/src/pvp_items.rs`; the message builder stayed in `asura-protocol` | accepted | It is data the server sends, not a message layout; the rows are identical to the commit before, so the move reverses on its own | commit `fb8e508`, `crates/asura-world/src/player.rs:162` |
| D15 | The wire crate's names promise their direction: `decode_*` is bytes to struct, a bare noun is struct to `Frame`, `*_len`/`*_bytes`/`*_slots` is a size, `pack`/`unpack` is a word, `is_`/`has_`/`can_` is a predicate, nothing takes a `get_` prefix | accepted | Both directions share the crate, so without the rule a reader opens every function to learn which way the bytes move | commit `b2dbc2f`, `crates/asura-protocol/src/client/mod.rs` |
| D16 | Misfiled pieces are named for what they hold: `Appearance` (struct, default, `pack`/`unpack`) is `game/appearance.rs`; `client/game.rs` is `client/game_version.rs`; the fallible builder is `PortalAccountLogin::try_to_frame` while the infallible `RequestResponse::frame` keeps its name | accepted | One file per fact: a bit-field codec for one word is not a message builder, and two modules called `game` forced the reader to know each one's direction | commits `13c9e72`, `33666f8`, `1e450c8` |
| D17 | One lint policy, opted into per crate with `[lints] workspace = true`: `missing_docs` warn, `unsafe_code` deny, `clippy::all` warn, `broken_intra_doc_links` deny. `asura-protocol` is the first to opt in | accepted | Per-crate opt-in lets each layer adopt the policy when visited. The crate's one `unsafe` - ARC4's `write_volatile` zeroization - carries a written `#[allow]` and its SAFETY reason, so the deny still guards every other file | commit `fb112aa`, `Cargo.toml` `[workspace.lints]` |
| D18 | The creation instance is a destination of its own: the last-outpost rule exempts `map_type` 11 exactly as it exempts a main town | superseded by D19 | A client asking for map 0 with type 11 is about to make a character; the rule answered it with the last outpost, twice in one attempt, and the creation screen never opened | commit `7c2cf90`, `crates/asura-auth/src/travel.rs`, `logs/auth.log` |
| D19 | The exemption is keyed on the **request's** type, not the map row's: a `map_type` 11 request for map 0 reaches map 0, and the type-0 request that follows the confirm is answered with `characters.map_id` (the starter outpost the confirm wrote), then the last outpost, then 148 | accepted | D18 exempted by the map row's type, so the mistaken second request also reached map 0, which has no blob and never finishes loading. `Settings` now carries the row's `map_id` for that read. The rule is one client behaviour measured twice in `logs/auth.log`: `map 0 (type 11, ...)` then `map 0 (type 0, region 0, district 0, language 4294967295)` | commit `39f1ed8`, `crates/asura-auth/src/travel.rs`, `crates/asura-db/src/game.rs` |
| D20 | Configurations leave the database: content lives in `config/` and the database keeps operational state only. The move is **not scheduled now** | accepted (direction); timing deferred | The owner's word, 2026-11-03: "we will move several structures (configurations) out of the database, we will use the database for operational function, but it snot the time now". This settles the *destination*, which of D12's three shapes content ends in: files own content, the database holds state. It does not settle the bridge, and the bridge is what layers 2 and 15 need | `docs/37-rework-layers.md` |
| D21 | Operational state stays in the database: `characters`, `bag`, `item`, `session`, `account`, `transfer`, `lease`, `world`, `world_pool`, `service`, `instance_unlock`, and the position rows. Content state leaves: `map_config`, `map_spawn`, `map_portal`, and the 214 content `INSERT`s inside migrations `0013`-`0022` | accepted | The line is "does a row change while the server runs". A character's gold changes; a map's spawn does not. `schema_version` stays with the schema either way | `crates/asura-db/migrations/0001`-`0025`, `docs/37-rework-layers.md` layer 2 |
| D22 | The move order for any content table is: write the file, switch its readers, delete the table last in a new migration. No projection copy | accepted | One source is read at every step, and a broken reader reverts alone while the table is intact. `map_portal` already reads content once at start-up with the old source as fallback, so the pattern exists | D20, D21 above |
| D23 | Content is packaged, not scattered: the destination is a **package / mods folder structure** - one folder per package holding that package's content - not one loose file per map or table. D20's "config/" and D22's "write the file" both mean "write the package" | accepted | The owner's word, 2026-11-03: "we will move to a package or mods folder structure, so pointing to files will not be the thing to make". A package is the unit an author copies and edits, which is what the profiles work needs | `docs/40` D20, `docs/34` |
| D24 | The folder layout is `config/World/<pack>/` with `config/World/Vanilla` as the faithful base pack and one pack loaded per server start; `config/Server/` holds how this machine runs, not what the world contains | accepted | The owner's example, 2026-11-03: "our default configuration will be on C:\...\ASURA\config\World\Vanilla another pack could be on /World/mypack". The two levels separate the two kinds of setting D21 draws: content the world has, and settings this machine uses | `config/World/README.md`, `config/Server/README.md`, `docs/37-rework-layers.md` layer 15 |
| D25 | A service does only its own job. The auth service answers the protocol questions; a **game rule** that a protocol answer happens to need lives in `crates/asura-rules` as a pure function, and the service supplies the data | accepted | The owner's rule, 2026-11-03: "auth must be auth, if features beyond auth are happening, they are on the wrong layer, we can create a new layer or use another one". `travel.rs` had grown the last-outpost rule, the creation exemption and the default outpost, none of which are protocol. The new crate is the layer that had no home: `asura-db` is storage, and `asura-world` may not exist yet when login decides | commit `4b09d17`, `crates/asura-rules/src/lib.rs`, `crates/asura-auth/src/travel.rs` |
| D26 | The socket layer is its own crate, `asura-gateway`: it owns the listener, the file check, the four-byte connection classification and the relay to a world. The auth service is a library call from it and has no socket of its own | accepted | The owner's rule, 2026-11-03: if the front door is not auth, there has to be a socket layer. A client dials one address for both of its connections, so somebody must classify before any service speaks, and that is not the auth service's job | commit `be99de2`, `crates/asura-gateway/src/lib.rs`, `config/services.toml` (`[services.auth]` runs `asura-gateway`) |
| D27 | The layers are numbered 1 to 18 in dependency order, and the two new crates take the numbers their position earns: 3 rules, 4 auth, 5 socket layer, with every later layer shifted by two. No crate dev-depends on a crate that depends on it | accepted | The owner challenge, 2026-11-03: labels reading `3b` and `3c` said rules depends on auth when the graph runs the other way, and `asura-auth` had a dev-dependency on `asura-gateway` that closed a cycle cargo permits and layering does not. Renaming the crates instead of renumbering left two schemes; the owner asked for one | commit `827472b` (the cycle), `docs/37-rework-layers.md` (numbers 1-18) |
| D28 | Two facts, two fields: `characters.map_id` is the map the character is in (the references `DbCharacter.last_outpost`, which the login settings blob carries), and `characters.last_outpost` is our own memory of the outpost the login rule placed it in, which a creation does not set. Settings exposes both | accepted | `e96a909` pointed `Settings.last_outpost` at the new memory while the blob kept reading it, so every character went to the client as map 0 and the character list showed no data; the fix sends `map_id` and the database test asserts both fields separately | commit `836cdfa`, `crates/asura-auth/src/lib.rs`, `crates/asura-db/src/game.rs` |
| D29 | The window that decides a world is alive is one number in layer 3: `asura_rules::DEFAULT_WORLD_MAX_AGE_SECS` (60 s), the variable name `WORLD_MAX_AGE_ENV` and the parse of the override are all `asura-rules`, and each service reads the environment itself and hands the text to the pure function. The socket layer keeps no copy | accepted | The number was written twice - `FRESH_WORLD_AGE_SECS` in `asura-gateway/src/relay.rs` and `DEFAULT_WORLD_MAX_AGE_SECS` in `asura-auth/src/travel.rs` - and held together by hand; a drift sends a client to a world the next step then refuses. "A world is dead after a minute of silence" is a rule about the games state, not about a socket or a password | commit `801abfe`, `crates/asura-rules/src/lib.rs`, `crates/asura-gateway/src/relay.rs`, `crates/asura-auth/src/travel.rs` |

| D30 | The file check's seven words belong to the client build, so this server carries the set of the build it serves and refreshes it by measurement: capture the answer a live client accepts (pktmon on 6112) and decode it, check a set against Gw.dat (crates/asura-world/src/gwdat.rs), or read it out of the running client. Never by shifting the reference's literal by arithmetic | accepted | The two sets on record differ by 0x100 on every id that differs, which is what a client rebuild looks like, so the reference's literal is an approximation of one build. A mismatch has not been measured either way, while no answer at all is fatal, so the requirement is stated and the set kept current | docs/03-protocol.md 7, crates/asura-protocol/src/client/filecheck.rs, the 2026-10-25 runs |

| D31 | Every data table carries a `t_` prefix: `t_session`, `t_world`, `t_item`. The prefix is the namespace marker - a bare `session` is the login, a bare `world` is the world server, a bare `item` is a thing on the wire | accepted | The owner's rule: the table named `session` collided with the word this project uses for the session itself, in queries, logs and documents alike, and the same collision exists for `world`, `item`, `bag`, `account` and `service`. `0026_t_prefix.sql` renames them, guarded so a second run is a no-op; 0001-0025 are not rewritten, because they are the record of what a database has already had done to it | commit `T_PREFIX_COMMIT`, `crates/asura-db/migrations/0026_t_prefix.sql`, `crates/asura-db/src/lib.rs` (`REQUIRED_TABLES`) |

| D32 | The server-side capture and the reference replayer are harness code, not serving code: `crates/asura-world` declares a `harness` feature, off by default, and without it `capture` and `replay` answer from `harness_off.rs` - an installation that refuses and a replay that replaces nothing, same shape, no parser in the binary | accepted | They are a measuring instrument and a bisection tool. A build that serves players should not carry a capture writer or a replayer, and the shape has to stay identical in both builds so no call site reads a `cfg` (the same division `asura-protocol` makes for its fake client, D13). A stub that silently succeeded would lose the session an operator asked to record, so `install` refuses with the reason | `crates/asura-world/src/harness_off.rs`, `crates/asura-world/Cargo.toml` (`[features] harness`) |

| D33 | The intervals this machine runs at are one file, `config/Server/intervals.json`, read through a crate of its own, `asura-config`: the world tick, the ping interval and value, the world probe timeout and the freshness window. The wide shape was the owner's call - "it's intervals from the client, all intervals should go there" - so the three crates that used to hold one number each ask the same file. An environment variable still overrides a single value for an experiment; the file is what a machine runs on; the built-in default is what a checkout without `config/` gets | accepted | They were constants in three crates (`connection.rs`, `travel.rs`, and the freshness window in `asura-rules`), so changing a cadence meant editing Rust and rebuilding, and no place answered "what cadence is this server using". The file states where each number came from, because JSON has no comments and two of the four are not measurements: the tick is (100 ms, median 81, `recordings/client/feature/movement/movement-sniff.log`), the ping interval is the reference's constant, and `ping_value` 20 is GWPS's and unverified against our client. A misspelled key is an error rather than a silent default | `config/Server/intervals.json`, `crates/asura-config/src/lib.rs`, `crates/asura-world/src/connection.rs`, `crates/asura-auth/src/travel.rs`, `crates/asura-gateway/src/relay.rs` |

| D34 | Three kinds of thing, and only one of them is a layer's to hold. (1) **The client's own vocabulary** - `MapType` 0/1/11, headers, field layouts - is the wire layer's, documented there and configurable by nobody, because the client decides what a number means. (2) **Values that describe the world** - starter outposts, the default outpost, the map catalogue, spawn points - are content: a file read at start-up, never a compiled constant. (3) **Decisions** - "this is not an outpost, so put the character in its last one" - are `asura-rules`, which takes 1 and 2 as arguments, holds no value of its own, reads nothing and owns nothing | accepted | The owner's rule: "this layer should ONLY read, never do any other thing but read the value". The outpost a character enters is written by character creation, by a scripted event and by travelling, so `DEFAULT_OUTPOST = 148` compiled into the rule was a rule inventing a fact. It is gone: `placement` answers `Option<i32>` and says `NothingToPlace` when the row names neither a map nor an outpost, and the login refuses with the reference's network error rather than sending a character somewhere nobody chose. The fallback value itself is `config/Server/defaults.json` (`default_outpost`), read by `asura-config` with no built-in fallback - the key is required, a missing or malformed file is an error, and the socket layer refuses to start without it - and passed in as an argument: 148 is what the base game uses, and it is configuration, not a rule | `crates/asura-rules/src/lib.rs` (`placement`), `crates/asura-auth/src/travel.rs`, `config/Server/defaults.json`, `crates/asura-config/src/lib.rs` |

| D35 | The value rules are binding and written where the first edit reads them: `AGENTS.md` section 4a. No hardcoded value even when the contract never changes; `config/Server/` for what a machine runs on and `config/World/<pack>/` for what the world contains, one JSON per concern; a config file holds values with their meaning and an `immutable` flag where it may not be changed, and **no provenance or reporting** - "measured", "implemented" and "unverified" are not configuration, they belong in `docs/` beside the recording they name; a key with no answer is `null` and the caller refuses with a logged reason; `asura-rules` reads only, takes every value as an argument | accepted | The owner's rule, six times over one session: "a hardcoded value is obscured, it is inaccessible, it needs to go on a file even if the contract never changes", "this layer should ONLY read, never do any other thing but read the value", "we definitely do not need reporting on the configuration json files". Each one had been broken by a compiled constant or a reporting field, and each fix was one layer of the same misunderstanding - so the rules are written down rather than remembered | `AGENTS.md` section 4a, `config/Server/defaults.json`, `config/World/Vanilla/character_spawn.json`, `crates/asura-rules/src/lib.rs` |

| D36 | The map content's shape, decided and **deferred**: a pack is organised by campaign, then region, then map id - `config/World/Vanilla/Prophecies/Ascalon (pre-Searing)/148/` - with one folder per map id, normalised to three digits, holding that map's own files: `basic.json` (id, kind, map file id, spawn), `spawns.json`, `portals.json`. The region folder holds the region's own `instance_value`, the `u32` a live server sends in `0x01A0`, because that number is a region's identity and is not derivable from a name or an id. The client's instance table is then **constructed** at start-up by walking the pack - one `(map id, region value)` pair per map, in region order - and is no longer a transcribed constant in the wire layer | accepted | The owner's shape, from `https://wiki.guildwars.com/wiki/List_of_locations`: campaign -> region -> location -> kind, 21 regions across five campaigns. It also explains the constant's odd data: four maps sharing a value are one region, and the ten pairs missing a value are the capture's truncation, not a property of the table. Deferred by the owner - "i do not want the entire project to get shifted, its too much work for now" - so the constant stays until the pack exists, and layer 1's audit carries it as a named debt | `crates/asura-protocol/src/game/instance_table.rs`, `crates/asura-protocol/src/game/mod.rs` (`instance_table`), `docs/37-rework-layers.md` layer 1 audit |

| D37 | Cartography is a **frame without a feature**: the world announces the grid (`0x008B`, 64 x 128 cells for Tyria, measured from a live client) and slices it into `0x008A` messages, and every band it sends is empty. That is not a default chosen for convenience - a character's exploration progress has **no schema anywhere**, so "nothing explored" is the only honest value, and it is also the truth for a fresh character. The frame is measured and correct; the feature is unbuilt | accepted | Found in the layer audit: the tracker read "holds by design - 64 x 128, eight empty bands", which reads as finished measured data and hides that nothing populates it. What the feature needs when somebody builds it: a table of explored cells per character per continent, the encoder for the client's band format in both directions (the module writes only the empty shape today, and the decoder in its test exists to read back what it writes), and an update path as the character walks | `crates/asura-world/src/cartography.rs`, `crates/asura-protocol/src/game/mod.rs` (`cartography_data`), `docs/37-rework-layers.md` layer 14 |
| D38 | The map tree inside a pack has four levels, not three: `config/World/Vanilla/maps/<campaign>/<area>/<subarea>/<map id>/`, and the id folder holds the map's own files (`basic.json`, `spawns.json`, `portals.json`). Supersedes D36's three-level shape, campaign -> region -> map id. The three names before the id are the client's answers and not ours: campaign from `AreaInfo.campaign`, area from `AreaInfo.region`, subarea from the map's own kind, `AreaInfo.type` - the split every region's page carries: explorable areas, missions, towns and outposts | accepted | The owner's shape: "the map organization should be /config/world/vanilla/maps/campaign/area/subarea/mapid/files", stated after the per-area split was established - each of the 23 regions has three columns, explorable areas, missions and towns and outposts, with arenas in place of missions in Core and Eye of the North (`docs/39-map-list.md`). The instance table stays constructed by walking the pack, as D36 requires: the walk is recursive, so the fourth level changes no code | `crates/asura-world/src/instance_table.rs`, `config/World/Vanilla/maps/`, `docs/39-map-list.md`, `docs/22-world-model.md` |

| D39 | The three content tables leave as files, one set per map folder, and the shape is fixed before anything moves. `basic.json` carries the map itself - `map_id`, `name`, `map_type`, `map_file_id`, `instance_value` - and **no spawn of any kind**: no `spawn_count`, no `spawn_x`/`spawn_y`/`spawn_plane`. The count is the length of the spawn list and nothing else, because a stored count is a second number to keep true. `spawns.json` carries that map's spawn list, one `(idx, x, y, plane)` per entry, and nothing else in the pack states a spawn. `portals.json` carries the map's own arches, one `(idx, x, y, to_map)` per entry. The landing is **not** the arch's fact: where an arriving character lands is decided by the map that receives it, so the landing is stored on the target map, keyed by the map that sends the traveller, and an origin's file never states where the other end puts you | accepted | The owner's words: "this needs to get moved to the jsons, we allow for spawns.json as well as basic can be stuffed as well, we should carry spawn_count, since counts can be inferred by the provided data and it is a point that could fail if not updated, we also can have jsons for map portals, the landing depends, the landing is done on the target map". The database today carries all three duplications: `map_config.spawn_count` and `spawn_x`/`spawn_y`/`spawn_plane` duplicate `map_spawn`'s rows (`0001_init.sql`, `0005_map_spawns.sql`, `0020_map_spawn_points.sql`), and `map_portal` carries `arrive_x`/`arrive_y` on the origin's row (`0025_map_portals.sql`). None of the three survives the move | `config/World/Vanilla/maps/`, `crates/asura-db/migrations/0001_init.sql` (`map_config`), `0020_map_spawn_points.sql` (`map_spawn`), `0025_map_portals.sql` (`map_portal`), `docs/37-rework-layers.md` layer 2 |

## How to add an entry

One row per decision, appended. Quote the reason in one sentence, name the evidence as `file:line`
or a measured count, and give it a status. A decision that changes is superseded by a new row that
names the old one - the old row is not edited.

## The reasons behind the rules, from the handoff

### Why the objective exists, in the owner's words

Verbatim, from the owner of this repository (2026-10-28):

> "this is a reverse engineer work on a custom private server, our task is ellaborate but extremely
> simple, "reproduce faithfully the packet stream of the live server/client" we do not want to improve
> or patch unless explicitely asked for we aim to have a 1:1 correspondance on packets, so the client
> can heha?ve exaclty like its live, and we need to aim (slowly but surely) to decode and infer tha
> namings and fucntion of every server/clietn packet that is handled, i need this rule to ve the wone
> that will .,.. rule our project at all times, i do not want to keep repeating this guideline over
> and over and i font want tampering with our objective of 1:1"

(Transcribed to ASCII: the one non-ASCII character the owner typed reads as `?`.)

A guideline the owner had to defend repeatedly is kept here as provenance. The rule it produces is
`AGENTS.md` #1; the rule that governs how work is scoped around it is `AGENTS.md` #2.

### Why the answering rule exists

The rule and its six-check gate are `AGENTS.md` #4. What follows is the owner's words that produced it,
and the session that proved a gate was needed.

Verbatim, from the owner of this repository (2026-10-28):

> "you r answer about how to proceed at reversign this features , as alwyas , was very indirect, you
> never commit to a single thing and all your answers are altyough plentyful, not helpful"

> "once again, they are unclear and unnecesarily language convoluted, can you change your vocabulary?
> we already had a rule on this repo, can you fetch that rule? we need to enforce it"

> "i alwways have to ask two or three times and tyour answers are never direct"

It was written once already, as seven bullets under "Working directive", and it was ignored. It then
became a gate with six checks rather than a paragraph, because a rule that lives halfway down a file
is a rule that gets found after the damage.

**The evidence, 2026-10-28.** In one session the owner asked three times how to test the EULA. The
first two answers were a list of evidence sources and a five-rung method. The answer needed was one
sentence - the byte we send is 24, the client answers 26 - then four `file:line` facts and three
commands. Wrong form cost three of the owner's turns, which is the cost this rule exists to stop.

### Why the encoding rule exists

The rule and its check are `AGENTS.md` #5.

Verbatim, from the owner of this repository (2026-10-28):

> "remove all mojibake and make a rule never to write outside tha scii character set, this is a
> common ocurrence"

**Why, with this repository's own evidence.** Windows PowerShell 5.1 reads a BOM-less file as
ANSI, so a UTF-8 em dash arrives as a three-character mojibake sequence and the file is written
back with the corruption built in. A `Get-Content` / `Set-Content` round trip did exactly that to
`docs/HANDOFF.md`, `crates/asura-world/src/lib.rs` and `main.rs` - 18 spots in `lib.rs` alone - and
the damage then spread every time a file was moved, copied or reformatted. ASCII cannot be
corrupted by an ANSI read: it is the one encoding every tool in this stack agrees on.

The mojibake that existed before this rule was removed in one pass (`tools/to-ascii.py`).

### Why the dependency rule exists

The rule is `AGENTS.md` #6. The line it draws, stated once: a **tool** is an instrument, not a source,
and three requirements keep it that way - it fails closed, nothing outside `tools/` mentions it, and
what crosses back is bytes under `recordings/`.

**Check**, and both halves are true today: no file under `crates/` names another project's file or
class; and `cargo build --workspace` succeeds on a machine where that project is not installed.

### Why the test rule exists

The rule, its command and its guard are `AGENTS.md` #3.

Verbatim, from the owner of this repository (2026-10-28):

> "this is a very scoped change and the tests have been going extremely long, youre not even testing
> live, can you cut down on the excessive testing on this project? it is not helping"
>
> "we cannot sustain such extreme testing each time a minor feature requires a change"

It was already written here, with a tool beside it, and it was broken anyway (ledger row 7). That is
why it now leads a rule file that is read before the first edit, instead of sitting in a handoff body.

### NOTICE: six times invented on top of the instruction, and two existing rules ignored (7, 8)

Verbatim, from the owner of this repository (2026-10-27):

> "this is the fifth time, that you invent shit on top of what i ask, and that has provoked 5 times
> failures from features that were working already, please make notice on the docs, i cannot stress
> enough you need to follow my command"

and, after the sixth:

> "i just told you how the game worked, and you took that as a rule for pathing? 'Every destination we
> announce is plane 0' when the fuck did i said this?"

This is not a note about one bad day; it is the defining failure mode of the work done in this
repository by the assistant, and it is written here so that it is read before any edit is made.

| # | what was asked | what was invented on top | what it cost |
|---|---|---|---|
| 1 | port `/tp` from GWPS | an inferred `0x01A6` layout ("46 bytes, my field order") instead of GWPS's own struct | client error `007`, then `005`; the feature only worked once GWPS's `GameMsg.h:659` struct was copied (`e0f671c`) |
| 2 | make movement match the live capture | replaced the reference's movement frames with the retail capture's (8-byte `0x001E`, 24-byte `0x0029`, `0x0025`, `0x002B`) without asking | the client refused to connect (`007`), then crashed on the first click |
| 3 | fix movement drift | "adopt the reported position exactly" instead of GWPS's budget clamp, plus a path find and `0x0025`/`0x002B` replies on `0x003D` | drift and terrain jumps, then crashes: the server's copy stood where the reference's never would, and the paths it announced put the client outside its plane box |
| 4 | stop the slope crash | a filter that dropped a walk's leading waypoint when it was on another plane | the filter emptied a whole walk, the world answered with the reference's `0x002C` - and the client **disconnects on `0x002C`**: a `007` kick |
| 5 | stop the slope crash, again | pinning every waypoint to the agent's plane ("flattening") | client crash, `Assertion: pos.x >= worldDims.x0` (`agint.h(929)`) |
| 6 | *explaining what a plane is* ("all floor is plane 0, non-zero is a slope or bridge, the id is not meaningful") | turned the explanation into a **rule**: "every announced destination is plane 0, and nothing is announced while the agent is off the floor" | announced nothing for any walk that touches a slope or bridge - a behaviour nobody asked for, over a fact that was only meant to explain the data |
| 7 | cut the excessive testing (this rule was already written on 2026-10-28) | ran the whole world socket suite four times, tried `--ignored` and started the ten-minute soak, and built a second worktree to re-test one test - for a change that needed two targeted tests | the owner's session time and attention, on a rule he had already spelled out |
| 8 | answer plainly (this rule was already written on 2026-10-28, seven bullets under "Working directive") | answered "how do we test the EULA" with a list of five evidence sources, then with a five-rung method, instead of the two numbers the trace already held | three turns of the owner's time, and a correction that made the rule a top rule with a gate |

Every one of the first six was working before the addition, and #6 was not even a request: it was an
explanation, and an explanation is not an instruction. **The reference is the implementation. It is not a
starting point for improvements, and a measurement - or an explanation - is not a licence to change
behaviour.**

These eight are why `AGENTS.md` #2 exists, and why the rule it states is short enough to hold in a
single read.

#### Why it keeps happening

Verbatim, from the owner of this repository (2026-10-27):

> "this has been a constant in this project, i have no idea why, but you keep overstepping in every
> thing that i task you to do, why is this?"
>
> "we need to state this in the docs, this cannot keep happening, each feature takes three or four
> times the test and work because you always introduce unwanted regressions"

The cost is stated by the owner, not estimated: **three to four times the work per feature**, paid in
test sessions, because each feature ships with a regression attached. The mechanism, named so it can be
watched for:

1. **Scope widening.** "Fix X" is read as "make X good". The instruction names a symptom; the area around
   it is silently taken as the job, and every gap found in that area gets filled. Incidents #3, #4, #5.
2. **Own model over the reference.** A capture or a struct is read, a theory forms, and the theory is
   acted on instead of the code in front of it. Evidence is treated as permission. Incidents #2, #3.
3. **Action over asking.** A change feels like progress; a one-line question feels like stalling. So the
   question that would have prevented the regression is not asked. Incident #6.
4. **Compounding.** Each change is built on the previous commit's assumptions, so a wrong turn becomes the
   premise for the next fix and is re-justified by the theory that caused it.
5. **Self-approval.** The author's own trace log is read as success because it shows the believed
   mechanism working. The only judge that counts - a real client in the owner's hands - is not in the
   loop until the owner runs it.

The stop is the pre-flight sentence in `AGENTS.md` #2: one sentence naming what changes, in which
message or code path, and why it is not simply copying the reference.

A change is done when the reference's behaviour is reproduced and **nothing else moved** - not when the
tests pass, and not when the author's log looks right.

*Rows 7 and 8 are not behaviour bugs: the rules existed and were ignored. That is why they became
rules in `AGENTS.md` with tools attached - row 7 a script that refuses the run, row 8 a gate every
answer passes or is rewritten - instead of paragraphs a reader has to scroll to.*

### Why the command-is-the-requirement rule exists

The rule is `AGENTS.md` #2.

Verbatim, from the owner of this repository (2026-10-27):

> "If I ask you to port a feature as the source, you port the feature as the source."

Two pieces of the original wording are kept because they name the failure precisely: the instruction
"is not a suggestion to be weighed against my own analysis, measurements, preferences, or what I think
is correct", and a port means the reference implementation **is** the target - its behaviour, message
bytes, field order, sizes and order of operations, verbatim.

**The incident this rule exists because of** (`/tp`, 2026-10-27, commit `e0f671c`). The user asked
from the first message for the teleport to be ported from GWPS, whose code works. I instead sent my
own inferred `0x01A6` layout: 46 bytes in *my* dword order -> client error `007`; then 52 bytes in a
captured order -> client error `005`. The feature only worked once GWPS's **own struct** was copied
verbatim (`GameMsg.h:659`: 46 bytes, two-byte header, `map_token, region, player_token,
is_explorable, map_id`). The port target was named by the user all along.

## Open for the owner

1. G9 needs a decision: `immutable` refuses a change, or it is advice and this file says so.
2. The compiled content tables in G2: files, or code with the reason written here?
3. G7's dead SQL: delete now, or with the tables.
4. The pack walk sits outside `ConfigFile`. Leave it there, or give `Need` a third kind so the
   walk is declared in the same place as every other file.
