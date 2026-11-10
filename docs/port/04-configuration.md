# Configuration: JSON, one reader, and what is left to move

Every configuration and every value in this tree is a JSON file read by one handler,
`common/src/Configuration/Json.rs`. `docs/decisions/0005-json-configuration.md` is the record of why
this deviates from the reference's `Key = value` format; this file is what an operator and a future
change need: how to write a file, where the files are, and which numbers are still constants in the
code and where they are going.

## Writing a file

A configuration file is a JSON object. A nested object is a **section**, and its keys are the
reference's dotted names; a leaf is a scalar, an array of scalars, or an object holding a `value` or
a `hex`.

```json
{
  "note": "what this whole file is for",

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

Three names are read past rather than configured: **`doc`** says where a value came from, **`note`**
says what a file is for, **`immutable`** marks a number the *client* decides rather than the server.
An `immutable` number without a `doc` is reported: a label with no reason is a rumour.

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

| file | is | read by |
|---|---|---|
| `authserver/authserver.json` | the auth program's own configuration | `authserver`, `-c` |
| `worldserver/worldserver.json` | the game program's own configuration | `worldserver`, `-c` |
| `data/file_check.json` | the 21-byte question and the 32-byte answer | `authserver/src/Server/file_check.rs` |
| `data/account_settings.json` | the 591-byte block every login is answered with | `authserver/src/Server/account_settings.rs` |
| `data/accounts.json` | who the portal says an account is - a record list | `game/src/Portal/accounts.rs` |
| `data/sql/base/db_*/*.sql` | the schema and its updates | the database updater |

The `.dist` files are committed; the live ones are not, because they hold a database password. Copy
one, edit it, keep it.

A file that holds **rows** - `accounts.json` today, worlds next - is a record list and is not a set
of keys: it is read as rows through the same reader (`Document::records`), so a single file can carry
both a `note` and a list without either being squeezed into the other's shape.

## What is still a constant, and where it is going

The owner's requirement is that no number lives only in code or only in the database. The first pass
moved the five files above. What remains, and the file each is moving to:

| what | where it is now | where it goes |
|---|---|---|
| the handshake's prime, exponent and sizes | `authserver/src/Server/handshake.rs` | `data/protocol/handshake.json`, `immutable` - the client's arithmetic |
| the client's connection headers and their first-message sizes | `authserver/src/Authentication/mod.rs` | `data/protocol/connections.json`, `immutable` |
| the auth conversation's message sizes | `authserver/src/Server/stream.rs` | `data/protocol/auth_messages.json`, `immutable` |
| the conversation's headers and reply shapes | `authserver/src/Server/session.rs`, `login_reply.rs` | `data/protocol/auth_messages.json`, `immutable` |
| the login answer's literals - features, campaigns, `unk5`, the heartbeat's tick | `authserver/src/Server/login_reply.rs` | `data/protocol/account.json`, `immutable` |
| the answer's status codes | `authserver/src/Server/login_reply.rs` | `data/protocol/account.json`, `immutable` |
| the error codes a refusal carries | `login_reply.rs`, `Portal/` | `data/protocol/account.json` |
| the portal's endpoints and answers | `game/src/Portal/` | `data/portal.json` |
| the world list (today only a database table) | `realmlist` in `kryta_login` | `data/realms.json`, a file row overriding a database row of the same id |
| the database layer's own defaults | `database/src/Database/mod.rs` | the program's own `.json` - already reachable, not yet read from there |
| network thread counts and buffer sizes | `shared/src/Network/` | each program's own `.json` - already reachable (`Network.Threads` is read; the rest are parameters) |

Two kinds of number are **not** moving, and the reason is in the decision record: the ones that are
part of an algorithm (RC4's state, the key derivation's round constants and rotation widths, SHA-1's
initial words) and the ones that are structure (a field's offset inside a layout the builder writes).
Separating those from the code that uses them would let a file desynchronise the wire while every
test still passed.
