# Research and control-panel requirements

The ASURA documentation recorded research and a proposed operator workflow that remain relevant to
Kryta. This page retains the decisions, questions and evidence needed to resume that work. It does
not claim the proposed features exist. Kryta's actual tools and commands are in
`docs/port/05-tooling.md`; client testing is in `docs/port/07-manual-test.md`.

## Control panel

The owner requested a control panel for operating the server, capturing and inspecting the client,
guiding tests, reviewing recordings, and editing configuration and game content. The ASURA plan is
`docs/evidence/asura-history/docs/work/control-panel-plan.md`; its ASURA commands, file paths and
implementation status are historical. Kryta's existing administration page and supervisor are
listed in `docs/port/05-tooling.md`. Reuse the capabilities that Kryta actually has, and identify
missing ones before deciding implementation steps.

The intended workflows include starting and stopping services, viewing status and logs, selecting a
client process, recording packets or reading supported client state, taking snapshots, associating
observations with a run, finding previous recordings, and editing supported content. An operator can
read client state without starting packet capture, and can capture a live client without starting the
private server. A run should preserve its target, client identity, timestamps, requested and actual
capture channels, artifact locations, observations and results, including partial failures.

Client-side capture, server-side wire capture and state snapshots measure different things. The
client capture reports message structures, while the server capture records decrypted wire messages;
`docs/port/02-wire.md` defines the three capture kinds. Preserve the capture layer and provenance and
keep original recordings separate from notes and derived files. A successful tool run, an automated
check, and a player's observation are distinct results. The player's actions remain manual unless
otherwise requested.

The panel should identify the process and environment being inspected and the profile being edited
or run once profile selection exists. A saved content change does not establish a live reload. The
client research runner operates on the client machine and should not be a dependency of ordinary
server operation. ASURA's historical plan documents capture tool and hook limitations; check the
current tools before carrying any specific command or capability forward.

Desktop versus browser delivery, local versus remote operation, initial capture concurrency, first
reader coverage, storage format and profile packaging remain open design choices. The requested
research and content workflows are part of the intended panel, while their implementation order and
initial scope require a concrete plan. Profile isolation and script ownership are described in
`docs/port/11-content-and-behaviour.md`.

## Skill and combat research

ASURA's `docs/research/skill-mechanics-and-behavior.md` records a source review of Rurik at
`0710df3b5cb068362c61120820753e59a36dd149` and illustrative wiki research. It reports cast queues,
activation and recharge, projectiles, effects, attack reactions, NPC selection and targeting. It also
records important limits: Rurik's queued-cast behavior explicitly differs from its stated retail
behavior; its round-robin skill choice and healing policy are authored policies; and some examples
are incomplete. These observations are source research, not proof of live-game parity or authority
for Kryta packet layouts. ASURA's own implementation and the ledger govern a port.

The examples illustrate why the eventual engine needs operations capable of expressing distinct
effects. Projectile impact, timed regeneration, eligible damage limits and attack-triggered reactions
are different operations. Their ordering, exact values and interaction with other effects require
appropriate evidence. The owner's Lua/Rust boundary and custom-skill acceptance case are maintained
in `docs/port/11-content-and-behaviour.md`.

## Client and world research

The later ASURA world-model review reports client-context observations in
`docs/evidence/asura-history/docs/reference/game-world-model.md`, "Client-context findings recorded
2026-10-30". It describes progress arrays, map spawn lists and portal props, and corrects an earlier
agent classification. That review did not identify an ASURA capture for those context readings.
Treat them as source-only findings when selecting further measurements, not as a new wire or trigger
specification. Current asserted rules and unresolved travel and spawn questions are in
`docs/game-rules.md`.

The recovered documents on initialization, surrounding packets, unnamed headers and parity contain
historical captures, interpretations and queues. Use their cited recordings to investigate gaps,
then compare against `docs/port/02-wire.md` and the current protocol ledger in
`docs/port/09-parity-audit.md`. Captured structures and wire frames must be interpreted by their own
capture layers. Do not take historical ASURA progress tables or old commands as Kryta status.

## Recovery coverage

The historical source index in `docs/evidence/asura-history/README.md` lists the last recovered version
of each documentation path and its commit. Earlier versions remain in ASURA's Git history. The index
is a research aid; the active Kryta documents own current status and instructions. The design and
research above preserve major material identified so far. A claim-by-claim comparison of all historical
files and earlier revisions has not been completed.
