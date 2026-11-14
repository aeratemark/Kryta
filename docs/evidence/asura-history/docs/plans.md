# Plans

Unbuilt work, one section per thread. Each section is as it was written on its date.
Where two sections disagree, `docs/40-guidelines.md` governs, and the newer date wins.
Nothing here is a decision the owner has not made.

## World simulation implementation handoff

> Moved here on 2026-11-06; it was the file `plans.md`.

Recorded 2026-11-01. This replaces the earlier high-level proposal in this file.
It is the detailed handoff requested by the owner for another implementation agent.
It documents intended work; writing this handoff does not implement any phase.

## Read this before acting

Read AGENTS.md first. It governs the work. Then read docs/HANDOFF.md for existing decisions and
the relevant source files named below. This document does not override those rules.

The simulator's first assignment is Phase 1: configurable map spawns and portal arrivals.
The owner expanded its editing interface into a control panel for server operations, Py4GW
Stealth capture sessions and in-game data reading, guided tests, and configuration/content
management (2026-11-02). Read
docs/plans.md for that panel's design and unresolved product choices.
The map editor described here is the panel's content increment, not its complete product scope.
Phases 2-7 describe the subsequent simulator work and its boundaries.
Start only the phase assigned by the owner. An instruction to implement a phase authorizes the
ordinary edits, builds, and targeted checks needed for that phase; it is not permission to add
unlisted features or change unrelated gameplay.

The seven phases here are simulation work phases. They are not the older infrastructure
milestones or the four packet-parity stages described elsewhere in the repository.

If the owner assigns the whole roadmap, follow the order and the stated decision gates. Do not
skip a gate, count a placeholder as completion, or silently choose a missing product/game rule.
Ask about unresolved requirements before doing the work that depends on the answer. Continue
independent authorized work where possible. Do not ask the owner to choose variable names,
routine local code organization, or other implementation details already settled by the tree.

## The owner's intention

The owner wants an actual Guild Wars world simulation built on ASURA's working connections,
travel, movement, pathfinding, inventory, database, and packet handling. Reuse those components.
Existing data readers and configuration must reach the running world instead of remaining
disconnected from it. Content changes must not require editing and rebuilding Rust each time.

The immediate example is map travel: default map spawns, portal destinations, and the landing
points used after a crossing must be editable as content. The first visible result is editing
the Ascalon City / Lakeside County configuration through a small manager and verifying the
result with the real client.

The eventual world contains NPC placements, roaming and patrolling agents, quest givers,
services, enemy groups, combat, main and secondary quests, chains, escorts, companions, and
story-dependent encounters. These are the direction of the roadmap, not a demand to implement
all of them in Phase 1.

The AI work expresses HeroAI's decision logic in external Lua scripts that use authoritative
Rust state queries, actions, and execution machinery.
It is not a port of client support structures: no shared memory, GLOBAL_CACHE, account slots,
client input queues, or injected client environment. Their replacement is assumed, not a
feasibility objection to the combat policy.

The owner's sizing example is 100 simultaneous instances with 200 AI-controlled agents each:
20,000 agents. That is a workload to measure, not a claim that one machine already supports it,
not an AI decision rate, and not permission to reduce gameplay fidelity.

The packet-stream objective remains the specification for reproducing the base game. Future
custom content is intended to be possible; that does not authorize changing base-game behavior.
When a requirement or feature choice is genuinely unresolved, ask instead of designing it on
the owner's behalf. This is an explicit instruction from the owner for this handoff.

## Decisions, intended scope, and unresolved choices

The owner selected self-contained game profiles with no automatic base-content inheritance.
Read docs/plans.md before designing content storage/loading. The future
launcher chooses the profile; launcher construction is not a prerequisite. Each profile has its
own account settings, characters, levels, and progress. Profile data format and the storage layout
that implements this isolation remain unresolved; decide them before dependent work.

| Item | Status | What the implementer must do |
|---|---|---|
| Rust provides and executes simulation mechanics; external Lua scripts define game behavior across all domains | Chosen | Apply the boundary in docs/plans.md; Rust retains authoritative state and behavior-tree execution |
| Lua is the content scripting language | Chosen | Do not reopen Rhai versus Lua |
| BehaviorTree.CPP defines behavior-tree execution rules | Chosen | Follow ADR 0014; implement in Rust |
| HeroAI supplies combat decision logic to port | Chosen direction | Preserve decisions while mapping them to server state and actions |
| Database stores installed content and durable game state | Existing architecture | Use asura-db; load active simulation state into memory |
| A small content manager arrives with configurable maps | Intended Phase 1 scope | Deliver an editing interface, not only SQL instructions or a CLI |
| JSON import/export for contributors | Earlier recommendation, not owner-selected | Ask before making this a required workflow; do not block independent map work on it |
| Event-driven Lua handlers | Proposed approach | Settle the event contract before Phase 6; Lua selection alone does not settle it |
| Manager UI technology and delivery | Not chosen | Ask the Phase 1 interface question before building the app shell |
| Spawn-selection, party ownership, persistence, and phasing rules | Partly recorded; partly open | Read existing decisions first; ask only about what remains unresolved |

Operational configuration remains with config/services.toml, environment, and the supervisor.
Map definitions and NPC/quest definitions are content. Health, cooldowns, inventory, and earned
experience are changing game state. Wire constants and field layouts belong to the protocol.
Do not turn all hardcoded numbers into operator settings.

A manager, a JSON importer if selected, and Lua actions must use the relevant shared Rust
operations. Do not create separate implementations of content validation, travel, combat, or
progression in each interface.

## Entry check for the implementation agent

1. Inspect git status and identify existing work. Do not overwrite, revert, or include unrelated
   changes in the implementation. Several movement changes were present during this handoff.
2. Check the actual code and current recordings, not only status paragraphs in older documents.
   Refresh file:line references if they moved. Do not assume a listed gap still exists.
3. State the active phase, the exact feature being changed, and its source of truth.
4. Apply AGENTS.md's pre-flight sentence before behavior changes. Content extraction and a
   gameplay-policy change are separate subjects.
5. Resolve only the decision gates needed for that phase. Missing later-phase decisions do not
   prevent independent earlier work.

Source locations inspected while preparing the plan:

| Location | What to verify |
|---|---|
| crates/asura-db/src/game.rs:55 | Default map settings read from map_config |
| crates/asura-db/src/game.rs:69 | map_spawn list reader; no caller under crates/ was found in the review |
| crates/asura-db/migrations/0020_map_spawn_points.sql | Existing spawn-list schema and row-zero seed |
| crates/asura-world/src/map.rs | In-memory map definition and shared path graphs |
| crates/asura-world/src/portals.rs:15 | Portal shape; current source/destination/arrival data |
| crates/asura-world/src/portals.rs:32 | Current reach value 250, documented as unmeasured |
| crates/asura-world/src/portals.rs:39 and :71 | Ordered crossed-door and discovered-door tables |
| crates/asura-world/src/travel.rs:408 | Arrival x/y is written before transfer, using plane 0 |
| crates/asura-world/src/connection.rs:254 | Current selection from map default and saved position |
| crates/asura-world/src/instance.rs:19, :34, :368 | Player slots, instance state, and registry keyed by map id |
| crates/asura-world/src/player.rs:144 | Experience test value 500 rather than maintained progression |
| crates/asura-protocol/src/game/mod.rs:335 | Fixed health/energy values used by existing builders |
| config/services.toml and docs/19-running.md | Existing process management and run path |
| docs/22-world-model.md, "Spawn points, and the outpost rule" | The owner's existing entry, travel, and last-outpost decisions |
| docs/plans.md | Combat-policy review and algorithmic scaling concerns |
| docs/adr/0014-behavior-tree-execution.md | Pinned sources, execution differences, and migration checks |

The portal radius being unmeasured is not permission to replace it with a guessed better value.
The existence of multiple spawn rows is not permission to activate a new selection rule while
extracting the current configuration.

## Phase 1 - Configurable map travel and the first manager

### Goal and fixed scope

Deliver database-backed map entry and portal-arrival configuration, editable through a small
manager, and consumed by the world without a Rust rebuild.

The initial editing and client acceptance flow covers:
- Ascalon City, map 148.
- Lakeside County, map 146.
- Their known crossings in both directions.

Preserve all other existing map and portal data. Restricting the first screen and client example
to two maps is not permission to delete the remaining catalogue or recorded crossings.

### Questions to ask before dependent work

- Should the first manager be a local browser app or a desktop app? Where should it run relative
  to the server/database? Do not choose a new UI framework or launch model before this answer.
- Is JSON import/export wanted in this first delivery, or should it remain later work? It is not
  a substitute for the editing screen.
- If the current map_config default and map_spawn row zero disagree, which recorded value is
  authoritative? First check their sources and the live behavior; do not silently select a winner.

Do not ask again about Rust, PostgreSQL, Lua, or the need for the small manager. These are settled
at the level needed here. If the assigned environment cannot deliver the selected manager form,
report the concrete limitation rather than replacing it with a different product.

### Work packages, in order

1. Inventory the default spawn fields, spawn-list rows, portal rows, readers, and travel consumers
   for the selected maps. Record which runtime paths consume each value.
2. Preserve the current values and their evidence when migrating compiled portal content into
   database-backed definitions. Keep discovered/unlinked doors distinct from working crossings.
3. Give content rows stable identities. Keep map identity, portal content identity, live instance
   identity, and agent identity distinct. A generated row number is not a measured client agent id.
4. Establish one authoritative read model for the supported defaults and portal arrivals.
   Maintain existing compatibility fields consistently where readers still need them. Updates
   spanning related rows must not leave the default and its list disagreeing after a failed save.
5. Preserve current trigger precedence. CROSSED is searched before PORTALS and there are overlapping
   entries. Do not deduplicate, reorder, or reinterpret them merely because the database has keys.
6. Preserve nullable destinations and arrivals. An unmeasured destination is not map 0. Do not
   require inventing a destination to import an inactive discovered doorway.
7. Load definitions into the world's memory through asura-db and connect the existing travel
   paths to them. Reuse the current transfer cycle. Do not query the database on every movement
   tick just to find a static portal.
8. Build the first manager screen and save operation against that same content model.
9. Document how to start the selected manager, save, restart the world, and verify the result.
   Extend the existing supervisor only as required by the owner-selected delivery form.

Initial editable information:
- The selected maps' current default spawn x, y, and plane.
- Existing known portal links' destination and landing x, y, and plane.

Show map/portal identity and source location so the owner can identify the record. A plane is a
pathing plane, not terrain height. Do not infer height or renumber planes.

Use the existing data representation for additional spawn rows and preserve them. Making
individual extra rows editable, adding/removing records, exposing source trigger coordinates or
radius, and adding random or conditional selection are separate feature choices; ask before
adding them to the first screen.

### Behavior boundaries

- Extract the current behavior first, preserving values and the order/meaning of emitted messages.
- A deliberate coordinate edit is the explicit configuration test, not a new default to commit.
  Restore the original development data after that test unless the owner says to retain the edit.
- Initial changes take effect after the world restarts. Do not add live mutation of active
  instances or hot reload to satisfy this phase.
- Do not change portal detection geometry, collision, movement, travel permissions, capacity
  policy, reconnect behavior, last-outpost rules, or the explorable initialization workaround.
- Normal entry selection, portal arrivals, quest placement, and reconnect/session placement are
  different cases. Read the existing owner decisions in docs/22-world-model.md. Implementing the
  missing cases belongs to separately assigned behavior work; do not smuggle it into extraction.
- If connecting an unused reader would change current placement, document that difference and
  resolve it before wiring the new selection into live behavior.
- The map editor is the content part of the broader control panel in docs/plans.md.
  Server operations and capture/test workflows are now explicitly requested panel capabilities.
  User/account administration, a visual map canvas, generic database browsing, and production
  remote administration are still unselected features, not consequences of that request.

### Validation and completion

Reject malformed coordinates and invalid required references with a useful record/field error.
Allow deliberately incomplete inactive doorway records. Validate planes against available
geometry where that evidence exists; do not pretend an unavailable graph proves validity.
A failed save leaves the previously valid records intact. A successful save must be read back
by both the manager and the world loading path.

Automated checks are limited to the touched loader, database/schema operations, and validation.
Follow AGENTS.md for shared-schema checks. Do not build a new general test framework.

Client completion steps:
1. Load the original data and travel through the known crossings in both directions.
2. Save one deliberate valid portal arrival change in the manager.
3. Restart through the supervisor, cross that portal, and confirm the configured arrival.
4. Test the selected map's default entry through the appropriate default-entry path; a portal
   crossing alone does not prove the default spawn is read.
5. Restore original test data and verify it is loaded again.

Phase 1 is complete only when the screen, save, world consumption, and real-client path all work
without rebuilding Rust for the content edit. Backend-only work is partial. If client testing
cannot be performed in the assigned session, report it as pending and give the owner the exact
steps; do not mark the phase client-confirmed.

## Phase 2 - Independent running instances

### Goal and work

Separate a map definition from each running copy. Replace the assumption that map_id alone
identifies all mutable state. Carry the actual assigned instance through reservation, travel,
connection claim, and reconnect, retaining the measured client protocol.

Reuse existing registry, capacity, transfer, and token mechanisms. A map definition and path
graph can be shared; agent state, timers, encounter state, and dropped items belong to one copy.
Extend placement and leases where needed instead of creating an unrecorded auth/world side channel.

### Ask before implementing unresolved policies

- How is a party identified and which party assignment is authoritative?
- When does a party reuse an instance, get a fresh one, or retain one after disconnect?
- What happens when a party changes membership or leadership?
- What distinguishes a district assignment from a private explorable assignment?

First check existing decisions and captures; do not derive party identity from an account or
session just because that column is available. The intended solo-isolation example does not
authorize resetting unrelated public-district behavior.

### Completion and exclusions

Two solo parties can enter independent copies of the same map. Players assigned to the same
copy share its changing state. A reconnect claims its assigned copy under the chosen policy.
Ending an empty copy releases its transient state without touching the other copy.
Exercise capacity refusal and stale-assignment handling in the affected path.

No new phasing system, cross-world social system, or instance-migration service is included.
Do not mark party sharing complete if only the solo allocation path exists.

## Shared prerequisite - Lua before scripted simulation behavior

Embed Lua and establish the minimum script/server contract before the first phase that executes
scripted game behavior. Phase 3's NPC behavior, Phase 4's skill definitions, and Phase 5's HeroAI
must use this integration; Phase 6 extends it for quests rather than introducing Lua for the
first time. Build Rust mechanics as needed, but do not fill the dependency gap by hardcoding
behavior that belongs in scripts.

Resolve the relevant state queries, supported actions, event/callback ordering, actor/instance
scope, execution limits, and error handling before their dependent scripts. Load behavior from
the selected profile under docs/plans.md. The exact API and formats remain
open; this prerequisite does not select a coroutine model, reload policy, or tree syntax.

## Phase 3 - NPCs owned by the instance

### Goal and work

Add live agents independently of connection-oriented player slots. Extend the manager's content
model with only the definitions, placements, groups, and routes needed for the selected NPC.
Reuse existing pathfinding and movement; do not replace the accepted movement implementation.

The instance owns each agent's identity, position/plane, movement, and lifecycle. Shared
definitions describe appearance, faction, and assigned behavior; mutable state is separate per
agent and per instance. Introduce actor-relative nearby queries for later combat searches.

Use one owner updating an instance at a time and bounded workers across instances, consistent
with docs/02-architecture.md. Network/database waits must not block CPU simulation work.
Thread count is not the number of agents or instances.

### Ask before unresolved content or behavior

Select the first real NPC, its measured appearance and entry messages, placement, and patrol.
Ask if those are not specified or evidenced. Do not invent an NPC model, faction behavior,
route, roam radius, respawn rule, or server/client-visible flag to make a demonstration pass.

### Completion and exclusions

The selected NPC is content-defined, appears correctly to a real client, and walks the specified
route. All players in one instance see the same NPC state; another instance has independent
NPC state. Joining/leaving, NPC removal, and teardown do not leave ghost or shared agents.

This phase establishes appearance and movement. Combat, quest availability, loot generation,
and generic population/respawn policies are later work. A stationary model alone is not a
completed patrol.

## Phase 4 - Authoritative combat

### Goal and work

Build a small real combat loop before connecting general HeroAI decisions to it. Maintain
health, resources, effects, targets, attacks/casts, completion times, and death in Rust.
Player requests and Lua-authored NPC/skill behavior use the same relevant world operations.
The shared Lua prerequisite must be available before executing these scripted behaviors.

Choose the first evidenced attack and small set of skills. Implement server mechanics for
legality checks, resource use, timing, result execution, state updates, and outgoing messages.
Declare each skill's behavior in external scripts, following the owner's distinction in
`docs/plans.md`; the scripting contract is still open, so do not silently
replace it with hardcoded skill behavior. Build load and update values from the same state.
Replace the affected stand-ins as their owning systems arrive.

### Ask before unresolved rules

Ask which initial encounter/skills to reproduce if not already assigned. Obtain missing rules
and packet evidence through the repository's source order. Do not invent damage formulas,
cooldowns, regen, aggro/leash ranges, resurrection, rewards, or respawn timing.
Experience 500 and fixed health/energy values are evidence of existing stand-ins, not balance
defaults to propagate into the final simulation.

### Completion and exclusions

A player and NPC can perform the selected combat actions; resources and health change once at
the correct points; the real client sees consistent results; death changes authoritative state.
Verify the affected interrupted/invalid-action path where the selected action requires it.

Do not call the game's full combat/skill system implemented after this example. Do not add
unrequested balance controls or configure packet constants through the manager.

## Phase 5 - HeroAI policy on the Rust combat engine

### Goal and work

Express the decision logic reviewed in docs/plans.md in external Lua
scripts against explicit server actor state queries and actions. Rust provides mechanics and
execution machinery. Follow the pinned execution reference and mapping procedure in ADR 0014.
The intended reuse is skill selection, targeting, conditions, and coordination policy, not the
client runtime. Skill knowledge, use conditions, priorities, combinations, resource-management
decisions, targeting, coordination, and build-specific tactics belong in profile-owned Lua.
Game-specific tree composition must also be externally authorable.

Start with the generic path and selected skillbars whose actions are implemented. Identify
specialized build dispatch that the real selection uses; report unsupported branches explicitly.
Share immutable definitions and retain separate actor execution state. Avoid repeated skillbar
preparation when inputs have not changed and measure repeated target scans before claiming scale.

### Required compatibility checks

For each affected branch, record:
- Original node and callback behavior, including caller checks outside the BT.
- Exact BehaviorTree.CPP variant used.
- Changes in immediate completion versus the Python extra tick.
- Whether earlier priorities/guards are rechecked while another child runs.
- Completed parallel children, repeat failures, and reentry behavior where applicable.
- What halting a branch does to a requested move/cast and who owns that action afterward.

Ask about an adaptation that changes gameplay intent or observable reaction timing. Do not
silently preserve every Python quirk or silently replace the policy with a smarter one.
Source implementation details are not permission to give all enemy types the same tactics.

### Completion and exclusions

The selected NPCs choose and perform valid skills/targets through the Rust combat engine, with
the affected decision and node-lifecycle checks recorded. Client-visible behavior and packets
are checked on the selected encounter. Unsupported skills and builds are listed, not replaced
by success stubs. Client shared-memory/cache replacement is not listed as an unsolved obstacle.

Demonstrate a custom skill implemented with existing mechanics and its HeroAI usage rules in a
copied profile through Lua edits alone, with no Rust change/rebuild or change to the original
profile. Unsupported mechanics remain explicit engine work.

A Lua migration with a Rust runtime alone does not establish 20,000-agent capacity. The owner has not selected a global
AI frequency or authorized changing reaction speed to reach that figure.

## Phase 6 - Lua through a complete quest

### Goal and work

Extend the Lua integration already required by scripted NPCs, skills, and HeroAI with the Rust
operations needed by one selected quest. Settle the quest-specific event source/order, scope,
when requested actions apply, and behavior after a handler fails before implementing handlers.
Lua defines quest behavior; supported world mechanics execute in Rust.

The first path is quest giver -> acceptance -> selected destination or enemy objective ->
completion -> persistence. Add escort/follow sequencing after that path works.
Ordinary objectives and their relationships belong in the selected content representation;
Lua defines the quest decisions and sequences using world operations, including ordinary and
exceptional progression. Data can supply objective parameters; it does not move game-specific
quest behavior into Rust.

### Ask before unresolved scripting and quest policies

- Which real quest and objective are the first acceptance case?
- Are handlers the selected authoring model, or are suspended quest flows also required now?
- Which progress is character, party, or instance state, and who receives objective credit?
- Which party progress chooses a population/phase when members differ?
- What are the accepted error, execution-limit, reload, and permitted-access policies?

Do not treat Lua selection as approval of any particular API, JSON schema, event frequency,
coroutine model, or hot-reload behavior. If a dependency requires raising the repository's Rust
toolchain requirement, ask before doing that.

### Completion and exclusions

A real client can interact, accept, satisfy the objective, complete the quest, and retain its
required progress across logout. Repeating an event does not duplicate completion or rewards.
If this quest includes rewards, use the selected measured reward rules and existing state
operations. A fetch quest also needs the missing pickup/objective path, not just inventory rows.

No direct Lua implementation of packets or database writes replaces the Rust operations.
A full quest editor, branching campaign, general phasing system, and live script migration are
outside the first quest unless explicitly added by the owner.

## Phase 7 - Measured capacity

### Goal and work

Measure the populated simulation rather than extrapolating from empty maps or a language
microbenchmark. Test one 200-agent instance first, then concurrent instances toward 100.
Include dense target selection, varied skillbars, healing/effects, interrupts, movement,
and relevant quest events. Distinguish idle from all-active combat.

Record hardware/build, content and workload, decision/update rates, CPU, memory, update latency,
missed deadlines, path searches, and outgoing traffic. Record actual counts, not only a target.
Keep client-visible timing and the simulation/network rates distinct.

### Ask before unresolved acceptance targets

Ask which hardware and latency/headroom targets define success. No previous illustrative
10-decisions-per-second example is a chosen rate. Do not claim one machine must support the
entire workload, nor silently lower fidelity or freeze relevant agents to achieve it.

### Completion and exclusions

Publish reproducible measured capacity per world process/machine and identify the limiting
work. Validate representative gameplay with the real client. Report if 100 instances could not
be run and why; do not turn a projection into a measured result.

Do not build a general benchmark platform during Phase 1. Any load tooling here must serve
this workload and follow the test-scope rules for this milestone.

## Progress and reporting

All phases remain planned in this handoff. Mark a phase implemented, automated-checked, and
client-confirmed separately. Existing features that a phase reuses retain their own evidence;
the existence of a type, table, packet builder, or manager button does not prove the full path.

| Phase | Next visible result | Status |
|---|---|---|
| 1 | Edit the selected maps' spawns/portal arrivals through the manager | Planned |
| 2 | Independent solo copies and the selected party-sharing policy | Planned |
| Shared prerequisite | Profile-owned Lua behavior executes through Rust operations before scripted NPC/skill/AI work | Planned |
| 3 | A configured NPC walking its route | Planned |
| 4 | A player and NPC resolving the selected combat actions | Planned |
| 5 | NPC skill and target decisions driven by ported HeroAI policy | Planned |
| 6 | One quest playable through completion and saved progress | Planned |
| 7 | Measured capacity with populated instances | Planned |

At each handoff, report the exact completed work, changed files, one-line check result,
real-client result or pending client steps, and the next actual unresolved question.
Record decisions in the existing decision ledger; replace text that a decision supersedes.

Start tests with tools/test-touched.ps1. Run only the commands justified by the assigned change.
If its status scan includes pre-existing work, identify that before choosing test scope.
Follow AGENTS.md for schema/shared-machinery checks and whole-suite justification.
Run the ASCII check for written files. No unrelated cleanup or source/project dependency enters
a phase merely because the implementer is already editing nearby code.

## Starter instruction for the next implementation session

Read AGENTS.md and docs/HANDOFF.md, then docs/plans.md.
For control-panel work, also read docs/plans.md and implement only the panel
increment assigned by the owner; resolve its deployment/interface questions before dependent work.
For simulator Phase 1, implement configurable default spawns and existing portal arrivals for
the initial map 148/146 editing flow, with its panel editing interface. Preserve other content
and the current travel behavior during extraction. Check the Phase 1 decision gates.
Ask about unresolved requirements instead of selecting features; complete independent
authorized work while waiting. Report the real-client acceptance steps and their actual status.

## 20 - Rurik: the feature gap, and the port queue

> Moved here on 2026-11-06; it was the file `plans.md`.

This is an analysis, not a change. Nothing in this document is built yet. It answers one question:
**which capabilities does the Rurik checkout have that this tree does not, or handles worse, and in what
order should they be ported here.**

Governance is already settled: Rurik is a recorded source (`docs/adr/0011-rurik-is-a-source.md`). That
ADR decides five things which shape every row below, and none of them is re-opened here:

1. Rurik is read, trusted where it cites an instruction address or a capture, and its findings are
   restated in this project's own words with this project's own evidence.
2. It is never a dependency. Nothing in `Cargo.toml`, nothing under `crates/`, no test requires the
   checkout. `cargo build --workspace` succeeds on a machine without it, and the same holds for Python:
   **no Python crosses into this tree**. Server mechanics are implemented in Rust; skill behavior
   is declared in external Lua scripts, as clarified in `docs/plans.md`.
3. Its vocabulary stays out. A module is named after the game's noun; no file under `crates/` describes
   itself as coming from Rurik.
4. Every fact taken from it is restated with our evidence: a recording under `recordings/`, a crash log,
   a test. Where Rurik is the only witness, the sentence says that.
5. A conflict resolves in favour of the client in the owner's hands.

## The client link: Py4GW Stealth (`docs/adr/0013-py4gw-stealth-is-our-link-to-the-client.md`)

Before any row below is read, one capability is not Rurik's and is not in doubt: **Py4GW Stealth is this
project's link to the client.** It injects a 4,736-byte bridge into a running `Gw.exe`, detours
`game_thread.leave_game_thread_func`, keeps a client-to-Python event ring, and resolves a catalogue of
28 subsystem offset files by byte pattern and by assert anchors. Two entries in that catalogue decide
the comparison in this document:

- **`gw_dat_reader.json`** reads the archive through the client's own open-file, image and `AMET` code.
  The client's parser answers, not ours.
- **`stoc.json`, `ctos.json`, `packet_sniffer.json`** resolve the server-to-client handler table pointer
  and the client-to-server send wrapper. That is the sniffer.

So **every row here that says "read this out of the running client" is answered by Py4GW Stealth, not by
Rurik**, and those rows are struck from the queue: the skill, attribute and area tables, string-to-text
resolution, the item modifier word, skill template strings, a chunk's contents read live, and packet
observation through the client's own tables. Rurik's versions of those are a cross-check at best. P14
keeps only the half a server needs with no client anywhere, and P7 only what a server must compose.

What remains is the narrow set that needs the client **closed**, which the catalogue does not touch: a
grep of all 28 offset files for a message format or field table returns nothing. Those four survive in
P2 below.

## Scope ruling: world simulation is a later stage

The owner's direction, recorded here because it decides what this document is for: **all world
simulation belongs to a later stage, and Rurik's simulation work is the source for that stage when it is
scheduled.** It is not the source for now.

`docs/adr/0003-simulation-in-scope.md` defines simulation as agents, movement, pathfinding, combat and
drops. Against that definition these rows are **deferred**. They stay listed so that nothing found in the
Rurik checkout is lost, and they are marked `DEFERRED` at their headings:

| row | what it is |
|---|---|
| P8 | non-player agents |
| P9 | the combat core |
| P10 | the skill substrate |
| P11 | party: henchmen and heroes |
| P12 | quests |
| P13 | progression persistence: attributes, ranks, unlocks, morale |

Every other row is not simulation - measuring tools, the archive, content authoring, map authoring, the
harness, the build discipline - and **that is the live queue**.

Movement and pathfinding are on ADR 0003's simulation list and are already built here
(`crates/asura-world/src/agent.rs`, `pathfind.rs`), so P3 measures what exists rather than porting
anything into it.

## The size of the two trees

| | Rurik | ASURA |
|---|---|---|
| language | Python 3, stdlib only | Rust |
| code | 732 `.py`, ~390,000 lines | 88 `.rs`, 38,585 lines (measured 2026-11-01) |
| research documents | 219 `.md`, 16.5 MB (`studies/`, 71 arcs) | 33 pages under `docs/`, 14 ADRs |
| server | one process: `toolkit/authsrv/authsrv.py`, 50,110 lines | 7 crates, tokio, PostgreSQL |
| world content | 11 declarative `.toml` tables, 480 KB | SQL migrations, `crates/asura-db/migrations/` |
| schema | 777 messages (`schema/messages.json`: AUTH_CMSG 57, AUTH_SMSG 39, GAME_CMSG 194, GAME_SMSG 487) | 782 rows, `crates/asura-protocol/src/packets.rs` (auth 60 + 40, game 194 + 488) |

The two schemas are the same catalogue at the same coverage, and that is the first finding: **the
message schema is not a gap**. Both descend from the same public table, Rurik validated its copy
field-for-field against build 38797 and recorded zero residual disagreement, and this tree's 782 rows
carry the same layouts. A port of the schema buys nothing; the gaps are elsewhere.

## What is already equal, and must not be ported

Reflex is the enemy here, so the equal list comes first. These are places the two trees do the same
thing and this one is not behind:

| area | where |
|---|---|
| auth handshake, ARC4, fixed-constant DH, file check, login burst | `crates/asura-protocol/src/client/handshake.rs`, `client/`, answered by `crates/asura-auth`, proven over a socket by `crates/asura-auth/tests/login.rs` |
| travel, placement, atomic reservation, leases, reaper | `crates/asura-db/src/placement.rs`, `crates/asura-auth/src/travel.rs` |
| instance load block, spawn point, spawn block, manifest | `crates/asura-world/src/map.rs`, `tests/load.rs` |
| path graph import and A\* over the trapezoid graph | `crates/asura-world/src/map_data.rs`, `pathfind.rs`, with the reference's own reciprocal/square-root tables |
| movement budget clamp, plane-change rules, cancel correction | `crates/asura-world/src/agent.rs` |
| the exploration grid (`0x008B` + `0x008A`, RLE bands, empty-band shape) | `crates/asura-world/src/cartography.rs` - and Rurik's `content/fog.toml` is what settled its position in the load block |
| chat channel split (`!`/`#`/`$`), `/stuck`, `/maps`, no server-generated chat on the wire | `crates/asura-world/src/chat.rs`, `docs/history.md` |
| inventory as rows, drop with the stack deducted, move, equip, position round trip | `crates/asura-world/src/item.rs`, `crates/asura-db/src/inventory.rs` |
| multi-player maps: introduction, withdrawal, broadcast | `crates/asura-world/src/instance.rs` (`PeerView`, `introduction`), `crates/asura-world/src/world.rs:166` |
| Gw.dat index and the FaCompress decompressor | `crates/asura-world/src/gwdat.rs`, `fa_compress.rs` |

Two of these are places this tree is ahead and the port must not regress: the crate split (Rurik's
server is one 50,110-line module) and the database (Rurik keeps characters in one JSON file per account,
`toolkit/authsrv/charstore.py`).

## The four kinds of gap

Every row of the queue is one of these. The distinction matters because they cost different things and
carry different risk:

- **B - behaviour gap.** Rurik sends or answers a message this tree does not. Changes what a client
  sees, needs a client run to accept.
- **I - instrument gap.** Rurik can measure something this tree cannot. Changes no behaviour, so it is
  safe to land unattended, and it shortens every later step.
- **A - authoring gap.** Rurik can write an artifact this tree can only read. Largest capability, least
  bearing on the packet stream.
- **H - handling gap.** Both trees do it; Rurik's arrangement is better and worth replacing ours.

## The port queue

Ordered by dependency and by value against the objective in `AGENTS.md` #1 (reproduce the packet stream,
name every packet), not by size. Each row gives what Rurik has, what this tree has, what a port means
here, what must not come along, and what accepts it.

### Phase 1 - instruments. No behaviour change, no client risk.

**P1 (I) - The opcode sweep: measure every server-to-client message the live server never showed us.**
Rurik: `toolkit/authsrv/smsgsweep.py`, `sweeploop.py`, `shotloop.py`, `shotlabel.py`; result recorded in
`studies/smsgsweep/FINDINGS.md` - 334 of 487 GAME_SMSG opcodes measured against a controlled client,
**82 ASSERTED, 3 FAULTED, 1 DROPPED_CHANNEL, 3 REPLIED, 235 SILENT**, every crash row paired with the
dialog its own run captured.
Here: nothing. `docs/history.md` reports 32 of the reference's client-message handlers present and
says nothing about the 488 server-to-client rows this tree has never sent.
A port: a tool under `tools/` that sends one catalogued opcode to a client bound to loopback, reads what
comes back, and records a verdict per opcode; the verdict table committed as a recording, not as prose.
**The two halves are different instruments**, and only one of them is ours to write: the *observation*
half is the client link's sniffer (`stoc.json`/`ctos.json`/`packet_sniffer.json`, ADR 0013); the
*stimulus* half - put one unknown message on the wire and read the client's reaction - is this row.
Not to take: the screenshot half before the cheap half proves itself, and any opcode fired inside a
client's own load traffic.
Accepts it: one row per opcode in `packets.rs` moves off `Known` with a recording named as its evidence.

**P2 (I) - The four questions that need the client closed.**
Rurik: `toolkit/clientscan/` - `msgshape.py` (a message's wire shape decoded from the client's own format
tables), `asserts.py` (the client's assertion strings read as a catalogue with file and line),
`pinned.py` and `buildid.py` (which build a byte pattern belongs to), `msghandler.py` (disassembles the
client's handler for one message), and `sigcorpus.py` (whether a byte-shape anchor still resolves).
Here: nothing.
Why these four and not the other six: **Py4GW Stealth answers everything that needs the client running**
(see "The client link" above). A message's wire shape, the assert table as a list, the build a pattern
was measured against, and one handler's disassembly are the questions the offsets catalogue does not ask
and cannot: grep all 28 of its files for a format or field table and nothing comes back.
A port: four readers behind `tools/`, each printing bytes rather than claims. `msgshape` and `asserts`
pay first - one gives a layout without a capture, the other is how twenty opcodes got named here.
Not to take: `capstone` or `pefile` as a build dependency of anything under `crates/` - a reader lives in
`tools/` and fails closed when the client is absent (`AGENTS.md` #6).
Accepts it: a number this tree currently guesses is printed by a tool from the owner's own `Gw.exe`, and
the guess is corrected or confirmed in the same change.

**P3 (I) - Movement-fidelity instruments.**
Rurik: the arc's instruments are `toolkit/clientscan/movesync.py` (separation between the client's own
position and the agent the server drives, with a two-armed hard bar), `movetap.py` (the client's live
movement solution), `noclipscore.py` (off-mesh walking and hole-crossing chords), `planecensus.py`,
`warpscan.py` (did the run test the teleport, and did it warp), `grantsim.py`/`resyncscore.py` (would a
different grant policy have snapped), and on the server side `toolkit/authsrv/leadgeom.py`,
`agtrack_mirror.py`, `speedwords.py`, `cancelwalk.py`, `castpolicy.py`.
Here: four scripts - `tools/analyze-live-move.py`, `analyze-live-move-math.py`, `live-client-speed.py`,
`window-live-move.py`.
Why this one is high: movement is where this tree's own incident ledger failed, six times
(`docs/HANDOFF.md`, rows 2 to 6). Every one of those was a change made without an instrument that could
say "the client and the server now disagree by this many units".
A port: `movesync`'s separation score and `warpscan`'s did-it-actually-warp verdict, over the recordings
already committed.
Not to take: Rurik's conclusions about the snap gate as *our* rule. Its finding is that no gate of the
client's test is a server lever; that is a fact to cite, not a design to adopt.
Accepts it: the score printed against an existing recording, agreeing with what that recording's own
trace says.

**P4 (I) - The tape player.**
Rurik: `toolkit/authsrv/tape.py` plus `studies/tape/FINDINGS.md`; R1.5 played 1,209 of 1,209 events of a
48.6 s recorded stream at recorded timing and drove a real client through Ascalon City, and a second run
rendered combat.
Here: `crates/asura-world/src/replay.rs` serves the reference's captured bytes per block - a bisection
aid, not a timed replay.
A port: replay a committed `recordings/` stream at recorded timing with zero messages of our own on the
channel. Rurik's own first run failed because 373 of its own `WORLD_SIMULATION_TICK`s were interleaved,
which is the defect to design out first.
Accepts it: a recording replays end to end with a measured count of non-tape sends, and that count is 0.

### Phase 2 - items, which is where this tree's next session already starts

`docs/HANDOFF.md` "Where the next session starts" item 3 lists five item behaviours as the next work.
Three of them have a Rurik source.

**P5 (B) - Pick a dropped item back up.**
Rurik: `toolkit/authsrv/loot.py` (ground drops and the pickup, `RANGERPRE-S15`) and `purse.py` (carried
gold).
Here: `docs/HANDOFF.md` records the decision "a dropped item becomes litter", taken because the reference
has no pick-up handler.
A port: `GAME_CMSG_INTERACT_ITEM` (`0x803F`) decoded and answered, so a drop is reversible.
Accepts it: drop an item, walk to it, pick it up, relog, and the item is still in the bag.

**P6 (B) - Buy and sell.**
Rurik: `toolkit/authsrv/merchant.py` (the purchase recipe and the gold/inventory messages it produces),
`toolkit/authsrv/probemerchant.py` (merchant window, stock declaration, prices, the accumulator drains).
Here: nothing; `GAME_CMSG_TRANSACT_ITEMS` (`0x804D`) is in the table and unhandled.
A port: the merchant window, the stock rows, and the gold/inventory pair a purchase produces.
Accepts it: a buy and a sell over a socket, with gold and the bag both correct afterwards.

**P7 (H) - Item modifiers and weapon sets.**
Rurik: `toolkit/clientscan/itemmods.py` decoded the client's item modifier word -
`{identifier: bits 29-20, arg: bits 17-8, arg2: bits 7-0}` over 157 identifier slots, validated on
5,266 of 5,266 real words - and a composed word was rendered by the retail client as
`Hammer Mastery +1 (Stacking)`. Also `visible in toolkit/authsrv/authsrv.py`: `configure_weapon_sets`,
`declare_weapon_sets`, `select_weapon_set`, `worn_armour`.
Here: items are rows (`asura_db::inventory::Item`) whose stats are copied from the reference's capture.
Handling, not a hole: the rows work, and the block is byte-equal to the capture.
**Narrowed by ADR 0013:** the word's *layout* is answered live by the client link, and Py4GW's own tree
ships a modifier parser (`Py4GWCoreLib/item_mods_src/item_modifier_parser.py`), so the decode is not
something to port. What stays is what a *server* must do with it: compose a word, send it, and see the
client render it.
A port: composing the modifier word rather than copying one, plus the four weapon-set messages.
Not to take: the 157 identifier meanings. Rurik records that bits 31, 30 and 19 are a fixed
per-identifier prefix that must be copied, not computed, and that meaning is recovered per identifier.
Accepts it: a composed modifier word renders in a live client, and the rendering is read back through
the client link rather than through a screenshot.

### Phase 3 - DEFERRED. World simulation: bodies (P8 to P11).

`docs/history.md`: "There are no NPCs, no combat and no quests". This is the largest behaviour gap and
it is five ports deep.

**P8 (B) - DEFERRED. Non-player agents.**
Rurik: `toolkit/authsrv/agents.py` ("what the client must be told to render one"), `npcdefs.py` (a capture
compiled into NPC definition rows), `npcaftercast.py`, `content/npcs.toml` (504 lines), and the spawn rows
in `content/world.toml`.
Here: `world_create_agent` exists in `crates/asura-protocol/src/game/mod.rs` and is called for players
only.
A port: a spawn table that yields agents that are not the player, with model, allegiance, level and
weapon, and the create burst around them.
Accepts it: an agent that is not the player stands in a map, in a live client, and does not move.

**P9 (B) - DEFERRED. Combat core: an attack that lands, and a death.**
Rurik: `toolkit/authsrv/combatmath.py` (armour and the swing arithmetic), and in `authsrv.py` the
dispatch for `0x0026` `ATTACK_AGENT`, the kill window as three messages - death, reward, and the attack
reply - plus revive. Ladder row R4a in `PLAN.md` section 3 records it working on the wire and on film, 2026-08-11,
including the crash that a wrong revive fraction caused (`CharPool.cpp:84`, `fraction <= 1.0f`).
Here: nothing.
A port: `GAME_CMSG_ATTACK_AGENT` handled, a swing resolved against the armour arithmetic, a death and a
revive, and the target re-acquired.
Not to take: Rurik's admission that `ENEMY_SKILL_FRACTION = 0.25` was invented - the skill ladder records
that constant as removed, and nothing invented goes in here.
Accepts it: a live client clicks an agent, the server drives the fight to a kill, and the client revives
it.

**P10 (B) - DEFERRED. The skill substrate.**
Rurik: the ordinary recharging cast lifecycle - `0x00E4 -> 0x00E5 -> 0x00E3 -> 0x00E6` -
and the **queue law**: a press during aftercast schedules from the aftercast's end. Damage is the skill's
scale endpoints interpolated by the client's own formula (`0x005A8920`:
`max(0, round(lo + (hi-lo)*rank/15.0))`). Hexes and enchantments came off the missing list 2026-08-20 via
`toolkit/authsrv/effects.py` (`0x0042` opens an episode, `0x0044` closes it), `hexjoin.py`, `bufflog.py`.
Energy and adrenaline are `toolkit/authsrv/pools.py`, `adrenjoin.py`, `adrenreplay.py`. Conditions from
`studies/isle/FINDINGS.md` (478 Bleeding .. 486 Weakness, +2077).
Here: `skillbar_update` and `update_unlocked_skills` exist as builders; no cast is answered.
A port: the evidenced cast lifecycle including its exceptions, the queue law, the scaling formula,
then conditions, then the effect episode pair. The pinned source review in
`docs/plans.md` records zero-recharge exceptions, the queue divergence,
and the boundary between mechanics and scripted skill behavior.
Not to take: `type_code` as a discriminator for what a magnitude means - Rurik records that it cannot
discriminate, and that an unmodelled skill resolves to None rather than 0.
Accepts it: a skill on the bar casts, lands on a target, and a second press inside the aftercast is
queued rather than dropped - each measured, not asserted.

**P11 (B) - DEFERRED. Party: henchmen and heroes.**
Rurik: `henchparty.py` (`0x01BF` PARTY_HENCHMAN_ADD `[u16,u16,string16(20),u8,u8]`, `0x01C2`
PARTY_HERO_ADD), `henchjoin.py`, `herolib.py` (a hero's usable library and what may go in a slot),
`heroqueue.py`, and `heroes_table.py`. The slice ladder (R-SLICE, landed 2026-09-13) has a Monk hero that
walks the formation, fights the leader's target, obeys stance/lock/flag, and crosses a transfer.
Here: `party_create`, `party_player_add`, `party_member_stream_end` - one player, one party.
A port: the two party-add messages, a hero row with its own body, bar and ranks, and formation walking.
Accepts it: a hero walks behind the player across a map transition and fights the player's target.

### Phase 4 - DEFERRED (P12 and P13), live (P14): the world beyond bodies

**P12 (B) - DEFERRED. Quests.**
Rurik: `toolkit/authsrv/questdefs.py` (quest rows out of `content/`, and the coded string a field goes on
as), `content/quests.toml`, `quests.toml`'s schema mirrored in `toolkit/authsrv/authsrv.py`
(`accept_quest`, `turn_in_quest`, `grant_quest_reward`, `complete_objective`,
`kill_completes_objective`, `_quest_markers`, `_quest_screen`, `_dialog_window`), `probequest.py`,
`studies/quests/`.
Here: nothing.
A port: the dialog window, accept, objectives, completion, reward, and the marker messages.
Not to take: quest prose. Rurik's own provenance scoping
(`PLAN.md` section 7 Q17) distinguishes an id resolved at run time from an authored body of text, and authored
text is the one thing neither tree may copy.
Accepts it: take a quest from a stationary NPC, complete its objective by killing an agent, turn it in,
and the reward arrives.

**P13 (B/H) - DEFERRED. Progression persistence.**
Rurik: `charstore.py`, `summarycensus.py` (`charsummary.py` for the summary blob), `attribspend.py` (the
rank and point model and the client's own refusal rules), `attribpoints.py`, `skillunlock.py` (two
unlocked-skill bitmaps, on two scopes), `morale.py` (`0x00EE` attr 10 and `0x009C`).
Here: gold, level, both professions and the campaign are columns now (`0008_character_traits.sql`,
`0009_character_settings.sql`), and the instance unlocks are `instance_unlock` rows
(`0023_instance_unlocks.sql`). Attributes, ranks and the unlocked-skill bitmaps are not stored.
Rurik's storage is the one thing not to take: one JSON file per account is a design this tree replaced
with PostgreSQL rows.
A port: attributes and ranks as columns with the reference's point model, and the unlocked-skill bitmaps.
Accepts it: an attribute is spent in a live client, the ranks are right, and a relog keeps them.

**P14 (A) - The archive's other chunks: only the half a server needs with no client.**
Rurik: `toolkit/mapdata/terrain.py` (the Bloated terrain chunk, read and write, 349 of 349 maps),
`strippedterrain.py`, `pathchunk.py`, `props.py`, `envchunk.py`, `soundchunk.py`, `trnblend.py`,
`trnvariation.py`, `trnshadow.py`, `fogrle.py` (the fog-of-war RLE, decode and build), and
`toolkit/clientscan/worldmap.py` (the atlas: which archive file holds each 512x512 continent tile) plus
`tilerender.py`.
Here: `map_data.rs` reads exactly one chunk - `ChunkId::Path` (`map_data.rs:90`). Terrain, zones, props,
water, mission, environment, locations, light and shore are enumerated in the `ChunkId` enum at
`map_data.rs:69` and never read.
**Narrowed by ADR 0013:** what a chunk *contains* is now a question for the client link, which reads the
archive through the client's own open-file, image and `AMET` code (`gw_dat_reader.json`). Rurik's
readers are the cross-check. What stays is the half that must work with no client anywhere: a world
server reads geometry at start-up, and a `Gw.dat` we author must be accepted by the client's parser.
A port: the terrain height field and the props list offline, then the fog RLE, then the atlas.
Accepts it: an authored or retail map's own height at a chosen position, read offline, agrees with what
the client reports for ground Z through the client link.

### Phase 5 - authoring. The largest capability, and the least about the packet stream.

**P15 (A) - Declarative content with per-row provenance.**
Rurik: `toolkit/content.py` (559 lines) loading `content/*.toml`: `maps.toml` 840 lines, `npcs.toml` 504,
`items.toml` 933, `areas.toml` 1,130, `world.toml` 3,790, `fog.toml`, `quests.toml`, `drops.toml`,
`partycap.toml`, `movecode.toml`, `compose.toml`. Every row carries its own `provenance` table, and the
loader **refuses** a `source = "client-table"` row that does not name its extractor and its build.
Here: SQL migrations (`crates/asura-db/migrations/`, 24 files) plus two seed files the supervisor applies
(`tools/map-config.sql`, `tools/seed-dev.sql`); content literals are kept as data, and the reference's own
map list is generated into `map_config`.
Handling, not a hole: migrations are versioned, reversible and tested, which a TOML file is not.
A port: **the provenance column and the loader's refusal, not the file format.** A content row that
cannot name where it came from is the defect this tree's incident ledger is full of.
Accepts it: a content row added without a source is refused at startup, with the refusal tested.

**P16 (A) - Gw.dat write, reversibly.**
Rurik: `datplan.py` (compute every byte that would change; change none), `datwrite.py`, `datdelta.py`,
`datjournal.py` (append-only record of what the writer did), `datsnapshot.py`, `datcheck.py`,
`rowbytes.py`, `refindex.py` (who else reads this id, asked before a write), `datalloc.py`,
`datmove.py`, `overlay.py` and `overlayrefgate.py` (declared, reversible, identifiable-from-bytes archive
profiles), `compose.py`, and the compression **encoder** `gwenc.py`/`gwmatch.py`/`gwentropy.py`.
Here: `gwdat.rs` is the index half by its own doc comment, and `fa_compress.rs` is the decompressor.
Nothing writes.
A port: plan-diff-apply with a journal, then the encoder, then the overlay profile.
Not to take: any writer that cannot be reversed from its own journal. Rurik's `datwrite` refuses on
purpose for a move that does not fit, and `datmove` is the separate verb that does it.
Accepts it: an entry is replaced and then restored byte-identically, with the restoration proven by hash
and not by intent.

**P17 (A) - Author a map the client will load.**
Rurik: `mapbuild.py` (a whole Bloated map from typed parameters - 7,841 bytes, 9 chunks, 97.04%
generated), `stripbuild.py`, `mapgen.py`, `deploy.py` (one command from `content/` to a map you can walk
around), `deployrefuse.py`; and `mapscale.py` for how big an authored area may be.
Here: none. `map_config` rows can name a map; nothing can make one.
A port: last. It is the only row here whose value is a mod kit rather than the live packet stream, and
it depends on P16 and on P14's chunk readers existing first.
Accepts it: an authored map loads in a live client, and geometry we chose confines the character.

### Phase 6 - discipline. Cheap, and it is what keeps the rest honest.

**P18 (H) - A harness run that judges itself from the client's own messages.**
Rurik: `toolkit/harness/session.py` - one command that pre-flights the ports, starts the stack, proves
each service owns its port, launches the client, drives login through EULA to Play, and prints one
`[PASS]`/`[FAIL]` per checkpoint; plus `portclaim.py`, `read_error_dialog.py` (the full text out of the
client's fatal-error dialog), `runwatch.py`, `progress.py`, `runladder.py`.
Here: `tools/test-battery.ps1`, `tools/run-client-login.ps1`, `ASURA_TRACE=1`.
A port: the checkpoint ladder and the dialog reader. The dialog reader is the single highest-value item
in this phase: three messages that no capture contains have already crashed a live client
(`docs/HANDOFF.md`), and today the crash text is read by hand.
Accepts it: one command prints a per-checkpoint verdict for a login, and a client crash produces the
assertion text without a human looking at the screen.

**P19 (H) - A run that measured nothing fails.**
Rurik: `toolkit/checks.py` (every verdict routes through one ledger and declares a `floor`; fewer checks
than the floor is a FAIL naming the shortfall) and `run_suite.py` (discovers tests from disk, never from a
document). It exists because `test_codec.py` printed ALL CHECKS PASSED with its fixture glob matching
nothing.
Here: `tools/test-touched.ps1 -WholeSuite -Justification "..."` for scope, and no floor mechanism.
A port: the floor, applied to the standing suites.
Accepts it: deleting a fixture makes the suite red with a named shortfall, not green.

**P20 (H) - Client-build discipline.**
Rurik: `toolkit/clientpatch/dhbuild.py` reads the DH struct and answers `ours`/`stock`/`unknown`, and
`cage.py`'s `assert_launch_safe(exe, host)` refuses a launch whose parameters do not match the server it
points at. It exists because two tools once selected a build by filename and picked the wrong one.
`snapshot_client.py` and `updatecheck.py` handle the fact that the client's DH parameters rotate with
every ArenaNet build, and `buildpins.py` pins which build every byte pattern was measured against.
Here: `client-patcher/patch-gw.py` and `keys.txt`. No classification, no launch gate, no build pin on the
tooling.
A port: the classification and the gate. Both are small and both prevent a whole class of wasted session.
Not to take: the live-capture policy. Rurik's authorization to run automation against ArenaNet on a
secondary account (`CLAUDE.md`, owner's decision 2026-08-06) is Rurik's owner's decision about Rurik's
account. It is not ours to inherit, and this tree's recordings stay what they are: captures of our own
server and of the reference.
Accepts it: pointing a stock-DH client at this server is refused before the process starts, with a named
reason.

## What must not be ported, and why

- **`authsrv.py`.** 50,110 lines in one module. The crate split is not the thing to give up.
- **Any Python.** `AGENTS.md` #6 and ADR 0011 #2. A port is a restatement.
- **Rurik's provenance gate as written.** Rurik refuses to hold any ArenaNet bytes, and its `.gitignore`
  rationale is built on that. This tree commits decrypted captures under `recordings/` as its evidence,
  and that is this tree's decision; adopting Rurik's gate would delete the evidence base. Read its
  *method* - measurement is permitted, expression is not, every row names its extractor and build - and
  keep our own rule for what may be committed.
- **Its monster-AI conclusions.** `PLAN.md` section 3's R4c row records AI as **not recoverable** from the client,
  the wire or any capture campaign: 0 of 937 embedded source paths under any `\Srv\` tree, 33 AI-adjacent
  searches over two independent routes, all zero. That is a finding to cite, not a design to port. Do not
  put AI on this queue until someone has a source neither tree has.
- **Its ladder as our ladder.** Rurik's status table is authoritative for Rurik. This tree's milestones
  live in `MILESTONES.md` and `docs/roadmap.md`.
- **Its vocabulary.** `crates/` keeps naming things after the game's nouns. `PathGraphs`, not Rurik's
  spelling of the same shape.

## What Rurik itself calls open, so a port must not promise it

- No AI, no pathing for non-player bodies, no resurrection shrine, no agent table (R4a and R4c rows).
- Conditions, energy, adrenaline and every non-damage mechanic were still listed as absent in R4b at the
  time its row was written; the effect substrate (hexes, enchantments) landed after that.
- Skill *meaning* is per-skill and sourced from outside the client's table; an unmodelled skill resolves
  to nothing.
- The vertical slice and the sandbox are outposts, quests and a corridor - not a campaign.

A port list that reads as "Rurik has everything" is wrong, and the rows above are the boundary.

## The branch taken, and the order, in one line each

- **Instruments before behaviour** (P1 to P4), because they carry no client risk, they land unattended, and
  every later port is accepted by a measurement that already exists.
- **Items before bodies** (P5 to P7), because `docs/HANDOFF.md` already names the item behaviours as the
  next work, and three of the five have a Rurik source.
- **Simulation is not in this queue** (P8 to P13). It is a later stage by the owner's direction, and the rows are kept only as a record of what Rurik holds for it.
- **Authoring last** (P15 to P17), because it is a mod kit, and the objective is the packet stream.
- **Discipline alongside** (P18 to P20), because the dialog reader and the floor are the two that stop a
  session being wasted.

## What this document does not decide

It does not change a single byte of this tree. Every row above still needs, before its first edit, the
one-sentence pre-flight `AGENTS.md` #2 requires: what changes, in which message or code path, and why it
is not simply copying the reference. Rurik is a source, not an instruction.

## 21 - The surrounding packets: what a live stream carries that this tree ignores

> Moved here on 2026-11-06; it was the file `plans.md`.

Analysis, and partly a change: the first ranked item below (the map table, `0x01A0`) and the portal
touch are in the tree now; the population and the remaining names are not (`docs/22-world-model.md`,
`docs/plans.md`).

The owner's direction, 2026-10-30:

> "there seem to be a lot of information that is pushed on account handling and other on map handling
> and login ... all this issues we have been having are because when we try to add a functionality, we
> ignore the surrounding non important packets, but those packets are the ones that we need to identify
> and put to use now"

The cartography work is the proof he is describing: the mission map and the world map did nothing, no
client message appeared, and the cause was a message earlier in the load that we never sent
(`docs/03-protocol.md` 11.4, `crates/asura-world/src/cartography.rs`). The rule this document produces:
**before a feature is written, inventory every message of the flow it lives in, and name the ones we are
not sending.**

## Method

Two sides are joined here, and only measured numbers appear:

- **the client's view of a live server**, from `recordings/client/` (`recordings/index.jsonl`), produced
  by `tools/sniff-capture.py`;
- **our side**, from `logs/world-8001.log` (the trace in `logs/`, not the console:
  `crates/asura-supervisor/src/process.rs:41`) and `crates/asura-protocol/src/packets.rs` for the names and
  the status column.

**The sniffer reports the client's struct sizes, not the wire sizes** (`docs/03-protocol.md` 5e), so the
comparison below is message by message and never size against size. A live row that is 2 bytes larger
than ours is the four-byte header; one that is 17 larger (`0x0020`) is struct padding.

## Flow A - entering a map (live capture of map 148)

`recordings/client/feature/maps/pre-searing-load-3.jsonl` (map 148, `0x019A` carries `0x94`) against a
session of our world on the same map (`logs/world-8001.log`, from the `== world-8001: started ==` at
line 10797; our counts cover the whole session, which includes ten minutes of standing still).

| header | our name | our status | live | ours | verdict |
|---|---|---|---|---|---|
| `0x000C` | `PING_REQUEST` | Emitted | 3 | 78 | 1:1 |
| `0x000D` | `PING_REPLY` | Emitted | 3 | 1 | 1:1 |
| `0x0018` | `PVP_UPDATE_UNLOCKED_HEROES` | Emitted | 1 | 1 | 1:1 |
| `0x001A` | `PVP_ITEM_STREAM_ADD` | Emitted | 62 | 387 | 1:1 |
| `0x001D` | `PVP_UPDATE_UNLOCKED_SKILLS` | Emitted | 1 | 1 | 1:1 |
| `0x001E` | `WORLD_SIMULATION_TICK` | Emitted | 492 | 3859 | 1:1 |
| `0x0020` | `WORLD_CREATE_AGENT` | Emitted | 42 | 1 | **population missing** |
| `0x0021` | `WORLD_REMOVE_AGENT` | Emitted | 1 | 0 | never sent |
| `0x0025` | `AGENT_UPDATE_DIRECTION` | Emitted | 2 | 0 | never sent |
| `0x0029` | `AGENT_MOVE_TO_POINT` | Emitted | 35 | 0 | not sent for other agents |
| `0x002A` | `AGENT_UPDATE_DESTINATION` | Known | 1 | 0 | never sent |
| `0x002B` | `AGENT_UPDATE_SPEED` | Emitted | 1 | 0 | never sent here |
| `0x002E` | `AGENT_UPDATE_ROTATION` | Known | 9 | 0 | never sent |
| `0x003C` | `GAME_SMSG_PLAYER_UPDATE_FLAGS` | Emitted | 29 | 1 | sent once per load, the player's own flags (`crates/asura-world/src/player.rs`) |
| `0x003E` | `AGENT_ALLY_DESTROY` | Known | 1 | 0 | never sent |
| `0x0048` | `AGENT_DISPLAY_CAPE` | Emitted | 23 | 1 | only for the player |
| `0x004C` | `QUEST_DESCRIPTION` | Known | 1 | 0 | never sent |
| `0x0053` | `QUEST_ADD_MARKER` | Known | 1 | 0 | never sent |
| `0x0056` | `NPC_UPDATE_PROPERTIES` | Known | 14 | 0 | never sent |
| `0x0057` | `NPC_UPDATE_MODEL` | Known | 13 | 0 | never sent |
| `0x0059` | `PLAYER_CREATE` | Emitted | 25 | 1 | only for the player |
| `0x005A` | `PLAYER_DESTROY` | Emitted | 1 | 0 | never sent |
| `0x006B` | `POST_PROCESS` | Known | 1 | 0 | never sent |
| `0x006D` | `NPC_UPDATE_WEAPONS` | Known | 19 | 0 | never sent |
| `0x006E` | `UPDATE_AGENT_VISUAL_EQUIPMENT` | Emitted | 23 | 1 | only for the player |
| `0x009A` | `AGENT_UPDATE_SCALE` | Known | 10 | 0 | never sent |
| `0x009B` | `AGENT_UPDATE_NPC_NAME` | Known | 22 | 0 | never sent |
| `0x009F` | `AGENT_PROPERTY_UPDATE_INT` | Emitted | 82 | 3 | only for the player |
| `0x00A2` | `AGENT_PROPERTY_UPDATE_FLOAT` | Emitted | 4 | 1 | only for the player |
| `0x00A6` | `AGENT_UPDATE_PROFESSION` | Known | 31 | 0 | never sent |
| `0x00B0` | `PLAYER_UPDATE_PARTY_SIZE` | Emitted | 30 | 0 | sent only on a join |
| `0x00B1` | `PLAYER_UPDATE_PARTY` | Emitted | 29 | 0 | sent only on a join |
| `0x00F0` | `AGENT_INITIAL_EFFECTS` | Emitted | 42 | 1 | only for the player |
| `0x00F3` | `TITLE_RANK_DATA` | Known | 3 | 0 | never sent |
| `0x00F4` | `TITLE_RANK_DISPLAY` | Known | 8 | 0 | never sent |
| `0x0118` | `GUILD_PLAYER_ROLE` | Known | 1 | 0 | never sent |
| `0x011F` | **unnamed** | Unnamed | 20 | 0 | **not named, never sent** |
| `0x0121` | `GUILD_GENERAL_INFO` | Known | 14 | 0 | never sent |
| `0x012E` | `GAME_SMSG_LOCATION_TAG` | Inferred | 1 | 0 | named from the source reading, never sent |
| `0x012F` | **unnamed** | Unnamed | 1 | 0 | **not named, never sent** |
| `0x013A` | `ITEM_UPDATE_CUSTOMIZED_NAME` | Known | 2 | 0 | never sent |
| `0x013E` | `ITEM_MOVED_TO_LOCATION` | Emitted | 57 | 7 | 1:1 shape, fewer items |
| `0x013F` | `INVENTORY_CREATE_BAG` | Emitted | 6 | 6 | 1:1 |
| `0x0144` | `ITEM_STREAM_CREATE` | Emitted | 1 | 1 | 1:1 |
| `0x0148` | `ITEM_SET_ACTIVE_WEAPON_SET` | Emitted | 1 | 1 | 1:1 |
| `0x015A` | `ITEM_SET_PROFESSION` | Emitted | 2 | 6 | 1:1 |
| `0x015E` | `GAME_SMSG_ITEM_LOW_DETAIL` | Observed | 118 | 0 | named, never sent |
| `0x0161` | `CREATE_NAMED_ITEM` | Emitted | 76 | 8 | 1:1 shape, fewer items |
| `0x017C` | `INSTANCE_LOAD_HEAD` | Emitted | 1 | 1 | 1:1 |
| `0x017D` | `INSTANCE_LOAD_PLAYER_NAME` | Emitted | 1 | 1 | 1:1 |
| `0x0186` | `INSTANCE_PLAYER_DATA_START` | Emitted | 1 | 1 | 1:1 |
| `0x0196` | `INSTANCE_LOAD_SPAWN_POINT` | Emitted | 1 | 1 | 1:1, and the file id agrees |
| `0x019A` | `INSTANCE_LOAD_INFO` | Emitted | 1 | 1 | 1:1 |
| `0x01A0` | `GAME_SMSG_INSTANCE_TABLE` | Emitted | 10 | 10 | sent: ten batches in every load block (`crates/asura-world/src/map.rs:432-437`) |
| `0x008B` `0x008A` | `MAP_INIT_START`, `CARTOGRAPHY_DATA` | Emitted | 0 | 1 + 1 | sent by us, absent from this capture |
| `0x0094` | `MAP_UPDATE_UNLOCKED_LIST` | Emitted | 0 | 1 | sent by us, absent from this capture |
| `0x0099` `0x01AC` `0x0198` `0x0199` `0x00F2` `0x018E` | map update, ready, manifest, loaded | Emitted | 0 | 1 + 1 + 2 + 4 + 1 + 1 | sent by us, absent from this capture |

Two readings of the bottom two rows, and the difference matters: either this capture began after those
messages had already gone by, or the retail build puts those roles on other headers. Evidence for the
first reading: the file's first record is already `0x019A` at `t = 24.454`, so the capture starts inside
the load and proves nothing about what preceded it. Evidence for the second: the character-creation
capture does contain two `0x008B` and **no `0x008A` at all**
(`recordings/client/feature/character_creation/character_creation-live-necro_factions.jsonl`). That
anomaly is unresolved and is listed as an open question, not as a defect.

The 148 spawn point agrees with ours on the number that matters: the live `0x0196` carries
`0x0001B97D` = 113021, which is exactly the `map_file_id` our world sends for map 148
(`logs/world-8001.log`, `joined map 148 (file id 113021)`).

## The map table: `0x01A0`, decoded

Ten records in the load of map 148, each a `u32` count of 16 followed by 16 pairs of
`(map id, u32 value)`, 132 bytes each (four-byte header included):

| record | pairs (map id -> value) |
|---|---|
| seq 751 | 148->`BB14F539` 145->`F15FBD2A` 146->`140AB7CE` 147->`4106ABB8` 149->`53E130C4` 150->`53E0B8E1` 151->`4C94ECA3` 160->`5808F8FE` 161->`127956FA` 162->`3252A63F` 163->`B9642F2E` 164->`8302BE3A` 165->`BAC5F32C` 166->`D50F5EEF` 779->`2ECE130E` |
| seq 763 | 877, 878, 879, 880, 881, 882, 4, 5, 6, 52, 65, 167, 168, 170, 171 |
| seq 766 | 173 to 188, then 208 |

and seven more, in order, to `seq 791` (count 3, ids 598 and 599). Together: **137 pairs over 147 map
ids, running up to 877** (`crates/asura-protocol/src/game/instance_table.rs:10-14` says the same), which
is the number the reference's manifest calls `MapId_Count`
(`crates/asura-world/src/map.rs:464`) - the sentinel for "every map up to here".

The second column is not unique per map: `0x197A39CC` belongs to 65, 318, 823 and 339; `0x7082A801` to
182, 319, 824 and 340; `0xBF665D0E` to 180, 320, 826 and 341. Groups of four maps share a value, so the
value is a **region or continent of the map, not the map itself** (a guess, and it is what the ids say,
nothing more).

**This message is sent now**: ten batches in every load block
(`crates/asura-world/src/map.rs:432-437`), because a client that is never told which map ids exist has no
table to resolve an exit against - the shape of the cartography failure.

## The unnamed rows, with their measured shape

Names below the line are **guesses and say so**; the columns are measurements.

| header | records | size (client) | shape | what it looks like | confidence |
|---|---|---|---|---|---|
| `0x003C` | 29 | 16 | `header, agent id, u32, u32 = 7` | per player agent, right after its create: `0x35`->0, `0x14`->5, `0x2B`->4, `0x3A`->5 | a per-agent number that is not profession; a guess |
| `0x011F` | 20 | 136 | mostly zero dwords, a three-unit UTF-16 run `G D H`, then `0x28, 0xF0, 0x3F, 0x3F, 2, 0x16, 0x5F` | a fixed-size record with a short label and a run of small numbers | unknown; a guess would be wrong |
| `0x012E` | 1 | 8 | header only | a toggle with no payload | unknown |
| `0x012F` | 1 | 556 | zeros, one `0xF1`, then `fa 00 6e 00 20 00 65 00 73 00 74 00` | one non-zero dword and one counted string | unknown |
| `0x015E` | 118 | 40 | nine dwords: `id, 0x29x, n, 0x14, m, 0xE7, k, 0x20xxxxxx, 9` with `0x14`, `0xE7`, `9` constant | per item: five varying numbers around three constants | a per-item property message |
| `0x01A0` | 10 | 132 / 28 | counted pairs, see above | **the map table: map id -> region** | high, and acted on below |

`0x015E` is the largest single unknown in a load (118 records against 76 items), and it is the one to
decode first on the item side.

## Flow B - a travel, 117 of them, and what the world must produce

`recordings/client/feature/character_creation/maps-tour3.jsonl` is 585 records and **five headers**:
`0x01A6` 117, `0x0099` 117, `0x017C` 117, `0x019A` 117, `0x0196` 117. Every travel is exactly that
cycle, on the game connection, and the capture contains **zero client-to-server records** - the request
that caused each travel went on the auth connection, which the sniffer did not see.

So the world side of a travel is fully specified by data we hold: the redirect (`0x01A6`, host[24] then
`map_token, region, map_id, is_explorable, player_token, 0`), then `0x0099` with the destination, then the
new connection's head. Our `travel::teleport` already produces the first two; `maps-tour3.jsonl` seq 16
is a travel with `is_explorable = 1`, so a live server does place a client in a non-outpost.

## Flow C - the character-creation instance

`character_creation-live-necro_factions.jsonl` (1109 records, 19 agent creates) and
`character_creation-live-warrior_prophecies.jsonl` (196 records, no agents). Same population pattern as
flow A, plus the creation exchanges this tree already answers.

## What the instrument answers, and what only a client can

`docs/adr/0013-py4gw-stealth-is-our-link-to-the-client.md` is the instrument, and its catalogue already
names the contexts the questions need. No offset, pattern or value from it belongs in this tree; the
names below are what to *read* with it:

| question | context to read |
|---|---|
| is the outpost gate an agent? | the agent table: every entry's type, model, name and position, live, next to the gate |
| what does the client believe about the map we sent? | the instance info: map id, file id, the values our `0x019A`/`0x0196` feed |
| is the map table ours to send? | the same instance info after a `0x01A0` arrives versus without one |
| what arms an exit? | the gameplay context, sampled before and after walking into the gate |

## The ranked next changes, each with its source

1. **~~Send `0x01A0`, the map table.~~ Delivered**: ten batches, 137 pairs over 147 ids, in every load
   block (`crates/asura-world/src/map.rs:432-437`); the client's map and mission surfaces open with it.
2. **Send the population.** Source: the 42 agent creates and their per-agent streams in the same
   capture. Accepts it: an outpost looks like the live one, and the gate question becomes one walk.
3. **Name the rest of the unnamed rows** from the same capture and move them off `Unnamed`: `0x011F`,
   `0x012F` and `0x8092` are what is left of this page's list (`docs/plans.md`).
4. **~~The portal touch.~~ Partly delivered**: a doorway a walk reaches starts the transfer
   (`crates/asura-world/src/travel.rs:364-433`); the `0x80C1` arm still only logs (`travel.rs:154-166`).

Items 1 and 2 change what a client sees and need the client to accept them; item 3 changes nothing on the
wire. None of them is a guess about behaviour: each reproduces messages a live server sent, in the order
it sent them.

## 23 - The initialization protocol: everything a client is fed when it enters an instance

> Moved here on 2026-11-06; it was the file `plans.md`.

**Status: the protocol, written down; the gap table says what is missing.** The owner's direction,
2026-10-31: "this must mean we are still ignoring data on initialization, we will need parity, we are
ignoring too many messages and our functionality is being ignored there ... lets keep a full
initialization protocol so we know all our structures are fed appropriately."

So this document is the standing specification for one thing: **the set and order of messages an instance
sends**. A feature that works against a live server but not here is a difference in this table until the
table says otherwise.

## The rule

An instance's initialization is not a bag of messages a client tolerates; it is a sequence that fills a
fixed set of client structures. A world that sends a subset leaves structures empty, and the client then
does nothing where it would otherwise do something - the exploration grid was the first instance of this
(`docs/03-protocol.md` 11.4), the empty unlocked list was the second, and the portal is the third
candidate.

The source for the sequence is a live load, not the reference's C: `GAME_SMSG_*` order and content are
read from `recordings/client/feature/maps/pre-searing-load-3.jsonl` (instance 148, a real client against a
live server) and cross-checked against today's own run in `logs/world-8001.log`. **Order, count and trigger
are what that file is read for**; a size in it is the client's message struct and never a wire size, so no
size and no field layout comes from it (`docs/03-protocol.md` 5e) - those come from a wire capture, and a
message added here is built at the size `crates/asura-protocol/src/packets.rs` declares.

## The live sequence, in order

Non-tick messages of that load, grouped in the order they arrived; the counts are that one capture's.

| # | messages | count | what structure it fills |
|---|---|---|---|
| 1 | `0x017C` `INSTANCE_LOAD_HEAD` | 1 | the load begins |
| 2 | `0x0186` `INSTANCE_PLAYER_DATA_START` | 1 | |
| 3 | `0x017D` `INSTANCE_LOAD_PLAYER_NAME` | 1 | |
| 4 | `0x019A` `INSTANCE_LOAD_INFO` | 1 | which instance this is |
| 5 | `0x0144` `ITEM_STREAM_CREATE`, `0x0148` `ITEM_SET_ACTIVE_WEAPON_SET` | 1 + 1 | the item stream opens |
| 6 | `0x0161` `CREATE_NAMED_ITEM`, `0x013E` `ITEM_MOVED_TO_LOCATION`, `0x013F` `INVENTORY_CREATE_BAG`, `0x013A` `ITEM_UPDATE_CUSTOMIZED_NAME`, `0x015A` `ITEM_SET_PROFESSION` | 76 + 57 + 6 + 2 + 2 | the inventory |
| 7 | `0x0196` `INSTANCE_LOAD_SPAWN_POINT` | 1 | where the character stands |
| 8 | `0x001D` `PVP_UPDATE_UNLOCKED_SKILLS`, `0x0018` `PVP_UPDATE_UNLOCKED_HEROES`, `0x001A` `PVP_ITEM_STREAM_ADD` | 1 + 1 + 62 | the PvP unlock tables |
| 9 | `0x0059` `PLAYER_CREATE`, `0x0020` `WORLD_CREATE_AGENT`, `0x0056` `NPC_UPDATE_PROPERTIES`, `0x0057` `NPC_UPDATE_MODEL`, `0x006D` `NPC_UPDATE_WEAPONS`, `0x00A6` `AGENT_UPDATE_PROFESSION`, `0x009A` `AGENT_UPDATE_SCALE`, `0x009B` `AGENT_UPDATE_NPC_NAME`, `0x00F0` `AGENT_INITIAL_EFFECTS`, `0x003C` (unnamed), `0x009F` `AGENT_PROPERTY_UPDATE_INT`, `0x00A2` `AGENT_PROPERTY_UPDATE_FLOAT`, `0x0048` `AGENT_DISPLAY_CAPE`, `0x006E` `UPDATE_AGENT_VISUAL_EQUIPMENT`, `0x00B0` `PLAYER_UPDATE_PARTY_SIZE`, `0x00B1` `PLAYER_UPDATE_PARTY` | 25 + 42 + 14 + 13 + 19 + 31 + 10 + 22 + 42 + 29 + 82 + 4 + 23 + 23 + 30 + 29 | **the agent array** - every body in the instance |
| 10 | `0x004C` `QUEST_DESCRIPTION`, `0x0053` `QUEST_ADD_MARKER` | 1 + 1 | the quest log |
| 11 | `0x0118` `GUILD_PLAYER_ROLE`, `0x0121` `GUILD_GENERAL_INFO` | 1 + 14 | the guild window |
| 12 | `0x00F3` `TITLE_RANK_DATA`, `0x00F4` `TITLE_RANK_DISPLAY` | 3 + 8 | the title track |
| 13 | `0x01A0` (unnamed) | 10 | the instance table (`docs/plans.md`) |
| 14 | `0x006B` `POST_PROCESS`, `0x003E` `AGENT_ALLY_DESTROY`, `0x0021` `WORLD_REMOVE_AGENT`, `0x005A` `PLAYER_DESTROY` | 1 each | the tail |

## The two layers, measured (2026-10-31)

One run entered the same outpost twice, with a different-district load of that outpost between them
(`recordings/asura/world/load-layers-20261031.jsonl`, five loads with their per-header counts). The
district load carried **0 agent creates** and 260 messages, against 8 creates and 539 messages for the
full load of the same map. That splits the initialization into two layers by measurement rather than
by guess:

| layer | what it is | sent on | headers |
|---|---|---|---|
| per character | items, titles, party, player flags, guild, the instance table | every load, a district change included | `0x0161`, `0x015E`?, `0x013E`, `0x013F`, `0x00F3`, `0x00F4`, `0x00B0`, `0x00B1`, `0x003C`, `0x0118`, `0x0121`, `0x01A0` |
| per map population | every body in the instance and its streams | only when the world sends a population | `0x0020`, `0x0021`, `0x0025`, `0x0029`, `0x002B`, `0x0028`, `0x0048`, `0x0056`, `0x0057`, `0x0059`, `0x005A`, `0x006D`, `0x006E`, `0x008C`, `0x009B`, `0x00A6`, `0x00AA`, `0x00F0`, `0x00F1`, `0x0115`, `0x0107` |

The same run measured the per-map shape of a load: 166 loaded with 8 creates and 538 messages, 162
with 4 and 3 creates and 342 and 238 messages, and `0x008A` carried 132 bytes on every load but the
session's first. `0x0196` carried a different position per map. So the population block is
`population = creates` and it is per instance, never a constant to copy from one capture.

What the client asks while this happens, in the same run: `0x8093` eight times, one per instance
(`166, 145, 146, 148, 160, 161, 162, 164`) when the world map opened; `0x00B1` once, naming 166 as the
chosen instance; `0x00C1` twice at a portal, `{1, 0}` then `{0, 0}`. Of the three, the world-map request
is answered - `GAME_CMSG_PARTY_TRAVEL` ends in the same transfer cycle (`crates/asura-world/src/
dispatch.rs:58`, `crates/asura-world/src/travel.rs:298`). `0x8093` is named
(`GAME_CMSG_INSTANCE_QUERY`) and read, and `0x80C1` is read: neither starts anything
(`crates/asura-world/src/travel.rs:154-166`).

## What this world sends today, in order

`load_block` then `items_block` then `player_block`, from `crates/asura-world/src/{map,item,player}.rs`:

| # | messages | state |
|---|---|---|
| 1-4 | `0x017C`, `0x0186`, `0x017D`, `0x019A` | sent, same order |
| 5 | `0x008B` `MAP_INIT_START` + `0x008A` `CARTOGRAPHY_DATA` | sent (this client needs it; the capture predates it) |
| 6 | `0x01A0` instance table, ten batches | **sent (2026-10-31)** |
| 7 | `0x0144`, `0x0148`, items | sent on the client's `0x8091` |
| 8 | `0x0196` | sent on the client's `0x8088` |
| 9 | PvP streams | sent |
| 10 | `0x0094` with the reached instances in the fifth list | **sent (2026-10-31)** |
| 11 | `0x0099`, `0x01AC`, manifest `0x0199`/`0x0198`, `0x00F2`, `0x018E` | sent |
| 12 | **the agent array: 21 or 42 bodies with their per-agent streams** | **not sent - one agent, the player** |
| 13 | quests, guild, titles | **not sent** |

## The explorable: one field decides which load the client runs (measured 2026-10-31)

`is_explorable` in the redirect is not a label a client stores. It selects **which load the client runs**, and
an explorable's load is a different procedure from an outpost's. Measured on the patched client against this
world, with one field moved and nothing else (`ASURA_TRAVEL_EXPLORABLE`,
`crates/asura-world/src/travel.rs`), over the same walk through the same portal:

| what the redirect carried | destination | redirect to the players block |
|---|---|---|
| `is_explorable 1` - the destination's own kind | 146 Lakeside County, explorable | **5138 ms** |
| `is_explorable 0` - forced | 146, the same walk | **849 ms** |
| `is_explorable 0` - forced | 148 Ascalon City, outpost | 849 ms |

The client's own clock agrees: in `recordings/client/feature/travel/explorable-flag-on-20261031-223636.jsonl`
the load with the flag at 1 spends **4984 ticks (7.0 s)** between its spawn request and its players request,
and the crossing back into the outpost in the same file spends 640 ticks (0.90 s). The live figure for that
step, over 36 loads in five captures, is 0.507-1.015 s, median 0.763 s.

**Why the flag costs 4.3 s here and nothing on a live server.** What is measured is that the flag selects a
load path, and that both of this world's paths are fed empties: 16 bytes of unexplored grid
(`crates/asura-world/src/cartography.rs`), a party of one, no agents at all, no quests, guild or titles. What
is **not** established is which of those empties the 4.3 s is spent on, and the wire argues against the
obvious answer: the client asks for the players block - the message that carries the population - only
*after* the wait (`0x0196` at tick 291072750, `0x8090` at 291077734 in the capture above), so the population
cannot be what it waits for. It asks for nothing at all during the wait, and neither server sends anything
in it. The wait is therefore inside the client's own load of the explorable, and the candidates are the
structures it builds before it asks: the map view from the grid, and the place itself.

A live server does not pay it: the same step there is 0.76 s with the same flag.

**The experiment that names it, and it needs no data.** `tools/context-dump.py` reads the client's contexts
without elevation. A dump taken before the crossing, one about a second after the spawn point (inside the
wait) and one after the load, in both modes, is what tells the empties apart: whichever context is being
worked on during the wait is the one the flag's path is waiting for. Until that is run this section names the
path, not the cause.

**What that makes this table.** Row 9 is no longer only an instance-completeness gap: it is what an explorable
is missing before the flag can honestly say 1 again, together with rows 10 to 12 and the grid. It is not yet
proven to be the cause of the 4.3 s - see the paragraph above - and the population work should not start until
the context dump says which structure the wait is spent on.

**The default, and what it costs.** `is_explorable` is sent as **0 for every travel**
(`crates/asura-world/src/travel.rs`, `explorable_flag_for`). That removes the wait and tells the client an
explorable is an outpost: no minimap for the map, no henchman or party behaviour, no explorable rules.
`ASURA_TRAVEL_EXPLORABLE=auto` restores the destination's own kind, which is what a live server sends and what
this default becomes once an explorable carries the initialisation this table says it is missing; `=1` forces
1 for a measurement. So this is a recorded deviation from live, taken 2026-10-31 so that travelling works, and
the world's startup line states which value a run used.

**What was ruled out** (all measured 2026-10-31 on the patched client, one variable at a time, each with the
5.1 s unchanged): the four withdrawer messages of the travel burst, restored and timed; the world clock running
inside the load, 49 ticks before the gate and 0 after it; the exploration grid and the instance table, removed
from the unasked block entirely (`ASURA_LOAD_EXTRA=off`, a measurement switch); and the database, which shows no
stall (a 400-iteration probe, the slowest round trip 156 ms). The wait is therefore inside the client's own
explorable load and its cause is still missing; the context dump named above is the measurement that finds it.

**What is not settled.** Live does not treat the field as a pure function of the destination either:
`recordings/client/feature/character_creation/maps-tour3.jsonl` carries 0, 1 and 0 for destination 29. So the
field's live rule is measured only for the pre-Searing redirects, where it follows the kind. One destination,
several visits, a live server is the measurement that closes it.

## The gap, and the order it is paid

The order below is the order of *completeness*. The explorable's 4.3 s wait is not proven to sit in any of
these rows - the section above says why, and the context dump is the measurement that names it - so the
population work is aimed by that dump rather than by this list.

1. **The agent array (live row 9).** The largest gap and the one that changes what the client builds for an
   instance: every body with its name, model, properties, weapons, profession, scale and effects. Source:
   `pre-searing-load-3.jsonl`, message by message, 42 creatures in that outpost. **It is also the one an
   explorable's load waits for**: with `is_explorable` at 1 and no agents the client falls back after 4.3 s
   (the section above), so this row is now a travel requirement and not only an instance-completeness one.
2. **Quests, guild and titles (rows 10 to 12).** Each is its own structure; a live instance sends them
   even for a character with none (empty lists and zero rows), because an unsent structure is not an empty
   one.
3. **The `0x0196` tail.** The reference zeroes the last eight bytes (`GameMsg.h:346`); a live server fills
   them with two dwords that change per visit. Unmeasured meaning, recorded here as a gap rather than
   guessed at.
4. **The client's own initialization messages we do not answer:** `0x8092` (120 bytes, still `Unnamed`)
   and `0x8093` (4 bytes, `{u16 instance}`, now named `GAME_CMSG_INSTANCE_QUERY` and `Observed`,
   `crates/asura-protocol/src/packets.rs:480`), sent 25 times each in one session while browsing the world
   map. Both are unhandled. A live capture of the *retail* build does not contain them, so what they are
   owed has to be measured against this client, not copied.

## How this document is used

- A new feature starts by finding its row here and asking what fills it.
- A message added to an instance sends the whole set its row names, not the one field the feature wanted.
- The row that changes says so in `docs/03-protocol.md` with the measurement behind it.
- When the whole table matches, the initialization is in parity, and that is the state the portal question
  is asked from.

## 24 - Stream parity: every message a live client exchanged, and what we do with it

> Moved here on 2026-11-06; it was the file `plans.md`.

Status: maintained by hand from the committed captures. Its generator was retired with the rest of the
parity tooling (`docs/history.md`); a capture that adds a header adds a row here.

The owner's rule, 2026-10-31: "100% replicate the stream without discriminating a single message, even
if that has nothing to do with our task". So every header a live capture contains is listed with what this
tree does about it, and no row is dropped because a feature does not need it.

Header values are written with the direction bit set the way the wire carries it (`0x80xx` from the client,
`0x00xx` from the server); the sniffer records the client's without it, which the script adds back.

**`sizes seen` is the client's message-struct size, never a wire size** (`docs/03-protocol.md` 5e). It
runs 2 bytes over the wire on every fixed message (`0x001E` 8 against 6, `0x0186` 4 against 2) and much
further where the struct pads (`0x000A` 240 against 140, `0x000B` 324 against 88). The column says "what
the client held": it is a count and a shape, not a length to send. The wire size per header is
`crates/asura-protocol/src/packets.rs`, and the wire forms to copy are
`recordings/gwps/world/load-stream.log` and the sessions under `recordings/asura/`.

| header | direction | live count | sizes seen | our name | our status |
|---|---|---|---|---|---|
| 0x000C | S -> C | 111 | 4 | GAME_SMSG_PING_REQUEST | Emitted |
| 0x000D | S -> C | 111 | 8 | GAME_SMSG_PING_REPLY | Emitted |
| 0x000F | S -> C | 160 | 16 | GAME_SMSG_ACCOUNT_FEATURE | Emitted |
| 0x0018 | S -> C | 6 | 20 | GAME_SMSG_PVP_UPDATE_UNLOCKED_HEROES | Emitted |
| 0x001A | S -> C | 435 | 20/28 | GAME_SMSG_PVP_ITEM_STREAM_ADD | Emitted |
| 0x001D | S -> C | 6 | 260 | GAME_SMSG_PVP_UPDATE_UNLOCKED_SKILLS | Emitted |
| 0x001E | S -> C | 4644 | 8 | GAME_SMSG_WORLD_SIMULATION_TICK | Emitted |
| 0x0020 | S -> C | 386 | 116 | GAME_SMSG_WORLD_CREATE_AGENT | Emitted |
| 0x0021 | S -> C | 195 | 8 | GAME_SMSG_WORLD_REMOVE_AGENT | Emitted |
| 0x0022 | S -> C | 8 | 12 | GAME_SMSG_WORLD_UPDATE_CONTROLLED_AGENT | Emitted |
| 0x0025 | S -> C | 369 | 20 | GAME_SMSG_AGENT_UPDATE_DIRECTION | Emitted |
| 0x0026 | S -> C | 7 | 12 | GAME_SMSG_AGENT_UPDATE_FLAGS | Observed |
| 0x0027 | S -> C | 2 | 12 | GAME_SMSG_AGENT_UPDATE_SPEED_BASE | Known |
| 0x0028 | S -> C | 36 | 8 | GAME_SMSG_AGENT_STOP_MOVING | Emitted |
| 0x0029 | S -> C | 1535 | 24 | GAME_SMSG_AGENT_MOVE_TO_POINT | Emitted |
| 0x002A | S -> C | 10 | 28 | GAME_SMSG_AGENT_UPDATE_DESTINATION | Known |
| 0x002B | S -> C | 255 | 16 | GAME_SMSG_AGENT_UPDATE_SPEED | Emitted |
| 0x002E | S -> C | 112 | 16 | GAME_SMSG_AGENT_UPDATE_ROTATION | Known |
| 0x002F | S -> C | 1 | 12 | GAME_SMSG_AGENT_UPDATE_ALLEGIANCE | Known |
| 0x0035 | S -> C | 4 | 16 | - | Unnamed |
| 0x0037 | S -> C | 33 | 16 | GAME_SMSG_AGENT_UPDATE_ATTRIBUTE_POINTS | Emitted |
| 0x003C | S -> C | 115 | 16 | GAME_SMSG_PLAYER_UPDATE_FLAGS | Emitted |
| 0x003E | S -> C | 1 | 8 | GAME_SMSG_AGENT_ALLY_DESTROY | Known |
| 0x0048 | S -> C | 61 | 12 | GAME_SMSG_AGENT_DISPLAY_CAPE | Emitted |
| 0x004C | S -> C | 35 | 520 | GAME_SMSG_QUEST_DESCRIPTION | Known |
| 0x0053 | S -> C | 12 | 24 | GAME_SMSG_QUEST_ADD_MARKER | Known |
| 0x0056 | S -> C | 152 | 52 | GAME_SMSG_NPC_UPDATE_PROPERTIES | Known |
| 0x0057 | S -> C | 131 | 24 | GAME_SMSG_NPC_UPDATE_MODEL | Known |
| 0x0059 | S -> C | 65 | 92 | GAME_SMSG_PLAYER_CREATE | Emitted |
| 0x005A | S -> C | 4 | 8 | GAME_SMSG_PLAYER_DESTROY | Emitted |
| 0x005D | S -> C | 1 | 248 | GAME_SMSG_CHAT_MESSAGE_CORE | Emitted |
| 0x005E | S -> C | 1 | 12 | GAME_SMSG_CHAT_MESSAGE_SERVER | Emitted |
| 0x0067 | S -> C | 1 | 16 | - | Unnamed |
| 0x006B | S -> C | 13 | 12 | GAME_SMSG_POST_PROCESS | Known |
| 0x006D | S -> C | 312 | 16 | GAME_SMSG_NPC_UPDATE_WEAPONS | Known |
| 0x006E | S -> C | 61 | 44 | GAME_SMSG_UPDATE_AGENT_VISUAL_EQUIPMENT | Emitted |
| 0x006F | S -> C | 1 | 16 | GAME_SMSG_UPDATE_AGENT_VISUAL_EQUIPMENT_SLOT | Observed |
| 0x007C | S -> C | 32 | 8 | - | Unnamed |
| 0x007D | S -> C | 33 | 8 | - | Unnamed |
| 0x008A | S -> C | 33 | 132 | GAME_SMSG_CARTOGRAPHY_DATA | Emitted |
| 0x008B | S -> C | 33 | 16 | GAME_SMSG_MAP_INIT_START | Emitted |
| 0x008C | S -> C | 39 | 16 | - | Unnamed |
| 0x008D | S -> C | 1 | 48 | GAME_SMSG_MAP_MARKER | Inferred |
| 0x0099 | S -> C | 255 | 12 | GAME_SMSG_MAP_UPDATE_CURRENT | Emitted |
| 0x009A | S -> C | 28 | 12 | GAME_SMSG_AGENT_UPDATE_SCALE | Known |
| 0x009B | S -> C | 191 | 72 | GAME_SMSG_AGENT_UPDATE_NPC_NAME | Known |
| 0x009C | S -> C | 33 | 12 | GAME_SMSG_AGENT_UPDATE_MORALE | Emitted |
| 0x009F | S -> C | 394 | 16 | GAME_SMSG_AGENT_PROPERTY_UPDATE_INT | Emitted |
| 0x00A0 | S -> C | 12 | 20 | GAME_SMSG_AGENT_PROPERTY_UPDATE_INT_TARGET | Known |
| 0x00A2 | S -> C | 30 | 16 | GAME_SMSG_AGENT_PROPERTY_UPDATE_FLOAT | Emitted |
| 0x00A3 | S -> C | 10 | 20 | GAME_SMSG_AGENT_PROPERTY_UPDATE_FLOAT_TARGET | Known |
| 0x00A4 | S -> C | 1 | 36 | GAME_SMSG_AGENT_PROJECTILE_LAUNCHED | Known |
| 0x00A5 | S -> C | 1 | 252 | GAME_SMSG_AGENT_UPDATE_SPEECH_BUBBLE | Known |
| 0x00A6 | S -> C | 92 | 16 | GAME_SMSG_AGENT_UPDATE_PROFESSION | Known |
| 0x00A7 | S -> C | 40 | 16 | - | Unnamed |
| 0x00AA | S -> C | 2 | 16 | GAME_SMSG_AGENT_CREATE_NPC | Known |
| 0x00AE | S -> C | 3 | 12 | GAME_SMSG_AGENT_UPDATE_MODEL | Known |
| 0x00B0 | S -> C | 82 | 12 | GAME_SMSG_PLAYER_UPDATE_PARTY_SIZE | Emitted |
| 0x00B1 | S -> C | 80 | 12 | GAME_SMSG_PLAYER_UPDATE_PARTY | Emitted |
| 0x00B5 | S -> C | 32 | 4 | GAME_SMSG_PREFERENCES_BLOB | Inferred |
| 0x00B6 | S -> C | 33 | 12 | GAME_SMSG_PLAYER_UPDATE_UNLOCKED_PROFESSIONS | Emitted |
| 0x00B7 | S -> C | 37 | 20 | GAME_SMSG_PLAYER_UPDATE_PROFESSION | Emitted |
| 0x00BA | S -> C | 7 | 4 | GAME_SMSG_MISSION_STREAM_START | Known |
| 0x00CF | S -> C | 9 | 12 | - | Unnamed |
| 0x00D0 | S -> C | 1 | 8 | - | Unnamed |
| 0x00DA | S -> C | 33 | 44 | GAME_SMSG_SKILLBAR_UPDATE | Emitted |
| 0x00DB | S -> C | 38 | 260 | GAME_SMSG_UPDATE_UNLOCKED_SKILLS | Emitted |
| 0x00E9 | S -> C | 33 | 64 | GAME_SMSG_CHARACTER_UPDATE_FACTIONS | Emitted |
| 0x00EF | S -> C | 33 | 12 | GAME_SMSG_CHAR_VISIBILITY_FLAGS | Emitted |
| 0x00F0 | S -> C | 352 | 12 | GAME_SMSG_AGENT_INITIAL_EFFECTS | Emitted |
| 0x00F1 | S -> C | 37 | 12 | GAME_SMSG_AGENT_UPDATE_EFFECTS | Known |
| 0x00F2 | S -> C | 32 | 8 | GAME_SMSG_INSTANCE_LOADED | Emitted |
| 0x00F3 | S -> C | 931 | 32 | GAME_SMSG_TITLE_RANK_DATA | Known |
| 0x00F4 | S -> C | 36 | 12 | GAME_SMSG_TITLE_RANK_DISPLAY | Known |
| 0x00F6 | S -> C | 442 | 76 | GAME_SMSG_TITLE_TRACK_INFO | Known |
| 0x00FE | S -> C | 1 | 4 | GAME_SMSG_CINEMATIC_SKIP_EVERYONE | Known |
| 0x0100 | S -> C | 2 | 8 | GAME_SMSG_CINEMATIC_START | Known |
| 0x0103 | S -> C | 1 | 4 | GAME_SMSG_CINEMATIC_DATA_END | Known |
| 0x0104 | S -> C | 2 | 4 | GAME_SMSG_CINEMATIC_DATA | Known |
| 0x0107 | S -> C | 15 | 84 | - | Unnamed |
| 0x010E | S -> C | 10 | 16 | GAME_SMSG_MANIPULATE_MAP_OBJECT | Known |
| 0x0111 | S -> C | 9 | 16 | GAME_SMSG_MANIPULATE_MAP_OBJECT2 | Known |
| 0x0115 | S -> C | 22 | 12 | GAME_SMSG_GADGET_SET_FLAGS | Inferred |
| 0x0118 | S -> C | 12 | 28 | GAME_SMSG_GUILD_PLAYER_ROLE | Known |
| 0x011A | S -> C | 18 | 124 | GAME_SMSG_TOWN_ALLIANCE_OBJECT | Known |
| 0x011B | S -> C | 1 | 12 | GAME_SMSG_TOWN_ALLIANCE_COMMIT | Inferred |
| 0x011C | S -> C | 9 | 4 | GAME_SMSG_TOWN_ALLIANCE_END | Inferred |
| 0x011F | S -> C | 60 | 136 | - | Unnamed |
| 0x0121 | S -> C | 43 | 160 | GAME_SMSG_GUILD_GENERAL_INFO | Known |
| 0x0127 | S -> C | 45 | 272 | GAME_SMSG_GUILD_PLAYER_INFO | Known |
| 0x012E | S -> C | 12 | 8 | GAME_SMSG_LOCATION_TAG | Inferred |
| 0x012F | S -> C | 3 | 556 | - | Unnamed |
| 0x013A | S -> C | 83 | 72 | GAME_SMSG_ITEM_UPDATE_CUSTOMIZED_NAME | Known |
| 0x013E | S -> C | 2322 | 20 | GAME_SMSG_ITEM_MOVED_TO_LOCATION | Emitted |
| 0x013F | S -> C | 239 | 28 | GAME_SMSG_INVENTORY_CREATE_BAG | Emitted |
| 0x0144 | S -> C | 39 | 12 | GAME_SMSG_ITEM_STREAM_CREATE | Emitted |
| 0x0148 | S -> C | 39 | 12 | GAME_SMSG_ITEM_SET_ACTIVE_WEAPON_SET | Emitted |
| 0x014B | S -> C | 15 | 20 | GAME_SMSG_ITEM_CHANGE_LOCATION | Known |
| 0x014D | S -> C | 10 | 12 | GAME_SMSG_ITEM_REMOVE | Emitted |
| 0x015A | S -> C | 61 | 12 | GAME_SMSG_ITEM_SET_PROFESSION | Emitted |
| 0x015B | S -> C | 25 | 12 | - | Unnamed |
| 0x015E | S -> C | 279 | 40 | GAME_SMSG_ITEM_LOW_DETAIL | Observed |
| 0x0161 | S -> C | 2672 | 176/180/184/188/192/196/200/212/216/228 | GAME_SMSG_CREATE_NAMED_ITEM | Emitted |
| 0x0168 | S -> C | 6 | 12 | - | Unnamed |
| 0x016E | S -> C | 11 | 8 | GAME_SMSG_LATENCY_ENABLE | Inferred |
| 0x017C | S -> C | 256 | 20 | GAME_SMSG_INSTANCE_LOAD_HEAD | Emitted |
| 0x017D | S -> C | 37 | 44 | GAME_SMSG_INSTANCE_LOAD_PLAYER_NAME | Emitted |
| 0x0186 | S -> C | 39 | 4 | GAME_SMSG_INSTANCE_PLAYER_DATA_START | Emitted |
| 0x0188 | S -> C | 1 | 64 | GAME_SMSG_CHAR_CREATION_SUCCESS | Emitted |
| 0x0189 | S -> C | 2 | 4 | GAME_SMSG_CHAR_CREATION_START | Emitted |
| 0x018E | S -> C | 11 | 4 | GAME_SMSG_INSTANCE_LOAD_FINISH | Emitted |
| 0x0194 | S -> C | 11 | 8 | - | Unnamed |
| 0x0196 | S -> C | 254 | 36 | GAME_SMSG_INSTANCE_LOAD_SPAWN_POINT | Emitted |
| 0x0197 | S -> C | 7 | 4 | GAME_SMSG_INSTANCE_MANIFEST_DATA | Known |
| 0x0198 | S -> C | 5 | 16 | GAME_SMSG_INSTANCE_MANIFEST_DONE | Emitted |
| 0x0199 | S -> C | 6 | 8 | GAME_SMSG_INSTANCE_MANIFEST_PHASE | Emitted |
| 0x019A | S -> C | 254 | 32 | GAME_SMSG_INSTANCE_LOAD_INFO | Emitted |
| 0x01A0 | S -> C | 30 | 28/132 | GAME_SMSG_INSTANCE_TABLE | Emitted |
| 0x01A6 | S -> C | 248 | 52 | GAME_SMSG_TRANSFER_GAME_SERVER_INFO | Emitted |
| 0x01AE | S -> C | 21 | 64 | - | Unnamed |
| 0x01B3 | S -> C | 10 | 12 | GAME_SMSG_CHARACTER_UDPATE_PARTY | Emitted |
| 0x01BE | S -> C | 11 | 8 | - | Unnamed |
| 0x01BF | S -> C | 11 | 8 | GAME_SMSG_PARTY_SET_DIFFICULTY | Observed |
| 0x01CC | S -> C | 10 | 16 | GAME_SMSG_PARTY_PLAYER_ADD | Emitted |
| 0x01D3 | S -> C | 9 | 8 | GAME_SMSG_PARTY_CREATE | Emitted |
| 0x01D4 | S -> C | 10 | 8 | GAME_SMSG_PARTY_MEMBER_STREAM_END | Emitted |
| 0x01DA | S -> C | 6 | 256 | GAME_SMSG_PARTY_LOCK | Known |
| 0x01E0 | S -> C | 8 | 8 | GAME_SMSG_PARTY_SEARCH_SEEK | Known |
| 0x800A | C -> S | 4 | 240 | GAME_CMSG_COMPUTER_INFO | Known |
| 0x800B | C -> S | 4 | 324 | GAME_CMSG_COMPUTER_HASH | Known |
| 0x800C | C -> S | 32 | 4 | GAME_CMSG_HEARTBEAT | Handled |
| 0x800D | C -> S | 34 | 4 | GAME_CMSG_PING_REQUEST | Handled |
| 0x8012 | C -> S | 12 | 8 | GAME_CMSG_QUEST_REQUEST_INFOS | Known |
| 0x803D | C -> S | 442 | 28 | GAME_CMSG_POSITION_UPDATE | Handled |
| 0x803E | C -> S | 1 | 16 | GAME_CMSG_MOVE_TO_COORD | Handled |
| 0x8047 | C -> S | 31 | 16 | GAME_CMSG_LAST_POS_BEFORE_MOVE_CANCELED | Handled |
| 0x8051 | C -> S | 3 | 12 | GAME_CMSG_INTERACT_GADGET | Known |
| 0x8060 | C -> S | 6 | 12 | GAME_CMSG_CHAR_CREATION_CHANGE_PROF | Handled |
| 0x8063 | C -> S | 1 | 4 | GAME_CMSG_SKIP_CINEMATIC | Known |
| 0x8084 | C -> S | 35 | 12 | GAME_CMSG_ITEM_UPDATE_EQUIPPED_COLOR | Known |
| 0x8088 | C -> S | 36 | 4 | GAME_CMSG_INSTANCE_LOAD_REQUEST_SPAWN | Handled |
| 0x8089 | C -> S | 2 | 4 | GAME_CMSG_CHAR_CREATION_REQUEST_PLAYER | Handled |
| 0x808B | C -> S | 1 | 56 | GAME_CMSG_CHAR_CREATION_CONFIRM | Handled |
| 0x8090 | C -> S | 37 | 24 | GAME_CMSG_INSTANCE_LOAD_REQUEST_PLAYERS | Handled |
| 0x8091 | C -> S | 37 | 12 | GAME_CMSG_INSTANCE_LOAD_REQUEST_ITEMS | Handled |
| 0x8093 | C -> S | 5 | 8 | GAME_CMSG_INSTANCE_QUERY | Observed |
| 0x80B1 | C -> S | 6 | 24 | GAME_CMSG_PARTY_TRAVEL | Handled |
| 0x80C1 | C -> S | 53 | 12 | GAME_CMSG_TRAVEL_INIT | Observed |

## What the list is for

- `not in our table` - a header the client or server really used that this tree cannot name. Each is a row to
  identify, and the captures that carry it are named in `recordings/index.jsonl`.
- `Unnamed` - a declared row with no name yet; `Known` - named, nothing acts on it; `Handled`/`Emitted` - this
  tree answers or sends it.
- A message being unrelated to a feature is not a reason to leave it out of this table.

## 28 - The game headers still to be named, and the test that names each

> Moved here on 2026-11-06; it was the file `plans.md`.

Stage 3 of the owner's staging (`docs/HANDOFF.md`, "The staging"). This page is the working list: every
game header a real session used whose row in `crates/asura-protocol/src/packets.rs` has no name, what the
committed captures prove about it, and the one action that would name it.

Written 2026-11-01 after the offline pass closed. Fourteen headers were named in that pass (`23de10e`,
`d17a60a`), and `0x8093` was named `GAME_CMSG_INSTANCE_QUERY` from a world-map capture after it; the
seventeen below are what is left, and every one of them needs a provoked capture.

## How a name is earned here

A name is a measurement, not a guess (`AGENTS.md` #1). It needs two witnesses that agree:

1. **the wire**: which capture, which sequence number, what the payload's fields are, which ids or values
   they carry, and which messages it always arrives between;
2. **the function**: either the client's own code naming it (a handler's asserts, read through a source
   that is cited by file and line - `docs/adr/0011-rurik-is-a-source.md`) or the player making the
   feature happen while the capture runs.

Where the first exists and the second does not, the row keeps its evidence and stays `Unnamed`. That is
not a failure: it is the difference between "we know the shape of this message" and "we know what the
game calls it", and writing the second without the first is how a wrong name gets into a table and stays
there.

## Why this cannot be finished offline

- The message catalogue of this lineage cannot name them. `Rurik schema/messages.json` is imported from
  the same OpenTyria `msgdefs.c` this table came from, and its own provenance note says so - "a
  convenience index of ONE lineage, NOT corroboration" - so it carries the same headers unnamed
  (`"table": "GAME_SMSG_0431"` for the ones that have no name).
- The handler studies are silent or negative on all seventeen: Rurik's `studies/smsgnames`,
  `studies/smsg` and `studies/newopcodes` return **NOT FOUND** for the item and agent rows and say
  nothing at all about the rest. The Eden checkout carries no opcode catalogue: seven `GAME_SMSG`
  mentions in its whole documentation tree, none of them about these.
- The reference server never sends any of them: `recordings/gwps/world/load-stream.log` has no `SEND`
  line for one of the seventeen, so the reference's own sequence cannot place them either.
- The captures are exhausted as evidence of *function*. Each header below has its payload, its
  neighbours and its rate recorded; nothing further can be read out of them.

## The seventeen

`wire` is the size in `packets.rs` (the reference's `max_size`, which for these fixed layouts is the wire
size). Counts are sightings in the committed captures, per file.

| header | wire | sightings | shape the capture proves | arrives between | what would name it |
|---|---|---|---|---|---|
| `0x0035` | 14 | doors 4 | agent id, then two floats whose second is 1.0 (2.472 / 1.0, 1.75 / 1.0) | `0x006D` `NPC_UPDATE_WEAPONS` and `0x00A0`; also after `0x0021` | a weapon or henchman gear change on a body whose block is re-sent |
| `0x0067` | 12 | character_creation 1 | vec2 of two `FLT_MAX` then a zero word | `0x009F` and `0x0100` `CINEMATIC_START` | any cinematic: the mission intro or a scripted scene |
| `0x007C` | 6 | doors 27, and 1 to 2 in each of three others | one dword, 1 in 31 of 32 sightings and 2 in one | `0x00F2` `INSTANCE_LOADED` and `0x007D` | one district entry with the header on a filter alone |
| `0x007D` | 6 | doors 28, and 1 to 2 in each of three others | one dword that reads as a player or agent id (2, `0x3d`, `0x1f`) | immediately after `0x007C` | the same entry; the pair is one declaration |
| `0x008C` | 11 | doors 36, movement 15 | an id (`0x20`, `0x21`, 32, 33), a second number 20-22, a byte 1 | `0x0029` `AGENT_MOVE_TO_POINT` and the tick stream | stand still, then walk, then fight, in one capture |
| `0x00A7` | 14 | movement 354, login-to-portal 38 | three dwords: 9-38, then 1-12, then 1-11; about three per second | between ticks, and next to `0x0021`, `0x0029` | walk 30 s, stand 30 s, fight 30 s |
| `0x00CF` | 10 | doors 9 | two dwords: 10, then 25, 1 or 0 | `0x009F` and `0x009B` / `0x00A3` | press an adrenal skill on a warrior |
| `0x00D0` | 6 | doors 1 | one dword, 10 | `0x0025`, `0x0029` and the tick stream | the same press; the source reading ties the trio to adrenaline spends |
| `0x0107` | 83 | doors 15 | ends in the same block `0x0020` ends with (scale 1.0, 250.0, 1.0, 12.0, name) | always immediately before a `0x0020` create | something appearing in front of the player: a monster spawn or a summoned body |
| `0x011F` | 138 | 20 in each of three loads | mostly zero dwords, a masked name run, then 0x28, 0xF0, 0x3F, 0x3F, 2, 0x16, 0x5F | `0x0118` `GUILD_PLAYER_ROLE` and `0x012E`; followed by `0x012F` | a load capture paired with a dump of `guild_context`/`party_context` right after it: the block is built at init, so a window opened afterwards reads what is already there and proves nothing (`docs/plans.md`) |
| `0x012F` | 562 | 1 per load | zeros, one 0xF1, then a counted string run | after the twenty `0x011F` rows, before `0x0127` | the same dump: it is that block's tail |
| `0x015B` | 10 | character_creation 25 | two identical dwords, an item id twice | between `0x0161` and `0x015A`, per item | one pass through the character-creation screen |
| `0x0168` | 6 | doors 6 | two words (0x31 / 0x81, 0x26 / 0x106) | after an item `0x0161`, before a `0x0020` create | a kill whose drop the player sees |
| `0x0194` | 6 | doors 9, login-to-portal 2 | one dword, always 0 | after `0x016E` and `0x018E` `INSTANCE_LOAD_FINISH` | a load into a new instance with the filter on this header |
| `0x01AE` | 55 | doors 17 | a leading dword 1, then a masked player name and a run of asset ids | after a `0x8090` players request and a ping, before the `0x000F` burst | enter an instance with another player present |
| `0x01BE` | 6 | doors 9, login-to-portal 2 | one dword, always 0 | `0x01B3` and `0x01BF`, before `0x00BA` | the party window, with a henchman added |
| `0x8092` | 122 | first-crossing 48, slow-crossing 24 | a u16 near the instance id, then a run of DAT-shaped ids | with `0x8093` while an instance is chosen or crossed | open the world map, click one destination, close it |

## The test protocol

One action per capture, so the new headers can be attributed to it:

```powershell
python tools/sniff-capture.py --dir both --seconds 90 --out recordings/client/feature/<name>.jsonl
```

- Start the capture, perform exactly the one action, stop it, and name the file after the action.
- `--header` narrows a capture to one header when the question is "did this appear at all"; leave it
  off for a naming run, because the neighbours are half the evidence.
- Read the result back with the counts: a header that appears only in this capture, at the moment of the
  action, is one of the four or five the action named.
- The verdict goes into the row's `name` and `evidence` in `crates/asura-protocol/src/packets.rs`, one
  subject per commit, with `docs/plans.md` updated in the same change.

## What the named fourteen look like, for the pattern to copy

`0x0026` and `0x015E` are the shape of a good row: a capture gives the sightings and the field values, the
client's own handler names the function, and both go into the evidence string with the file and the
sequence number. `0x016E` shows the weaker case - the source reading names the function and the capture
confirms the shape, so the status is `Inferred` and the evidence says which half is which. `0x009C` was
that case and is `Emitted` since this tree sends it.

## Not this list, and deliberately

- The 306 unnamed game rows no capture contains (323 unnamed game rows in
  `crates/asura-protocol/src/packets.rs`, 17 of them on this page): they need a stimulus, not an
  observation - one opcode on the wire and the client's reaction read (`docs/plans.md`, P1).
- The client-to-server headers this tree frames but ignores (`0x0084` 35 per live session, `0x0012` 12,
  `0x0051` 3): they are named already and belong to stage 4, `docs/plans.md`.

## 29 - The initialization capture: what to run, and what it answers

> Moved here on 2026-11-06; it was the file `plans.md`.

The initialization phase of the client is not built on demand. A live server fills its structures
once, as the instance is entered - the agent array, the party, the instance, the item tables, the
guild and town-alliance block - and a window opened afterwards reads what is already there. So a
window proves nothing about the messages that built it, and the measurement has to be the stream
and the structures taken at the same moment.

This page is the run sheet for that. It is the input to stage 3 (`docs/HANDOFF.md`) and its
working list is `docs/plans.md`.

## The two halves

| half | instrument | what it gives | what it needs |
|---|---|---|---|
| the stream | `tools/sniff-capture.py` | every message both directions, with seq, time, header, size and bytes | elevated shell, 32-bit Python, the library root |
| the structures | `tools/context-dump.py` | the contexts as the client holds them, one JSONL record each, names hashed | 32-bit Python, the library root; **read-only**, no elevation |

Both resolve the library root the same way (`tools/sniff-capture.py:89`, `--py4gw-root`,
`PY4GW_STEALTH`), so there is nothing to configure twice. The dump connects with
`game_thread=False`, which the library documents as the read-only connection, and refuses to fall
back to the writing one.

## A gap is read from `tick`, never from `t`

Each record carries two clocks and only one of them is the packet's. `t` is stamped when this tool's
drain loop reads the entry out of Py4GW's ring, so every record in one drain shares a late time, and a
stall in the loop hides a gap. `tick` is the client's own counter at the hook, taken from the entry
itself. **A duration is a difference of `tick`**, and the summary record carries `tick_first` and
`tick_last` so the unit is read off the file: span over `seconds`. Two readings of one travel bug were
wrong because they were taken from `t` (2026-10-31); the same file read from `tick` gave 7.0 s and
0.90 s for two crossings the drain clock could not tell apart from a 0.05 s step.

## One command

`.\tools\live-session.ps1 -Preset init` is the run below, driven: it relaunches itself elevated once,
starts the raw both-direction capture, dumps the structures on a cadence (ten seconds for this preset -
`tools/live-session.ps1:78`, default documented at `:17` - and only the contexts an instance entry
changes), prints the six actions, and records a `MARK` line every time Enter is pressed in its console.

The six actions for this preset are the ones a client performs anyway and nobody has to stage: enter an
outpost from character select, travel to the explorable it exits to, come back, change district, open
the world map and close it, walk to a portal and cross it. Falls back to the manual steps below when a
particular run needs a different shape.

## The run, by hand

Terminal A, elevated (the sniffer patches the client, the dump does not):

```powershell
# 1. start this BEFORE the character enters the instance, and leave it running
python tools/sniff-capture.py --dir both --seconds 600 --raw --out D:\captures\init-148.jsonl
```

`--raw` keeps every byte, which is what the field work needs, and it also keeps names and UUIDs:
that file stays off this repository. `--dir both` and no `--header` are deliberate; the neighbours
are half the evidence.

Terminal B, a normal shell:

```powershell
# the client is logged in and standing at character select, or in an outpost
python tools/context-dump.py --label before-load

# 2. now walk the character into the instance under test (outpost 148)
#    wait about ten seconds after the load settles, then:
python tools/context-dump.py --label after-load-148

# 3. open the world map, close it, and dump again
python tools/context-dump.py --label after-worldmap

# 4. travel to a second instance (146), wait ten seconds, and dump again
python tools/context-dump.py --label after-travel-146
```

Then stop terminal A with Ctrl+C. Four dumps and one capture, in the order above.

## What each file answers

- **`before-load` against `after-load-148`** is the initialization, field by field: whatever changed
  was set by that load, and whatever did not change was set earlier (login) or never set at all.
  A field that appears in the first dump and not the second is a field this server will have to
  fill some other way.
- **`after-worldmap`** separates the map panels' own state (`world_map_context`,
  `mission_map_context`) from the instance's. The exploration grid and the instance table live here.
- **`after-travel-146`** shows which fields a *second* load rewrites and which ones persist, which is
  what tells a per-instance structure from a per-character one.
- **The capture** carries the messages, in order, with the timings that place each one inside the
  load.

## What I do with it

1. For each context, the fields that changed across the dumps are the target list; the capture gives
   the messages that arrived in the same window.
2. A field is tied to a message by value: an agent's model id, level, profession and name-hash in
   `agent_array` against the fields of `0x0020`, `0x0056`, `0x0057`, `0x006D`, `0x009B`, `0x00A6`,
   `0x009A` in that window. That is the two-witness rule `docs/28` states, with the structure as the
   second witness instead of a window.
3. What that produces, in order: the agent-array field map (the ten messages this tree never sends),
   then items (`0x015E`, `0x013A`), then the guild and town-alliance block (`0x011A`, `0x011B`,
   `0x011C`, and the twenty rows of `0x011F` with their `0x012F` tail - `guild_context` carries
   `TownAllianceStruct` and `GuildHistoryEventStruct`, which is the structure that block fills).
4. Each finding lands as one commit: the row in `crates/asura-protocol/src/packets.rs` with its
   evidence, and the order and set recorded in `docs/plans.md`.

## What this does not settle

- The 306 game rows no capture contains (of 323 unnamed game rows in
  `crates/asura-protocol/src/packets.rs`): they need a stimulus, one opcode at a time
  (`docs/plans.md`, P1; the arithmetic is `docs/plans.md`).
- The map's population itself - which body stands where, with which model and name - is data, not
  protocol. The capture says what a live server sends for it; producing it for our own maps is the
  archive job, and the reason the agent array cannot simply be copied by hand.
- The contexts are the instrument's; nothing from it enters this tree as a file, offset or table
  (`docs/adr/0013-py4gw-stealth-is-our-link-to-the-client.md` #4). What is committed here is the
  dump under `recordings/asura/world/`, names hashed, and the field map derived from it.

## World content and scripting

> Moved here on 2026-11-06; it was the file `plans.md`.

Design discussion recorded 2026-11-01. This is a planning document, not an implemented subsystem.
The source for the decisions below is the owner's instructions in that discussion, not a claim
about the live game's internal design.

## Chosen: Rust simulation and Lua content scripts

The owner selected **Lua** for content scripting on 2026-11-01, replacing Rhai after reading both:
"Seems too new, not much documentation and examples and real world usage. I think ill do LUA
instead." Contributors should be able to edit text files and run them through ASURA without
installing a separate scripting interpreter or rebuilding the server for each content change. Lua is
intended to be embedded in the server; it has not been added to Cargo or the runtime yet.

What the change carries, recorded so it is not discovered later:

- the build gains a C toolchain. A vendored Lua compiles from C source: 50.6 s cold build and 19.8 s
  incremental in a scratch crate on 2026-11-01, where the seven member crates in `Cargo.toml`
  (`asura-auth`, `asura-world`, `asura-webgate`, `asura-protocol`, `asura-db`, `asura-fakeworld`,
  `asura-supervisor`) are all pure Rust today;
- the sandbox becomes ours to assemble: no `io`, `os`, `debug` or `package`, no bytecode loading, an
  instruction hook and a memory ceiling. Lua supplies the parts; the policy is ours;
- quests gain suspension. Coroutines let a quest wait mid-flow, so the handler shape proposed below
  is one option rather than the only one.

The owner already has behavior-tree AI in Py4GW Reforged. Its game decisions are to be authored
in external Lua scripts; Rust supplies the behavior execution machinery and world operations.
BehaviorTree.CPP is the selected execution reference for the Rust node runtime; the HeroAI port
must handle the differences recorded in `docs/adr/0014-behavior-tree-execution.md`. That port has
not started. No external project becomes a runtime dependency; the dependency rule still applies.

## Simulation-wide boundary: mechanics and behavior

The owner's distinction applies to the entire game simulation, not only skills. External
scripts define game behavior: what happens, when, to whom, under which conditions, and what
happens next. The server implements mechanics: the supported operations that execute those
rules and change authoritative world state. Scripting is the primary place to author game
behavior, not an optional layer for choosing among complete hardcoded implementations.

Scripts can contain conditions, calculations, sequences, and event reactions. The boundary is
not simply data versus code: a script describes and requests behavior using supported server
operations; the server executes those operations. The server also enforces the constraints of
its mechanics. A script cannot create an operation the server does not provide.

| Domain | External scripts define behavior | Server mechanics execute |
|---|---|---|
| NPCs and AI | Who to attack, heal, follow, avoid, or help; priorities and conditions | World queries, behavior execution, and supported actor actions |
| Movement | Where and when to move, patrol routes, chase and retreat decisions | Pathfinding, movement, collision, and position updates |
| Combat and skills | Target rules, costs and timings, effects, reactions, and skill functionality | Resource accounting, cast scheduling, projectiles, damage/effect operations, and death state |
| Quests and missions | Objectives, prerequisites, progression, branches, success, and failure | Quest-state storage, event delivery, and supported state/reward operations |
| Spawns and encounters | Which actors appear, where, when, and how encounters progress | Actor creation/removal, placement, and lifecycle operations |
| Loot and rewards | What is awarded or dropped, to whom, under which rules | Supported random draws, item creation, inventory/currency changes, and persistence |
| Dialogue and interactions | What an NPC says, offered choices, conditions, and consequences | Interaction delivery, supported client messages, and world operations |
| Maps and instances | Encounter setup, entry reactions, scripted transitions, and completion rules | Map loading, instance lifecycle, placement, travel, and geometry operations |

These rows describe responsibilities, not existing APIs or claims that these mechanics have
already been implemented. The same boundary applies to other game behavior beyond this list.
For example, a script can declare an ambush after a quest checkpoint and a reward after the
ambushers die; the server must provide the checkpoint events, spawning, death events, and
reward operations. If the server offers no flight mechanic, scripts cannot make actors fly.

Rust retains the execution machinery, authoritative world state, persistence operations, and
packet construction. The selected Rust behavior-tree runtime executes behavior; it does not
require game-specific decisions to be hardcoded in Rust. Existing HeroAI decisions are behavior
to expose through the external scripting boundary, while the runtime remains a server mechanic.
The exact connection between scripts and that runtime is still open.

This simulation-wide distinction was clarified by the owner on 2026-11-02. The API, its names,
script formats, and event contract remain unselected. This records the intended boundary and
authorizes no runtime implementation. Skill examples and the Rurik review are in
[35-skill-mechanics-and-behavior.md](35-skill-mechanics-and-behavior.md).

The packet-stream objective remains binding for reproduction of the base game. The owner's wish
to support more complex custom content is a future capability, not permission to change existing
game behavior or invent protocol messages.

## HeroAI and custom skill behavior

HeroAI's skill knowledge and game decisions are behavioral configuration supplied in external
Lua: skill selection, use conditions, target rules, priorities, combinations, resource-
management decisions, coordination, and build-specific tactics. A profile's author edits this
configuration, including for custom skills, without modifying or rebuilding Rust when the
required mechanics already exist. A skill's behavior configuration and HeroAI's rules for using
it are separate entries, both supplied by the selected profile.

The division is fixed (owner, 2026-11-02): **Rust owns the technical layer** - the behavior-tree
runtime and its tick, node kinds and execution semantics, per-agent tree state, halting and
reset, actor-relative queries, wrapped actions and calls, budgets, and error policy. **Lua is the
configuration layer**: it declares the behavioral configuration and constructs the tree from
Rust-provided node kinds and wrapped leaves. Lua does not tick, does not hold running node state,
and does not execute per-tick logic. Tree construction is configuration; execution is a Rust
mechanic. The Lua binding shape and the configuration format remain open.

A later acceptance case must add a custom skill using existing mechanics, define its behavior
and HeroAI usage in a copied profile, and demonstrate the NPC using it without editing or
rebuilding Rust. The original profile must remain unchanged. A new unsupported mechanic still
requires engine work; an unfamiliar skill alone is not a reason to hardcode its behavior.

## Behavior-tree class review (2026-11-01)

**Finding: the class supplies reusable behavior compositions and HeroAI migration evidence.**
BehaviorTree.CPP is the target execution reference, as decided in
`docs/adr/0014-behavior-tree-execution.md`; the Python runtime is not a specification to port
verbatim. The review below records its current rules so the later port can account for differences.
It establishes no instance scheduling, server combat mechanics, or capacity result for thousands
of instances. This is a source review, not a port or an executed simulation test.

The source inspected is Py4GW Reforged at commit
`a5fd2e54d504605200a752b1bc991d618ed33c41`, retrieved from the repository's `main` tree.
The file paths and line numbers below refer to that commit. The source is the only witness for
these execution details; no live-client behavior is inferred from them.

- [BehaviorTree.py](https://github.com/apoguita/Py4GW_Reforged/blob/a5fd2e54d504605200a752b1bc991d618ed33c41/Py4GWCoreLib/py4gwcorelib_src/BehaviorTree.py).
- [HeroAI/headless_tree.py](https://github.com/apoguita/Py4GW_Reforged/blob/a5fd2e54d504605200a752b1bc991d618ed33c41/Py4GWCoreLib/HeroAI/headless_tree.py).

### What one tick actually visits

The owner's description of walking all nodes has two distinct parts: the wrapper walks the whole
currently exposed tree to assign the blackboard, but executing the tree does not unconditionally
execute every node. The implementation, rather than its descriptive comments, gives these rules:

| Source in `Py4GWCoreLib/py4gwcorelib_src/BehaviorTree.py` | Observed behavior |
|---|---|
| `NodeState`, line 85 | Three outcomes: running, success, failure |
| `_propagate_blackboard`, line 2045; `tick`, line 2054 | Every wrapper tick seeds client context, recursively assigns the blackboard, then ticks the root |
| `SequenceNode._tick_impl`, line 608 | Resumes at the saved child index; advances through immediate successes in the same call; stops at running or failure |
| `SelectorNode._tick_impl`, line 687 | Resumes at the saved child index; advances through failures in the same call; stops at running or success |
| `ChoiceNode._tick_impl`, line 764 | Starts from the first child each call; stops at the first non-failure result |
| `ParallelNode._tick_impl`, line 1232 | Ticks children sequentially in the calling thread; stops immediately on failure; successful children are ticked again while the parent remains running |
| `WaitNode._tick_impl`, line 1460 | Polls its callback whenever reached |
| `WaitUntilNode._tick_impl`, line 1558 | Checks its timeout and throttle; skips the callback until its check interval is due |
| `SubtreeNode`, lines 1311 and 1348 | Constructs a subtree lazily, ticks its root directly, and discards the subtree on reset |

For example, a sequence containing an already-successful condition followed by a running movement
action resumes the movement action on its next tick. It does not recheck that condition until the
sequence resets. A choice reevaluates earlier priorities instead. This distinction affects combat
guards and interruption, so a port must not silently turn one node kind into the other.

One additional timing rule matters: `ActionNode._tick_impl` (line 448) returns running when its
callback first finishes, even with `aftercast_ms = 0`. It returns the saved final result on a later
tick. A port that returns success immediately changes the sequence's timing.

`HeroAI/headless_tree.py:260` calls the tree after its update and initialization checks; its tree
construction at line 278 combines alive/distance/knockdown guards, a casting check, and a selector
for looting, out-of-combat actions, following, and combat. This is evidence of the class's existing
composition, not proof that client automation leaves implement authoritative server NPC behavior.

### Fit with the planned simulation

- Rust nodes follow the selected execution reference while the HeroAI port explicitly maps its
  source control flow to those rules. Leaves query and change ASURA's own agent state. Client
  globals, player-only context seeding, shared-memory commands, ImGui, and
  client action queues do not provide the server's world API (`BehaviorTree.py:57`, `:2024`;
  `HeroAI/headless_tree.py:1`, `:130`, `:165`).
- Every agent needs separate execution state: child indices, action flags, timers, lazy subtrees,
  and its blackboard are mutable (`BehaviorTree.py:127`, `:436`, `:593`, `:672`, `:1308`, `:1555`).
  Sharing immutable definitions is a possible Rust representation; sharing these mutable node
  objects between agents would mix their progress. The representation is not selected yet.
- Lua configures game-specific behavior, including escort/follow decisions and combat-tree
  structure; Rust executes the generic runtime and requested mechanics. Quest configuration can
  reuse scripted behaviors and react to outcomes. The configuration format is not selected.
- The instance scheduler decides which agents run and when. The class only processes a tick when
  called (`BehaviorTree.py:2054`); neither its running state nor its wait nodes automatically park
  work until an event. Turning polling into event subscriptions would be a separate design change.

### Scaling limits to retain in the plan

The wrapper's blackboard pass visits all currently exposed nodes before selective execution. A
tree with N exposed nodes therefore has an O(N) preparation pass on every wrapper tick, even when
only one action is running (`BehaviorTree.py:2045`). The target runtime supplies node context
without reproducing that assignment walk, following ADR 0014. The Rust representation is open;
the Python class is not being changed.

The reached nodes also collect timestamps and metrics on every call (`BehaviorTree.py:149`).
Wait throttling reduces callback work, not the cost of reaching that wait. A repeater can process
many immediately completing repetitions in one call (`BehaviorTree.py:933`). These costs and
worst-case work need measuring with representative trees; a Rust translation alone establishes
no throughput figure.

Reset and interruption need a server contract: clearing node state does not itself cancel a move,
cast, or other world action already requested (`BehaviorTree.py:234`, `:488`, `:755`). Reducing tick
frequency or adding wakeups also changes reaction timing unless that behavior is preserved.
Those policies, the simulation clock used by waits, and per-agent work budgets remain open.

The review supports reuse of HeroAI's behavior intentions and compositions, with execution
differences handled through ADR 0014. It does not establish support for 1,000 simultaneous populated
instances; that needs combat, pathfinding, memory, and scheduling measurements after an
implementation exists.

## Proposed: event-driven scripting

The discussion favors short handlers that react to events and request existing Rust actions.
The owner's final wording was "possible event driven scriptting"; the event model remains a
proposal rather than a settled implementation contract.

Candidate events discussed:

- A quest is accepted, advances, completes, or fails.
- An NPC interaction or dialogue choice occurs.
- An enemy dies or a named group is defeated.
- A character enters an area or an escort reaches a checkpoint.
- A relevant item is obtained or inventory changes.
- A timer expires.

These are content concepts, not registered event names or measured live-client triggers.
Delivery order, handler scheduling, and failure behavior are open.

Example sequence, independent of API syntax:

1. Accept the escort quest and start an existing escort behavior.
2. Reach a checkpoint and spawn an ambush group.
3. Defeat the group and let the escort continue.
4. Reach the destination and complete the objective.

Rust handles the walking and fighting throughout. A companion that follows the party instead of
leading it uses a different available behavior; it does not require scripts to calculate movement.

## Quest content

The owner described ordinary quests as combinations of these requirements:

| Requirement | Content describes | Rust support needed |
|---|---|---|
| Fetch or loot items | Item identity and required count | Inventory queries and relevant events |
| Kill a boss or pack | Enemy or group identity and completion condition | Death events and group tracking |
| Reach a destination | Area or checkpoint and conditions | Area-entry or checkpoint detection |
| Escort an NPC | Route, checkpoints, success and failure conditions | Movement, behavior trees, and combat |
| Have an NPC accompany the party | Companion identity and when it follows or leaves | Following and combat behavior |
| Require earlier progress | Prerequisites and follow-up quests | Quest-state queries and updates |

Standard objectives can be combined into sequences, requirements that all must be met, or
alternative ways to succeed. Lua provides room for special branches and sequences beyond those
ordinary combinations. A new sequence of existing actions is distinct from a new engine mechanic.

Main quests, secondary quests, and quest chains share these concepts. An NPC's existence, movement,
and available quests are separate: a roaming NPC can offer a quest, and a permanent villager can
offer different quests according to progress.

Whether ordinary objectives are authored as data, Lua, or both is not decided. JSON was discussed
as a possible data format, not selected as the storage format. A map-canvas editor and state
graphs were discussed as authoring approaches, not approved implementation work. The initial
map/portal editing screen is part of the handoff in `docs/plans.md`.

## Map spawns and layouts

The content design discussed separates the following concepts. These are planning terms, not
existing schema tables or Rust types.

| Concept | Meaning |
|---|---|
| Agent definition | Model, faction, stats, available services, and assigned behavior |
| Placement | Position and facing, or a permitted area in which to spawn |
| Group | Agents belonging to a pack, patrol, reinforcement wave, or ambush |
| Movement assignment | Standing, roaming within an area, or following a route |
| Spawn conditions | Always present, quest-dependent, or created after an event |
| Lifetime | Despawn conditions, whether death persists, and any respawn delay |
| Quest relationships | Quests an NPC offers or accepts, subject to availability conditions |

Examples covered: a map's normal enemy population, roaming service NPCs, side-quest givers,
quest companions, checkpoint ambushes, and reinforcements after a boss dies.

Layouts can be authored as exact placements, spawn areas, or named population sets such as a
peaceful village and an occupied village. These are available approaches, not a decision to use
random placement or replace measured base-game layouts.

Phasing means changing the population or visibility according to progress. Layering means separate
populations of a location. Neither follows automatically from selecting Lua. Character, party,
and instance state have different scopes; the population-selection policy is not decided.

## Open questions

- Do ordinary quests use objective definitions, Lua handlers, or both?
- Does possessing an item satisfy an objective, or must it be obtained during the quest?
- Does an explorable population follow the leader, combined party quests, or a per-quest rule?
- Which progress survives logout, and which belongs only to the running encounter or instance?
- How are separate instances, phases, and layers identified and selected?
- What game actions and state queries does the scripting API expose?
- How are events ordered, handlers scheduled, and duplicate triggers handled?
- What happens after a script error or execution-limit failure, including actions already applied?
- What execution, data-size, and access limits apply to scripts and Rust callbacks?
- When does changed content take effect, and what happens to active quests during reload?
- Which runtime version, Cargo features, content format, and packaging are used?

Script execution cost is now measured for one workload: 75 ns per decision in Lua 5.4 against
1,027 ns in Rhai on the same loop, release build, 2026-11-01. Memory per instance, debugger
usability, Windows/Linux packaging effort, and reload behavior are still unmeasured, and this
document makes no claim about them.

## Current code and reading

- `crates/asura-world/src/instance.rs`: the current instance, player slots, movement tick, and
  registry keyed by map id. It does not establish the future multi-instance or layering policy.
- `crates/asura-world/Cargo.toml`: no scripting runtime dependency at the time of this discussion.
- `docs/22-world-model.md`: game nouns and the owner's existing world-model decisions.
- `docs/02-architecture.md`: service ownership and the content database's architectural role.
- `docs/adr/0014-behavior-tree-execution.md`: selected execution reference and HeroAI node mappings.
- `docs/plans.md`: headless combat policy, client assumptions, and the
  100-instance / 200-agent scaling review.
- `docs/plans.md`: detailed implementation handoff, owner intent, the early
  map/portal manager, seven phases, acceptance checks, and questions to ask before dependent work.
- `docs/plans.md`: the expanded panel for server operations, capture sessions,
  guided tests, recordings, and configuration/content management (owner, 2026-11-02).
- [mlua, the Lua binding for Rust](https://docs.rs/mlua/latest/mlua/).
- [The Lua 5.4 reference manual](https://www.lua.org/manual/5.4/): a debug hook counts instructions,
  and an instruction count does not bound the time spent inside a Rust function.

This records the selected language and the design under discussion. It does not authorize a
runtime integration, AI port, schema migration, content conversion, or change to live behavior.

## HeroAI headless: server reuse and scaling

> Moved here on 2026-11-06; it was the file `plans.md`.

Source review, 2026-11-01. The owner's workload is 100 instances with 200 AI-controlled agents in
each: 20,000 independently controlled agents. The owner asked whether the client-side headless
HeroAI can supply their combat decisions through Lua scripts backed by Rust mechanics.

## Finding

HeroAI supplies substantial reusable combat decision logic. The owner clarified that the port
uses ASURA's own state and combat systems: client support structures are outside its scope.
Their replacement is assumed, not a feasibility objection. Capacity for 20,000 agents remains
unmeasured. The small behavior tree is orchestration; skill selection, target searches, state
queries, and the work requested by those decisions are the larger body of work to examine.

The proposed port expresses the decision rules in Lua against ASURA's agent state and action API.
Skill knowledge, target conditions, priorities, combinations, coordination, and build-specific
tactics are editable behavior supplied by the selected profile, including for custom skills.
They must not require Rust changes when the needed mechanics already exist. The port needs
actor-relative context, server-owned combat mechanics, and the execution mappings in
`docs/adr/0014-behavior-tree-execution.md`. This review adds no implementation or runtime policy.

## Source and limits of this review

Inspected Py4GW Reforged commit `a5fd2e54d504605200a752b1bc991d618ed33c41`:
[repository tree](https://github.com/apoguita/Py4GW_Reforged/tree/a5fd2e54d504605200a752b1bc991d618ed33c41).
Paths below are relative to that tree. All observations are from source inspection; the source
is the only witness. No source code was executed, no client capture was made, and no CPU or
memory benchmark was performed for this workload.

The review follows the headless wrapper, generic fallback combat evaluator, targeting helpers,
cache/party data, interrupt observer, and build-selection dispatch. It does not establish complete
skill coverage, validate the correctness of the combat rules, or review every profession's skill
definitions and every specialized build implementation.

## Actual decision path

| Stage | Source | Work performed |
|---|---|---|
| Headless tick | `Py4GWCoreLib/HeroAI/headless_tree.py:255`, `:260` | Processes control messages, resurrection-scroll upkeep and cache refresh, then initialization and tree execution |
| Initialization | `Py4GWCoreLib/HeroAI/headless_tree.py:231` | Checks client/map/party state, assigns cached data, and refreshes combat data before ticking the tree |
| BT orchestration | `Py4GWCoreLib/HeroAI/headless_tree.py:278` | Selects looting, out-of-combat work, following, or combat, behind guards and a casting wait |
| Build dispatch | `Py4GWCoreLib/Builds/Any/HeroAI.py:90`, `:169`, `:237` | Resolves a build from professions and skillbar, retains coroutine progress, and dispatches to that build or generic fallback |
| Generic combat | `Py4GWCoreLib/HeroAI/combat.py:1880`, `:2120` | Scans up to eight prioritized slots, chooses the first usable skill/target, or attempts auto-attack in combat |
| Cast checks | `Py4GWCoreLib/HeroAI/combat.py:1592` | Rejects cooldown, casting, adrenaline, energy, and other failures before expensive target resolution at `:1686` |
| Target and effect checks | `Py4GWCoreLib/HeroAI/combat.py:772`, `:1045`, `:1065` | Chooses suitable enemies/allies and evaluates skill-specific conditions |
| Issue action | `Py4GWCoreLib/HeroAI/combat.py:2169` | Requests skill use through the client skillbar wrapper |

Build selection is not a full registry search on every unchanged tick: a matching signature reuses
the resolved build (`Py4GWCoreLib/Builds/Any/HeroAI.py:99`). The signature itself is checked during
execution. Build discovery and per-registry runtime build instances also have caches
(`Py4GWCoreLib/BuildMgr.py:2112`, `:2173`, `:2201`). A generic-only benchmark would not measure
the specialized builds selected by the real dispatch.

The AI decides what to attempt. Calling `UseSkill` does not implement authoritative damage,
healing, effects, resource consumption, cast completion, or attack resolution. Those mechanics
remain server simulation work. The custom skill definitions include decision conditions and
coordination flags (`Py4GWCoreLib/HeroAI/custom_skill_src/skill_types.py:7`, `:103`); that is not
proof of a complete implementation of the game's skill effects.

## Assumed replacement of client support structures

These source dependencies identify where decisions obtain their inputs today. The port supplies
those inputs from the server; it does not reproduce shared memory, global caches, or account
structures. The table is a mapping aid, not a list of barriers to server feasibility.

| Assumption and evidence | Consequence for server reuse |
|---|---|
| `GameData` and `CacheData` are process singletons (`Py4GWCoreLib/HeroAI/cache_data.py:22`, `:89`) | Creating 200 wrappers would not create 200 independent actor contexts |
| Decisions repeatedly use `Player.GetAgentID`, position, target, and the local skillbar (`Py4GWCoreLib/HeroAI/combat.py:96`, `:772`, `:1592`) | Every query needs an explicit controlled agent and instance; enemy/ally relationships must be relative to that agent |
| Energy comes from a party shared-memory record and returns -1 when unavailable (`Py4GWCoreLib/HeroAI/utils.py:95`) | Ordinary server NPCs cannot depend on account slots to obtain their own energy |
| Party and effect queries use shared account records, with fallbacks for incompletely observed entities (`Py4GWCoreLib/HeroAI/party_cache.py:52`; `combat.py:1017`; `utils.py:115`) | Read authoritative state through an actor context; preserve or explicitly adapt client-visibility assumptions |
| Casting is inferred by a per-frame observer, and feasibility includes ping (`Py4GWCoreLib/HeroAI/interrupt.py:205`, `:648`, `:697`) | Server cast state can answer these queries directly, but the NPC's allowed knowledge and reaction delay are explicit behavior decisions |
| Following uses account assignments, shared-memory commands, and client controls (`Py4GWCoreLib/HeroAI/follow/follower_runtime.py:5`, `:143`) | Reuse relevant follow intent against Rust movement; account control is not an NPC movement interface |

The server's more complete information must not silently make NPCs react instantly or know more
than their intended policy allows. Replacing observation and network compensation with server
state requires a recorded timing/perception decision, especially for interrupts.

## Scaling risks found in the source

These are structural costs and benchmark candidates, not measured rankings of CPU consumption.

### Repeated skill preparation

`CacheData.Update` throttles part of the client-cache refresh with a default 75 ms interval
(`cache_data.py:144`, `:282`). `UpdateCombat` is separate: it calls `CombatClass.Update` and
`PrioritizeSkills` without that throttle (`cache_data.py:255`). Headless initialization calls it
before the tree's casting gate (`headless_tree.py:231`). Build preparation calls it again when
that coroutine path runs (`Py4GWCoreLib/Builds/Any/HeroAI.py:147`).

`PrioritizeSkills` reconstructs eight skill records, then scans them through nature, type, combo,
and remaining-slot passes (`combat.py:347`). The decision data and ordering need not be rebuilt
merely because time advanced if their inputs have not changed. A server representation can retain
an immutable skillbar plan and read current cooldown/resources separately; the invalidation inputs
and preservation of ordering still need to be established during the port.

### Repeated target scans and sorts

The skill-aware clustered-enemy helper scores each candidate by querying nearby enemies twice and
calculating cluster coordinates (`Py4GWCoreLib/HeroAI/targeting.py:320`). Each filtered-enemy query
walks the enemy array (`Py4GWCoreLib/routines_src/Agents.py:178`). If C candidates are considered
from an enemy array of E entries, those inner queries alone perform O(C * E) filtering work for
one clustered-target selection, before other scoring work. Dense cases approach quadratic work.

Weapon-spell targeting also counts nearby enemies for each eligible ally (`targeting.py:432`).
Lowest-health ally targeting constructs, filters, and sorts candidates (`targeting.py:98`).
Nearest-enemy selection sorts its filtered array (`routines_src/Agents.py:198`). These operations
are potentially repeated by many actors and multiple skill candidates.

Spatial indexing and reusable per-instance query data are candidates for the Rust implementation.
Reuse must preserve range, faction, skill-specific eligibility, ordering, and tie-breaking. A
single shared chosen target is not equivalent to per-agent targeting.

### Repeated observation and state lookup

The interrupt observer scans visible enemies every client frame (`interrupt.py:205`, `:745`).
Effect checks can rebuild effect lists and scan shared account slots (`combat.py:1017`, `:1045`).
The server already owns cast and effect state; replicating those client observation loops for each
NPC would duplicate work. Indexed access or versioned caches are approaches to evaluate, with
invalidation and reaction timing defined explicitly.

The custom skill table is already constructed once at module scope (`combat.py:31`). Its
`CustomSkillClass` allocates 3,433 entries (`custom_skill.py:19`); this is table capacity, not a
verified supported-skill count. The server can likewise share immutable profile skill definitions while
keeping skill cooldowns, selected targets, cast progress, and behavior state separate per agent.

## What 100 instances with 200 agents means

The active-agent count is 20,000 if all 200 agents in every instance require decisions. For a
decision frequency f, the workload is 20,000 * f agent decisions per second. Frequency is not
selected here; BT decision frequency, movement integration, cast deadlines, and network updates
are different concerns.

Illustrative arithmetic at 10 decisions per second, or 200,000 decisions per second:

| Average CPU time per agent decision | Aggregate CPU-seconds required each second |
|---|---|
| 10 microseconds | 2 |
| 50 microseconds | 10 |
| 100 microseconds | 20 |

These are examples, not measurements or required rates. They exclude other server work and spare
capacity. Bursts and slow individual instance updates matter as well as averages. Serving 100
independent instances allows distributing work across cores and world processes; no capacity per
machine is established by this source review.

Keep all 200 agents active for a worst-case combat measurement. Dormant-agent scheduling is a
separate potential saving, not a reason to omit that case or freeze distant quest participants.

## Port and validation boundary

The source supports proceeding with a port of HeroAI's combat policy against server state and
actions, following the node mapping in ADR 0014. Its searches, preparation work, and scheduling
still need measurement before claiming efficient execution for 20,000 agents.

When implementation is authorized:

1. Express the selected generic and specialized build decisions in profile-owned Lua scripts
   using Rust state queries and actions, including faction, party, cast timing, and coordination
   scope. Keep game-specific conditions and tree composition externally authorable.
2. Preserve decisions while replacing repeated client observation and preparation work; apply
   ADR 0014 to node lifecycle differences.
3. Measure one 200-agent instance with varied skillbars, dense area targeting, healing, effects,
   interrupts, and movement. Distinguish idle and all-active combat cases.
4. Measure concurrent instances, through 100 when resources permit, recording decision/update
   latency, missed deadlines, CPU, memory, path searches, and outgoing traffic.
5. Add a custom skill and its HeroAI usage rules within a copied profile, using existing
   mechanics, and demonstrate the NPC using it without modifying or rebuilding Rust.
6. Validate gameplay and the packet stream with a real client. Synthetic timing alone does not
   establish parity, complete combat behavior, or acceptable reaction timing.

No new benchmark harness or external runtime dependency is introduced by this review.

## ASURA control panel: operations, research, tests, and content

> Moved here on 2026-11-06; it was the file `plans.md`.

Design draft, 2026-11-02. The owner expanded the requested manager beyond map content editing:
it must start/stop and handle the server, launch and manage Py4GW Stealth sniffing sessions,
read in-game data through Py4GW Stealth, provide the user-facing place to conduct tests, and
serve as the configuration and management console. The owner explicitly confirmed that Stealth
is a major research capability, not merely a packet-capture attachment.

This defines the requested capabilities and a proposed interface/workflow. It does not select
desktop versus browser, a UI framework, remote-host support, or an implementation language for
the panel. Those choices remain questions before dependent implementation.
The simulator remains Rust, content scripting remains Lua, and the server start path remains
asura-supervisor. No process, capture, test, or configuration edit was started by writing this.

## Product intention

The owner should be able to answer the following questions from one interface:

- Is my server running, and what needs attention?
- Which client and environment am I recording, and where is the evidence?
- What does the running client actually hold: its map, agents, party, items, skills, and quests?
- What test am I performing, what steps remain, and what happened?
- What configuration/content is installed, and how do I change it correctly?

Commands and protocol details can remain available in expandable details; the main workflow
uses task names, client choices, clear status, instructions, and results. A user should not have
to reconstruct a session from several terminals and unnamed capture files.

The control panel does not replace the game client as the gameplay acceptance test. It guides
the operator, starts the selected tools, records outcomes, and keeps evidence connected.
Selecting a target, starting a capture, and changing content are separate user actions.

Py4GW Stealth integration is a core requested panel capability. Its external runtime remains
optional for operating ASURA itself: a machine without the research rig can still start/stop
and manage the server. Missing research setup disables the affected research actions with a
clear explanation; it does not turn the whole control panel into an unavailable application.

## Requested scope and proposed navigation

| Workspace | User-facing purpose | Initial information and actions |
|---|---|---|
| Overview | Know what is happening now | Service readiness, active captures/test runs, recent results, concrete setup failures |
| Server | Operate ASURA | Start/stop through the supervisor, restart a selected service, inspect status and logs |
| Capture sessions | Record a selected client/environment | Choose client and capture settings, start/stop, take a supported state snapshot, inspect session status and output |
| Live client data | Inspect the running game through Py4GW Stealth | Select a client, browse supported data readers, refresh a view, inspect field results, and save a labeled snapshot |
| Tests | Perform a named test with evidence | Read steps, start the selected capture tasks, mark progress/observations, finish with an explicit result |
| Content and configuration | Maintain the installed world and operator settings | Map/default spawn/portal-arrival editor first; service settings kept distinct from game content |
| Recordings | Find and inspect previous evidence | Filter by subject, target, date, client, build, and capture layer; open the associated run and artifacts |

These are proposed workspaces within one panel, not separate applications or requirements
for separate backend services. A settings dialog or tabs can replace a navigation item if the owner
chooses that arrangement. New NPC/quest editors arrive with their simulator phases, not as
empty controls that pretend those systems already work.

## The central workflow: a test run

A test run is the record joining an intention, a target, the work performed, and its evidence.
A standalone capture or data-inspection session is allowed; the operator does not need to
invent a test or run packet sniffing just to read the client.

Example: investigating alternating click movement and manual movement across a bridge.

1. Open the movement test or create a named run with the relevant instructions.
2. Select the target environment and the actual client process.
3. Choose client packet capture, server wire capture where available, and state snapshots as
   separate evidence channels. The panel shows prerequisites and which channels are ready.
4. Start the requested recording work and perform the gameplay steps in the real client.
5. Mark a checkpoint or write an observation without stopping the recording.
6. Stop cleanly and preserve the files, exit/cleanup status, and notes.
7. Record whether the gameplay issue reproduced. Link another run against live or ASURA to
   perform the same steps for comparison.

The panel must not infer that all proposed channels are required for every test. A simple
targeted recording must remain simple. A live recording does not require starting ASURA.

### Run information

Store enough information to resume review without the original terminal:

- Stable run id, subject, created/start/end times and original clock/timezone information.
- Declared environment, such as live or ASURA; source of that identification and any uncertainty.
- Selected client PID and executable identity available from the tool; do not trust a filename
  alone to establish which server it is connected to.
- ASURA build/world/config identity where known, and the test instructions used.
- Requested capture channels, directions, filters, duration, and actual tool/interpreter identity.
- Artifact paths, capture layer, process outcome, missing/partial output, and cleanup result.
- Operator checkpoints, observations, gameplay verdict, and links to related runs.

Persist the run record so closing or reopening the panel does not erase the history. The storage
format for this record is not chosen yet. Raw recordings stay immutable; notes and indices are
separate from the captured bytes. Reuse recordings/index.jsonl for the public archive instead
of introducing a competing public recording catalogue.

## Capture sessions

### Starting a session

The operator selects a client explicitly, especially when several Gw processes exist. Display
the selected identity and declared target beside Start and throughout the run. Pin packet and
context tasks to that same PID; never silently fall back to another client when it exits.

Check the prerequisites of the chosen channel, rather than requiring every research tool just
to open the panel. Client sniffing requires the external rig, an x86 Python interpreter,
appropriate access to the client, and a compatible client process. Normal server operation
must still work without them.

Create a distinct output location for each run and preserve it on failure. The current capture
tools accept explicit output paths. Do not overwrite a previous capture because a subject name
or the panel's session directory is reused.

Packet capture and periodic state dumps are separate tasks under the session. Preserve channel
failures independently: a working capture plus a failed dump is not an entirely successful run,
but the valid capture must not be discarded.

### Progress and stopping

Proposed session states are Preparing, Needs setup, Starting, Recording, Stopping, Completed,
Failed, and Interrupted. These describe actual tool state, not just which button was pressed.
Show elapsed time, latest activity where available, output location, and actionable errors.

Stop requests must let the capture tool leave its loop, flush, and disconnect. A process exiting
does not prove that hook cleanup succeeded. Record that result separately. If the client exits,
the helper is lost, or forced termination is necessary, preserve partial output and mark the
session interrupted/cleanup uncertain. Reattaching after uncertain cleanup needs a verified
recovery path; do not automatically start another hook on the same client.

ADR 0013 records that conflicting owners of the same hook corrupt the client. Coordinate
research tasks for a client and verify which packet/context operations can coexist before
launching them together. Independent recordings of different clients are a separate capability;
the initial number of concurrent sessions remains a question for the owner.

### Capture layers

Keep these distinctions visible in session details and recording views:

| Channel | Meaning | Valid use |
|---|---|---|
| Client packet capture | Client message structures, normally four-byte headers | Headers, order, triggers, counts, and observed client fields |
| ASURA server capture | Decrypted wire messages, two-byte headers | Wire sizes, layouts, bytes, and outgoing/incoming server behavior |
| State snapshot | Client state at a recorded moment | Relate a packet sequence to what the client actually held |

Use tools/capture-read.py's layer rules. Do not compare client-structure sizes directly with
wire sizes or label the difference a protocol regression. Preserve original timestamps and
client ticks; different streams do not automatically share a clock or comparable timestamp.
Operator markers help correlate actions, but do not manufacture a proven latency measurement.

Follow the existing tool's sanitized/raw distinction and recordings/README.md. A raw session
does not become a public archive entry automatically. Any sanitized derivative is a new file,
not an edit to the raw evidence. The default and available controls need to reflect the source
tool rather than inheriting live-session.ps1's unconditional raw mode by accident.

Server wire capture currently uses startup configuration. If enabling it requires a restart,
show that requirement and let the operator choose it explicitly; a Capture button must not
silently restart a running world or claim a channel is active before output exists.

## Live client data through Py4GW Stealth

The panel needs a proper client-data workspace, not only a button that writes an opaque dump.
This is the other half of the research workflow: packet capture shows what was sent/received;
the data readers show the client's state before and after an action.

### Reader groups and support

Expose supported measurements in understandable groups. The following groups describe the
intended organization, not a claim that every field or interactive browser is already wrapped
by ASURA's tools:

| Group | Data to inspect where the installed readers support it |
|---|---|
| Map and instance | Current map/instance information, position/plane, entry/spawn and portal measurements, pathing |
| Agents | Agent lists, identity/type, position, and available living-agent state |
| Character and party | Character context, party membership, selected targets, and available progression state |
| Items and inventory | Item context, bags/equipment, and available item properties |
| Skills and effects | Skillbar state, available skill properties, casting and active effects |
| Quests and interactions | Available quest, dialogue/service, trade, and interaction context |
| Additional contexts | Other supported client contexts and archive measurements used in the research |

Discover and verify available read operations from the installed rig and the existing measurement
tools. Separate Available, Unsupported, Unavailable in this client state, and Failed. Do not
show zero or an empty list as though a reader succeeded when it did not. The first implemented
groups and their fields need a concrete acceptance case before coding; do not silently promise
the entire external API in the first panel increment.

The panel presents data and measurements. Copy no external source, offset, byte pattern, or
catalogue table into the server. Record the tool, client/build, selected reader, time, and
output beside conclusions, following ADR 0013.

### Inspection workflow

1. Select the actual client and declared environment; keep that identity visible while reading.
2. Open a supported data group and request a refresh. Show when the result was read and any
   reader errors; an old result must not masquerade as current live state.
3. Inspect the returned records and fields. Display named units and identifiers where verified;
   label unknown meanings rather than inventing names from their values.
4. Save a labeled snapshot, independently or attached to the active test run.
5. Perform the gameplay action, read again, and retain the second snapshot with the observation.

Manual refresh and recorded snapshots establish the initial workflow. Periodic sampling is
available where the tool already supports it or is added as assigned work, with its interval
shown explicitly. It does not mean polling every exposed reader on every UI frame.

Reading state does not require enabling the packet sniffer. tools/context-dump.py explicitly
uses the rig's read-only connection with game_thread=False; retain that capability instead of
requiring the hook-writing connection for every inspection. Capability-specific prerequisites
still apply, and client read results are not necessarily atomic snapshots of all contexts.

Client inspection and ASURA configuration are separate operations. A measured spawn, portal,
or agent property can provide evidence for a later content edit, but reading it must not
automatically change database content or running-world behavior.

The owner requested data reading, not arbitrary client writes or a general memory editor.
Expose the supported read workflows; additional client-control features require an instruction.

## Tests: the operator's testing interface

Distinguish three things in the results:

- Tool execution: started, stopped, failed, interrupted, or completed normally.
- Automated checks: which command/check actually passed, failed, or was skipped.
- Gameplay observation: passed, failed, inconclusive, not performed, with the operator's notes.

A sniffer exit code of zero is not a passed movement test. A passing Rust test is not a
client-confirmed feature. A process failure is not proof of a gameplay regression.

Existing live-session presets provide instructions to reuse. The first tests should use real
existing procedures, such as travel and the recorded movement reproduction, rather than a new
invented full-game test catalogue. Version or snapshot instructions in each run so a later
edit to a test description does not change what a historical run says was performed.

User actions in the client remain manual unless the owner explicitly asks for automation.
The panel guides, records checkpoints, and launches the selected measurements. It does not
press game buttons or issue gameplay commands merely because a test is underway.

Targeted automated checks can be exposed as explicit jobs using the current testing policy.
Do not run the whole battery automatically on panel start, after every content save, or before
every capture. Preserve the reason required for a whole-suite/milestone run.

## Server operations and configuration

Reuse asura-supervisor for process ownership, readiness, retries, stop/restart, and registry
updates. The panel must not introduce a second watcher that races the supervisor.
Show stale/unknown status honestly rather than displaying an old ready value as current.

Initial operations are start, status, stop, selected-service restart, and viewing its logs.
Build/schema/seed/rolling-deploy jobs already exist as commands, but whether to expose all of
them in the first panel is an owner scope decision. The supervisor checks PostgreSQL; starting
the database remains outside its current responsibilities.

Service configuration and world content are different views. A saved configuration is not
necessarily applied to a running process. Display pending restart/application status instead
of claiming a live change the runtime does not implement.

Content editing continues to use the initial map 148/146 flow in docs/plans.md.
The selected content model is one self-contained profile, without inheritance or mod stacking
(docs/plans.md). Identify the profile being edited and the profile currently
loaded by the server; a content save does not switch the running profile. Each profile has separate
account settings, characters, levels, inventory, and progress. Profile packaging and the physical
storage layout remain unresolved. The future launcher supplies startup selection.
Share validation with the world/database operations. Do not add a generic SQL console, user
administration, a visual map canvas, or simulator controls for nonexistent features just because
the product is now called a control panel.

## Execution components and boundaries

| Component | Role | Existing boundary to preserve |
|---|---|---|
| Panel interface | The user's workflows, status, steps, results, forms | Final desktop/browser delivery is not selected |
| Operations/content adapter | Invoke supervisor operations and the shared content operations | No duplicate service supervisor or independent SQL implementation |
| Windows Py4GW research runner | Manage packet sessions and supported in-game data reads; return structured results and artifacts | Core panel research capability, optional for server operation; research remains in tools and no server crate imports or requires Py4GW Stealth |
| Run history and artifact index | Link commands, captures, checks, observations, and files | Recorded evidence is preserved; public archive uses its existing index |

The research runner executes on the machine containing the game client, which is Windows.
It is not an auth/world runtime component. Server-only installations remain independent of
the external client toolchain. The owner explicitly requested user-initiated capture launching;
it must remain independent of normal server startup and CI.

The exact UI/backend packaging and communication are not selected. Choosing a remote server
host later does not move the client sniffer to that host: it still runs beside the client.
Do not add remote execution, endpoints, or a required new service before deployment scope is
settled. The interface itself need not inherit the research runner's elevated privileges.

## Current tools: reuse and gaps

These are source-inspection findings, not tests of the future panel. Recheck before coding.

| Source | Existing capability or gap |
|---|---|
| crates/asura-supervisor/src/control.rs:70 | Status, stop, restart, and rolling-deploy control already exist on loopback |
| crates/asura-supervisor/src/main.rs | CLI start/build/schema/seed commands already exist |
| tools/sniff-capture.py:673 | Direction, duration, headers, output, PID, polling, raw mode, and external root are parameters |
| tools/sniff-capture.py:558 | The capture handles KeyboardInterrupt and has cleanup code; the runner must preserve that path |
| tools/context-dump.py:483 | External root, PID, label, selected contexts, and output are parameters |
| tools/context-dump.py:24 and :74 | Existing read-only data collection and named context readers provide the starting point for live data inspection |
| tools/live-session.ps1:37 | Guided presets, capture plus periodic dumps, and a manifest provide an existing workflow |
| tools/live-session.ps1:105 | A reused output path deletes the prior capture; not suitable for panel session history unchanged |
| tools/live-session.ps1:124 and :140 | It selects the first client and does not pass an explicit PID to the sniffer |
| tools/live-session.ps1:168 | It force-stops the sniffer; not a verified clean shutdown contract for the panel |
| tools/capture-read.py | Existing struct/wire identification and header-offset handling |
| recordings/README.md | Public archive naming, immutable evidence, sanitation, and index format |
| docs/adr/0013-py4gw-stealth-is-our-link-to-the-client.md | External research boundaries, operating requirements, and hook ownership |
| tools/test-touched.ps1 and tools/test-battery.ps1 | Existing targeted-check and whole-battery entry points |

Do not simply put a button around live-session.ps1 and call the session manager done. Keep its
useful workflow, but implement the selected PID, output ownership, stop, and status contracts
before relying on it. Any script fixes are separate scoped implementation work, not authorized
as part of writing this design.

## Proposed implementation increments

1. Agree on local/remote deployment, UI delivery, initial capture concurrency, and the exact
   first end-to-end acceptance flow. Do not choose a framework before these answers.
2. Deliver the panel shell with real supervisor status/control/logs. Show research setup
   availability without requiring the external rig to open the panel.
3. Deliver the Py4GW research workflow: one explicit-client capture session with distinct output,
   live status, clean stop, preserved evidence, persisted history, and correct failure reporting;
   plus the first selected live-data reader views with refresh and labeled snapshots. Data
   reading must work independently of packet capture.
4. Deliver one guided manual test linking its capture, checkpoints, notes, and operator result.
5. Add the map/default-spawn/portal-arrival editor once its content loading/save path is ready.
   Do not claim content management is complete when only process buttons work.
6. Extend to additional evidence channels, paired runs, and later simulator content as those
   capabilities are selected and implemented. Comparisons must respect capture layers.

This is the proposed panel work order. It does not reorder the simulator's combat/AI/quest
dependencies or make every workspace an initial release requirement.

## Questions before implementation

- Is the first setup entirely on the owner's Windows machine, or must it manage a remote
  server while captures run on the Windows client machine from the first release?
- Is the requested interface a desktop application or a browser application?
- Does the first release need one active client capture at a time or several simultaneously?
- Which live-data groups/measurements are the first acceptance case, alongside server start/stop
  and packet-session management? The capability itself is required; its initial field coverage
  is not yet selected.
- Must the map editor be delivered in that same first release, or in the next content increment?
- Before accepting session lifecycle behavior, what should closing the panel do to an active
  capture or long-running job? It must not silently force-kill an attached sniffer.

These are implementation gates, not reasons to repeat settled decisions about Lua, Rust, or
the packet-stream objective. The owner selects the unresolved product choices.

## Selectable self-contained game profiles

> Moved here on 2026-11-06; it was the file `plans.md`.

Owner's design decision, 2026-11-02. No loader, launcher, database change, or profile data is
implemented by this document.

## Intention

ASURA is an offline server whose first content target faithfully reproduces the base game.
The owner also wants other setups: earlier game-era content/rules, extended campaigns, and
custom stories using the same world and server engine.

A future launcher selects what to load and starts the server. The current work must support
that selection without requiring the launcher to exist first. The startup option and on-disk
packaging format are not chosen yet; do not invent a launcher UI as part of loader support.

## Chosen model: one self-contained profile

The owner selected all-or-nothing content loading. Load one selected profile for a server
launch. Do not merge its content with a base profile or search other profiles for missing data.

The owner's example is explicit: if a profile contains three quests, it has three quests.
It does not acquire the rest of the base game's quests automatically.

The intended authoring workflow is to copy the base profile, modify that copy, and share the
resulting profile. Adding three quests to that copy distributes the copied base content plus
the additions. The copy does not retain an inheritance relationship to its original.

| Profile example | Result |
|---|---|
| Base profile | Its installed base-game content |
| A profile containing three quests | Those three quests, with no base-quest fallback |
| A copy of the base profile with three quests added | The copied base quests and the three additions |
| A copy of the base profile with quests removed | The remaining quests; removed quests are not restored from elsewhere |

Self-contained means no implicit content inheritance. It does not mean every profile must
reproduce the entire base game or duplicate the Rust executable, pathing assets, or other
shared engine resources. Required content references must still resolve within that profile
or to an explicitly supported engine resource.

## Boundaries for implementation

- Keep the engine separate from the selected game content. Rust runs the mechanics; the
  profile supplies the supported definitions, relevant settings, and Lua behavior.
- Keep the faithful base profile identifiable. Editing a custom copy does not edit the base.
- Load only the selected profile's definitions and scripts. Do not scan all installed profiles
  for extra quests, scripts, or replacements.
- Keep installed database content consistent with the selected profile. Selecting another
  profile must not leave previous-profile quests or NPCs visible by accident.
- The exact database separation mechanism is unresolved. Separate databases, schemas, and
  profile-keyed rows are not interchangeable implementation assumptions; decide before writing
  the affected storage/loading code.
- A missing required reference is a validation error. Missing optional content stays absent.
  Do not fill gaps with base content, guessed records, or successful no-op handlers.
- Selecting a profile happens for startup. Live profile switching and mixed profiles inside
  one running server are not part of the chosen simple loading model.
- Keep runtime progress and transient state distinct from distributable content. The owner
  selected separate account settings, characters, levels, and progress for each profile;
  do not share them across profiles.
- New mechanics still need engine support. Selecting profiles does not authorize a Rust
  plugin ABI, arbitrary native modules, or a new client protocol/version.

This model does not need overlays, patch stacking, profile dependency graphs, load-order conflict
resolution, or merge rules. Do not add them to support the author's three-quest example.

## Profile-owned HeroAI and custom skills

The selected profile supplies both skill behavior and HeroAI behavior. Authors can modify
skill knowledge, usage conditions, targeting, priorities, combinations, coordination, and
build-specific tactics in Lua. A custom skill pack must be able to change the corresponding
HeroAI rules without editing or rebuilding Rust if the engine already supports its mechanics.

This follows the existing self-contained model: copy the base profile to reuse its skill and
HeroAI scripts, then edit the copy. Loading a custom profile does not silently fall back to
base HeroAI scripts or a hardcoded Rust skill registry. Required behavior references must
resolve within the selected profile or use explicitly supported generic engine operations.
No new pack overlay, script inheritance, or independent package loader is selected here.

## Chosen model: separate experiences and saved state

The owner clarified: each mod has its own account settings and is a separate experience, with
separate characters, levels, and progress. Profile selection therefore selects both the game
content and that profile's account/player state.

Keep account settings, character records, levels/experience, inventory/equipment, quest history,
unlocks, and other durable gameplay progress scoped to the selected profile. Runtime sessions
and instance assignments must resolve against that same profile. Selecting another profile
must not reuse the previous profile's character or its state.

Switching back to an existing profile resumes its own persisted experience under that game's
normal login rules. It does not reset that profile merely because a different one was loaded
in between. Do not add cross-profile character transfer or automatic save conversion.

This decides logical isolation, not the physical database layout. Whether to use separate
databases, schemas, or profile-scoped records still needs an explicit implementation design.
Sharing the server executable or database service does not authorize sharing player state.

## Control panel and future launcher

The loader needs an explicit selected-profile input that the future launcher can supply.
The flag/path name and profile metadata format remain to be designed.

The control panel's content editor must identify which profile it edits. If it is showing a
running server, distinguish the profile being edited from the one currently loaded. Saving
content does not claim that a live server switched profiles or reloaded content.

Copying/exporting a base profile must mean copying distributable content and its scripts, not
cloning account credentials, active sessions, leases, or a player's private progress.
The export/copy mechanism and format are unresolved. JSON was previously recommended for
content exchange, but the owner has not selected it; Lua is chosen for behavior, not implicitly
for all profile configuration.

## Decisions still needed

1. What is the profile's distributable representation and how does the database install/read it?
2. Which storage layout implements the chosen separation of content and account/player state?
3. How is a profile identified and checked against the engine features/version it requires?

Resolve these before dependent implementation. Do not reopen the selected no-inheritance model
or build the future launcher to answer a loader/storage question.

## Future acceptance cases

- Starting the base profile loads its own supported content.
- Starting a valid three-quest profile exposes only those three quests.
- Starting a copied-and-extended base profile exposes the copied content and additions.
- A removed optional content record does not reappear through fallback.
- Switching the startup selection does not mix content from the previous profile.
- Account settings and characters changed in profile A do not change those in profile B.
- A character's level, inventory, quest progress, and unlocks remain local to its profile.
- Returning to profile A restores its own persisted state rather than profile B's state or a reset.
- A copied profile can add a custom skill using existing mechanics and teach HeroAI to use it
  through Lua changes alone, without modifying/rebuilding Rust or changing the original profile.
- Required HeroAI behavior is resolved from the selected profile, without hidden base fallback.
- Invalid required references are reported without silently completing missing content.
- A profile copy leaves the original and private operational/player data untouched.

These are acceptance requirements for later implementation, not test results.

## Skill mechanics and scripted behavior

> Moved here on 2026-11-06; it was the file `plans.md`.

Research and owner clarification recorded 2026-11-02. This is documentation for later work;
no combat runtime, scripting API, or skill implementation is introduced by this review.

## Owner's distinction: scripts declare behavior; the server executes mechanics

This is a simulation-wide distinction: NPC decisions, movement behavior, combat, skills, quests,
spawns, encounters, loot, dialogue, interactions, and map behavior belong in external scripts.
Mechanics are the supported server capabilities that execute that behavior. Scripts define
rules, conditions, calculations, sequences, and reactions; Rust executes the requested operations
against authoritative state. Skills below are examples of this general boundary, not its scope.
The full responsibility table is in docs/plans.md.

The owner's example is flight: if the server offers no flight mechanic, scripts cannot fly.
A declaration cannot supply a missing server capability. This distinction also applies to
skills: declaring a projectile, damage, or an enchantment requires those server mechanics.

| External skill behavior declares | Server mechanic executes |
|---|---|
| Which targets qualify and what conditions change the result | Target queries and validation against authoritative state |
| Resource requirements, activation, recharge, and duration values | Resource accounting, cast scheduling, interruption, and timers |
| Launch a projectile; on impact, damage the target | Projectile movement, hit resolution, damage calculation, and health changes |
| Apply regeneration while an enchantment lasts | Effect ownership, regeneration accounting, expiration, and removal |
| Limit eligible incoming damage while an effect lasts | Supported damage adjustment operations at the appropriate resolution stage |
| React to an attack by damaging the attacker | Event delivery and execution of the requested damage operation |
| Which skill an NPC chooses and who it targets | Execution of the chosen action through the same world operations |

These are skill-specific examples of the general boundary, not selected API names or a file
format. Rust behavior-tree execution remains a server mechanic; game-specific decisions belong
in external behavior scripts. How scripts connect to that runtime, and the exact events and
operations exposed to Lua, remain open. Scripts run through the server; they do not independently change client state or send
invented packets. See [30-world-content-and-scripting.md](30-world-content-and-scripting.md).

## Rurik source review

Checkout reviewed: C:/Users/Apo/Documents/GitHub/Rurik at commit
0710df3b5cb068362c61120820753e59a36dd149.
Paths and line numbers below refer to that revision, not ASURA. Rurik's source is the only
witness for these implementation findings. No Rurik server, tests, or client session was run
for this review; its cited recordings were not independently replayed into ASURA evidence.
Rurik remains a research source, never a runtime dependency (ADR 0011).

| Capability | Implementation found | Source at reviewed revision |
|---|---|---|
| Player requests | Handles ordinary skills and attack skills with activation gates | toolkit/authsrv/authsrv.py:27210 |
| Cast execution | Deferred resource payment and animation at actual queued-cast start | toolkit/authsrv/authsrv.py:27939 |
| Cast timing | Pending records advanced through completion, aftercast, and recharge | toolkit/authsrv/authsrv.py:28046 |
| Skill data | Timing comes from content; magnitude meaning is read separately from its value | toolkit/authsrv/authsrv.py:24725, 5447; toolkit/authsrv/skillread.py |
| Lasting effects | An effect episode opens and closes on its bearer | toolkit/authsrv/effects.py:1 |
| Projectiles | Player spell launch and later impact have separate handling | toolkit/authsrv/authsrv.py:17020 |
| Reactions | Active hexes marked as attack triggers damage the attacker | toolkit/authsrv/authsrv.py:29252 |
| NPC execution | Separate hostile attacks, ally casts, and ally attacks | toolkit/authsrv/authsrv.py:32526, 33301, 37368 |

Important limits and divergences:

- The ordinary recharging cast sequence is E4, E5, E3, E6, but it is not universal.
  completion_sends_e5 omits E5 for zero recharge under the current setting, and cast_tick
  documents that zero-recharge cycles close at E3 without E6
  (toolkit/authsrv/authsrv.py:28034-28062).
- begin_cast explicitly states that its queue stacks requests where retail replaces the queue.
  Its extra begin-time resource check and remaining busy window are documented consequences,
  not a retail queue specification (toolkit/authsrv/authsrv.py:27956-27966).
- An unmodeled damage skill returns None, not zero; missing timing content instead falls back
  to zero with a log message. These are different reference behaviors, not ASURA defaults
  (toolkit/authsrv/authsrv.py:5447, 24725).
- The old land_skill docstring describes only the player as target, but later implementation
  notes explicitly record target parameterization for party casts. Read the current call path
  rather than treating that older description as the full capability
  (toolkit/authsrv/authsrv.py:40451, 33314-33320).

## NPC behavior found in Rurik

| Decision | Implementation found | Source |
|---|---|---|
| Choose a skill | Round robin over ready slots, skipping disabled skills | toolkit/authsrv/authsrv.py:40388 |
| Choose a heal target | Lowest health fraction below HERO_HEAL_AT, set to 0.9 | toolkit/authsrv/authsrv.py:33172, 15031 |
| Choose an eligible target | Enemy, ally, self, and other-ally paths; offensive range checks | toolkit/authsrv/authsrv.py:33400 |
| Hold an unnecessary effect | Checks whether the primary wearer already carries the effect | toolkit/authsrv/authsrv.py:32429, 33449 |
| Remove an affliction | Removal targeting checks for the applicable affliction | toolkit/authsrv/authsrv.py:33426 |
| Pursue or return | NPC following and return-to-spawn handling | toolkit/authsrv/authsrv.py:37458, 38891 |
| Fight from casting range | Hostile caster classification and range handling | toolkit/authsrv/authsrv.py:33832 |
| Escape damaging areas | Scatter state and movement handling | toolkit/authsrv/authsrv.py:39414 |

pick_skill explicitly labels its round robin a testing function, not measured ArenaNet AI
(toolkit/authsrv/authsrv.py:40388). Healing target selection also declares itself authored
policy (33175). These policies must not become claims of live-game parity through a port.
The primary-wearer effect check also documents incomplete coverage for area effects (32444).
The monster AI research is studies/monsterai/FINDINGS.md; it distinguishes observable outcomes
from the unavailable original server decision function. That study is research, not proof of
any unmeasured selection, aggro, or leash rule.

## Skills that illustrate the distinction

The following wiki values were researched for explanation at attribute rank 12, without other
modifiers. Wiki descriptions are not ASURA captures and do not establish packet layouts,
processing order, rounding, or timing details. Rurik implementation evidence is listed separately.

| Skill | Published behavior at rank 12 | Script declaration and required mechanics |
|---|---|---|
| Flare | 5 energy, 1 s activation, no recharge; 56 base fire damage on projectile hit; Overcast adds adjacent foes | Declare projectile and conditional impact behavior; server executes flight, hit resolution, targeting, and damage |
| Healing Breeze | 10 energy, 1 s activation, 5 s recharge; +8 regeneration for 15 s | Declare a timed enchantment contributing regeneration; server maintains and removes its contribution |
| Protective Spirit | 10 energy, 0.25 s activation, 5 s recharge; 19 s protection limiting eligible damage to 10% maximum health | Declare the protection behavior; server must expose the necessary damage-resolution operation |
| Empathy, PvE | 10 energy, 2 s activation, 10 s recharge; 13 s hex, 46 reactive damage, attacks deal 12 less damage | Declare the attack reaction and outgoing damage adjustment; server supplies events and damage operations |

Wiki sources consulted in the discussion:

- https://wiki.guildwars.com/wiki/Flare
- https://wiki.guildwars.com/wiki/Healing_Breeze
- https://wiki.guildwars.com/wiki/Protective_Spirit
- https://wiki.guildwars.com/wiki/Empathy
- https://wiki.guildwars.com/wiki/Health_regeneration
- https://wiki.guildwars.com/wiki/Aftercast_delay
- https://wiki.guildwars.com/wiki/Event_stack

The research identified why shared mechanics need distinct operations: regeneration is not
healing, degeneration is not damage, and Protective Spirit does not reduce life stealing or
health loss. Empathy reacts even to missed or blocked attacks; successful-hit events alone do
not describe it. Interactions and processing order still need live verification.

Rurik examples to revisit:

- Flare's content identifies fire damage (content/world.toml:144). Its projectile speed is
  reconstructed from Fireball sharing the projectile, not a recorded live Flare cast (2925).
- Healing Breeze's content identifies regeneration and cites a live recording combining its
  contribution with degeneration (content/world.toml:3335). This is Rurik's evidence claim.
- Empathy-style attack reactions use each active hex's stored caster and attribute rank to
  request armor-ignoring damage (toolkit/authsrv/authsrv.py:29252). This does not establish
  complete reproduction of every part of Empathy, including its damage reduction.
- Protective Spirit was a conceptual example in this discussion; this review did not establish
  a complete Rurik implementation of it.

## Deferred work

Implementation is deferred at the owner's request. No skill set, encounter, API, script schema,
event ordering, or missing mechanic is selected for implementation here. Before a later port,
separate the reference's measured execution from its authored policies and divergences, define
the script/server contract, and obtain ASURA's own evidence through the repository source order.
