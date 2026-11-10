# The tools: what this tree owns, and what it still borrows

A server is not the only thing this project runs. There is a client to patch so it talks to us instead
of ArenaNet, a machine setting that decides whether it reaches us at all, and captures that say what
the client sent when a log is not enough. Those are tools, they are part of the deliverable, and they
live in this tree.

## The rule

**No tool reads another project's tree at run time.** A tool here takes its input from this repository
or from the machine, and a fact it needs from outside is imported once, as data, with the import named.
A port that reaches into `C:\Users\Apo\Documents\GitHub\ASURA` when it runs is not ported yet - it is
borrowed, and the borrow is the thing that breaks when that tree moves.

Two consequences:

- An imported row set is data, not a link: `data/inventory.json` and `data/equipment.json` are this
  tree's files, and `tools/import-creation-inventory.py` is the record of where they came from. Running
  it again is provenance, not a dependency - nothing in `cargo test` or either program calls it.
- A tool that must depend on something external says so at the top of its own file, and says what
  happens without it.

## What is here now

| tool | is | needs | ported from |
|---|---|---|---|
| `tools/client/patch-gw.py` | patches a `Gw.exe` into `Gw.custom.exe`: the handshake's DH keys, the `CreateMutexA` patch, the mutex name, and optionally the baked-in ArenaNet hostnames | `pip install pefile` | ASURA `client-patcher/patch-gw.py` |
| `tools/client/keys.txt` | the key material the patch writes: `generator`, `prime`, `server_private`, `server_public` | - | ASURA `client-patcher/keys.txt`; the prime and `server_private` are the same values as `shared/src/Cryptography/Handshake.rs` |
| `tools/client/hosts-redirect.py` | sends the client's seven baked-in names to this machine, or puts them back; recognises a block an earlier tool left | an elevated shell to write the hosts file | ASURA `client-patcher/hosts-redirect.py`, itself from GWPS `tools/hosts-gwps.ps1` |
| `tools/adapt-protocol-table.py` | moved the ported message table into `game/src/Server/Protocol/Tables.rs`, once | nothing; it is the record of the move | ASURA `crates/asura-protocol/src/packets.rs` |
| `tools/import-creation-inventory.py` | imported the bags, items and profession sets into `data/*.json`, once | a checkout of ASURA to read | ASURA `state/inventory/seed.rs` and `default_equipment.rs` |

## The administration tool

```powershell
copy tools\manage\manage.json.dist tools\manage\manage.json   # then state the two database strings
cargo run -p kryta-manage -- -c tools\manage\manage.json       # http://127.0.0.1:6602/
```

`tools/manage/` is the third binary: one page over every piece of game data an operator changes. It is
not a server - no client talks to it, it serves no game protocol, and neither server program knows it
exists (`docs/decisions/0008-administration-tool.md`). It links the same libraries the servers link and
reaches the data through the nouns' own repositories, so what a page writes is what a server reads.

| page | is | state |
|---|---|---|
| `/` | what exists, and how much of it | accounts, with how many accepted the EULA |
| `/accounts` | the rows the login reads: create, remove, and set the EULA version | done |
| `/characters` | a character's settings, the columns the client sees | next |
| `/worlds` | the realm list and `data/realms.json` | next |
| `/content` | the maps, inventories and profession sets, validated before a write | next |
| `/schema` | what the updater has applied | next |

Proved end to end on this machine: the page lists both accounts, `POST /accounts/eula` set a version and
the row read back as `version 26`, `POST /accounts/create` wrote a row and `POST /accounts/delete` removed
it, each answering a redirect to the list.

The EULA itself is a column now (`data/sql/base/db_auth/account_eula.sql`): the version an account
accepted, or 0 for none. The auth program's answer reports it, so a client that has accepted stops asking.

## Building and running the client

```powershell
pip install pefile                                                                    # once per machine
python tools/client/patch-gw.py "F:\GW\GW1\Gw.exe" tools/client/keys.txt --output "F:\GW\GW1\Gw.custom.exe"
python tools/client/hosts-redirect.py status
& 'F:\GW\GW1\Gw.custom.exe' -authsrv 127.0.0.1 -portal 127.0.0.1
```

Proved: Kryta's patcher, run against this machine's `Gw.exe` with Kryta's `keys.txt`, produces a
byte-identical `Gw.custom.exe` to the one that was in use (SHA-256 `4937CBF7...`), so the port changes
nothing about the client it builds.

The hosts block is **off by default and that is the normal state**: with the patch above the client
dials the address on its command line, so nothing needs the block. `enable --file-server` is for a
client built with `--detach`, which repoints the client's own DNS base name and therefore needs both
the block and a content server.

## Capturing a client

```powershell
# start an elevated PowerShell first: py4gw.connect() is a write, and refuses without it
python tools/client/sniff-capture.py --seconds 90 --dir both --out recordings/client/feature/<subject>/<name>.jsonl
python tools/client/sniff-capture.py --self-test        # needs neither a client nor elevation
```

Proved: the self-test passes from this tree (masking, record writing, the capture header), and the
copy carries the same records ASURA's did.

**Py4GW_Stealth is a declared dependency of this tool and of nothing else here.** It is a separate
project that injects into the client (`--py4gw-root`, or `PY4GW_STEALTH`); no program in this
repository reads from it. A record is the client's message *struct*, not the packed frame -
`docs/port/02-wire.md`, "Three kinds of capture", and the `layer` field in every
capture's header say which one a file holds.

## Evidence

`recordings/` and `docs/evidence/` hold the measurements this tree cites, copied here so a citation
points at something in the repository. `docs/evidence/README.md` says what each file is, where it came
from, and that a document there is evidence rather than a rule.

## What is still borrowed

| borrowed | used for | status |
|---|---|---|
| `ASURA/crates/asura-protocol`, `crates/asura-world` | read for the protocol: the message table, the builders, the creation handlers | not a dependency: read, ported into `game/src/Server/Protocol/`, `Packets/` and `Creation/`, cited where a fact came from, and never called at run time |
| `ASURA/tools/*` beyond the four above | their own milestones: test batteries, seed scripts, table generators | not used here; a tool this tree needs is ported before it is used |

## Where a tool goes

`tools/` is one directory per subject, not one file per script: `tools/client/` is everything about the
client the server talks to, and `tools/import-*.py` are the one-way imports of another project's data.
A new tool states, at the top of its own file, what it reads, what it writes, and what it needs that
this tree does not have.
