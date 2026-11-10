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
| `tools/import-creation-inventory.py` | imported the bags, items and profession sets into `data/*.json`, once | a checkout of ASURA to read | ASURA `state/inventory/seed.rs` and `default_equipment.rs` |

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

## What is still borrowed, and what porting it costs

| borrowed | used for | what porting needs |
|---|---|---|
| ASURA `tools/sniff-capture.py` | every capture this tree cites was made with it | the capture itself is done by **Py4GW_Stealth**, a third project that injects into the client (elevated, 32-bit). Porting the front end is easy; the dependency behind it is not ours to move. Until then this tree's own evidence is the server's traces (`docs/port/03-tracing.md`) |
| `ASURA/recordings/**` and `ASURA/docs/reference/client-protocol.md` | cited as the source of wire facts in a dozen code files | copy the pieces this tree relies on into `docs/evidence/`, once, with their provenance - a capture is not a dependency until the day it is gone |
| `ASURA/crates/asura-protocol`, `crates/asura-world` | read for the protocol: builders, message table, creation handlers | not a dependency: read, ported into `game/src/Server/Protocol/`, `Packets/` and `Creation/`, and never called at run time |

## Where a tool goes

`tools/` is one directory per subject, not one file per script: `tools/client/` is everything about the
client the server talks to, and `tools/import-*.py` are the one-way imports of another project's data.
A new tool states, at the top of its own file, what it reads, what it writes, and what it needs that
this tree does not have.
