# World content, scripting and profiles

The owner selected the following direction in ASURA on 2026-11-01 and 2026-11-02. This is a design
for future work, not a description of implemented Kryta features. Recovered source: ASURA
`docs/guidance/scripting-and-behavior.md`, `docs/guidance/game-packs.md`, and
`docs/reference/decisions/0014-behavior-tree-execution.md`, under `docs/evidence/asura-history/`.
Kryta's structure is governed by `AGENTS.md` and `docs/port/01-structure.md`; client-visible
behaviour is governed by ASURA and the rules and evidence in `docs/game-rules.md`.

## Rust mechanics and Lua behaviour

The owner chose Lua for external game behaviour and Rust for the server's mechanics. This distinction
covers the whole simulation. The script describes conditions, decisions, sequences, reactions and
what action to request. Rust owns authoritative state and carries out supported operations. A new
behaviour using existing mechanics should be authorable without rebuilding the server; a new mechanic
requires engine support. Script APIs, event names, packaging, reload semantics and the Lua integration
remain to be designed.

| Subject | Behaviour authored in content | Mechanics supplied by the server |
|---|---|---|
| NPCs and AI | Priorities, targets, tactics and reactions | World queries, behaviour execution and actor actions |
| Movement | Patrol, chase, follow and retreat decisions | Pathfinding, collision, movement and position updates |
| Combat and skills | Use conditions, targets, timings, effects and combinations | Resource accounting, casting, projectile, effect, damage and death operations |
| Quests and missions | Prerequisites, objectives, branches, progression and rewards | State, event delivery and supported actions |
| Spawns and encounters | Population, conditions, waves and progression | Creation, placement, lifecycle and state |
| Dialogue and interactions | Choices, conditions and consequences | Interaction delivery, client messages and state changes |
| Maps and instances | Entry reactions, encounters and scripted transitions | Map loading, instance lifecycle, placement, travel and geometry |

These examples describe intended ownership, not current APIs. Measured client sequences and the
rules ledger remain the basis for reproducing base-game behaviour. Content can only request actions
that the engine supports. The proposal to use event-driven handlers is recorded as a proposal; event
ordering, scheduling, retries and failure semantics are open.

## HeroAI and behaviour trees

The owner chose BehaviorTree.CPP as the semantic reference for the Rust execution engine and Py4GW
Reforged HeroAI as a source for game decisions. The selected profile supplies skill behaviour and
HeroAI's rules for using skills in Lua. Rust owns node execution, per-agent running state, ticking,
halt/reset and access to authoritative queries and actions. How Lua declarations and game-specific
leaf decisions connect to the runtime, including callback timing, is unresolved. The earlier wording
that Lua does not execute per-tick logic must be reconciled with the instruction that game-specific
leaf decisions belong in Lua before implementing that boundary.

A future acceptance case copies a base profile, adds a custom skill using existing mechanics, and
teaches HeroAI to use it through that profile's Lua content without changing or rebuilding Rust.
The original profile remains unchanged. The skill's behaviour and AI usage are separate content.

The source comparison in `docs/evidence/asura-history/docs/reference/decisions/0014-behavior-tree-execution.md`
records material differences between Py4GW nodes and BehaviorTree.CPP. A migration must compare
running, interruption, reset and completion semantics for each used branch. Its closest mappings
include sequence to Sequence and selector to Fallback; reactive variants can change reevaluation and
cancellation. Parallel completion, delayed action completion, repeat, running conditions and halting
also need explicit examination. A matching node name alone does not establish parity.

`docs/evidence/asura-history/docs/research/heroai-server-port.md` records the source-only analysis at
Py4GW Reforged commit `a5fd2e54d504605200a752b1bc991d618ed33c41`, including combat selection,
target scans, skill preparation, observations and interruption. The owner's sizing example was 100
instances with 200 AI-controlled agents each. No benchmark in that review establishes capacity.

## Profiles and isolated experiences

One selected, self-contained profile supplies the game content for a server launch. It does not
inherit missing content from another profile. A profile containing three quests has three quests;
a copy of the base profile with three added has the copied quests plus those additions. The copy is
independent. Engine resources may be shared where explicitly supported.

Each profile is a separate experience with its own account settings, characters, levels, inventory,
quest history, unlocks and other progress. Returning to a profile resumes its own saved experience.
The selected profile owns its Lua behaviour and HeroAI skill decisions. Copying distributable content
must not copy credentials, active sessions or private player progress.

The owner selected logical isolation; database versus schema versus profile-scoped rows remains open.
Profile identification, installation and distribution, engine compatibility, selection input and
launcher integration also remain to be designed. The presence of JSON configuration or database rows
in Kryta does not establish a profile loader or isolated saved state. A future launcher can supply the
selection, but the support need not depend on building that launcher first.

## Content authoring

The recorded design distinguishes agent definitions, placement and facing, groups, movement
assignments, spawn conditions, lifetime and quest relationships. A quest may involve collecting,
fighting, reaching a location, escorting, following and prerequisite progress. The scripting
boundary supports combining available operations into such sequences. Which ordinary objectives are
data, Lua or both remains open. Phasing and layering need explicit population and state ownership;
they are not supplied automatically by a scripting language.

The older ASURA plans describe a map/spawn/portal manager and later content editors. Kryta's existing
administration tool is described in `docs/port/05-tooling.md`; its pages do not establish a complete
profile or quest editor. Keep operator settings, distributable content and player state distinct when
those interfaces are designed.

## Remaining design questions

- Which game decisions, including existing placement rules, are profile-authored and which remain in
  Rust under the current ASURA parity and AzerothCore structure requirements?
- How are scripts registered, scheduled, interrupted and bounded, and what happens to state when a
  script fails or content changes?
- What events and world operations can scripts use, and what ordering and transactional guarantees
  do they have?
- How do Lua leaf decisions interact with Rust-owned behaviour-tree ticking and per-agent state?
- How do profile selection, storage isolation, distribution and compatibility work in Kryta?
- How do quest state, party state, phasing, instance state and spawn selection interact? See the
  unresolved world-model questions in `docs/game-rules.md`.

## Status and evidence

`game/src/Scripting/` owns the script manager. Three world lifecycle hooks are wired, and their
current limits and proof are in `docs/port/10-separation-guide.md`. Lua content integration and a
module crate remain future work. No conclusion about client acceptance follows from this design.
`docs/port/06-world-plan.md` maps world subjects to their nouns.
