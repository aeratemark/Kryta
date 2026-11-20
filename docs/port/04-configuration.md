# Configuration: settings in JSON

Where a value lives is decided by one question: is it a number the program runs by, a value that
describes the world, or a decision? A number the program runs by is a setting in its own program's
file. A value that describes the world or a service is a file under `config/`, and the setting only
says which file. A decision is code. `config/README.md` is the tree and what belongs in each folder;
this page is the rule and the inventory.

```text
worldserver/worldserver.json     the game program's settings - the one file it reads
authserver/authserver.json       the login program's settings
config/Server/                   what a machine chooses and answers (commands.json today)
config/World/<pack>/             what a world contains; Vanilla is the faithful base
data/sql/, data/path-graphs.bin  the archive's and the database's: schema, seeded rows, extracted graphs
```

`worldserver` reads `Console.Enable` (default `true`): when enabled and standard input is an
interactive terminal, it starts the embedded console. Supervised non-interactive input is not consumed.
`Console.QueueCapacity` (default `64`) limits pending console requests and must be greater than zero.

`worldserver` reads `Control.Enable` (default `false`), `Control.Port` (default `7878`, always bound to
`127.0.0.1`), `Control.CredentialFile` (default empty) and `Control.TimeoutSeconds` (default `5`, must be
positive): the local control endpoint a separate manager or CLI asks for world operations. It refuses to
start when enabled without a credential file or with a non-positive timeout. `tools/manage` reaches it
through its own `ControlPort`, `ControlCredentialFile` and `ControlTimeoutSeconds`; the credential file is
created on this machine with `cargo run -p worldserver --bin kryta-cli -- init-credential <file>`, which
also restricts it to the service user. Neither program logs the credential.

`tools/manage` reads `SupervisorStackFile` (default `tools/supervisor/stack.json`) and
`SupervisorTimeoutSeconds` (default `20`). The former is the supervisor's existing configuration file:
the manager reads only its `control_port`, then asks the running supervisor for `status`, `stop <service>`
or `restart <service>`. It does not copy service rows, start a missing supervisor, or own a process.

`worldserver` reads `CaptureFile` (default empty, which is off): where to record the wire, every game
message both directions as plaintext JSONL with two-byte headers (`game/Server/Capture.rs`). It is a
setting rather than a command-line flag because a diagnostic run should be able to name its file in the
same place as everything else it runs with; the file is evidence and unsanitised, so it belongs under
`recordings/` or a scratch path, and its reader takes the layer from the file's own header
(`tools/capture/read.py`).

Settings go in a JSON file: one per program, read by one handler. This file says how one is written and
which files there are. `docs/decisions/0005-json-configuration.md` records why the format is JSON rather
than the reference's `Key = value`.

## Writing a file

A configuration file is a JSON object. A nested object is a **section**, and its keys are the
reference's dotted names; a leaf is a scalar, an array of scalars, or an object holding a `value` or
a `hex`.

```json
{
  "note": "what this whole file is for",

  "information": {
    "what": "this client asks which game files it should have before anything else",
    "measured": "a live client on 2026-10-26, build 39112",
    "how to replace the bytes": "capture the reply a client accepts, never compute one"
  },

  "RealmServerPort": 6112,

  "StatusInterval": {
    "value": 5,
    "doc": "how often the program writes its own status line, in seconds"
  },

  "Network": { "Trace": true, "Threads": 1 },

  "Appender": { "Console": [1, 5, 0, "1 9 3 6 5 8"] },

  "LoginDatabaseInfo": "127.0.0.1;5432;kryta;change-me;kryta_login"
}
```

That file's keys are `RealmServerPort`, `StatusInterval`, `Network.Trace`, `Network.Threads`,
`Appender.Console` and `LoginDatabaseInfo`.

| written as | the key's text |
|---|---|
| `"BindIP": "0.0.0.0"` | `0.0.0.0` |
| `"RealmServerPort": 6112` | `6112` |
| `"Trace": true` | `1`, and `false` is `0` - the reference reads a flag as text and compares it with `1` |
| `[1, 5, 0, "1 9 3 6 5 8"]` | `1,5,0,1 9 3 6 5 8` |
| `{"hex": "f1 02 20 00"}` | the bytes, decoded by the one decoder there is |
| `null` | the key is absent - how a file says "not set" |
| `{"value": 0, "immutable": true, "doc": "..."}` | `0`, described |

Four names are read past rather than configured, at every level of a file - the root, a section, a
leaf, a row - so no configuration key here is called one of them:

| name | is |
|---|---|
| `doc` | where a value came from, in one line |
| `note` | what a whole file is for, in one line |
| `immutable` | a number the *client* decides rather than the server: editing it does not change the client. One without a `doc` is reported, because a label with no reason is a rumour |
| `information` | commentary of any shape - an object, an array, nested trees, prose - for the explanation that will not fit in one line. Its contents are never keys and are never read |

`information` is where a file writes down what it is for, how it was measured, what would replace it
and what it links to. `data/realms.json` is the worked example: a note, an `information` block saying
how the file is read and what is deliberately not in it, then the rows themselves.

Two more rules, both enforced with a message rather than silence:

- A key stated twice - which needs two spellings of one path - is reported and the first wins.
- A value that cannot be a key (`{"value": {"nested": 1}}`, an array of objects) is reported and
  skipped. A file that states keys and has every one refused is refused as a whole, with the
  reasons, rather than being called empty.

Precedence is unchanged from the reference: an environment variable `AC_<KEY IN UPPER SNAKE>` wins
over the file, and the file wins over the default the program passes. The default in the code is the
reference's `GetOption` signature and is never the effective value when a shipped file states the
key.

## The files

Configuration, and nothing else:

| file | is | read by |
|---|---|---|
| `authserver/authserver.json` | the auth program's own configuration | `authserver`, `-c` |
| `worldserver/worldserver.json` | the game program's own configuration | `worldserver`, `-c` |
| `data/realms.json` | the worlds a login may send a player to, merged over the `realmlist` table | `authserver`, `WorldsFile` |
| `tools/manage/manage.json` | the Configuration Manager's own configuration: bind, port, token, database strings, world-control credential location, and supervisor stack-file location | `kryta-manage`, `-c` |

The `.dist` files are committed; the live ones are not, because they hold a database password. Copy
one, edit it, keep it.

**A setting is in a file only once the program reads it.** The file states what the ported stages use
and nothing else, so no key sits there configuring nothing: the listener's port and thread count, the
thread pool, the console, SOAP and the world's own data directory arrive with the stages that read
them. Both files are checked against the code that reads them - a run with every key missing from them
warns, so a pruned file that is too small fails loudly rather than silently.

The file check's bytes, the account settings block and the accounts are not settings and are not in this
list: the first two are constants in `authserver/src/Server/file_check.rs` and `account_settings.rs`, and
the accounts are rows in `kryta_login`'s `account` table. `Config` - the type a program's file is read
into - has no method that answers bytes, so a payload cannot be read through the configuration reader at
all; bytes are decoded by `common::utilities::hex_bytes`.

A file that holds **rows** is a record list and is not a set of keys: it is read as rows through the same
reader (`Document::records`), so a single file can carry both a `note` and a list without either being
squeezed into the other's shape. `data/realms.json` is the one today: a world is a setting, and it is
also a row in the `realmlist` table, so the file is merged over the table - same id overrides, new id is
added, and a field the file does not state keeps the table's value.

## Every setting is in a file

| setting | is set in |
|---|---|
| the login listener: port, bind address, thread count, trace | `authserver.json` |
| the portal: enabled, where it answers | `authserver.json` |
| the login database: connection string, worker and synchronous connections, trace | `authserver.json` |
| the character database: connection string, worker and synchronous connections | `authserver.json`, `worldserver.json`, under `CharacterDatabaseInfo` and `CharacterDatabase` |
| the login database, in the world program: connection string, worker and synchronous connections | `worldserver.json`, under `LoginDatabaseInfo` and `LoginDatabase` - the world reads it for the travel record its own program cannot be told by another program |
| the world list: every field of every world, including a world the table does not hold | `data/realms.json`, merged over `realmlist` |
| the intervals this program reports and refreshes at | `authserver.json` |
| where a travel request is placed: the map id that is the creation instance (`CreationMap`, 0), and the outpost a character with no map anywhere enters (`FallbackOutpost`, 148) | `authserver.json`, read by `game/src/Maps/Placement.rs` through the login session |
| the world loop's floor: how long may pass before the loop does not bother to update (`MinWorldUpdateTime`, 1 ms - the reference's own key of the same name in `worldserver.conf`, its `WorldUpdateLoop` sleeping the remainder at `Main.cpp:586-616`) | `worldserver.json`, top level |
| the world's numbers, every one a game rule with a default (`docs/game-rules.md`): a district's allotment (R5), a party's cap (R15), the spawn scatter (R13), whether instances tick (R33), the simulation tick's interval (R33, 100 ms by default, ASURA's own `intervals.json`), how often the instance table is swept (R33/R40, 1 s by default), the compass range (R34), the disconnect hold (R37), the mission button (R56), the number a ping reply carries (20, GWPS's own and the value the reference's log carried) | `worldserver.json`, the `World` section |
| the agent a load creates: its health (`AgentHealth`, 400), energy (`AgentEnergy`, 25), energy regeneration (`AgentEnergyRegen`, 0.0) and walking speed (`AgentSpeed`, 288) - and the appearance a character with no bits of its own is drawn with (`Appearance`, `0x03100000`: profession 1, hair style 3) | `worldserver.json`, the `World` section |
| how far a client may place itself when it cancels a walk and still be believed (`MaxAllowedCorrection`, 250 units - the reference's `MAXIMUM_ALLOWED_CORRECTION`, through ASURA's `config/Server/movement.json`) | `worldserver.json`, the `World` section |
| what a redirect's `is_explorable` carries (`TravelExplorable`: `0` for every travel by default, `1` for every travel, or `auto` for the destination's own kind); ASURA reads the same three words from `ASURA_TRAVEL_EXPLORABLE` and defaults to `0` because that is the value an explorable's load completes at (`ASURA/crates/asura-world/src/travel.rs:21-69`) | `worldserver.json`, the `World` section |
| the outpost a fresh character starts in and has unlocked (`StartingOutpost`, 148 - the one unlock of R12, and the same fact the login's `FallbackOutpost` names) | `worldserver.json`, the `World` section |
| half the side of a point doorway's box (`PointBoxHalfExtent`, 250 world units): a doorway stated as a wall never uses it, and `0` disables point doorways (`ASURA/config/Server/portals.json`, `point_box_half_extent`) | `worldserver.json`, the `World` section |
| the world itself: every map record, the entry points where a character may stand, and the arches a step crosses - the tree is classified here as one unit, so a file inside it needs no row of its own | `worldserver.json`, `WorldPack`, naming `config/World/Vanilla`, whose `config/World/Vanilla/maps/` folder is the tree (`<campaign>/<area>/<kind>/<map id>/{basic,spawns,portals}.json`). A folder that is not its record's `map_id`, or a pack with no `maps/`, stops the program - serving half a world is worse than not serving (`docs/decisions/0014-the-world-content-is-a-pack.md`) |
| the creation screen's content: the bags and items a client is given, and what a new character wears per campaign and profession | `worldserver.json`, `InventoryFile` and `EquipmentFile`, naming `config/World/Vanilla/creation/inventory.json` and `config/World/Vanilla/creation/equipment.json` |
| which slot of the client's equipped bag each kind of item is worn in | `worldserver.json`, `EquippedSlotsFile`, naming `config/World/Vanilla/creation/worn_slots.json`. The index is the client's own (its nine-slot equipped array, the placement inside the equipped bag, the agent's worn list), so it is content and not a table compiled in - a wrong one draws the character without the piece (`docs/game-rules.md` E234) |
| the chat command surface: the names a player may type, what each takes, and which operation it runs | `worldserver.json`, `CommandsFile`, naming `config/Server/commands.json`. The surface is **content**, so the setting is only where the file is; the rows themselves are data (`docs/decisions/0011-the-command-surface-is-content.md`) |
| the extracted graphs a serving world answers movement from | `worldserver.json`, `PathGraphs`, defaulting to `data/path-graphs.bin`. It is a portable `ASURAPG1` cache; it may omit maps, but the serving process never reads `Gw.dat` (`docs/decisions/0014-headless-navigation-assets.md`) |
| the cache the manager inspects and the archive it uses for an explicit map export | `manage.json`, `PathGraphsFile` and manager-only `MapArchiveFile`. The latter is empty unless an operator intends an offline dump; neither key is read by `worldserver` |
| loot tables: what a source drops and how often - a setting, not a row | a JSON file under `data/loot/`, arriving with the drop stage |
| logging: directory, appenders, logger levels | `authserver.json`, `worldserver.json` |
| the PID file, the effective-config dump | `authserver.json`, `worldserver.json` |
| everything the world program will need - its port, its thread pool, its map threads, the console, SOAP, its data directory | `worldserver.json`, arriving with the stage that reads them |

Two consequences of taking that seriously, both now in the code:

- **The connection string is critical.** A program cannot fall back to a compiled-in host and open a
  database nobody named, so a file without `LoginDatabaseInfo` or `CharacterDatabaseInfo` is a fatal
  configuration error rather than a silent default (`main.rs`, `mark_critical`). The reference's parser
  still tolerates a short string and fills the rest; no shipped file is short.
- **A setting arrives with the stage that reads it.** `Network.Threads` is read because the listeners
  exist; `WorldServerPort` is not in `worldserver.json` yet because nothing binds it. A key in a file
  is therefore a key the program uses, never a promise.

## Three kinds of data, and the test for each

Every piece of data in this tree is one of three things. The test is not the source project's file
layout and not a judgement per feature: it is what the thing **is**.

| kind | is | lives in | examples |
|---|---|---|---|
| **game data** | the world's state: what exists, what a character has, where it stands | **database rows**, through the noun's repository | accounts, characters and their settings, item definitions (`item_template`), a character's bags and items, map records, spawns, instances, portals, travel |
| **a setting** | a choice an operator makes and may share | **a JSON file** | a program's configuration, the world list, the profession sets a new character starts wearing, loot tables |
| **what the client dictates** | a value the client decides and the server can only obey | **a constant in the code that answers with it**, citing the capture | message headers and sizes, the file check's bytes, the account settings block |

**ASURA's rule is stricter than a table, and this tree takes it as binding** (`AGENTS.md` section 4a
there, decisions D34, D35 and D39 in `ASURA/docs/reference/decisions/layer-decisions.md`):

- **No hardcoded value, even when the contract never changes.** A compiled number is invisible and
  unreachable; it goes in a file so it can be read and reviewed. A reader has **no built-in
  fallback**, and **a missing or malformed file is an error that stops the program** - not a warning
  that leaves a world half configured.
- **A file holds values with their meaning, and no provenance or reporting.** "measured",
  "implemented" and "unverified" are not configuration; they belong in `docs/` beside the recording
  they name. Where a value is one of the client's own numbers and must not be changed, the file says
  `immutable` - a note to the editor, not a runtime check.
- **A key with no answer is `null`**, and the caller refuses the action with a logged reason rather
  than choosing on the player's behalf.
- **The rules read only.** A decision takes values as arguments; it holds no value of its own and reads
  no file (that is the reference's arrangement too: a rule is code, a value is a file).
- **A stored count is a second number to keep true.** A list's length is its count; a landing belongs
  to the map that receives the traveller, not to the map that sends it (D39).

Where this tree enforces it, rather than stating it:

| the rule | the thing that fails when it is broken |
|---|---|
| a missing file is an error | `Creation::load` returns an error for a missing inventory, equipment table or worn-slot table; `Pack::load` refuses a pack with no `maps/` |
| no built-in fallback | every reader requires its keys (`number`, `float`, `required`), and a `null` reaches the caller as `None` |
| the doc is the inventory | `every_config_file_is_classified_in_the_configuration_doc` walks `config/` and fails, naming them, on any file this page does not classify |

Two consequences:

- **A port never decides this per piece.** A feature arriving from another tree is placed by asking which
  of the three it is - not by where the other tree happened to keep it. Items are the worked example: a
  player's inventory is state (rows), an item's definition is world data (rows), a loot table is a setting
  (a file), and none of them is code.
- **A row is never a file, and a setting is never a row.** An account is not a setting; a loot table is not
  a row. When the two are confused, an operator shares a file that should have been one person's data, or
  edits a row that should have been a setting.
- **A JSON asset is an authoring input, not a second mutable world store.** It may define shared content
  for validation and import into the noun's rows, or it may be the file a setting names. Its exact reader,
  validation and import path belong to the feature that owns it. Player progress and other authoritative
  mutable state remain rows.
- **Lua is neither a store nor a setting.** When the runtime and bindings land, Lua authors behaviour by
  calling validated Rust operations. It does not own authoritative state, packet construction, or a
  replacement persistence format.

Nothing else in the tree is a setting: what the client dictates is a constant in the code that answers
with it, and what a player does is a row.

