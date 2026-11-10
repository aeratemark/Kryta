# 0005 - Configuration is JSON, read by one handler

Status: accepted. This is an addition the owner required, not a reading of the reference; the owner
may correct it, and a correction replaces this text rather than being appended to it.

## The question

The reference's configuration format is `Key = value` in a `.conf` file, parsed by
`src/common/Configuration/Config.cpp`. This tree ported that parser and every quirk in it, and both
programs read a `authserver.conf` / `worldserver.conf`.

The owner's requirement: **every hardcoded number and every configuration value is handled from JSON
configuration files**, so that users can configure and share their setups; and **no datum may live
only in code or only in the database**. A JSON file is a thing two people can diff, a web page can
show, and another program can read; a `.conf` with `Key = value` is nearly that, and a constant
inside a Rust function is not.

## The decision

1. **The format is JSON.** `authserver/authserver.json` and `worldserver/worldserver.json` are the
   programs' own files; `data/*.json` are the value files; the `.dist` files are committed and the
   live ones are not, because they hold credentials.
2. **One reader, in `common/src/Configuration/Json.rs`.** Not one per consumer: a file is parsed
   into `Document`, flattened to the reference's `dotted.key -> text` bag, and every typed getter,
   severity policy, environment override, `describe()` and masking above it is unchanged. Record
   lists - accounts today, worlds next - are read as rows through the same `Document`, never by a
   hand-written parse in the module that wants them.
3. **Key names stay the reference's.** `RealmServerPort`, `Logger.network.trace`,
   `LoginDatabaseInfo` mean what they meant in the `.conf`. The change is the file format, not the
   vocabulary, so every document and call site still says the same word.
4. **Every value carries its reason where it is stated.** JSON has no comments, so `doc` states
   where a number came from and `note` says what a file is for; both are read past, never treated as
   keys. A number the client decides rather than the server is marked `immutable`, and one marked
   immutable without a `doc` is a complaint.
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

The owner's rule is "every hardcoded number or config", and taking that to its word would put the
RC4 state and SHA-1's initial words in a file. The line this tree draws, and the reason it is drawn
here rather than nowhere:

- **A number goes into a file** when it is a machine's choice (ports, intervals, buffers), content
  (maps, spawns, defaults, accounts, realms), or a client fact that a build can change (message
  sizes, error codes, map types). The third kind is marked `immutable` with the evidence in `doc`.
- **A number stays in code** only when it is *part of an algorithm* - the RC4 S-box, the key
  derivation's round constants and rotation widths, SHA-1's initial state - or *structural*: a
  field's offset inside a layout that the builder writes, where the layout and the number are one
  thing and separating them would let a file desynchronise the wire.

Everything else in the tree that is still a constant is tracked, one line each, in
`docs/port/04-configuration.md`, with the file it is moving to.

## What it costs

- A deviation from the reference's own config format, and the reference's `.conf` files replaced by
  JSON ones. Its parser survives; its files do not.
- `serde_json` joins `common`'s dependencies. It is used for its data model - `Value` and the
  parser - not for deriving a struct per module, which would have reintroduced a hand-written reader
  by the back door and made an option name a Rust identifier.
- Two shapes to understand instead of one: settings become keys, and rows stay rows. The
  alternative - squeezing rows into keys - would make `accounts[2].token` a key name, which is how a
  configuration turns into a database nobody can query.
