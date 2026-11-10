# Configuration: JSON, one reader, and what is left to move

A **configuration option** is something an operator chooses and shares. Those live in JSON, one file per
program, read by one handler, `common/src/Configuration/Json.rs`. This file says what an option is and
is not, how to write a file, where the files are, and which numbers are still constants and where they
are going. `docs/decisions/0005-json-configuration.md` records why the format is JSON rather than the
reference's `Key = value`.

## What is a configuration option, and what is not

Three kinds of thing a server holds, and only the first is configuration. Confusing them is how a
payload became a file and an account became a file, both of which had to be undone:

| kind | is | example | lives |
|---|---|---|---|
| **configuration option** | what an operator chooses, edits and shares to run *their* server | `RealmServerPort`, `BindIP`, `LoginDatabaseInfo`, `Network.Trace`, `Logger.root`, a world's address | a JSON file: `authserver/authserver.json`, `worldserver/worldserver.json` |
| **protocol payload / client fact** | what the client demands, byte for byte. Nobody chooses it; there is no second valid value for a given client build | the 21-byte file check and its 32-byte answer, the 591-byte account settings block, message sizes, the handshake's arithmetic | **code**, as a constant whose `doc` names the capture it came from, refreshed by rebuilding |
| **state** | what people do | an account, a character, a session, a travel | **a row** in the database |

A file is not a place for a payload because a payload has no choices in it, and a payload is not a
configuration option however it is written - hex text in a JSON file is still a payload. This is what
keeps the configuration small enough for one person to read and share.


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
being squeezed into the other's shape. Nothing in the tree is a row list today - the world list is the
next one, and it will merge with the `realmlist` table rather than replace it, because a world is a
deployment and deployments are configuration.

## What still belongs in a configuration file, and what does not

The earlier version of this section listed the protocol's own numbers - the handshake's arithmetic,
message sizes, status codes, the login answer's literals - as things "moving to JSON, immutable". That
was the same mistake in a slower form: those are client facts, not choices, so they are settled where
they are. What genuinely remains is the opposite direction, options that live in code today:

| what | where it is now | where it goes |
|---|---|---|
| the world list (today only a database table) | `realmlist` in `kryta_login` | `data/realms.json`, a file row overriding a database row of the same id: a world is a deployment, and deployments are configuration |
| the database layer's own defaults - host, port, user, database names | `database/src/Database/mod.rs` | the program's own `.json`, where the connection string already is |
| network thread counts, connection backlog, buffer sizes | `shared/src/Network/`, passed as parameters | each program's own `.json` - `Network.Threads` is already read; the rest arrive as options when a program needs to choose them |

And what is settled as code, so that it is not "pending" again: the handshake's prime, exponent and
sizes (`handshake.rs`); the client's three connection headers (`Authentication/`); the auth
conversation's message sizes and its replies' shapes (`stream.rs`, `login_reply.rs`); the login
answer's literals and status codes; the file check and account settings payloads; the portal's
endpoint paths and its XML answers.

Two kinds of number are in code for a reason worth stating: the ones that are part of an *algorithm*
(RC4's state, the key derivation's round constants and rotation widths, SHA-1's initial words) and the
ones that are *structure* (a field's offset inside a layout the builder writes). Separating those from
the code that uses them would let a file desynchronise the wire while every test still passed.
