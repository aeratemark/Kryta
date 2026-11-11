# Rule coverage: where each of the owner's rules stands in this tree

The owner's ledger (`docs/game-rules.md`) is what this port is measured against: 58 asserted rules. This
file answers, for each one, one question - what is there today? It is a statement of today, and it is
updated in the same commit as the change that moves a rule.

The states are:

- **in** - implemented and proven (the test or the live run is named).
- **data** - the rows or the setting exist, and the behaviour that consumes them waits on a capture or
  on a later step.
- **blocked** - the next piece needs a measurement this tree does not have; `docs/port/06-world-plan.md`
  says which piece and why it is not invented.
- **not started** - nothing exists.

## Districts and instances

| rule | state | where, and what proves it |
|---|---|---|
| R1 a map is a definition, an instance is a running copy | in | `game/Districts/`: `Districts::enter` opens and hands out a copy; `game/Maps/` holds the definition |
| R2 outposts are shared copies | in | the same copy is handed out while it has room (`a_filled_district_is_left_for_the_first_with_room`) |
| R3 explorables are private per party | in | with no parties, one connection is one party, so each entry opens a copy (`an_explorable_gets_a_copy_of_its_own`) |
| R4 an outpost has numbered districts | in | copies are numbered and the load reports the number in `0x019A` |
| R5 a new district opens at the allotment | in | `World.DistrictAllotment` (100) and the first-with-room rule; `a_filled_district_is_left_for_the_first_with_room` |
| R16 one district is enough to begin | in | the first entry opens district 1 and the login answers the district list with exactly one |
| R22 a joining character takes the first district with room | in | `Districts::enter` with no named district |
| R57 a named district is honoured | in | `enter` with a hint opens or joins that number; `a_named_district_is_honoured_even_before_it_exists` |
| R58 a full district cannot be entered | data | `enter` refuses and the connection closes; the refusal's own wire is not captured |
| R37/R38/R39/R40 teardown and the disconnect hold | in | an empty copy is held 900 s and disposed by the world's tick; a socket that ends is a disconnect (`a_copy_is_held_for_the_hold_and_then_disposed`). The reconnection itself is not built |
| R32 an instance lives while a party is inside | in | the copy is held after it empties and disposed by the sweep |
| R33 ticking is configurable | in | `World.InstanceTick` drives the world's sweep and the world's status says whether it ticks |
| R20/R12 world-map travel reaches unlocked outposts; unlocks gate travel | data | `character_unlock` rows with the starting unlock written at the confirm; the gate is applied by the travel answer, which is blocked |
| R21 a portal crossing lands on the destination's arrival point | data | `map_arrival` read into the world; the crossing itself is blocked |
| R13 entry takes a random spawn point, scattered | in | `WorldSession::entry_point` and the load's `0x0196`; `An outpost connection is loaded with its entry point` |
| R14 a load places the character at its last outpost | in | the load writes `last_outpost` on the character's row; the login's list reports it |

## The travel unit: party, heroes, henchmen

| rule | state | where, and what proves it |
|---|---|---|
| R6, R7, R8, R9, R10 the party travels as one, the leader drives it | not started | `game/Parties/` is a stub |
| R17, R18, R19 party persistence and leadership | not started | the same |
| R23, R24, R25, R27, R28 heroes and henchmen | not started | `game/Heroes/` is a stub |
| R15 the party cap is not a constant | data | `World.PartySizeCap` (8) and `map.party_cap` read by `DatabaseMaps::party_cap`; nothing applies it yet |
| R26 an arrival at a smaller cap truncates the party | not started | needs parties |

## Characters, skills, movement

| rule | state | where, and what proves it |
|---|---|---|
| R29, R30, R31 what is in a map before anyone arrives | blocked | the agent-creation stream is not captured; the spawn rows for agents do not exist |
| R34 the compass range gates spawns and ticks | blocked | needs agents; the range is a setting (`World.CompassRange` 10,000) |
| R43 aggro is a bubble | not started | needs agents |
| R44 death and the death penalty | not started | `game/Combat/` is a stub |
| R46 pre-Searing level convention | not started | no levelling beyond the row's `level` 1 |
| R48 level 20 and experience | not started | `characters.level`/`experience` columns exist; nothing changes them |
| R50 skills: quests, trainers, capture | not started | `game/Skills/` is a stub |
| R51 attribute points from level-ups | not started | |
| R55 a quest affects only the log | not started | `game/Quests/` is a stub |
| R45 gold is per character; the bank holds gold and materials | data | `characters.gold` written 0 at creation; no bank rows |
| R49 Xunlai storage is per account | not started | |

## Items and commerce

| rule | state | where, and what proves it |
|---|---|---|
| R45 items a character holds are the character's | in | `item_instance` rows written at the confirm, drawn by the load (`a_creation_confirm_writes_a_character_row`) |
| moving, wearing and destroying an item | in | `0x8072`/`0x8030`/`0x8069` move or delete the row and answer with the change (`a_travel_record_decides_the_map_and_the_district`) |
| item definitions are world data | in | `item_template` seeded at start-up and read back (`definitions_are_seeded_once_and_read_back`) |
| R54 merchant NPCs hold a fixed list | not started | `game/Trader/` is a stub |

## Missions, quests, chat, guilds, titles

| rule | state | where, and what proves it |
|---|---|---|
| R52 missions are instances with success/failure | not started | |
| R56 a mission outpost presents the Enter Mission button | data | `World.MissionButton` setting; the button is the client's |
| R55 quests | not started | |
| the chat and social wire | not started | `game/Chat/` is a stub |
| guilds, friends, alliance | not started | `game/Guilds/` is a stub |
| titles and character slots | not started | |

## Every number

| rule | state | where, and what proves it |
|---|---|---|
| R53 every number and threshold stays configurable, with the game's value as the default | in | `game/World/WorldConfig.rs` reads them from `worldserver.json`'s `World` section and the program logs each one at start-up |
