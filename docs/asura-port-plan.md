# Porting ASURA into Kryta: implementation plan

2026-11-22

## Purpose and ground rules

Kryta already carries ASURA's login, maps, travel, pathfinding, items, chat, guilds and experience; the large subjects still to bring across are NPCs, quests, skills and combat, vendors, cartography, titles and the rest of party. This plan says how each one is ported, in what order, and what proves it done. It gives movement, NPCs and quests their own sections because those are where we already have findings.

Read for this plan: Kryta `AGENTS.md`, `docs/port/01, 06, 08, 09, 11, 15, 17, 18`, `docs/game-rules.md`, `pseudo_lua/`, `worldserver/worldserver.json`; ASURA `MILESTONES.md`, `docs/HANDOFF.md`, `docs/49, 53, 55, 58, 60`, `config/Server/movement.json`, and the `crates/asura-world/src` file list.

**The rules every round follows** (from Kryta `AGENTS.md`, not new):

1. **Rule 0.** ASURA is the authority for every message, field, order and handler step. Each port cites the ASURA `file:line` above the code. Where ASURA has nothing, we stop and ask; we do not write our own version.
2. **Rule 0a.** The owner picks the subject. Within it the largest file goes first. Read the file before estimating it. A round may end mid-file; it may not switch subjects to end green.
3. **Every round opens with one line:** the ASURA file, its line count, and the lines carried.
4. **The audit is the ledger.** `docs/port/09-parity-audit.md` is updated in the same commit, and anything partial is marked *ported thin*.
5. **Structure stays AzerothCore's.** ASURA decides behaviour; the reference decides where the code lives (`game/<Noun>/`). Infrastructure and game mechanics never share a commit.
6. **Content is JSON under `config/`.** ASURA migrations that *insert rows* become pack files; `data/sql` stays schema only (rule 7).
7. **The client is the judge.** A subject is done when a real client does it, not when the tests pass.

**Goal and reference (owner, 2026-11-22).** `pseudo_lua/` is the goal *as a concept*: the NPCs, placements, dialogs, behaviours and quests Kryta is after, and the idea of separating a model, a record and a placed reference. Its actual files, values, layout and Lua are open to being changed or rewritten completely as the port needs; they are never copied as behaviour. The reference for *how* to get there is ASURA, but only its content that is backed by evidence (a capture or a client run) and by tests. ASURA values that are authored, chosen, unmeasured or shortcuts are not carried; where the evidence is missing, the step waits for a capture or the owner.

**Which ASURA: always the local repo (owner, 2026-11-22).** Every port reads `D:\ASURA-main` as it stands on disk, not a pinned commit. Kryta's `AGENTS.md` still names `C:\Users\Apo\Documents\GitHub\ASURA` at `ca9f532`, so that line should be updated to say the same. Earlier ports came from `ca9f532` and `071735e`; the local repo is newer (it has `npc/`, `quest.rs`, `skill/`, `vendor/`, `approach.rs`, `compass.rs`), which is why movement needs a re-diff.

## Where Kryta stands against ASURA

Kryta has ported the whole spine (login to walking through a door) and three later subjects; everything that puts life in a map is still to come. Status is from `docs/port/09-parity-audit.md` and `18-kryta-code-feature-inventory.md`; ASURA's own status is its `MILESTONES.md` mark (`[x]` client, `[o]` test, `[~]` built, untested).

| Subject | ASURA source | ASURA status | Kryta today | Next step |
| --- | --- | --- | --- | --- |
| Login, EULA, character list, front door | `asura-auth` | `[x]` | Ported, end-to-end tested | none |
| Character creation | `creation.rs`, `default_equipment.rs` | `[o]` | Ported; gaps listed in `08-rules-coverage.md` | close gaps per ASURA |
| Maps, instance table, districts, spawns | `catalogue.rs`, `instance_table.rs`, `map_spawns.rs` | `[x]` | Ported (265 maps, 145 hosted) | none |
| Travel, `/tp`, doorways | `travel.rs`, `portals.rs` | `[x]` / `[~]` | Ported; seat reservation left out on purpose | client-confirm doorways |
| Pathfinding and graphs | `pathfind.rs`, `map_data.rs`, `graph_dump.rs` | `[x]` | Ported whole and wired | none |
| Movement handlers, directional walk | `agent.rs`, `pathfind/directional.rs` | `[x]` / provisional | Ported from `ca9f532` | re-diff against current ASURA (Movement) |
| Items: move, equip, unequip, destroy, drop, pickup, `/item` | `item.rs` | `[x]` | Every client item operation runs through `InvModel` with the captured frames, a drop is refused outside an explorable (E51), and `/item` gives a live definition, 2026-11-22 (audit 7h); `/item clear` remains | the load block's diff (`item.rs:22-236`) |
| Item catalogue and modifiers | `item_catalogue.rs`, `item_modifier.rs`, `inventory_model.rs` | `[x]` | Ported whole, 2026-11-22 (audit 7h); the stored `definition_id` column makes the lookup whole, and the four row-writing handlers run on `InvModel` | the load block's diff (`item.rs:22-236`) |
| Chat and commands | `chat.rs` | `[x]` | Ported; `/xp <total>` thin | finish thin rows |
| Experience and levels | `experience.rs` | `[~]` | Ported (thin backfill) | none |
| Guilds | `guild.rs` | `[~]` | Ported | none |
| Party | `party.rs` (2,222 lines) | `[~]` | Ported thin: lines 1207-1826 left | arrival, resign, hold |
| Cartography | `cartography.rs` (1,968 lines) | `[~]` | Announcement only; surveyed | port |
| Titles | `titles.rs` | `[x]` / `[~]` | Not started | on hold: apoguita is working on titles |
| Account settings blob | `connection.rs` | `[x]` | Not checked | audit |
| **NPCs** | `npc/` (9 files, ~205 KB) | `[~]` | N1-N3 ported and N4 ported thin (audit 7g): ASURA's 11 maps of NPCs load; the two authored patrols walk; the Script leaf and its Lua source wait | client proof, then the Lua source and the fight |
| **NPC dialogs** | `dialog.rs` | `[~]` | Not started | see NPCs |
| Vendors, collectors, storage | `vendor/` | `[~]` | Not started (`game/Trader/` stub) | after NPCs |
| Interact / walk to target | `approach.rs` | built | Not started | with NPCs |
| Skills and combat | `skill/` (~220 KB), `npc/fight.rs` | `[~]` | Not started | after NPCs |
| **Quests** | `quest.rs` (70 KB) | `[~]` | Not started in Rust; Lua prototype | see Quests |
| Compass drawing, callouts | `compass.rs` | built | Not started | with party |
| Registry, leases, reaper, containers, CI | `asura-supervisor`, `docker/` | `[o]` | Absent by design | owner decision |

## How one port round runs

Every subject below goes through the same nine steps. This is `06-world-plan.md`'s delivery model plus Rule 0a, written as a checklist.

1. **Open the line.** Write the opening line: ASURA file, revision, line count, lines to carry this round.
2. **Survey before sizing.** Read the file function by function. Write the survey into `09-parity-audit.md` as a section (the way 7b, 7c and 7d already do) before any code.
3. **Place it in Kryta's tree.** Map each ASURA file to its noun (`game/Agents/`, `game/Behaviour/`, `game/Quests/`, ...) using `06-world-plan.md`'s noun table. A new directory needs the owner's word.
4. **Split data from code.** ASURA JSON under `config/World/Vanilla/` comes across as pack files; ASURA migrations become schema-only files under `data/sql/` (any `INSERT` becomes pack content). Each new file is classified in `04-configuration.md`, or the test fails.
5. **Port the wire first.** Builders and readers go into `game/Server/Packets/<Family>Packets.rs` with ASURA's byte-for-byte tests.
6. **Port the state owner and handlers.** Same names, same order, same quirks, each with its `ASURA/...:line` comment. Dispatch arms are added as ASURA's `dispatch.rs` adds them.
7. **Port ASURA's tests with the code.** Its unit tests come across unchanged in meaning; Kryta's `worldserver/tests/end_to_end.rs` gets one case per subject.
8. **Update the ledgers in the same commit.** `09-parity-audit.md` verdict, `01-structure.md` status, and `08-rules-coverage.md` if a ledger rule moves.
9. **Client proof.** Run `07-manual-test.md`'s journey plus the subject's own check, with `--capture` on, and compare the capture with ASURA's recording of the same action. Only then is the row marked done.

A round that cannot finish ends mid-file and says so: file, lines carried, lines left.

## Movement

Kryta's pathfinding is done; what is left is making sure the movement *handlers* match ASURA's current code, not the `ca9f532` copy they were ported from but the local repo, and getting owner rulings on three numbers. Kryta's `game/Agents/mod.rs` is about 21 KB; ASURA's `agent.rs` is now about 64 KB, so the handlers need a fresh diff before anything else is built on them.

### What we know

| # | Finding | Source | Kryta now |
| --- | --- | --- | --- |
| 1 | Walking is answered from the dumped graph (`ASURAPG1`), not from `Gw.dat`. 136 of 138 hosted maps walk from their own spawn. | audit 7b, section 9 | Ported and wired; `worldserver` reads only the cache |
| 2 | Maps 119 and 642 fail because their `spawns.json` states no plane (row says 0, point is on plane 21/22). ASURA fails the same way. | audit section 9 | Pack defect, open |
| 3 | A click with no path sends nothing. ASURA's old position broadcast once disconnected a client and once snapped it into a wall. | ASURA `agent.rs:722-743` | Ported |
| 4 | **Alternating click / keyboard / autorun on the Ascalon bridge.** ASURA sent no reply to 14 manual moves and 4 stops; live replies with destinations and direction updates, and answers a stop with type 9 then the stop point. Fix: manual movement replaces the old path, replies go to player and peers, stop clears the path, next click restores speed. Owner-confirmed 2026-11-01. | ASURA HANDOFF; `recordings/client/feature/movement/alternating-*.jsonl` | **Unverified**: must check Kryta's port includes this fix |
| 5 | Directional (keyboard) walker is provisional. Owner: "better but not perfect", "not of my liking". | ASURA HANDOFF 2026-11-06 | Ported with the caveat kept |
| 6 | Tick cadence about 100 ms (median 81) from the live client. | ASURA `intervals.json` | `SimulationTickMs` 100 |
| 7 | A cancelled walk's client position is believed up to a limit. Reference 100, ASURA doc says 150, ASURA file says 250; live accepted 292.9 and 319.7. | ASURA `movement.json` | `MaxAllowedCorrection` 250 |
| 8 | Doorways test the **raw reported segment**, not simulated ticks; touching the wall counts (endpoints inclusive); a point door uses a box from `portals.json`. | ASURA HANDOFF 2026-11-06 | `cross_portal` ported; client proof pending |
| 9 | Walking into a map unlocks it; the unlock never gates the walk. This caused the `007` on portals until 2026-11-20. | ASURA `player.rs:442-471` | Fixed |
| 10 | `0x01A6` `is_explorable` at 1 made an explorable load take 5,138 ms vs 849 ms at 0. | ASURA HANDOFF 2026-10-31 | `TravelExplorable` default 0 |
| 11 | Interact / attack / follow: server sends `0x002A` (target's position) to the walker, repeats it every 500 ms while the target walks, sends `0x0028` on arrival. A player already there gets no walk. | ASURA `approach.rs` (from `movement-live-20261109-200703.jsonl`) | Not ported |
| 12 | Interact stop range 50 is "chosen, not measured"; live still sent a walk at 62. | ASURA `movement.json` | Not ported |
| 13 | Server sends a destination, not a position; `0x002C` is the teleport. | `game-rules.md` E82, E83 | Matches current handlers |

### Plan

1. **Re-diff `agent.rs`** (current ASURA) against `game/Agents/mod.rs` and `Handlers/MovementHandler.rs`, function by function. Record the result as audit section 7f. Port any missing lines, finding 4 first. Opening line: `agent.rs`, its line count, the lines that differ.
2. **Re-diff `pathfind/directional.rs`.** If ASURA has changed since the port, carry the change. Keep it marked provisional; do not tune it ourselves (Rule 0, finding 5).
3. **Port `approach.rs`** (finding 11) once NPCs exist, since its first user is interacting with an NPC. It needs `0x002A`'s builder and the 500 ms announce.
4. **Fix the two spawn files** (finding 2) by adding the measured plane, as pack data with its source.
5. **Client-prove** the bridge route (finding 4), a doorway crossing at a wall end (finding 8), and a keyboard walk, each with `--capture`, compared against ASURA's recordings.

**Owner rulings needed:** the correction limit (finding 7: 100, 150, 250 or higher), the interact range (finding 12, which Rule 0 cannot accept as "chosen"), and whether the directional walker's current state is acceptable to carry or should wait.

## NPCs

NPCs are the next big subject: Kryta has none in Rust, while ASURA stands 79 bodies across the 11 pre-Searing maps and Kryta's own `pseudo_lua/` prototype already models 491. The plan ports ASURA's framework in six stages, static bodies first, so a client sees NPCs before any Lua or combat exists. Ledger rules R29-R31 and R34 are currently *blocked* in `08-rules-coverage.md` waiting on exactly this.

### What ASURA has (`docs/49-npc-framework.md`)

- **Three records.** A *definition* (type, shared across the pack), a *placement* (one body on one map), and the running *agent* owned by the instance. Scripts and quests name a placement id, never an agent id.
- **Content** in `config/World/Vanilla/npcs/definitions.json`, `npcs/sight.json`, and one `npcs.json` per map folder (placements and routes). Start-up refuses on any bad row, and checks every moving body stands on the path graph.
- **The entry burst**, measured from a live Ascalon City load: `[0x0056 [0x0057]] [0x009A] 0x009B|0x009F 0x00F0 0x0020 [0x0161...] 0x006D`. A definition goes once per connection, just before its first body. `0x0021` removes a body that leaves compass range.
- **Ids.** NPCs get agent ids when the instance is created, before any player; a player's join skips them.
- **The tick.** A body is active only while a viewer is within `compass_range` (10,000). Each tick: advance walks, tick behaviour trees, then send leaves and entries per viewer.
- **Behaviour.** A Rust tree runtime (`Sequence`, `Fallback`, `Wait`, `Stand`, `Patrol`, `Burrow`, `Script`), with tree shapes read from `npcs/behaviors.lua`. Two authored patrols, the Plague Worm burrow cycle.
- **Armed NPCs.** Eight Ascalon bodies carry captured `0x0161` item rows and `0x006D` weapons.
- **Dialogs** (`dialog.rs`, `docs/55`): a talk opens a captured greeting and choices; the Purveyor's seven choices are captured.

### What Kryta has

- `pseudo_lua/`, **the goal**: 84 models, 168 records, 491 placements over 11 maps, 42 NPCs' captured dialog pages, behaviours, and a dependency-free runtime whose smoke test checks Sir Tydus's burst against ASURA's measured shape. Kryta's NPC pack keeps its model / record / reference idea, but the files themselves may be reshaped or rewritten as needed. Each value enters the Rust pack only when it has evidence behind it (its README marks which are captured, which are catalogue, which are assumptions).
- `World.CompassRange` (10,000) is already a setting. The `Behaviour`, `Scripting` and `Combat` directories exist but are empty. Lua is decided (`mlua`, lua54, vendored) but not built.

### Where it goes in Kryta

| ASURA file | Kryta home | Notes |
| --- | --- | --- |
| `npc/burst.rs`, `npc/frames.rs`, the five builders in `asura-protocol` | `game/Server/Packets/AgentPackets.rs` | byte tests against the capture come too |
| `npc/content.rs`, `npc/file.rs`, `npc/mod.rs` | `game/Agents/` (NPC content and loader) | pack files under `config/World/Vanilla/` |
| `npc/table.rs` | `game/Districts/` (one table per district) | Kryta's district is ASURA's instance |
| `npc/behavior.rs` | `game/Behaviour/` | the AzerothCore `AI/` counterpart |
| `npc/lua.rs` | `game/Scripting/` | needs the Lua runtime first |
| `dialog.rs`, `approach.rs` | `game/Handlers/` + `game/Agents/` | interaction |
| `npc/fight.rs` | `game/Combat/` | after skills |
| `vendor/` | `game/Trader/` | after dialogs |

### Stages

1. **N1 Wire.** *(ported 2026-11-22, audit 7g.)* Port the builders (`0x0056`, `0x0057`, `0x009A`, `0x009B`, `0x009F`, `0x00A2`, `0x00F0`, `0x006D`, NPC create) and the burst order, with ASURA's capture-byte tests.
2. **N2 Content.** *(loader and every refusal ported 2026-11-22, audit 7g; the pack files are next.)* Port the loader and every refusal rule, with pack files built on the model / record / reference idea (the `pseudo_lua` files are reworked into whatever shape the loader needs). Start with the evidence-backed bodies (ASURA's captured map 148 load, then the catalogue rows ASURA tested) and grow towards the goal's 491 as captures back each one; ground-check against Kryta's graph cache.
3. **N3 Population.** *(ported thin 2026-11-22, audit 7g: standing bodies, per-copy agent ids, load and sight; client proof pending.)* NPC table per district, seeded before players; ids in file order; the load's agent array in id order; per-viewer sight with compass churn and dormancy. **First client proof:** 23 bodies in Ascalon City, a second client sees the same ones, leaving and returning does not duplicate them.
4. **N4 Behaviour.** *(runtime ported thin 2026-11-22, audit 7g: `Stand`, `Patrol`, `Wait`, `Sequence`, `Fallback`, `Burrow`, the burrow cycle and the body tick, with ASURA's tests and built-in `PackBehaviors`; the two map 148 patrols walk. The Script leaf is the one part left.)* Then the Lua source once `game/Scripting/` has its runtime: it builds the script entries, `worm_lakeside` and `worm_queen` among them, which currently park and stand.
5. **N5 Armed bodies.** `0x0161` items before `0x006D`, item ids allocated per body.
6. **N6 Interaction.** `approach.rs` (walk to the NPC) and `dialog.rs` (greeting and choices). Vendors and combat follow as their own subjects.

### ASURA NPC parts that are not evidence-backed, so not carried as-is

- None of ASURA's NPC acceptance rows has a real-client proof yet. Its byte tests against the capture are the evidence; the rest is proved first by Kryta's client run.
- `h003B` goes out as 0 where the live load varies: needs a capture reading.
- The `walk_speed` pair (`0x0027` + `0x002B`) caused error `007` on client 39167; not carried. NPC speed 120 is an owner choice, not a measurement (cmonmeow uses 220).
- The close guard pair's five-spot random patrol is an observation, not a capture; it waits for a full-loop capture.

## Quests

Kryta has no quest code (`game/Quests/mod.rs` is an empty stub); its quest work so far is the Lua prototype in `pseudo_lua/presearing quests/`. ASURA has one quest working end to end (quest 80) plus the content for 40 more. Quests depend on NPC dialogs, so the Rust work that needs no NPC goes first and the rest waits for stage N6.

### What Kryta has

- 72 pre-Searing quest files, each linked to the NPC records that give and take it, plus `log_wire.lua` (the log's wire text).
- A prototype runtime (`pseudo_lua/runtime.lua`) with ASURA's stages (active, completed, rewarded) and credits for talk, deliver, escort and kill steps. Its smoke test plays Message from a Friend (100 xp, 10 gold) and The Worm Problem (250 xp).
- Experience is already ported, so experience rewards have somewhere to land.

### What ASURA has (`docs/58-quests.md`, `quest.rs`)

- **Stage rule** (`asura_rules::quest`): active, completed, rewarded, or abandoned; reacceptance only from abandoned (or rewarded if repeatable); every transition names its attempt cycle so a late packet changes nothing.
- **Log messages:** `0x0049` add, `0x004C` details, `0x004D` marker, `0x0052` remove, `0x0054` objectives; add always before details. Layouts from preforged's 2022 fixtures, each opcode one higher on this build.
- **Content:** `config/World/Vanilla/quests.json`, 38 quests from preforged's catalogue and capture, 40 with an offer and a reward. The character's name inside quest text is substituted at send time.
- **State:** one row per character and quest (migration `0038_character_quests.sql`), restored into the log at load.
- **Working:** quest 80 end to end; talk objectives for quests 55, 62 and 79; skills taught on accept or turn-in.
- **Not built in ASURA:** kill, item, escort and timer objectives (turn-in opens on accept as a shortcut); item rewards; second-profession unlocks; the skill-point message; per-map markers.

### Plan

1. **Q1 Wire.** Port the five log builders with their fixture tests into `game/Server/Packets/QuestPackets.rs`.
2. **Q2 Rule.** Port the stage rule into `game/Quests/`. It is pure, with no NPC or database dependency.
3. **Q3 Content and state.** Pack file `config/World/Vanilla/quests.json` (classified in `04-configuration.md`); a schema-only `character_quest` table under `data/sql/base/db_characters/`.
4. **Q4 Load restore.** Send each stored quest at load (add, then details). **First client proof:** a quest set with a command draws in the log after a relog.
5. **Q5 Offer, accept, turn-in.** Through dialog choices, after NPC stage N6. **Client proof:** quest 80 end to end.
6. **Q6 Rewards and talk objectives.** Experience (ported), gold, skill points; talk credits for 55, 62, 79. Skill teaching waits for the skills subject.

### Where `pseudo_lua` fits

`pseudo_lua`'s 72 quests, with their kill, deliver and escort steps, are the goal in concept; the quest files and runtime can be rewritten freely. ASURA's captured offers, turn-ins and log text (38 to 48 quests) are the evidence-backed reference for reaching it. ASURA's shortcut of opening every turn-in on accept is not carried: a quest with an unmodelled objective is not offered until its objective has evidence behind it. Kill, deliver and escort credits are built as captures of each become available.

**Open in ASURA too:** which of the Town Crier's two captured buttons is the acceptance; ASURA uses the first and expects the client to confirm it.

## Order of work

The proposed order follows dependencies: movement is re-checked first because everything walks on it, NPCs come next because dialogs, quests, vendors and combat all need a body to talk to or fight. Rule 0a says the owner picks the subject, so this is a proposal for the owner to confirm or reorder. Sizes are file sizes in the current ASURA snapshot.

| # | Subject | ASURA files | Size | Needs first |
| --- | --- | --- | --- | --- |
| 1 | Movement re-diff | `agent.rs`, `pathfind/directional.rs` | 64 KB + 12 KB | none |
| 2 | NPCs N1-N5 (static, patrols, armed) | `npc/` except `fight.rs` | about 180 KB | 1 |
| 3 | Quests Q1-Q4 (wire, rule, state, restore) | `quest.rs`, `asura-rules` quest | 70 KB | none; can run beside 2 |
| 4 | NPC N6: walk-to and dialogs | `approach.rs`, `dialog.rs` | 18 KB + 26 KB | 2 |
| 5 | Quests Q5-Q6 (offer, accept, reward) | `quest.rs` remainder | | 3, 4 |
| 6 | Item catalogue and modifiers; drop and pickup with their ground items; `/item` (all done 2026-11-22) - `/item clear` and the load block's diff remain | `item_catalogue.rs`, `item_modifier.rs`, `inventory_model.rs`, `item.rs` | 27 + 21 + 40 + 34 KB | none |
| 7 | Merchants, collectors, Xunlai, gold | `vendor/` | 77 KB | 4, 6 |
| 8 | Skills and combat | `skill/`, `npc/fight.rs` | about 250 KB | 2, 6 |
| 9 | Party remainder and compass relays | `party.rs` lines 1207-1826, `compass.rs` | 620 lines + 5 KB | none |
| 10 | Cartography | `cartography.rs` | 1,968 lines | 1 |
| 11 | Titles (on hold: apoguita is working on it; not in this plan) | `titles.rs` | 25 KB | 10 |

Each row runs through the nine-step round above, gets its own audit section, and ends with a client proof. From a subject ASURA marks `[~]` (built, untested), only the parts backed by a capture and a test are carried; the rest waits for evidence.

## Not ported, or owner decision first

These ASURA pieces are left out on purpose or need a decision recorded in `docs/decisions/` before any work, because they are about deployment rather than what the client sees.

| ASURA piece | Why it is not in the plan |
| --- | --- |
| World registry, heartbeats, leases, reaper, build selection (`asura-supervisor`, migrations 0002, 0010) | Kryta's realms are operator-managed; `12-research-and-control-panel.md` says this is not accepted. Owner decision. |
| Seat reservation on travel (`travel.rs:419-431`) | Part of the lease model above. Kryta's district allotment covers capacity. |
| Rolling deploy, two-container stack, CI workflow | Packaging and tooling, not port subjects. Owner decision. |
| Capture replay harness (`replay.rs`, `capture.rs`, `harness_off.rs`) | Kryta uses `recordings/` and its `--ignored` end-to-end tests instead. |
| On-demand map loading switch | Both servers preload by default; not a gap. |
| Features ASURA itself lacks (`Preforged features not yet in ASURA.csv`: loot, ID and salvage kits, trade, emotes, friends, death penalty, ...) | Rule 0 has no source to port. Each needs ASURA to build it first, or an owner ruling. |

## Open questions for the owner

- [ ] **Order.** Confirm or reorder the eleven subjects above (Rule 0a: the owner picks).
- [ ] **Lua timing.** Build the Lua runtime (`game/Scripting/`, scripting plan step 2) before NPC stage N4, or run N4 on the built-in behaviours first?
- [ ] **Movement numbers.** Correction limit (100 / 150 / 250 / higher) and the interact stop range (50 is unmeasured).
- [ ] **Directional walker.** Carry ASURA's provisional version as is, or hold it until ASURA's owner accepts it?
- [ ] **First-seen positions.** `pseudo_lua`'s 114 "seen live" placements are a body's first position in one visit, not a proven spawn point. Is that enough evidence to place them?
- [ ] **Spawn planes for maps 119 and 642.** Is a measured plane available, or do these maps wait for a capture?
