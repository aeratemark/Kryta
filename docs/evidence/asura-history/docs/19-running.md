# Running ASURA

One page, in order: what you need once, how to start the stack, what success looks like, and what to do when it does not. Everything here is checked against the tree (`config/services.toml`, `crates/asura-supervisor`, the ports below); if a command here disagrees with the code, the code is right and this page is a bug.

## What you need, once

- **A checkout of this repository**, on Windows (where the game client runs) or Linux (a server host, `docs/19-running.md`).
- **Rust with cargo** - the whole stack is built from it.
- **PostgreSQL, running.** The supervisor checks for it and waits, but never starts it: on Windows it is a service, on Linux systemd. "Managed elsewhere" is the boundary (`config/services.toml`).
- **A `.env` file**: copy `.env.example` to `.env` and fill in the password. `ASURA_DATABASE_URL` is the one mandatory value; the auth bind and the trace switch are optional.
- Optional: a real `Gw.dat`, only for regenerating the path-graph dump (`asura-world --dump-path-graphs FILE`, which writes the maps of `map_config`; the flag takes the output file and does nothing without it). The worlds serve the bundled `data/path-graphs.bin`, so nothing needs it to run.

## First time: the schema

```powershell
cargo run -p asura-supervisor -- schema
```

Applies the numbered migrations, in order, through `psql` (the same command on Windows and Linux). A fresh database needs this once; re-running it is harmless.

## Start the stack: one command

```powershell
cargo run -p asura-supervisor -- start
```

What that one command does, in order:

1. reads `config/services.toml` (what this machine runs, with arguments and log paths);
2. proves the database is reachable;
3. builds the services (fast when up to date);
4. seeds the dev identities and the map catalogue (the rows a fresh database needs);
5. starts the services in their dependency order - the order a service's `depends_on` names, and
   alphabetical among services that declare none, which today is all three: the socket layer on
   `127.0.0.1:6112` (`asura-gateway`, service name `auth`), web gate `127.0.0.1:6601`, world `8001` on
   `6113` (`crates/asura-supervisor/src/config.rs:192-200`, `config/services.toml`);
6. registers every service it started in the database's `service` table, and keeps the rows fresh;
7. watches them until Ctrl+C or `stop` - a crashed service is retried at most 5 times, 2 s apart, then reported FAILED (it never restarts forever).

Ctrl+C stops the services *through* the supervisor (they run in their own process group, so they never see the console's Ctrl+C themselves). Closing the console still ends everything attached to it; a hard kill of the supervisor (no Ctrl+C, no `stop`) orphans the services - there is no code that can run on SIGKILL.

## What success looks like

```powershell
cargo run -p asura-supervisor -- status
```

- the local table shows `webgate`, `auth` and `world-8001` as `ready`;
- the registry table shows a row for each, marked `fresh` (a row whose heartbeat is older than 60 s prints `STALE` with its age);
- the three ports above accept connections.

With the game client, success is: the client asks the web gate on `6601` who it is, sends its login to auth on `6112`, reaches character select, and entering the world sends it to `world-8001` on `6113`. `tools/run-client-login.ps1` automates that attempt and prints a verdict (LOGIN ACCEPTED / REFUSED / NO LOGIN ARRIVED).

The logs are one file per service: `logs/webgate.log`, `logs/auth.log`, `logs/world-8001.log`. They are quiet by default - events only. For a diagnosis, set `ASURA_TRACE=1` in the shell before `start` (every service inherits it) and every packet is written to the logs.

## Everyday commands

```powershell
cargo run -p asura-supervisor -- status                # what is running, ready, restarting, or failed
cargo run -p asura-supervisor -- stop                  # stop everything (and the supervisor)
cargo run -p asura-supervisor -- stop --only NAME      # stop one service
cargo run -p asura-supervisor -- restart NAME          # stop and start one service
cargo run -p asura-supervisor -- build                 # build only
cargo run -p asura-supervisor -- schema                # apply the migrations
cargo run -p asura-supervisor -- seed                  # the dev identities and the map catalogue
cargo run -p asura-supervisor -- deploy-world --old 8001 --new 8002
```

`--config FILE` names a different service file (default `config/services.toml`); `build` does not read
it, and `schema` and `seed` accept it without using it (`crates/asura-supervisor/src/main.rs:52-74`).
The control channel answers on `127.0.0.1:7711`; a second supervisor on the same machine is refused.

## Rolling deploy

Uncomment the `[services.world-8002]` block in `config/services.toml` (a second world on this machine needs its own `--bind`, `127.0.0.1:6114`), then:

```powershell
cargo run -p asura-supervisor -- deploy-world --old 8001 --new 8002
```

The new build starts and is proven registered **before** the old world is marked draining. From then on the newest build serves new arrivals and reconnects; the old world keeps the players already inside it, and leaves when its last player has disconnected. Nobody is kicked. The rules and the why: `docs/02-architecture.md` (Deployment and draining).

## Starting the database yourself

The supervisor checks for PostgreSQL and waits for it, but never starts it: it is "managed elsewhere" by design. Start it the way your operating system manages services:

- **Windows:** `Start-Service 'postgresql-x64-<version>'` (find the exact name with `Get-Service *postgres*`). Starting a service may need an elevated shell; the supervisor is not elevated, so it cannot do this for you.
- **Linux:** `sudo systemctl start postgresql`.

If it is not running, `start` stops with a clear database error instead of pretending.

`tools/test-battery.ps1` is the local "everything" command (it ends printing the real-client steps, never automating them). The testing policy is in `docs/HANDOFF.md`: the real client is the test; the suite is not a gate.

## The client

The game client is Windows-only. The stock `Gw.exe` cannot talk to this server: it is patched first
with `client-patcher/patch-gw.py` and the keys in `client-patcher/keys.txt`, which are the same DH
parameters `crates/asura-protocol/src/client/handshake.rs` uses - the patched `Gw.custom.exe` and the
server agree on the handshake only as a pair. The patcher's README (`client-patcher/README.md`) has
the exact commands, the `portal.dll` step and what each patch changes.

Launch the patched client against the web gate, e.g. `-portal 127.0.0.1`, and log in as one of the
seeded accounts (`user1@example.com` / `user2@example.com` / `user3@example.com`; the web gate does
not check the password). What a good session looks like on the wire is in `recordings/`
(`recordings/README.md`).

The patcher leaves every baked-in ArenaNet hostname alone by default: the client stays live, and only
the auth and portal addresses come from its command line. That is the working configuration and the one
the seeded accounts were proven against.

Nothing else is redirected by a patch. A hostname reaches this machine only through the machine's hosts
file, put there by an operator, and the content request is answered only by a file server someone
installed. Both are deliberate and both are reversible:

1. `python client-patcher/hosts-redirect.py enable --file-server` (elevated; machine-wide block)
2. install the file server that answers `File1` content
3. build the matching executable with `--detach`, which rewrites the seven hostnames, the DNS base
   entry included

All three or none: a `--detach` build with the block off, or without the file server, cannot connect -
the client completes the auth handshake, receives the session info, then closes before the portal login
(`0x8038`) and `logs/webgate.log` never grows. Say which build is running before every session with
`python client-patcher/hosts-redirect.py status`; the block is machine-wide and removed with
`hosts-redirect.py disable`. `client-patcher/README.md`, "Read this first", is the same checklist with
the commands.

## Refreshing the file check's seven words

The reply that answers "which game files should I have" carries six `Gw.dat` file ids and a zero, and they
belong to the client build (`docs/03-protocol.md` 7, `docs/40` D30). A new client brings a new set, and
the server keeps answering with the old one until somebody replaces it. Do this once per client build.

**Do not shift the old ids by arithmetic.** The two sets on record differ by `0x100` on every id that
differs, which is what one rebuild looked like; it is not a rule about the next one.

### 1. Read the answer a live client accepted (exact, needs admin, one login)

Do it in the default configuration - `python client-patcher/hosts-redirect.py status` says `disabled` - so
the client asks ArenaNet and gets a real answer rather than ours.

```powershell
# elevated shell, from the repository root
pktmon filter add filecheck -t TCP -p 6112
pktmon start --capture --pkt-size 0 --file-name C:\temp\filecheck.etl
```

Now launch the client and log in as far as the character list, then:

```powershell
pktmon stop
pktmon etl2pcap C:\temp\filecheck.etl --out C:\temp\filecheck.pcapng
pktmon filter remove
```

Find the reply in the capture. With `tshark` installed:

```powershell
tshark -r C:\temp\filecheck.pcapng -Y "tcp.port == 6112" -x | Select-String 'f1 02 20 00'
```

With nothing installed, search the file for the header and print the seven words after it - the capture is
plain packet bytes, so a byte search finds it:

```powershell
$bytes = [IO.File]::ReadAllBytes("C:\temp\filecheck.pcapng")
for ($i = 0; $i -le $bytes.Length - 32; $i++) {
  if ($bytes[$i] -eq 0xF1 -and $bytes[$i+1] -eq 0x02 -and $bytes[$i+2] -eq 0x20 -and $bytes[$i+3] -eq 0x00) {
    "at ${i}: " + ((0..6 | ForEach-Object { [BitConverter]::ToUInt32($bytes, $i + 4 + 4 * $_) }) -join ' ')
  }
}
```

A real match prints six nonzero ids in tight consecutive order and one zero, e.g. `391339 391348 344988
391350 391351 0 391349`. Anything else is a coincidence in the file, not the reply.

Record the build beside the set, so the next person can tell whether it is still current:

```powershell
Get-FileHash F:\GW\GW1\Gw.exe, F:\GW\GW1\Gw.dat -Algorithm SHA256
```

### 2. Check a set against the archive (no admin, no client)

`crates/asura-world/src/gwdat.rs` reads `Gw.dat`'s file table, and the example prints what it finds:

```powershell
cargo run -p asura-world --example gwdat-census -- F:\GW\GW1\Gw.dat 0x5F8AB
```

Run it for all six. It says whether the id is in that archive and how big its entry is. This cannot
produce the set - the archive does not know which six files the check names - but it rejects a set taken
from another build, which is exactly what a shifted id looks like.

### 3. Ask the running client (not built yet)

Py4GW Stealth reads the client through its own image and archive code, so it can be asked what the client
expects with no capture and no admin. Nobody has done it: it needs that package's file-client API, and
`tools/sniff-capture.py` will not show the check - the sniffer records messages at the client's handler
table, and this request is a bare blob that arrives before any message.

### Where the answer goes

`crates/asura-protocol/src/client/filecheck.rs`, `FILE_CHECK_RESPONSE`, with the build and the date in the
comment above it. The gateway writes those bytes at `crates/asura-gateway/src/lib.rs`, step 0. That address
also serves the content the client streams; this server answers the check and nothing else, which is why a
redirected build still needs a content file server installed (`client-patcher/README.md`).

Whether a stale set is refused is not measured. The one run where a server of ours answered left the
client's other file sockets on ArenaNet, so it may never have compared our words. No answer at all is
fatal, and that one is measured.

## Linux

`docs/19-running.md`: the same supervisor commands; the client stays on Windows; the Linux host has not had a live run recorded yet, and the runbook says so. A world registered at a second address (the multi-machine proof) is `docs/history.md`.

## Docker (optional)

A second runner lives in `docker/` and is optional: the non-Docker path above is unchanged and
remains the default. The bundle is two containers - PostgreSQL, and one "machine" container that runs
`asura-supervisor` exactly as a host does: same start/stop, same schema and seed, same registry rows,
same logs. Docker only supplies the machine and the network around it.

```powershell
docker compose -f docker/compose.yml up -d --build
docker compose -f docker/compose.yml exec machine target/debug/asura-supervisor status
docker compose -f docker/compose.yml logs -f machine
docker compose -f docker/compose.yml down
```

- The Windows client dials `127.0.0.1:6112` and `127.0.0.1:6601` as it always does; those are the
  only published ports. Worlds are dialled by the auth service inside the same container.
- `config/services.docker.toml` is the same service list as `config/services.toml`, with one
  difference: auth (`:6112`) and the web gate (`:6601`) bind `0.0.0.0` inside the container, because a
  published port forwards to the container's network address, not its loopback. The world stays on
  loopback - the client never dials it directly.
- The database password is `${ASURA_DB_PASSWORD}` (default `asura`); set it in the shell or a `.env`
  next to the compose file before the first `up`.
- The image stamps the world's build id as `docker` (`ASURA_BUILD`), since a Docker build has no
  `.git`; a rolling deploy still works because placement uses `world.started_at`.
- The supervisor treats `docker stop` (SIGTERM) as the same quit as Ctrl+C, so a stop marks the
  registry rows `stopped` instead of being killed blind. Verified: `docker compose stop machine`
  left every `service` row at `stopped`.

**What was run (2026-10-28, Docker Desktop 4.97.0 on Windows):** the image builds, both containers
start, the supervisor applies the migrations, seeds the identities and maps, starts webgate, auth and
world-8001 (all `ready`), the registry rows read `ready`/`fresh`, and from the host the gate answers
`GET /Spawned/WebGate/session/create.xml` with a session UUID and the auth port accepts connections.
**Not yet run: the real game client against the bundle** - that is the owner's session, exactly as
for the host path.

## CI: format and lint, nothing else

`.github/workflows/ci.yml` runs two checks on every push:

- `cargo fmt --all -- --check` - the code is formatted;
- `cargo clippy --workspace --all-targets -- -D warnings` - the linter is clean.

Nothing else runs automatically. The test suite needs a live PostgreSQL database to mean anything, and the repository's testing policy says the game client is the real test and the suite is not a gate (`docs/HANDOFF.md`) - so CI is the cheap, database-free half, and the acceptance runs stay in the battery and with the real client. The decision is recorded in `docs/history.md`, S7.

## When it goes wrong

| what you see | what it is | what to do |
|---|---|---|
| `cannot listen on 127.0.0.1:7711 ... another supervisor is probably running` | a supervisor is already up | `cargo run -p asura-supervisor -- stop`, then `start` |
| `could not bind 127.0.0.1:6113` | something else holds a port | `stop` waits for its ports to free; a leftover process needs killing once (`Get-NetTCPConnection -LocalPort 6113` on Windows names it) |
| the client shows error `007` | the server and client disagree on a message | `ASURA_TRACE=1`, then compare the trace with a good session in `recordings/` |
| the client closes shortly after the login screen appears, and `logs/auth.log` ends the session after the unnamed `0x8023` with no `AUTH_CMSG_PORTAL_ACCOUNT_LOGIN` | a repointed client without the provisioning: the hosts block is off, or there is no file server | `python client-patcher/hosts-redirect.py status`; enable the block and install the file server, or run a plain build, which is live |
| registry rows say `STALE` | a service stopped publishing without leaving | `status` shows the state; the reaper frees orphaned pool leases; `stop`, then `start` |
| `ASURA_DATABASE_URL is not configured` | there is no `.env`, or it is missing the URL | copy `.env.example` to `.env` and fill it in |
| `cargo cannot reach crates.io` | no network, or a sandbox token blocking TLS | the stack runs offline once dependencies are cached; from a normal shell, or with network access allowed |

## Running the server on Linux

> Moved here on 2026-11-06; it was its own file, `19-running.md`. The text below is as it was written.

> **Status: a runbook, and honest about what it is not.** The server (web gate, auth, world servers,
> database) runs on Linux. The **game client remains Windows-only** - the client rig, its patching
> and its hooks are Windows work; a Linux machine is a server host. The services and the supervisor
> use no platform-specific APIs and the supervisor finds `asura-*` with or without the `.exe` suffix,
> so the same repository runs on both. What Linux has **not** had yet is a live run by this project:
> the evidence section at the end is empty on purpose, and the first operator fills it in.

## What runs where

| piece | runs on | listens |
|---|---|---|
| `asura-webgate` | the server host | 6601 |
| `asura-gateway` | the server host | 6112 |
| `asura-world` (one process per world) | the server host, or another machine | 6113 (and its registered public address) |
| PostgreSQL | the server host, or another machine | 5432 |
| the game client | a Windows machine | connects to the gate and the auth address |

The client is never told to connect to a world directly: it dials the address it was launched with
(the auth service), and the front door routes the gameplay connection (`docs/03-protocol.md` #4).
So a remote client only needs the **web gate** and the **auth service** reachable; the world can stay
on `127.0.0.1` when everything is on one machine.

## Prerequisites

- **Rust**: a current stable toolchain (`rustup`); the workspace declares `rust-version = "1.75"`.
- **Git**.
- **PostgreSQL 16**, running, with a role and a database for ASURA.
- A `.env` at the repository root (copy `.env.example`): `ASURA_DATABASE_URL` is the one every
  program reads. Secrets stay there; topology stays in `config/services.toml`.

## First time on a fresh machine

Run from the repository root.

1. **Role and database** (with a superuser, e.g. `sudo -u postgres psql`):

   ```sql
   CREATE ROLE asura LOGIN PASSWORD '<password>';
   CREATE DATABASE asura OWNER asura;
   ```

2. **Schema**: one command applies every migration, in order (they are idempotent, and a program
   started against an older schema refuses to run):

   ```bash
   cargo run -p asura-supervisor -- schema
   ```

   It drives `psql` - taken from `ASURA_PSQL` first, then PATH (`psql` on Linux), then the usual
   Windows install (`crates/asura-supervisor/src/psql.rs:13-31`).
   The equivalent by hand is a `psql -v ON_ERROR_STOP=1 -f` loop over
   `crates/asura-db/migrations/*.sql`.

3. **`.env`**: copy `.env.example` to `.env` and set `ASURA_DATABASE_URL` (and the `ASURA_DB_*`
   values if you keep them in sync).

4. **Build** - the first build needs the network for the crates; after that the supervisor's
   `--offline` build works from the cache:

   ```bash
   cargo build
   ```

5. **Seed** - one command writes both sets of rows: the dev identities the client presents, and the
   map catalogue. The SQL is in the repository (`tools/seed-dev.sql`, `tools/map-config.sql`) and
   is applied the same way on Windows and Linux:

   ```bash
   cargo run -p asura-supervisor -- seed
   ```

   The three portal pairs (they must resolve, or the login is refused):

   | account | portal user id | session id |
   |---|---|---|
   | `user1@example.com` | `fa520ee2-4419-4eb4-ae49-6e9abe6ef24f` | `d8b9bf5d-90b1-4cbd-9b76-88da7be763b6` |
   | `user2@example.com` | `5fb222bc-9d19-4308-ac3e-3c62685bc6ae` | `224aee30-d8a5-4f18-ad8e-b5fcbf121c4e` |
   | `user3@example.com` | `9957c79c-b93c-4b60-9d84-9f38a1f62be3` | `ccdb4b8d-1c88-48c0-bda0-0a5e8d87f4da` |

6. **Start** - one command, as on Windows:

   ```bash
   cargo run -p asura-supervisor -- start
   ```

   It builds what is missing, seeds the same rows step 5 writes (idempotent, so a running start
   re-seeds harmlessly), starts the services in dependency order - alphabetical while none declares
   `depends_on`, so auth, then the gate, then world (`crates/asura-supervisor/src/config.rs:192-200`) -
   waits for each to be healthy, and watches
   until Ctrl+C. The first build of the three services inside `start` uses `--offline`; because
   step 4 fetched everything, that succeeds.

7. **Verify**:

   ```bash
   cargo run -p asura-supervisor -- status        # webgate, auth, world: ready
   curl http://127.0.0.1:6601/health              # 200 ok
   ```

## Reaching it from a Windows client

The client is launched with the addresses it should dial:

```
Gw.custom.exe -authsrv <server-host> -portal <server-host>
```

- The **web gate** (`-portal`) and the **auth service** (`-authsrv`) must be reachable from the
  client. In `config/services.toml`, their `args` bind `127.0.0.1` - change to the reachable address
  (e.g. `"0.0.0.0:6601"` and `"0.0.0.0:6112"`) and allow the ports through the host firewall.
- The **world** can stay on `127.0.0.1` when the gate and auth run on the same machine: the front
  door dials the world's registered address from there. If a world runs on another machine, its
  `--public` must name an address the **auth host** can reach (it is what the front door dials).
- Game traffic is the client's second connection to the auth address; no wiring changes are needed
  for it (the front door routes it).

## Day to day

```bash
cargo run -p asura-supervisor -- start            # the one command; Ctrl+C stops
cargo run -p asura-supervisor -- status
cargo run -p asura-supervisor -- stop             # or stop --only <service>, or restart <service>
```

- Logs: `logs/webgate.log`, `logs/auth.log`, `logs/world-<id>.log` (appended).
- The supervisor stops, on exit, exactly what it started; the database is checked and waited for,
  never started (on Linux it is a systemd unit, managed elsewhere).
- A crashed service is restarted at most `restart_max` times (see `config/services.toml`), then
  reported `failed` - never forever. A stop waits for the service's port to be free.

## A release build

The supervisor's `build` step builds the services in **debug** (`start` also runs it, harmlessly, if
the programs already exist). For a production host:

```bash
cargo build --release
```

and point `config/services.toml`'s `exe` entries at `target/release/asura-*` (no `.exe` on Linux;
the supervisor finds either). Nothing else changes.

## What this document does not cover yet

- **No CI-built binaries** - Linux artifacts (CI) are a later item; today, build from source.
- **No live Linux run recorded** - the code is cross-platform by construction, not by measurement.
  The row below stays empty until someone runs it and writes down what they saw.
- **The client rig is Windows-only** - that is the client's own work, not the server's.
- The supervisor's own tests are live-database tests: they run with `ASURA_DATABASE_URL` set (and
  skip cleanly without one).

## Evidence (fill in when someone runs it)

| what | expected | observed |
|---|---|---|
| first start | `webgate`, `auth`, `world-<id>` all `ready` | |
| gate health | `GET /health` -> `200 ok` | |
| client login from Windows | character list, then select | |
| travel | `/tp` lands on the world | |
| stop | Ctrl+C (or `stop`); every port free | |
| notes | - | |

## The run sections, from the handoff

### Run it: one document

The "how do I run this" page is **`docs/19-running.md`** - what you need once, the one command, what
success looks like, the everyday supervisor commands, the rolling deploy, the client, CI, and a
troubleshooting table. It is the only place those instructions live now; this section keeps only the
operational facts behind them:

- **The project runs natively on this machine**: `asura-supervisor`, host path, and it is the one start
  path on both operating systems (`AGENTS.md` #7). `docker/` exists as an offer and does not drive the
  project.

- **The run is quiet by default**: `-Trace` prints every packet, and the packet log is
  off unless asked for, because a world tick every 500 ms is three lines a second forever and it buries the
  lines that matter. What is *always* printed is the events - `auth: login ok for ...`, `auth: travel to map
  146 placed on world 8001 ...`, `world: ... joined map 146 ... with 6 bag(s) and 9 item(s)`, a saved position, a
  dropped item, a moved item, a worn item, a refused move, a chat that was not sent.
- **The web gate is not optional for a client attempt.** With `-portal 127.0.0.1` the client asks the gate
  on `6601` who it is before it sends a login; with nothing there it opens the auth connection, does the key
  exchange, sends its preamble, and closes - which looks like "I cannot connect to the server" while the
  server is running perfectly (`AUTH_CMSG_PORTAL_ACCOUNT_LOGIN` never arrives). The gate is this
  project's own (`crates/asura-webgate`).
- **The launcher scripts are retired** (2026-10-28, S1's separate step): `tools/start-server.ps1`, `tools/play.ps1` and `tools/stop-server.ps1` are gone - no more PowerShell in the run path, and the supervisor is the one start path on Windows and Linux (`docs/19-running.md`). What they did that the supervisor does not: `start-server.ps1` started PostgreSQL (elevation); the database is "managed elsewhere", so start it yourself - `Start-Service 'postgresql-x64-<version>'` on Windows, `sudo systemctl start postgresql` on Linux. A hard stop no longer leaves a visible "what is still leased" note: the supervisor's stop is clean, and a hard kill still leaves a connected player's pool slot for the unscheduled reaper. M2 is provable at any time with `tools/m2-verify.ps1` (integration tests + the fake-world scenario, live database).

### Run it: the supervisor (cross-platform)

`crates/asura-supervisor` starts this machine's services from `config/services.toml` and watches them
(`docs/history.md`, S1; the how is `docs/19-running.md`). It is the one start path on
both operating systems; the PowerShell launchers it replaced are retired. **Starting the server is one command**
(`start`); it builds what is missing, seeds the rows a fresh database needs (the same steps the old
start script ran), starts everything, and watches until Ctrl+C or `stop`. It also carries the two setup
steps as cross-platform commands: `schema` (the migrations) and `seed` (the dev identities and the map
catalogue) - the SQL lives in `tools/seed-dev.sql` and `tools/map-config.sql`, applied through `psql`
the same way on Windows and Linux. Every service it starts is registered in the `service` table (S2):
kind, address, port, build, state and a heartbeat; `asura-supervisor status` prints that registry with
a STALE marker, and a stopped supervisor leaves the rows at `stopped`. A world's build is stamped with
`git describe` (`--build` overrides it), and `deploy-world --old N --new M` starts the new build,
proves it is registered, and only then marks the old world draining - the old world keeps its players
and leaves once its last one reconnects, with nobody kicked (`docs/02-architecture.md`, Deployment and
draining). On a Linux server host, the runbook is `docs/19-running.md` (same commands; the
game client stays Windows-only, and Linux has not had a live run recorded yet). The command list lives
in `docs/19-running.md`; it is not repeated here.

Two operational rules the supervisor encodes: a crash is retried at most `restart_max` times
(`config/services.toml`), then the service is reported FAILED - never restarted forever; and the
services run in their own process group, so Ctrl+C stops them *through* the supervisor (closing the
console window still ends everything attached to it, and a hard kill of the supervisor orphans the
services - there is no code that can run on SIGKILL).

(No network is needed for either once dependencies are cached. If `cargo` cannot reach crates.io,
the sandbox token may be blocking TLS - run it from a normal shell, or with full access.)
