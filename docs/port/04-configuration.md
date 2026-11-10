# Configuration: settings in JSON

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
| `tools/manage/manage.json` | the administration tool's own configuration: bind, port, token, the two database strings | `kryta-manage`, `-c` |

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
| the world list: every field of every world, including a world the table does not hold | `data/realms.json`, merged over `realmlist` |
| the intervals this program reports and refreshes at | `authserver.json` |
| the creation screen's content: the bags and items a client is given, and what a new character wears per campaign and profession | `worldserver.json`, `InventoryFile` and `EquipmentFile`, naming `data/inventory.json` and `data/equipment.json` |
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
| **game data** | the world's state: what exists, what a character has, where it stands | **database rows**, through the noun's repository | accounts, characters and their settings, item definitions (`item_template`), a character's bags and items, map records, spawns, portals, travel |
| **a setting** | a choice an operator makes and may share | **a JSON file** | a program's configuration, the world list, the profession sets a new character starts wearing, loot tables |
| **what the client dictates** | a value the client decides and the server can only obey | **a constant in the code that answers with it**, citing the capture | message headers and sizes, the file check's bytes, the account settings block |

Two consequences:

- **A port never decides this per piece.** A feature arriving from another tree is placed by asking which
  of the three it is - not by where the other tree happened to keep it. Items are the worked example: a
  player's inventory is state (rows), an item's definition is world data (rows), a loot table is a setting
  (a file), and none of them is code.
- **A row is never a file, and a setting is never a row.** An account is not a setting; a loot table is not
  a row. When the two are confused, an operator shares a file that should have been one person's data, or
  edits a row that should have been a setting.

Nothing else in the tree is a setting: what the client dictates is a constant in the code that answers
with it, and what a player does is a row.

