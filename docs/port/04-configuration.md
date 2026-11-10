# Configuration: settings, in JSON, read by one handler

**A configuration file holds settings: the values that differ per operator.** That is the whole rule.
Ask it before writing one file: would two operators running this build legitimately have different
values? Yes - port, address, database connection, log level - then it is a setting and it belongs here.
No - then it does not go in a file, and what it is instead is decided by where it already lives: a
constant in the code that uses it if the client dictates it, a row in the database if a player made it.

Three things this rule has settled the hard way:

- `RealmServerPort`, `BindIP`, `LoginDatabaseInfo` differ per operator, so they are settings -
  `authserver/authserver.json`, `worldserver/worldserver.json`.
- An account differs per *person*, not per operator, so it is a row in `kryta_login`'s `account`
  table. It was in a file once, and that was wrong (`docs/decisions/0006-accounts-are-rows.md`).
- The file check's 21 and 32 bytes are the same for every operator running this client build, so they
  are constants in `authserver/src/Server/file_check.rs`, naming the capture they came from. They were
  a file once too, and that was the same mistake.

The rest of this file: how a file is written, which files there are, and which settings still live in
code. `docs/decisions/0005-json-configuration.md` records why the format is JSON rather than the
reference's `Key = value`.

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

And the two things that are **not** in this list, because they were and should not have been:

- **Payloads.** The file check and the account settings block are constants in the code that answers
  with them, `authserver/src/Server/file_check.rs` and `account_settings.rs`, each naming the capture
  its bytes came from. They became `data/*.json` during the move to JSON, and that was wrong: nothing
  about them is a choice.
- **Accounts.** A login name and the identity pair it carries are state: rows in `kryta_login`'s
  `account` table (`docs/decisions/0006-accounts-are-rows.md`). A file holds what an operator
  configures and shares; a table holds what people do.

The mechanism that let a payload look like configuration is gone with them: `Config` - the type a
program's file is read into - has no method that answers bytes, so a payload read through it is not
possible. Bytes are decoded by `common::utilities::hex_bytes`, the one decoder, wherever they live.

A file that holds **rows** is a record list and is not a set of keys: it is read as rows through the
same reader (`Document::records`), so a single file can carry both a `note` and a list without either
being squeezed into the other's shape. Nothing in the tree is a row list today.

## Settings that still live in code

By the one rule, these differ per operator today but have no file to differ in yet:

| setting | where it is now | where it goes |
|---|---|---|
| the world list | `realmlist` in `kryta_login` | `data/realms.json`, a file row overriding a database row of the same id - a world's address is the operator's choice, so it is a setting, and today only the table can hold it |
| the database layer's defaults - host, port, user, database names | `database/src/Database/mod.rs` | the program's own `.json`, beside the connection string that already overrides them |
| network thread counts, connection backlog, buffer sizes | `shared/src/Network/`, passed as parameters | each program's own `.json` - `Network.Threads` is read already; the rest arrive when a program needs to choose them |

Everything else in the code is not a setting and is not waiting for a file: the client dictates it
(message sizes, status codes, the handshake's arithmetic, the file check and account settings
payloads, the portal's endpoint paths) or it is structure (a field's offset in a layout its builder
writes). Two exceptions stay in code even as settings go into files, and both would break the wire if
separated: a number that is part of an algorithm - RC4's state, the key derivation's rounds and
rotation widths, SHA-1's initial words - and a field's offset.
