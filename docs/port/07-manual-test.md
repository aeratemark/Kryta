# The manual test: the client against this server, step by step

What to do with a real client, what the server must say while you do it, and what to send back when it
does not. Everything here is a claim about today's tree: if a step does not behave as written, the log
line it should have printed is the thing to report.

## Before the first run

```powershell
# the client, patched and pointed at this machine (docs/port/05-tooling.md)
python tools/client/patch-gw.py "F:\GW\GW1\Gw.exe" tools/client/keys.txt --output "F:\GW\GW1\Gw.custom.exe"
python tools/client/hosts-redirect.py status
```

The databases are the ones the programs are configured for: `kryta_login`, `kryta_world`,
`kryta_characters` on `127.0.0.1:5432` (the connection strings are in each program's own JSON, which is
not committed when it holds a password).

## Starting the programs

Three processes, each in its own window:

```powershell
cargo run -p authserver  -- -c authserver/authserver.json
cargo run -p worldserver -- -c worldserver/worldserver.json
cargo run -p kryta-manage -- -c tools/manage/manage.json   # optional: http://127.0.0.1:6602/
```

The login program must end with `READY: listening on 127.0.0.1:6112` and the world program with
`READY: listening on 127.0.0.1:6113 as world 2`; both also print a status line every 30 seconds - the
world's says `status: up N s, ... A connection(s) accepted, L live, M map(s) hosted, I instance(s) open`,
the login's `status: up N s, ... A connection(s) accepted, L live` - and that line is the first thing to
read when something seems stuck: `accepted` says something reached the port, `live` says something is
being served.

## The journey, and what the server must say

| step | you do | the server must log |
|---|---|---|
| 1 | launch the client (`Gw.custom.exe -authsrv 127.0.0.1 -portal 127.0.0.1`) | `Connection from ...: Auth connection`, the exchange, then `the login` with the five-message answer |
| 2 | type an account that exists (`user1@example.com`) | `1 character(s) listed: ...` (or `0` for a fresh account) |
| 3 | open the world map on the character screen | `the district list asked (transaction N, territory T); answered with one district in language 0 (R16)` |
| 4 | click a character and enter | `the client asks to travel to map M (...)` from the world map, then the login's `told to dial 127.0.0.1:6113 as world 2`, and the world's `the travel record is claimed - map 148, district 1, language 0` |
| 5 | create a character (the creation screen) | `the character '...' was created as ...`, `the character's N starting item row(s) are written`, `map 148 ('Ascalon City (pre-Searing)') is unlocked for the new character (R12)` |
| 6 | the outpost loads | `map 148 (...) is loading - 4 message(s), 40 bytes`, then on the client's asks: `the ping pair`, `the inventory and the map's record`, `the character stands at X, Y (plane 0) on map 148`, `the player and the load's last message`, `map 148 is loaded - the character '...' is in it`; a character that predates the unlock rows gets `the character has no unlock rows yet, so map 148 - the world's first outpost - is unlocked for it (R12)`, and a travel to a map it has not unlocked closes the connection with the refusal's log line (R12) |
| 7 | stand still in the outpost | a `0x001E` tick every `World.SimulationTickMs` milliseconds (105 by default; visible with `Network.Trace` off as nothing, on as trace lines; the status line's `up` time climbing) |
| 8 | walk | the world keeps ticking; each movement is read (trace: `a movement: X, Y` at debug) and the session stays up |
| 9 | drag an item in the inventory | `the item's new place answered with 1 message(s), 11 bytes`; the item stays where you put it |
| 10 | equip an item | the same placement answer, into the equipped bag's slot for that kind of item |
| 11 | destroy an item | `the item's removal answered with 1 message(s), 8 bytes`; the item is gone |
| 12 | reopen the world map | the district count answer again (step 3) |
| 13 | leave the client | `Gameplay connection ... closed.` and, after the hold, `Instance on map 148, district 1, was empty for 900 s and is disposed (R37, R40)` |

## What is not expected to work yet, and why

These are the plan's known gaps, each waiting on a capture rather than on work:

- **Travelling between maps** (a door, a portal, the world map's travel button): the client's request is
  read and named in the log (`the client asks to travel to map M ...; the redirect's own wire is not
  captured yet`), and nothing else happens. The redirect the client expects is the one message of this
  path without a measurement (`docs/port/06-world-plan.md`, "Where the ledger says we do not know").
- **Other players, NPCs and monsters in the outpost**: the map is drawn with no agents in it. The
  agent-creation stream is not captured on this build; the load sends its own body and nothing else.
- **Walking on real terrain**: movement is read and kept on the player's agent, but the world has no
  terrain data - the `Gw.dat` archive format has not been measured in this tree.

## What to send back

For any step that behaves differently: the client's screen (a screenshot is enough), the last 50 lines
of each program's log, and the step number above. The log line the step's row says must appear is the
one that says which half broke - the login program's, the world program's, or the seam between them.

## The same journey without the client

The world half of this journey is also played automatically against the *program* by
`cargo test -p worldserver --test end_to_end -- --ignored --nocapture`: it starts the real
`worldserver`, performs the creation screen, loads map 148, asks for the items, the spawn point and the
players, sends the travel request and reads the ticks - the same wire steps 5 to 8 do with a client.
The login half is `cargo test -p authserver --test end_to_end -- --ignored --nocapture`, which starts
the real `authserver` and performs the client's file check, version message, key exchange and portal
login, reading the character list it answers with - then the district list (`0x800B`, one district in
language 0, R16), the character click (`0x800A`, answered with status 0 for a name on the account) and
the map request (`0x8029`), checking the world it names in the answer and the travel row it records
(`0x0009` + `0x0003`, the hand-off the world claims). It then starts the real `worldserver` as well,
presents the tokens the login answered with - the world's port and the travel's player token - and
checks that the world claims that travel and answers a load block for the travel's map: the seam
between the two programs. Run it with both servers stopped, because it binds the world's own port.
Neither replaces the client: the client is still the judge of what it draws.
