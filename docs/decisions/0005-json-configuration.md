# 0005 - Configuration is JSON, read by one handler

Status: accepted. This is an addition the owner required, not a reading of the reference; the owner
may correct it, and a correction replaces this text rather than being appended to it.

## The question

The reference's configuration format is `Key = value` in a `.conf` file, parsed by
`src/common/Configuration/Config.cpp`. This tree ported that parser and every quirk in it, and both
programs read a `authserver.conf` / `worldserver.conf`.

The owner's requirement, as it reads today: **a configuration file holds settings - the values that differ per operator**. It was first taken as **every hardcoded number and every configuration value is handled from JSON
configuration files**, so that users can configure and share their setups; and **no datum may live
only in code or only in the database**. A JSON file is a thing two people can diff, a web page can
show, and another program can read; a `.conf` with `Key = value` is nearly that, and a constant
inside a Rust function is not.

## The decision

1. **The format is JSON, for settings only.** `authserver/authserver.json` and
   `worldserver/worldserver.json` are the programs' own files; the `.dist` files are committed and the
   live ones are not, because they hold credentials. A configuration file holds what an operator
   *chooses and shares*. It holds no payload the client dictates and no row a person lives in - a
   correction to this record's first version, which was written broadly enough that two payloads and
   an account list ended up in files. Both were undone: accounts are rows (0006), and the payloads
   are constants again. `docs/port/04-configuration.md` is the classification.
2. **One reader, in `common/src/Configuration/Json.rs`.** Not one per consumer: a file is parsed
   into `Document`, flattened to the reference's `dotted.key -> text` bag, and every typed getter,
   severity policy, environment override, `describe()` and masking above it is unchanged. A record
   list - a file of rows, if one ever earns its place - is read as rows through the same `Document`,
   never by a hand-written parse in the module that wants it.
3. **Key names stay the reference's.** `RealmServerPort`, `Logger.network.trace`,
   `LoginDatabaseInfo` mean what they meant in the `.conf`. The change is the file format, not the
   vocabulary, so every document and call site still says the same word.
4. **Every value carries its reason where it is stated.** JSON has no comments, so `doc` states
   where a number came from, `note` says what a file is for, and `information` holds commentary of
   any shape - an object, an array, prose - for the explanation that will not fit in one line; all
   three are read past, never treated as keys. A number the client decides rather than the server is
   marked `immutable`, and one marked immutable without a `doc` is a complaint.
5. **The reference's parser stays, and no shipped file uses it.** `Config::parse` is the
   reference's translation with its measured quirks; it is kept because it is the reference's, and
   a `.conf` handed to this server still loads. Nothing in `data/` or in a program's directory is
   `.conf` any more.

The rules for how a document becomes that bag - sections, leaves, arrays, `value`, `hex`, `null` for
"not set" - are written down where they are implemented, at the head of
`common/src/Configuration/Json.rs`, and repeated for an operator in `docs/port/04-configuration.md`.

## Precedence, which the reference already had

An environment variable (`AC_` plus the key in upper snake) wins over the file, and the file wins over
the default the caller passes. That order does not change with the format, and it is what makes a
one-off experiment possible without editing a file a user shares.

For a datum that also lives in the database, the rule is the second half of the requirement: a
configuration file can *name* it, so a user can run a world without hand-editing rows. The realm
list is the first case, and it is not implemented yet - `docs/port/04-configuration.md` carries it as
the next piece with the precedence it will take (a file row overrides a database row of the same id,
and the load reports which came from where).

## What stays in code

One question decides it: **would two operators running this build legitimately have different
values?**

- **Yes** - a port, an address, a database connection, a log level - it is a setting and it goes in the
  program's file.
- **No** - and then no file at all: a constant in the code that uses it when the client dictates the
  value (message sizes, status codes, the handshake's arithmetic, the file check and account settings
  payloads), a row in the database when a player made it (an account, a character).
- **Not a setting, but also not a choice of anybody's** - a number that is part of an algorithm (the
  RC4 S-box, the key derivation's round constants and rotation widths, SHA-1's initial state) or
  *structural* (a field's offset inside a layout its builder writes) - stays in code, because
  separating it would let a file desynchronise the wire.

An earlier version of this section read the rule as "every hardcoded number or config", which put
content and client facts in files beside the settings. That is what the first three rows of this
record's cost line above are paying for.

The settings that are still constants today - the world list above all - are tracked, one line each, in
`docs/port/04-configuration.md`.

## What it costs

- A deviation from the reference's own config format, and the reference's `.conf` files replaced by
  JSON ones. Its parser survives; its files do not.
- `serde_json` joins `common`'s dependencies. It is used for its data model - `Value` and the
  parser - not for deriving a struct per module, which would have reintroduced a hand-written reader
  by the back door and made a setting's name a Rust identifier.
- Three wrong things were carried into files by the first version of this record and taken out again:
  the account list (a player's, so a row - 0006) and two payloads (the client's, so constants). What
  the format change actually costs is small; what reading the rule as "every number" cost was three
  corrections.
