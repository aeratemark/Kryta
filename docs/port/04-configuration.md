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

`information` is where a file writes down what it is for, how it was measured, what would replace
it and what it links to. `data/file_check.json` is the worked example: an `information` block naming
the capture the bytes came from and the way to replace them, then the two byte strings themselves.
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
squeezed into the other's shape. Nothing in the tree is a row list today.

## Settings that are still not in a file

| setting | where it is now | where it goes |
|---|---|---|
| the world list | `realmlist` in `kryta_login` | `data/realms.json`, a file row overriding a database row of the same id |
| the database layer's defaults - host, port, user, database names | `database/src/Database/mod.rs` | the program's own `.json`, beside the connection string that already overrides them |
| the network thread count, the acceptor's backlog, the buffer sizes | a literal at each `start_network` call, and defaults inside `shared/src/Network/` | the program's own `.json`, as `Network.Threads` and its siblings, when a program needs to choose them |

Every other setting the programs have is in their file and read from it.
