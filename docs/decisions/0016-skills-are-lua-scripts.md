# 0016 - What a skill does is a Lua script

## The reference

ASURA_testfork declares each skill's behaviour as data: a `skills.json` row carries an `effects` list
of operations (`attack`, `damage`, `heal`, `condition`, `effect`, `field`, `chain`, ...), read by
`crates/asura-world/src/skill/catalog.rs` (`read_op`) and run by `skill/engine.rs` (`complete`,
engine.rs:1018-1020, and `run`, 1152-1436). Preforged does the same in C++ from its own tables.

## This tree

The owner's decision (2026-11-23): skills are implemented as Lua scripts, not as declared op lists.

## The decision

Each `skills.json` row names a `script` (`skills/<name>.lua`) in place of `effects`. The script returns
`{ on_use = function(cast) ... end }`. When the skill completes, the engine builds a read-only `CastView`
(the skill, its rank, the rank-read `scale`/`bonus`/`duration`, and snapshots of the caster and the
target) and asks the host (`game/Scripting/SkillScripts.rs`). The script queues operations -
`cast:heal{ to = "target", amount = "scale" }`, nested ones built with `op.<word>{...}` - and the host
reads each through ASURA's own `read_op`. The engine then runs them exactly as ASURA runs its declared
ones.

So the split is: **the script decides what happens; the ported engine decides how.** Energy, casting,
aftercast, recharge, adrenaline, armor, damage formulas, conditions, effects, fields, chains, deaths,
regeneration and every message stay ASURA's, line for line, and ASURA's 26 engine tests still pass
unchanged. A script can branch on the cast (`if cast.target.hexed then ...`), which a declared list
cannot.

The 44 committed scripts are one-to-one translations of ASURA's 44 `effects` lists, so the pack does
today exactly what ASURA's does. A row whose list held an `unsupported` op (pets, corpses and minions)
keeps that one op under `effects`: the cast is refused at admission, before the script is asked, as
ASURA refuses it.

The Lua state is sandboxed (`table`, `string`, `math` only) and each call has an instruction budget; a
script fault or a runaway script completes the cast doing nothing, releases the slot (`0x00E3`), and is
kept for the log (`Engine::take_script_faults`). A row without `script` still runs its `effects`, so the
reference's path is never removed.

## What it is not

It is not a protocol change: no message, field or order differs from ASURA's. It is not a scripting
surface for the engine's internals: a script cannot write health, energy or timers directly, only queue
the operations the engine already has. A new kind of operation is still a Rust change in `Model.rs`,
`Catalog.rs` and `Engine.rs`, and its word is added to `OP_WORDS`.
