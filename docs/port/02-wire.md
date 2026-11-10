# The wire, as this tree knows it

A statement of today: which of this client's wire facts this server uses, where each came from, and
what is not written. It is not a plan and not a specification of its own - when it and a recording
disagree, the recording is right and this file is wrong.

Two rules govern every line here, and both come from `AGENTS.md`:

- The **arrangement** is the reference's: the byte buffer in `shared/src/Packets/`, the opcode table in
  the game library's `Protocol/`, one file per message family in `Packets/`.
- The **contents** are this client's, measured, and each fact names the document or capture that
  recorded it. None of it is Azeroth's protocol.

## The frame

After the handshake every message is a `u16` header and then its fields, with **no length field**:
a header whose bit 15 is set came from the client, and the header selects the field layout.

- Source: `ASURA/docs/reference/client-protocol.md` section 1, which corrects an earlier claim of a
  length field and names the three reference functions that show it.
- Why it matters: a port that writes a length the client does not expect gets error `007` and
  nothing else.

Not written: the opcode table itself. The reference keeps it in `game/Server/Protocol/Opcodes` and
the record of this client's headers reports 194 client-to-server and 488 server-to-client, with the
field layouts measured per message (`client-protocol.md` section 11.7). None of them is in this tree
yet, because the conversation that uses them is not written either.

## The file check

Before the version message, on the same socket, the client asks which game files it should have:
**21 bytes in, 32 bytes out**. It compares the whole answer, and treats a wrong or missing one as
fatal.

| | |
|---|---|
| the question | 21 bytes, `01 00 00 00 00 F1 00 10 00 01` then eleven zeros |
| the answer | 32 bytes: header `0x02F1`, length `0x0020`, then six file ids in tight consecutive order with a zero among them |

- Source of the shape: this client's protocol, recorded in `ASURA/docs/reference/client-protocol.md`
  section 7. The reference server has no equivalent - its client does not ask this - which is why
  `game/FileCheck/` exists in this tree's naming.
- In this tree: both byte strings are constants in `authserver/src/Server/file_check.rs`, because they
  are a **payload** rather than a setting - nobody chooses them and the client compares
  them exactly (`docs/port/04-configuration.md`). Refreshing them is a new build: capture the reply a
  live client accepts and replace the literal.
- Proved by: the constants' lengths (21 and 32) and header; a real socket sending the question
  and then the preamble on the same connection; and a question that only starts like it, which is
  refused rather than answered.
- What the ids are not: a rule. They are a client-build fact, refreshed by capturing the reply a
  live client accepts - never by arithmetic on the previous set.

A socket that does not answer this leaves the client at "Connecting" with nothing to report, which
is exactly what this server did to the client before the check existed.
## The portal, before the login message

The client is launched with `-portal 127.0.0.1`, speaks plain HTTP to `localhost:6601`, and takes the
pair its login message will carry from four endpoints. Between "the client reached the auth port" and
"the client can log in" this is the step, and it is a different protocol on a different port.

| # | request | answer |
|---|---|---|
| 1 | `GET /Spawned/WebGate/session/create.xml` | `<Reply><Session>UUID</Session></Reply>` |
| 2 | `POST /Spawned/WebGate/users/login.xml` | the `UserId`/`LoginName`/`Provider` block |
| 3 | `POST /Spawned/WebGate/my_account/game_accounts.xml` | a one-row array for the game code |
| 4 | `POST /Spawned/WebGate/my_account/token.xml` | `<Reply><Token>UUID</Token></Reply>` |

The session comes from call 1 and travels in `Authorization: Arena <session>`. The refusals are the
reference's own: a missing or unknown session is `401`, a provider that is not `Portal` is
`400 Unsupported provider`, and a login name nobody knows is `400 who are you?`.

- Source: `ASURA/docs/reference/client-protocol.md` section 4, measured with a real client at build
  39112, where all four answered `200` and the pair they returned is what the login message carried.
- In this tree: `game/src/Portal/`, which is one of the directories the naming adds because the
  reference has no equivalent. The arrangement is a request-to-response function with no socket in
  it, so the same code answers a test and a program.
- Not decided here: who an account is. `Accounts` is supplied by the caller, and the login database
  still has no account table - that is the identity question, and it is the owner's.
- Not built: the HTTP listener on 6601, and which program hosts it. `game/Portal/` is the code's home
  by this tree's naming; a third program is not in the naming, and the reference has two.
## The answer to the login

After the exchange and a `0x8038`, the client is owed five messages in this order, and each is sent
whatever the account holds:

| # | message | size |
|---|---|---|
| 1 | `0x0007` character info, once per character | 102 for a four-unit name |
| 2 | `0x0016` account settings | 599 |
| 3 | `0x0014` friend stream end | 10 |
| 4 | `0x0011` account info | 81 |
| 5 | `0x0003` request response | 10 |

With no characters that is 700 bytes, and the client shows an empty character list; with one it is 802.
A refusal sends only `0x0003`, with status `11`.

- Source: `ASURA/recordings/gwps/world/load-stream.log`, the reference's own send trace of one login,
  which prints each message with its header, its size and its first 24 bytes. That trace is the
  authority for the sizes, and it is what settles a disagreement in the older notes: their arithmetic
  sums an extra leading 10-byte message that the trace does not carry.
- In this tree: the builders are `authserver/src/Server/login_reply.rs`, and the 591-byte block is a
  constant in `authserver/src/Server/account_settings.rs` - a payload, not a setting, so
  refreshing it is a new build rather than an edit.
- **Checked byte for byte against that trace**, without a client: five tests compare each builder's
  output - all ten bytes of `0x0003` and `0x0014`, and the first 24 of `0x0016`, `0x0011` and `0x0007`
  - with the bytes the reference printed. Every one matches.
- What the trace does not show, and a client alone can judge: the fields inside a message past its
  first 24 bytes, and the `eula_accepted` byte of `0x0011`.

## Splitting the stream: the table is the protocol

No message carries its length, so the only thing that can say where one ends is a table of sizes -
which is exactly what the reference's own reader uses. This server keeps one, which is why a single
read can hold four messages and still be answered correctly.

| header | size | what it is |
|---|---|---|
| `0x8001` | counted | computer info: two UTF-16 strings, each with a `u16` count |
| `0x8002` | 22 | computer hash, and the build number in its first dword |
| `0x8000` | 6 | heartbeat |
| `0x8023` | 6 | unnamed; the reference logs it and ignores it |
| `0x8038` | counted | portal login: request id, two 16-byte ids, then two counted strings |
| `0x8026` | 3 | accept EULA, one version byte |
| `0x8035` | 6 | ask server response |
| `0x800E` | 6 | set player status |
| `0x8029` | 26 | request game instance, six dwords |

- Source: `ASURA/recordings/gwps/world/load-stream.log`, the same trace, one line per message either
  direction, with its size. The two counted messages are walked rather than measured because their
  length is on the wire: a `u16` count of UTF-16 units, not a count of bytes and not padded out to the
  field the reference's struct reserves.
- In this tree: `authserver/src/Server/stream.rs`, which answers one whole message, "not yet", or the
  header it cannot size.
- Why it is a rule rather than an optimisation: a run against this server before the table existed
  logged a single `0x8026` with nine bytes. The client had sent the EULA (three) and its ask-response
  (six) in one write; both were read as one body, the ask-response was never answered, and the client
  waited and then reset the connection.
- A header with no known size **stops the reading** and is reported with the bytes behind it. A length
  invented for it would desynchronise every message after it, and the log would blame the wrong one.

## The request for a map, and the address it is given

| # | direction | header | size |
|---|---|---|---|
| 1 | client -> server | `0x8026` accept EULA | 3 |
| 2 | server | nothing - the reference sends nothing for it | - |
| 3 | client -> server | `0x8035` ask server response | 6 |
| 4 | server -> client | `0x0003` request response, status 0 | 10 |
| 5 | client -> server | `0x800E` set player status | 6 |
| 6 | server | nothing - the reference only logs it | - |
| 7 | client -> server | `0x8029` request game instance | 26 |
| 8 | server -> client | `0x0009` game server info | 42 |
| 9 | server -> client | `0x0003` request response, status 0 | 10 |

The `0x0009` carries `req_id`, `map_token` (the world's own id), `map_id`, a 24-byte host field and a
player token drawn per travel. The host field is Winsock's shape rather than a string: two bytes of
address family little-endian (`2` for IPv4, `23` for IPv6 - Windows' values, not Linux's), then the
**port big-endian**, then the address bytes, then zero padding. The trace shows
`02 00 | 17 E0 | 7F 00 00 01`: family 2, port 6112, 127.0.0.1.

A request this server cannot place is answered with the response alone and status `2`, and with no
address: the client then shows an error it can describe, where an address nothing listens on is a
connect timeout with no message.

- Source: `ASURA/recordings/gwps/world/load-stream.log` for the order and the bytes, and
  `ASURA/docs/reference/client-protocol.md` section 5a for the two layouts and the refusal rule.
- In this tree: `authserver/src/Server/login_reply.rs` (`game_server_info`, `host_blob`,
  `GM_ERROR_NETWORK_ERROR`) and `authserver/src/Server/session.rs`, which answers six dwords with an
  address taken from the realm list's own row.
- **Measured, 2026-11-10, build 39167: the client dials the address it was launched with, not the
  one it is handed.** This server named `127.0.0.1:6113` - the world program's own port, a port no
  program is listening on - and a listener on 6113 accepted nothing at all. The gameplay connection
  (`00 05 0C 00`, the 64-byte version message) arrived on **6112**, the auth port the client was
  launched against, 35 ms after the `0x0009` naming 6113 was sent. The client's own request, in the
  same session, was for `map 0` with `map_type 11` - the character-creation instance.
- What that settles: the address in `0x0009` is not how a client finds its game server here. The
  reference never had to know because its `0x0009` names its own auth address and it hands the
  connection to a game server over its control channel; this port must do the same, which is
  `docs/decisions/0004-gameplay-connections-arrive-at-the-front-door.md`. Naming the launch address
  instead and answering the game protocol on the auth port was rejected: it fuses the two halves.
- In this tree: `authserver/src/Server/front_door.rs`. It reads the client's 64 bytes only for
  `map_token` (offset 12), forwards those bytes whole and unmodified to the world the id names, and
  copies bytes both ways until either side ends. It does not decode the game protocol and does not
  decide who may play: the world reads the same 64 bytes, compares the token pair against the travel
  the login recorded, and closes the connection on a mismatch. A world that does not answer closes
  the client in 3 s.

## The gameplay connection's 64-byte version message

The first thing a client sends on a gameplay connection, and the whole of what the front door reads:

| off | size | field |
|---|---|---|
| 0 | 4 | the header, `0x000C0500` |
| 4 | 4 | the client's build - never compared |
| 8 | 4 | `h0008`, observed 1 |
| 12 | 4 | `map_token` - the world's id from the answer to `0x8029` |
| 16 | 4 | `map_id` - the map that answer named |
| 20 | 4 | `player_token` - drawn per travel by the login |
| 24 | 16 | the account, little-endian UUID |
| 40 | 16 | the character, the same |
| 56 | 8 | `h0038`, `h003C`, both observed 0 |

- Source: `ASURA/docs/reference/client-protocol.md` section 11.1, which records the reference reading
  the same fields in the same order (`Connection_ReadVersion`).
- In this tree: `game/src/Server/Protocol/` - the first message of the game library's protocol, with
  the offsets spelled out in a test so a reordering fails a test rather than a client.
- Not yet used for anything but routing: the token pair is what the world will compare against the
  travel the login recorded, and the login does not record a travel yet.

## The first four bytes of a connection

The client's very first bytes are a bare little-endian `u32`, not a framed message. Decoding them as
one would read `0x0400` as a message id.

| header | what it is | first message size |
|---|---|---|
| `0x000C0400` | an auth connection | 16 |
| `0x000C0500` | a gameplay connection | 64 |
| `0x000C0FFF` | a control connection | 4 |

- Source: `ASURA/docs/reference/client-protocol.md` section 2.
- In this tree: `authserver/src/Authentication/`, whose `ConnectionKind::classify` answers one of the
  three and refuses anything else rather than guessing.
- Proved by: a real socket sending `0x000C0400` and being told what it is, and one sending
  `[EF, BE, AD, DE]` and being refused with its own bytes logged.

## The exchange

```text
client -> server   header 0x4200, then 64 bytes: the client's public value, little endian,
                   read as a 512-bit integer                            (66 bytes total)
server             shared = client_public ^ server_private mod prime   (512-bit prime, 511-bit
                   exponent; neither is chosen by the server)
server             20 random bytes: the master secret
server -> client   header 0x1601, then master[i] XOR shared_le[i]      (22 bytes total)
both               key = the client's own 20-byte digest of the master
both               RC4 keyed with those 20 bytes, one instance per direction, both from offset 0
```

- Source: `ASURA/docs/reference/client-protocol.md` section 3, whose sizes and constants come from
  the reference's handshake and the client's own arithmetic.
- In this tree: `authserver/src/Server/handshake.rs` (the sizes, the two constants as decimal
  digits, `shared_secret`, `server_seed`) and `common/src/Cryptography/Arc4.rs` (the cipher).
- Proved by: three published RC4 vectors; the constants' own bit widths; and a test that plays the
  client against a real listener and recovers the master out of the XOR.

Two deviations, both recorded where they are taken:

- a public value congruent to zero modulo the prime is **refused**, because the shared secret would
  be zero and the exchange would have exchanged nothing. The reference carries on with the zero; the
  real client cannot present one, so the refusal costs nothing.
- the master secret comes from the operating system's entropy, and a machine without any logs that
  instead of silently using zeros.

## The digest the connection is scrambled with, and what will prove it

After the server seed, both sides derive a 20-byte key from the master and run RC4 with it, one
instance per direction, both from offset zero.

- Source of the shape: recorded behaviour - five rounds over the five little-endian words of the
  master, from SHA-1's initial state, with its first two round constants and two f functions, and a
  digest that adds the working state back onto the input words. Recorded in
  `ASURA/docs/reference/client-protocol.md` section 3 and implemented there.
- In this tree: `authserver/src/Server/handshake.rs` (`derive_key`) and `common/src/Cryptography/`
  (`Arc4`). It is the one wire fact here that is **transcribed rather than derived**, because there
  is no other record of it and nothing in this repository can prove it: no capture holds a master
  and its digest, there is no published vector, and no other path would fail loudly if a rotation
  constant were wrong.
- What *is* checkable here: that it maps 20 bytes to 20, deterministically, and that a changed
  master changes the digest; and that a message encrypted with the derived key is read back through
  it as its own id.

**The client is the proof, and the log now says what it says.** Every message the client sends after
the exchange is decrypted, named by its header, and answered for itself:

| the log line | what it means |
|---|---|
| `login request N, user <uuid> session <uuid> character '<name>' type '<type>'` | the derivation is right and the framing is right, because a wrong key reads the login message as noise and a wrong frame reads it as the wrong size |
| `message N is 0xXXXX with ... bytes, which this server does not answer` | a message this server has no behaviour for; its header and size are in the line |
| `a message with header 0xXXXX has no size this server knows` | the stream cannot be split any further; every byte received is printed beside it |
| `auth.trace ... 0xXXXX body: ...` | at `auth.trace` = 6: the decrypted body of each message, which the raw byte trace cannot show |

That is the whole reason both traces exist: a wrong digest, an unknown length and a message that
never arrived are three different failures, and each one now names itself.

## Where a fact goes when it is learned

A new message's header, size and layout: this file and the code that uses it, in the same commit,
with the capture or the document section named beside it. A fact with no evidence is a guess wearing
a constant's clothes, and the reference's own history has a list of what that costs.
