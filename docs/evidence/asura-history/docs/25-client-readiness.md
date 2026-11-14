# Client readiness: what works, what differs, and what to send back

This is the page to read before a client session and after one. It states what the client can do against this
server today, which messages still differ from the reference and why, and the three commands that turn a
session into evidence.

## What works

Measured on the login-to-load path and on a portal crossing:

| step | what the client does | what answers it |
|---|---|---|
| login | patched client dials auth with `-authsrv 127.0.0.1 -portal 127.0.0.1` | `asura-auth` - the whole login batch, the character list, the settings block |
| travel to a map | world-map travel to an outpost | `asura-auth` places it; the client dials the world with its token pair |
| map load | the loading screen fills | `asura-world` - the load block, in the reference's order, at the reference's sizes |
| walking | click to move, waypoints, portal bubbles | `0x0029` per waypoint with its trap id, `0x001E` every ~100 ms |
| crossing a portal | walk into a door | the eight-message travel: effect, stop, redirect, map update, then the new connection |

Three families are exact as of the last round: the load block (`0x017C`, `0x0186`, `0x017D` shape, `0x019A`,
`0x0196`, `0x008B`, `0x008A` shape, `0x0094`, `0x01A0`), the agent population (`0x0020` at 116 bytes, `0x0021`,
`0x0022`, `0x009F`, `0x00A2`, `0x006E`, `0x00F0`), and the item and skill streams (`0x013F`, `0x0144`,
`0x0148`, `0x014D`, `0x015A`, `0x013E`, `0x001A`, `0x00DA`, `0x00DB`, `0x001D`, `0x0018`, `0x00F1`, `0x00F2`,
`0x0099`, `0x01A6`, `0x0028`, `0x000C`, `0x000D`, `0x01D3`, `0x01D4`, `0x01CC`, `0x01B3`, `0x00B0`, `0x00B1`,
`0x005A`).

## What still differs, and why

Five messages go out at a size the reference never sends, and all five carry a size that follows their
**content** rather than a fixed layout - `crates/asura-protocol/src/live_notes.rs` lists them with the
reason, and the server counts them apart from a real mismatch. None of them is a field at the wrong width:

| message | ours | live | what is missing |
|---|---|---|---|
| `0x0161` create named item | 36..60 | 176..228 | the item's display name and its modifier list; this world's items carry neither |
| `0x0188` creation success | 46 | 64 | the sixteen-byte settings blob; `creation.rs` passes `&[]` |
| `0x008A` cartography | 20 | 132 | an explored-terrain blob; this world sends an unexplored grid because the client tracks exploration, not the server |
| `0x0059` player update | 37..51 | 92 | the name plus the agent tail live appends; the field widths of the name are masked in every capture we hold |
| `0x017D` load player name | 16..30 | 44 | the same: a counted UTF-16 name whose width the masked captures cannot settle |

The two name messages are the only ones a measurement would unblock, and one `--raw` sniffer capture of a
map load settles both (`python tools/sniff-capture.py --raw ...`; `--raw` keeps every byte and warns).

## Before the session

```powershell
asura-supervisor start          # auth, webgate, world-8001; the world writes every message it sends
F:\GW\GW1\Gw.custom.exe -authsrv 127.0.0.1 -portal 127.0.0.1
```

The world records each session to `logs/capture-world-8001.jsonl`: every game message, both directions,
plaintext, one JSON line each. Nothing needs to be switched on for a session to be diagnosable after it.

## After the session

```powershell
.\tools\client-check.ps1                                                      # the whole verdict, in order
```

That runs the checks below and prints a line for each: the stack is up, the capture holds this session,
what the per-session summary said, every `LIVE-SIZE` line, every `CONFORMANCE` line, the size table for the
capture, and the command to run when something is worth chasing. The three it wraps, when one of them needs
running on its own:

```powershell
Select-String 'LIVE-SIZE|stream summary|CONFORMANCE' logs\world-8001.log     # what the server says about itself
.\tools\stream-sizes.ps1 -Capture logs\capture-world-8001.jsonl              # ours against the live sizes
python tools\stream-diff.py logs\capture-world-8001.jsonl `
    recordings\client\feature\maps\pre-searing-load-3.jsonl                   # ours against a live session
```

What the three lines mean:

- `world: stream summary for "<name>": N frame(s) sent, every one at a size the reference uses (12 whose
  size follows their content)` - one line per session, at the disconnect. This is the line a clean session
  prints. If instead it reads `M at a size the reference never sends, across K message kind(s): 0x....`,
  those headers are the defect: they are the ones whose layout drifted, and the five in the table above
  never appear there because their size follows their payload.
- `world: LIVE-SIZE sent 0xHHHH at N bytes; the reference sends ...` - printed once per message per process,
  at the moment the first such frame goes out.
- `world: CONFORMANCE sent ...` - a frame that disagrees with its own catalogue row. This should never
  appear; if it does, the table is stale and every other number in the log is suspect.

What to send back: the client's own error number or what it drew, plus the minute of the session. The capture
file and the log hold the rest, so nothing needs reproducing.

## The map population: built, switched off

A live load spawns every agent the map holds. The data for that is harvested and in the tree -
`crates/asura-world/src/npc_spawns.rs`, generated by `tools/gen-npc-spawns.py` from every `0x0020` a capture
recorded, attributed to the instance whose load info preceded it: 231 agents over the 12 instances the
captures loaded (`145`, `146`, `147`, `148`, `160`-`166` and one load with no instance id) - but **this
server does not spawn them unless `ASURA_NPC_POPULATION=1`**. A load sends the player's own agent and nobody
else, which is the stream this server was reproducing before the harvest existed.

The population is not ready to be on. It carries no agent description - no name (`0x009B`), weapons
(`0x006D`) or properties (`0x0056`, `0x0057`) - so an agent would appear as a model standing still with no
name, which is not what a live server sends and not something to put in front of a client. Those messages
are the top rows of `docs/26-packet-gaps.md`, and the switch stays off until they exist and the owner asks
for the feature.

`python tools/gen-npc-spawns.py --check` (part of the battery) keeps the table honest while it waits.

## What is still missing, in order

`docs/26-packet-gaps.md` lists every message a live server sends whose row this server has never marked as
built - sixty-nine of them, ranked by how often a live session sends each. The top of that list is the NPC
population: a live load spawns the map's agents and describes each one (name, model, weapons, profession,
titles), and this server spawns only the player. That is the largest remaining piece of work, and the one
that changes most of what a player sees. `python tools/live-gaps.py --check` keeps the list honest.

## What the server cannot see

The client's window, its error dialogs, and the terrain it thinks it has explored. Anything read off the
screen has to come from a person; everything about the stream comes from the files above.
